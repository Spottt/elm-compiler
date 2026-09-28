//! Types consumed by elm/browser's debugger, following Elm.Compiler.Type.Extract.
//! Callers must retain alias wrappers and source variable names during inference.
use crate::{
    names::{Space, SymbolId, SymbolKind, Symbols},
    repl_type,
    type_localizer::Localizer,
    types::Catalog,
    unify::{Engine, Term, Ty},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn extract(
    engine: &mut Engine,
    catalog: &Catalog,
    symbols: &Symbols,
    message: Ty,
) -> Result<Value, String> {
    let mut extractor = Extractor {
        engine,
        symbols,
        pending: BTreeSet::new(),
    };
    let message = extractor.render(message)?;
    let mut seen = BTreeSet::new();
    let mut aliases = BTreeMap::new();
    let mut unions = BTreeMap::new();
    while let Some(id) = extractor.pending.pop_first() {
        if !seen.insert(id) {
            continue;
        }
        let symbol = symbols.get(id);
        let name = Localizer::default().name(&symbol.module, &symbol.name);
        if let Some(alias) = catalog.aliases.get(&id) {
            let args = alias
                .parameters
                .iter()
                .map(|ty| extractor.render(*ty))
                .collect::<Result<Vec<_>, _>>()?;
            aliases.insert(
                name,
                json!({"args":args,"type":extractor.render(alias.root)?}),
            );
            continue;
        }
        let SymbolKind::Type {
            arity,
            alias: false,
        } = symbol.kind
        else {
            return Err(format!("missing debugger alias definition {name}"));
        };
        if symbol.module.as_ref() == "elm/core:List" && symbol.name.as_ref() == "List" {
            unions.insert(name, json!({"args":["a"],"tags":{}}));
            continue;
        }
        let mut parameters = None;
        let mut tags = BTreeMap::new();
        for (constructor, scheme) in &catalog.constructors {
            let ctor = symbols.get(*constructor);
            if !matches!(ctor.kind, SymbolKind::Constructor { result, record: false, .. } if result == id)
            {
                continue;
            }
            let mut root = scheme.root;
            let mut args = Vec::new();
            while let Some(term) = extractor.engine.structure(root) {
                match &*term {
                    Term::Function(arg, result) => {
                        args.push(extractor.render(*arg)?);
                        root = *result;
                    }
                    Term::Named(result, vars) if *result == id => {
                        parameters = Some(
                            vars.iter()
                                .map(|ty| extractor.render(*ty))
                                .collect::<Result<Vec<_>, _>>()?,
                        );
                        break;
                    }
                    _ => return Err(format!("invalid debugger constructor {}", ctor.name)),
                }
            }
            tags.insert(ctor.name.to_string(), args);
        }
        let parameters = parameters.unwrap_or_default();
        if parameters.len() != arity {
            return Err(format!("missing debugger type parameters for {name}"));
        }
        unions.insert(name, json!({"args":parameters,"tags":tags}));
    }
    Ok(json!({"versions":{"elm":"0.19.1"},"types":{
        "message":message,"aliases":aliases,"unions":unions
    }}))
}

struct Extractor<'a> {
    engine: &'a mut Engine,
    symbols: &'a Symbols,
    pending: BTreeSet<SymbolId>,
}
impl Extractor<'_> {
    fn render(&mut self, ty: Ty) -> Result<String, String> {
        let graph = self.engine.export_types(&[ty], |id| {
            let symbol = self.symbols.get(id);
            Ok(json!([symbol.module.as_ref(), symbol.name.as_ref()]).to_string())
        })?;
        // export_types also walks the expanded body of aliases. These hidden
        // dependencies matter even though rendering keeps the alias's name.
        for node in graph["nodes"]
            .as_array()
            .ok_or("missing debugger type nodes")?
        {
            if matches!(node[0].as_str(), Some("named" | "alias")) {
                let (module, name): (String, String) =
                    serde_json::from_str(node[1].as_str().ok_or("missing debugger type name")?)
                        .map_err(|e| e.to_string())?;
                self.pending.insert(
                    self.symbols
                        .lookup(&module, &name, Space::Type)
                        .ok_or_else(|| format!("missing debugger symbol {module}.{name}"))?,
                );
            }
        }
        repl_type::render_width(&graph, 0, &Localizer::default(), isize::MAX as usize)
    }
}
