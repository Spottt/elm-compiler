//! Development harness for the project documentation API. Use make --docs
//! to exercise command-line selection and output-file handling.
fn main() -> Result<(), String> {
    let entries = std::env::args()
        .skip(1)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    let home = std::env::var_os("ELM_HOME")
        .map(std::path::PathBuf::from)
        .ok_or("ELM_HOME is required")?;
    let graph = planexpo_elm::project::discover_many_for_check(
        std::path::Path::new("elm.json"),
        &entries,
        &home,
    )?;
    let report = planexpo_elm::analyze::documentation(&graph)?;
    println!(
        "{}",
        serde_json::to_string(&report.documentation).map_err(|e| e.to_string())?
    );
    Ok(())
}
