//! Acquire missing dependency sources before the read-only module discovery.
use crate::{
    package_network::{PACKAGE_SERVER, PackageNetwork},
    package_solver::{self, Version, valid_name},
};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

pub fn prepare(config: &Value, home: &Path) -> Result<(), String> {
    prepare_with(config, home, || PackageNetwork::new(PACKAGE_SERVER))
}
/// Factory injection keeps tests independent from the public package service.
pub fn prepare_with(
    config: &Value,
    home: &Path,
    network: impl FnOnce() -> Result<PackageNetwork, String>,
) -> Result<(), String> {
    match config["type"].as_str() {
        Some("application") => {
            let selected = application_versions(config)?;
            if selected
                .iter()
                .all(|(name, version)| cached(home, name, *version))
            {
                return verify_application(home, &selected);
            }
            let network = network()?;
            let state = network.registry(home)?;
            for (name, version) in &selected {
                if !state
                    .registry
                    .versions(name)
                    .is_some_and(|versions| versions.contains(version))
                {
                    return Err(format!(
                        "package {name}@{version} is not in the package registry"
                    ));
                }
            }
            for (name, version) in &selected {
                if !cached(home, name, *version) {
                    if !state.online {
                        return Err(format!(
                            "package {name}@{version} is unavailable in the offline cache"
                        ));
                    }
                    network.package(home, name, *version)?;
                }
            }
            verify_application(home, &selected)
        }
        Some("package") => {
            package_solver::validate_dependencies(
                &config["dependencies"],
                &config["test-dependencies"],
            )?;
            match package_solver::solve(home, &config["dependencies"], &config["test-dependencies"])
            {
                Ok(_) => return Ok(()),
                Err(error) if crate::dependency_error::is_bad_cache(&error) => {
                    return Err(error);
                }
                Err(_) => (),
            }
            let network = network()?;
            let state = network.registry(home)?;
            if !state.online {
                return Err("no solution using locally cached Elm packages".into());
            }
            let selected = package_solver::solve_online(
                home,
                &config["dependencies"],
                &config["test-dependencies"],
                &network,
            )?;
            for (name, version) in selected {
                network.package(home, &name, Version::parse(&version)?)?;
            }
            Ok(())
        }
        _ => Err("elm.json type must be application or package".into()),
    }
}
pub(crate) fn cached(home: &Path, name: &str, version: Version) -> bool {
    let root = home
        .join("0.19.1/packages")
        .join(name)
        .join(version.to_string());
    root.join("src").is_dir() && root.join("elm.json").is_file()
}

/// With exact application pins, solver verification is equivalent to checking
/// every metadata constraint against the declared set. A missing transitive
/// dependency is a hand-edited manifest, not permission to silently add it.
pub(crate) fn verify_application(
    home: &Path,
    selected: &BTreeMap<String, Version>,
) -> Result<(), String> {
    for (name, version) in selected {
        let path = home
            .join("0.19.1/packages")
            .join(name)
            .join(version.to_string())
            .join("elm.json");
        let config = crate::package_network::cached_metadata(home, name, *version)?
            .ok_or_else(|| format!("{}: missing package metadata", path.display()))?;
        let elm = package_solver::Constraint::parse(
            config["elm-version"]
                .as_str()
                .ok_or("package missing elm-version constraint")?,
        )?;
        if !elm.contains(Version::ELM) {
            return Err(format!(
                "package {name}@{version} does not support Elm 0.19.1"
            ));
        }
        for (dependency, constraint) in config["dependencies"]
            .as_object()
            .ok_or("package missing dependencies")?
        {
            let constraint = package_solver::Constraint::parse(
                constraint.as_str().ok_or("invalid dependency constraint")?,
            )?;
            let Some(pinned) = selected.get(dependency) else {
                return Err(format!(
                    "hand-edited dependencies: {name} requires missing package {dependency}"
                ));
            };
            if !constraint.contains(*pinned) {
                return Err(format!(
                    "incompatible dependencies: {name}@{version} rejects {dependency}@{pinned}"
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn application_versions(config: &Value) -> Result<BTreeMap<String, Version>, String> {
    let mut selected = BTreeMap::new();
    let mut sections = BTreeMap::new();
    for (index, (section, category)) in [
        ("dependencies", "direct"),
        ("dependencies", "indirect"),
        ("test-dependencies", "direct"),
        ("test-dependencies", "indirect"),
    ]
    .into_iter()
    .enumerate()
    {
        if section == "test-dependencies" && config[section].is_null() {
            continue;
        }
        for (name, value) in config[section][category]
            .as_object()
            .ok_or("missing dependency object")?
        {
            if !valid_name(name) {
                return Err(format!("invalid package name {name}"));
            }
            let version = Version::parse(value.as_str().ok_or("invalid dependency version")?)?;
            if let Some(previous) = selected.insert(name.clone(), version) {
                // Details.checkAppDeps permits only indirect/test-direct
                // overlap, and only when both versions are identical.
                if previous != version || sections.get(name) != Some(&1) || index != 2 {
                    return Err(crate::dependency_error::hand_edited());
                }
            }
            sections.insert(name.clone(), index);
        }
    }
    Ok(selected)
}
