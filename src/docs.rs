//! Documentation overview parsing and export-list validation, following
//! Elm.Docs and Parse.Space in the Elm 0.19.1 compiler. JSON/type generation
//! and CLI integration are separate steps.
use crate::{
    ast::{Declaration, Span, Syntax, TypeId},
    lexer::DocComment,
    module::{Exposed, Exposing},
    parser::Documentation,
    unicode,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, PartialEq, Eq)]
pub struct DocumentedName<'s> {
    pub name: &'s str,
    pub span: Span,
    pub column: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum NameProblem {
    OnlyInDocs {
        name: String,
        span: Span,
        column: u32,
    },
    OnlyInExports {
        name: String,
    },
    Duplicate {
        name: String,
        spans: Vec<Span>,
        columns: Vec<u32>,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    ImplicitExposing,
    MissingOverview,
    Syntax {
        offset: u32,
        column: u32,
        message: &'static str,
    },
    Names(Vec<NameProblem>),
    Definitions(Vec<DefinitionProblem>),
    InvalidExport(String),
}

fn inner(c: char) -> bool {
    c == '_' || c.is_ascii_digit() || unicode::is_alpha(c)
}
fn operator(c: char) -> bool {
    "+-/*=.<>:&|^?%!".contains(c)
}

struct Cursor<'s> {
    source: &'s str,
    pos: usize,
    end: usize,
    column: u32,
}
impl<'s> Cursor<'s> {
    fn rest(&self) -> &'s str {
        &self.source[self.pos..self.end]
    }
    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }
    fn advance(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
            self.column = if c == '\n' { 1 } else { self.column + 1 };
        }
    }
    fn take(&mut self, text: &str) -> bool {
        if !self.rest().starts_with(text) {
            return false;
        }
        for _ in text.chars() {
            self.advance();
        }
        true
    }
    fn error(&self, message: &'static str) -> Error {
        Error::Syntax {
            offset: self.pos as u32,
            column: self.column,
            message,
        }
    }
    fn spaces(&mut self) -> Result<(), Error> {
        loop {
            match self.peek() {
                Some(' ' | '\n') => self.advance(),
                // Parse.Space ignores CR without advancing its column.
                Some('\r') => self.pos += 1,
                Some('\t') => return Err(self.error("tabs are not allowed in documentation lists")),
                _ if self.take("--") => {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.advance();
                    }
                }
                _ if self.rest().starts_with("{-") && !self.rest().starts_with("{-|") => {
                    self.take("{-");
                    let mut depth = 1usize;
                    while depth > 0 {
                        if self.take("{-") {
                            depth += 1;
                        } else if self.take("-}") {
                            depth -= 1;
                        } else if self.peek() == Some('\t') {
                            return Err(self.error("tabs are not allowed in comments"));
                        } else if self.peek().is_none() {
                            return Err(self.error("unterminated comment"));
                        } else {
                            self.advance();
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }
    fn name(&mut self) -> Result<DocumentedName<'s>, Error> {
        let start = self.pos;
        let column = self.column;
        let name;
        if self
            .peek()
            .is_some_and(|c| unicode::is_lower(c) || unicode::is_upper(c))
        {
            self.advance();
            while self.peek().is_some_and(inner) {
                self.advance();
            }
            name = &self.source[start..self.pos];
            if crate::parser::reserved(name) {
                return Err(Error::Syntax {
                    offset: start as u32,
                    column,
                    message: "expected a documented name",
                });
            }
        } else if self.take("(") {
            let op_start = self.pos;
            while self.peek().is_some_and(operator) {
                self.advance();
            }
            name = &self.source[op_start..self.pos];
            // Unlike source expression parsing, Elm.Docs permits (..).
            if matches!(name, "." | "|" | "->" | "=" | ":") {
                return Err(Error::Syntax {
                    offset: op_start as u32,
                    column: column + 1,
                    message: "reserved documentation operator",
                });
            }
            if name.is_empty() || !self.take(")") {
                return Err(self.error("expected a parenthesized operator"));
            }
        } else {
            return Err(self.error("expected a documented name"));
        }
        Ok(DocumentedName {
            name,
            column,
            span: Span {
                start: start as u32,
                end: self.pos as u32,
            },
        })
    }
}

/// The marker can occur anywhere in prose (including inline code), but must
/// end at an identifier boundary. Keep names in source order and original spans.
pub fn parse_overview(source: &str, comment: DocComment) -> Result<Vec<DocumentedName<'_>>, Error> {
    let mut cursor = Cursor {
        source,
        pos: comment.start as usize + 3,
        end: comment.end as usize - 2,
        column: comment.column + 3,
    };
    let mut names = Vec::new();
    while cursor.peek().is_some() {
        if cursor.rest().starts_with("@docs")
            && cursor.rest()[5..].chars().next().is_none_or(|c| !inner(c))
        {
            cursor.take("@docs");
            cursor.spaces()?;
            loop {
                names.push(cursor.name()?);
                cursor.spaces()?;
                // A comma in column one is not consumed as a continuation.
                if cursor.column <= 1 || !cursor.take(",") {
                    break;
                }
                cursor.spaces()?;
            }
        } else {
            cursor.advance();
        }
    }
    Ok(names)
}

/// Call after normal module/type validation. This checks documentation names,
/// not whether the declarations have annotations or documentation comments.
pub fn validate_names<'s>(
    ast: &Syntax<'s>,
    docs: &Documentation<'s>,
) -> Result<Vec<DocumentedName<'s>>, Error> {
    let Exposing::Explicit(exports) = &ast.header.exposing else {
        return Err(Error::ImplicitExposing);
    };
    let overview = docs.overview.ok_or(Error::MissingOverview)?;
    let names = parse_overview(ast.source, overview)?;
    let exports = exports
        .iter()
        .map(|export| match export {
            Exposed::Value(name) | Exposed::Operator(name) | Exposed::Type { name, .. } => {
                name.as_str()
            }
        })
        .collect::<BTreeSet<_>>();
    let mut documented: BTreeMap<&str, Vec<&DocumentedName<'_>>> = BTreeMap::new();
    for name in &names {
        documented.entry(name.name).or_default().push(name);
    }
    let keys = exports
        .iter()
        .copied()
        .chain(documented.keys().copied())
        .collect::<BTreeSet<_>>();
    let mut problems = Vec::new();
    for name in keys {
        match documented.get(name) {
            Some(spans) if spans.len() > 1 => problems.push(NameProblem::Duplicate {
                name: name.into(),
                spans: spans.iter().map(|n| n.span).collect(),
                columns: spans.iter().map(|n| n.column).collect(),
            }),
            Some(spans) if !exports.contains(name) => problems.push(NameProblem::OnlyInDocs {
                name: name.into(),
                span: spans[0].span,
                column: spans[0].column,
            }),
            None => problems.push(NameProblem::OnlyInExports { name: name.into() }),
            _ => {}
        }
    }
    if problems.is_empty() {
        Ok(names)
    } else {
        Err(Error::Names(problems))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DefinitionProblem {
    NoAnnotation { name: String },
    NoComment { name: String },
}

/// Validated source information used by the subsequent JSON/type renderer.
/// `declaration` points to the alias, union, value, port, or infix declaration.
/// An operator's annotation and comment come from its implementation function.
#[derive(Debug, PartialEq, Eq)]
pub struct DocumentedExport {
    pub name: String,
    pub declaration: usize,
    pub annotation: Option<TypeId>,
    pub comment: DocComment,
    pub constructors: bool,
}

/// Run after normal semantic checks. Documentation checks are ordered like
/// Elm.Docs.fromModule: explicit exports, overview, names, then definitions.
/// For each value/operator, a missing annotation takes priority over its
/// missing comment. Independent exported declarations accumulate errors.
pub fn validate(
    ast: &Syntax<'_>,
    docs: &Documentation<'_>,
) -> Result<Vec<DocumentedExport>, Error> {
    validate_names(ast, docs)?;
    let Exposing::Explicit(exports) = &ast.header.exposing else {
        unreachable!()
    };
    let mut annotations = BTreeMap::new();
    let mut declarations = BTreeMap::new();
    for (index, declaration) in ast.declarations.iter().enumerate() {
        match declaration {
            Declaration::Annotation { name, ty } => {
                annotations.insert(*name, *ty);
            }
            Declaration::Value { name, .. }
            | Declaration::Alias { name, .. }
            | Declaration::Union { name, .. } => {
                declarations.insert(*name, index);
            }
            Declaration::Port { name, ty } => {
                declarations.insert(*name, index);
                annotations.insert(*name, *ty);
            }
            Declaration::Infix { operator, .. } => {
                declarations.insert(*operator, index);
            }
            Declaration::Destruct { .. } => {}
        }
    }
    let exports = exports
        .iter()
        .map(|export| match export {
            Exposed::Value(name) | Exposed::Operator(name) | Exposed::Type { name, .. } => {
                (name.as_str(), export)
            }
        })
        .collect::<BTreeMap<_, _>>();
    let mut documented = Vec::new();
    let mut problems = Vec::new();
    for (name, export) in exports {
        let invalid = || Error::InvalidExport(name.into());
        let index = *declarations.get(name).ok_or_else(invalid)?;
        let declaration = &ast.declarations[index];
        let (real_name, needs_annotation, constructors) = match (export, declaration) {
            (
                Exposed::Value(_),
                Declaration::Value { name, .. } | Declaration::Port { name, .. },
            ) => (*name, true, false),
            (Exposed::Operator(_), Declaration::Infix { function, .. }) => {
                let implementation = declarations.get(function).ok_or_else(invalid)?;
                if !matches!(ast.declarations[*implementation], Declaration::Value { .. }) {
                    return Err(invalid());
                }
                (*function, true, false)
            }
            (Exposed::Type { constructors, .. }, Declaration::Union { name, .. }) => {
                (*name, false, *constructors)
            }
            (Exposed::Type { .. }, Declaration::Alias { name, .. }) => (*name, false, false),
            _ => return Err(invalid()),
        };
        let annotation = if needs_annotation {
            match annotations.get(real_name) {
                Some(ty) => Some(*ty),
                None => {
                    problems.push(DefinitionProblem::NoAnnotation {
                        name: real_name.into(),
                    });
                    continue;
                }
            }
        } else {
            None
        };
        match docs.declarations.get(real_name) {
            Some(comment) => documented.push(DocumentedExport {
                name: name.into(),
                declaration: index,
                annotation,
                comment: *comment,
                constructors,
            }),
            None => problems.push(DefinitionProblem::NoComment {
                name: real_name.into(),
            }),
        }
    }
    if problems.is_empty() {
        Ok(documented)
    } else {
        Err(Error::Definitions(problems))
    }
}

/// Render a canonical public type using resolved identities rather than source
/// import aliases. A work stack keeps long arrow/application chains off the
/// Rust call stack. Record fields and constructor arguments retain source order.
pub fn render_type(
    ast: &Syntax<'_>,
    resolved: &crate::names::Resolved,
    symbols: &crate::names::Symbols,
    ty: TypeId,
) -> Result<String, String> {
    use crate::ast::Type;
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Context {
        None,
        Function,
        Application,
    }
    enum Work {
        Type(TypeId, Context),
        Text(String),
    }
    let mut work = vec![Work::Type(ty, Context::None)];
    let mut output = String::new();
    while let Some(item) = work.pop() {
        let (id, context) = match item {
            Work::Text(text) => {
                output.push_str(&text);
                continue;
            }
            Work::Type(id, context) => (id, context),
        };
        let node = ast
            .types
            .get(id.0 as usize)
            .ok_or("invalid documentation type ID")?;
        let mut parts = Vec::new();
        match &node.kind {
            Type::Var(name) => output.push_str(name),
            Type::Unit => output.push_str("()"),
            Type::Function(_, _) => {
                let wrap = context != Context::None;
                if wrap {
                    output.push('(');
                }
                let mut current = id;
                while let Type::Function(arg, result) = &ast.types[current.0 as usize].kind {
                    parts.push(Work::Type(*arg, Context::Function));
                    parts.push(Work::Text(" -> ".into()));
                    current = *result;
                }
                parts.push(Work::Type(current, Context::Function));
                if wrap {
                    parts.push(Work::Text(")".into()));
                }
            }
            Type::Constructor(_, args) => {
                let symbol = resolved
                    .types
                    .get(id.0 as usize)
                    .and_then(|symbol| *symbol)
                    .ok_or("unresolved documentation type")?;
                let symbol = symbols.get(symbol);
                let home = symbol
                    .module
                    .rsplit_once(':')
                    .map_or(symbol.module.as_ref(), |(_, home)| home);
                let wrap = context == Context::Application && !args.is_empty();
                if wrap {
                    output.push('(');
                }
                output.push_str(home);
                output.push('.');
                output.push_str(&symbol.name);
                for arg in args {
                    parts.push(Work::Text(" ".into()));
                    parts.push(Work::Type(*arg, Context::Application));
                }
                if wrap {
                    parts.push(Work::Text(")".into()));
                }
            }
            Type::Tuple(items) => {
                output.push_str("( ");
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        parts.push(Work::Text(", ".into()));
                    }
                    parts.push(Work::Type(*item, Context::None));
                }
                parts.push(Work::Text(" )".into()));
            }
            Type::Record { extension, fields } => {
                if fields.is_empty() && extension.is_none() {
                    output.push_str("{}");
                } else {
                    output.push_str("{ ");
                    if let Some(extension) = extension {
                        output.push_str(extension);
                        output.push_str(" | ");
                    }
                    for (index, (name, ty)) in fields.iter().enumerate() {
                        if index > 0 {
                            parts.push(Work::Text(", ".into()));
                        }
                        parts.push(Work::Text(format!("{name} : ")));
                        parts.push(Work::Type(*ty, Context::None));
                    }
                    parts.push(Work::Text(" }".into()));
                }
            }
        }
        work.extend(parts.into_iter().rev());
    }
    Ok(output)
}

/// Produce one module in the official docs.json schema after normal type checks.
pub fn to_json(
    ast: &Syntax<'_>,
    docs: &Documentation<'_>,
    resolved: &crate::names::Resolved,
    symbols: &crate::names::Symbols,
) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let exports = validate(ast, docs).map_err(|error| format!("{error:?}"))?;
    let mut values = Vec::new();
    let mut aliases = Vec::new();
    let mut unions = Vec::new();
    let mut binops = Vec::new();
    let render = |ty| render_type(ast, resolved, symbols, ty);
    for export in exports {
        let comment = export.comment.normalized_text(ast.source);
        let name = export.name;
        match &ast.declarations[export.declaration] {
            Declaration::Value { .. } | Declaration::Port { .. } => {
                let ty = render(export.annotation.ok_or("missing documented annotation")?)?;
                values.push(json!({"name":name,"comment":comment,"type":ty}));
            }
            Declaration::Alias { parameters, ty, .. } => {
                aliases.push(
                    json!({"name":name,"comment":comment,"args":parameters,"type":render(*ty)?}),
                );
            }
            Declaration::Union {
                parameters,
                variants,
                ..
            } => {
                let mut cases = Vec::new();
                if export.constructors {
                    for (name, args) in variants {
                        let types = args
                            .iter()
                            .map(|ty| render(*ty))
                            .collect::<Result<Vec<_>, _>>()?;
                        cases.push(json!([name, types]));
                    }
                }
                unions.push(json!({"name":name,"comment":comment,"args":parameters,"cases":cases}));
            }
            Declaration::Infix {
                associativity,
                precedence,
                ..
            } => {
                let ty = render(
                    export
                        .annotation
                        .ok_or("missing documented operator annotation")?,
                )?;
                binops.push(json!({"name":name,"comment":comment,"type":ty,"associativity":associativity,"precedence":precedence}));
            }
            _ => return Err("invalid documented declaration".into()),
        }
    }
    Ok(
        json!({"name":ast.header.name,"comment":docs.overview.ok_or("missing module overview")?.normalized_text(ast.source),"unions":unions,"aliases":aliases,"values":values,"binops":binops}),
    )
}
