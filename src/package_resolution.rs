//! Project-local dependency validation and version selection, independent from JS caches.
use crate::{
    package_network::{PACKAGE_SERVER, PackageNetwork},
    package_solver,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Write, path::Path};

pub fn resolve(manifest: &Path, home: &Path) -> Result<BTreeMap<String, String>, String> {
    resolve_with(manifest, home, || PackageNetwork::new(PACKAGE_SERVER))
}
pub fn resolve_with(
    manifest: &Path,
    home: &Path,
    network: impl FnOnce() -> Result<PackageNetwork, String>,
) -> Result<BTreeMap<String, String>, String> {
    let source = fs::read(manifest).map_err(|e| e.to_string())?;
    let config = crate::outline::decode(std::str::from_utf8(&source).map_err(|e| e.to_string())?)?;
    let application = validate_project(&config);
    fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let home = home.canonicalize().map_err(|e| e.to_string())?;
    let modified = fs::metadata(manifest)
        .and_then(|m| m.modified())
        .map_err(|e| e.to_string())?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos()
        .to_string();
    let key = json!({"schema":5,"manifest":format!("{:x}",Sha256::digest(&source)),"modified":modified,"home":home});
    let directory = manifest
        .parent()
        .ok_or("missing project directory")?
        .join("elm-stuff/planexpo-rust");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("dependencies.lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let path = directory.join("dependencies-v1.json");
    if let Ok(bytes) = fs::read(&path)
        && let Ok(saved) = serde_json::from_slice::<Value>(&bytes)
        && saved["key"] == key
        && let Ok(selected) =
            serde_json::from_value::<BTreeMap<String, String>>(saved["selected"].clone())
        && let Ok(application) = &application
        && match application {
            Some(pins) => {
                pins.len() == selected.len()
                    && pins
                        .iter()
                        .all(|(name, version)| selected.get(name) == Some(&version.to_string()))
            }
            None => {
                satisfies(&config["dependencies"], &selected)
                    && satisfies(&config["test-dependencies"], &selected)
            }
        }
        && let Ok(fingerprint) = metadata_fingerprint(&home, &selected)
        && saved["metadata"] == fingerprint
    {
        return Ok(selected);
    }
    let network = network()?;
    let state = network.registry(&home)?;
    let selected = if let Some(pins) = application? {
        let actual = package_solver::solve_pinned(&home, &pins, state.online.then_some(&network))?;
        if actual.len() != pins.len() {
            return Err(crate::dependency_error::hand_edited());
        }
        actual
    } else if state.online {
        package_solver::solve_online(
            &home,
            &config["dependencies"],
            &config["test-dependencies"],
            &network,
        )?
    } else {
        package_solver::solve(&home, &config["dependencies"], &config["test-dependencies"])?
    };
    for (name, version) in &selected {
        network.package(&home, name, package_solver::Version::parse(version)?)?;
    }
    let metadata = metadata_fingerprint(&home, &selected)?;
    let bytes = serde_json::to_vec(&json!({"key":key,"selected":selected,"metadata":metadata}))
        .map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(selected)
}
fn metadata_fingerprint(home: &Path, selected: &BTreeMap<String, String>) -> Result<Value, String> {
    let mut fingerprints = BTreeMap::new();
    for (name, version) in selected {
        if !package_solver::valid_name(name) {
            return Err("invalid cached package name".into());
        }
        package_solver::Version::parse(version)?;
        let root = home.join("0.19.1/packages").join(name).join(version);
        if !root.join("src").is_dir() {
            return Err("cached package sources missing".into());
        }
        let bytes = fs::read(root.join("elm.json")).map_err(|e| e.to_string())?;
        fingerprints.insert(name, format!("{:x}", Sha256::digest(bytes)));
    }
    serde_json::to_value(fingerprints).map_err(|e| e.to_string())
}

fn satisfies(dependencies: &Value, selected: &BTreeMap<String, String>) -> bool {
    dependencies.as_object().is_some_and(|deps| {
        deps.iter().all(|(name, constraint)| {
            let Some(version) = selected
                .get(name)
                .and_then(|v| package_solver::Version::parse(v).ok())
            else {
                return false;
            };
            constraint
                .as_str()
                .and_then(|c| package_solver::Constraint::parse(c).ok())
                .is_some_and(|c| c.contains(version))
        })
    })
}

pub(crate) fn validate_project(
    config: &Value,
) -> Result<Option<BTreeMap<String, package_solver::Version>>, String> {
    match config["type"].as_str() {
        Some("application") => {
            let elm = package_solver::Version::parse(
                config["elm-version"]
                    .as_str()
                    .ok_or("missing elm-version")?,
            )?;
            if elm != package_solver::Version::ELM {
                return Err(crate::dependency_error::application_version(elm));
            }
            Ok(Some(crate::dependencies::application_versions(config)?))
        }
        Some("package") => {
            let elm = package_solver::Constraint::parse(
                config["elm-version"]
                    .as_str()
                    .ok_or("missing elm-version")?,
            )?;
            if !elm.contains(package_solver::Version::ELM) {
                return Err(crate::dependency_error::package_version(elm));
            }
            package_solver::validate_dependencies(
                &config["dependencies"],
                &config["test-dependencies"],
            )?;
            Ok(None)
        }
        _ => Err("elm.json type must be application or package".into()),
    }
}
