//! Validated, immutable relative nodes. Neither Ty offsets nor runtime SymbolIds
//! are retained across imports into independently changing inference engines.
use super::*;
use serde_json::Value;

pub(crate) struct PreparedTypes {
    descriptors: Vec<PreparedDescriptor>,
    symbols: Vec<String>,
    roots: Vec<Ty>,
    display_names: BTreeMap<Ty, String>,
    field_orders: BTreeMap<Ty, Vec<String>>,
}
// Records dominate retained type graphs. Their fields are immutable and sorted:
// a flat array avoids retaining a tree allocation for every prepared record.
enum PreparedDescriptor {
    Other(Descriptor),
    Record { fields: Vec<(Rc<str>, Ty)>, extension: Option<Ty> },
}
impl PreparedDescriptor {
    fn new(mut descriptor: Descriptor) -> Self {
        if let Descriptor::Structure(term) = &mut descriptor
            && let Term::Record { fields, extension } = Rc::make_mut(term)
        {
            return Self::Record {
                fields: std::mem::take(fields).into_iter().collect(),
                extension: *extension,
            };
        }
        Self::Other(descriptor)
    }
}
impl PreparedTypes {
    pub(crate) fn new(artifact: &Value) -> Result<Self, String> {
        // Reuse the complete existing validator, including unreachable cycles
        // and optional metadata. This staging engine has zero nodes/offset.
        let zero = SymbolId(0);
        let mut staging = Engine::new(Builtins { int: zero, float: zero, string: zero, char: zero, list: zero });
        staging.track_debug_types();
        let mut symbols = Vec::new();
        let mut indices = HashMap::new();
        let roots = staging.import_types(artifact, |key| {
            if let Some(id) = indices.get(key) { return Ok(*id); }
            let id = SymbolId(u32::try_from(symbols.len()).map_err(|_| "too many portable type names")?);
            symbols.push(key.to_owned()); indices.insert(key.to_owned(), id); Ok(id)
        })?;
        Ok(Self {
            descriptors: staging.nodes.into_iter().map(|node| PreparedDescriptor::new(node.descriptor)).collect(),
            symbols, roots,
            display_names: staging.display_names.unwrap_or_default(),
            field_orders: staging.debug_field_order.unwrap_or_default(),
        })
    }
    pub(crate) fn root_count(&self) -> usize { self.roots.len() }
    pub(crate) fn import_into(&self, engine: &mut Engine, mut symbol: impl FnMut(&str) -> Result<SymbolId, String>) -> Result<Vec<Ty>, String> {
        let invalid = || "invalid type graph artifact".to_string();
        let offset = u32::try_from(engine.nodes.len()).map_err(|_| invalid())?;
        let count = u32::try_from(self.descriptors.len()).map_err(|_| invalid())?;
        offset.checked_add(count).ok_or_else(invalid)?;
        // Resolve every name afresh, including unreachable nodes, before any
        // mutation. The prepared graph stores only indices into this table.
        let symbols = self.symbols.iter().map(|name| symbol(name)).collect::<Result<Vec<_>, _>>()?;
        let remap = |ty: Ty| Ty(offset + ty.0);
        engine.nodes.reserve(self.descriptors.len());
        for descriptor in &self.descriptors {
            let descriptor = match descriptor {
                PreparedDescriptor::Record { fields, extension } => {
                    Descriptor::Structure(Rc::new(Term::Record {
                        fields: fields.iter().map(|(name, ty)| (name.clone(), remap(*ty))).collect(),
                        extension: extension.map(remap),
                    }))
                }
                PreparedDescriptor::Other(Descriptor::Structure(term)) => {
                    let mut term = (**term).clone();
                    match &mut term {
                        Term::Alias(id, args, real) => {
                            *id = symbols[id.0 as usize];
                            for ty in args { *ty = remap(*ty); }
                            *real = remap(*real);
                        }
                        Term::Named(id, args) => {
                            *id = symbols[id.0 as usize];
                            for ty in args { *ty = remap(*ty); }
                        }
                        Term::Tuple(items) => { for ty in items { *ty = remap(*ty); } }
                        Term::Function(a, b) => { *a = remap(*a); *b = remap(*b); }
                        Term::Unit => {}
                        Term::Record { fields, extension } => {
                            for ty in fields.values_mut() { *ty = remap(*ty); }
                            *extension = extension.map(remap);
                        }
                    }
                    Descriptor::Structure(Rc::new(term))
                }
                PreparedDescriptor::Other(other) => other.clone(),
            };
            engine.node(descriptor);
        }
        if let Some(names) = &mut engine.display_names {
            names.extend(self.display_names.iter().map(|(ty, name)| (remap(*ty), name.clone())));
        }
        if let Some(orders) = &mut engine.debug_field_order {
            orders.extend(self.field_orders.iter().map(|(ty, order)| (remap(*ty), order.clone())));
        }
        Ok(self.roots.iter().map(|ty| remap(*ty)).collect())
    }
    pub(crate) fn estimated_bytes(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>() + self.descriptors.capacity() * std::mem::size_of::<PreparedDescriptor>()
            + self.roots.capacity() * std::mem::size_of::<Ty>() + self.symbols.capacity() * std::mem::size_of::<String>();
        bytes += self.symbols.iter().map(|s| s.capacity() + 16).sum::<usize>();
        // A name can be shared by many records. Count each retained allocation
        // once, while still charging every field slot and Rc header allocation.
        let mut names = std::collections::HashSet::new();
        let mut name_bytes = |name: &Rc<str>| {
            if names.insert(name.as_ptr()) { name.len() + 32 } else { 0 }
        };
        for descriptor in &self.descriptors {
            if let PreparedDescriptor::Record { fields, .. } = descriptor {
                bytes += fields.capacity() * std::mem::size_of::<(Rc<str>, Ty)>();
                bytes += fields.iter().map(|(name, _)| name_bytes(name)).sum::<usize>();
            }
            if let PreparedDescriptor::Other(Descriptor::Structure(term)) = descriptor {
                bytes += std::mem::size_of::<Term>() + 32;
                match &**term {
                    Term::Alias(_, args, _) | Term::Named(_, args) | Term::Tuple(args) => bytes += args.capacity() * std::mem::size_of::<Ty>(),
                    Term::Record { fields, .. } => bytes += fields.keys().map(|name| name_bytes(name) + 112).sum::<usize>(),
                    _ => {}
                }
            }
        }
        bytes += self.display_names.values().map(|name| name.capacity() + 128).sum::<usize>();
        bytes += self.field_orders.values().map(|order| 128 + order.capacity() * std::mem::size_of::<String>() + order.iter().map(|name| name.capacity() + 16).sum::<usize>()).sum::<usize>();
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn engine() -> Engine {
        let zero = SymbolId(0);
        Engine::new(Builtins { int: zero, float: zero, string: zero, char: zero, list: zero })
    }
    #[test]
    fn prepared_imports_relocate_and_resolve_again_without_losing_metadata() {
        let graph = json!({"version":1,"nodes":[["variable",1,"any"],["named","List",[0]],["alias","Box",[0],1],["record",{"value":2},null],["function",0,3],["unit"],["rigid",2,"comparable"],["variable",2,"any"],["tuple",[5,6]],["record",{"other":8},7]],"roots":[4,0,3,8,9,6],"variable_names":{"0":"item","6":"rigid","7":"row"},"record_field_order":{"3":["value"],"9":["other"]}});
        let prepared = PreparedTypes::new(&graph).unwrap();
        for (offset, base, tracking) in [(0, 10, false), (5, 100, true), (2, 300, true)] {
            let mut direct = engine(); let mut cached = engine();
            for _ in 0..offset { direct.variable(0, Constraint::Any); cached.variable(0, Constraint::Any); }
            if tracking { direct.track_debug_types(); cached.track_debug_types(); }
            let resolve = |name: &str| Ok(SymbolId(base + u32::from(name == "Box")));
            let a = direct.import_types(&graph, resolve).unwrap();
            let b = prepared.import_into(&mut cached, resolve).unwrap();
            assert_eq!(a, b);
            let encode = |id: SymbolId| Ok(id.0.to_string());
            assert_eq!(direct.export_types(&a, encode).unwrap(), cached.export_types(&b, encode).unwrap());
            // Mutating one destination must not change subsequent imports.
            let unit = cached.term(Term::Unit);
            cached.unify(b[1], unit).unwrap();
        }
    }
    #[test]
    fn repeated_field_text_is_shared_across_prepared_records_and_imports() {
        let graph = json!({"version":1,"nodes":[["variable",1,"any"],["record",{"shared":0},null],["record",{"shared":0},null]],"roots":[1,2]});
        let prepared = PreparedTypes::new(&graph).unwrap();
        let name = |index| match &prepared.descriptors[index] {
            PreparedDescriptor::Record { fields, .. } => fields[0].0.clone(),
            _ => panic!("expected record"),
        };
        assert!(Rc::ptr_eq(&name(1), &name(2)));
        let mut destination = engine();
        for _ in 0..2 {
            let roots = prepared.import_into(&mut destination, |_| unreachable!()).unwrap();
            let view = destination.structure(roots[0]).unwrap();
            let Term::Record { fields, .. } = view.as_ref() else { panic!("expected record") };
            assert!(Rc::ptr_eq(&name(1), fields.keys().next().unwrap()));
        }
        let distinct = PreparedTypes::new(&json!({"version":1,"nodes":[["variable",1,"any"],["record",{"shared":0},null],["record",{"unique":0},null]],"roots":[1,2]})).unwrap();
        assert_eq!(distinct.estimated_bytes() - prepared.estimated_bytes(), "shared".len() + 32);
    }
    #[test]
    fn invalid_graphs_and_failed_resolution_cannot_mutate_the_destination() {
        for graph in [
            json!({"version":2,"nodes":[],"roots":[]}),
            json!({"version":1,"nodes":[["unit"]],"roots":[8]}),
            json!({"version":1,"nodes":[["unit"],["function",1,1]],"roots":[0]}),
            json!({"version":1,"nodes":[["variable",1,"unknown"]],"roots":[0]}),
            json!({"version":1,"nodes":[["unit"]],"roots":[0],"variable_names":{"0":"a"}}),
            json!({"version":1,"nodes":[["record",{},null]],"roots":[0],"record_field_order":{"0":["absent"]}}),
        ] { assert!(PreparedTypes::new(&graph).is_err()); }
        let prepared = PreparedTypes::new(&json!({"version":1,"nodes":[["unit"],["named","unknown",[]]],"roots":[0]})).unwrap();
        let mut destination = engine(); destination.variable(0, Constraint::Any);
        assert!(prepared.import_into(&mut destination, |_| Err("missing symbol".into())).is_err());
        assert_eq!(destination.node_count(), 1);
    }
}
