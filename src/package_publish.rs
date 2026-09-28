//! Publication preflight and semantic-version rules from Elm 0.19.1 Publish.
//! These checks do not register a package or create/push a Git tag.
use crate::{
    api_diff::{self, Magnitude},
    package_bump::{self, Candidate},
    package_solver::Version,
};
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    Application,
    NoExposedModules,
    NoSummary,
    NoReadme,
    ShortReadme,
    NoLicense,
    NotInitialVersion(Version),
    AlreadyPublished(Version),
    InvalidBump {
        stated: Version,
        latest: Version,
    },
    BadBump {
        old: Version,
        stated: Version,
        stated_magnitude: Magnitude,
        required: Version,
        actual_magnitude: Magnitude,
    },
    InvalidDocumentation(String),
    Io(String),
}

/// The outline must first pass the normal Elm outline decoder and validator.
pub fn check_description(outline: &Value) -> Result<(), Problem> {
    if outline["type"] == "application" {
        return Err(Problem::Application);
    }
    let exposed = &outline["exposed-modules"];
    let nonempty = exposed.as_array().is_some_and(|items| !items.is_empty())
        || exposed.as_object().is_some_and(|groups| {
            groups
                .values()
                .any(|items| items.as_array().is_some_and(|items| !items.is_empty()))
        });
    if !nonempty {
        return Err(Problem::NoExposedModules);
    }
    if outline["summary"].as_str().is_none_or(|summary| {
        summary.is_empty() || summary == "helpful summary of your project, less than 80 characters"
    }) {
        return Err(Problem::NoSummary);
    }
    Ok(())
}

pub fn check_readme(root: &Path) -> Result<(), Problem> {
    let metadata = match fs::metadata(root.join("README.md")) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return Err(Problem::NoReadme),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(Problem::NoReadme);
        }
        Err(error) => return Err(Problem::Io(error.to_string())),
    };
    if metadata.len() < 300 {
        Err(Problem::ShortReadme)
    } else {
        Ok(())
    }
}

pub fn check_license(root: &Path) -> Result<(), Problem> {
    match fs::metadata(root.join("LICENSE")) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(Problem::NoLicense),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(Problem::NoLicense),
        Err(error) => Err(Problem::Io(error.to_string())),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum VersionPlan {
    Initial,
    Compare(Candidate),
}

/// Run after building the local documentation, as in the official publisher.
/// Compare indicates exactly which published docs must be fetched next.
pub fn version_plan(stated: Version, published: &[Version]) -> Result<VersionPlan, Problem> {
    let Some(latest) = published.iter().max().copied() else {
        return if stated == Version([1, 0, 0]) {
            Ok(VersionPlan::Initial)
        } else {
            Err(Problem::NotInitialVersion(stated))
        };
    };
    if published.contains(&stated) {
        return Err(Problem::AlreadyPublished(stated));
    }
    package_bump::possibilities(published)
        .into_iter()
        .find(|candidate| candidate.new == stated)
        .map(VersionPlan::Compare)
        .ok_or(Problem::InvalidBump { stated, latest })
}

pub fn check_api(candidate: &Candidate, old_docs: &Value, new_docs: &Value) -> Result<(), Problem> {
    let magnitude = api_diff::diff(old_docs, new_docs)
        .map_err(Problem::InvalidDocumentation)?
        .magnitude();
    let required = magnitude.bump(candidate.old);
    if candidate.new == required {
        Ok(())
    } else {
        Err(Problem::BadBump {
            old: candidate.old,
            stated: candidate.new,
            stated_magnitude: candidate.magnitude,
            required,
            actual_magnitude: magnitude,
        })
    }
}

/// Build the exposed package API using the same dependency and type checks as make.
/// Called only after the description, README and license checks have passed.
pub fn build_documentation(manifest: &Path, home: &Path) -> Result<Value, String> {
    let selected = crate::package_resolution::resolve(manifest, home)?;
    crate::dependency_build::verify(manifest, home, &selected)?;
    let entries = crate::project::exposed_entries(manifest)?;
    let graph =
        crate::project::discover_many_selected(manifest, &entries, home, false, Some(&selected))?;
    Ok(Value::Array(
        crate::analyze::documentation(&graph)?.documentation,
    ))
}

/// Obtain the precise previous API selected by Publish.verifyVersion, then
/// validate the claimed change. The returned plan is also used by progress UI.
pub fn verify_version(
    network: &crate::package_network::PackageNetwork,
    home: &Path,
    name: &str,
    stated: Version,
    published: &[Version],
    documentation: &Value,
) -> Result<VersionPlan, String> {
    let render = |problem: Problem| {
        crate::publish_diagnostic::problem(&problem).unwrap_or_else(|| format!("{problem:?}"))
    };
    let plan = version_plan(stated, published).map_err(render)?;
    if let VersionPlan::Compare(ref candidate) = plan {
        let old = network
            .documentation(home, name, candidate.old)
            .map_err(|error| {
                crate::diff_diagnostic::documentation_context(
                    &format!(
                        "I need the docs for {} to verify that {stated} really does come next",
                        candidate.old
                    ),
                    error,
                )
            })?;
        check_api(candidate, &old, documentation).map_err(render)?;
    }
    Ok(plan)
}
