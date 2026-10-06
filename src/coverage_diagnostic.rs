//! Exhaustiveness diagnostics from Elm 0.19.1 Reporting.Error.Pattern.
use crate::{
    ast::{Span, Syntax},
    docs_diagnostic::{reflow, snippet_positions, text},
};
use serde_json::json;
use std::path::Path;

pub fn missing(
    ast: &Syntax<'_>,
    path: &Path,
    span: Span,
    context: &str,
    witnesses: &[String],
) -> String {
    let position = |offset: u32| {
        let prefix = &ast.source[..offset as usize];
        (
            prefix.bytes().filter(|c| *c == b'\n').count() + 1,
            prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
        )
    };
    let start = position(span.start);
    let end = position(span.end);
    let (title, preface, possibilities, advice, hint) = match context {
        "argument" => (
            "UNSAFE PATTERN",
            if crate::edition::fixed_possibilities_typo() {
                "This pattern does not cover all possibilities:"
            } else {
                "This pattern does not cover all possiblities:"
            },
            "Other possibilities include:",
            "I would have to crash if I saw one of those! So rather than pattern matching in function arguments, put a `case` in the function body to account for all possibilities.",
            None,
        ),
        "destruct" => (
            "UNSAFE PATTERN",
            "This pattern does not cover all possible values:",
            "Other possibilities include:",
            "I would have to crash if I saw one of those! You can use `let` to deconstruct values only if there is ONE possiblity. Switch to a `case` expression to account for all possibilities.",
            Some(
                "Are you calling a function that definitely returns values with a very specific shape? Try making the return type of that function more specific!",
            ),
        ),
        _ => (
            "MISSING PATTERNS",
            "This `case` does not have branches for all possibilities:",
            "Missing possibilities include:",
            "I would have to crash if I saw one of those. Add branches for them!",
            Some(
                "If you want to write the code for each branch later, use `Debug.todo` as a placeholder. Read <https://elm-lang.org/0.19.1/missing-patterns> for more guidance on this workflow.",
            ),
        ),
    };
    let mut message = snippet_positions(ast.source, start, end, preface).unwrap();
    text(&mut message, format!("\n{possibilities}\n\n    "));
    message.push(
        json!({"bold":false,"underline":false,"color":"yellow","string":witnesses.join("\n    ")}),
    );
    text(&mut message, format!("\n\n{}", reflow(advice)));
    if let Some(hint) = hint {
        text(&mut message, "\n\n".into());
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Hint"}));
        // Include the label in wrapping, then emit it as a separate styled chunk.
        text(&mut message, reflow(&format!("Hint: {hint}"))[4..].into());
    }
    crate::docs_diagnostic::encode(&json!({"type":"compile-errors", "errors":[{
        "path":path,"name":ast.header.name,"problems":[{"title":title,
        "region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},
        "message":message}]}]}))
}

pub fn redundant(
    ast: &Syntax<'_>,
    path: &Path,
    overall: Span,
    pattern: Span,
    index: usize,
) -> String {
    let position = |offset: u32| {
        let prefix = &ast.source[..offset as usize];
        (
            prefix.bytes().filter(|c| *c == b'\n').count() + 1,
            prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
        )
    };
    let start = position(pattern.start);
    let end = position(pattern.end);
    let first_line = position(overall.start).0;
    let last_line = position(overall.end).0;
    let width = last_line.to_string().len();
    let suffix = if (11..=13).contains(&(index % 100)) {
        "th"
    } else {
        match index % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    let mut message = vec![];
    text(
        &mut message,
        format!("The {index}{suffix} pattern is redundant:\n\n"),
    );
    let underline = start.0 == end.0 && end.0 >= last_line;
    for (offset, line) in ast
        .source
        .split('\n')
        .enumerate()
        .skip(first_line - 1)
        .take(last_line - first_line + 1)
    {
        let number = offset + 1;
        text(&mut message, format!("{number:>width$}|"));
        if !underline && (start.0..=end.0).contains(&number) {
            message.push(json!({"bold":false,"underline":false,"color":"RED","string":">"}));
        } else {
            text(&mut message, " ".into());
        }
        text(&mut message, format!("{line}\n"));
    }
    if underline {
        text(&mut message, " ".repeat(start.1 + width + 1));
        message.push(json!({"bold":false,"underline":false,"color":"RED","string":"^".repeat(end.1.saturating_sub(start.1).max(1))}));
    }
    text(
        &mut message,
        format!(
            "\n{}",
            reflow(
                "Any value with this shape will be handled by a previous pattern, so it should be removed."
            )
        ),
    );
    crate::docs_diagnostic::encode(&json!({"type":"compile-errors", "errors":[{
        "path":path,"name":ast.header.name,"problems":[{"title":"REDUNDANT PATTERN",
        "region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},
        "message":message}]}]}))
}
