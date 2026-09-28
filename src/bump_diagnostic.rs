//! Reports from Elm 0.19.1 Reporting.Exit.bumpToReport (LICENSE-ELM).
use crate::{dependency_error::reflow, package_solver::Version};
use serde_json::{Value, json};
fn report(title: &str, path: Option<&str>, message: Vec<Value>) -> String {
    format!(
        "ELM_DEPENDENCY_JSON:{}",
        json!({"type":"error","path":path,"title":title,"message":message})
    )
}
fn colored(color: &str, text: impl Into<String>) -> Value {
    json!({"bold":false,"underline":false,"color":color,"string":text.into()})
}
fn paragraph(text: &str, highlight: &str, color: &str) -> Vec<Value> {
    let text = reflow(text);
    match text.split_once(highlight) {
        Some((before, after)) => vec![json!(before), colored(color, highlight), json!(after)],
        None => vec![json!(text)],
    }
}
pub fn no_outline() -> String {
    report(
        "BUMP WHAT?",
        None,
        vec![json!(format!(
            "I cannot find an elm.json so I am not sure what you want me to bump.\n\n{}",
            reflow(
                "Elm packages always have an elm.json that says current the version number. If you run this command from a directory with an elm.json file, I will try to bump the version in there based on the API changes."
            )
        ))],
    )
}
pub fn application() -> String {
    report(
        "CANNOT BUMP APPLICATIONS",
        Some("elm.json"),
        vec![json!(reflow(
            "Your elm.json says this is an application. That means it cannot be published on <https://package.elm-lang.org> and therefore has no version to bump!"
        ))],
    )
}
pub fn no_exposed() -> String {
    let mut message = paragraph(
        "To bump a package, the \"exposed-modules\" field of your elm.json must list at least one module.",
        "\"exposed-modules\"",
        "yellow",
    );
    message.push(json!(
        "\n\nTry adding some modules back to the \"exposed-modules\" field."
    ));
    report("NO EXPOSED MODULES", Some("elm.json"), message)
}
pub fn unexpected_version(version: Version, allowed: &[Version]) -> String {
    let mut message = paragraph(
        &format!(
            "Your elm.json says I should bump relative to version {version}, but I cannot find that version on <https://package.elm-lang.org>. That means there is no API for me to diff against and figure out if these are MAJOR, MINOR, or PATCH changes."
        ),
        &version.to_string(),
        "RED",
    );
    message.push(json!("\n\n"));
    message.extend(paragraph(
        &format!(
            "Try bumping again after changing the \"version\" in elm.json {}",
            if allowed.len() == 1 {
                "to:"
            } else {
                "to one of these:"
            }
        ),
        "\"version\"",
        "yellow",
    ));
    message.push(json!("\n\n"));
    for (i, version) in allowed.iter().enumerate() {
        if i > 0 {
            message.push(json!("\n"));
        }
        message.push(colored("GREEN", version.to_string()));
    }
    report("CANNOT BUMP", Some("elm.json"), message)
}
pub const NEW_PACKAGE: &str = "This package has never been published before. Here's how things work:\n\n  - Versions all have exactly three parts: MAJOR.MINOR.PATCH\n\n  - All packages start with initial version 1.0.0\n\n  - Versions are incremented based on how the API changes:\n\n        PATCH = the API is the same, no risk of breaking code\n        MINOR = values have been added, existing values are unchanged\n        MAJOR = existing values have been changed or removed\n\n  - I will bump versions for you, automatically enforcing these rules\n\n";
