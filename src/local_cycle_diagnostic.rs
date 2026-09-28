//! Canonicalize.Expression's local SCC diagnostics, including destructuring edges.
use crate::{
    ast::{Declaration, ExprId, Pattern, Syntax},
    names::{Binding, Resolved},
};
use std::collections::{BTreeMap, BTreeSet};

pub fn error(
    ast: &Syntax<'_>,
    resolved: &Resolved,
    definitions: &[&Declaration<'_>],
) -> Option<String> {
    struct Node<'a> {
        name: Option<&'a str>,
        invalid: bool,
        body: Option<ExprId>,
        edges: BTreeSet<String>,
    }
    let mut nodes = BTreeMap::new();
    for definition in definitions {
        match definition {
            Declaration::Value {
                name,
                body,
                arguments,
            } => {
                nodes.insert(
                    name.to_string(),
                    Node {
                        name: Some(*name),
                        invalid: arguments.is_empty(),
                        body: Some(*body),
                        edges: BTreeSet::new(),
                    },
                );
            }
            Declaration::Destruct { pattern, body } => {
                let mut names = std::collections::VecDeque::new();
                let mut pending = vec![*pattern];
                while let Some(id) = pending.pop() {
                    match &ast.patterns[id.0 as usize].kind {
                        Pattern::Var(name) | Pattern::Alias(_, name) => names.push_front(*name),
                        Pattern::Record(fields) => {
                            for name in fields.iter().rev() {
                                names.push_front(*name);
                            }
                        }
                        _ => {}
                    }
                    pending.extend(
                        crate::infer::pattern_children(&ast.patterns[id.0 as usize].kind)
                            .into_iter()
                            .rev(),
                    );
                }
                let key = format!("_M${}", names.front().copied().unwrap_or(""));
                for name in names {
                    nodes.insert(
                        name.to_string(),
                        Node {
                            name: Some(name),
                            invalid: true,
                            body: None,
                            edges: BTreeSet::from([key.clone()]),
                        },
                    );
                }
                nodes.insert(
                    key,
                    Node {
                        name: None,
                        invalid: false,
                        body: Some(*body),
                        edges: BTreeSet::new(),
                    },
                );
            }
            _ => {}
        }
    }
    // Names alone would confuse this let with outer or nested scopes. Restrict
    // references to the identities bound by these particular declarations.
    let mut bindings = BTreeMap::new();
    for definition in definitions {
        match definition {
            Declaration::Value { name, body, .. } => {
                if let Some(id) = resolved.definitions.get(body) {
                    bindings.insert(*id, *name);
                }
            }
            Declaration::Destruct { pattern, .. } => {
                let mut pending = vec![*pattern];
                while let Some(id) = pending.pop() {
                    if let Some(items) = resolved.pattern_bindings.get(&id) {
                        for (name, local) in items {
                            bindings.insert(*local, name.as_str());
                        }
                    }
                    pending.extend(crate::infer::pattern_children(
                        &ast.patterns[id.0 as usize].kind,
                    ));
                }
            }
            _ => {}
        }
    }
    for node in nodes.values_mut() {
        if let Some(body) = node.body {
            let mut pending = vec![body];
            while let Some(id) = pending.pop() {
                if let Some(Binding::Local(local)) = resolved.expressions[id.0 as usize]
                    && let Some(name) = bindings.get(&local)
                {
                    node.edges.insert((*name).to_owned());
                }
                pending.extend(crate::infer::expr_children(
                    &ast.expressions[id.0 as usize].kind,
                ));
            }
        }
    }
    let ranks: BTreeMap<_, _> = nodes
        .keys()
        .enumerate()
        .map(|(i, key)| (key.as_str(), i))
        .collect();
    let mut transpose = vec![BTreeSet::new(); nodes.len()];
    for (index, node) in nodes.values().enumerate() {
        for edge in &node.edges {
            if let Some(target) = ranks.get(edge.as_str()) {
                transpose[*target].insert(index);
            }
        }
    }
    let nodes: Vec<_> = nodes.values().collect();
    for group in crate::infer::components(&transpose).into_iter().rev() {
        if group.len() == 1 && !transpose[group[0]].contains(&group[0]) {
            continue;
        }
        // Data.Graph flattens each SCC with ascending outgoing edges. The
        // generic component finder uses stack order, which differs for a
        // destructuring expression with several bound names.
        let members: BTreeSet<_> = group.iter().copied().collect();
        let mut pending = vec![group[0]];
        let mut visited = BTreeSet::new();
        let mut group = Vec::new();
        while let Some(index) = pending.pop() {
            if !members.contains(&index) || !visited.insert(index) {
                continue;
            }
            group.push(index);
            pending.extend(
                nodes[index]
                    .edges
                    .iter()
                    .rev()
                    .filter_map(|key| ranks.get(key.as_str()).copied()),
            );
        }
        if let Some(pivot) = group.iter().position(|index| nodes[*index].invalid) {
            let name = nodes[group[pivot]].name?;
            let others: Vec<_> = group[pivot + 1..]
                .iter()
                .chain(group[..pivot].iter())
                .filter_map(|index| nodes[*index].name)
                .collect();
            return Some(crate::source_error::locate_slice(
                ast.source,
                name,
                crate::name_diagnostic::local_cycle(name, &others),
            ));
        }
    }
    None
}
