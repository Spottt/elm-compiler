//! Structural hashes independent of allocation order, inference levels and DAG
//! sharing. Ordered roots share one alpha-renaming scope (e.g. alias parameters
//! followed by its body). Call separately for independently quantified schemes.
use super::*;
use sha2::{Digest, Sha256};

fn add(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

/// The exclusive engine borrow prevents inference/compaction while these node
/// hashes are shared. Only variable-free subgraphs survive between root scopes.
pub(crate) struct Fingerprints<'engine, F> {
    engine: &'engine mut Engine,
    symbol_name: F,
    concrete: HashMap<Ty, [u8; 32]>,
}
impl Engine {
    pub(crate) fn fingerprints<F: FnMut(SymbolId) -> Result<String, String>>(&mut self, symbol_name: F) -> Fingerprints<'_, F> {
        Fingerprints { engine: self, symbol_name, concrete: HashMap::new() }
    }
    /// Exported values must be closed: free variables carry relationships to
    /// the surrounding inference state that a standalone hash cannot express.
    pub fn fingerprint_scheme(&mut self, scheme: &Scheme, symbol_name: impl FnMut(SymbolId) -> Result<String, String>) -> Result<[u8; 32], String> {
        self.fingerprints(symbol_name).scheme(scheme)
    }
    pub fn fingerprint_types(&mut self, roots: &[Ty], symbol_name: impl FnMut(SymbolId) -> Result<String, String>) -> Result<[u8; 32], String> {
        self.fingerprints(symbol_name).types(roots)
    }
}
impl<F: FnMut(SymbolId) -> Result<String, String>> Fingerprints<'_, F> {
    pub(crate) fn scheme(&mut self, scheme: &Scheme) -> Result<[u8; 32], String> {
        if !self.engine.is_closed_scheme(scheme) { return Err("open type scheme fingerprint".into()); }
        self.types(&[scheme.root])
    }
    pub(crate) fn types(&mut self, roots: &[Ty]) -> Result<[u8; 32], String> {
        let mut hashes = HashMap::<Ty, [u8; 32]>::new();
        let mut active = BTreeSet::new();
        let mut variables = 0_u64;
        let mut pending: Vec<_> = roots.iter().rev().map(|ty| (*ty, false)).collect();
        while let Some((ty, finish)) = pending.pop() {
            let ty = self.engine.find(ty);
            if hashes.contains_key(&ty) {
                continue;
            }
            if let Some(hash) = self.concrete.get(&ty) {
                hashes.insert(ty, *hash);
                continue;
            }
            let mut descriptor = self.engine.nodes[ty.0 as usize].descriptor.clone();
            if let Descriptor::Structure(term) = &descriptor
                && let Term::Record {
                    fields,
                    extension: Some(tail),
                } = &**term
            {
                let row = self.engine.gather_fields(fields, Some(*tail))?;
                descriptor = Descriptor::Structure(Rc::new(Term::Record {
                    fields: row.fields.into_owned(),
                    extension: row.extension,
                }));
            }
            if !finish {
                if !active.insert(ty) {
                    return Err("cyclic type fingerprint".into());
                }
                if let Descriptor::Structure(term) = &descriptor {
                    pending.push((ty, true));
                    pending.extend(Engine::children(term).rev().map(|ty| (ty, false)));
                    continue;
                }
            }
            let is_concrete = match &descriptor {
                Descriptor::Structure(term) => Engine::children(term)
                    .all(|child| self.concrete.contains_key(&self.engine.find(child))),
                _ => false,
            };
            let mut hash = Sha256::new();
            match descriptor {
                Descriptor::Variable { constraint, .. } => {
                    add(&mut hash, b"variable");
                    add(&mut hash, &variables.to_le_bytes());
                    variables += 1;
                    add(
                        &mut hash,
                        match constraint {
                            Constraint::Any => b"any",
                            Constraint::Number => b"number",
                            Constraint::Comparable => b"comparable",
                            Constraint::Appendable => b"appendable",
                            Constraint::CompAppend => b"compappend",
                        },
                    );
                }
                Descriptor::Rigid { .. } => return Err("rigid type fingerprint".into()),
                Descriptor::Structure(term) => {
                    match &*term {
                        Term::Alias(id, _, _) => {
                            add(&mut hash, b"alias");
                            add(&mut hash, (self.symbol_name)(*id)?.as_bytes());
                        }
                        Term::Named(id, _) => {
                            add(&mut hash, b"named");
                            add(&mut hash, (self.symbol_name)(*id)?.as_bytes());
                        }
                        Term::Function(_, _) => add(&mut hash, b"function"),
                        Term::Unit => add(&mut hash, b"unit"),
                        Term::Tuple(_) => add(&mut hash, b"tuple"),
                        Term::Record { fields, extension } => {
                            add(&mut hash, b"record");
                            add(&mut hash, &[u8::from(extension.is_some())]);
                            add(&mut hash, &(fields.len() as u64).to_le_bytes());
                            for name in fields.keys() {
                                add(&mut hash, name.as_bytes());
                            }
                        }
                    }
                    let children = Engine::children(&term);
                    add(&mut hash, &(children.clone().count() as u64).to_le_bytes());
                    for child in children {
                        add(&mut hash, &hashes[&self.engine.find(child)]);
                    }
                }
            }
            active.remove(&ty);
            let digest = hash.finalize().into();
            if is_concrete { self.concrete.insert(ty, digest); }
            hashes.insert(ty, digest);
        }
        let mut hash = Sha256::new();
        add(&mut hash, b"elm-type-fingerprint-v1");
        for root in roots {
            add(&mut hash, &hashes[&self.engine.find(*root)]);
        }
        Ok(hash.finalize().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_concrete_hashes_keep_each_polymorphic_root_scope_independent() {
        let mut engine = Engine::new(Builtins {
            int: SymbolId(0), float: SymbolId(1), string: SymbolId(2),
            char: SymbolId(3), list: SymbolId(4),
        });
        let int = engine.term(Term::Named(SymbolId(0), vec![]));
        let record = engine.term(Term::Record { fields: BTreeMap::from([("field".into(), int)]), extension: None });
        let x = engine.variable(1, Constraint::Any);
        let y = engine.variable(1, Constraint::Any);
        let polymorphic = engine.term(Term::Function(x, record));
        let scopes = [vec![polymorphic], vec![y, polymorphic], vec![x, polymorphic], vec![record, polymorphic]];
        let identity = |id: SymbolId| Ok(format!("type-{}", id.0));
        let expected: Vec<_> = scopes.iter().map(|roots| engine.fingerprint_types(roots, identity).unwrap()).collect();
        let names = std::cell::Cell::new(0);
        {
            let mut shared = engine.fingerprints(|id| { names.set(names.get() + 1); identity(id) });
            for (roots, expected) in scopes.iter().zip(&expected) {
                assert_eq!(&shared.types(roots).unwrap(), expected);
            }
            assert!(shared.concrete.contains_key(&record));
            assert!(!shared.concrete.contains_key(&polymorphic));
            assert!(!shared.concrete.contains_key(&x));
        }
        assert_eq!(names.get(), 1, "closed named subgraph is hashed once across four scopes");
        engine.unify(x, int).unwrap();
        assert_ne!(engine.fingerprint_types(&[polymorphic], identity).unwrap(), expected[0]);
    }
}
