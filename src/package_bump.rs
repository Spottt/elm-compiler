//! Version candidates from Elm 0.19.1 Deps.Bump, plus guarded manifest updates.
use crate::{
    api_diff::Magnitude,
    package_solver::{Constraint, Version},
};
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::Value;
use std::{collections::BTreeMap, fs, io::Write, path::Path};

#[derive(Debug, PartialEq, Eq)]
pub struct Candidate {
    pub old: Version,
    pub new: Version,
    pub magnitude: Magnitude,
}

/// Major changes start at the latest release; minor changes at the latest
/// release of each major; patches at the latest release of each major/minor.
pub fn possibilities(published: &[Version]) -> Vec<Candidate> {
    let mut versions = published.to_vec();
    versions.sort();
    let Some(latest) = versions.last().copied() else {
        return Vec::new();
    };
    let mut majors = BTreeMap::new();
    let mut minors = BTreeMap::new();
    for version in versions {
        majors.insert(version.0[0], version);
        minors.insert((version.0[0], version.0[1]), version);
    }
    let candidate = |old, magnitude: Magnitude| Candidate {
        old,
        new: magnitude.bump(old),
        magnitude,
    };
    let mut result = vec![candidate(latest, Magnitude::Major)];
    result.extend(
        majors
            .into_values()
            .map(|old| candidate(old, Magnitude::Minor)),
    );
    result.extend(
        minors
            .into_values()
            .map(|old| candidate(old, Magnitude::Patch)),
    );
    result
}

pub fn bumpable_versions(published: &[Version]) -> Vec<Version> {
    let mut versions: Vec<_> = possibilities(published)
        .into_iter()
        .map(|c| c.old)
        .collect();
    versions.sort();
    versions.dedup();
    versions
}

struct PackageOutline<'a> {
    value: &'a Value,
    summary: &'a serde_json::value::RawValue,
    groups: Option<&'a OrderedFields>,
}
struct OrderedFields(Vec<(String, Value)>);
impl Serialize for OrderedFields {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut object = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            object.serialize_entry(key, value)?;
        }
        object.end()
    }
}
impl Serialize for PackageOutline<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut object = serializer.serialize_map(Some(9))?;
        for key in [
            "type",
            "name",
            "summary",
            "license",
            "version",
            "exposed-modules",
            "elm-version",
            "dependencies",
            "test-dependencies",
        ] {
            match key {
                "summary" => object.serialize_entry(key, self.summary)?,
                "exposed-modules" if self.groups.is_some() => {
                    object.serialize_entry(key, self.groups.unwrap())?
                }
                "dependencies" | "test-dependencies" => {
                    let mut entries: Vec<_> = self.value[key]
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    entries.sort_by(|(a, _), (b, _)| crate::registry::compare_names(a, b));
                    object.serialize_entry(key, &OrderedFields(entries))?;
                }
                _ => object.serialize_entry(key, &self.value[key])?,
            }
        }
        object.end()
    }
}

/// Apply only after the caller obtains confirmation. Recheck both original
/// bytes and symlink destination to avoid discarding a concurrent edit.
pub fn change_version(manifest: &Path, original: &[u8], target: Version) -> Result<(), String> {
    let destination = manifest.canonicalize().map_err(|e| e.to_string())?;
    let mut outline =
        crate::outline::decode(std::str::from_utf8(original).map_err(|e| e.to_string())?)?;
    crate::outline::validate(&outline, manifest.parent().ok_or("manifest has no parent")?)
        .map_err(|e| e.encode())?;
    if outline["type"] != "package" {
        return Err("cannot change the version of an application".into());
    }
    outline["version"] = Value::String(target.to_string());
    // Outline.write encodes parsed constraints, including Word16-normalized
    // version components, rather than replaying their original JSON spelling.
    let normalize = |value: &mut Value| -> Result<(), String> {
        let constraint = Constraint::parse(value.as_str().ok_or("missing constraint")?)?;
        *value = Value::String(constraint.to_string());
        Ok(())
    };
    normalize(&mut outline["elm-version"])?;
    for key in ["dependencies", "test-dependencies"] {
        for value in outline[key]
            .as_object_mut()
            .ok_or("missing dependency dictionary")?
            .values_mut()
        {
            normalize(value)?;
        }
    }

    // Json.String retains the source spelling of summary escapes. ExposedDict
    // retains input order; only dependency dictionaries use package-name order.
    let fields = serde_json::from_slice::<crate::outline::JsonFields>(original)
        .map_err(|e| e.to_string())?
        .first();
    let summary = fields.get("summary").ok_or("missing summary")?;
    let groups = if outline["exposed-modules"].is_object() {
        let raw = fields
            .get("exposed-modules")
            .ok_or("missing exposed modules")?;
        let entries = serde_json::from_str::<crate::outline::JsonFields>(raw.get())
            .map_err(|e| e.to_string())?;
        Some(OrderedFields(
            entries
                .0
                .into_iter()
                .map(|(k, v)| {
                    serde_json::from_str(v.get())
                        .map(|value| (k, value))
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<_, _>>()?,
        ))
    } else {
        None
    };

    let parent = destination.parent().ok_or("manifest has no parent")?;
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
    PackageOutline {
        value: &outline,
        summary,
        groups: groups.as_ref(),
    }
    .serialize(&mut serializer)
    .map_err(|e| e.to_string())?;
    temporary.write_all(b"\n").map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    if fs::read(manifest).map_err(|e| e.to_string())? != original
        || manifest.canonicalize().map_err(|e| e.to_string())? != destination
    {
        return Err(
            "elm.json changed while preparing the version update; retry with the updated project"
                .into(),
        );
    }
    temporary.persist(destination).map_err(|e| e.to_string())?;
    Ok(())
}
