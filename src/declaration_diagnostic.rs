//! Declaration-start reports corresponding to Reporting.Error.Syntax.
use crate::docs_diagnostic::{reflow, snippet_positions, text};
use serde_json::{Value, json};
use std::path::Path;

/// An EOF immediately after a top-level definition's equals sign. Other
/// unfinished expressions need their own parser context and are not guessed.
pub fn unfinished_definition(
    source: &str,
    module: &str,
    path: &Path,
    explanation: &str,
) -> Option<Value> {
    if !explanation.starts_with("expected an indented continuation;")
        && !explanation.starts_with("expected expression;")
    {
        return None;
    }
    let tokens = crate::lexer::lex(source).ok()?;
    let equals = tokens.last()?;
    if equals.text(source) != "=" || !source[equals.end as usize..].trim().is_empty() {
        return None;
    }
    let first = tokens.iter().find(|token| token.row == equals.row)?;
    let name = first.text(source);
    if first.column != 1 || first.kind != crate::lexer::Kind::Lower || crate::parser::reserved(name)
    {
        return None;
    }
    let line = equals.row as usize;
    let column = equals.column as usize + 1;
    let mut message = snippet_positions(
        source,
        (line, column),
        (line, column),
        &format!("I got stuck while parsing the `{name}` definition:"),
    )?;
    text(&mut message, "\nI was expecting to see an expression next. What is it equal to?\n\nHere is a valid definition (with a type annotation) for reference:\n\n    greet : String -> String\n    greet name =\n      ".into());
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"\"Hello \""}));
    text(&mut message, " ++ name ++ ".into());
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"\"!\""}));
    text(&mut message, "\n\nThe top line (called a \"type annotation\") is optional. You can leave it off if\nyou want. As you get more comfortable with Elm and as your project grows, it\nbecomes more and more valuable to add them though! They work great as\ncompiler-verified documentation, and they often improve error messages!".into());
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{
            "title":"UNFINISHED DEFINITION","region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
        }]}]}),
    )
}

pub fn report(source: &str, name: &str, path: &Path, line: usize, column: usize) -> Option<Value> {
    let tail: String = source
        .split('\n')
        .nth(line - 1)?
        .chars()
        .skip(column - 1)
        .collect();
    let word: String = tail
        .chars()
        .take_while(|c| crate::unicode::is_alphanumeric(*c) || *c == '_')
        .collect();
    let close = match tail.chars().next() {
        Some(')') => Some(("parenthesis", ")", "STRAY PARENTHESIS")),
        Some(']') => Some(("square bracket", "]", "STRAY SQUARE BRACKET")),
        Some('}') => Some(("curly brace", "}", "STRAY CURLY BRACE")),
        _ => None,
    };
    let (title, preface, width) = if let Some((term, _, title)) = close {
        (
            title,
            format!("I was not expecting to see a {term} here:"),
            0,
        )
    } else if crate::parser::reserved(&word) {
        (
            "RESERVED WORD",
            format!("I was not expecting to run into the `{word}` keyword here:"),
            word.len(),
        )
    } else if tail.chars().next().is_some_and(|c| "({[\"'@#$".contains(c)) {
        (
            "UNEXPECTED SYMBOL",
            format!(
                "I am getting stuck because this line starts with the {} symbol:",
                tail.chars().next()?
            ),
            0,
        )
    } else if tail.chars().next().is_some_and(crate::unicode::is_upper) {
        (
            "UNEXPECTED CAPITAL LETTER",
            "Declarations always start with a lower-case letter, so I am getting stuck here:"
                .into(),
            0,
        )
    } else {
        (
            "WEIRD DECLARATION",
            "I am trying to parse a declaration, but I am getting stuck here:".into(),
            0,
        )
    };
    let mut message = snippet_positions(source, (line, column), (line, column + width), &preface)?;
    if let Some((term, bracket, _)) = close {
        text(
            &mut message,
            format!(
                "\n{}",
                reflow(&format!(
                    "This {bracket} does not match up with an earlier open {term}. Try deleting it?"
                ))
            ),
        );
    } else if title == "RESERVED WORD" && matches!(word.as_str(), "if" | "case") {
        conditional_example(&mut message, &word);
    } else if title == "RESERVED WORD" {
        text(
            &mut message,
            format!(
                "\n{}",
                reflow(if word == "import" {
                    "It is reserved for declaring imports at the top of your module. If you want another import, try moving it up top with the other imports. If you want to define a value or function, try changing the name to something else!"
                } else {
                    "It is a reserved word. Try changing the name to something else?"
                })
            ),
        );
    } else {
        if title == "UNEXPECTED CAPITAL LETTER" {
            let first = word.chars().next()?;
            let suggestion = format!(
                "{}{}",
                crate::unicode::to_lower(first),
                &word[first.len_utf8()..]
            );
            let advice = reflow(&format!("Try a name like {suggestion} instead?"));
            let prefix = "Try a name like";
            let rest = advice.strip_prefix(prefix)?;
            let trimmed = rest.trim_start_matches([' ', '\n']);
            let spacing = &rest[..rest.len() - trimmed.len()];
            let suffix = trimmed.strip_prefix(&suggestion)?;
            text(&mut message, format!("\n{prefix}{spacing}"));
            message
                .push(json!({"bold":false,"underline":false,"color":"GREEN","string":suggestion}));
            text(&mut message, format!("{suffix}\n\n"));
            message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
            text(
                &mut message,
                ": Here are a couple valid declarations for reference:\n\n".into(),
            );
        } else {
            text(
                &mut message,
                format!(
                    "\n{}\n\n",
                    reflow(
                        "When a line has no spaces at the beginning, I expect it to be a declaration like one of these:"
                    )
                ),
            );
        }
        declaration_examples(&mut message);
        let advice = match title {
            "UNEXPECTED CAPITAL LETTER" => {
                "Notice that they always start with a lower-case letter. Capitalization matters!"
            }
            "UNEXPECTED SYMBOL" => {
                "If this is not supposed to be a declaration, try adding some spaces before it?"
            }
            _ => {
                "Try to make your declaration look like one of those? Or if this is not supposed to be a declaration, try adding some spaces before it?"
            }
        };
        text(&mut message, format!("\n\n{}", reflow(advice)));
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":title,"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column + width}},"message":message
        }]}]}),
    )
}

fn declaration_examples(message: &mut Vec<Value>) {
    text(
        message,
        "    greet : String -> String\n    greet name =\n      ".into(),
    );
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"\"Hello \""}));
    text(message, " ++ name ++ ".into());
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"\"!\""}));
    // D.indent prefixes the blank line inside the example block as well.
    text(message, "\n    \n    ".into());
    message.push(json!({"bold":false,"underline":false,"color":"CYAN","string":"type"}));
    text(message, " User = Anonymous | LoggedIn String".into());
}

fn conditional_example(message: &mut Vec<Value>, keyword: &str) {
    text(
        message,
        format!(
            "\n{}\n\n",
            reflow(&format!(
                "It is reserved for writing `{keyword}` expressions. Try using a different name?"
            ))
        ),
    );
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
    let article = if keyword == "if" { "an" } else { "a" };
    let note = reflow(&format!(
        "Note: If you are trying to write {article} `{keyword}` expression, it needs to be part of a definition. So you could write something like this instead:"
    ));
    text(message, note[4..].into());
    let styled = |color: &str, word: &str| json!({"bold":false,"underline":false,"color":color,"string":word});
    if keyword == "case" {
        text(message, "\n\n    getWidth maybeWidth =\n      ".into());
        message.push(styled("CYAN", "case"));
        text(message, " maybeWidth ".into());
        message.push(styled("CYAN", "of"));
        text(message, "\n        ".into());
        message.push(styled("BLUE", "Just"));
        text(message, " width ->\n          width + ".into());
        message.push(styled("yellow", "200"));
        text(message, "\n    \n        ".into());
        message.push(styled("BLUE", "Nothing"));
        text(message, " ->\n          ".into());
        message.push(styled("yellow", "400"));
        text(
            message,
            "\n\nThis defines a `getWidth` function that you can use elsewhere in your program."
                .into(),
        );
    } else {
        text(message, "\n\n    greet name =\n      ".into());
        message.push(styled("CYAN", "if"));
        text(message, " name == ".into());
        message.push(styled("yellow", "\"Abraham Lincoln\""));
        text(message, " ".into());
        message.push(styled("CYAN", "then"));
        text(message, " ".into());
        message.push(styled("yellow", "\"Greetings Mr. President.\""));
        text(message, " ".into());
        message.push(styled("CYAN", "else"));
        text(message, " ".into());
        message.push(styled("yellow", "\"Hey!\""));
        // Preserve the official example's function-name mismatch.
        text(
            message,
            format!(
                "\n\n{}",
                reflow(
                    "This defines a `reviewPowerLevel` function that you can use elsewhere in your program."
                )
            ),
        );
    }
}
