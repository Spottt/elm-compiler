//! Render a rejected occurs-check without constructing a cyclic inference graph.
use crate::{
    ast::{Span, Syntax},
    names::Symbols,
    type_localizer::Localizer,
    unify::{Engine, Ty},
};
use serde_json::json;

pub(crate) fn capture(
    ast: &Syntax<'_>,
    symbols: &Symbols,
    engine: &mut Engine,
    cycles: &[(Ty, Ty)],
    root: Ty,
    name: &str,
    span: Span,
) -> Option<String> {
    let roots: Vec<_> = std::iter::once(root)
        .chain(
            cycles
                .iter()
                .flat_map(|(variable, value)| [*variable, *value]),
        )
        .collect();
    let graph = engine
        .export_types(&roots, |id| {
            let symbol = symbols.get(id);
            Ok(json!([symbol.module.as_ref(), symbol.name.as_ref()]).to_string())
        })
        .ok()?;
    let substitutions: Option<std::collections::BTreeMap<_, _>> = (0..cycles.len())
        .map(|index| {
            Some((
                graph["roots"][index * 2 + 1].as_u64()? as usize,
                graph["roots"][index * 2 + 2].as_u64()? as usize,
            ))
        })
        .collect();
    let graph = unfold(&graph, substitutions?)?;
    let tipe = crate::repl_type::render_width(
        &graph,
        0,
        &Localizer::from_header(&ast.header, "application"),
        76,
    )
    .ok()?;
    let start = crate::docs_diagnostic::position(ast.source, span.start)?;
    let end = crate::docs_diagnostic::position(ast.source, span.end)?;
    let mut message = crate::docs_diagnostic::snippet_positions(
        ast.source,
        start,
        end,
        &format!("I am inferring a weird self-referential type for {name}:"),
    )?;
    crate::docs_diagnostic::text(
        &mut message,
        format!(
            "\n{}\n\n    ",
            crate::docs_diagnostic::reflow(
                "Here is my best effort at writing down the type. You will see ∞ for parts of the type that repeat something already printed out infinitely."
            )
        ),
    );
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":tipe.replace('\n',"\n    ")}));
    crate::docs_diagnostic::text(
        &mut message,
        format!(
            "\n\n{}",
            crate::docs_diagnostic::reflow(
                "Staring at this type is usually not so helpful, so I recommend reading the hints at <https://elm-lang.org/0.19.1/infinite-type> to get unstuck!"
            )
        ),
    );
    Some(crate::docs_diagnostic::encode(
        &json!({"type":"compile-errors","errors":[{"path":"","name":ast.header.name,"problems":[{"title":"INFINITE TYPE","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    ))
}

// Only the detached diagnostic graph may be cyclic. Expand each occurrence
// with its own ancestor set, keeping shared variables shared for naming.
fn unfold(
    graph: &serde_json::Value,
    substitutions: std::collections::BTreeMap<usize, usize>,
) -> Option<serde_json::Value> {
    use std::collections::{BTreeMap, BTreeSet};
    struct Expansion<'a> {
        source: &'a serde_json::Value,
        substitutions: BTreeMap<usize, usize>,
        taken: BTreeSet<String>,
        counts: BTreeMap<String, usize>,
        nodes: Vec<serde_json::Value>,
        names: serde_json::Map<String, serde_json::Value>,
        leaves: BTreeMap<usize, usize>,
        active: BTreeSet<usize>,
    }
    impl Expansion<'_> {
        fn visit(&mut self, id: usize) -> Option<usize> {
            let id = self.substitutions.get(&id).copied().unwrap_or(id);
            if self.nodes.len() >= 10000 || self.active.len() >= 128 {
                return None;
            }
            if self.active.contains(&id) {
                let index = self.nodes.len();
                self.nodes.push(json!(["infinite"]));
                return Some(index);
            }
            if let Some(index) = self.leaves.get(&id) {
                return Some(*index);
            }
            let mut node = self.source["nodes"][id].clone();
            let leaf = matches!(node[0].as_str()?, "variable" | "rigid" | "unit");
            let index = self.nodes.len();
            self.nodes.push(serde_json::Value::Null);
            self.active.insert(id);
            match node[0].as_str()? {
                "function" => {
                    node[1] = json!(self.visit(node[1].as_u64()? as usize)?);
                    node[2] = json!(self.visit(node[2].as_u64()? as usize)?);
                }
                "named" | "alias" | "tuple" => {
                    let slot = if node[0] == "tuple" { 1 } else { 2 };
                    for child in node[slot].as_array_mut()? {
                        *child = json!(self.visit(child.as_u64()? as usize)?);
                    }
                    if node[0] == "alias" {
                        node[3] = json!(self.visit(node[3].as_u64()? as usize)?);
                    }
                }
                "record" => {
                    for child in node[1].as_object_mut()?.values_mut() {
                        *child = json!(self.visit(child.as_u64()? as usize)?);
                    }
                    if !node[2].is_null() {
                        node[2] = json!(self.visit(node[2].as_u64()? as usize)?);
                    }
                }
                _ => {}
            }
            self.active.remove(&id);
            if leaf {
                self.leaves.insert(id, index);
                if let Some(name) = self.source["variable_names"][id.to_string()].as_str() {
                    self.names.insert(index.to_string(), json!(name));
                } else if matches!(node[0].as_str(), Some("variable" | "rigid")) {
                    let constraint = node[2].as_str()?;
                    let count = self.counts.entry(constraint.into()).or_default();
                    let name = loop {
                        let name = if constraint == "any" {
                            format!(
                                "{}{}",
                                (b'a' + (*count % 26) as u8) as char,
                                if *count < 26 {
                                    String::new()
                                } else {
                                    (*count / 26).to_string()
                                }
                            )
                        } else {
                            format!(
                                "{constraint}{}",
                                if *count == 0 {
                                    String::new()
                                } else {
                                    count.to_string()
                                }
                            )
                        };
                        *count += 1;
                        if self.taken.insert(name.clone()) {
                            break name;
                        }
                    };
                    self.names.insert(index.to_string(), json!(name));
                }
            }
            self.nodes[index] = node;
            Some(index)
        }
    }
    let mut expansion = Expansion {
        source: graph,
        substitutions,
        taken: graph["variable_names"]
            .as_object()
            .into_iter()
            .flat_map(|names| {
                names
                    .values()
                    .filter_map(|name| name.as_str().map(str::to_string))
            })
            .collect(),
        counts: BTreeMap::new(),
        nodes: Vec::new(),
        names: Default::default(),
        leaves: Default::default(),
        active: Default::default(),
    };
    // Keep the variable names of the most recent cyclic constraint when
    // displaying the enclosing binding, including other record fields.
    let seed = graph["roots"].as_array()?.last()?.as_u64()? as usize;
    expansion.visit(seed)?;
    let root = expansion.visit(graph["roots"][0].as_u64()? as usize)?;
    Some(json!({"nodes":expansion.nodes,"roots":[root],"variable_names":expansion.names}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfolding_cuts_cycles_but_preserves_shared_type_variables() {
        let graph = json!({"roots":[0],"nodes":[
            ["function",1,2], ["tuple",[3,2]],
            ["variable",1,"any"], ["variable",1,"any"]
        ]});
        let expanded = unfold(&graph, [(3, 0)].into()).unwrap();
        assert_eq!(
            crate::repl_type::render(&expanded, 0, &Localizer::default()).unwrap(),
            "( ∞, a ) -> a"
        );
        assert_eq!(graph["nodes"][3][0], "variable");
    }
}
