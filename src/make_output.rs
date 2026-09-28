//! Output diagnostics from Elm 0.19.1 Reporting.Exit (BSD-3-Clause).
//! Static examples/styles are kept verbatim; module names are laid out dynamically.
use serde_json::{Value, json};

pub fn error(html: bool, missing: &[String], entry_count: usize) -> String {
    let templates: Value = serde_json::from_str(include_str!("make_output_messages.json"))
        .expect("checked output diagnostic templates");
    let key = if html && entry_count > 1 {
        "multiple-html"
    } else if html {
        "no-main-html"
    } else if missing.len() > 1 {
        "multiple-no-main"
    } else {
        "no-main-js"
    };
    let mut report = templates[key].clone();
    if !html {
        let name = missing
            .first()
            .expect("missing main diagnostic needs a module");
        if missing.len() > 1 {
            let intro = crate::docs_diagnostic::reflow(&format!(
                "When producing a JS file, I require that given files all have `main` values. That way functions like Elm.{name}.init() are definitely defined in the resulting file. I am missing `main` values in:"
            ));
            report["message"][0] = json!(format!("{intro}\n\n    "));
            report["message"][1]["string"] = json!(missing.join("\n    "));
        } else {
            let intro = crate::docs_diagnostic::reflow(&format!(
                "When producing a JS file, I require that the given file has a `main` value. That way Elm.{name}.init() is definitely defined in the resulting file!"
            ));
            let advice = crate::docs_diagnostic::reflow(
                "Try adding a `main` value to your file? Or if you just want to verify that this module compiles, switch to --output=/dev/null to skip the code gen phase altogether.",
            );
            report["message"][0] = json!(format!("{intro}\n\n{advice}\n\n"));
        }
    }
    format!("ELM_DEPENDENCY_JSON:{report}")
}

/// Generation rejects Debug only after all source errors have been checked.
pub fn debug_remnants(modules: &std::collections::BTreeSet<String>) -> String {
    let templates: Value = serde_json::from_str(include_str!("make_output_messages.json"))
        .expect("checked output diagnostic templates");
    let mut report = templates["debug-remnants"].clone();
    report["message"][1]["string"] = json!(modules.iter().cloned().collect::<Vec<_>>().join("\n    "));
    format!("ELM_DEPENDENCY_JSON:{report}")
}
