//! Declaration context is recovered only on the unbound-variable error path.
use crate::ast::{Declaration, Span, Syntax, Type};
use crate::docs_diagnostic::{reflow, text};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

fn offset(source: &str, part: &str) -> Option<u32> {
    let start = (part.as_ptr() as usize).checked_sub(source.as_ptr() as usize)?;
    (source.get(start..start + part.len()) == Some(part)).then_some(start as u32)
}
fn point(source: &str, offset: u32) -> Option<(usize, usize)> {
    let before = source.get(..offset as usize)?;
    Some((
        before.bytes().filter(|b| *b == b'\n').count() + 1,
        before.rsplit('\n').next()?.chars().count() + 1,
    ))
}
pub fn unbound(ast: &Syntax<'_>, missing: &str, reason: String) -> String {
    details(ast, missing).map_or_else(
        || reason.clone(),
        |(span, data)| {
            crate::source_error::locate(
                ast.source,
                span,
                crate::name_diagnostic::type_variables(reason.clone(), data),
            )
        },
    )
}
fn details(ast: &Syntax<'_>, missing: &str) -> Option<(Span, Value)> {
    let at = offset(ast.source, missing)?;
    let (name, parameters, alias, roots) = ast
        .declarations
        .iter()
        .filter_map(|d| match d {
            Declaration::Alias {
                name,
                parameters,
                ty,
            } => Some((*name, parameters, true, vec![*ty])),
            Declaration::Union {
                name,
                parameters,
                variants,
            } => Some((
                *name,
                parameters,
                false,
                variants
                    .iter()
                    .rev()
                    .flat_map(|(_, args)| args.iter().copied())
                    .collect(),
            )),
            _ => None,
        })
        .filter(|(name, _, _, _)| offset(ast.source, name).is_some_and(|start| start <= at))
        .max_by_key(|(name, _, _, _)| offset(ast.source, name))?;
    let mut free = BTreeMap::new();
    let mut pending: Vec<_> = roots.into_iter().rev().collect();
    while let Some(id) = pending.pop() {
        match &ast.types[id.0 as usize].kind {
            Type::Var(name) => {
                free.insert(*name, offset(ast.source, name)?);
            }
            Type::Constructor(_, args) | Type::Tuple(args) => {
                pending.extend(args.iter().rev().copied())
            }
            Type::Function(a, b) => {
                pending.push(*b);
                pending.push(*a);
            }
            Type::Record { extension, fields } => {
                if let Some(name) = extension {
                    free.insert(*name, offset(ast.source, name)?);
                }
                pending.extend(fields.iter().rev().map(|(_, ty)| *ty));
            }
            Type::Unit => {}
        }
    }
    let unused: BTreeMap<_, _> = parameters
        .iter()
        .filter(|name| alias && !free.contains_key(**name))
        .map(|name| Some((*name, offset(ast.source, name)?)))
        .collect::<Option<_>>()?;
    for parameter in parameters {
        free.remove(parameter);
    }
    if free.is_empty() && unused.is_empty() {
        return None;
    }
    let tokens = crate::lexer::lex(ast.source).ok()?;
    let name_at = offset(ast.source, name)?;
    let first = tokens
        .iter()
        .rposition(|t| t.start < name_at && t.column == 1 && t.text(ast.source) == "type")?;
    let last = tokens
        .iter()
        .enumerate()
        .skip(first + 1)
        .find(|(_, t)| t.start > name_at && t.column == 1)
        .map_or(tokens.len() - 1, |(i, _)| i - 1);
    let span = Span {
        start: tokens[first].start,
        end: tokens[last].end,
    };
    let variables: Vec<_> = free.iter().map(|(name, at)| {
        Some(json!({"name":name,"start":point(ast.source,*at)?,"end":point(ast.source,*at + name.len() as u32)?}))
    }).collect::<Option<_>>()?;
    let unused: Vec<_> = unused.iter().map(|(name, at)| {
        Some(json!({"name":name,"start":point(ast.source,*at)?,"end":point(ast.source,*at + name.len() as u32)?}))
    }).collect::<Option<_>>()?;
    Some((
        span,
        json!({"kind":"type_variables","name":name,"parameters":parameters,"alias":alias,"variables":variables,"unused":unused}),
    ))
}
fn colored(message: &mut Vec<Value>, value: &str, color: &str) {
    message.push(json!({"bold":false,"underline":false,"color":color,"string":value}));
}
fn snippet(
    source: &str,
    start: (usize, usize),
    end: (usize, usize),
    highlight: ((usize, usize), (usize, usize)),
    message: &mut Vec<Value>,
) {
    let width = end.0.to_string().len();
    for (i, line) in source
        .split('\n')
        .enumerate()
        .skip(start.0 - 1)
        .take(end.0 - start.0 + 1)
    {
        text(message, format!("{:>width$}|", i + 1));
        if start.0 != end.0 && (highlight.0.0..=highlight.1.0).contains(&(i + 1)) {
            colored(message, ">", "RED");
        } else {
            text(message, " ".into());
        }
        text(message, format!("{line}\n"));
    }
    if start.0 == end.0 {
        text(message, " ".repeat(highlight.0.1 + width + 1));
        colored(
            message,
            &"^".repeat(highlight.1.1.saturating_sub(highlight.0.1).max(1)),
            "RED",
        );
    }
}
pub fn report(
    source: &str,
    module: &str,
    path: &Path,
    start: (usize, usize),
    end: (usize, usize),
    data: &Value,
) -> Option<Value> {
    if !data["unused"].as_array()?.is_empty() {
        return alias_report(source, module, path, start, end, data);
    }
    let name = data["name"].as_str()?;
    let kind = if data["alias"].as_bool()? {
        "type alias"
    } else {
        "type"
    };
    let variables = data["variables"].as_array()?;
    let names: Vec<_> = variables
        .iter()
        .map(|v| v["name"].as_str())
        .collect::<Option<_>>()?;
    let first = *names.first()?;
    let single = names.len() == 1;
    let overview = if single {
        format!("The `{name}` {kind} uses an unbound type variable `{first}` in its definition:")
    } else {
        let mut list = names[..names.len() - 1].join(", ");
        if names.len() > 2 {
            list.push(',');
        }
        format!(
            "Type variables {list} and {} are unbound in the `{name}` {kind} definition:",
            names.last()?
        )
    };
    let formatted = reflow(&overview);
    let mut rest = formatted.as_str();
    let mut message = Vec::new();
    if !single {
        text(&mut message, "Type variables ".into());
        rest = rest.strip_prefix("Type variables ")?;
    }
    for (index, variable) in names.iter().enumerate() {
        if !single && index > 0 {
            let separator = if index == names.len() - 1 {
                if names.len() > 2 { ", and " } else { " and " }
            } else {
                ", "
            };
            // Reflow may replace any separator space with a newline.
            let count = separator.len();
            let actual = rest.get(..count)?;
            if actual.replace('\n', " ") != separator {
                return None;
            }
            text(&mut message, actual.into());
            rest = &rest[count..];
        }
        let label = if single {
            format!("`{variable}`")
        } else {
            variable.to_string()
        };
        let (before, after) = rest.split_once(&label)?;
        text(&mut message, before.into());
        colored(&mut message, &label, "yellow");
        rest = after;
    }
    text(&mut message, format!("{rest}\n\n"));
    let highlight = if single {
        let begin: (usize, usize) = serde_json::from_value(variables[0]["start"].clone()).ok()?;
        let finish: (usize, usize) = serde_json::from_value(variables[0]["end"].clone()).ok()?;
        (begin, finish)
    } else {
        (start, end)
    };
    snippet(source, start, end, highlight, &mut message);
    text(
        &mut message,
        format!(
            "\nYou probably need to change the declaration to something like this:\n\n    {kind} {name}"
        ),
    );
    for parameter in data["parameters"].as_array()? {
        text(&mut message, format!(" {}", parameter.as_str()?));
    }
    for variable in &names {
        text(&mut message, " ".into());
        colored(&mut message, variable, "GREEN");
    }
    text(
        &mut message,
        format!(
            " = ...\n\n{}",
            reflow(&format!(
                "Why? Well, imagine one `{name}` where `{first}` is an Int and another where it is a Bool. When we explicitly list the type variables, the type checker can see that they are actually different types."
            ))
        ),
    );
    let title = if single {
        "UNBOUND TYPE VARIABLE"
    } else {
        "UNBOUND TYPE VARIABLES"
    };
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":title,"region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    )
}

// Reflow replaces single separating spaces with newlines, retaining byte offsets.
// The supplied styled fragments contain identifiers only and cannot be split.
fn paragraph(message: &mut Vec<Value>, parts: Vec<(String, Option<&str>)>) -> Option<()> {
    let raw: String = parts.iter().map(|(s, _)| s.as_str()).collect();
    let wrapped = reflow(&raw);
    if raw.len() != wrapped.len() {
        return None;
    }
    let mut at = 0;
    for (s, color) in parts {
        let fragment = wrapped.get(at..at + s.len())?;
        if let Some(color) = color {
            colored(message, fragment, color);
        } else {
            text(message, fragment.into());
        }
        at += s.len();
    }
    Some(())
}
fn list_parts(names: &[&str], quoted_single: bool) -> Vec<(String, Option<&'static str>)> {
    let mut parts = Vec::new();
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            parts.push((
                if i == names.len() - 1 {
                    if names.len() > 2 { ", and " } else { " and " }
                } else {
                    ", "
                }
                .into(),
                None,
            ));
        }
        parts.push((
            if names.len() == 1 && quoted_single {
                format!("`{name}`")
            } else {
                name.to_string()
            },
            Some("yellow"),
        ));
    }
    parts
}
fn alias_report(
    source: &str,
    module: &str,
    path: &Path,
    start: (usize, usize),
    end: (usize, usize),
    data: &Value,
) -> Option<Value> {
    let name = data["name"].as_str()?;
    let unused = data["unused"].as_array()?;
    let names: Vec<_> = unused
        .iter()
        .map(|v| v["name"].as_str())
        .collect::<Option<_>>()?;
    let unbound: Vec<_> = data["variables"]
        .as_array()?
        .iter()
        .map(|v| v["name"].as_str())
        .collect::<Option<_>>()?;
    let mixed = !unbound.is_empty();
    let single = names.len() == 1 && !mixed;
    let (title, overview) = if mixed {
        (
            "TYPE VARIABLE PROBLEMS",
            format!("Type alias `{name}` has some type variable problems."),
        )
    } else if single {
        (
            "UNUSED TYPE VARIABLE",
            format!(
                "Type alias `{name}` does not use the `{}` type variable.",
                names[0]
            ),
        )
    } else {
        let list: String = list_parts(&names, false)
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        (
            "UNUSED TYPE VARIABLES",
            format!("Type variables {list} are unused in the `{name}` definition."),
        )
    };
    let mut message = Vec::new();
    text(&mut message, format!("{}\n\n", reflow(&overview)));
    let highlight = if single {
        (
            serde_json::from_value(unused[0]["start"].clone()).ok()?,
            serde_json::from_value(unused[0]["end"].clone()).ok()?,
        )
    } else {
        (start, end)
    };
    snippet(source, start, end, highlight, &mut message);
    text(&mut message, "\n".into());
    if mixed {
        let mut parts = vec![(
            if unbound.len() == 1 {
                "Type variable "
            } else {
                "Type variables "
            }
            .into(),
            None,
        )];
        parts.extend(list_parts(&unbound, true));
        parts.push((
            if unbound.len() == 1 {
                " appears in the definition, but I do not see it declared. "
            } else {
                " are used in the definition, but I do not see them declared. "
            }
            .into(),
            None,
        ));
        parts.push((
            if names.len() == 1 {
                "Likewise, type variable "
            } else {
                "Likewise, type variables "
            }
            .into(),
            None,
        ));
        parts.extend(list_parts(&names, true));
        // Preserve the reference compiler's spelling for protocol fidelity.
        parts.push((
            if names.len() == 1 {
                " is delared, but not used."
            } else {
                " are delared, but not used."
            }
            .into(),
            None,
        ));
        paragraph(&mut message, parts)?;
        text(
            &mut message,
            "\n\nMy guess is that a definition like this will work better:\n\n    type alias "
                .into(),
        );
        text(&mut message, name.into());
    } else {
        let mut parts = vec![("I recommend removing ".into(), None)];
        parts.extend(list_parts(&names, true));
        parts.push((" from the declaration, like this:".into(), None));
        paragraph(&mut message, parts)?;
        text(&mut message, "\n\n    type alias ".into());
        colored(&mut message, name, "GREEN");
    }
    for p in data["parameters"].as_array()? {
        let p = p.as_str()?;
        if !names.contains(&p) {
            text(&mut message, format!(" {p}"));
        }
    }
    for p in &unbound {
        text(&mut message, " ".into());
        colored(&mut message, p, "GREEN");
    }
    text(&mut message, " = ...".into());
    if !mixed {
        text(
            &mut message,
            format!(
                "\n\n{}",
                reflow(
                    "Why? Well, if I allowed `type alias Height a = Float` I would need to answer some weird questions. Is `Height Bool` the same as `Float`? Is `Height Bool` the same as `Height Int`? My solution is to not need to ask them!"
                )
            ),
        );
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":title,"region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    )
}
