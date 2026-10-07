//! Adapt located compiler failures to Elm's JSON protocol. Until every compiler
//! phase returns structured diagnostics, only accept file prefixes that exist
//! and coordinates that actually lie within that source. Never invent a region.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

const REPL_PREFIX: &str = "ELM_REPL_ERROR:";
pub fn repl_error(entry: &Path, message: String) -> String {
    // Complete CLI documents are not source diagnostics. Keep their envelope
    // intact so main prints the document without exposing the internal marker.
    if message.starts_with("ELM_CLI_RAW:") {
        return message;
    }
    format!(
        "{REPL_PREFIX}{}",
        serde_json::to_string(&(entry, message)).expect("REPL error encoding")
    )
}
fn repl_message(message: &str) -> Option<(PathBuf, String)> {
    serde_json::from_str(message.strip_prefix(REPL_PREFIX)?).ok()
}
fn repl_paths(mut report: Value, entry: Option<&Path>) -> Value {
    if let Some(entry) = entry
        && let Some(errors) = report["errors"].as_array_mut()
    {
        for error in errors {
            if error["path"]
                .as_str()
                .is_some_and(|path| Path::new(path) == entry)
            {
                error["path"] = "REPL".into();
            }
        }
    }
    report
}

/// Preserve Elm's public CLI text without reproducing a runtime deadlock.
/// Manager initialization is asynchronous for these commands in Elm 0.19.1.
pub fn proxy_cli_error(message: &str, command: &str, ansi: bool) -> Option<String> {
    let exception = message.strip_prefix("elm: ")?.strip_suffix('\n')?;
    if !exception.starts_with("HttpExceptionContentWrapper {unHttpExceptionContentWrapper = InvalidProxyEnvironmentVariable ") {
        return None;
    }
    let asynchronous = match command {
        "make" | "init" | "install" => true,
        "diff" | "bump" => false,
        _ => return None,
    };
    let detail = if asynchronous {
        "thread blocked indefinitely in an MVar operation"
    } else {
        exception
    };
    let banner = internal_error_document(detail, ansi);
    Some(if asynchronous { format!("{message}{banner}") } else { banner })
}

pub fn confirmation_eof() -> String {
    use std::io::IsTerminal;
    format!("ELM_CLI_RAW:{}", internal_error_document(
        "<stdin>: hGetLine: end of file", std::io::stderr().is_terminal()))
}

pub fn output_io_error(path: &Path, operation: &str, error: std::io::Error) -> String {
    use std::io::{ErrorKind, IsTerminal};
    let detail = match (operation, error.kind()) {
        ("openBinaryFile", ErrorKind::NotFound) => "does not exist (No such file or directory)",
        ("openBinaryFile", ErrorKind::IsADirectory) => "inappropriate type (Is a directory)",
        ("openBinaryFile" | "createDirectory", ErrorKind::PermissionDenied) => {
            "permission denied (Permission denied)"
        }
        ("createDirectory", ErrorKind::NotADirectory) => "inappropriate type (Not a directory)",
        ("createDirectory", ErrorKind::AlreadyExists) => "already exists (File exists)",
        _ => return format!("{}: {error}", path.display()),
    };
    format!("ELM_CLI_RAW:{}", internal_error_document(
        &format!("{}: {operation}: {detail}", path.display()),
        std::io::stderr().is_terminal()))
}

fn internal_error_document(detail: &str, ansi: bool) -> String {
    let mut banner = include_str!("proxy_error_banner.txt").to_owned();
    if ansi {
        for title in ["-- ERROR -----------------------------------------------------------------------", "-- REQUEST ---------------------------------------------------------------------"] {
            banner = banner.replace(title, &format!("\x1b[33m{title}\x1b[0m"));
        }
        banner = banner.replacen("\n>   ", "\n\x1b[91m>\x1b[0m   ", 1);
    }
    banner.replace("{DETAIL}", detail)
}

pub fn report(message: &str) -> Value {
    let report = report_for_reference(message);
    let text = report.to_string();
    match planexpo_elm::edition::localize_links(&text) {
        std::borrow::Cow::Borrowed(_) => report,
        std::borrow::Cow::Owned(localized) => serde_json::from_str(&localized).unwrap_or(report),
    }
}

fn report_for_reference(message: &str) -> Value {
    if let Some((entry, message)) = repl_message(message) {
        return repl_paths(report_for_reference(&message), Some(&entry));
    }
    if let Some((name, message)) = planexpo_elm::source_error::module_message(message) {
        let mut result = report(&message);
        if let Some(errors) = result["errors"].as_array_mut() {
            for error in errors {
                error["name"] = Value::String(name.clone());
            }
        }
        return result;
    }
    if let Some(messages) = planexpo_elm::source_error::batch_messages(message) {
        let mut errors: Vec<Value> = Vec::new();
        for message in messages {
            let item = report(&message);
            let Some(modules) = item["errors"].as_array() else {
                return item;
            };
            for module in modules {
                if let Some(existing) = errors.iter_mut().find(|existing| {
                    existing["path"] == module["path"] && existing["name"] == module["name"]
                }) {
                    if let (Some(target), Some(problems)) = (
                        existing["problems"].as_array_mut(),
                        module["problems"].as_array(),
                    ) {
                        target.extend(problems.iter().cloned());
                    }
                } else {
                    errors.push(module.clone());
                }
            }
        }
        planexpo_elm::edition::sort_module_reports(&mut errors);
        return json!({"type":"compile-errors", "errors":errors});
    }
    if let Some(report) = planexpo_elm::docs_diagnostic::report_encoded(message) {
        return report;
    }
    if let Some(report) = planexpo_elm::dependency_error::report_encoded(message) {
        return report;
    }
    if let Some(report) = planexpo_elm::outline::report_encoded(message) {
        return report;
    }
    for (offset, _) in message.match_indices(".elm:") {
        let path = &message[..offset + 4];
        // Sources that are not UTF-8 are reported against their lossy text.
        let Ok(source) = fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned()) else {
            continue;
        };
        let detail = message[offset + 5..].trim_start();
        let mut parts = detail.splitn(3, ':');
        let line = parts.next().and_then(|s| s.parse::<usize>().ok());
        let column = parts.next().and_then(|s| s.parse::<usize>().ok());
        let explanation = parts.next();
        let mut end_position = None;
        let explanation = explanation.map(|text| {
            if planexpo_elm::source_error::is_located(detail) {
                let mut rest = text.splitn(3, ':');
                let end_line = rest.next().and_then(|s| s.parse::<usize>().ok());
                let end_column = rest.next().and_then(|s| s.parse::<usize>().ok());
                if let (Some(l), Some(c), Some(message)) = (end_line, end_column, rest.next())
                    && l > 0
                    && c > 0
                    && source
                        .split('\n')
                        .nth(l - 1)
                        .is_some_and(|s| c <= s.chars().count() + 1)
                    && (l, c) >= (line.unwrap_or(0), column.unwrap_or(0))
                {
                    end_position = Some(json!({"line":l,"column":c}));
                    return message;
                }
            }
            text
        });
        if let (Some(line), Some(column), Some(explanation)) = (line, column, explanation)
            && line > 0
            && column > 0
            && source
                .split('\n')
                .nth(line - 1)
                .is_some_and(|s| column <= s.chars().count() + 1)
        {
            let (tokens, _) = planexpo_elm::lexer::lex_prefix(&source);
            let name = planexpo_elm::module::header(&source, &tokens)
                .map(|header| header.name)
                .unwrap_or_else(|_| {
                    Path::new(path)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                });
            if let Some(report) = planexpo_elm::declaration_diagnostic::unfinished_definition(
                &source,
                &name,
                Path::new(path),
                explanation.trim_start(),
            ) {
                return report;
            }
            if let Some(end) = &end_position
                && let Some(report) = planexpo_elm::name_diagnostic::report(
                    &source,
                    &name,
                    Path::new(path),
                    (line, column),
                    (
                        end["line"].as_u64().unwrap_or(line as u64) as usize,
                        end["column"].as_u64().unwrap_or(column as u64) as usize,
                    ),
                    explanation.trim_start(),
                )
            {
                return report;
            }
            if let Some(report) = planexpo_elm::space_diagnostic::report(
                &source,
                &name,
                Path::new(path),
                line,
                column,
                explanation.trim_start(),
            ) {
                return report;
            }
            if let Some(report) = planexpo_elm::control_diagnostic::report(
                &source,
                &name,
                Path::new(path),
                line,
                column,
                explanation.trim_start(),
            ) {
                return report;
            }
            if let Some(report) = planexpo_elm::literal_diagnostic::report(
                &source,
                &name,
                Path::new(path),
                line,
                column,
                explanation.trim_start(),
            ) {
                return report;
            }
            if let Some(report) = planexpo_elm::number_diagnostic::report(
                &source,
                &name,
                Path::new(path),
                line,
                column,
                explanation.trim_start(),
            ) {
                return report;
            }
            if let Some(report) = planexpo_elm::header_diagnostic::report(
                &source,
                &name,
                Path::new(path),
                line,
                column,
                explanation.trim_start(),
            ) {
                return report;
            }
            if let Some(report) = planexpo_elm::pattern_diagnostic::report(
                &source,
                &name,
                Path::new(path),
                line,
                column,
                explanation.trim_start(),
            ) {
                return report;
            }
            if explanation.trim_start().starts_with("declaration start;")
                && let Some(report) = planexpo_elm::declaration_diagnostic::report(
                    &source,
                    &name,
                    Path::new(path),
                    line,
                    column,
                )
            {
                return report;
            }
            // A point region is valid for EOF and avoids guessing token length.
            let position = json!({"line":line,"column":column});
            return json!({"type":"compile-errors","errors":[{
                "path":path,"name":name,"problems":[{
                    "title":title(explanation),
                    "region":{"start":position,"end":end_position.unwrap_or_else(|| position.clone())},
                    "message":[explanation.trim_start()]
                }]
            }]});
        }
        return json!({"type":"error","path":path,"title":"RUST COMPILER ERROR","message":[detail]});
    }
    json!({"type":"error","path":null,"title":"RUST COMPILER ERROR","message":[message]})
}

fn title(message: &str) -> &str {
    let message = message.trim_start();
    if message.starts_with("unknown name ") {
        return "NAMING ERROR";
    }
    if message.starts_with("shadowing local name ") {
        return "SHADOWING";
    }
    if message.starts_with("ambiguous name ") {
        return "AMBIGUOUS NAME";
    }
    if [
        "duplicate record field ",
        "duplicate local name ",
        "duplicate declaration ",
        "duplicate type parameter ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
    {
        return "NAME CLASH";
    }
    if message == "tuples can have at most 3 entries" {
        return "BAD TUPLE";
    }
    if (message.starts_with("type ") || message.starts_with("constructor pattern "))
        && let Some((_, counts)) = message.rsplit_once(" expects ")
        && let Some((expected, actual)) = counts.split_once(" arguments, got ")
        && let (Ok(expected), Ok(actual)) = (expected.parse::<usize>(), actual.parse::<usize>())
    {
        return if actual < expected {
            "TOO FEW ARGS"
        } else {
            "TOO MANY ARGS"
        };
    }
    if message.contains("invalid GLSL:") {
        "SHADER PROBLEM"
    } else if message.contains("Cannot unify these types:") {
        "TYPE MISMATCH"
    } else {
        "RUST COMPILER ERROR"
    }
}

pub fn terminal(message: &str) -> String {
    let text = if let Some((entry, message)) = repl_message(message) {
        terminal_context(&message, Some(&entry))
    } else {
        terminal_context(message, None)
    };
    planexpo_elm::edition::localize_links(&text).into_owned()
}
fn terminal_context(message: &str, entry: Option<&Path>) -> String {
    if let Some((_, message)) = planexpo_elm::source_error::module_message(message) {
        return terminal_context(&message, entry);
    }
    if let Some(messages) = planexpo_elm::source_error::batch_messages(message) {
        let combined = report(message);
        if combined["errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty())
        {
            return planexpo_elm::docs_diagnostic::terminal(&repl_paths(combined, entry));
        }
        return messages
            .iter()
            .map(|message| terminal_context(message, entry))
            .collect::<Vec<_>>()
            .join("\n\n");
    }
    if let Some(report) = planexpo_elm::docs_diagnostic::report_encoded(message) {
        return planexpo_elm::docs_diagnostic::terminal(&repl_paths(report, entry));
    }
    if let Some(rendered) = planexpo_elm::dependency_error::terminal_encoded(message) {
        return rendered;
    }
    if let Some(report) = planexpo_elm::outline::report_encoded(message) {
        return planexpo_elm::dependency_error::terminal_report(&report)
            .unwrap_or_else(|| message.to_owned());
    }
    let report = report(message);
    if planexpo_elm::name_diagnostic::has_details(message) {
        return planexpo_elm::docs_diagnostic::terminal(&repl_paths(report, entry));
    }
    if matches!(
        report["errors"][0]["problems"][0]["title"].as_str(),
        Some(
            "EXPECTING MODULE NAME"
                | "MODULE NAME MISSING"
                | "UNFINISHED MODULE DECLARATION"
                | "UNFINISHED PORT MODULE DECLARATION"
                | "UNFINISHED EXPOSING"
                | "PROBLEM EXPOSING CUSTOM TYPE VARIANTS"
                | "PROBLEM IN EXPOSING"
                | "RESERVED SYMBOL"
                | "RESERVED WORD"
                | "UNEXPECTED SYMBOL"
                | "EXPECTING IMPORT NAME"
                | "EXPECTING IMPORT ALIAS"
                | "UNFINISHED IMPORT"
                | "WEIRD DECLARATION"
                | "STRAY PARENTHESIS"
                | "STRAY SQUARE BRACKET"
                | "STRAY CURLY BRACE"
                | "UNEXPECTED CAPITAL LETTER"
                | "UNEXPECTED PATTERN"
                | "UNFINISHED PATTERN"
                | "UNEXPECTED NAME"
                | "UNFINISHED RECORD PATTERN"
                | "UNFINISHED LIST PATTERN"
                | "PROBLEM IN PATTERN"
                | "UNFINISHED TUPLE PATTERN"
                | "UNFINISHED PARENTHESES"
                | "UNFINISHED TUPLE"
                | "UNFINISHED OPERATOR FUNCTION"
                | "EXPECTING RECORD ACCESSOR"
                | "UNFINISHED LIST"
                | "UNFINISHED RECORD"
                | "NEED MORE INDENTATION"
                | "PROBLEM IN RECORD"
                | "EXTRA COMMA"
                | "MISSING ARGUMENT"
                | "UNFINISHED ANONYMOUS FUNCTION"
                | "UNFINISHED LET"
                | "UNFINISHED DEFINITION"
                | "MISSING EXPRESSION"
                | "EXPECTING DEFINITION"
                | "NAME MISMATCH"
                | "PROBLEM IN DEFINITION"
                | "MISSING COLON?"
                | "LET PROBLEM"
                | "ENDLESS STRING"
                | "MISSING SINGLE QUOTE"
                | "NEEDS DOUBLE QUOTES"
                | "UNKNOWN ESCAPE"
                | "BAD UNICODE ESCAPE"
                | "WEIRD NUMBER"
                | "WEIRD HEXIDECIMAL"
                | "LEADING ZEROS"
                | "MISSING ARROW"
                | "UNFINISHED CASE"
                | "UNFINISHED IF"
                | "WEIRD ELSE BRANCH"
                | "UNEXPECTED OPERATOR"
                | "BAD INFIX"
                | "BAD MODULE DECLARATION"
                | "NO TABS"
                | "ENDLESS COMMENT"
        )
    ) {
        return planexpo_elm::docs_diagnostic::terminal(&repl_paths(report, entry));
    }
    let Some(error) = report["errors"]
        .as_array()
        .and_then(|errors| errors.first())
    else {
        return message.to_owned();
    };
    let problem = &error["problems"][0];
    let path = error["path"].as_str().unwrap_or("");
    let start = &problem["region"]["start"];
    let end = &problem["region"]["end"];
    let line = start["line"].as_u64().unwrap_or(1) as usize;
    let column = start["column"].as_u64().unwrap_or(1) as usize;
    let end_line = end["line"].as_u64().unwrap_or(line as u64) as usize;
    let end_column = end["column"].as_u64().unwrap_or(column as u64) as usize;
    let display_path = if entry.is_some_and(|entry| entry == Path::new(path)) {
        "REPL"
    } else {
        path
    };
    let mut out = format!(
        "-- {} -- {display_path}:{line}:{column}\n\n",
        problem["title"].as_str().unwrap_or("COMPILER ERROR")
    );
    if let Ok(source) = fs::read_to_string(path) {
        for (index, text) in source.split('\n').enumerate().skip(line - 1).take(5) {
            if index + 1 > end_line {
                break;
            }
            let text: String = text.chars().take(240).collect();
            out.push_str(&format!("{:>4} | {text}\n", index + 1));
            if index + 1 == line {
                let width = if line == end_line {
                    end_column.saturating_sub(column)
                } else {
                    text.chars().count().saturating_sub(column - 1)
                };
                out.push_str(&format!(
                    "     | {}{}\n",
                    " ".repeat((column - 1).min(240)),
                    "^".repeat(width.clamp(1, 80))
                ));
            }
        }
        if end_line >= line + 5 {
            out.push_str("     | …\n");
        }
    }
    out.push('\n');
    out.push_str(problem["message"][0].as_str().unwrap_or(message));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repl_preserves_complete_cli_errors() {
        let message = "ELM_CLI_RAW:elm: invalid proxy\n";
        assert_eq!(repl_error(Path::new("Elm_Repl.elm"), message.into()), message);
    }

    #[test]
    fn batched_problems_in_one_module_have_one_blank_separator() {
        let messages = ["First problem", "Second problem"].map(|message| {
            planexpo_elm::docs_diagnostic::encode(&json!({
                "type": "compile-errors",
                "errors": [{"path": "Main.elm", "name": "Main", "problems": [{
                    "title": "NAME CLASH", "message": [message]
                }]}]
            }))
        });
        let rendered = terminal(&planexpo_elm::source_error::batch(messages.into()));
        assert!(rendered.contains("First problem\n\n-- NAME CLASH"));
        assert!(rendered.ends_with("Second problem\n"));
    }

    #[test]
    fn non_source_errors_keep_their_original_message() {
        let message = "unsupported option --output=missing.elm:foo";
        let result = report(message);
        assert_eq!(result["type"], "error");
        assert!(result["path"].is_null());
        assert_eq!(result["message"][0], message);
    }

    #[test]
    fn unlocated_errors_keep_the_file_without_inventing_a_region() {
        let path = std::env::temp_dir().join(format!("elm-report-{}.elm", std::process::id()));
        fs::write(&path, "bad = ()\n").unwrap();
        for detail in [
            "bad: incompatible types",
            "0:0: invalid coordinates",
            "99:1: beyond EOF",
        ] {
            let result = report(&format!("{}: {detail}", path.display()));
            assert_eq!(result["type"], "error");
            assert_eq!(result["path"], path.to_str().unwrap());
            assert_eq!(result["message"][0], detail);
        }
        fs::remove_file(path).unwrap();
    }
}
