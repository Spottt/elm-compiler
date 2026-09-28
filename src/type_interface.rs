//! Semantic dependency identity for inferred types. Runtime implementations are
//! deliberately excluded; parsing, coverage and code generation still run.
use crate::{
    fixity::{Associativity, Table},
    names::{Interface, SymbolId, SymbolKind, Symbols},
    types::Catalog,
    unify::{Engine, Scheme},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Called after all checks and code generation for a module. Other Elm modules
/// can only refer to exported values (including private operator implementations).
/// Removing handles lets the next engine compaction reclaim private type graphs.
pub fn retain_public_globals(
    module: &str,
    interface: &Interface,
    symbols: &Symbols,
    globals: &mut BTreeMap<SymbolId, Scheme>,
) {
    let public: std::collections::BTreeSet<_> = interface
        .values
        .values()
        .chain(interface.operators.values())
        .copied()
        .collect();
    globals.retain(|id, _| symbols.get(*id).module.as_ref() != module || public.contains(id));
}

fn add(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

pub fn fingerprint(
    interface: &Interface,
    operators: &Table,
    symbols: &Symbols,
    catalog: &Catalog,
    engine: &mut Engine,
    globals: &BTreeMap<SymbolId, Scheme>,
) -> Result<[u8; 32], String> {
    let identity = |id: SymbolId| {
        let symbol = symbols.get(id);
        Ok(serde_json::to_string(&(symbol.module.as_ref(), symbol.name.as_ref())).unwrap())
    };
    let mut hash = Sha256::new();
    add(&mut hash, b"elm-type-interface-v1");
    for (tag, entries) in [
        ("values", &interface.values),
        ("constructors", &interface.constructors),
        ("operators", &interface.operators),
    ] {
        add(&mut hash, tag.as_bytes());
        add(&mut hash, &(entries.len() as u64).to_le_bytes());
        for (name, id) in entries {
            add(&mut hash, name.as_bytes());
            let scheme = globals
                .get(id)
                .or_else(|| catalog.constructors.get(id))
                .ok_or_else(|| format!("missing interface scheme {name}"))?;
            add(&mut hash, &engine.fingerprint_scheme(scheme, identity)?);
            if tag == "operators" {
                let fixity = operators
                    .get(name)
                    .ok_or_else(|| format!("missing interface fixity {name}"))?;
                add(
                    &mut hash,
                    &[
                        fixity.precedence,
                        match fixity.associativity {
                            Associativity::Left => 0,
                            Associativity::Right => 1,
                            Associativity::Non => 2,
                        },
                    ],
                );
            }
        }
    }
    add(&mut hash, b"types");
    add(&mut hash, &(interface.types.len() as u64).to_le_bytes());
    for (name, id) in &interface.types {
        add(&mut hash, name.as_bytes());
        add(&mut hash, identity(*id)?.as_bytes());
        let SymbolKind::Type { arity, alias } = symbols.get(*id).kind else {
            return Err(format!("invalid interface type {name}"));
        };
        add(&mut hash, &(arity as u64).to_le_bytes());
        add(&mut hash, &[u8::from(alias)]);
        if alias {
            let alias = catalog
                .aliases
                .get(id)
                .ok_or_else(|| format!("missing interface alias {name}"))?;
            let mut roots = alias.parameters.clone();
            roots.push(alias.root);
            add(&mut hash, &engine.fingerprint_types(&roots, identity)?);
        }
    }
    Ok(hash.finalize().into())
}
