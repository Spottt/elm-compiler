//! Read-only probe for comparing application install plans with Elm 0.19.1.
use std::{env, fs, path::Path};
fn main() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    let [home, manifest, package] = args.as_slice() else {
        return Err("expected ELM_HOME, application elm.json, and package name".into());
    };
    let application = serde_json::from_slice(&fs::read(manifest).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let result = planexpo_elm::installation::plan(
        Path::new(home),
        &application,
        package,
        None,
    )?;
    println!("{}", result.outline);
    Ok(())
}
