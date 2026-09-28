//! Portable inferred globals. Symbol identities are resolved against the freshly
//! parsed module graph; generated JavaScript and ASTs are deliberately excluded.
use crate::{
    names::{Interface, Space, SymbolId, Symbols},
    unify::{Engine, Scheme, Ty},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Types needed after a module cache hit: foreign references plus local main
/// validation and port conversion. Other expression types are not used by JS
/// generation; constructors and aliases are rebuilt from declarations.
pub fn required_globals(
    module: &str,
    ast: &crate::ast::Syntax<'_>,
    interface: &Interface,
    symbols: &Symbols,
) -> Result<BTreeSet<SymbolId>, String> {
    use crate::ast::Declaration;
    let mut required: BTreeSet<_> = interface
        .values
        .values()
        .chain(interface.operators.values())
        .copied()
        .collect();
    for declaration in &ast.declarations {
        let name = match declaration {
            Declaration::Value { name: "main", .. } => "main",
            Declaration::Port { name, .. } => name,
            _ => continue,
        };
        required.insert(
            symbols
                .lookup(module, name, Space::Value)
                .ok_or_else(|| format!("missing required cached value {name}"))?,
        );
    }
    Ok(required)
}

pub fn export(
    module: &str,
    symbols: &Symbols,
    engine: &mut Engine,
    globals: &BTreeMap<SymbolId, Scheme>,
) -> Result<Value, String> {
    export_filtered(module, symbols, engine, globals, None)
}

pub fn export_selected(
    module: &str,
    symbols: &Symbols,
    engine: &mut Engine,
    globals: &BTreeMap<SymbolId, Scheme>,
    required: &BTreeSet<SymbolId>,
) -> Result<Value, String> {
    for id in required {
        if !globals.contains_key(id) || symbols.get(*id).module.as_ref() != module {
            return Err("missing required inferred module scheme".into());
        }
    }
    export_filtered(module, symbols, engine, globals, Some(required))
}

fn export_filtered(
    module: &str,
    symbols: &Symbols,
    engine: &mut Engine,
    globals: &BTreeMap<SymbolId, Scheme>,
    required: Option<&BTreeSet<SymbolId>>,
) -> Result<Value, String> {
    let mut roots = Vec::new();
    let mut entries = Vec::new();
    for (id, scheme) in globals {
        let symbol = symbols.get(*id);
        if symbol.module.as_ref() != module || required.is_some_and(|set| !set.contains(id)) {
            continue;
        }
        if !engine.is_closed_scheme(scheme) {
            return Err("module contains an open inferred scheme".into());
        }
        let root = roots.len();
        roots.push(scheme.root);
        let quantified: Vec<_> = scheme
            .quantified
            .iter()
            .map(|ty| {
                let index = roots.len();
                roots.push(*ty);
                index
            })
            .collect();
        entries.push(json!({"name":symbol.name.as_ref(),"root":root,"quantified":quantified}));
    }
    let graph = engine.export_types(&roots, |id| {
        let s = symbols.get(id);
        Ok(serde_json::to_string(&(s.module.as_ref(), s.name.as_ref())).unwrap())
    })?;
    Ok(json!({"version":1,"module":module,"entries":entries,"graph":graph}))
}

pub fn import(
    artifact: &Value,
    module: &str,
    symbols: &Symbols,
    engine: &mut Engine,
) -> Result<BTreeMap<SymbolId, Scheme>, String> {
    let expected: BTreeSet<_> = symbols
        .entries
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.module.as_ref() == module && matches!(s.kind, crate::names::SymbolKind::Value)
        })
        .map(|(i, _)| SymbolId(i as u32))
        .collect();
    import_selected(artifact, module, symbols, engine, &expected)
}

pub fn import_selected(
    artifact: &Value,
    module: &str,
    symbols: &Symbols,
    engine: &mut Engine,
    expected: &BTreeSet<SymbolId>,
) -> Result<BTreeMap<SymbolId, Scheme>, String> {
    let invalid = || "invalid inferred module artifact".to_string();
    if artifact.get("version").and_then(Value::as_u64) != Some(1)
        || artifact.get("module").and_then(Value::as_str) != Some(module)
    {
        return Err(invalid());
    }
    let graph = artifact.get("graph").ok_or_else(invalid)?;
    let root_count = graph
        .get("roots")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?
        .len();
    let index = |value: &Value| {
        value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n < root_count)
            .ok_or_else(invalid)
    };
    let entries = artifact
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    let mut checked = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in entries {
        let name = entry
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        let id = symbols
            .lookup(module, name, Space::Value)
            .ok_or_else(invalid)?;
        if !seen.insert(id) {
            return Err(invalid());
        }
        let root = index(entry.get("root").ok_or_else(invalid)?)?;
        let quantified = entry
            .get("quantified")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?
            .iter()
            .map(&index)
            .collect::<Result<Vec<_>, _>>()?;
        checked.push((id, root, quantified));
    }
    if &seen != expected {
        return Err(invalid());
    }
    // All metadata is checked before importing nodes, so an invalid artifact
    // leaves the destination engine unchanged.
    let roots = engine.import_types(graph, |key| {
        let (owner, name): (String, String) = serde_json::from_str(key).map_err(|_| invalid())?;
        symbols
            .lookup(&owner, &name, Space::Type)
            .ok_or_else(invalid)
    })?;
    Ok(checked
        .into_iter()
        .map(|(id, root, quantified)| {
            (
                id,
                Scheme {
                    root: roots[root],
                    quantified: quantified
                        .into_iter()
                        .map(|index| roots[index])
                        .collect::<BTreeSet<Ty>>(),
                },
            )
        })
        .collect())
}
