//! Unknown-name diagnostics retain the resolver's actual visible candidates.
use crate::docs_diagnostic::{reflow, snippet_positions, text};
use serde_json::{Value, json};
use std::path::Path;
const MARKER: &str = "\nELM_NAME_CANDIDATES:";

pub fn unknown(name: &str, thing: &str, known_prefix: bool, candidates: Vec<String>) -> String {
    format!(
        "unknown name {name}{MARKER}{}",
        json!({"name":name,"thing":thing,"known_prefix":known_prefix,"candidates":candidates})
    )
}
pub fn ambiguous(name: &str, thing: &str, homes: Vec<String>, choices: &[String]) -> String {
    format!(
        "ambiguous name {name}: defined by {}. Use distinct import aliases or change the exposed names{MARKER}{}",
        choices.join(" and "),
        json!({"kind":"ambiguous","name":name,"thing":thing,"homes":homes})
    )
}
pub fn unknown_operator(op: &str, candidates: Vec<String>) -> String {
    format!(
        "unknown operator {op}{MARKER}{}",
        json!({"kind":"operator","name":op,"candidates":candidates})
    )
}
pub fn associativity(first: &str, second: &str, reason: String) -> String {
    format!(
        "{reason}{MARKER}{}",
        json!({"kind":"associativity","first":first,"second":second})
    )
}
pub fn arity(name: &str, thing: &str, expected: usize, actual: usize, reason: String) -> String {
    let name = name.rsplit('.').next().unwrap_or(name);
    format!(
        "{reason}{MARKER}{}",
        json!({"kind":"arity","name":name,"thing":thing,"expected":expected,"actual":actual})
    )
}
pub fn record_pattern(name: &str) -> String {
    let short = name.rsplit('.').next().unwrap_or(name);
    format!(
        "record alias {name} cannot be used as a pattern constructor{MARKER}{}",
        json!({"kind":"record_pattern","name":short})
    )
}
pub fn type_variables(reason: String, details: Value) -> String {
    format!("{reason}{MARKER}{details}")
}
pub fn shadowing(source: &str, name: &str, first: &str) -> String {
    let reason = format!("shadowing local name {name}");
    let Some(offset) = (first.as_ptr() as usize).checked_sub(source.as_ptr() as usize) else {
        return reason;
    };
    if source.get(offset..offset + first.len()) != Some(first) {
        return reason;
    }
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    format!(
        "{reason}{MARKER}{}",
        json!({"kind":"shadowing","name":name,"first":[line,column,column+first.chars().count()]})
    )
}
pub fn annotation(data: Value) -> String {
    format!("annotation type mismatch{MARKER}{data}")
}
pub fn local_cycle(name: &str, others: &[&str]) -> String {
    format!(
        "cyclic local value: recursive let groups must contain only functions{MARKER}{}",
        json!({"kind":"cycle","local":true,"name":name,"others":others})
    )
}
pub fn cycle(name: &str, others: &[&str]) -> String {
    format!(
        "cyclic global value: immediate dependencies cannot be recursive{MARKER}{}",
        json!({"kind":"cycle","name":name,"others":others})
    )
}
pub fn duplicate_pattern(
    source: &str,
    name: &str,
    later: &str,
    context: &str,
    function: Option<&str>,
) -> String {
    let reason = format!("duplicate local name {name}");
    let Some(offset) = (later.as_ptr() as usize).checked_sub(source.as_ptr() as usize) else {
        return reason;
    };
    let Some(prefix) = source.get(..offset) else {
        return reason;
    };
    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    format!(
        "{reason}{MARKER}{}",
        json!({"kind":"duplicate_pattern","name":name,"context":context,"function":function,"first":[line,column,column+later.chars().count()]})
    )
}
pub fn has_details(message: &str) -> bool {
    message.contains(MARKER)
}

pub(crate) fn distance(a: &str, b: &str) -> usize {
    let a: Vec<_> = a
        .chars()
        .map(|c| c.to_lowercase().next().unwrap())
        .collect();
    let b: Vec<_> = b
        .chars()
        .map(|c| c.to_lowercase().next().unwrap())
        .collect();
    let mut rows = vec![vec![0; b.len() + 1]; a.len() + 1];
    for (i, row) in rows.iter_mut().enumerate() {
        row[0] = i;
    }
    for (i, value) in rows[0].iter_mut().enumerate() {
        *value = i;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            rows[i][j] = (rows[i - 1][j] + 1)
                .min(rows[i][j - 1] + 1)
                .min(rows[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]));
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                rows[i][j] = rows[i][j].min(rows[i - 2][j - 2] + 1);
            }
        }
    }
    rows[a.len()][b.len()]
}
pub fn report(
    source: &str,
    module: &str,
    path: &Path,
    start: (usize, usize),
    end: (usize, usize),
    detail: &str,
) -> Option<Value> {
    let (_, encoded) = detail.split_once(MARKER)?;
    let data: Value = serde_json::from_str(encoded).ok()?;
    if data["kind"] == "record_access" {
        return crate::record_access_diagnostic::report(source, module, path, start, end, &data);
    }
    if data["kind"] == "call_arity" {
        return crate::annotation_diagnostic::arity_report(source, module, path, start, end, &data);
    }
    if data["kind"] == "annotation_mismatch" {
        return crate::annotation_diagnostic::report(source, module, path, start, end, &data);
    }
    if data["kind"] == "duplicate_pattern" {
        return duplicate_pattern_report(source, module, path, start, end, &data);
    }
    if data["kind"] == "cycle" {
        return cycle_report(source, module, path, start, end, &data);
    }
    if data["kind"] == "shadowing" {
        return shadowing_report(source, module, path, start, end, &data);
    }
    if data["kind"] == "type_variables" {
        return crate::type_variable_diagnostic::report(source, module, path, start, end, &data);
    }
    if data["kind"] == "record_pattern" {
        let name = data["name"].as_str()?;
        let mut message = snippet_positions(
            source,
            start,
            end,
            &reflow(&format!(
                "You can construct records by using `{name}` as a function, but it is not available in pattern matching like this:"
            )),
        )?;
        text(
            &mut message,
            "\nI recommend matching the record as a variable and unpacking it later.".into(),
        );
        return Some(
            json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":"BAD PATTERN","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
        );
    }
    if data["kind"] == "arity" {
        let name = data["name"].as_str()?;
        let thing = data["thing"].as_str()?;
        let expected = data["expected"].as_u64()?;
        let actual = data["actual"].as_u64()?;
        let title = if actual < expected {
            "TOO FEW ARGS"
        } else {
            "TOO MANY ARGS"
        };
        let mut message = snippet_positions(
            source,
            start,
            end,
            &reflow(&format!(
                "The `{name}` {thing} needs {expected} argument{}, but I see {actual} instead:",
                if expected == 1 { "" } else { "s" }
            )),
        )?;
        let hint = if actual < expected {
            "What is missing? Are some parentheses misplaced?"
        } else if actual - expected == 1 {
            "Which is the extra one? Maybe some parentheses are missing?"
        } else {
            "Which are the extra ones? Maybe some parentheses are missing?"
        };
        text(&mut message, format!("\n{hint}"));
        return Some(
            json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":title,"region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
        );
    }
    if data["kind"] == "associativity" {
        let first = data["first"].as_str()?;
        let second = data["second"].as_str()?;
        let mut message = snippet_positions(
            source,
            start,
            end,
            &format!("You cannot mix ({first}) and ({second}) without parentheses."),
        )?;
        text(
            &mut message,
            "\nI do not know how to group these expressions. Add parentheses for me!".into(),
        );
        return Some(
            json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":"INFIX PROBLEM","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
        );
    }
    if data["kind"] == "operator" {
        return operator_report(source, module, path, start, end, &data);
    }
    if data["kind"] == "ambiguous" {
        return ambiguous_report(source, module, path, start, end, &data);
    }
    let name = data["name"].as_str()?;
    let thing = data["thing"].as_str()?;
    let mut candidates: Vec<_> = data["candidates"]
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect::<Option<_>>()?;
    candidates.sort_by_key(|candidate| distance(name, candidate));
    candidates.truncate(4);
    let mut message = snippet_positions(
        source,
        start,
        end,
        &format!("I cannot find a `{name}` {thing}:"),
    )?;
    let details = match name.rsplit_once('.') {
        None => {
            if candidates.is_empty() {
                "Is there an `import` or `exposing` missing up top?".into()
            } else {
                "These names seem close though:".into()
            }
        }
        Some((prefix, name)) => {
            if data["known_prefix"].as_bool()? {
                format!(
                    "The `{prefix}` module does not expose a `{name}` {thing}.{}",
                    if candidates.is_empty() {
                        ""
                    } else {
                        " These names seem close though:"
                    }
                )
            } else if candidates.is_empty() {
                format!("I cannot find a `{prefix}` module. Is there an `import` for it?")
            } else {
                format!("I cannot find a `{prefix}` import. These names seem close though:")
            }
        }
    };
    text(&mut message, format!("\n{}\n\n", reflow(&details)));
    for candidate in candidates {
        text(&mut message, "    ".into());
        message.push(json!({"bold":false,"underline":false,"color":"yellow","string":candidate}));
        text(&mut message, "\n".into());
    }
    if !data["candidates"].as_array()?.is_empty() {
        text(&mut message, "\n".into());
    }
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Hint"}));
    text(&mut message,": Read <https://elm-lang.org/0.19.1/imports> to see how `import`\ndeclarations work in Elm.".into());
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":"NAMING ERROR","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    )
}

fn ambiguous_report(
    source: &str,
    module: &str,
    path: &Path,
    start: (usize, usize),
    end: (usize, usize),
    data: &Value,
) -> Option<Value> {
    let name = data["name"].as_str()?;
    let thing = data["thing"].as_str()?;
    let homes: Vec<_> = data["homes"]
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect::<Option<_>>()?;
    let qualified = name.rsplit_once('.');
    let mut message = snippet_positions(
        source,
        start,
        end,
        &format!(
            "This usage of `{name}` is ambiguous{}",
            if qualified.is_some() { "." } else { ":" }
        ),
    )?;
    let details = if qualified.is_some() {
        format!(
            "It could refer to a {thing} from {} of these imports:",
            if homes.len() == 2 { "either" } else { "any" }
        )
    } else {
        format!(
            "This name is exposed by {} of your imports, so I am not sure which one to use:",
            homes.len()
        )
    };
    text(&mut message, format!("\n{}\n\n", reflow(&details)));
    for home in homes {
        text(&mut message, "    ".into());
        if let Some((prefix, _)) = qualified {
            message.push(json!({"bold":false,"underline":false,"color":"CYAN","string":"import"}));
            text(&mut message, format!(" {home}"));
            if home != prefix {
                text(&mut message, " ".into());
                message.push(json!({"bold":false,"underline":false,"color":"CYAN","string":"as"}));
                text(&mut message, format!(" {prefix}"));
            }
        } else {
            message.push(json!({"bold":false,"underline":false,"color":"yellow","string":format!("{home}.{name}")}));
        }
        text(&mut message, "\n".into());
    }
    text(&mut message, "\n".into());
    if qualified.is_some() {
        text(&mut message,"Read <https://elm-lang.org/0.19.1/imports> to learn how to clarify which one you\nwant.".into());
    } else {
        text(
            &mut message,
            format!(
                "{}\n\n",
                reflow(
                    "I recommend using qualified names for imported values. I also recommend having at most one `exposing (..)` per file to make name clashes like this less common in the long run."
                )
            ),
        );
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
        text(&mut message,": Check out <https://elm-lang.org/0.19.1/imports> for more info on the\nimport syntax.".into());
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":"AMBIGUOUS NAME","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    )
}

fn operator_report(
    source: &str,
    module: &str,
    path: &Path,
    start: (usize, usize),
    end: (usize, usize),
    data: &Value,
) -> Option<Value> {
    let op = data["name"].as_str()?;
    let heading = match op {
        "===" => "Elm does not have a (===) operator like JavaScript.".into(),
        "!=" | "!==" => "Elm uses a different name for the “not equal” operator:".into(),
        "**" => "I do not recognize the (**) operator:".into(),
        "%" => "Elm does not use (%) as the remainder operator:".into(),
        _ => format!("I do not recognize the ({op}) operator."),
    };
    let mut message = snippet_positions(source, start, end, &heading)?;
    text(&mut message, "\n".into());
    match op {
        "===" => text(&mut message, "Switch to (==) instead.".into()),
        "!=" | "!==" => {
            text(&mut message, "Switch to (/=) instead.\n\n".into());
            message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
            let note = reflow(&format!(
                "Note: Our (/=) operator is supposed to look like a real “not equal” sign (≠). I hope that history will remember ({op}) as a weird and temporary choice."
            ));
            text(&mut message, note.strip_prefix("Note").unwrap().into());
        }
        "**" => text(
            &mut message,
            reflow("Switch to (^) for exponentiation. Or switch to (*) for multiplication."),
        ),
        "%" => {
            text(
                &mut message,
                format!(
                    "{}\n\n{}\n\n{}",
                    reflow(
                        "If you want the behavior of (%) like in JavaScript, switch to: <https://package.elm-lang.org/packages/elm/core/latest/Basics#remainderBy>"
                    ),
                    reflow(
                        "If you want modular arithmetic like in math, switch to: <https://package.elm-lang.org/packages/elm/core/latest/Basics#modBy>"
                    ),
                    reflow("The difference is how things work when negative numbers are involved.")
                ),
            );
        }
        _ => {
            let mut candidates: Vec<_> = data["candidates"]
                .as_array()?
                .iter()
                .map(Value::as_str)
                .collect::<Option<_>>()?;
            candidates.sort_by(|left, right| crate::edition::compare_names(left, right));
            candidates.sort_by_key(|candidate| distance(op, candidate));
            candidates.truncate(2);
            let formatted: Vec<_> = candidates.iter().map(|op| format!("({op})")).collect();
            let mut body = "Is there an `import` and `exposing` entry for it?".to_owned();
            if !formatted.is_empty() {
                body.push_str(&format!(
                    " Maybe you want {} instead?",
                    formatted.join(" or ")
                ));
            }
            let body = reflow(&body);
            let mut rest = body.as_str();
            // Preserve styled suggestions after wrapping the full paragraph.
            for alternative in formatted {
                let (before, after) = rest.split_once(&alternative)?;
                text(&mut message, before.into());
                message.push(
                    json!({"bold":false,"underline":false,"color":"GREEN","string":alternative}),
                );
                rest = after;
            }
            text(&mut message, rest.into());
        }
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":"UNKNOWN OPERATOR","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    )
}

fn shadowing_report(
    source: &str,
    module: &str,
    path: &Path,
    start: (usize, usize),
    end: (usize, usize),
    data: &Value,
) -> Option<Value> {
    let name = data["name"].as_str()?;
    let line = data["first"][0].as_u64()? as usize;
    let column = data["first"][1].as_u64()? as usize;
    let last = data["first"][2].as_u64()? as usize;
    // Since 0.19.2 the official duplicate detection hands the two occurrences over in the other order.
    let (line, column, last, start, end) = if crate::edition::swapped_duplicate_regions() && start.0 == end.0 {
        (start.0, start.1, end.1, (line, column), (line, last))
    } else {
        (line, column, last, start, end)
    };
    let mut message = if line == start.0 && start.0 == end.0 {
        let source_line = source.split('\n').nth(line - 1)?;
        let mut m = vec![];
        text(
            &mut m,
            format!(
                "These variables cannot have the same name:\n\n{line}| {source_line}\n{}",
                " ".repeat(column + line.to_string().len() + 1)
            ),
        );
        m.push(
            json!({"bold":false,"underline":false,"color":"RED","string":"^".repeat(last-column)}),
        );
        text(
            &mut m,
            " ".repeat((start.1 as u16).wrapping_sub(last as u16) as usize),
        );
        m.push(json!({"bold":false,"underline":false,"color":"RED","string":"^".repeat(end.1-start.1)}));
        m
    } else {
        let mut m = snippet_positions(
            source,
            (line, column),
            (line, last),
            &format!("The name `{name}` is first defined here:"),
        )?;
        text(&mut m, "\n".into());
        for chunk in snippet_positions(
            source,
            start,
            end,
            "But then it is defined AGAIN over here:",
        )? {
            if let Some(s) = chunk.as_str() {
                text(&mut m, s.into());
            } else {
                m.push(chunk);
            }
        }
        m
    };
    text(
        &mut message,
        "\nThink of a more helpful name for one of them and you should be all set!\n\n".into(),
    );
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
    text(&mut message,": Linters advise against shadowing, so Elm makes “best practices” the\ndefault. Read <https://elm-lang.org/0.19.1/shadowing> for more details on this\nchoice.".into());
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":"SHADOWING","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    )
}

fn cycle_report(
    source: &str,
    module: &str,
    path: &Path,
    start: (usize, usize),
    end: (usize, usize),
    data: &Value,
) -> Option<Value> {
    let name = data["name"].as_str()?;
    let others = data["others"].as_array()?;
    let local = data["local"].as_bool().unwrap_or(false);
    let mut message = snippet_positions(
        source,
        start,
        end,
        &if others.is_empty() {
            format!(
                "The `{name}` value is defined directly in terms of itself, causing an infinite loop."
            )
        } else if local {
            "I do not allow cyclic values in `let` expressions.".into()
        } else {
            format!("The `{name}` definition is causing a very tricky infinite loop.")
        },
    )?;
    text(&mut message, "\n".into());
    if others.is_empty() {
        for (index,(question,details)) in [
            (if crate::edition::corrected_wording() { "Are you trying to mutate a variable?" } else { "Are you are trying to mutate a variable?" },format!("Elm does not have mutation, so when I see {name} defined in terms of {name}, I treat it as a recursive definition. Try giving the new value a new name!")),
            ("Maybe you DO want a recursive value?",format!("To define {name} we need to know what {name} is, so let’s expand it. Wait, but now we need to know what {name} is, so let’s expand it... This will keep going infinitely!")),
        ].into_iter().enumerate() {
            if index>0 {text(&mut message,"\n\n".into());}
            for (index,word) in question.split(' ').enumerate() {
                if index>0 {text(&mut message," ".into());}
                message.push(json!({"bold":false,"underline":false,"color":"yellow","string":word}));
            }
            text(&mut message,reflow(&format!("{question} {details}"))[question.len()..].into());
        }
    } else {
        text(
            &mut message,
            format!(
                "{}\n\n    ┌─────┐\n",
                reflow(&format!(
                    "The `{name}` value depends on itself through the following chain of definitions:"
                ))
            ),
        );
        for (index, name) in std::iter::once(Some(name))
            .chain(others.iter().map(Value::as_str))
            .enumerate()
        {
            if index > 0 {
                text(&mut message, "    │     ↓\n".into());
            }
            text(&mut message, "    │    ".into());
            message.push(json!({"bold":false,"underline":false,"color":"yellow","string":name?}));
            text(&mut message, "\n".into());
        }
        text(&mut message, "    └─────┘".into());
    }
    text(&mut message, "\n\n".into());
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Hint"}));
    text(&mut message,if others.is_empty() {": The root problem is often a typo in some variable name, but I recommend\nreading <https://elm-lang.org/0.19.1/bad-recursion> for more detailed advice,\nespecially if you actually do need a recursive value."} else {": The root problem is often a typo in some variable name, but I recommend\nreading <https://elm-lang.org/0.19.1/bad-recursion> for more detailed advice,\nespecially if you actually do want mutually recursive values."}.into());
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":if local { "CYCLIC VALUE" } else { "CYCLIC DEFINITION" },"region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    )
}

fn duplicate_pattern_report(
    source: &str,
    module: &str,
    path: &Path,
    start: (usize, usize),
    end: (usize, usize),
    data: &Value,
) -> Option<Value> {
    let name = data["name"].as_str()?;
    let heading = match data["context"].as_str()? {
        "function" => format!(
            "The `{}` function has multiple `{name}` arguments.",
            data["function"].as_str()?
        ),
        "lambda" => format!("This anonymous function has multiple `{name}` arguments."),
        "case" => format!("This `case` pattern has multiple `{name}` variables."),
        "let" => format!("This `let` expression defines `{name}` more than once!"),
        _ => format!("This pattern contains multiple `{name}` variables."),
    };
    let line = data["first"][0].as_u64()? as usize;
    let column = data["first"][1].as_u64()? as usize;
    let last = data["first"][2].as_u64()? as usize;
    // Since 0.19.2 the official duplicate detection hands the two occurrences over in the other order.
    let (line, column, last, start, end) = if crate::edition::swapped_duplicate_regions() && start.0 == end.0 {
        (start.0, start.1, end.1, (line, column), (line, last))
    } else {
        (line, column, last, start, end)
    };
    let mut message = if line == start.0 && start.0 == end.0 {
        let source_line = source.split('\n').nth(line - 1)?;
        let mut m = vec![];
        text(
            &mut m,
            format!(
                "{}\n\n{line}| {source_line}\n{}",
                reflow(&heading),
                " ".repeat(column + line.to_string().len() + 1)
            ),
        );
        m.push(
            json!({"bold":false,"underline":false,"color":"RED","string":"^".repeat(last-column)}),
        );
        // Reporting.Render.Code uses Word16 subtraction, including wraparound
        // when the later occurrence is printed before the earlier occurrence.
        text(
            &mut m,
            " ".repeat((start.1 as u16).wrapping_sub(last as u16) as usize),
        );
        m.push(json!({"bold":false,"underline":false,"color":"RED","string":"^".repeat(end.1-start.1)}));
        m
    } else {
        let mut m = snippet_positions(
            source,
            (line, column),
            (line, last),
            &format!("{heading} One here:"),
        )?;
        text(&mut m, "\n".into());
        for chunk in snippet_positions(source, start, end, "And another one here:")? {
            if let Some(s) = chunk.as_str() {
                text(&mut m, s.into());
            } else {
                m.push(chunk);
            }
        }
        m
    };
    text(
        &mut message,
        "\nHow can I know which one you want? Rename one of them!".into(),
    );
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":"NAME CLASH","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    )
}
