//! Structured documentation module, definition, name-list and syntax errors.
use crate::{
    ast::{Declaration, Span, Syntax},
    docs::{DefinitionProblem, Error, NameProblem},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io::IsTerminal};
const PREFIX: &str = "ELM_DOCS_JSON:";
pub fn encode(report: &Value) -> String {
    format!("{PREFIX}{report}")
}
pub fn report_encoded(message: &str) -> Option<Value> {
    serde_json::from_str(message.strip_prefix(PREFIX)?).ok()
}
pub(crate) fn position(source: &str, offset: u32) -> Option<(usize, usize)> {
    let prefix = source.get(..offset as usize)?;
    Some((
        prefix.bytes().filter(|b| *b == b'\n').count() + 1,
        prefix.rsplit('\n').next()?.chars().count() + 1,
    ))
}
fn source_span(source: &str, name: &str) -> Option<Span> {
    let start = (name.as_ptr() as usize).checked_sub(source.as_ptr() as usize)?;
    (source.get(start..start + name.len()) == Some(name)).then_some(Span {
        start: start as u32,
        end: (start + name.len()) as u32,
    })
}
fn exports(ast: &Syntax<'_>) -> Option<BTreeMap<String, Span>> {
    let tokens = crate::lexer::lex(ast.source).ok()?;
    let mut i = tokens
        .iter()
        .position(|t| t.text(ast.source) == "exposing")?
        + 2;
    let mut result = BTreeMap::new();
    while let Some(token) = tokens.get(i) {
        let text = token.text(ast.source);
        if text == ")" {
            break;
        }
        if text == "(" {
            let op = tokens.get(i + 1)?;
            let end = tokens.get(i + 2)?;
            if end.text(ast.source) != ")" {
                return None;
            }
            result.insert(
                op.text(ast.source).into(),
                Span {
                    start: token.start,
                    end: end.end,
                },
            );
            i += 3;
        } else if matches!(
            token.kind,
            crate::lexer::Kind::Upper | crate::lexer::Kind::Lower
        ) {
            result.insert(
                text.into(),
                Span {
                    start: token.start,
                    end: token.end,
                },
            );
            i += 1;
            if tokens.get(i).is_some_and(|t| t.text(ast.source) == "(") {
                i += 3;
            }
        } else {
            i += 1;
        }
    }
    Some(result)
}
pub(crate) fn reflow(text: &str) -> String {
    reflow_width(text, 80)
}
pub(crate) fn reflow_width(text: &str, limit: usize) -> String {
    let mut output = String::new();
    let mut column = 0;
    for word in text.split_whitespace() {
        let width = word.chars().count();
        if column > 0 {
            if column + 1 + width > limit {
                output.push('\n');
                column = 0;
            } else {
                output.push(' ');
                column += 1;
            }
        }
        output.push_str(word);
        column += width;
    }
    output
}
/// Lay out a paragraph at Elm's terminal width without counting styling as text.
/// A word can cross chunk boundaries (e.g. an underlined Hint and plain colon).
pub(crate) fn reflow_chunks(chunks: Vec<Value>) -> Vec<Value> {
    let mut words = Vec::new();
    let mut word = Vec::new();
    for chunk in chunks {
        let value = chunk
            .as_str()
            .or_else(|| chunk["string"].as_str())
            .unwrap_or("");
        for part in value.split_inclusive(char::is_whitespace) {
            let token = part.trim_end_matches(char::is_whitespace);
            if !token.is_empty() {
                if chunk.is_string() {
                    text(&mut word, token.into());
                } else {
                    let mut styled = chunk.clone();
                    styled["string"] = json!(token);
                    word.push(styled);
                }
            }
            if token.len() != part.len() && !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    let mut output = Vec::new();
    let mut column = 0;
    for word in words {
        let width: usize = word
            .iter()
            .map(|chunk| {
                chunk
                    .as_str()
                    .or_else(|| chunk["string"].as_str())
                    .unwrap_or("")
                    .chars()
                    .count()
            })
            .sum();
        if column != 0 {
            if column + 1 + width > 80 {
                text(&mut output, "\n".into());
                column = 0;
            } else {
                text(&mut output, " ".into());
                column += 1;
            }
        }
        for chunk in word {
            if let Some(value) = chunk.as_str() {
                text(&mut output, value.into());
            } else {
                output.push(chunk);
            }
        }
        column += width;
    }
    output
}
pub(crate) fn text(chunks: &mut Vec<Value>, value: String) {
    if let Some(Value::String(previous)) = chunks.last_mut() {
        previous.push_str(&value);
    } else {
        chunks.push(Value::String(value));
    }
}
fn red(value: String) -> Value {
    json!({"bold":false,"underline":false,"color":"RED","string":value})
}

fn snippet(source: &str, span: Span, preface: &str) -> Option<Vec<Value>> {
    let (line, column) = position(source, span.start)?;
    let (end_line, end_column) = position(source, span.end)?;
    snippet_positions(source, (line, column), (end_line, end_column), preface)
}

pub(crate) fn snippet_positions(
    source: &str,
    (line, column): (usize, usize),
    (end_line, end_column): (usize, usize),
    preface: &str,
) -> Option<Vec<Value>> {
    let mut message = vec![];
    text(&mut message, format!("{}\n\n", reflow(preface)));
    let width = end_line.to_string().len();
    for (index, source_line) in source
        .split('\n')
        .enumerate()
        .skip(line - 1)
        .take(end_line - line + 1)
    {
        text(&mut message, format!("{:>width$}|", index + 1));
        if line == end_line {
            text(&mut message, " ".into());
        } else {
            message.push(red(">".into()));
        }
        text(&mut message, format!("{source_line}\n"));
    }
    if line == end_line {
        text(&mut message, " ".repeat(column + width + 1));
        message.push(red("^".repeat(end_column.saturating_sub(column).max(1))));
    }
    Some(message)
}

pub(crate) fn snippet_highlight(
    source: &str,
    region: Span,
    start: (usize, usize),
    end: (usize, usize),
    preface: &str,
) -> Option<Vec<Value>> {
    let first = position(source, region.start)?.0;
    let last = position(source, region.end)?.0;
    let width = last.to_string().len();
    let underline = start.0 == end.0 && end.0 >= last;
    let mut message = vec![];
    text(&mut message, format!("{}\n\n", reflow(preface)));
    for (offset, line) in source
        .split('\n')
        .enumerate()
        .skip(first - 1)
        .take(last - first + 1)
    {
        let number = offset + 1;
        text(&mut message, format!("{number:>width$}|"));
        if !underline && (start.0..=end.0).contains(&number) {
            message.push(red(">".into()));
        } else {
            text(&mut message, " ".into());
        }
        text(&mut message, format!("{line}\n"));
    }
    if underline {
        text(&mut message, " ".repeat(start.1 + width + 1));
        message.push(red("^".repeat(end.1.saturating_sub(start.1).max(1))));
    }
    Some(message)
}

/// The exposing region includes parentheses. The absent-overview region is
/// the whitespace/comments consumed immediately after that closing parenthesis.
fn header_regions(ast: &Syntax<'_>) -> Option<(Span, Span)> {
    let tokens = crate::lexer::lex(ast.source).ok()?;
    let start = tokens
        .iter()
        .position(|token| token.text(ast.source) == "exposing")?
        + 1;
    let mut depth = 0usize;
    for index in start..tokens.len() {
        match tokens[index].text(ast.source) {
            "(" => depth += 1,
            ")" => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    let end = tokens[index].end;
                    let next = tokens
                        .get(index + 1)
                        .map_or(ast.source.len() as u32, |token| token.start);
                    return Some((
                        Span {
                            start: tokens[start].start,
                            end,
                        },
                        Span {
                            start: end,
                            end: next,
                        },
                    ));
                }
            }
            _ => {}
        }
    }
    None
}

pub fn report(ast: &Syntax<'_>, error: &Error, path: &std::path::Path) -> Option<Value> {
    let (title, span, preface, advice) = match error {
        Error::ImplicitExposing => (
            "IMPLICIT EXPOSING",
            header_regions(ast)?.0,
            "I need you to be explicit about what this module exposes:",
            "A great API usually hides some implementation details, so it is rare that everything in the file should be exposed. And requiring package authors to be explicit about this is a way of adding another quality check before code gets published. So as you write out the public API, ask yourself if it will be easy to understand as people read the documentation!",
        ),
        Error::MissingOverview => (
            "NO DOCS",
            header_regions(ast)?.1,
            "You must have a documentation comment between the module declaration and the imports.",
            "Learn more at <https://package.elm-lang.org/help/documentation-format>",
        ),
        Error::Syntax {
            offset,
            column,
            message,
        } => return syntax_report(ast, *offset, *column, message, path),
        Error::Names(problems) => return name_report(ast, problems, path),
        _ => return definition_report(ast, error, path),
    };
    let (line, column) = position(ast.source, span.start)?;
    let (end_line, end_column) = position(ast.source, span.end)?;
    let mut message = snippet(ast.source, span, preface)?;
    text(&mut message, format!("\n{}", reflow(advice)));
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":ast.header.name,"problems":[{
            "title":title,"region":{"start":{"line":line,"column":column},"end":{"line":end_line,"column":end_column}},"message":message
        }]}]}),
    )
}

fn syntax_report(
    ast: &Syntax<'_>,
    offset: u32,
    column: u32,
    kind: &str,
    path: &std::path::Path,
) -> Option<Value> {
    let details = match kind {
        "expected a documented name" => {
            "I was expecting to see the name of another exposed value from this module."
        }
        "expected a parenthesized operator" => {
            "I am trying to parse an operator like (+) or (*) but something is going wrong."
        }
        "reserved documentation operator" => {
            "I am trying to parse an operator like (+) or (*) but it looks like you are using a reserved symbol in this case."
        }
        _ => return None,
    };
    let (line, _) = position(ast.source, offset)?;
    let column = column as usize;
    let mut message = snippet_positions(
        ast.source,
        (line, column),
        (line, column),
        "I was partway through parsing your module documentation, but I got stuck here:",
    )?;
    text(&mut message, format!("\n{}\n\n", reflow(details)));
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Hint"}));
    text(&mut message, ": Read through <https://package.elm-lang.org/help/documentation-format> for\ntips on how to write module documentation!".into());
    let point = json!({"line":line,"column":column});
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":ast.header.name,"problems":[{
            "title":"PROBLEM IN DOCS","region":{"start":point,"end":point},"message":message
        }]}]}),
    )
}

// Documentation spacing ignores CR, while prose and nested comments count it.
// Keep the parser column rather than reconstructing it from raw source bytes.
fn name_positions(
    source: &str,
    span: Span,
    column: u32,
) -> Option<((usize, usize), (usize, usize))> {
    let line = position(source, span.start)?.0;
    let width = source
        .get(span.start as usize..span.end as usize)?
        .chars()
        .count();
    Some(((line, column as usize), (line, column as usize + width)))
}

fn name_report(ast: &Syntax<'_>, errors: &[NameProblem], path: &std::path::Path) -> Option<Value> {
    let export_spans = exports(ast)?;
    let mut problems = Vec::new();
    for error in errors {
        let (title, start, end, message) = match error {
            NameProblem::OnlyInDocs { name, span, column } => {
                let (start, end) = name_positions(ast.source, *span, *column)?;
                let mut message = snippet_positions(
                    ast.source,
                    start,
                    end,
                    &format!(
                        "I do not see `{name}` in the `exposing` list, but it is in your module documentation:"
                    ),
                )?;
                text(
                    &mut message,
                    format!(
                        "\n{}",
                        reflow(&format!(
                            "Does it need to be added to the `exposing` list as well? Or maybe you removed `{name}` and forgot to delete it here?"
                        ))
                    ),
                );
                ("DOCS MISTAKE", start, end, message)
            }
            NameProblem::OnlyInExports { name } => {
                let span = *export_spans.get(name)?;
                let mut message = snippet(
                    ast.source,
                    span,
                    &format!(
                        "I do not see `{name}` in your module documentation, but it is in your `exposing` list:"
                    ),
                )?;
                text(
                    &mut message,
                    format!(
                        "\n{}\n\n",
                        reflow(&format!(
                            "Add a line like `@docs {name}` to your module documentation!"
                        ))
                    ),
                );
                message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
                text(&mut message, ": See <https://elm-lang.org/0.19.1/docs> for more guidance on writing high\nquality docs.".into());
                (
                    "DOCS MISTAKE",
                    position(ast.source, span.start)?,
                    position(ast.source, span.end)?,
                    message,
                )
            }
            NameProblem::Duplicate {
                name,
                spans,
                columns,
            } => {
                let first = *spans.first()?;
                let second = *spans.get(1)?;
                let ((line1, col1), (end1, end_col1)) =
                    name_positions(ast.source, first, *columns.first()?)?;
                let ((line2, col2), (end2, end_col2)) =
                    name_positions(ast.source, second, *columns.get(1)?)?;
                let mut message;
                if line1 == end1 && end1 == line2 && line2 == end2 {
                    message = vec![];
                    text(
                        &mut message,
                        format!(
                            "{}\n\n",
                            reflow(&format!(
                                "There can only be one `{name}` in your module documentation, but it is listed twice:"
                            ))
                        ),
                    );
                    text(
                        &mut message,
                        format!(
                            "{line1}| {}\n{}",
                            ast.source.split('\n').nth(line1 - 1)?,
                            " ".repeat(col1 + line1.to_string().len() + 1)
                        ),
                    );
                    message.push(red("^".repeat(end_col1.saturating_sub(col1))));
                    text(&mut message, " ".repeat(col2.saturating_sub(end_col1)));
                    message.push(red("^".repeat(end_col2.saturating_sub(col2))));
                } else {
                    message = snippet_positions(
                        ast.source,
                        (line1, col1),
                        (end1, end_col1),
                        &format!(
                            "There can only be one `{name}` in your module documentation, but I see two. One here:"
                        ),
                    )?;
                    text(&mut message, "\n".into());
                    for chunk in snippet_positions(
                        ast.source,
                        (line2, col2),
                        (end2, end_col2),
                        "And another one over here:",
                    )? {
                        if let Value::String(value) = chunk {
                            text(&mut message, value);
                        } else {
                            message.push(chunk);
                        }
                    }
                }
                text(&mut message, "\nRemove one of them!".into());
                ("DUPLICATE DOCS", (line2, col2), (end2, end_col2), message)
            }
        };
        let (line, column) = start;
        let (end_line, end_column) = end;
        problems.push(json!({"title":title,"region":{"start":{"line":line,"column":column},"end":{"line":end_line,"column":end_column}},"message":message}));
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":ast.header.name,"problems":problems}]}),
    )
}

pub fn definition_report(ast: &Syntax<'_>, error: &Error, path: &std::path::Path) -> Option<Value> {
    let Error::Definitions(errors) = error else {
        return None;
    };
    let export_spans = exports(ast)?;
    let mut comment_spans: BTreeMap<&str, Vec<Span>> = BTreeMap::new();
    // Spans are handed out in the order the definitions are checked.
    let mut ordered_exports: Vec<_> = export_spans.iter().collect();
    ordered_exports.sort_by(|(left, _), (right, _)| crate::edition::compare_documented(left, right));
    for (name, span) in ordered_exports {
        let real_name = ast
            .declarations
            .iter()
            .find_map(|d| match d {
                Declaration::Infix {
                    operator, function, ..
                } if operator == name => Some(*function),
                _ => None,
            })
            .unwrap_or(name);
        comment_spans.entry(real_name).or_default().push(*span);
    }
    let mut used: BTreeMap<&str, usize> = BTreeMap::new();
    let mut problems = Vec::new();
    for error in errors {
        let (name, annotation) = match error {
            DefinitionProblem::NoAnnotation { name } => (name.as_str(), true),
            DefinitionProblem::NoComment { name } => (name.as_str(), false),
        };
        let span = if annotation {
            // An operator is reported under its own name but shown at its implementation.
            let implementation = ast
                .declarations
                .iter()
                .find_map(|d| match d {
                    Declaration::Infix { operator, function, .. } if *operator == name => Some(*function),
                    _ => None,
                })
                .unwrap_or(name);
            ast.declarations.iter().find_map(|d| match d {
                Declaration::Value { name: found, .. } if *found == implementation => {
                    source_span(ast.source, found)
                }
                _ => None,
            })?
        } else {
            let index = used.entry(name).or_default();
            let span = *comment_spans.get(name)?.get(*index)?;
            *index += 1;
            span
        };
        let (line, column) = position(ast.source, span.start)?;
        let (end_line, end_column) = position(ast.source, span.end)?;
        let (title, missing, advice) = if annotation {
            (
                "NO TYPE ANNOTATION",
                "a type annotation",
                "I use the type variable names from your annotations when generating docs. So if you say `Html msg` in your type annotation, I can use `msg` in the docs and make them a bit clearer. So add an annotation and try to use nice type variables!",
            )
        } else {
            (
                "NO DOCS",
                "a documentation comment",
                "Add documentation with nice examples of how to use it!",
            )
        };
        let mut message = snippet(
            ast.source,
            span,
            &format!("The `{name}` definition does not have {missing}."),
        )?;
        text(&mut message, format!("\n{}\n\n", reflow(advice)));
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
        text(&mut message,": Read <https://elm-lang.org/0.19.1/docs> for more advice on writing great\ndocs. There are a couple important tricks!".into());
        problems.push(json!({"title":title,"region":{"start":{"line":line,"column":column},"end":{"line":end_line,"column":end_column}},"message":message}));
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":ast.header.name,"problems":problems}]}),
    )
}

/// Render documentation errors using Elm's terminal layout and TTY detection.
/// The caller supplies the final newline when writing this string.
pub fn terminal(report: &Value) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    let root = cwd
        .ancestors()
        .find(|dir| dir.join("elm.json").is_file())
        .unwrap_or(&cwd);
    render_terminal(report, root, std::io::stderr().is_terminal())
}

pub fn terminal_in_root(report: &Value, root: &std::path::Path) -> String {
    render_terminal(report, root, false)
}

fn styled(output: &mut String, content: &str, style: &str, ansi: bool) {
    if ansi && !style.is_empty() {
        output.push_str("\x1b[");
        output.push_str(style);
        output.push('m');
        output.push_str(content);
        output.push_str("\x1b[0m");
    } else {
        output.push_str(content);
    }
}

fn render_terminal(report: &Value, root: &std::path::Path, ansi: bool) -> String {
    let mut output = String::new();
    if let Some(errors) = report["errors"].as_array() {
        let mut errors: Vec<_> = errors.iter().collect();
        errors.sort_by_key(|error| {
            let path = std::path::Path::new(error["path"].as_str().unwrap_or(""));
            std::fs::metadata(root.join(path))
                .and_then(|m| m.modified())
                .ok()
        });
        for (index, error) in errors.iter().enumerate() {
            if index > 0 {
                let before = format!(
                    "{}  ↑    ",
                    errors[index - 1]["name"].as_str().unwrap_or("")
                );
                let after = error["name"].as_str().unwrap_or("");
                styled(
                    &mut output,
                    &format!(
                        "{}{before}\n====o======================================================================o====\n    ↓  {after}\n\n",
                        " ".repeat(80usize.saturating_sub(before.chars().count()))
                    ),
                    "31",
                    ansi,
                );
                output.push('\n');
            }
            if let Some(problems) = error["problems"].as_array() {
                for problem in problems {
                    let title = problem["title"].as_str().unwrap_or("DOCS ERROR");
                    let absolute = std::path::Path::new(error["path"].as_str().unwrap_or(""));
                    let relative = absolute
                        .strip_prefix(root)
                        .unwrap_or(absolute)
                        .to_string_lossy();
                    let dashes = 80usize
                        .saturating_sub(5 + title.chars().count() + relative.chars().count())
                        .max(1);
                    styled(
                        &mut output,
                        &format!("-- {title} {} {relative}", "-".repeat(dashes)),
                        "36",
                        ansi,
                    );
                    output.push_str("\n\n");
                    if let Some(chunks) = problem["message"].as_array() {
                        for chunk in chunks {
                            let content = chunk
                                .as_str()
                                .or_else(|| chunk["string"].as_str())
                                .unwrap_or("");
                            // Documentation reports use red carets and underlined Note/Hint.
                            let style = if chunk["color"].as_str() == Some("RED") {
                                "91"
                            } else if chunk["color"].as_str() == Some("CYAN") {
                                "96"
                            } else if chunk["color"].as_str() == Some("BLUE") {
                                "94"
                            } else if chunk["color"].as_str() == Some("GREEN") {
                                "92"
                            } else if chunk["color"].as_str() == Some("yellow") {
                                "33"
                            } else if chunk["underline"].as_bool() == Some(true) {
                                "4"
                            } else {
                                ""
                            };
                            styled(&mut output, content, style, ansi);
                        }
                    }
                    output.push_str("\n\n");
                }
            }
        }
    }
    output.pop(); // main's eprintln supplies the final newline.
    output
}
