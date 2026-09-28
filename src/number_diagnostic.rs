//! Malformed numeric literal reports from Reporting.Error.Syntax.toNumberReport.
use crate::docs_diagnostic::{reflow, snippet_positions, text};
use serde_json::{Value, json};
use std::path::Path;

pub fn report(source: &str, name: &str, path: &Path, line: usize, column: usize, kind: &str) -> Option<Value> {
    let dot = kind.strip_prefix("number dot ");
    let (title, preface) = match kind {
        "leading zero" => ("LEADING ZEROS", "I do not accept numbers with leading zeros:"),
        "invalid hexadecimal literal" => ("WEIRD HEXIDECIMAL", "I thought I was reading a hexidecimal number until I got here:"),
        "invalid number suffix" | "missing exponent digits" => ("WEIRD NUMBER", "I thought I was reading a number, but I ran into some weird stuff here:"),
        _ if dot.is_some() => ("WEIRD NUMBER", "Numbers cannot end with a dot like this:"),
        _ => return None,
    };
    let mut message = snippet_positions(source, (line,column), (line,column), preface)?;
    text(&mut message, "\n".into());
    if let Some(integer) = dot {
        text(&mut message, "Switching to ".into());
        message.push(json!({"bold":false,"underline":false,"color":"GREEN","string":integer}));
        text(&mut message, " or ".into());
        message.push(json!({"bold":false,"underline":false,"color":"GREEN","string":format!("{integer}.0")}));
        text(&mut message, " will work though!".into());
    } else if kind == "leading zero" {
        text(&mut message, "Just delete the leading zeros and it should work!\n\n".into());
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
        let note = reflow("Note: Some languages let you to specify octal numbers by adding a leading zero. So in C, writing 0111 is the same as writing 73. Some people are used to that, but others probably want it to equal 111. Either path is going to surprise people from certain backgrounds, so Elm tries to avoid this whole situation.");
        text(&mut message, note[4..].to_string());
    } else if kind == "invalid hexadecimal literal" {
        text(&mut message, format!("{}\n\n    0x2B\n    0x002B\n    0x00ffb3", reflow("Valid hexidecimal digits include 0123456789abcdefABCDEF, so I can only recognize things like this:")));
    } else {
        text(&mut message, "I recognize numbers in the following formats:\n\n    42\n    3.14\n    6.022e23\n    0x002B\n\nSo is there a way to write it like one of those?".into());
    }
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":title,"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
    }]}]}))
}
