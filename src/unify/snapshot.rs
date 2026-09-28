//! Flat, portable type graphs for typed-module artifacts. Runtime Ty/SymbolId
//! values never cross the artifact boundary; named types use caller-owned keys.
use super::*;
use serde_json::{Value, json};

impl Engine {
    /// Only closed schemes can be detached from their original module graph.
    pub fn is_closed_scheme(&mut self, scheme: &Scheme) -> bool {
        let quantified: BTreeSet<_> = scheme.quantified.iter().map(|ty| self.find(*ty)).collect();
        let mut pending = vec![scheme.root];
        let mut seen = BTreeSet::new();
        while let Some(ty) = pending.pop() {
            let ty = self.find(ty);
            if !seen.insert(ty) {
                continue;
            }
            match &self.nodes[ty.0 as usize].descriptor {
                Descriptor::Variable { .. } if quantified.contains(&ty) => {}
                Descriptor::Variable { .. } | Descriptor::Rigid { .. } => return false,
                Descriptor::Structure(term) => pending.extend(Self::children(term)),
            }
        }
        true
    }

    pub fn export_types(
        &mut self,
        roots: &[Ty],
        mut symbol_name: impl FnMut(SymbolId) -> Result<String, String>,
    ) -> Result<Value, String> {
        let mut ids = HashMap::new();
        let mut live = Vec::new();
        let mut pending = roots.to_vec();
        while let Some(old) = pending.pop() {
            let old = self.find(old);
            if ids.contains_key(&old) {
                continue;
            }
            ids.insert(old, live.len());
            live.push(old);
            if let Descriptor::Structure(term) = &self.nodes[old.0 as usize].descriptor {
                pending.extend(Self::children(term));
            }
        }
        let mut nodes = Vec::with_capacity(live.len());
        for old in live {
            let descriptor = self.nodes[old.0 as usize].descriptor.clone();
            let mut index = |ty| ids[&self.find(ty)];
            let node = match descriptor {
                Descriptor::Variable { level, constraint } => {
                    json!(["variable", level, constraint_name(constraint)])
                }
                Descriptor::Rigid { level, constraint } => {
                    json!(["rigid", level, constraint_name(constraint)])
                }
                Descriptor::Structure(term) => match &*term {
                    Term::Alias(symbol, args, real) => json!([
                        "alias",
                        symbol_name(*symbol)?,
                        args.iter().copied().map(&mut index).collect::<Vec<_>>(),
                        index(*real)
                    ]),
                    Term::Named(symbol, args) => json!([
                        "named",
                        symbol_name(*symbol)?,
                        args.iter().copied().map(&mut index).collect::<Vec<_>>()
                    ]),
                    Term::Function(a, b) => json!(["function", index(*a), index(*b)]),
                    Term::Unit => json!(["unit"]),
                    Term::Tuple(items) => json!([
                        "tuple",
                        items.iter().copied().map(&mut index).collect::<Vec<_>>()
                    ]),
                    Term::Record { fields, extension } => json!([
                        "record",
                        fields
                            .iter()
                            .map(|(name, ty)| (name.clone(), index(*ty)))
                            .collect::<BTreeMap<_, _>>(),
                        extension.map(&mut index)
                    ]),
                },
            };
            nodes.push(node);
        }
        let roots: Vec<_> = roots.iter().map(|ty| ids[&self.find(*ty)]).collect();
        let mut artifact = json!({"version":1,"nodes":nodes,"roots":roots});
        if let Some(names) = &self.display_names {
            let names: BTreeMap<_, _> = names
                .iter()
                .filter_map(|(ty, name)| {
                    ids.get(ty)
                        .filter(|id| matches!(nodes[**id][0].as_str(), Some("variable" | "rigid")))
                        .map(|id| (id.to_string(), name.clone()))
                })
                .collect();
            artifact["variable_names"] = json!(names);
        }
        Ok(artifact)
    }

    /// Validate the whole artifact before changing this engine. Corrupt caches
    /// must be recoverable misses, never panics or partially restored type state.
    pub fn import_types(
        &mut self,
        artifact: &Value,
        mut symbol: impl FnMut(&str) -> Result<SymbolId, String>,
    ) -> Result<Vec<Ty>, String> {
        let invalid = || "invalid type graph artifact".to_string();
        if artifact.get("version").and_then(Value::as_u64) != Some(1) {
            return Err(invalid());
        }
        let nodes = artifact
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        let offset = u32::try_from(self.nodes.len()).map_err(|_| invalid())?;
        let count = u32::try_from(nodes.len()).map_err(|_| invalid())?;
        offset.checked_add(count).ok_or_else(invalid)?;
        let index = |value: &Value| -> Result<Ty, String> {
            let id = value
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| *n < count)
                .ok_or_else(invalid)?;
            Ok(Ty(id))
        };
        let indices = |value: &Value| -> Result<Vec<Ty>, String> {
            value
                .as_array()
                .ok_or_else(invalid)?
                .iter()
                .map(&index)
                .collect()
        };
        let roots = indices(artifact.get("roots").ok_or_else(invalid)?)?;
        let mut descriptors = Vec::with_capacity(nodes.len());
        let mut edges = Vec::with_capacity(nodes.len());
        for node in nodes {
            let parts = node.as_array().ok_or_else(invalid)?;
            let tag = parts.first().and_then(Value::as_str).ok_or_else(invalid)?;
            let descriptor = match (tag, parts.len()) {
                ("variable" | "rigid", 3) => {
                    let level = parts[1]
                        .as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or_else(invalid)?;
                    let constraint = parse_constraint(parts[2].as_str().ok_or_else(invalid)?)?;
                    if tag == "variable" {
                        Descriptor::Variable { level, constraint }
                    } else {
                        Descriptor::Rigid { level, constraint }
                    }
                }
                _ => {
                    let term = match (tag, parts.len()) {
                        ("alias", 4) => Term::Alias(
                            symbol(parts[1].as_str().ok_or_else(invalid)?)?,
                            indices(&parts[2])?,
                            index(&parts[3])?,
                        ),
                        ("named", 3) => Term::Named(
                            symbol(parts[1].as_str().ok_or_else(invalid)?)?,
                            indices(&parts[2])?,
                        ),
                        ("function", 3) => Term::Function(index(&parts[1])?, index(&parts[2])?),
                        ("unit", 1) => Term::Unit,
                        ("tuple", 2) => {
                            let items = indices(&parts[1])?;
                            if !(2..=3).contains(&items.len()) {
                                return Err(invalid());
                            }
                            Term::Tuple(items)
                        }
                        ("record", 3) => Term::Record {
                            fields: parts[1]
                                .as_object()
                                .ok_or_else(invalid)?
                                .iter()
                                .map(|(name, value)| Ok((name.clone(), index(value)?)))
                                .collect::<Result<_, String>>()?,
                            extension: if parts[2].is_null() {
                                None
                            } else {
                                Some(index(&parts[2])?)
                            },
                        },
                        _ => return Err(invalid()),
                    };
                    Descriptor::Structure(Rc::new(term))
                }
            };
            edges.push(match &descriptor {
                Descriptor::Structure(term) => Self::children(term),
                _ => vec![],
            });
            descriptors.push(descriptor);
        }
        // Iterative DFS rejects cycles even in unreachable nodes. The flat format
        // and this traversal also support deeply nested valid types safely.
        let mut colors = vec![0u8; nodes.len()];
        for root in 0..nodes.len() {
            let mut pending = vec![(root, false)];
            while let Some((node, finish)) = pending.pop() {
                if finish {
                    colors[node] = 2;
                    continue;
                }
                match colors[node] {
                    2 => continue,
                    1 => return Err(invalid()),
                    _ => {}
                }
                colors[node] = 1;
                pending.push((node, true));
                pending.extend(
                    edges[node]
                        .iter()
                        .rev()
                        .map(|child| (child.0 as usize, false)),
                );
            }
        }
        drop(edges);
        drop(colors);
        // Validate optional REPL metadata before mutating the destination.
        let mut display_names = BTreeMap::new();
        if let Some(names) = artifact.get("variable_names") {
            for (id, name) in names.as_object().ok_or_else(invalid)? {
                let id: usize = id.parse().map_err(|_| invalid())?;
                let name = name
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(invalid)?;
                if !matches!(
                    descriptors.get(id),
                    Some(Descriptor::Variable { .. } | Descriptor::Rigid { .. })
                ) {
                    return Err(invalid());
                }
                display_names.insert(Ty(offset + id as u32), name.to_string());
            }
        }
        let remap = |ty: Ty| Ty(offset + ty.0);
        for descriptor in descriptors {
            let descriptor = match descriptor {
                Descriptor::Structure(mut term) => {
                    // These terms belong exclusively to the validated artifact.
                    // Relocate in place instead of cloning every record field.
                    match Rc::make_mut(&mut term) {
                        Term::Alias(_, args, real) => {
                            for arg in args {
                                *arg = remap(*arg);
                            }
                            *real = remap(*real);
                        }
                        Term::Named(_, items) | Term::Tuple(items) => {
                            for item in items {
                                *item = remap(*item);
                            }
                        }
                        Term::Function(a, b) => {
                            *a = remap(*a);
                            *b = remap(*b);
                        }
                        Term::Unit => {}
                        Term::Record { fields, extension } => {
                            for ty in fields.values_mut() {
                                *ty = remap(*ty);
                            }
                            *extension = extension.map(remap);
                        }
                    }
                    Descriptor::Structure(term)
                }
                other => other,
            };
            self.node(descriptor);
        }
        if let Some(names) = &mut self.display_names {
            names.extend(display_names);
        }
        Ok(roots.into_iter().map(remap).collect())
    }
}
fn constraint_name(value: Constraint) -> &'static str {
    match value {
        Constraint::Any => "any",
        Constraint::Number => "number",
        Constraint::Comparable => "comparable",
        Constraint::Appendable => "appendable",
        Constraint::CompAppend => "compappend",
    }
}
fn parse_constraint(value: &str) -> Result<Constraint, String> {
    match value {
        "any" => Ok(Constraint::Any),
        "number" => Ok(Constraint::Number),
        "comparable" => Ok(Constraint::Comparable),
        "appendable" => Ok(Constraint::Appendable),
        "compappend" => Ok(Constraint::CompAppend),
        _ => Err("invalid type constraint in artifact".into()),
    }
}
