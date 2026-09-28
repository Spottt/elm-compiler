//! REPL type rendering with alias and source-variable names, using Elm's
//! grouped and aligned layout at 80 columns. No type is silently truncated.
use crate::{repl_doc::Doc, type_localizer::Localizer};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub fn render(graph: &Value, root: usize, names: &Localizer) -> Result<String, String> {
    let nodes = graph["nodes"].as_array().ok_or("missing type nodes")?;
    let root = index(&graph["roots"][root])?;
    let (variables, taken) = source_names(graph, nodes, root)?;
    Renderer {
        nodes,
        names,
        variables,
        taken,
        counts: BTreeMap::new(),
        active: BTreeSet::new(),
    }
    .node(root, 0)
    .map(|doc| doc.render(80))
}

// Elm reserves existing names in reverse child order before allocating fresh
// variables. This ensures a generated `a` cannot hide a source variable `a`.
fn source_names(
    graph: &Value,
    nodes: &[Value],
    root: usize,
) -> Result<(BTreeMap<usize, String>, BTreeSet<String>), String> {
    let mut names = BTreeMap::new();
    let mut taken = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let node = nodes.get(id).ok_or("missing type node")?;
        if let Some(name) = graph["variable_names"][id.to_string()].as_str() {
            let mut candidate = name.to_string();
            let mut suffix = 0;
            while !taken.insert(candidate.clone()) {
                suffix += 1;
                candidate = format!("{name}{suffix}");
            }
            names.insert(id, candidate);
        }
        match node[0].as_str() {
            Some("function") => pending.extend([index(&node[1])?, index(&node[2])?]),
            Some("tuple" | "named" | "alias") => {
                let slot = if node[0] == "tuple" { 1 } else { 2 };
                for child in node[slot].as_array().ok_or("invalid type children")? {
                    pending.push(index(child)?);
                }
            }
            Some("record") => {
                if !node[2].is_null() {
                    pending.push(index(&node[2])?);
                }
                for child in node[1].as_object().ok_or("invalid record fields")?.values() {
                    pending.push(index(child)?);
                }
            }
            _ => {}
        }
    }
    Ok((names, taken))
}

fn index(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid type node index".into())
}

struct Renderer<'a> {
    nodes: &'a [Value],
    names: &'a Localizer,
    variables: BTreeMap<usize, String>,
    counts: BTreeMap<String, usize>,
    taken: BTreeSet<String>,
    active: BTreeSet<usize>,
}

impl Renderer<'_> {
    // Precedence: function 0, named application 1, atom 2.
    fn node(&mut self, id: usize, context: u8) -> Result<Doc, String> {
        if self.active.len() >= 1024 || !self.active.insert(id) {
            return Err("recursive or excessively deep REPL type".into());
        }
        let node = self.nodes.get(id).ok_or("missing type node")?;
        let (text, precedence) = match node[0].as_str().ok_or("missing type node tag")? {
            "variable" | "rigid" => {
                let constraint = node[2].as_str().ok_or("missing type constraint")?;
                if !["any", "number", "comparable", "appendable", "compappend"]
                    .contains(&constraint)
                {
                    return Err("invalid type constraint".into());
                }
                let name = self
                    .variables
                    .entry(id)
                    .or_insert_with(|| {
                        let count = self.counts.entry(constraint.into()).or_default();
                        loop {
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
                        }
                    })
                    .clone();
                (Doc::text(name), 2)
            }
            "unit" => (Doc::text("()"), 2),
            "function" => {
                let mut chain = Vec::new();
                let mut cursor = id;
                let mut seen = BTreeSet::new();
                loop {
                    if !seen.insert(cursor) {
                        return Err("recursive function type".into());
                    }
                    let current = self.nodes.get(cursor).ok_or("missing function result")?;
                    if current[0] != "function" {
                        chain.push(self.node(cursor, 1)?);
                        break;
                    }
                    chain.push(self.node(index(&current[1])?, 1)?);
                    cursor = index(&current[2])?;
                }
                let parts = chain
                    .into_iter()
                    .enumerate()
                    .map(|(i, part)| {
                        if i == 0 {
                            part
                        } else {
                            Doc::concat(vec![Doc::text("-> "), part])
                        }
                    })
                    .collect();
                (Doc::sep(parts).align(), 0)
            }
            "tuple" => {
                let values = node[1]
                    .as_array()
                    .ok_or("invalid tuple")?
                    .iter()
                    .map(|n| self.node(index(n)?, 0))
                    .collect::<Result<Vec<_>, _>>()?;
                let entries = values
                    .into_iter()
                    .enumerate()
                    .map(|(i, value)| {
                        Doc::concat(vec![Doc::text(if i == 0 { "( " } else { ", " }), value])
                    })
                    .collect();
                (Doc::sep(vec![Doc::cat(entries), Doc::text(")")]).align(), 2)
            }
            "named" | "alias" => {
                let (module, name): (String, String) =
                    serde_json::from_str(node[1].as_str().ok_or("invalid type name")?)
                        .map_err(|e| e.to_string())?;
                let args = node[2].as_array().ok_or("invalid type arguments")?;
                let mut parts = vec![Doc::text(self.names.name(&module, &name))];
                for arg in args {
                    parts.push(self.node(index(arg)?, 2)?);
                }
                (Doc::sep(parts).hang(4), if args.is_empty() { 2 } else { 1 })
            }
            "record" => {
                let mut fields = BTreeMap::new();
                let mut row = id;
                let mut seen = BTreeSet::new();
                let extension = loop {
                    if !seen.insert(row) {
                        return Err("recursive record row".into());
                    }
                    let record = self.nodes.get(row).ok_or("missing record row")?;
                    if record[0] == "alias" {
                        row = index(&record[3])?;
                        continue;
                    }
                    if record[0] != "record" {
                        break Some(row);
                    }
                    for (name, value) in record[1].as_object().ok_or("invalid record fields")? {
                        fields.insert(name.clone(), index(value)?);
                    }
                    if record[2].is_null() {
                        break None;
                    }
                    row = index(&record[2])?;
                };
                let fields = fields
                    .into_iter()
                    .map(|(name, id)| {
                        Ok(
                            Doc::sep(vec![Doc::text(format!("{name} :")), self.node(id, 0)?])
                                .hang(4),
                        )
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let text = match extension {
                    Some(row) => {
                        let extension = self.node(row, 0)?;
                        let entries = fields
                            .into_iter()
                            .enumerate()
                            .map(|(i, field)| {
                                Doc::concat(vec![
                                    Doc::text(if i == 0 { "| " } else { ", " }),
                                    field,
                                ])
                            })
                            .collect();
                        Doc::sep(vec![
                            Doc::sep(vec![
                                Doc::concat(vec![Doc::text("{ "), extension]),
                                Doc::cat(entries),
                            ])
                            .hang(4),
                            Doc::text("}"),
                        ])
                        .align()
                    }
                    None if fields.is_empty() => Doc::text("{}"),
                    None => {
                        let entries = fields
                            .into_iter()
                            .enumerate()
                            .map(|(i, field)| {
                                Doc::concat(vec![
                                    Doc::text(if i == 0 { "{ " } else { ", " }),
                                    field,
                                ])
                            })
                            .collect();
                        Doc::sep(vec![Doc::cat(entries), Doc::text("}")]).align()
                    }
                };
                (text, 2)
            }
            _ => return Err("unknown type node".into()),
        };
        self.active.remove(&id);
        Ok(if precedence < context {
            Doc::cat(vec![Doc::text("("), text, Doc::text(")")])
        } else {
            text
        })
    }
}
