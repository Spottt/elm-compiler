//! Structured module-name syntax reports, following Reporting.Error.Syntax.
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
    if kind == "bad effect header" {
        let mut message = snippet_positions(
            source, (line, column), (line, column),
            "I cannot parse this module declaration:",
        )?;
        text(&mut message, format!("\n{}", reflow(
            "This type of module is reserved for the @elm organization. It is used to define certain effects, avoiding building them into the compiler."
        )));
        return Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":"BAD MODULE DECLARATION", "region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
        }]}]}));
    }
    if kind == "bad infix" {
        let mut message = snippet_positions(
            source,
            (line, column),
            (line, column),
            "Something went wrong in this infix operator declaration:",
        )?;
        text(
            &mut message,
            format!(
                "\n{}",
                reflow(
                    "This feature is used by the @elm organization to define the languages built-in operators."
                )
            ),
        );
        return Some(
            json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
                "title":"BAD INFIX", "region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
            }]}]}),
        );
    }
    if let Some(kind) = kind.strip_prefix("import ") {
        return import_report(source, name, path, line, column, kind);
    }
    if let Some(details) = kind.strip_prefix("exposing ") {
        let (start, problem) = details.split_once(' ')?;
        return exposing_report(
            source,
            name,
            path,
            start.parse().ok()?,
            (line, column),
            problem,
        );
    }
    let (port, unfinished) = match kind {
        "expected module name" => (false, false),
        "expected port module name" => (true, false),
        "unfinished module declaration" => (false, true),
        "unfinished port module declaration" => (true, true),
        _ => return None,
    };
    let mut message = snippet_positions(
        source,
        (line, column),
        (line, column),
        if unfinished {
            if port {
                "I am parsing an `port module` declaration, but I got stuck here:"
            } else {
                "I am parsing an `module` declaration, but I got stuck here:"
            }
        } else {
            "I was parsing an `module` declaration until I got stuck here:"
        },
    )?;
    text(
        &mut message,
        if unfinished {
            if port {
                "\nHere are some examples of valid `port module` declarations:\n\n"
            } else {
                "\nHere are some examples of valid `module` declarations:\n\n"
            }
        } else {
            "\nI was expecting to see the module name next, like in these examples:\n\n"
        }
        .into(),
    );
    let examples: &[(&str, &str)] = if port {
        &[
            ("WebSockets", "(send, listen, keepAlive)"),
            ("Maps", "(Location, goto)"),
        ]
    } else if unfinished {
        &[("Main", "(..)"), ("Dict", "(Dict, empty, get)")]
    } else {
        &[
            ("Dict", "(..)"),
            ("Maybe", "(..)"),
            ("Html.Attributes", "(..)"),
            ("Json.Decode", "(..)"),
        ]
    };
    let cyan = |word: &str| json!({"bold":false,"underline":false,"color":"CYAN","string":word});
    for (index, (example, exposing)) in examples.iter().enumerate() {
        text(
            &mut message,
            if index == 0 { "    " } else { "\n    " }.into(),
        );
        if port {
            message.push(cyan("port"));
            text(&mut message, " ".into());
        }
        message.push(cyan("module"));
        text(&mut message, format!(" {example} "));
        message.push(cyan("exposing"));
        text(&mut message, format!(" {exposing}"));
    }
    if unfinished {
        text(&mut message, "\n\n".into());
        if port {
            message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
            text(
                &mut message,
                ": Read <https://elm-lang.org/0.19.1/ports> for more help.".into(),
            );
        } else {
            text(
                &mut message,
                reflow(
                    "I generally recommend using an explicit exposing list. I can skip compiling a bunch of files when the public interface of a module stays the same, so exposing fewer values can help improve compile times!",
                ),
            );
        }
    } else {
        text(
            &mut message,
            format!(
                "\n\nNotice that the module names {}start with capital letters. That is required!",
                if port { "" } else { "all " }
            ),
        );
    }
    let title = if unfinished {
        if port {
            "UNFINISHED PORT MODULE DECLARATION"
        } else {
            "UNFINISHED MODULE DECLARATION"
        }
    } else {
        "EXPECTING MODULE NAME"
    };
    let point = json!({"line":line,"column":column});
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":title,"region":{"start":point,"end":point},"message":message
        }]}]}),
    )
}

fn import_report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    kind: &str,
) -> Option<Value> {
    let title = match kind {
        "name" => "EXPECTING IMPORT NAME",
        "alias" => "EXPECTING IMPORT ALIAS",
        "end" | "exposed-list" => "UNFINISHED IMPORT",
        _ => return None,
    };
    let mut message = snippet_positions(
        source,
        (line, column),
        (line, column),
        if kind == "end" {
            "I am partway through parsing an import, but I got stuck here:"
        } else {
            "I was parsing an `import` until I got stuck here:"
        },
    )?;
    type ImportExample<'a> = (&'a str, Option<&'a str>, Option<&'a str>);
    let (intro, examples, advice): (&str, &[ImportExample<'_>], &str) = match kind {
        "name" => (
            "I was expecting to see a module name next, like in these examples:",
            &[
                ("Dict", None, None),
                ("Maybe", None, None),
                ("Html.Attributes", Some("A"), None),
                ("Json.Decode", None, Some("(..)")),
            ],
            "Notice that the module names all start with capital letters. That is required!",
        ),
        "alias" => (
            "I was expecting to see an alias next, like in these examples:",
            &[
                ("Html.Attributes", Some("Attr"), None),
                ("WebGL.Texture", Some("Texture"), None),
                ("Json.Decode", Some("D"), None),
            ],
            "Notice that the alias always starts with a capital letter. That is required!",
        ),
        "exposed-list" => (
            "I was expecting to see the list of exposed values next. For example, here are two ways to expose values from the `Html` module:",
            &[
                ("Html", None, Some("(..)")),
                ("Html", None, Some("(Html, div, text)")),
            ],
            "I generally recommend the second style. It is more explicit, making it much easier to figure out where values are coming from in large projects!",
        ),
        _ => (
            "Here are some examples of valid `import` declarations:",
            &[
                ("Html", None, None),
                ("Html", Some("H"), None),
                ("Html", Some("H"), Some("(..)")),
                ("Html", None, Some("(Html, div, text)")),
            ],
            "You are probably trying to import a different module, but try to make it look like one of these examples!",
        ),
    };
    text(&mut message, format!("\n{}\n\n", reflow(intro)));
    let cyan = |word: &str| json!({"bold":false,"underline":false,"color":"CYAN","string":word});
    for (index, (module, alias, exposing)) in examples.iter().enumerate() {
        text(
            &mut message,
            if index == 0 { "    " } else { "\n    " }.into(),
        );
        message.push(cyan("import"));
        text(&mut message, format!(" {module}"));
        if let Some(alias) = alias {
            text(&mut message, " ".into());
            message.push(cyan("as"));
            text(&mut message, format!(" {alias}"));
        }
        if let Some(exposing) = exposing {
            text(&mut message, " ".into());
            message.push(cyan("exposing"));
            text(&mut message, format!(" {exposing}"));
        }
    }
    text(&mut message, format!("\n\n{}", reflow(advice)));
    if kind != "exposed-list" {
        text(
            &mut message,
            "\n\nRead <https://elm-lang.org/0.19.1/imports> to learn more.".into(),
        );
    }
    let point = json!({"line":line,"column":column});
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":title,"region":{"start":point,"end":point},"message":message
        }]}]}),
    )
}

fn exposing_report(
    source: &str,
    name: &str,
    path: &Path,
    start_line: usize,
    (line, column): (usize, usize),
    kind: &str,
) -> Option<Value> {
    if start_line == 0 || start_line > line {
        return None;
    }
    // Reporting.Render.Code inspects the displayed column in the raw line,
    // rather than the token stream (notably different after layout CRs).
    let value = if kind == "value" {
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
        if crate::parser::reserved(&word) {
            Some(("value-keyword", word))
        } else {
            let op: String = tail
                .chars()
                .take_while(|c| "+-/*=.<>:&|^?%!".contains(*c))
                .collect();
            if op.is_empty() {
                None
            } else {
                Some(("value-symbol", op))
            }
        }
    } else {
        None
    };
    let kind = value.as_ref().map_or(kind, |(kind, _)| *kind);
    let highlight_width = value.as_ref().map_or(0, |(_, text)| text.chars().count());
    let (title, preface) = match kind {
        "start" => (
            "PROBLEM IN EXPOSING",
            "I want to parse exposed values, but I am getting stuck here:",
        ),
        "value" => (
            "PROBLEM IN EXPOSING",
            "I got stuck while parsing these exposed values:",
        ),
        "value-keyword" => ("RESERVED WORD", "I got stuck on this reserved word:"),
        "value-symbol" => ("UNEXPECTED SYMBOL", "I got stuck on this symbol:"),
        "end" | "indent-end" | "indent-value" => (
            "UNFINISHED EXPOSING",
            "I was partway through parsing exposed values, but I got stuck here:",
        ),
        "privacy" => (
            "PROBLEM EXPOSING CUSTOM TYPE VARIANTS",
            "It looks like you are trying to expose the variants of a custom type:",
        ),
        "operator" => (
            "PROBLEM IN EXPOSING",
            "I just saw an open parenthesis, so I was expecting an operator next:",
        ),
        "operator-close" => (
            "PROBLEM IN EXPOSING",
            "It looks like you are exposing an operator, but I got stuck here:",
        ),
        "reserved-." | "reserved-|" | "reserved-->" | "reserved-=" | "reserved-:" => {
            ("RESERVED SYMBOL", "I cannot expose this as an operator:")
        }
        _ => return None,
    };
    let mut message = vec![];
    text(&mut message, format!("{preface}\n\n"));
    let width = line.to_string().len();
    for (index, source_line) in source
        .split('\n')
        .enumerate()
        .skip(start_line - 1)
        .take(line - start_line + 1)
    {
        text(
            &mut message,
            format!("{:>width$}| {source_line}\n", index + 1),
        );
    }
    text(&mut message, " ".repeat(column + width + 1));
    message.push(json!({"bold":false,"underline":false,"color":"RED","string":"^".repeat(highlight_width.max(1))}));
    match kind {
        "start" => {
            text(&mut message, "\n".into());
            styled_reflow(
                &mut message,
                "Exposed values are always surrounded by parentheses. So try adding a ( here?",
                &[("(", "GREEN")],
            )?;
            text(&mut message, "\n\n".into());
            message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
            text(
                &mut message,
                ": Here are some valid examples of `exposing` for reference:\n\n".into(),
            );
            exposing_examples(
                &mut message,
                &[("Html", "(..)"), ("Html", "(Html, div, text)")],
            );
            text(
                &mut message,
                format!(
                    "\n\n{}",
                    reflow(
                        "If you are getting tripped up, you can just expose everything for now. It should get easier to make an explicit exposing list as you see more examples in the wild."
                    )
                ),
            );
        }
        "value" => {
            text(
                &mut message,
                format!(
                    "\n{}\n\n",
                    reflow(
                        "I do not have an exact recommendation, so here are some valid examples of `exposing` for reference:"
                    )
                ),
            );
            exposing_examples(
                &mut message,
                &[
                    ("Html", "(..)"),
                    ("Basics", "(Int, Float, Bool(..), (+), not, sqrt)"),
                ],
            );
            text(
                &mut message,
                format!(
                    "\n\n{}",
                    reflow(
                        "These examples show how to expose types, variants, operators, and functions. Everything should be some permutation of these examples, just with different names."
                    )
                ),
            );
        }
        "value-keyword" => {
            let word = &value.as_ref()?.1;
            text(
                &mut message,
                format!(
                    "\n{}",
                    reflow(&format!(
                        "It looks like you are trying to expose `{word}` but that is a reserved word. Is there a typo?"
                    ))
                ),
            );
        }
        "value-symbol" => {
            let op = &value.as_ref()?.1;
            text(&mut message, "\nIf you are trying to expose an operator, add parentheses around it like this:\n\n    ".into());
            message.push(json!({"bold":false,"underline":false,"color":"yellow","string":op}));
            text(&mut message, " -> ".into());
            message.push(
                json!({"bold":false,"underline":false,"color":"GREEN","string":format!("({op})")}),
            );
            // Elm's JSON renderer retains the empty plain chunk after the
            // final styled suggestion, even though terminals display nothing.
            message.push(json!(""));
        }
        "operator" => {
            text(&mut message, "\n".into());
            styled_reflow(
                &mut message,
                "It is possible to expose operators, so I was expecting to see something like (+) or (|=) or (||) after I saw that open parenthesis.",
                &[("(+)", "yellow"), ("(|=)", "yellow"), ("(||)", "yellow")],
            )?;
        }
        "operator-close" => {
            text(&mut message, "\n".into());
            styled_reflow(
                &mut message,
                "I was expecting to see the closing parenthesis immediately after the operator. Try adding a ) right here?",
                &[(")", "GREEN")],
            )?;
        }
        "reserved-." | "reserved-->" => text(
            &mut message,
            "\nTry getting rid of this entry? Maybe I can give you a better hint after that?"
                .into(),
        ),
        "reserved-|" | "reserved-=" | "reserved-:" => {
            let example = match kind {
                "reserved-|" => "(||)",
                "reserved-=" => "(==)",
                _ => "(::)",
            };
            text(&mut message, "\nMaybe you want ".into());
            message.push(json!({"bold":false,"underline":false,"color":"yellow","string":example}));
            text(&mut message, " instead?".into());
        }
        "privacy" => {
            text(&mut message, "\n".into());
            let advice = reflow(
                "You need to write something like Status(..) or Entity(..) though. It is all or nothing, otherwise `case` expressions could miss a variant and crash!",
            );
            let mut rest = advice.as_str();
            for example in ["Status(..)", "Entity(..)"] {
                let (before, after) = rest.split_once(example)?;
                text(&mut message, before.into());
                message.push(
                    json!({"bold":false,"underline":false,"color":"yellow","string":example}),
                );
                rest = after;
            }
            text(&mut message, format!("{rest}\n\n"));
            message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
            let note = reflow(
                "Note: It is often best to keep the variants hidden! If someone pattern matches on the variants, it is a MAJOR change if any new variants are added. Suddenly their `case` expressions do not cover all variants! So if you do not need people to pattern match, keep the variants hidden and expose functions to construct values of this type. This way you can add new variants as a MINOR change!",
            );
            text(&mut message, note[4..].into());
        }
        "end" => text(
            &mut message,
            "\nMaybe there is a comma missing before this?".into(),
        ),
        "indent-value" => text(
            &mut message,
            "\nI was expecting another value to expose.".into(),
        ),
        _ => {
            text(
                &mut message,
                "\nI was expecting a closing parenthesis. Try adding a ".into(),
            );
            message.push(json!({"bold":false,"underline":false,"color":"GREEN","string":")"}));
            text(&mut message, " right here?\n\n".into());
            let note = reflow(
                "Note: I can get confused when there is not enough indentation, so if you already have a closing parenthesis, it probably just needs some spaces in front of it.",
            );
            message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
            text(&mut message, note[4..].into());
        }
    }
    let position = json!({"line":line,"column":column});
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":title,"region":{"start":position,"end":{"line":line,"column":column + highlight_width}},"message":message
        }]}]}),
    )
}

fn exposing_examples(message: &mut Vec<Value>, examples: &[(&str, &str)]) {
    for (index, (module, listing)) in examples.iter().enumerate() {
        text(message, if index == 0 { "    " } else { "\n    " }.into());
        message.push(json!({"bold":false,"underline":false,"color":"CYAN","string":"import"}));
        text(message, format!(" {module} "));
        message.push(json!({"bold":false,"underline":false,"color":"CYAN","string":"exposing"}));
        text(message, format!(" {listing}"));
    }
}

fn styled_reflow(message: &mut Vec<Value>, prose: &str, styles: &[(&str, &str)]) -> Option<()> {
    let wrapped = reflow(prose);
    let mut rest = wrapped.as_str();
    for (word, color) in styles {
        let (before, after) = rest.split_once(word)?;
        text(message, before.into());
        message.push(json!({"bold":false,"underline":false,"color":color,"string":word}));
        rest = after;
    }
    text(message, rest.into());
    Some(())
}

/// A file found through a configured source directory needs a declared name.
pub fn missing_name(name: &str, path: &Path) -> Value {
    let mut message = vec![json!(
        "I need the module name to be declared at the top of this file, like this:\n\n    "
    )];
    let cyan = |word: &str| json!({"bold":false,"underline":false,"color":"CYAN","string":word});
    message.push(cyan("module"));
    text(&mut message, format!(" {name} "));
    message.push(cyan("exposing"));
    text(
        &mut message,
        " (..)\n\nTry adding that as the first line of your file!\n\n".into(),
    );
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
    let note = reflow(
        "Note: It is best to replace (..) with an explicit list of types and functions you want to expose. When you know a value is only used within this module, you can refactor without worrying about uses elsewhere. Limiting exposed values can also speed up compilation because I can skip a bunch of work if I see that the exposed API has not changed.",
    );
    text(&mut message, note[4..].to_owned());
    let point = json!({"line":1,"column":1});
    json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":"MODULE NAME MISSING","region":{"start":point,"end":point},"message":message
    }]}]})
}
