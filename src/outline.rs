//! elm.json shape and filesystem validation from Elm.Outline (Elm 0.19.1).
use crate::package_solver::{Constraint, Version, valid_name};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

#[derive(Debug)]
pub struct Problem {
    pub title: &'static str,
    pub message: String,
    missing_field: Option<String>,
    path: Vec<String>,
    rendered: Option<Value>,
}
impl Problem {
    fn new(title: &'static str, message: impl Into<String>) -> Self {
        Self {
            title,
            message: message.into(),
            missing_field: None,
            path: Vec::new(),
            rendered: None,
        }
    }
    fn prepend(mut self, path: &[&str]) -> Self {
        self.path
            .splice(0..0, path.iter().map(|part| (*part).to_string()));
        self
    }
    pub fn report(&self) -> Value {
        self.rendered.clone().unwrap_or_else(|| json!({"type":"error","path":"elm.json","title":self.title,"message":[self.message]}))
    }
    pub fn encode(&self) -> String {
        format!("ELM_OUTLINE_JSON:{}", self.report())
    }
}
pub fn report_encoded(message: &str) -> Option<Value> {
    message
        .strip_prefix("ELM_OUTLINE_JSON:")
        .and_then(|text| serde_json::from_str(text).ok())
}
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Problem> {
    value
        .as_object()
        .ok_or_else(|| {
            Problem::new(
                "EXPECTING OBJECT",
                format!("The object containing {name} must be a JSON object."),
            )
        })?
        .get(name)
        .ok_or_else(|| {
            let mut problem = Problem::new(
                "MISSING FIELD",
                format!("elm.json requires the {name} field."),
            );
            problem.missing_field = Some(name.into());
            problem
        })
}
fn string<'a>(value: &'a Value, context: &str) -> Result<&'a str, Problem> {
    value.as_str().ok_or_else(|| {
        Problem::new("EXPECTING STRING", format!("{context} must be a string.")).prepend(&[context])
    })
}
fn version(value: &Value, context: &str) -> Result<Version, Problem> {
    Version::parse(string(value, context)?).map_err(|e| {
        let mut problem = Problem::new("PROBLEM WITH VERSION", e);
        problem.path.push(context.into());
        problem
    })
}
fn constraint(value: &Value, context: &str) -> Result<Constraint, Problem> {
    Constraint::parse(string(value, context)?).map_err(|e| {
        let mut problem = Problem::new("PROBLEM WITH CONSTRAINT", e);
        problem.path.push(context.into());
        problem
    })
}
fn dependencies(value: &Value, constraints: bool, context: &str) -> Result<(), Problem> {
    for (name, value) in value.as_object().ok_or_else(|| {
        Problem::new("EXPECTING OBJECT", format!("{context} must be an object."))
            .prepend(&context.split('.').collect::<Vec<_>>())
    })? {
        if !valid_name(name) {
            let mut problem = Problem::new(
                "PROBLEM WITH DEPENDENCY NAME",
                format!("Invalid dependency name: {name}"),
            );
            problem.path = context.split('.').map(str::to_string).collect();
            problem.path.push(name.clone());
            return Err(problem);
        }
        let result = if constraints {
            constraint(value, name).map(|_| ())
        } else {
            version(value, name).map(|_| ())
        };
        result.map_err(|mut problem| {
            let mut path: Vec<String> = context.split('.').map(str::to_string).collect();
            path.append(&mut problem.path);
            problem.path = path;
            problem
        })?;
    }
    Ok(())
}
/// Decode-time package checks, without the project-level elm/core requirement.
/// Deps.Solver uses Outline.decoder, whereas Outline.read adds that requirement.
pub fn validate_package_metadata(config: &Value) -> Result<(), Problem> {
    if string(field(config, "type")?, "type")? != "package" {
        return Err(Problem::new(
            "UNEXPECTED TYPE",
            "Dependency metadata must describe a package.",
        ));
    }
    validate_inner(config, Path::new("."), false)
}

pub fn validate(config: &Value, root: &Path) -> Result<(), Problem> {
    validate_inner(config, root, true).map_err(|mut problem| {
        if (problem.missing_field.is_some()
            || matches!(
                problem.title,
                "NO SOURCE DIRECTORIES"
                    | "SUMMARY TOO LONG"
                    | "UNKNOWN LICENSE"
                    | "INVALID PACKAGE NAME"
                    | "PROBLEM WITH VERSION"
                    | "PROBLEM WITH CONSTRAINT"
                    | "PROBLEM WITH DEPENDENCY NAME"
                    | "UNEXPECTED TYPE"
                    | "PROBLEM WITH MODULE NAME"
                    | "EXPECTING STRING"
                    | "EXPECTING ARRAY"
                    | "EXPECTING OBJECT"
            ))
            && let Ok(source) = std::fs::read_to_string(root.join("elm.json"))
        {
            problem.rendered = if let Some(field) = &problem.missing_field {
                crate::outline_diagnostic::missing_field(&source, &problem.path, field)
            } else if matches!(
                problem.title,
                "EXPECTING STRING" | "EXPECTING ARRAY" | "EXPECTING OBJECT"
            ) {
                crate::outline_diagnostic::expectation(&source, &problem.path, problem.title)
            } else if problem.title == "PROBLEM WITH MODULE NAME" {
                crate::outline_diagnostic::modules(&source)
            } else if problem.title == "NO SOURCE DIRECTORIES" {
                crate::outline_diagnostic::empty_sources(&source)
            } else if matches!(
                problem.title,
                "PROBLEM WITH DEPENDENCY NAME" | "UNEXPECTED TYPE"
            ) {
                crate::outline_diagnostic::names(&source, &problem.path, problem.title)
            } else if matches!(
                problem.title,
                "PROBLEM WITH VERSION" | "PROBLEM WITH CONSTRAINT"
            ) {
                crate::outline_diagnostic::version_constraint(&source, &problem.path, problem.title)
            } else {
                crate::outline_diagnostic::package_metadata(&source, problem.title)
            };
        }
        problem
    })
}

fn validate_inner(config: &Value, root: &Path, project: bool) -> Result<(), Problem> {
    match string(field(config, "type")?, "type")? {
        "application" => {
            version(field(config, "elm-version")?, "elm-version")?;
            let dirs = field(config, "source-directories")?
                .as_array()
                .ok_or_else(|| {
                    Problem::new("EXPECTING ARRAY", "source-directories must be an array.")
                        .prepend(&["source-directories"])
                })?;
            if dirs.is_empty() {
                return Err(Problem::new(
                    "NO SOURCE DIRECTORIES",
                    "source-directories must contain at least one directory.",
                ));
            }
            for (index, dir) in dirs.iter().enumerate() {
                string(dir, &index.to_string())
                    .map_err(|problem| problem.prepend(&["source-directories"]))?;
            }
            for section in ["dependencies", "test-dependencies"] {
                for category in ["direct", "indirect"] {
                    dependencies(
                        field(field(config, section)?, category).map_err(|mut problem| {
                            problem.path.insert(0, section.into());
                            problem
                        })?,
                        false,
                        &format!("{section}.{category}"),
                    )?;
                }
            }
            if config["dependencies"]["direct"].get("elm/core").is_none() {
                return Err(Problem::new(
                    "MISSING DEPENDENCY",
                    crate::dependency_error::missing_required_package(true, false),
                ));
            }
            if config["dependencies"]["direct"].get("elm/json").is_none()
                && config["dependencies"]["indirect"].get("elm/json").is_none()
            {
                return Err(Problem::new(
                    "MISSING DEPENDENCY",
                    crate::dependency_error::missing_required_package(false, false),
                ));
            }
            let missing = dirs
                .iter()
                .map(|v| v.as_str().unwrap())
                .filter(|s| !root.join(s).is_dir())
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                let mut problem = Problem::new(
                    if missing.len() == 1 {
                        "MISSING SOURCE DIRECTORY"
                    } else {
                        "MISSING SOURCE DIRECTORIES"
                    },
                    format!("Source directories do not exist: {}", missing.join(", ")),
                );
                problem.rendered = Some(crate::outline_diagnostic::missing_sources(&missing));
                return Err(problem);
            }
            let mut grouped = BTreeMap::<_, Vec<&str>>::new();
            for dir in dirs {
                let text = dir.as_str().unwrap();
                let canonical = root
                    .join(text)
                    .canonicalize()
                    .map_err(|e| Problem::new("MISSING SOURCE DIRECTORY", e.to_string()))?;
                grouped.entry(canonical).or_default().push(text);
            }
            if let Some((canonical, entries)) = grouped
                .into_iter()
                .filter(|(_, entries)| entries.len() > 1)
                .min_by(|(left, _), (right, _)| left.as_os_str().cmp(right.as_os_str()))
            {
                // Elm chooses the first canonical path, with newest spellings first.
                let first = entries[entries.len() - 1];
                let second = entries[entries.len() - 2];
                let mut problem = Problem::new(
                    "REDUNDANT SOURCE DIRECTORIES",
                    format!("Source directory listed more than once: {first}"),
                );
                problem.rendered = Some(crate::outline_diagnostic::duplicate_sources(
                    &canonical, first, second,
                ));
                return Err(problem);
            }
        }
        "package" => {
            let name = string(field(config, "name")?, "name")?;
            if !valid_name(name) {
                return Err(Problem::new(
                    "INVALID PACKAGE NAME",
                    format!("Invalid package name: {name}"),
                ));
            }
            if string(field(config, "summary")?, "summary")?.len() >= 80 {
                return Err(Problem::new(
                    "SUMMARY TOO LONG",
                    "The summary must be less than 80 bytes in the JSON source.",
                ));
            }
            let license = string(field(config, "license")?, "license")?;
            if !LICENSES.iter().any(|(code, _)| *code == license) {
                return Err(Problem::new(
                    "UNKNOWN LICENSE",
                    format!("Unrecognized Elm 0.19.1 license identifier: {license}"),
                ));
            }
            version(field(config, "version")?, "version")?;
            let exposed = field(config, "exposed-modules")?;
            if let Some(groups) = exposed.as_object() {
                for (header, modules) in groups {
                    if header.len() >= 20 {
                        // D.oneOf prefers the list alternative on this depth
                        // tie: the official diagnostic is EXPECTING ARRAY.
                        return Err(Problem::new(
                            "EXPECTING ARRAY",
                            "Exposed-module headings must be less than 20 bytes in the JSON source.",
                        ).prepend(&["exposed-modules"]));
                    }
                    modules_list(modules)
                        .map_err(|problem| problem.prepend(&["exposed-modules", header]))?;
                }
            } else {
                modules_list(exposed).map_err(|problem| problem.prepend(&["exposed-modules"]))?;
            }
            dependencies(field(config, "dependencies")?, true, "dependencies")?;
            dependencies(
                field(config, "test-dependencies")?,
                true,
                "test-dependencies",
            )?;
            constraint(field(config, "elm-version")?, "elm-version")?;
            if project && name != "elm/core" && config["dependencies"].get("elm/core").is_none() {
                return Err(Problem::new(
                    "MISSING DEPENDENCY",
                    crate::dependency_error::missing_required_package(true, true),
                ));
            }
        }
        _ => {
            return Err(Problem::new(
                "UNEXPECTED TYPE",
                "The type field must be application or package.",
            ));
        }
    }
    Ok(())
}
fn modules_list(value: &Value) -> Result<(), Problem> {
    for (index, name) in value
        .as_array()
        .ok_or_else(|| {
            Problem::new(
                "EXPECTING ARRAY",
                "Exposed modules must be an array of module names.",
            )
        })?
        .iter()
        .enumerate()
    {
        let name = string(name, &index.to_string())?;
        if module_name_error_offset(name).is_some() {
            return Err(Problem::new(
                "PROBLEM WITH MODULE NAME",
                format!("Invalid exposed module name: {name}"),
            ));
        }
    }
    Ok(())
}

// Elm.ModuleName.parser consumes a segment before checking the byte limit.
// Return a byte offset; snippet rendering converts it to the source column.
pub(crate) fn module_name_error_offset(name: &str) -> Option<usize> {
    let mut start = true;
    for (offset, character) in name.char_indices() {
        if start {
            if !crate::unicode::is_upper(character) {
                return Some(offset);
            }
            start = false;
        } else if character == '.' {
            start = true;
        } else if !(crate::unicode::is_alpha(character)
            || character.is_ascii_digit()
            || character == '_')
        {
            return Some(offset);
        }
    }
    (start || name.len() >= 256).then_some(name.len())
}

// Historical OSI/SPDX whitelist from Elm 0.19.1 Elm/Licenses.hs.
// Upstream SHA-256: e0be94d304e97400542cb3d0d572aec3e89dc1be17b76dbb2a21d39ad8363e32; see LICENSE-ELM.
pub(crate) const LICENSES: &[(&str, &str)] = &[
    ("0BSD", "BSD Zero Clause License"),
    ("AAL", "Attribution Assurance License"),
    ("AFL-1.1", "Academic Free License v1.1"),
    ("AFL-1.2", "Academic Free License v1.2"),
    ("AFL-2.0", "Academic Free License v2.0"),
    ("AFL-2.1", "Academic Free License v2.1"),
    ("AFL-3.0", "Academic Free License v3.0"),
    ("AGPL-3.0", "GNU Affero General Public License v3.0"),
    ("APL-1.0", "Adaptive Public License 1.0"),
    ("APSL-1.0", "Apple Public Source License 1.0"),
    ("APSL-1.1", "Apple Public Source License 1.1"),
    ("APSL-1.2", "Apple Public Source License 1.2"),
    ("APSL-2.0", "Apple Public Source License 2.0"),
    ("Apache-1.1", "Apache License 1.1"),
    ("Apache-2.0", "Apache License 2.0"),
    ("Artistic-1.0", "Artistic License 1.0"),
    ("Artistic-1.0-Perl", "Artistic License 1.0 (Perl)"),
    ("Artistic-1.0-cl8", "Artistic License 1.0 w/clause 8"),
    ("Artistic-2.0", "Artistic License 2.0"),
    ("BSD-2-Clause", "BSD 2-clause \"Simplified\" License"),
    (
        "BSD-3-Clause",
        "BSD 3-clause \"New\" or \"Revised\" License",
    ),
    ("BSL-1.0", "Boost Software License 1.0"),
    (
        "CATOSL-1.1",
        "Computer Associates Trusted Open Source License 1.1",
    ),
    (
        "CDDL-1.0",
        "Common Development and Distribution License 1.0",
    ),
    ("CECILL-2.1", "CeCILL Free Software License Agreement v2.1"),
    ("CNRI-Python", "CNRI Python License"),
    ("CPAL-1.0", "Common Public Attribution License 1.0"),
    ("CPL-1.0", "Common Public License 1.0"),
    ("CUA-OPL-1.0", "CUA Office Public License v1.0"),
    ("ECL-1.0", "Educational Community License v1.0"),
    ("ECL-2.0", "Educational Community License v2.0"),
    ("EFL-1.0", "Eiffel Forum License v1.0"),
    ("EFL-2.0", "Eiffel Forum License v2.0"),
    ("EPL-1.0", "Eclipse Public License 1.0"),
    ("EUDatagrid", "EU DataGrid Software License"),
    ("EUPL-1.1", "European Union Public License 1.1"),
    ("Entessa", "Entessa Public License v1.0"),
    ("Fair", "Fair License"),
    ("Frameworx-1.0", "Frameworx Open License 1.0"),
    ("GPL-2.0", "GNU General Public License v2.0 only"),
    ("GPL-3.0", "GNU General Public License v3.0 only"),
    ("HPND", "Historic Permission Notice and Disclaimer"),
    ("IPA", "IPA Font License"),
    ("IPL-1.0", "IBM Public License v1.0"),
    ("ISC", "ISC License"),
    ("Intel", "Intel Open Source License"),
    ("LGPL-2.0", "GNU Library General Public License v2 only"),
    ("LGPL-2.1", "GNU Lesser General Public License v2.1 only"),
    ("LGPL-3.0", "GNU Lesser General Public License v3.0 only"),
    ("LPL-1.0", "Lucent Public License Version 1.0"),
    ("LPL-1.02", "Lucent Public License v1.02"),
    ("LPPL-1.3c", "LaTeX Project Public License v1.3c"),
    (
        "LiLiQ-P-1.1",
        "Licence Libre du Québec – Permissive version 1.1",
    ),
    (
        "LiLiQ-R-1.1",
        "Licence Libre du Québec – Réciprocité version 1.1",
    ),
    (
        "LiLiQ-Rplus-1.1",
        "Licence Libre du Québec – Réciprocité forte version 1.1",
    ),
    ("MIT", "MIT License"),
    ("MPL-1.0", "Mozilla Public License 1.0"),
    ("MPL-1.1", "Mozilla Public License 1.1"),
    ("MPL-2.0", "Mozilla Public License 2.0"),
    (
        "MPL-2.0-no-copyleft-exception",
        "Mozilla Public License 2.0 (no copyleft exception)",
    ),
    ("MS-PL", "Microsoft Public License"),
    ("MS-RL", "Microsoft Reciprocal License"),
    ("MirOS", "MirOS Licence"),
    ("Motosoto", "Motosoto License"),
    ("Multics", "Multics License"),
    ("NASA-1.3", "NASA Open Source Agreement 1.3"),
    ("NCSA", "University of Illinois/NCSA Open Source License"),
    ("NGPL", "Nethack General Public License"),
    ("NPOSL-3.0", "Non-Profit Open Software License 3.0"),
    ("NTP", "NTP License"),
    ("Naumen", "Naumen Public License"),
    ("Nokia", "Nokia Open Source License"),
    ("OCLC-2.0", "OCLC Research Public License 2.0"),
    ("OFL-1.1", "SIL Open Font License 1.1"),
    ("OGTSL", "Open Group Test Suite License"),
    ("OSET-PL-2.1", "OSET Public License version 2.1"),
    ("OSL-1.0", "Open Software License 1.0"),
    ("OSL-2.0", "Open Software License 2.0"),
    ("OSL-2.1", "Open Software License 2.1"),
    ("OSL-3.0", "Open Software License 3.0"),
    ("PHP-3.0", "PHP License v3.0"),
    ("PostgreSQL", "PostgreSQL License"),
    ("Python-2.0", "Python License 2.0"),
    ("QPL-1.0", "Q Public License 1.0"),
    ("RPL-1.1", "Reciprocal Public License 1.1"),
    ("RPL-1.5", "Reciprocal Public License 1.5"),
    ("RPSL-1.0", "RealNetworks Public Source License v1.0"),
    ("RSCPL", "Ricoh Source Code Public License"),
    ("SISSL", "Sun Industry Standards Source License v1.1"),
    ("SPL-1.0", "Sun Public License v1.0"),
    ("SimPL-2.0", "Simple Public License 2.0"),
    ("Sleepycat", "Sleepycat License"),
    ("UPL-1.0", "Universal Permissive License v1.0"),
    ("VSL-1.0", "Vovida Software License v1.0"),
    ("W3C", "W3C Software Notice and License (2002-12-31)"),
    ("Watcom-1.0", "Sybase Open Watcom Public License 1.0"),
    ("Xnet", "X.Net License"),
    ("ZPL-2.0", "Zope Public License 2.0"),
    ("Zlib", "zlib License"),
];

/// Elm's Json.String and customString operate on source snippets, retaining
/// JSON escapes. Check JSON syntax first, then preserve those string contents.
pub fn decode(source: &str) -> Result<Value, String> {
    decode_named_fields(&crate::outline_json::normalize(source)?)
}

/// Decodes the manifest of the project being built. It is read like the release it
/// selects, so that release is selected first; dependency manifests never select one.
pub fn decode_project(source: &str) -> Result<Value, String> {
    crate::edition::select_from_manifest(source);
    decode(source)
}

pub fn decode_bytes(bytes: &[u8]) -> Result<Value, String> {
    decode(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)
}

/// Json.Decode.field chooses the first occurrence. Dependency dictionaries use
/// Map.fromList instead, and must retain their distinct last-occurrence behavior.
pub(crate) struct JsonFields(pub(crate) Vec<(String, Box<serde_json::value::RawValue>)>);
impl<'de> serde::Deserialize<'de> for JsonFields {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FieldsVisitor;
        impl<'de> serde::de::Visitor<'de> for FieldsVisitor {
            type Value = JsonFields;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut fields = Vec::new();
                while let Some((key, value)) =
                    map.next_entry::<String, Box<serde_json::value::RawValue>>()?
                {
                    fields.push((key, value));
                }
                Ok(JsonFields(fields))
            }
        }
        deserializer.deserialize_map(FieldsVisitor)
    }
}

// Invalid containers only need their shape: diagnostics retain the original
// source separately. Do not deserialize their potentially unbounded contents.
fn scalar_or_shape(source: &str) -> Result<Value, String> {
    match source.trim_start().as_bytes().first() {
        Some(b'[') => Ok(Value::Array(Vec::new())),
        Some(b'{') => Ok(Value::Object(serde_json::Map::new())),
        _ => serde_json::from_str(source).map_err(|error| error.to_string()),
    }
}
fn string_list(source: &str) -> Result<Value, String> {
    if !source.trim_start().starts_with('[') {
        return scalar_or_shape(source);
    }
    let entries: Vec<&serde_json::value::RawValue> =
        serde_json::from_str(source).map_err(|error| error.to_string())?;
    entries
        .into_iter()
        .map(|raw| scalar_or_shape(raw.get()))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

fn decode_named_fields(source: &str) -> Result<Value, String> {
    if !source.trim_start().starts_with('{') {
        return scalar_or_shape(source);
    }
    let fields = serde_json::from_str::<JsonFields>(source)
        .map_err(|e| e.to_string())?
        .first();
    let kind = fields
        .get("type")
        .and_then(|raw| serde_json::from_str::<String>(raw.get()).ok());
    let application = kind.as_deref() == Some("application");
    let known_fields: Option<&[&str]> = match kind.as_deref() {
        Some("application") => Some(&[
            "type",
            "elm-version",
            "source-directories",
            "dependencies",
            "test-dependencies",
        ]),
        Some("package") => Some(&[
            "type",
            "name",
            "summary",
            "license",
            "version",
            "exposed-modules",
            "elm-version",
            "dependencies",
            "test-dependencies",
        ]),
        _ => None,
    };
    let mut result = serde_json::Map::new();
    if known_fields.is_none() {
        // With a missing/invalid type, Outline.decoder never visits other data.
        for (key, raw) in fields {
            result.insert(key, scalar_or_shape(raw.get())?);
        }
        return Ok(Value::Object(result));
    }
    for (key, raw) in fields {
        // Outline.decoder only visits named fields. Syntax has already been
        // checked, so ignored extension values need not become recursive Values.
        if known_fields.is_some_and(|names| !names.contains(&key.as_str())) {
            continue;
        }
        let value = if application
            && matches!(key.as_str(), "dependencies" | "test-dependencies")
            && raw.get().trim_start().starts_with('{')
        {
            let sections = serde_json::from_str::<JsonFields>(raw.get())
                .map_err(|e| e.to_string())?
                .first();
            let mut values = serde_json::Map::new();
            for (name, raw) in sections {
                let value = if matches!(name.as_str(), "direct" | "indirect") {
                    dependency_dictionary(raw.get(), false)?
                } else {
                    continue;
                };
                values.insert(name, value);
            }
            Value::Object(values)
        } else if !application && matches!(key.as_str(), "dependencies" | "test-dependencies") {
            dependency_dictionary(raw.get(), true)?
        } else if !application && key == "exposed-modules" {
            exposed_groups(raw.get())?
        } else if key == "source-directories" {
            string_list(raw.get())?
        } else {
            scalar_or_shape(raw.get())?
        };
        result.insert(key, value);
    }
    Ok(Value::Object(result))
}

impl JsonFields {
    pub(crate) fn first(
        self,
    ) -> std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> {
        let mut result = std::collections::BTreeMap::new();
        for (key, value) in self.0 {
            result.entry(key).or_insert(value);
        }
        result
    }
}

fn dependency_dictionary(source: &str, constraints: bool) -> Result<Value, String> {
    if !source.trim_start().starts_with('{') {
        return scalar_or_shape(source);
    }
    let JsonFields(entries) = serde_json::from_str(source).map_err(|e| e.to_string())?;
    let mut result = serde_json::Map::new();
    for (name, raw) in entries {
        let value = scalar_or_shape(raw.get())?;
        // Retain an invalid occurrence for the normal outline validator rather
        // than losing it to Map.fromList's last-value projection. Validation
        // still runs in outline field order (e.g. a missing name wins first).
        let valid = valid_name(&name)
            && if constraints {
                constraint(&value, &name).is_ok()
            } else {
                version(&value, &name).is_ok()
            };
        if !valid {
            // Only the first invalid entry can affect a rejected outline.
            // Preserve it alone so map key sorting cannot change its priority.
            return Ok(Value::Object(serde_json::Map::from_iter([(name, value)])));
        }
        result.insert(name, value);
    }
    Ok(Value::Object(result))
}

fn exposed_groups(source: &str) -> Result<Value, String> {
    if !source.trim_start().starts_with('{') {
        return string_list(source);
    }
    let JsonFields(groups) = serde_json::from_str(source).map_err(|e| e.to_string())?;
    let mut result = serde_json::Map::new();
    for (heading, raw) in groups {
        let modules = string_list(raw.get())?;
        if heading.len() >= 20 || modules_list(&modules).is_err() {
            // Defer reporting until the exposed-modules field is validated.
            return Ok(Value::Object(serde_json::Map::from_iter([(
                heading, modules,
            )])));
        }
        let Value::Array(modules) = modules else {
            unreachable!("validated module list")
        };
        let combined = result
            .entry(heading)
            .or_insert_with(|| Value::Array(Vec::new()));
        combined.as_array_mut().unwrap().extend(modules);
    }
    Ok(Value::Object(result))
}
