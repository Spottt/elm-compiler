//! Typed port converters mirroring Optimize.Port's Json.Encode/Decode calls.
use crate::{
    entry, js_names,
    kernel::Mode,
    names::{Space, SymbolId, Symbols},
    unify::{Engine, Term, Ty},
};
use std::collections::BTreeSet;
pub struct Converter {
    pub javascript: String,
    pub dependencies: BTreeSet<SymbolId>,
}
pub fn converter(
    engine: &mut Engine,
    symbols: &Symbols,
    root: Ty,
    incoming: bool,
    mode: Mode,
) -> Result<Converter, String> {
    converter_with_fields(
        engine,
        symbols,
        root,
        incoming,
        mode,
        &crate::fields::Fields::default(),
    )
}
pub fn converter_with_fields(
    engine: &mut Engine,
    symbols: &Symbols,
    root: Ty,
    incoming: bool,
    mode: Mode,
    fields: &crate::fields::Fields,
) -> Result<Converter, String> {
    entry::payload(engine, symbols, root)?;
    let mut generator = Generator {
        engine,
        symbols,
        mode,
        fields,
        dependencies: BTreeSet::new(),
    };
    let javascript = generator.convert(root, incoming, 0)?;
    Ok(Converter {
        javascript,
        dependencies: generator.dependencies,
    })
}
struct Generator<'a> {
    engine: &'a mut Engine,
    symbols: &'a Symbols,
    mode: Mode,
    fields: &'a crate::fields::Fields,
    dependencies: BTreeSet<SymbolId>,
}
impl Generator<'_> {
    fn global(&mut self, module: &str, name: &str) -> Result<String, String> {
        let id = self
            .symbols
            .lookup(module, name, Space::Value)
            .or_else(|| self.symbols.lookup(module, name, Space::Constructor))
            .ok_or_else(|| format!("missing converter dependency {module}.{name}"))?;
        self.dependencies.insert(id);
        js_names::global(module, name)
    }
    fn json(&mut self, incoming: bool, name: &str) -> Result<String, String> {
        self.global(
            if incoming {
                "elm/json:Json.Decode"
            } else {
                "elm/json:Json.Encode"
            },
            name,
        )
    }
    fn convert(&mut self, root: Ty, incoming: bool, depth: usize) -> Result<String, String> {
        if depth > 512 {
            return Err("port type nesting exceeds 512".into());
        }
        let term = self
            .engine
            .structure(root)
            .ok_or("unresolved port payload")?;
        match &*term {
            Term::Unit => {
                let null = self.json(incoming, "null")?;
                Ok(if incoming {
                    format!(
                        "{null}({})",
                        if !matches!(self.mode, Mode::Production) {
                            "_Utils_Tuple0"
                        } else {
                            "0"
                        }
                    )
                } else {
                    format!("(function($value){{return {null};}})")
                })
            }
            Term::Named(id, args) => {
                let symbol = self.symbols.get(*id);
                let name = symbol.name.to_string();
                match (name.as_str(), args.as_slice()) {
                    ("Int" | "Float" | "Bool" | "String", []) => {
                        self.json(incoming, &name.to_lowercase())
                    }
                    ("Value", []) if incoming => self.json(true, "value"),
                    ("Value", []) => self.global("elm/core:Basics", "identity"),
                    ("List" | "Array", [arg]) => {
                        let function = self.json(incoming, &name.to_lowercase())?;
                        let child = self.convert(*arg, incoming, depth + 1)?;
                        Ok(format!("{function}({child})"))
                    }
                    ("Maybe", [arg]) => {
                        let child = self.convert(*arg, incoming, depth + 1)?;
                        let null = self.json(incoming, "null")?;
                        if incoming {
                            let nothing = self.global("elm/core:Maybe", "Nothing")?;
                            let just = self.global("elm/core:Maybe", "Just")?;
                            let one_of = self.json(true, "oneOf")?;
                            let map = self.json(true, "map")?;
                            Ok(format!(
                                "{one_of}(_List_fromArray([{null}({nothing}),A2({map},{just},{child})]))"
                            ))
                        } else {
                            let destruct = self.global("elm/core:Maybe", "destruct")?;
                            Ok(format!(
                                "(function($value){{return A3({destruct},{null},{child},$value);}})"
                            ))
                        }
                    }
                    _ => Err("unsupported named port payload".into()),
                }
            }
            Term::Tuple(items) => {
                if incoming {
                    let values = (0..items.len())
                        .map(|i| format!("$p{i}"))
                        .collect::<Vec<_>>()
                        .join(",");
                    let succeed = self.json(true, "succeed")?;
                    let index = self.json(true, "index")?;
                    let and_then = self.json(true, "andThen")?;
                    let mut result = format!("{succeed}(_Utils_Tuple{}({values}))", items.len());
                    for (i, item) in items.iter().enumerate().rev() {
                        let child = self.convert(*item, true, depth + 1)?;
                        result = format!(
                            "A2({and_then},function($p{i}){{return {result};}},A2({index},{i},{child}))"
                        );
                    }
                    Ok(result)
                } else {
                    let list = self.json(false, "list")?;
                    let identity = self.global("elm/core:Basics", "identity")?;
                    let mut values = Vec::new();
                    for (i, item) in items.iter().enumerate() {
                        let child = self.convert(*item, false, depth + 1)?;
                        values.push(format!("({child})($value.{})", (b'a' + i as u8) as char));
                    }
                    Ok(format!(
                        "(function($value){{return A2({list},{identity},_List_fromArray([{}]));}})",
                        values.join(",")
                    ))
                }
            }
            Term::Record { fields, extension } => {
                // Flags are checked after inference, so an extensible alias can
                // have a fully closed row here. Decode the complete record,
                // including fields contributed by each row of that alias.
                let mut fields = std::borrow::Cow::Borrowed(fields);
                let mut extension = *extension;
                let mut seen = BTreeSet::new();
                seen.insert(self.engine.find(root));
                while let Some(row) = extension {
                    if !seen.insert(self.engine.find(row)) {
                        return Err("cyclic payload record extension".into());
                    }
                    let row = self
                        .engine
                        .structure(row)
                        .ok_or("open payload record extension")?;
                    let Term::Record {
                        fields: extra,
                        extension: next,
                    } = &*row
                    else {
                        return Err("payload record extension is not a record".into());
                    };
                    for (name, ty) in extra {
                        if fields.to_mut().insert(name.clone(), *ty).is_some() {
                            return Err(format!("duplicate payload field {name}"));
                        }
                    }
                    extension = *next;
                }
                if incoming {
                    let succeed = self.json(true, "succeed")?;
                    let field = self.json(true, "field")?;
                    let and_then = self.json(true, "andThen")?;
                    let entries = fields
                        .keys()
                        .enumerate()
                        .map(|(i, name)| Ok(format!("{}:$p{i}", quote(&self.fields.name(name)?))))
                        .collect::<Result<Vec<_>, String>>()?
                        .join(",");
                    let mut result = format!("{succeed}({{{entries}}})");
                    // Upstream folds ascending fields around the decoder: the
                    // last field is checked first, affecting error reporting.
                    for (i, (name, item)) in fields.iter().enumerate() {
                        let child = self.convert(*item, true, depth + 1)?;
                        result = format!(
                            "A2({and_then},function($p{i}){{return {result};}},A2({field},{},{child}))",
                            quote(name)
                        );
                    }
                    Ok(result)
                } else {
                    let object = self.json(false, "object")?;
                    let mut entries = Vec::new();
                    for (name, item) in fields.iter() {
                        let child = self.convert(*item, false, depth + 1)?;
                        entries.push(format!(
                            "_Utils_Tuple2({},({child})($value[{}]))",
                            quote(name),
                            quote(&self.fields.name(name)?)
                        ));
                    }
                    Ok(format!(
                        "(function($value){{return {object}(_List_fromArray([{}]));}})",
                        entries.join(",")
                    ))
                }
            }
            _ => Err("unsupported port payload structure".into()),
        }
    }
}
fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}
