//! Kernel template reader, following Elm.Kernel's import table and tag scanner.
//! JavaScript fragments borrow the source; names remain unresolved until linking.
use crate::{
    lexer::lex,
    module::{Exposed, Exposing, Import, header},
};
use std::collections::BTreeMap;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chunk<'s> {
    JavaScript(&'s str),
    ElmVariable { module: String, name: String },
    KernelVariable { module: String, name: String },
    ElmField(&'s str),
    JsField(usize),
    JsEnum(usize),
    Debug,
    Prod,
}
pub struct Content<'s> {
    pub imports: Vec<Import>,
    pub chunks: Vec<Chunk<'s>>,
}
pub fn parse(source: &str) -> Result<Content<'_>, String> {
    let (block, body) = source
        .strip_prefix("/*")
        .and_then(|s| s.split_once("*/"))
        .ok_or("missing kernel import header")?;
    let text = format!("module Kernel exposing (..)\n{block}");
    let tokens = lex(&text)?;
    let header = header(&text, &tokens)?;
    if header.body_start < tokens.len() {
        return Err("unexpected content in kernel import header".into());
    }
    let mut variables = BTreeMap::new();
    for import in &header.imports {
        let kernel = import.name.strip_prefix("Elm.Kernel.");
        let prefix = match (kernel, &import.alias) {
            (Some(_), Some(_)) => return Err("kernel imports cannot have aliases".into()),
            (Some(name), None) => name,
            (None, Some(alias)) => alias,
            (None, None) if !import.name.contains('.') => &import.name,
            _ => return Err("dotted Elm imports need an alias in kernel code".into()),
        };
        let Exposing::Explicit(exposed) = &import.exposing else {
            return Err("kernel imports require explicit exposure".into());
        };
        for item in exposed {
            let name =
                match item {
                    Exposed::Value(name)
                    | Exposed::Type {
                        name,
                        constructors: false,
                    } => name,
                    _ => return Err(
                        "kernel imports cannot expose operators or union constructors with (..)"
                            .into(),
                    ),
                };
            let chunk = if let Some(module) = kernel {
                Chunk::KernelVariable {
                    module: module.into(),
                    name: name.clone(),
                }
            } else {
                Chunk::ElmVariable {
                    module: import.name.clone(),
                    name: name.clone(),
                }
            };
            variables.insert(format!("{prefix}_{name}"), chunk);
        }
    }
    let mut chunks = Vec::new();
    let mut fields = BTreeMap::new();
    let mut enums: BTreeMap<u8, BTreeMap<&str, usize>> = BTreeMap::new();
    let mut position = 0;
    while let Some(offset) = body[position..].find("__") {
        let start = position + offset;
        let tag_start = start + 2;
        let Some(first) = body[tag_start..].chars().next() else {
            break;
        };
        let end = tag_start
            + first.len_utf8()
            + body[tag_start + first.len_utf8()..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .map(char::len_utf8)
                .sum::<usize>();
        let tag = &body[tag_start..end];
        let chunk = match first {
            '$' => Chunk::ElmField(&tag[1..]),
            '0'..='9' => {
                let group = enums.entry(first as u8).or_default();
                let next = group.len();
                Chunk::JsEnum(*group.entry(tag).or_insert(next))
            }
            'a'..='z' => {
                let next = fields.len();
                Chunk::JsField(*fields.entry(tag).or_insert(next))
            }
            _ if tag == "DEBUG" => Chunk::Debug,
            _ if tag == "PROD" => Chunk::Prod,
            _ => variables
                .get(tag)
                .cloned()
                .ok_or_else(|| format!("unknown kernel tag __{tag}"))?,
        };
        chunks.push(Chunk::JavaScript(&body[position..start]));
        chunks.push(chunk);
        position = end;
    }
    chunks.push(Chunk::JavaScript(&body[position..]));
    Ok(Content {
        imports: header.imports,
        chunks,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Development,
    Production,
}
/// Render a kernel template through the linker's shared naming tables. Elm
/// globals must be resolved using this file's package/import context; record
/// fields must use the same mapping as generated Elm expressions.
pub fn render(
    content: &Content<'_>,
    mode: Mode,
    mut elm_global: impl FnMut(&str, &str) -> Result<String, String>,
    mut elm_field: impl FnMut(&str) -> Result<String, String>,
    mut js_field: impl FnMut(usize) -> String,
) -> Result<String, String> {
    let mut output = String::new();
    for chunk in &content.chunks {
        match chunk {
            Chunk::JavaScript(text) => output.push_str(text),
            Chunk::ElmVariable { module, name } => output.push_str(&elm_global(module, name)?),
            Chunk::KernelVariable { module, name } => {
                output.push('_');
                output.push_str(module);
                output.push('_');
                output.push_str(name);
            }
            Chunk::ElmField(name) => output.push_str(&elm_field(name)?),
            Chunk::JsField(index) => output.push_str(&js_field(*index)),
            Chunk::JsEnum(index) => output.push_str(&index.to_string()),
            Chunk::Debug if matches!(mode, Mode::Production) => output.push_str("_UNUSED"),
            Chunk::Prod if matches!(mode, Mode::Development) => output.push_str("_UNUSED"),
            Chunk::Debug | Chunk::Prod => {}
        }
    }
    Ok(output)
}
