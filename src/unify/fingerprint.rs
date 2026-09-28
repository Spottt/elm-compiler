//! Structural hashes independent of allocation order, inference levels and DAG
//! sharing. Ordered roots share one alpha-renaming scope (e.g. alias parameters
//! followed by its body). Call separately for independently quantified schemes.
use super::*;
use sha2::{Digest, Sha256};

fn add(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

impl Engine {
    /// Exported values must be closed: free variables carry relationships to
    /// the surrounding inference state that a standalone hash cannot express.
    pub fn fingerprint_scheme(
        &mut self,
        scheme: &Scheme,
        symbol_name: impl FnMut(SymbolId) -> Result<String, String>,
    ) -> Result<[u8; 32], String> {
        if !self.is_closed_scheme(scheme) {
            return Err("open type scheme fingerprint".into());
        }
        self.fingerprint_types(&[scheme.root], symbol_name)
    }

    pub fn fingerprint_types(
        &mut self,
        roots: &[Ty],
        mut symbol_name: impl FnMut(SymbolId) -> Result<String, String>,
    ) -> Result<[u8; 32], String> {
        let mut hashes = HashMap::<Ty, [u8; 32]>::new();
        let mut active = BTreeSet::new();
        let mut variables = 0_u64;
        let mut pending: Vec<_> = roots.iter().rev().map(|ty| (*ty, false)).collect();
        while let Some((ty, finish)) = pending.pop() {
            let ty = self.find(ty);
            if hashes.contains_key(&ty) {
                continue;
            }
            let mut descriptor = self.nodes[ty.0 as usize].descriptor.clone();
            if let Descriptor::Structure(term) = &descriptor
                && let Term::Record {
                    fields,
                    extension: Some(tail),
                } = &**term
            {
                let row = self.gather_fields(fields, Some(*tail))?;
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
                    pending.extend(Self::children(term).into_iter().rev().map(|ty| (ty, false)));
                    continue;
                }
            }
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
                            add(&mut hash, symbol_name(*id)?.as_bytes());
                        }
                        Term::Named(id, _) => {
                            add(&mut hash, b"named");
                            add(&mut hash, symbol_name(*id)?.as_bytes());
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
                    let children = Self::children(&term);
                    add(&mut hash, &(children.len() as u64).to_le_bytes());
                    for child in children {
                        add(&mut hash, &hashes[&self.find(child)]);
                    }
                }
            }
            active.remove(&ty);
            hashes.insert(ty, hash.finalize().into());
        }
        let mut hash = Sha256::new();
        add(&mut hash, b"elm-type-fingerprint-v1");
        for root in roots {
            add(&mut hash, &hashes[&self.find(*root)]);
        }
        Ok(hash.finalize().into())
    }
}
