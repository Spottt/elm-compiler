//! String and character literal diagnostics from Reporting.Error.Syntax.
use crate::docs_diagnostic::{reflow, snippet_positions, text};
use serde_json::{Value, json};
use std::path::Path;

fn color(value: &str, color: &str) -> Value {
    json!({"bold":false,"underline":false,"color":color,"string":value})
}
fn note(message: &mut Vec<Value>, body: &str) {
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
    text(message, reflow(&format!("Note: {body}"))[4..].to_string());
}
const MULTILINE: &str = "    \"\"\"\n    # Multi-line Strings\n    \n    - start with triple double quotes\n    - write whatever you want\n    - no need to escape newlines or double quotes\n    - end with triple double quotes\n    \"\"\"";

pub fn report(source: &str, name: &str, path: &Path, line: usize, column: usize, kind: &str) -> Option<Value> {
    if let Some(details) = kind.strip_prefix("literal unicode ") {
        return unicode_report(source, name, path, line, column, details);
    }
    let char_width = kind.strip_prefix("literal char width ").and_then(|s| s.parse::<usize>().ok());
    let (title, preface, width) = match kind {
        "literal endless char" => ("MISSING SINGLE QUOTE", "I thought I was parsing a character, but I got to the end of the line without seeing the closing single quote:", 0),
        "literal endless single" => ("ENDLESS STRING", "I got to the end of the line without seeing the closing double quote:", 0),
        "literal endless multi" => ("ENDLESS STRING", "I cannot find the end of this multi-line string:", 3),
        "literal unknown escape" => ("UNKNOWN ESCAPE", "Backslashes always start escaped characters, but I do not recognize this one:", 2),
        _ if char_width.is_some() => ("NEEDS DOUBLE QUOTES", "The following string uses single quotes:", char_width?),
        _ => return None,
    };
    let mut message = snippet_positions(source, (line,column), (line,column+width), preface)?;
    text(&mut message, "\n".into());
    match kind {
        "literal endless char" => text(&mut message, "Add a closing single quote here!".into()),
        "literal endless single" => {
            text(&mut message, "Strings look like ".into());
            message.push(color("\"this\"", "GREEN"));
            text(&mut message, " with double quotes on each end. Is the closing double\nquote missing in your code?\n\n".into());
            note(&mut message, "For a string that spans multiple lines, you can use the multi-line string syntax like this:");
            text(&mut message, "\n\n".into());
            message.push(color(MULTILINE, "yellow"));
            text(&mut message, String::new());
        }
        "literal endless multi" => {
            text(&mut message, "Add a \"\"\" somewhere after this to end the string.\n\n".into());
            note(&mut message, "Here is a valid multi-line string for reference:");
            text(&mut message, "\n\n".into());
            message.push(color(MULTILINE, "yellow"));
            text(&mut message, String::new());
        }
        "literal unknown escape" => {
            text(&mut message, "Valid escape characters include:\n\n".into());
            message.push(color("    \\n\n    \\r\n    \\t\n    \\\"\n    \\'\n    \\\\\n    \\u{003D}", "yellow"));
            text(&mut message, "\n\nDo you want one of those instead? Maybe you need \\\\ to escape a backslash?\n\n".into());
            note(&mut message, "The last style lets encode ANY character by its Unicode code point. That means \\u{0009} and \\t are the same. You can use that style for anything not covered by the other six escapes!");
        }
        _ => {
            text(&mut message, "Please switch to double quotes instead:\n\n    ".into());
            message.push(color("'this'", "yellow"));
            text(&mut message, " => ".into());
            message.push(color("\"this\"", "GREEN"));
            text(&mut message, "\n\n".into());
            note(&mut message, "Elm uses double quotes for strings like \"hello\", whereas it uses single quotes for individual characters like 'a' and 'ø'. This distinction helps with code like (String.any (\\c -> c == 'X') \"90210\") where you are inspecting individual characters.");
        }
    }
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":title,"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
    }]}]}))
}


fn unicode_report(source: &str, name: &str, path: &Path, line: usize, column: usize, details: &str) -> Option<Value> {
    let mut parts = details.split_whitespace();
    let kind = parts.next()?;
    let width = parts.next()?.parse::<usize>().ok()?;
    let digits = if kind == "length" { parts.next()?.parse::<usize>().ok()? } else { 0 };
    let preface = match kind {
        "format" => "I ran into an invalid Unicode escape:",
        "code" => "This is not a valid code point:",
        "length" if digits < 4 => "Every code point needs at least four digits:",
        "length" => "This code point has too many digits:",
        _ => return None,
    };
    let mut message = snippet_positions(source, (line,column), (line,column+width), preface)?;
    text(&mut message, "\n".into());
    match kind {
        "format" => {
            text(&mut message, "Here are some examples of valid Unicode escapes:\n\n".into());
            message.push(color("    \\u{0041}\n    \\u{03BB}\n    \\u{6728}\n    \\u{1F60A}", "yellow"));
            text(&mut message, format!("\n\n{}", reflow("Notice that the code point is always surrounded by curly braces. Maybe you are missing the opening or closing curly brace?")));
        }
        "code" => text(&mut message, "The valid code points are between 0 and 10FFFF inclusive.".into()),
        "length" if digits < 4 => {
            let value = parts.next()?.parse::<i64>().ok()?;
            let suggestion = format!("\\u{{{}{value:X}}}", "0".repeat(4-digits));
            text(&mut message, "Try ".into());
            message.push(color(&suggestion, "GREEN"));
            text(&mut message, " instead?".into());
        }
        _ => {
            let rendered = reflow("Valid code points are between \\u{0000} and \\u{10FFFF}, so try trimming any leading zeros until you have between four and six digits.");
            let (before, remaining) = rendered.split_once("\\u{0000}")?;
            let (middle, after) = remaining.split_once("\\u{10FFFF}")?;
            text(&mut message, before.into());
            message.push(color("\\u{0000}", "GREEN"));
            text(&mut message, middle.into());
            message.push(color("\\u{10FFFF}", "GREEN"));
            text(&mut message, after.into());
        }
    }
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":"BAD UNICODE ESCAPE","region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
    }]}]}))
}
