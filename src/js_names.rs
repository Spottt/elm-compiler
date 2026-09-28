//! Shared JavaScript identities and constructor layouts for expressions/kernel.
use crate::{
    ast::{Declaration, ExprId, PatternId, Syntax},
    codegen::Reference,
    kernel::Mode,
    names::{Binding, LocalId, Resolved, Space, SymbolId, SymbolKind, Symbols},
    pattern_codegen::Names,
};
use std::collections::BTreeMap;
pub fn local(id: LocalId) -> String {
    format!("$l{}", id.0)
}
pub fn global(module: &str, name: &str) -> Result<String, String> {
    let (package, module) = module
        .split_once(':')
        .ok_or("missing canonical module owner")?;
    let (author, project) = if package == "application" {
        ("author", "project")
    } else {
        package.split_once('/').ok_or("invalid package identity")?
    };
    Ok(format!(
        "${}${}${}${name}",
        author.replace('-', "_"),
        project.replace('-', "_"),
        module.replace('.', "$")
    ))
}
/// The same escaping must be used for Elm records and kernel ElmField tags.
macro_rules! reserved_words {
    ($($word:literal),+ $(,)?) => {
        const JS_RESERVED: &str = concat!($( $word, " ", )+);
        fn is_reserved(name: &str) -> bool {
            matches!(name, $( $word )|+)
        }
    };
}
reserved_words!(
    "do", "if", "in", "NaN", "int", "for", "new", "try", "var", "let",
    "null", "true", "eval", "byte", "char", "goto", "long", "case", "else", "this",
    "void", "with", "enum", "false", "final", "float", "short", "break", "catch", "throw",
    "while", "class", "const", "super", "yield", "double", "native", "throws", "delete", "return",
    "switch", "typeof", "export", "import", "public", "static", "boolean", "default", "finally", "extends",
    "package", "private", "Infinity", "abstract", "volatile", "function", "continue", "debugger", "undefined", "arguments",
    "transient", "interface", "protected", "instanceof", "implements", "synchronized",
);
pub fn field(name: &str) -> String {
    // Match the static vocabulary directly instead of scanning it for every
    // record access, update, port conversion and kernel field reference.
    let helper = matches!(name.as_bytes(), [b'F' | b'A', b'2'..=b'9']);
    if helper || is_reserved(name) {
        format!("_{name}")
    } else {
        name.into()
    }
}

/// Elm's stable kernel field ABI (Generate.JavaScript.Name.fromInt).
/// HtmlAsJson and other kernel consumers refer to these names literally.
pub fn kernel_field(index: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$0123456789";
    fn encode(mut index: u128, width: usize) -> String {
        let mut result = vec![0; width];
        for byte in result.iter_mut().rev() {
            *byte = ALPHABET[(index % 64) as usize];
            index /= 64;
        }
        String::from_utf8(result).unwrap()
    }
    if index < 53 {
        return (ALPHABET[index] as char).to_string();
    }
    let mut index = (index - 53) as u128;
    let mut width = 2;
    let mut block = 54_u128 * 64;
    loop {
        let mut reserved = JS_RESERVED
            .split_whitespace()
            .filter(|s| s.len() == width)
            .collect::<Vec<_>>();
        reserved.sort_unstable_by(|a, b| b.cmp(a));
        let available = block - reserved.len() as u128;
        if index < available {
            let name = encode(index, width);
            return match reserved.iter().position(|&s| s == name) {
                Some(position) => encode(block - 1 - position as u128, width),
                None => name,
            };
        }
        index -= available;
        width += 1;
        block *= 64;
    }
}
#[derive(Default, Clone)]
pub struct Layouts {
    pub fields: crate::fields::Fields,
    indices: BTreeMap<SymbolId, usize>,
    enums: std::collections::BTreeSet<SymbolId>,
    unboxed: std::collections::BTreeSet<SymbolId>,
}
impl Layouts {
    pub fn inline_constant(&self, id: SymbolId, mode: Mode) -> bool {
        mode == Mode::Production && self.enums.contains(&id)
    }

    pub fn register(
        &mut self,
        ast: &Syntax<'_>,
        module: &str,
        symbols: &Symbols,
    ) -> Result<(), String> {
        for declaration in &ast.declarations {
            if let Declaration::Union { variants, .. } = declaration {
                for (index, (name, _)) in variants.iter().enumerate() {
                    let id = symbols
                        .lookup(module, name, Space::Constructor)
                        .ok_or("missing constructor identity")?;
                    self.indices.insert(id, index);
                    if variants.len() == 1 && variants[0].1.len() == 1 {
                        self.unboxed.insert(id);
                    }
                    if variants.iter().all(|(_, args)| args.is_empty()) {
                        self.enums.insert(id);
                    }
                }
            }
        }
        Ok(())
    }
    pub fn constructor_expression(
        &self,
        id: SymbolId,
        symbols: &Symbols,
        mode: Mode,
    ) -> Result<String, String> {
        let symbol = symbols.get(id);
        let SymbolKind::Constructor {
            arity,
            record: false,
            ..
        } = symbol.kind
        else {
            return Err("expected a union constructor".into());
        };
        if symbol.module.as_ref() == "elm/core:Basics"
            && matches!(symbol.name.as_ref(), "True" | "False")
        {
            return Ok(symbol.name.to_lowercase());
        }
        if mode == Mode::Production && self.enums.contains(&id) {
            return self.tag(id, symbols, mode);
        }
        if mode == Mode::Production && self.unboxed.contains(&id) {
            return Ok("function(value){return value;}".into());
        }
        if arity == 0 {
            return Ok(format!("({{$:{}}})", self.tag(id, symbols, mode)?));
        }
        // Match Elm's field ABI and the function/object shape inspected by
        // elm-test. Large arities use shallow currying, as record aliases do.
        let arguments = (0..arity).map(|i| format!("c{i}")).collect::<Vec<_>>();
        let fields = arguments
            .iter()
            .enumerate()
            .map(|(i, arg)| format!(",{}:{arg}", kernel_field(i)))
            .collect::<String>();
        let function = format!(
            "function({}){{return {{$:{}{fields}}};}}",
            arguments.join(","),
            self.tag(id, symbols, mode)?
        );
        Ok(match arity {
            1 => function,
            2..=9 => format!("F{arity}({function})"),
            _ => format!("_Rust_curry({arity},{function})"),
        })
    }
    pub fn tag(&self, id: SymbolId, symbols: &Symbols, mode: Mode) -> Result<String, String> {
        let symbol = symbols.get(id);
        if !matches!(mode, Mode::Production) {
            return Ok(serde_json::to_string(symbol.name.as_ref()).unwrap());
        }
        let index = *self.indices.get(&id).ok_or("missing constructor layout")? as i64;
        let tag = if (symbol.module.as_ref() == "elm/core:Dict"
            && symbol.name.as_ref() == "RBNode_elm_builtin")
            || symbol.name.as_ref() == "RBEmpty_elm_builtin"
        {
            -(index + 1)
        } else {
            index
        };
        Ok(tag.to_string())
    }
}
pub struct Naming<'a> {
    pub symbols: &'a Symbols,
    pub resolved: &'a Resolved,
    pub layouts: &'a Layouts,
    pub mode: Mode,
}
impl Naming<'_> {
    pub fn reference(&self, id: ExprId) -> Result<Reference, String> {
        match self.resolved.expressions[id.0 as usize].ok_or("missing expression binding")? {
            Binding::Local(id) => Ok(Reference {
                name: local(id),
                kernel: false,
            }),
            Binding::Global(id) if self.layouts.inline_constant(id, self.mode) => {
                let symbol = self.symbols.get(id);
                let name = if symbol.module.as_ref() == "elm/core:Basics"
                    && matches!(symbol.name.as_ref(), "True" | "False")
                {
                    symbol.name.to_lowercase()
                } else {
                    self.layouts.tag(id, self.symbols, self.mode)?
                };
                Ok(Reference {
                    name,
                    kernel: false,
                })
            }
            Binding::Global(id) => self.symbol(id),
        }
    }
    pub fn symbol(&self, id: SymbolId) -> Result<Reference, String> {
        let symbol = self.symbols.get(id);
        let kernel = matches!(symbol.kind, SymbolKind::Kernel);
        let name = if kernel {
            let (_, home) = symbol
                .module
                .split_once(":Elm.Kernel.")
                .ok_or("invalid kernel identity")?;
            format!("_{home}_{}", symbol.name)
        } else if symbol.module.as_ref() == "elm/core:Basics"
            && matches!(symbol.name.as_ref(), "True" | "False")
        {
            symbol.name.to_lowercase()
        } else {
            global(&symbol.module, &symbol.name)?
        };
        Ok(Reference { name, kernel })
    }
}
impl Names for Naming<'_> {
    fn switch_case(
        &mut self,
        pattern: PatternId,
        value: &str,
    ) -> Result<Option<(String, String)>, String> {
        let Some(id) = self.resolved.constructors[pattern.0 as usize] else {
            return Ok(None);
        };
        if self.mode != Mode::Production || !self.layouts.enums.contains(&id) {
            return Ok(None);
        }
        let symbol = self.symbols.get(id);
        let label = if symbol.module.as_ref() == "elm/core:Basics"
            && matches!(symbol.name.as_ref(), "True" | "False")
        {
            symbol.name.to_lowercase()
        } else {
            self.layouts.tag(id, self.symbols, self.mode)?
        };
        Ok(Some((value.into(), label)))
    }

    fn local(&mut self, id: PatternId, name: &str) -> Result<String, String> {
        let binding = self
            .resolved
            .pattern_bindings
            .get(&id)
            .and_then(|items| items.iter().find(|(n, _)| n == name))
            .ok_or("missing pattern binding")?;
        Ok(local(binding.1))
    }
    fn field(&mut self, name: &str) -> Result<String, String> {
        self.layouts.fields.name(name)
    }
    fn constructor(&mut self, id: PatternId, value: &str) -> Result<(String, Vec<String>), String> {
        let id = self.resolved.constructors[id.0 as usize]
            .ok_or("missing constructor pattern identity")?;
        let symbol = self.symbols.get(id);
        let SymbolKind::Constructor { arity, .. } = symbol.kind else {
            return Err("not a constructor".into());
        };
        if symbol.module.as_ref() == "elm/core:Basics"
            && matches!(symbol.name.as_ref(), "True" | "False")
        {
            return Ok((format!("{value}==={}", symbol.name.to_lowercase()), vec![]));
        }
        if self.mode == Mode::Production && self.layouts.unboxed.contains(&id) {
            return Ok(("true".into(), vec![value.into()]));
        }
        let tag = self.layouts.tag(id, self.symbols, self.mode)?;
        Ok((
            if self.mode == Mode::Production && self.layouts.enums.contains(&id) {
                format!("{value}==={tag}")
            } else {
                format!("{value}.$==={tag}")
            },
            (0..arity)
                .map(|i| format!("{value}.{}", kernel_field(i)))
                .collect(),
        ))
    }
}
