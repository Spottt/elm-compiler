//! Dependency diagnostics ported from Elm 0.19.1 Reporting.Exit (LICENSE-ELM).
use crate::package_solver::Version;
use serde_json::{Value, json};
use std::io::IsTerminal;
const PREFIX: &str = "ELM_DEPENDENCY_JSON:";

pub(crate) fn bad_cache(name: &str, version: Version) -> String {
    let first = reflow(&format!(
        "I need the elm.json of {name} {version} to help me search for a set of compatible packages. I had it cached locally, but it looks like the file was corrupted!"
    ));
    let second = reflow(
        "I deleted the cached version, so the next run should download a fresh copy. Hopefully that will get you unstuck, but it will not resolve the root problem if a 3rd party tool is modifing cached files for some reason.",
    );
    let report = json!({"type":"error","path":null,"title":"PROBLEM SOLVING PACKAGE CONSTRAINTS",
                        "message":[format!("{first}\n\n{second}")]});
    format!("{PREFIX}{report}")
}

pub fn report_encoded(message: &str) -> Option<Value> {
    message
        .strip_prefix(PREFIX)
        .and_then(|text| serde_json::from_str(text).ok())
}

pub fn terminal_encoded(message: &str) -> Option<String> {
    terminal_report(&report_encoded(message)?)
}

pub fn terminal_report(report: &Value) -> Option<String> {
    let ansi = std::io::stderr().is_terminal();
    let title = report["title"].as_str()?;
    let mut body = String::new();
    for chunk in report["message"].as_array()? {
        let content = chunk.as_str().or_else(|| chunk["string"].as_str())?;
        let mut styles = Vec::new();
        if ansi {
            if chunk["bold"].as_bool() == Some(true) {
                styles.push("1");
            }
            if chunk["underline"].as_bool() == Some(true) {
                styles.push("4");
            }
            if let Some(color) = chunk["color"].as_str()
                && let Some(code) = match color {
                    "BLACK" => Some("90"),
                    "RED" => Some("91"),
                    "GREEN" => Some("92"),
                    "YELLOW" => Some("93"),
                    "BLUE" => Some("94"),
                    "MAGENTA" => Some("95"),
                    "CYAN" => Some("96"),
                    "WHITE" => Some("97"),
                    "black" => Some("30"),
                    "red" => Some("31"),
                    "green" => Some("32"),
                    "yellow" => Some("33"),
                    "blue" => Some("34"),
                    "magenta" => Some("35"),
                    "cyan" => Some("36"),
                    "white" => Some("37"),
                    _ => None,
                }
            {
                styles.push(code);
            }
        }
        if !styles.is_empty() {
            body.push_str(&format!("\x1b[{}m", styles.join(";")));
        }
        body.push_str(content);
        if !styles.is_empty() {
            body.push_str("\x1b[0m");
        }
    }
    let ending = if let Some(path) = report["path"].as_str() {
        format!(
            "{} {path}",
            "-".repeat(80usize.saturating_sub(5 + title.len() + path.len()).max(1))
        )
    } else {
        "-".repeat(80usize.saturating_sub(4 + title.len()).max(1))
    };
    let header = format!("-- {title} {ending}");
    let header = if ansi {
        format!("\x1b[36m{header}\x1b[0m")
    } else {
        header
    };
    Some(format!("{header}\n\n{body}\n"))
}

pub(crate) fn reflow(paragraph: &str) -> String {
    let mut result = String::new();
    let mut column = 0;
    for word in paragraph.split_whitespace() {
        let width = word.chars().count();
        if column > 0 {
            if column + 1 + width > 80 {
                result.push('\n');
                column = 0;
            } else {
                result.push(' ');
                column += 1;
            }
        }
        result.push_str(word);
        column += width;
    }
    result
}

pub(crate) fn is_bad_cache(error: &str) -> bool {
    report_encoded(error)
        .is_some_and(|report| report["title"] == "PROBLEM SOLVING PACKAGE CONSTRAINTS")
}

pub(crate) fn no_solution(online: bool) -> String {
    if online {
        styled_report(
            "INCOMPATIBLE DEPENDENCIES",
            &[
                "The dependencies in your elm.json are not compatible.",
                "Did you change them by hand? Try to change it back! It is much more reliable to add dependencies with elm install or the dependency management tool in elm reactor.",
                "Please ask for help on the community forums if you try those paths and are still having problems!",
            ],
        )
    } else {
        styled_report(
            "TROUBLE VERIFYING DEPENDENCIES",
            &[
                "I could not connect to https://package.elm-lang.org to get the latest list of packages, and I was unable to verify your dependencies with the information I have cached locally.",
                "Are you able to connect to the internet? These dependencies may work once you get access to the registry!",
                "Note: If you changed your dependencies by hand, try to change them back! It is much more reliable to add dependencies with elm install or the dependency management tool in elm reactor.",
            ],
        )
    }
}

pub(crate) fn hand_edited() -> String {
    styled_report(
        "ERROR IN DEPENDENCIES",
        &[
            "It looks like the dependencies elm.json in were edited by hand (or by a 3rd party tool) leaving them in an invalid state.",
            "Try to change them back to what they were before! It is much more reliable to add dependencies with elm install or the dependency management tool in elm reactor.",
            "Please ask for help on the community forums if you try those paths and are still having problems!",
        ],
    )
}

fn styled_report(title: &str, paragraphs: &[&str]) -> String {
    // Pretty-print's colored command documents are indivisible during fillSep.
    let text = paragraphs
        .iter()
        .map(|paragraph| {
            reflow(
                &paragraph
                    .replace("elm install", "elm_install")
                    .replace("elm reactor", "elm_reactor"),
            )
            .replace("elm_install", "elm install")
            .replace("elm_reactor", "elm reactor")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut chunks = Vec::new();
    let mut rest = text.as_str();
    let styles = [
        ("elm install", false),
        ("elm reactor", false),
        ("Note", true),
    ];
    while let Some((position, word, underline)) = styles
        .iter()
        .filter_map(|(word, underline)| {
            rest.find(word)
                .map(|position| (position, *word, *underline))
        })
        .min_by_key(|(position, _, _)| *position)
    {
        chunks.push(json!(&rest[..position]));
        chunks.push(json!({"bold":false,"underline":underline,"color":if underline {Value::Null} else {json!("GREEN")},"string":word}));
        rest = &rest[position + word.len()..];
    }
    chunks.push(json!(rest));
    let report = json!({"type":"error","path":"elm.json","title":title,"message":chunks});
    format!("{PREFIX}{report}")
}

pub(crate) fn missing_required_package(core: bool, package: bool) -> String {
    let intro = if core {
        "I need to see an \"elm/core\" dependency your elm.json file. The default imports of `List` and `Maybe` do not work without it."
    } else {
        "I need to see an \"elm/json\" dependency your elm.json file. It helps me handle flags and ports."
    };
    let advice = if package {
        "If you modified your elm.json by hand, try to change it back! And if you are having trouble getting back to a working elm.json, it may be easier to find a working package and start fresh with their elm.json file."
    } else {
        "If you modified your elm.json by hand, try to change it back! And if you are having trouble getting back to a working elm.json, it may be easier to delete it and use `elm init` to start fresh."
    };
    format!("{}\n\n{}", reflow(intro), reflow(advice))
}

pub(crate) fn application_version(required: Version) -> String {
    version_report(vec![
        json!(
            "Your elm.json says this application needs a different version of Elm.\n\nIt requires "
        ),
        version_color("GREEN", required.to_string()),
        json!(", but you are using "),
        version_color("RED", Version::ELM.to_string()),
        json!(" right now."),
    ])
}

pub(crate) fn package_version(required: crate::package_solver::Constraint) -> String {
    version_report(vec![
        json!("Your elm.json says this package needs a version of Elm in this range:\n\n    "),
        version_color("yellow", required.to_string()),
        json!("\n\nBut you are using Elm "),
        version_color("RED", Version::ELM.to_string()),
        json!(" right now."),
    ])
}

fn version_color(color: &str, string: String) -> Value {
    json!({"bold":false,"underline":false,"color":color,"string":string})
}

fn version_report(message: Vec<Value>) -> String {
    let report =
        json!({"type":"error","path":"elm.json","title":"ELM VERSION MISMATCH","message":message});
    format!("{PREFIX}{report}")
}

pub(crate) fn registry_problem(detail: &str) -> String {
    let message = format!(
        "{}\n\n{detail}",
        reflow(
            "I need the list of published packages to verify your dependencies, but I could not load it."
        )
    );
    let report = json!({"type":"error","path":null,"title":"PROBLEM LOADING PACKAGE LIST","message":[message]});
    format!("{PREFIX}{report}")
}
