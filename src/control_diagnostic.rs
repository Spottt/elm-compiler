//! Control-expression syntax reports from Reporting.Error.Syntax.
use crate::docs_diagnostic::{reflow, text};
use serde_json::{Value, json};
use std::path::Path;
fn color(value: &str, color: &str) -> Value {
    json!({"bold":false,"underline":false,"color":color,"string":value})
}
pub fn report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    kind: &str,
) -> Option<Value> {
    if kind.starts_with("\\ ") { return lambda_report(source, name, path, line, column, kind); }
    if kind.starts_with("let ") { return let_report(source, name, path, line, column, kind); }
    if kind.starts_with("local-definition ") { return local_definition_report(source, name, path, line, column, kind); }
    if kind.starts_with("local-equals ") { return local_equals_report(source, name, path, line, column, kind); }
    if kind.starts_with("local-destruct ") { return local_destruct_report(source, name, path, line, column, kind); }
    if kind.starts_with("local-annotation ") { return local_annotation_report(source, name, path, line, column, kind); }
    if kind.starts_with("expression-start ") { return expression_start_report(source, name, path, line, column, kind); }
    if kind.starts_with("{ ") { return record_report(source, name, path, line, column, kind); }
    if kind.starts_with("[ ") { return list_report(source, name, path, line, column, kind); }
    if kind == "record accessor" {
        let source_line = source.split('\n').nth(line.checked_sub(1)?)?;
        let mut message = vec![];
        text(&mut message, format!("I am trying to parse a record accessor here:\n\n{line}| {source_line}\n{}", " ".repeat(column+line.to_string().len()+1)));
        message.push(color("^", "RED"));
        text(&mut message, "\nSomething like ".into());
        message.push(color(".name", "yellow"));
        text(&mut message, " or ".into());
        message.push(color(".price", "yellow"));
        text(&mut message, " that accesses a value from a record.\n\n".into());
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
        text(&mut message, ": Record field names must start with a lower case letter!".into());
        return Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":"EXPECTING RECORD ACCESSOR","region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
        }]}]}));
    }
    if let Some(rest) = kind.strip_prefix("( ") {
        let (phase, start) = rest.split_once(' ')?;
        let phase = phase.strip_prefix("indent-").unwrap_or(phase);
        let start = start.split(';').next()?.parse::<usize>().ok()?;
        if !matches!(phase, "open" | "end" | "next" | "close" | "operator-close" | "reserved-pipe" | "reserved-arrow" | "reserved-equals" | "reserved-colon") || start == 0 || start > line || column == 0 { return None; }
        let reserved = phase.starts_with("reserved-");
        let mut message = vec![];
        let heading = if reserved { "I ran into an unexpected symbol here:" } else { match phase {
            "open" => "I just saw an open parenthesis, so I was expecting to see an expression next.",
            "end" => "I was expecting to see a closing parenthesis next:",
            "operator-close" => "I was expecting a closing parenthesis here:",
            "close" => "I was expecting to see a closing parentheses next, but I got stuck here:",
            _ => "I think I am in the middle of parsing a tuple. I just saw a comma, so I was expecting to see an expression next.",
        }};
        text(&mut message, format!("{}\n\n", reflow(heading)));
        let digits = line.to_string().len();
        for (index, source_line) in source.split('\n').enumerate().skip(start-1).take(line-start+1) {
            text(&mut message, format!("{:>digits$}| {source_line}\n",index+1));
        }
        text(&mut message," ".repeat(column+digits+1));
        message.push(color("^", "RED"));
        match phase {
            "reserved-pipe" | "reserved-equals" | "reserved-colon" => {
                text(&mut message, "\nTry ".into());
                message.push(color(match phase { "reserved-pipe" => "(||)", "reserved-equals" => "(==)", _ => "(::)" }, "yellow"));
                text(&mut message, match phase {
                    "reserved-pipe" => " instead? To turn boolean OR into a function?",
                    "reserved-equals" => " instead? To make a function that checks equality?",
                    _ => " instead? To add values to the front of lists?",
                }.into());
            }
            "reserved-arrow" => {
                text(&mut message, "\nMaybe you wanted ".into());
                message.push(color("(>)", "yellow"));
                text(&mut message, " or ".into());
                message.push(color("(>=)", "yellow"));
                text(&mut message, " instead?".into());
            }
            "open" => {
                text(&mut message, "\nSomething like ".into());
                message.push(color("(4 + 5)", "yellow"));
                text(&mut message, " or ".into());
                message.push(color("(String.reverse \"desserts\")", "yellow"));
                text(&mut message, ". Anything where you are\nputting parentheses around normal expressions.".into());
            }
            "end" | "close" | "operator-close" => {
                text(&mut message, "\nTry adding a ".into());
                message.push(color(")", "yellow"));
                text(&mut message, if phase == "close" { " to see if that helps?" } else { " to see if that helps!" }.into());
            }
            _ => {
                text(&mut message, "\nA tuple looks like ".into());
                message.push(color("(3,4)", "yellow"));
                text(&mut message, " or ".into());
                message.push(color("(\"Tom\",42)", "yellow"));
                text(&mut message, ", so I think there is an expression\nmissing here?".into());
            }
        }
        if !reserved {
        text(&mut message, "\n\n".into());
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
        let note = if phase == "operator-close" { "Note: I think I am parsing an operator function right now, so I am expecting to see something like (+) or (&&) where an operator is surrounded by parentheses with no extra spaces." } else if phase == "close" { "Note: I can get stuck when I run into keywords, operators, parentheses, or brackets unexpectedly. So there may be some earlier syntax trouble (like extra parenthesis or missing brackets) that is confusing me." } else if phase == "end" { "Note: I can get confused by indentation in cases like this, so maybe you have a closing parenthesis but it is not indented enough?" } else { "Note: I can get confused by indentation in cases like this, so maybe you have an expression but it is not indented enough?" };
        text(&mut message, reflow(note)[4..].to_string());
        }
        return Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":if reserved { "UNEXPECTED SYMBOL" } else if phase == "operator-close" { "UNFINISHED OPERATOR FUNCTION" } else if phase == "next" { "UNFINISHED TUPLE" } else { "UNFINISHED PARENTHESES" },"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
        }]}]}));
    }
    if kind.starts_with("if ") {
        return if_report(source, name, path, line, column, kind);
    }
    let (phase, start) = kind.strip_prefix("case ")?.split_once(' ')?;
    if !matches!(
        phase,
        "arrow"
            | "of"
            | "indent-arrow"
            | "indent-expr"
            | "indent-of"
            | "indent-pattern"
            | "indent-branch"
    ) {
        return None;
    }
    let indentation = phase.starts_with("indent-") || phase == "of";
    let start = start.split(';').next()?.parse::<usize>().ok()?;
    if start == 0 || start > line || column == 0 {
        return None;
    }
    let source_line = source.split('\n').nth(line - 1)?;
    let offset = source_line
        .char_indices()
        .nth(column - 1)
        .map_or(source_line.len(), |(offset, _)| offset);
    let rest = &source_line[offset..];
    let (tokens, _) = crate::lexer::lex_prefix(rest);
    let token = tokens.first().map(|t| t.text(rest)).unwrap_or("");
    let keyword = !indentation && crate::parser::reserved(token);
    let (title, width) = if indentation {
        ("UNFINISHED CASE", 0)
    } else if keyword {
        ("RESERVED WORD", token.chars().count())
    } else if matches!(token, ":" | "=") {
        ("UNEXPECTED OPERATOR", 0)
    } else {
        ("MISSING ARROW", 0)
    };
    let mut message = vec![];
    text(
        &mut message,
        if indentation {
            "I was partway through parsing a `case` expression, but I got stuck here:\n\n"
        } else {
            "I am partway through parsing a `case` expression, but I got stuck here:\n\n"
        }
        .into(),
    );
    let digits = line.to_string().len();
    for (index, source_line) in source
        .split('\n')
        .enumerate()
        .skip(start - 1)
        .take(line - start + 1)
    {
        text(
            &mut message,
            format!("{:>digits$}| {source_line}\n", index + 1),
        );
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(color(&"^".repeat(width.max(1)), "RED"));
    text(&mut message, "\n".into());
    if keyword {
        text(
            &mut message,
            reflow(&format!(
                "It looks like you are trying to use `{token}` in one of your patterns, but it is a reserved word. Try using a different name?"
            )),
        );
    } else if !indentation && matches!(token, ":" | "=") {
        text(&mut message, "I am seeing ".into());
        message.push(color(token, "yellow"));
        text(&mut message, " but maybe you want ".into());
        message.push(color(if token == ":" { "::" } else { "->" }, "GREEN"));
        text(
            &mut message,
            if token == ":" {
                " instead? For pattern matching on lists?"
            } else {
                " instead?"
            }
            .into(),
        );
    } else {
        if indentation {
            match phase {
                "indent-arrow" => {
                    text(
                        &mut message,
                        "I just saw a pattern, so I was expecting to see a ".into(),
                    );
                    message.push(color("->", "yellow"));
                    text(&mut message, " next.".into());
                }
                "indent-of" | "of" => {
                    text(&mut message, "I was expecting to see the ".into());
                    message.push(color("of", "yellow"));
                    text(&mut message, " keyword next.".into());
                }
                "indent-expr" => text(
                    &mut message,
                    "I was expecting to see a expression next.".into(),
                ),
                "indent-pattern" => text(
                    &mut message,
                    "I was expecting to see a pattern next.".into(),
                ),
                "indent-branch" => text(
                    &mut message,
                    reflow(
                        "I was expecting to see an expression next. What should I do when I run into this particular pattern?",
                    ),
                ),
                _ => unreachable!(),
            }
            text(&mut message, "\n\n".into());
        } else {
            text(
                &mut message,
                "I was expecting to see an arrow next.\n\n".into(),
            );
        }
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
        let note = reflow(if indentation {
            "Note: Here is an example of a valid `case` expression for reference."
        } else {
            "Note: Sometimes I get confused by indentation, so try to make your `case` look something like this:"
        });
        text(&mut message, note[4..].to_string());
        text(&mut message, "\n\n    ".into());
        message.push(color("case", "CYAN"));
        text(&mut message, " maybeWidth ".into());
        message.push(color("of", "CYAN"));
        text(&mut message, "\n      ".into());
        message.push(color("Just", "BLUE"));
        text(&mut message, " width ->\n        width + ".into());
        message.push(color("200", "yellow"));
        text(&mut message, "\n\n      ".into());
        message.push(color("Nothing", "BLUE"));
        text(&mut message, " ->\n        ".into());
        message.push(color("400", "yellow"));
        text(
            &mut message,
            format!(
                "\n\n{}",
                reflow(if indentation {
                    "Notice the indentation. Each pattern is aligned, and each branch is indented a bit more than the corresponding pattern. That is important!"
                } else {
                    "Notice the indentation! Patterns are aligned with each other. Same indentation. The expressions after each arrow are all indented a bit more than the patterns. That is important!"
                })
            ),
        );
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":title,"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
        }]}]}),
    )
}

fn if_report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    kind: &str,
) -> Option<Value> {
    let (phase, start) = kind.strip_prefix("if ")?.split_once(' ')?;
    let indentation = phase.starts_with("indent-");
    let phase = phase.strip_prefix("indent-").unwrap_or(phase);
    if !matches!(
        phase,
        "condition" | "then" | "else" | "then-branch" | "else-branch" | "else-start"
    ) {
        return None;
    }
    let start = start.split(';').next()?.parse::<usize>().ok()?;
    if start == 0 || start > line || column == 0 {
        return None;
    }
    let misplaced_else = if indentation && phase == "else" {
        source.split('\n').nth(line).and_then(|next| {
            let stripped = next.trim_start_matches(' ');
            let suffix = stripped.strip_prefix("else")?;
            if suffix
                .chars()
                .next()
                .is_some_and(|c| crate::unicode::is_alphanumeric(c) || c == '_')
            {
                return None;
            }
            Some((line + 1, next.len() - stripped.len() + 1))
        })
    } else {
        None
    };
    let (line, column) = misplaced_else.unwrap_or((line, column));
    let width = if misplaced_else.is_some() { 4 } else { 0 };
    let branch = match phase {
        "then-branch" => Some("then"),
        "else-branch" => Some("else"),
        _ => None,
    };
    let mut message = vec![];
    text(
        &mut message,
        if phase == "else-start" {
            "I just saw the start of an `else` branch, but then I got stuck here:\n\n".into()
        } else if misplaced_else.is_some() {
            "I was partway through an `if` expression when I got stuck here:\n\n".into()
        } else if indentation && phase == "else" {
            "I was expecting to see an `else` branch after this:\n\n".into()
        } else {
            match branch {
                Some(branch) => {
                    format!("I got stuck after the start of this `{branch}` branch:\n\n")
                }
                None => {
                    "I was expecting to see more of this `if` expression, but I got stuck here:\n\n"
                        .into()
                }
            }
        },
    );
    let digits = line.to_string().len();
    for (index, source_line) in source
        .split('\n')
        .enumerate()
        .skip(start - 1)
        .take(line - start + 1)
    {
        text(
            &mut message,
            format!("{:>digits$}| {source_line}\n", index + 1),
        );
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(color(&"^".repeat(width.max(1)), "RED"));
    text(&mut message, "\n".into());
    if misplaced_else.is_some() {
        text(&mut message, "I think this ".into());
        message.push(color("else", "CYAN"));
        text(
            &mut message,
            " keyword needs to be indented more. Try adding some spaces\nbefore it.".into(),
        );
    } else if indentation && phase == "else" {
        text(
            &mut message,
            "I know what to do when the condition is True, but what happens when it is False?
Add an "
                .into(),
        );
        message.push(color("else", "CYAN"));
        text(&mut message, " branch to handle that scenario!".into());
    } else {
        match phase {
            "condition" => {
                text(
                    &mut message,
                    "I was expecting to see an expression like ".into(),
                );
                message.push(color("x < 0", "yellow"));
                text(&mut message, " that evaluates to True or False.".into());
            }
            "then" | "else" => {
                text(&mut message, "I was expecting to see the ".into());
                message.push(color(phase, "CYAN"));
                text(&mut message, " keyword next.".into());
            }
            _ => text(
                &mut message,
                reflow("I was expecting to see an expression next. Maybe it is not filled in yet?"),
            ),
        }
    }
    if indentation && phase != "else" {
        text(&mut message, "\n\n".into());
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
        let note = match branch {
            Some(branch) => format!(
                "Note: I can be confused by indentation, so if the `{branch}` branch is already present, it may not be indented enough for me to recognize it."
            ),
            None => {
                "Note: I can be confused by indentation. Maybe something is not indented enough?"
                    .into()
            }
        };
        text(&mut message, reflow(&note)[4..].to_string());
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
            "title":if misplaced_else.is_some() { "WEIRD ELSE BRANCH" } else { "UNFINISHED IF" },"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
        }]}]}),
    )
}

fn list_report(source: &str, name: &str, path: &Path, line: usize, column: usize, kind: &str) -> Option<Value> {
    let (phase, start) = kind.strip_prefix("[ ")?.split_once(' ')?;
    let start = start.split(';').next()?.parse::<usize>().ok()?;
    if start == 0 || start > line || column == 0 { return None; }
    let heading = match phase {
        "open" | "close" => "I am partway through parsing a list, but I got stuck here:",
        "indent-open" | "indent-end" => "I cannot find the end of this list:",
        "indent-next" => "I was expecting to see another list entry after this comma:",
        "entry" => "I was expecting to see another list entry after that last comma:",
        _ => return None,
    };
    let mut message = vec![];
    text(&mut message, format!("{}\n\n",reflow(heading)));
    let digits = line.to_string().len();
    for (index, source_line) in source.split('\n').enumerate().skip(start-1).take(line-start+1) {
        text(&mut message, format!("{:>digits$}| {source_line}\n",index+1));
    }
    text(&mut message," ".repeat(column+digits+1));
    message.push(color("^","RED"));
    text(&mut message,"\n".into());
    match phase {
        "open" | "close" => {
            text(&mut message,"I was expecting to see a closing square bracket before this, so try adding a ".into());
            message.push(color("]","yellow"));
            text(&mut message,"\nand see if that helps?".into());
        }
        "indent-open" => {
            text(&mut message,"You could change it to something like ".into());
            message.push(color("[3,4,5]","yellow"));
            text(&mut message," or even just ".into());
            message.push(color("[]","yellow"));
            text(&mut message,". Anything where\nthere is an open and close square brace, and where the elements of the list are\nseparated by commas.".into());
        }
        "indent-end" => {
            text(&mut message,"You can just add a closing ".into());
            message.push(color("]","yellow"));
            text(&mut message," right here, and I will be all set!".into());
        }
        _ => text(&mut message,reflow("Trailing commas are not allowed in lists, so the fix may be to delete the comma?")),
    }
    text(&mut message,"\n\n".into());
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
    let note = match phase {
        "open" | "close" => "Note: When I get stuck like this, it usually means that there is a missing parenthesis or bracket somewhere earlier. It could also be a stray keyword or operator.",
        "indent-open" | "indent-end" => "Note: I may be confused by indentation. For example, if you are trying to define a list across multiple lines, I recommend using this format:",
        _ => "Note: I recommend using the following format for lists that span multiple lines:",
    };
    text(&mut message,reflow(note)[4..].to_string());
    if !matches!(phase,"open" | "close") {
        text(&mut message,"\n\n    [ ".into());
        message.push(color("\"Alice\"","yellow"));
        text(&mut message,"\n    , ".into());
        message.push(color("\"Bob\"","yellow"));
        text(&mut message,"\n    , ".into());
        message.push(color("\"Chuck\"","yellow"));
        text(&mut message,format!("\n    ]\n\n{}",reflow("Notice that each line starts with some indentation. Usually two or four spaces. This is the stylistic convention in the Elm ecosystem.")));
    }
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":"UNFINISHED LIST","region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
    }]}]}))
}

fn record_report(source: &str, name: &str, path: &Path, line: usize, column: usize, kind: &str) -> Option<Value> {
    let (phase, start) = kind.strip_prefix("{ ")?.split_once(' ')?;
    let phase = phase.strip_prefix("indent-").unwrap_or(phase);
    let start = start.split(';').next()?.parse::<usize>().ok()?;
    if start == 0 || start > line || column == 0 { return None; }
    let misplaced = if phase == "end" {
        source.split('\n').nth(line).and_then(|next| {
            let stripped = next.trim_start_matches(' ');
            stripped.starts_with('}').then_some((line + 1, next.len() - stripped.len() + 1))
        })
    } else { None };
    let (line, column) = misplaced.unwrap_or((line, column));
    let field_token = matches!(phase, "open-token" | "field-token");
    let source_line = source.split('\n').nth(line-1)?;
    let offset = source_line.char_indices().nth(column-1).map_or(source_line.len(), |(offset,_)| offset);
    let rest = &source_line[offset..];
    let (tokens, _) = crate::lexer::lex_prefix(rest);
    let token = tokens.first().map(|t| t.text(rest)).unwrap_or("");
    let keyword = field_token && crate::parser::reserved(token);
    let extra_comma = phase == "field-token" && matches!(token, "," | "}");
    let width = if keyword { token.chars().count() } else { 0 };
    let regular_note = matches!(phase, "field" | "equals-token" | "open-token" | "field-token") || misplaced.is_some();
    let heading = match phase {
        "open-token" if keyword => "I just started parsing a record, but I got stuck on this field name:",
        "open-token" => "I just started parsing a record, but I got stuck here:",
        "field-token" if keyword => "I am partway through parsing a record, but I got stuck on this field name:",
        "field-token" => "I am partway through parsing a record, but I got stuck here:",
        "equals-token" | "close-token" => "I am partway through parsing a record, but I got stuck here:",
        "end" => "I was partway through parsing a record, but I got stuck here:",
        "open" => "I just saw the opening curly brace of a record, but then I got stuck here:",
        "equals" => "I am partway through parsing a record. I just saw a record field, so I was expecting to see an equals sign next:",
        "expr" => "I am partway through parsing a record, and I was expecting to run into an expression next:",
        "field" => "I am partway through parsing a record, but I got stuck after that last comma:",
        _ => return None,
    };
    let mut message = vec![];
    text(&mut message,format!("{}\n\n",reflow(heading)));
    let digits = line.to_string().len();
    for (index, source_line) in source.split('\n').enumerate().skip(start-1).take(line-start+1) {
        text(&mut message,format!("{:>digits$}| {source_line}\n",index+1));
    }
    text(&mut message," ".repeat(column+digits+1));
    message.push(color(&"^".repeat(width.max(1)),"RED"));
    text(&mut message,"\n".into());
    if keyword {
        text(&mut message,reflow(&format!("It looks like you are trying to use `{token}` as a field name, but that is a reserved word. Try using a different name!")));
    } else { match phase {
        "field-token" if token == "," => text(&mut message,"I am seeing two commas in a row. This is the second one!\n\nJust delete one of the commas and you should be all set!".into()),
        "field-token" if token == "}" => text(&mut message,reflow("Trailing commas are not allowed in records. Try deleting the comma that appears before this closing curly brace.")),
        "open-token" | "field-token" => {
            text(&mut message,if phase == "open-token" { "I was expecting to see a record field defined next, so I am looking for a name\nlike " } else { "I was expecting to see another record field defined next, so I am looking for a\nname like " }.into());
            message.push(color("userName","yellow"));
            text(&mut message," or ".into());
            message.push(color("plantHeight","yellow"));
            text(&mut message,".\n\n".into());
            message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
            text(&mut message,": Field names must start with a lower-case letter. After that, you can use\nany sequence of letters, numbers, and underscores.".into());
        }
        "close-token" => {
            text(&mut message,"I was expecting to see a closing curly brace before this, so try adding a ".into());
            message.push(color("}","yellow"));
            text(&mut message," and\nsee if that helps?".into());
        }
        "equals-token" => {
            text(&mut message,"I just saw a field name, so I was expecting to see an equals sign next. So try\nputting an ".into());
            message.push(color("=","GREEN"));
            text(&mut message," sign here?".into());
        }
        "end" if misplaced.is_some() => text(&mut message,reflow("I need this curly brace to be indented more. Try adding some spaces before it!")),
        "end" => {
            text(&mut message,"I was expecting to see a closing curly brace next. Try putting a ".into());
            message.push(color("}","GREEN"));
            text(&mut message," next and see\nif that helps?".into());
        }
        "open" => {
            text(&mut message,"I am expecting a record like ".into());
            message.push(color("{ x = 3, y = 4 }","yellow"));
            text(&mut message," here. Try defining some fields of\nyour own?".into());
        }
        "equals" => {
            text(&mut message,"Try putting an ".into());
            message.push(color("=","GREEN"));
            text(&mut message," followed by an expression?".into());
        }
        "expr" => {
            text(&mut message,"Try putting something like ".into());
            message.push(color("42","yellow"));
            text(&mut message," or ".into());
            message.push(color("\"hello\"","yellow"));
            text(&mut message," for now?".into());
        }
        _ => text(&mut message,reflow("Trailing commas are not allowed in records, so the fix may be to delete that last comma? Or maybe you were in the middle of defining an additional field?")),
    }
    }
    if !keyword {
    text(&mut message,"\n\n".into());
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
    let note = if phase == "close-token" { "Note: When I get stuck like this, it usually means that there is a missing parenthesis or bracket somewhere earlier. It could also be a stray keyword or operator." } else if regular_note { "Note: If you are trying to define a record across multiple lines, I recommend using this format:" } else { "Note: I may be confused by indentation. For example, if you are trying to define a record across multiple lines, I recommend using this format:" };
    text(&mut message,reflow(note)[4..].to_string());
    if phase != "close-token" {
    text(&mut message,"\n\n    { name = ".into());
    message.push(color("\"Alice\"","yellow"));
    text(&mut message,"\n    , age = ".into());
    message.push(color("42","yellow"));
    text(&mut message,"\n    , height = ".into());
    message.push(color("1.75","yellow"));
    text(&mut message,format!("\n    }}\n\n{}",reflow(&format!("Notice that each line starts with some indentation. Usually two or four spaces. This is the stylistic convention in the Elm ecosystem{}",if regular_note { "." } else { "!" }))));
    }
    }
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":if keyword { "RESERVED WORD" } else if extra_comma { "EXTRA COMMA" } else if matches!(phase,"equals-token" | "close-token" | "open-token" | "field-token") { "PROBLEM IN RECORD" } else if misplaced.is_some() { "NEED MORE INDENTATION" } else { "UNFINISHED RECORD" },"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
    }]}]}))
}

fn lambda_report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    kind: &str,
) -> Option<Value> {
    let (raw_phase, start) = kind.strip_prefix("\\ ")?.split_once(' ')?;
    let indentation = raw_phase.starts_with("indent-");
    let phase = raw_phase.strip_prefix("indent-").unwrap_or(raw_phase);
    let start = start.split(';').next()?.parse::<usize>().ok()?;
    if !matches!(phase, "argument" | "arrow" | "body") || start == 0 || start > line || column == 0 {
        return None;
    }
    let source_line = source.split('\n').nth(line - 1)?;
    let offset = source_line.char_indices().nth(column - 1).map_or(source_line.len(), |(offset, _)| offset);
    let rest = &source_line[offset..];
    let (tokens, _) = crate::lexer::lex_prefix(rest);
    let token = tokens.first().map(|t| t.text(rest)).unwrap_or("");
    let keyword = !indentation && crate::parser::reserved(token);
    let width = if keyword { token.chars().count() } else { 0 };
    let heading = if keyword { "I was parsing an anonymous function, but I got stuck here:" } else { match phase {
        "argument" => "I just saw the beginning of an anonymous function, so I was expecting to see an argument next:",
        "arrow" => "I just saw the beginning of an anonymous function, so I was expecting to see an arrow next:",
        _ => "I was expecting to see the body of your anonymous function next:",
    }};
    let mut message = vec![];
    text(&mut message, format!("{}\n\n", reflow(heading)));
    let digits = line.to_string().len();
    for (index, source_line) in source.split('\n').enumerate().skip(start - 1).take(line - start + 1) {
        text(&mut message, format!("{:>digits$}| {source_line}\n", index + 1));
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(color(&"^".repeat(width.max(1)), "RED"));
    if keyword {
        text(&mut message, format!("\n{}", reflow(&format!("It looks like you are trying to use `{token}` as an argument, but it is a reserved word in this language. Try using a different argument name!"))));
    } else if phase == "argument" {
        text(&mut message, "\nSomething like ".into());
        message.push(color("x", "yellow"));
        text(&mut message, " or ".into());
        message.push(color("name", "yellow"));
        text(&mut message, ". Anything that starts with a lower case letter!".into());
    } else {
        text(&mut message, "\nThe syntax for anonymous functions is ".into());
        message.push(color("(\\x -> x + 1)", "yellow"));
        text(&mut message, if phase == "arrow" {
            " so I am missing the arrow\nand the body of the function."
        } else {
            " so I am missing all the\nstuff after the arrow!"
        }.into());
    }
    if indentation {
    text(&mut message, "\n\n".into());
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
    let note = if phase == "argument" {
        "Note: The syntax for anonymous functions is (\\x -> x + 1) where the backslash is meant to look a bit like a lambda if you squint. This visual pun seemed like a better idea at the time!"
    } else {
        "Note: It is possible that I am confused about indetation! I generally recommend switching to named functions if the definition cannot fit inline nicely, so either (1) try to fit the whole anonymous function on one line or (2) break the whole thing out into a named function. Things tend to be clearer that way!"
    };
    text(&mut message, reflow(note)[4..].to_string());
    }
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":if keyword { "RESERVED WORD" } else if phase == "argument" { "MISSING ARGUMENT" } else { "UNFINISHED ANONYMOUS FUNCTION" },
        "region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
    }]}]}))
}

fn let_report(
    source: &str,
    name: &str,
    path: &Path,
    line: usize,
    column: usize,
    kind: &str,
) -> Option<Value> {
    let (raw_phase, start) = kind.strip_prefix("let ")?.split_once(' ')?;
    let indentation = raw_phase.starts_with("indent-");
    let phase = raw_phase.strip_prefix("indent-").unwrap_or(raw_phase);
    let start = start.split(';').next()?.parse::<usize>().ok()?;
    if !matches!(phase, "definition" | "body" | "in" | "name") || start == 0 || start > line || column == 0 {
        return None;
    }
    let source_line = source.split('\n').nth(line - 1)?;
    let offset = source_line.char_indices().nth(column - 1).map_or(source_line.len(), |(offset, _)| offset);
    let rest = &source_line[offset..];
    let (tokens, _) = crate::lexer::lex_prefix(rest);
    let token = tokens.first().map(|t| t.text(rest)).unwrap_or("");
    let keyword = phase == "name" && crate::parser::reserved(token);
    let width = if keyword { token.chars().count() } else { 0 };
    let mut message = vec![];
    text(&mut message, "I was partway through parsing a `let` expression, but I got stuck here:\n\n".into());
    let digits = line.to_string().len();
    for (index, source_line) in source.split('\n').enumerate().skip(start - 1).take(line - start + 1) {
        text(&mut message, format!("{:>digits$}| {source_line}\n", index + 1));
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(color(&"^".repeat(width.max(1)), "RED"));
    if keyword {
        text(&mut message, format!("\n{}", reflow(&format!("It looks like you are trying to use `{token}` as a variable name, but it is a reserved word! Try using a different name instead."))));
    } else {
    if phase == "in" {
        text(&mut message, if indentation { "\nI was expecting to see the " } else { "\nBased on the indentation, I was expecting to see the " }.into());
        message.push(color("in", "CYAN"));
        text(&mut message, if indentation { " keyword next. Or maybe more of that expression?\n\n" } else { " keyword next. Is there a\ntypo?\n\n" }.into());
    } else {
    let explanation = if phase == "name" {
        "I was expecting the name of a definition next."
    } else if phase == "definition" {
        "I was expecting a value to be defined here."
    } else {
        "I was expecting an expression next. Tell me what should happen with the value you just defined!"
    };
    text(&mut message, format!("\n{}\n\n", reflow(explanation)));
    }
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
    if phase == "in" && !indentation {
        text(&mut message, reflow("Note: This can also happen if you are trying to define another value within the `let` but it is not indented enough. Make sure each definition has exactly the same amount of spaces before it. They should line up exactly!")[4..].to_string());
    } else {
    text(&mut message, ": Here is an example with a valid `let` expression for reference:\n\n    viewPerson person =\n      ".into());
    message.push(color("let", "CYAN"));
    text(&mut message, "\n        fullName =\n          person.firstName ++ ".into());
    message.push(color("\" \"", "yellow"));
    text(&mut message, " ++ person.lastName\n      ".into());
    message.push(color("in", "CYAN"));
    text(&mut message, format!("\n      div [] [ text fullName ]\n\n{}", reflow("Here we defined a `viewPerson` function that turns a person into some HTML. We use a `let` expression to define the `fullName` we want to show. Notice the indentation! The `fullName` is indented more than the `let` keyword, and the actual value of `fullName` is indented a bit more than that. That is important!")));
    }
    }
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":if keyword { "RESERVED WORD" } else if indentation || phase == "name" { "UNFINISHED LET" } else { "LET PROBLEM" },"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
    }]}]}))
}

fn local_definition_report(source: &str, name: &str, path: &Path, line: usize, column: usize, kind: &str) -> Option<Value> {
    let mut parts = kind.strip_prefix("local-definition ")?.splitn(3, ' ');
    let phase = parts.next()?;
    let start = parts.next()?.parse::<usize>().ok()?;
    let definition = parts.next()?;
    if !matches!(phase, "equals" | "type" | "body") || start == 0 || start > line || column == 0 { return None; }
    let heading = format!("I got stuck while parsing the `{definition}` {}:", if phase == "type" { "type annotation" } else { "definition" });
    let mut message = vec![];
    text(&mut message, format!("{}\n\n", reflow(&heading)));
    let digits = line.to_string().len();
    for (index, source_line) in source.split('\n').enumerate().skip(start - 1).take(line - start + 1) {
        text(&mut message, format!("{:>digits$}| {source_line}\n", index + 1));
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(color("^", "RED"));
    let explanation = match phase {
        "equals" => "I was expecting to see an argument or an equals sign next.",
        "type" => "I just saw a colon, so I am expecting to see a type next.",
        _ => "I was expecting to see an expression next. What is it equal to?",
    };
    text(&mut message, format!("\n{}\n\nHere is a valid definition (with a type annotation) for reference:\n\n    greet : String -> String\n    greet name =\n      ", reflow(explanation)));
    message.push(color("\"Hello \"", "yellow"));
    text(&mut message, " ++ name ++ ".into());
    message.push(color("\"!\"", "yellow"));
    text(&mut message, format!("\n\n{}", reflow("The top line (called a \"type annotation\") is optional. You can leave it off if you want. As you get more comfortable with Elm and as your project grows, it becomes more and more valuable to add them though! They work great as compiler-verified documentation, and they often improve error messages!")));
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":"UNFINISHED DEFINITION","region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
    }]}]}))
}

fn local_equals_report(source: &str, name: &str, path: &Path, line: usize, column: usize, kind: &str) -> Option<Value> {
    let (start, definition) = kind.strip_prefix("local-equals ")?.split_once(' ')?;
    let start = start.parse::<usize>().ok()?;
    let definition = definition.split(';').next()?;
    if start == 0 || start > line || column == 0 { return None; }
    let source_line = source.split('\n').nth(line - 1)?;
    let rest: String = source_line.chars().skip(column - 1).collect();
    let (tokens, _) = crate::lexer::lex_prefix(&rest);
    let token = tokens.first().map(|t| t.text(&rest)).unwrap_or("");
    let keyword = crate::parser::reserved(token);
    let op: String = rest.chars().take_while(|c| "+-/*=.<>:&|^?%!".contains(*c)).collect();
    let width = if keyword { token.chars().count() } else { op.chars().count() };
    let title = if keyword { "RESERVED WORD" } else if op == "->" { "MISSING COLON?" } else if !op.is_empty() { "UNEXPECTED SYMBOL" } else { "PROBLEM IN DEFINITION" };
    let mut message = vec![];
    if keyword {
        let heading = reflow(&format!("The name `{token}` is reserved in Elm, so it cannot be used as an argument here:"));
        let marker = format!("`{token}`");
        let (before, after) = heading.split_once(&marker)?;
        text(&mut message, format!("{before}`"));
        message.push(color(token, "CYAN"));
        text(&mut message, format!("`{after}\n\n"));
    } else {
        let heading = if op == "->" { "I was not expecting to see an arrow here:".into() } else if !op.is_empty() { "I was not expecting to see this symbol here:".into() } else { format!("I got stuck while parsing the `{definition}` definition:") };
        text(&mut message, format!("{}\n\n", reflow(&heading)));
    }
    let digits = line.to_string().len();
    for (index, source_line) in source.split('\n').enumerate().skip(start - 1).take(line - start + 1) {
        text(&mut message, format!("{:>digits$}| {source_line}\n", index + 1));
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(color(&"^".repeat(width.max(1)), "RED"));
    if keyword {
        text(&mut message, "\nTry renaming it to something else.\n\n".into());
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
        if token == "as" {
            text(&mut message, ": This keyword is reserved for pattern matches like ((x,y) ".into());
            message.push(color("as", "CYAN"));
            text(&mut message, " point) where\nyou want to name a tuple and the values it contains.".into());
        } else {
            text(&mut message, reflow(&format!("Note: The `{token}` keyword has a special meaning in Elm, so it can only be used in certain situations."))[4..].to_string());
        }
    } else {
        if op == "->" {
            text(&mut message, "\nThis usually means a ".into());
            message.push(color(":", "GREEN"));
            text(&mut message, " is missing a bit earlier in a type annotation. It could\nbe something else though, so here is a valid definition for reference:".into());
        } else {
            text(&mut message, format!("\n{}", reflow("I am not sure what is going wrong exactly, so here is a valid definition (with an optional type annotation) for reference:")));
        }
        text(&mut message, "\n\n    greet : String -> String\n    greet name =\n      ".into());
        message.push(color("\"Hello \"", "yellow"));
        text(&mut message, " ++ name ++ ".into());
        message.push(color("\"!\"", "yellow"));
        let ending = if op.is_empty() { "Try to use that format!".into() } else { format!("Try to use that format with your `{definition}` definition!") };
        text(&mut message, format!("\n\n{}", reflow(&ending)));
    }
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":title,"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+width}},"message":message
    }]}]}))
}

fn local_destruct_report(source: &str, name: &str, path: &Path, line: usize, column: usize, kind: &str) -> Option<Value> {
    let (phase, start) = kind.strip_prefix("local-destruct ")?.split_once(' ')?;
    let start = start.split(';').next()?.parse::<usize>().ok()?;
    if !matches!(phase, "equals" | "indent-equals" | "indent-body") || start == 0 || start > line || column == 0 { return None; }
    let body = phase == "indent-body";
    let mut message = vec![];
    text(&mut message, if body { "I got stuck while parsing this definition:\n\n" } else { "I got stuck trying to parse this definition:\n\n" }.into());
    let digits = line.to_string().len();
    for (index, source_line) in source.split('\n').enumerate().skip(start - 1).take(line - start + 1) {
        text(&mut message, format!("{:>digits$}| {source_line}\n", index + 1));
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(color("^", "RED"));
    let explanation = if body { "I was expecting to see an expression next. What is it equal to?" } else { "I was expecting to see an equals sign next, followed by an expression telling me what to compute." };
    text(&mut message, format!("\n{}", reflow(explanation)));
    let rest: String = source.split('\n').nth(line - 1)?.chars().skip(column - 1).collect();
    let operator: String = rest.chars().take_while(|c| "+-/*=.<>:&|^?%!".contains(*c)).collect();
    if phase == "equals" && operator == ":" {
        text(&mut message, "\n\n".into());
        message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
        text(&mut message, reflow("Note: It looks like you may be trying to write a type annotation? It is not possible to add type annotations on destructuring definitions like this. You can assign a name to the overall structure, put a type annotation on that, and then destructure separately though.")[4..].to_string());
    }
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":if phase == "equals" { "PROBLEM IN DEFINITION" } else { "UNFINISHED DEFINITION" },"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
    }]}]}))
}

fn local_annotation_report(source: &str, name: &str, path: &Path, line: usize, column: usize, kind: &str) -> Option<Value> {
    let mut parts = kind.strip_prefix("local-annotation ")?.split(';').next()?.splitn(4, ' ');
    let phase = parts.next()?;
    let start = parts.next()?.parse::<usize>().ok()?;
    let definition = parts.next()?;
    let detail = parts.next()?.trim();
    if start == 0 || start > line || column == 0 { return None; }
    let (title, heading) = match phase {
        "alignment" => ("PROBLEM IN DEFINITION", format!("I got stuck while parsing the `{definition}` definition:")),
        "missing" => ("EXPECTING DEFINITION", format!("I just saw the type annotation for `{definition}` so I was expecting to see its definition here:")),
        "mismatch" => ("NAME MISMATCH", format!("I just saw a type annotation for `{definition}`, but it is followed by a definition for `{detail}`:")),
        _ => return None,
    };
    let mut message = vec![];
    text(&mut message, format!("{}\n\n", reflow(&heading)));
    let digits = line.to_string().len();
    for (index, source_line) in source.split('\n').enumerate().skip(start - 1).take(line - start + 1) {
        text(&mut message, format!("{:>digits$}| {source_line}\n", index + 1));
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(color("^", "RED"));
    match phase {
        "alignment" => {
            let indent = detail.parse::<u16>().ok()?;
            let offset = indent.wrapping_sub(column as u16);
            text(&mut message, format!("\n{}", reflow(&format!("I just saw a type annotation indented {indent} spaces, so I was expecting to see the corresponding definition next with the exact same amount of indentation. It looks like this line needs {offset} more {}?", if offset == 1 { "space" } else { "spaces" }))));
        }
        "mismatch" => {
            text(&mut message, "\nThese names do not match! Is there a typo?\n\n    ".into());
            message.push(color(detail, "yellow"));
            text(&mut message, " -> ".into());
            message.push(color(definition, "GREEN"));
            message.push(json!(""));
        }
        _ => {
            text(&mut message, format!("\n{}\n\nHere is a valid definition (with a type annotation) for reference:\n\n    greet : String -> String\n    greet name =\n      ", reflow("Type annotations always appear directly above the relevant definition, without anything else in between.")));
            message.push(color("\"Hello \"", "yellow"));
            text(&mut message, " ++ name ++ ".into());
            message.push(color("\"!\"", "yellow"));
            text(&mut message, format!("\n\n{}", reflow("The top line (called a \"type annotation\") is optional. You can leave it off if you want. As you get more comfortable with Elm and as your project grows, it becomes more and more valuable to add them though! They work great as compiler-verified documentation, and they often improve error messages!")));
        }
    }
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":title,"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
    }]}]}))
}

fn expression_start_report(source: &str, name: &str, path: &Path, line: usize, column: usize, kind: &str) -> Option<Value> {
    let (start, thing) = kind.strip_prefix("expression-start ")?.split_once(' ')?;
    let start = start.parse::<usize>().ok()?;
    let thing = thing.split(';').next()?;
    if start == 0 || start > line || column == 0 { return None; }
    let mut message = vec![];
    text(&mut message, format!("{}\n\n", reflow(&format!("I am partway through parsing {thing}, but I got stuck here:"))));
    let digits = line.to_string().len();
    for (index, source_line) in source.split('\n').enumerate().skip(start - 1).take(line - start + 1) {
        text(&mut message, format!("{:>digits$}| {source_line}\n", index + 1));
    }
    text(&mut message, " ".repeat(column + digits + 1));
    message.push(color("^", "RED"));
    text(&mut message, "\nI was expecting to see an expression like ".into());
    message.push(color("42", "yellow"));
    text(&mut message, " or ".into());
    message.push(color("\"hello\"", "yellow"));
    text(&mut message, ". Once there is something\nthere, I can probably give a more specific hint!\n\n".into());
    message.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
    text(&mut message, reflow(if crate::edition::corrected_wording() { "Note: This can also happen if I run into reserved words like `let` or `as` unexpectedly. Or if I run into operators in unexpected spots. Point is, there are a couple ways I can get confused and give sort of weird advice!" } else { "Note: This can also happen if run into reserved words like `let` or `as` unexpectedly. Or if I run into operators in unexpected spots. Point is, there are a couple ways I can get confused and give sort of weird advice!" })[4..].to_string());
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":name,"problems":[{
        "title":"MISSING EXPRESSION","region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column}},"message":message
    }]}]}))
}
