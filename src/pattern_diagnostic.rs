//! Pattern syntax reports following Reporting.Error.Syntax.toPTupleReport.
use crate::docs_diagnostic::{reflow, snippet_positions, text};
use serde_json::{Value, json};
use std::path::Path;

pub fn report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    kind: &str,
) -> Option<Value> {
    if let Some(width) = kind.strip_prefix("pattern float ") {
        return float_report(
            source,
            name,
            path,
            line,
            column,
            width.split(';').next()?.parse().ok()?,
        );
    }
    if let Some(kind) = kind.strip_prefix("pattern alias ") {
        return alias_report(
            source,
            name,
            path,
            line,
            column,
            kind.starts_with("indent "),
        );
    }
    if kind.starts_with("pattern wildcard name;") {
        return wildcard_report(source, name, path, line, column);
    }
    if let Some(start) = kind.strip_prefix("pattern start indent ") {
        return cons_indent_report(source, name, path, line, column, start.parse().ok()?);
    }
    if let Some(context) = kind.strip_prefix("pattern start ") {
        return start_report(source, name, path, line, column, context.split(';').next()?);
    }
    if let Some(details) = kind.strip_prefix("pattern list ") {
        return list_report(source, name, path, line, column, details);
    }
    if let Some(details) = kind.strip_prefix("pattern record ") {
        return record_report(source, name, path, line, column, details);
    }
    let details = kind.strip_prefix("pattern parentheses ")?;
    let (phase, rest) = details.split_once(' ')?;
    let ending = match phase {
        "open" | "indent-open" | "indent-end" | "indent-next" => false,
        "end" => true,
        _ => return None,
    };
    let start_line: usize = rest.split(';').next()?.parse().ok()?;
    if start_line == 0 || start_line > line || column == 0 {
        return None;
    }
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
    let indentation = phase.starts_with("indent-");
    let keyword = !indentation && crate::parser::reserved(&word);
    let operator: String = tail
        .chars()
        .take_while(|c| "+-/*=.<>:&|^?%!".contains(*c))
        .collect();
    let close = match tail.chars().next() {
        Some(']') => Some(("square bracket", ']', "STRAY SQUARE BRACKET")),
        Some('}') => Some(("curly brace", '}', "STRAY CURLY BRACE")),
        Some(')') => Some(("parenthesis", ')', "STRAY PARENTHESIS")),
        _ => None,
    };
    let (title, width, preface) = if indentation {
        match phase {
            "indent-open" => (
                "UNFINISHED PARENTHESES",
                0,
                "I just saw an open parenthesis, but then I got stuck here:".into(),
            ),
            "indent-end" => (
                "UNFINISHED PARENTHESES",
                0,
                "I was expecting a closing parenthesis next:".into(),
            ),
            _ => (
                "UNFINISHED TUPLE PATTERN",
                0,
                "I am partway through parsing a tuple pattern, but I got stuck here:".into(),
            ),
        }
    } else if keyword {
        (
            "RESERVED WORD",
            word.len(),
            if ending {
                "I ran into a reserved word in this pattern:".into()
            } else {
                format!("It looks like you are trying to use `{word}` as a variable name:")
            },
        )
    } else if ending && !operator.is_empty() {
        (
            "UNEXPECTED SYMBOL",
            operator.len(),
            format!("I ran into the {operator} symbol unexpectedly in this pattern:"),
        )
    } else if ending && let Some((term, _, title)) = close {
        (
            title,
            0,
            format!("I ran into a an unexpected {term} in this pattern:"),
        )
    } else {
        (
            "UNFINISHED PARENTHESES",
            0,
            if ending {
                "I was partway through parsing a pattern, but I got stuck here:".into()
            } else {
                "I just saw an open parenthesis, but I got stuck here:".into()
            },
        )
    };
    let mut message = vec![];
    text(&mut message, format!("{}\n\n", reflow(&preface)));
    let digits = line.to_string().len();
    for (index, source_line) in source
        .split('\n')
        .enumerate()
        .skip(start_line - 1)
        .take(line - start_line + 1)
    {
        text(
            &mut message,
            format!("{:>digits$}| {source_line}\n", index + 1),
        );
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(
        json!({"bold":false,"underline":false,"color":"RED","string":"^".repeat(width.max(1))}),
    );
    if phase == "indent-end" {
        text(&mut message, "\nTry adding a ".into());
        message.push(json!({"bold":false,"underline":false,"color":"yellow","string":")"}));
        text(&mut message, " to see if that helps?".into());
        note(
            &mut message,
            "I can get confused by indentation in cases like this, so maybe you have a closing parenthesis but it is not indented enough?",
        );
    } else if phase == "indent-next" {
        let advice = reflow(
            "I was expecting to see a pattern next. I am expecting the final result to be something like (x,y) or (name, _).",
        );
        pattern_examples(&mut message, &advice)?;
        note(
            &mut message,
            "I can get confused by indentation in cases like this, so the problem may be that the next part is not indented enough?",
        );
    } else if keyword {
        text(
            &mut message,
            format!(
                "\n{}",
                reflow(&if ending {
                    format!("The `{word}` keyword is reserved. Try using a different name instead!")
                } else {
                    "This is a reserved word! Try using some other name?".into()
                })
            ),
        );
    } else if ending && !operator.is_empty() {
        text(
            &mut message,
            format!(
                "\n{}",
                reflow(
                    "Only the :: symbol that works in patterns. It is useful if you are pattern matching on lists, trying to get the first element off the front. Did you want that instead?"
                )
            ),
        );
    } else if ending && let Some((term, bracket, _)) = close {
        text(
            &mut message,
            format!(
                "\n{}",
                reflow(&format!(
                    "This {bracket} does not match up with an earlier open {term}. Try deleting it?"
                ))
            ),
        );
    } else if ending {
        let advice = reflow(
            "I was expecting a closing parenthesis next, so try adding a ) to see if that helps?",
        );
        let (prefix, suffix) = advice.split_once(')')?;
        text(&mut message, format!("\n{prefix}"));
        message.push(json!({"bold":false,"underline":false,"color":"yellow","string":")"}));
        text(&mut message, suffix.into());
    } else {
        let advice = reflow(
            "I was expecting to see a pattern next. Maybe it will end up being something like (x,y) or (name, _)?",
        );
        pattern_examples(&mut message, &advice)?;
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":title,"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
        }]}]}),
    )
}

fn pattern_examples(message: &mut Vec<Value>, advice: &str) -> Option<()> {
    let (prefix, suffix) = advice.split_once("(x,y)")?;
    let (middle, end) = suffix.split_once("(name, _)")?;
    text(message, format!("\n{prefix}"));
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"(x,y)"}));
    text(message, middle.into());
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"(name, _)"}));
    text(message, end.into());
    Some(())
}
fn note(message: &mut Vec<Value>, advice: &str) {
    text(message, "\n\n".into());
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
    let rendered = reflow(&format!("Note: {advice}"));
    text(message, rendered[4..].into());
}

fn start_report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    context: &str,
) -> Option<Value> {
    if line == 0 || column == 0 || !matches!(context, "argument" | "binding") {
        return None;
    }
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
    let keyword = crate::parser::reserved(&word);
    let operator: String = tail
        .chars()
        .take_while(|c| "+-/*=.<>:&|^?%!".contains(*c))
        .collect();
    let minus = operator == "-";
    let (title, width, preface) = if keyword {
        let placement = if context == "argument" {
            "as an argument"
        } else {
            "in this pattern"
        };
        (
            "RESERVED WORD",
            word.len(),
            format!("It looks like you are trying to use `{word}` {placement}:"),
        )
    } else if minus {
        (
            "UNEXPECTED SYMBOL",
            0,
            "I ran into a minus sign unexpectedly in this pattern:".into(),
        )
    } else {
        (
            "PROBLEM IN PATTERN",
            0,
            "I wanted to parse a pattern next, but I got stuck here:".into(),
        )
    };
    let mut message = snippet_positions(source, (line, column), (line, column + width), &preface)?;
    if keyword {
        text(
            &mut message,
            "\nThis is a reserved word! Try using some other name?".into(),
        );
    } else if minus {
        text(
            &mut message,
            format!(
                "\n{}",
                reflow(
                    "It is not possible to pattern match on negative numbers at this time. Try using an `if` expression for that sort of thing for now."
                )
            ),
        );
    } else {
        let advice = reflow(
            "I am not sure why I am getting stuck exactly. I just know that I want a pattern next. Something as simple as maybeHeight or result would work!",
        );
        let (prefix, rest) = advice.split_once("maybeHeight")?;
        let (middle, suffix) = rest.split_once("result")?;
        text(&mut message, format!("\n{prefix}"));
        message
            .push(json!({"bold":false,"underline":false,"color":"yellow","string":"maybeHeight"}));
        text(&mut message, middle.into());
        message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"result"}));
        text(&mut message, suffix.into());
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":title,"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
        }]}]}),
    )
}

fn list_report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    details: &str,
) -> Option<Value> {
    let (phase, start) = details.split_once(' ')?;
    let start_line: usize = start.split(';').next()?.parse().ok()?;
    if start_line == 0 || start_line > line || column == 0 {
        return None;
    }
    let word: String = source
        .split('\n')
        .nth(line - 1)?
        .chars()
        .skip(column - 1)
        .take_while(|c| crate::unicode::is_alphanumeric(*c) || *c == '_')
        .collect();
    let keyword = phase == "open" && crate::parser::reserved(&word);
    let width = if keyword { word.len() } else { 0 };
    let title = if keyword {
        "RESERVED WORD"
    } else {
        "UNFINISHED LIST PATTERN"
    };
    let preface = if keyword {
        format!("It looks like you are trying to use `{word}` to name an element of a list:")
    } else {
        match phase {
            "open" | "indent-open" => {
                "I just saw an open square bracket, but then I got stuck here:"
            }
            "end" | "indent-end" => {
                "I was expecting a closing square bracket to end this list pattern:"
            }
            "indent-next" => "I am partway through parsing a list pattern, but I got stuck here:",
            _ => return None,
        }
        .into()
    };
    let mut message = vec![];
    text(&mut message, format!("{}\n\n", reflow(&preface)));
    let digits = line.to_string().len();
    for (index, source_line) in source
        .split('\n')
        .enumerate()
        .skip(start_line - 1)
        .take(line - start_line + 1)
    {
        text(
            &mut message,
            format!("{:>digits$}| {source_line}\n", index + 1),
        );
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(
        json!({"bold":false,"underline":false,"color":"RED","string":"^".repeat(width.max(1))}),
    );
    if keyword {
        text(
            &mut message,
            "\nThis is a reserved word though! Try using some other name?".into(),
        );
    } else if phase == "indent-next" {
        text(
            &mut message,
            "\nI was expecting to see another pattern next. Maybe a variable name.".into(),
        );
    } else {
        text(&mut message, "\nTry adding a ".into());
        message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"]"}));
        text(&mut message, " to see if that helps?".into());
    }
    match phase {
        "indent-open" => note(
            &mut message,
            "I can get confused by indentation in cases like this, so maybe there is something next, but it is not indented enough?",
        ),
        "indent-end" => note(
            &mut message,
            "I can get confused by indentation in cases like this, so maybe you have a closing square bracket but it is not indented enough?",
        ),
        "indent-next" => note(
            &mut message,
            "I can get confused by indentation in cases like this, so maybe there is more to this pattern but it is not indented enough?",
        ),
        _ => (),
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":title,"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
        }]}]}),
    )
}

fn record_report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    details: &str,
) -> Option<Value> {
    let (phase, start) = details.split_once(' ')?;
    let start_line: usize = start.split(';').next()?.parse().ok()?;
    if start_line == 0
        || start_line > line
        || column == 0
        || !matches!(
            phase,
            "open" | "field" | "end" | "indent-open" | "indent-field" | "indent-end"
        )
    {
        return None;
    }
    let word: String = source
        .split('\n')
        .nth(line - 1)?
        .chars()
        .skip(column - 1)
        .take_while(|c| crate::unicode::is_alphanumeric(*c) || *c == '_')
        .collect();
    let keyword = phase == "field" && crate::parser::reserved(&word);
    let width = if keyword { word.len() } else { 0 };
    let title = if keyword {
        "RESERVED WORD"
    } else {
        "UNFINISHED RECORD PATTERN"
    };
    let preface = if keyword {
        format!("I was not expecting to see `{word}` as a record field name:")
    } else {
        "I was partway through parsing a record pattern, but I got stuck here:".into()
    };
    let mut message = vec![];
    text(&mut message, format!("{}\n\n", reflow(&preface)));
    let digits = line.to_string().len();
    for (index, source_line) in source
        .split('\n')
        .enumerate()
        .skip(start_line - 1)
        .take(line - start_line + 1)
    {
        text(
            &mut message,
            format!("{:>digits$}| {source_line}\n", index + 1),
        );
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(
        json!({"bold":false,"underline":false,"color":"RED","string":"^".repeat(width.max(1))}),
    );
    if keyword {
        text(
            &mut message,
            format!(
                "\n{}",
                reflow(
                    "This is a reserved word, not available for variable names. Try another name!"
                )
            ),
        );
    } else {
        if matches!(phase, "end" | "indent-end") {
            let advice =
                reflow("I was expecting to see a closing curly brace next. Try adding a } here?");
            let (prefix, suffix) = advice.split_once('}')?;
            text(&mut message, format!("\n{prefix}"));
            message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"}"}));
            text(&mut message, suffix.into());
        } else {
            text(
                &mut message,
                "\nI was expecting to see a field name next.".into(),
            );
        }
        text(&mut message, "\n\n".into());
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Hint"}));
        let hint = reflow(
            "Hint: A record pattern looks like {x,y} or {name,age} where you list the field names you want to access.",
        );
        let (prefix, rest) = hint[4..].split_once("{x,y}")?;
        let (middle, suffix) = rest.split_once("{name,age}")?;
        text(&mut message, prefix.into());
        message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"{x,y}"}));
        text(&mut message, middle.into());
        message
            .push(json!({"bold":false,"underline":false,"color":"yellow","string":"{name,age}"}));
        text(&mut message, suffix.into());
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":title,"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
        }]}]}),
    )
}

fn wildcard_report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
) -> Option<Value> {
    if line == 0 || column == 0 {
        return None;
    }
    let word: String = source
        .split('\n')
        .nth(line - 1)?
        .chars()
        .skip(column - 1)
        .take_while(|c| crate::unicode::is_alpha(*c) || c.is_ascii_digit() || *c == '_')
        .collect();
    let remainder = word.trim_start_matches('_');
    let suggestion = remainder.chars().next().map(|c| {
        format!(
            "{}{}",
            crate::unicode::to_lower(c),
            &remainder[c.len_utf8()..]
        )
    });
    let mut message = snippet_positions(
        source,
        (line, column),
        (line, column + word.chars().count()),
        "Variable names cannot start with underscores like this:",
    )?;
    // Render each highlighted example as a single document word, as Elm does.
    let examples = suggestion.as_deref().unwrap_or("x or age");
    let advice = reflow(&format!(
        "You can either have an underscore like _ to ignore the value, or you can have a name like {examples} to use the matched value."
    ));
    let (prefix, rest) = advice.split_once('_')?;
    text(&mut message, format!("\n{prefix}"));
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"_"}));
    let marker = "name like";
    let (middle, rest) = rest.split_once(marker)?;
    text(&mut message, format!("{middle}{marker}"));
    let trimmed = rest.trim_start_matches([' ', '\n']);
    text(&mut message, rest[..rest.len() - trimmed.len()].into());
    if let Some(suggestion) = suggestion {
        let suffix = trimmed.strip_prefix(&suggestion)?;
        message.push(json!({"bold":false,"underline":false,"color":"yellow","string":suggestion}));
        text(&mut message, suffix.into());
    } else {
        let rest = trimmed.strip_prefix('x')?;
        let (between, suffix) = rest.split_once("age")?;
        message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"x"}));
        text(&mut message, between.into());
        message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"age"}));
        text(&mut message, suffix.into());
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":"UNEXPECTED NAME","region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+word.chars().count()}},"message":message
        }]}]}),
    )
}

fn alias_report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    indentation: bool,
) -> Option<Value> {
    let mut message = snippet_positions(
        source,
        (line, column),
        (line, column),
        "I was expecting to see a variable name after the `as` keyword:",
    )?;
    let chunks: Vec<(&str, Option<&str>)> = vec![
        ("The `as` keyword lets you write patterns like ((", None),
        ("x", Some("yellow")),
        (",", None),
        ("y", Some("yellow")),
        (") ", None),
        ("as", Some("CYAN")),
        (" point", Some("yellow")),
        (
            ") so you can refer to individual parts of the tuple with ",
            None,
        ),
        ("x", Some("yellow")),
        (" and ", None),
        ("y", Some("yellow")),
        (" or you refer to the whole thing with ", None),
        (if indentation { "point." } else { "point" }, Some("yellow")),
        (if indentation { "" } else { "." }, None),
    ];
    let raw: String = chunks.iter().map(|(s, _)| *s).collect();
    let wrapped = reflow(&raw);
    // The fixed ASCII prose has single spaces; wrapping replaces them with
    // newlines, preserving byte offsets for the colored document fragments.
    if raw.len() != wrapped.len() {
        return None;
    }
    text(&mut message, "\n".into());
    let mut offset = 0;
    for (chunk, color) in chunks {
        let rendered = &wrapped[offset..offset + chunk.len()];
        if let Some(color) = color {
            // Elm retains an empty plain fragment between adjacent colors.
            if message.last().is_some_and(Value::is_object) {
                message.push(json!(""));
            }
            message.push(json!({"bold":false,"underline":false,"color":color,"string":rendered}));
        } else {
            text(&mut message, rendered.into());
        }
        offset += chunk.len();
    }
    text(
        &mut message,
        format!(
            "\n\n{}",
            reflow(
                "So I was expecting to see a variable name after the `as` keyword here. Sometimes people just want to use `as` as a variable name though. Try using a different name in that case!"
            )
        ),
    );
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":"UNFINISHED PATTERN","region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
        }]}]}),
    )
}

fn float_report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    width: usize,
) -> Option<Value> {
    let mut message = snippet_positions(
        source,
        (line, column),
        (line, column + width),
        "I cannot pattern match with floating point numbers:",
    )?;
    let example = "(abs (actual - expected) < 0.001)";
    let advice = reflow(&format!(
        "Equality on floats can be unreliable, so you usually want to check that they are nearby with some sort of {example} check."
    ));
    let (prefix, suffix) = advice.split_once(example)?;
    text(&mut message, format!("\n{prefix}"));
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":example}));
    text(&mut message, suffix.into());
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":"UNEXPECTED PATTERN","region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
        }]}]}),
    )
}

fn cons_indent_report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    start_line: usize,
) -> Option<Value> {
    if start_line == 0 || start_line > line || column == 0 {
        return None;
    }
    let mut message = vec![];
    text(
        &mut message,
        "I wanted to parse a pattern next, but I got stuck here:\n\n".into(),
    );
    let digits = line.to_string().len();
    for (index, source_line) in source
        .split('\n')
        .enumerate()
        .skip(start_line - 1)
        .take(line - start_line + 1)
    {
        text(
            &mut message,
            format!("{:>digits$}| {source_line}\n", index + 1),
        );
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(json!({"bold":false,"underline":false,"color":"RED","string":"^"}));
    let advice = reflow(
        "I am not sure why I am getting stuck exactly. I just know that I want a pattern next. Something as simple as maybeHeight or result would work!",
    );
    let (prefix, rest) = advice.split_once("maybeHeight")?;
    let (middle, suffix) = rest.split_once("result")?;
    text(&mut message, format!("\n{prefix}"));
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"maybeHeight"}));
    text(&mut message, middle.into());
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":"result"}));
    text(&mut message, suffix.into());
    note(
        &mut message,
        "I can get confused by indentation. If you think there is a pattern next, maybe it needs to be indented a bit more?",
    );
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":"UNFINISHED PATTERN","region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
        }]}]}),
    )
}
