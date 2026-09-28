//! Installation plans ported from Elm 0.19.1 terminal/src/Install.hs.
//! Planning never writes the project. Confirmation and verification are separate.
use crate::{package_network::PackageNetwork, package_solver};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, io::Write, path::Path};

#[derive(Debug, PartialEq, Eq)]
pub enum PlanKind {
    AlreadyInstalled,
    PromoteIndirect,
    PromoteTest,
    Changes,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Insert(String),
    Replace { old: String, new: String },
    Remove(String),
}

#[derive(Debug)]
pub struct Plan {
    pub kind: PlanKind,
    pub outline: Value,
    pub changes: BTreeMap<String, Change>,
}

/// Apply an approved plan only after verifying its dependencies. Preserve the
/// original bytes on failure and refuse to overwrite an intervening user edit.
pub fn apply(
    home: &Path,
    manifest: &Path,
    original: &[u8],
    plan: &Plan,
    network: Option<&PackageNetwork>,
) -> Result<(), String> {
    if plan.kind == PlanKind::AlreadyInstalled {
        return Ok(());
    }
    // Resolve an existing manifest symlink rather than replacing the link.
    let destination = manifest.canonicalize().map_err(|e| e.to_string())?;
    verify(home, &plan.outline, network)?;
    if fs::read(manifest).map_err(|e| e.to_string())? != original
        || manifest.canonicalize().map_err(|e| e.to_string())? != destination
    {
        return Err("elm.json changed during installation; retry with the updated project".into());
    }
    let parent = destination
        .parent()
        .ok_or("manifest has no parent directory")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary
        .as_file()
        .set_permissions(
            fs::metadata(&destination)
                .map_err(|e| e.to_string())?
                .permissions(),
        )
        .map_err(|e| e.to_string())?;
    let mut serializer = serde_json::Serializer::with_formatter(
        &mut temporary,
        serde_json::ser::PrettyFormatter::with_indent(b"    "),
    );
    serde::Serialize::serialize(&plan.outline, &mut serializer).map_err(|e| e.to_string())?;
    temporary.write_all(b"\n").map_err(|e| e.to_string())?;
    temporary.persist(destination).map_err(|e| e.to_string())?;
    Ok(())
}

/// Verify every selected dependency, including test dependencies and exposed
/// modules not imported by the project. Do not compile project sources or write
/// its manifest: a failed dependency must leave the original project intact.
pub fn verify(
    home: &Path,
    outline: &Value,
    network: Option<&PackageNetwork>,
) -> Result<(), String> {
    let selected = match crate::package_resolution::validate_project(outline)? {
        Some(pins) => {
            let selected = package_solver::solve_pinned(home, &pins, network)?;
            if selected.len() != pins.len() {
                return Err(crate::dependency_error::hand_edited());
            }
            selected
        }
        None => match network {
            Some(network) => package_solver::solve_online(
                home,
                &outline["dependencies"],
                &outline["test-dependencies"],
                network,
            )?,
            None => package_solver::solve(
                home,
                &outline["dependencies"],
                &outline["test-dependencies"],
            )?,
        },
    };
    if let Some(network) = network {
        for (name, version) in &selected {
            network.package(home, name, package_solver::Version::parse(version)?)?;
        }
    }
    for (name, version) in &selected {
        let manifest = home
            .join("0.19.1/packages")
            .join(name)
            .join(version)
            .join("elm.json");
        let entries = crate::project::dependency_exposed_entries(&manifest)?;
        if entries.is_empty() {
            continue;
        }
        let graph = crate::project::discover_many_selected(
            &manifest,
            &entries,
            home,
            false,
            Some(&selected),
        )?;
        let report = crate::analyze::generate_modules(&graph)?;
        if !report.generation_errors.is_empty() {
            return Err(format!(
                "cannot build {name} {version}: {}",
                report.generation_errors.join("\n")
            ));
        }
    }
    Ok(())
}

fn entries(value: &Value) -> Result<BTreeMap<String, String>, String> {
    value
        .as_object()
        .ok_or("expected dependency object")?
        .iter()
        .map(|(name, version)| {
            Ok((
                name.clone(),
                version
                    .as_str()
                    .ok_or("expected dependency version")?
                    .into(),
            ))
        })
        .collect()
}

fn dependencies(outline: &Value) -> Result<BTreeMap<String, String>, String> {
    let mut all = BTreeMap::new();
    for section in ["test-dependencies", "dependencies"] {
        if outline["type"] == "application" {
            for field in ["indirect", "direct"] {
                all.extend(entries(&outline[section][field])?);
            }
        } else {
            all.extend(entries(&outline[section])?);
        }
    }
    Ok(all)
}

pub fn plan(
    home: &Path,
    outline: &Value,
    package: &str,
    network: Option<&PackageNetwork>,
) -> Result<Plan, String> {
    if !package_solver::valid_name(package) {
        return Err(format!("invalid package name {package}"));
    }
    let application = match outline["type"].as_str() {
        Some("application") => true,
        Some("package") => false,
        _ => return Err("expected an application or package outline".into()),
    };
    let old = dependencies(outline)?;
    let mut updated = outline.clone();
    let direct = if application {
        &outline["dependencies"]["direct"]
    } else {
        &outline["dependencies"]
    };
    if direct.get(package).is_some() {
        return Ok(Plan {
            kind: PlanKind::AlreadyInstalled,
            outline: updated,
            changes: BTreeMap::new(),
        });
    }
    let sources: &[(&str, Option<&str>, PlanKind)] = if application {
        &[
            ("dependencies", Some("indirect"), PlanKind::PromoteIndirect),
            ("test-dependencies", Some("direct"), PlanKind::PromoteTest),
            ("test-dependencies", Some("indirect"), PlanKind::PromoteTest),
        ]
    } else {
        &[("test-dependencies", None, PlanKind::PromoteTest)]
    };
    for (section, field, kind) in sources {
        let source = if let Some(field) = field {
            &mut updated[section][field]
        } else {
            &mut updated[section]
        };
        if let Some(version) = source
            .as_object_mut()
            .ok_or("expected dependency object")?
            .remove(package)
        {
            if application {
                updated["dependencies"]["direct"][package] = version;
            } else {
                updated["dependencies"][package] = version;
            }
            return Ok(Plan {
                kind: match kind {
                    PlanKind::PromoteIndirect => PlanKind::PromoteIndirect,
                    _ => PlanKind::PromoteTest,
                },
                outline: updated,
                changes: BTreeMap::new(),
            });
        }
    }
    if let Some(registry) = crate::registry::Registry::read_cached(home)?
        && registry.versions(package).is_none()
    {
        return Err(crate::install_diagnostic::unknown(
            package,
            &registry.nearby_names(package),
            network.is_some(),
        ));
    }
    let solution_error =
        |error| crate::install_diagnostic::solution(error, package, application, network.is_some());
    if application {
        updated = package_solver::add_to_application(home, outline, package, network)
            .map_err(solution_error)?;
    } else {
        let mut constraints = outline["dependencies"].clone();
        constraints[package] = json!("1.0.0 <= v <= 65535.0.0");
        let selected = if let Some(network) = network {
            package_solver::solve_online(home, &constraints, &outline["test-dependencies"], network)
        } else {
            package_solver::solve(home, &constraints, &outline["test-dependencies"])
        }
        .map_err(solution_error)?;
        let version = package_solver::Version::parse(
            selected
                .get(package)
                .ok_or("incomplete dependency solution")?,
        )?;
        updated["dependencies"][package] = json!(format!(
            "{version} <= v < {}.0.0",
            version.0[0].wrapping_add(1)
        ));
    }
    let new = dependencies(&updated)?;
    let mut changes = BTreeMap::new();
    for (name, version) in &old {
        match new.get(name) {
            None => {
                changes.insert(name.clone(), Change::Remove(version.clone()));
            }
            Some(next) if next != version => {
                changes.insert(
                    name.clone(),
                    Change::Replace {
                        old: version.clone(),
                        new: next.clone(),
                    },
                );
            }
            _ => {}
        }
    }
    for (name, version) in new {
        if !old.contains_key(&name) {
            changes.insert(name, Change::Insert(version));
        }
    }
    Ok(Plan {
        kind: PlanKind::Changes,
        outline: updated,
        changes,
    })
}
