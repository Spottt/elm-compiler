//! Package comparison reports from Elm 0.19.1 Reporting.Exit (LICENSE-ELM).
use crate::{dependency_error::reflow, package_solver::Version};
use serde_json::{Value, json};
fn color(name: &str, text: impl Into<String>) -> Value {
    json!({"bold":false,"underline":false,"color":name,"string":text.into()})
}
fn report(title: &str, path: Option<&str>, message: Vec<Value>) -> String {
    format!("ELM_DEPENDENCY_JSON:{}", json!({"type":"error","path":path,"title":title,"message":message}))
}
pub fn no_outline() -> String {
    report("DIFF WHAT?", None, vec![
        json!(format!("{}\n\nIf you are just curious to see a diff, try running this command:\n\n    ", reflow("I cannot find an elm.json so I am not sure what you want me to diff. Normally you run `elm diff` from within a project!"))),
        color("GREEN", "elm diff elm/http 1.0.0 2.0.0"),
    ])
}
pub fn application() -> String {
    report("CANNOT DIFF APPLICATIONS", Some("elm.json"), vec![
        json!(format!("{}\n\nIf you are just curious to see a diff, try running this command:\n\n    ", reflow("Your elm.json says this project is an application, but `elm diff` only works with packages. That way there are previously published versions of the API to diff against!"))),
        color("yellow", "elm diff elm/json 1.0.0 1.1.2"),
    ])
}
pub fn no_exposed() -> String {
    report("NO EXPOSED MODULES", Some("elm.json"), vec![json!(format!("{}\n\nTry adding some modules back to the \"exposed-modules\" field.", reflow("Your elm.json has no \"exposed-modules\" which means there is no public API at all right now! What am I supposed to diff?")))])
}
pub fn unpublished() -> String {
    report("UNPUBLISHED", None, vec![json!("This package is not published yet. There is nothing to diff against!")])
}
pub fn unknown_package(name: &str, suggestions: &[String]) -> String {
    report("UNKNOWN PACKAGE", None, vec![
        json!("I cannot find a package called:\n\n    "), color("RED", name),
        json!("\n\nMaybe you want one of these instead?\n\n    "),
        color("yellow", suggestions.join("\n    ")),
        json!("\n\nBut check <https://package.elm-lang.org> to see all possibilities!"),
    ])
}
pub fn unknown_version(version: Version, known: &[Version]) -> String {
    let mut known = known.to_vec();
    known.sort();
    let mut rows = String::new();
    let mut previous = None;
    for version in known {
        if let Some(major) = previous {
            rows.push_str(if major == version.0[0] { " " } else { "\n    " });
        }
        rows.push_str(&version.to_string());
        previous = Some(version.0[0]);
    }
    report("UNKNOWN VERSION", None, vec![
        json!("Version "), color("RED", version.to_string()),
        json!(" has never been published, so I cannot diff against it.\n\nHere are all the versions that HAVE been published:\n\n    "),
        color("yellow", rows), json!("\n\nWant one of those instead?"),
    ])
}

pub fn documentation(version: Version, error: crate::package_network::DocumentationError) -> String {
    documentation_context(&format!("I need the docs for {version} to compute this diff"), error)
}
pub fn documentation_context(context: &str, error: crate::package_network::DocumentationError) -> String {
    use crate::package_network::DocumentationError;
    match error {
        DocumentationError::CorruptCache => report("PROBLEM LOADING DOCS", None, vec![
            json!(format!("{}\n\n{}", reflow(&format!("{context}, but the local copy seems to be corrupted.")), reflow("I deleted the cached version, so the next run should download a fresh copy of the docs. Hopefully that will get you unstuck, but it will not resolve the root problem if, for example, a 3rd party editor plugin is modifing cached files for some reason."))),
        ]),
        DocumentationError::InvalidData { url, body } => invalid_response("PROBLEM LOADING DOCS", context, &url, &body),
        DocumentationError::Other(message) => message,
    }
}

/// Preserve the Rust transport's actual detail. Haskell-specific exception
/// formatting still needs a transport-independent error classification.
pub fn registry(url: &str, detail: &str) -> String {
    registry_context("I need the latest list of published packages before I do this diff", url, detail)
}
pub fn registry_context(context: &str, url: &str, detail: &str) -> String {
    let detail = detail.strip_prefix(&format!("{url}: ")).unwrap_or(detail);
    report("PROBLEM UPDATING PACKAGE LIST", None, vec![
        json!(format!("{}\n\n    ", reflow(&format!("{context}, so I tried to fetch:")))),
        color("yellow", url),
        json!(format!("\n\nBut my HTTP library is giving me the following error message:\n\n    {detail}\n\n{}", reflow("Are you somewhere with a slow internet connection? Or no internet? Does the link I am trying to fetch work in your browser? Maybe the site is down? Does your internet connection have a firewall that blocks certain domains? It is usually something like that!"))),
    ])
}

pub fn registry_data(url: &str, body: &[u8]) -> String {
    invalid_response("PROBLEM UPDATING PACKAGE LIST", "I need the latest list of published packages before I do this diff", url, body)
}
pub(crate) fn invalid_response(title: &str, context: &str, url: &str, body: &[u8]) -> String {
            let short = body.len() <= 76;
            let text = String::from_utf8_lossy(body);
            let sample = if short { text.into_owned() } else { format!("{}...", text.chars().take(73).collect::<String>()) };
            report(title, None, vec![
                json!(format!("{}\n\n    ", reflow(&format!("{context}, so I fetched:")))),
                color("yellow", url),
                json!(format!("\n\n{}\n\n    ", reflow(&format!("I got the data back, but it was not what I was expecting. The response body contains {} bytes. Here is the {}:", body.len(), if short { "whole thing" } else { "beginning" })))),
                color("yellow", sample),
                json!(format!("\n\n{}", reflow("Does this error keep showing up? Maybe there is something weird with your internet connection. We have gotten reports that schools, businesses, airports, etc. sometimes intercept requests and add things to the body or change its contents entirely. Could that be the problem?"))),
            ])
}
