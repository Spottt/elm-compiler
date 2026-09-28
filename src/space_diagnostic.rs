//! Space/comment reports following Reporting.Error.Syntax.toSpaceReport.
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
    let comment = match kind {
        "tabs are not allowed in Elm layout" | "tabs are not allowed in block comments" => false,
        "unterminated block comment" => true,
        _ => return None,
    };
    let width = if comment { 2 } else { 0 };
    let mut message = snippet_positions(
        source,
        (line, column),
        (line, column + width),
        if comment {
            "I cannot find the end of this multi-line comment:"
        } else {
            "I ran into a tab, but tabs are not allowed in Elm files."
        },
    )?;
    if comment {
        text(
            &mut message,
            "\nAdd a -} somewhere after this to end the comment.\n\n".into(),
        );
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Hint"}));
        let hint = reflow(
            "Hint: Multi-line comments can be nested in Elm, so {- {- -} -} is a comment that happens to contain another comment. Like parentheses and curly braces, the start and end markers must always be balanced. Maybe that is the problem?",
        );
        text(&mut message, hint[4..].into());
    } else {
        text(&mut message, "\nReplace the tab with spaces.".into());
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":if comment {"ENDLESS COMMENT"} else {"NO TABS"},
            "region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column + width}},"message":message
        }]}]}),
    )
}
