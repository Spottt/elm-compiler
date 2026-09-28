//! Internal probe of publication checks before building documentation.
use planexpo_elm::package_publish::{self as publish, Problem};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::current_dir()?;
    let outline = serde_json::from_slice(&std::fs::read(root.join("elm.json"))?)?;
    let result = publish::check_description(&outline)
        .and_then(|()| publish::check_readme(&root))
        .and_then(|()| publish::check_license(&root));
    if let Err(ref problem) = result {
        let report =
            planexpo_elm::publish_diagnostic::problem(problem).ok_or("unhandled report")?;
        eprintln!(
            "{}",
            planexpo_elm::dependency_error::terminal_encoded(&report).ok_or("invalid report")?
        );
    }
    println!(
        "{}",
        match result {
            Err(Problem::Application) => "UNPUBLISHABLE",
            Err(Problem::NoExposedModules) => "NO EXPOSED MODULES",
            Err(Problem::NoSummary) => "NO SUMMARY",
            Err(Problem::NoReadme) => "NO README",
            Err(Problem::ShortReadme) => "SHORT README",
            Err(Problem::NoLicense) => "NO LICENSE FILE",
            Ok(()) => "READY TO BUILD",
            Err(problem) => return Err(format!("{problem:?}").into()),
        }
    );
    Ok(())
}
