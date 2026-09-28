//! Internal publication pipeline probe, deliberately stops before GitHub/network publication.
use planexpo_elm::{
    package_network::{PACKAGE_SERVER, PackageNetwork},
    package_publish as publish,
    package_solver::Version,
};
use std::{env, fs, path::PathBuf};
fn run() -> Result<(), String> {
    let root = env::current_dir().map_err(|e| e.to_string())?;
    let home = PathBuf::from(env::var_os("ELM_HOME").ok_or("missing ELM_HOME")?);
    let network = PackageNetwork::new(PACKAGE_SERVER)?;
    let registry = network.latest_registry_with_context(
        &home,
        "I need the latest list of published packages to make sure this is safe to publish",
    )?;
    let manifest = root.join("elm.json");
    let bytes = fs::read_to_string(&manifest).map_err(|e| e.to_string())?;
    let outline = planexpo_elm::outline::decode(&bytes)?;
    planexpo_elm::outline::validate(&outline, &root).map_err(|e| e.encode())?;
    for check in [
        publish::check_description(&outline),
        publish::check_readme(&root),
        publish::check_license(&root),
    ] {
        check.map_err(|e| {
            planexpo_elm::publish_diagnostic::problem(&e).unwrap_or_else(|| format!("{e:?}"))
        })?;
    }
    let docs = publish::build_documentation(&manifest, &home)?;
    let name = outline["name"].as_str().ok_or("missing name")?;
    let version = Version::parse(outline["version"].as_str().ok_or("missing version")?)?;
    publish::verify_version(
        &network,
        &home,
        name,
        version,
        registry.versions(name).unwrap_or(&[]),
        &docs,
    )?;
    let render_git = |error| {
        planexpo_elm::publish_diagnostic::git(&error).unwrap_or_else(|| format!("{error:?}"))
    };
    let git =
        planexpo_elm::publish_git::Git::find(env::var_os("PATH").as_deref()).map_err(render_git)?;
    git.verify_tag(&root, version).map_err(render_git)?;
    println!("LOCAL TAG VERIFIED");
    Ok(())
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            if let Some(report) = planexpo_elm::dependency_error::terminal_encoded(&error) {
                eprintln!("{report}");
            } else {
                eprintln!("{error}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}
