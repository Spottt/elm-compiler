//! elm.json shape and filesystem validation from Elm.Outline (Elm 0.19.1).
use crate::package_solver::{Constraint, Version, valid_name};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

#[derive(Debug)]
pub struct Problem {
    pub title: &'static str,
    pub message: String,
}
impl Problem {
    fn new(title: &'static str, message: impl Into<String>) -> Self {
        Self {
            title,
            message: message.into(),
        }
    }
    pub fn report(&self) -> Value {
        json!({"type":"error","path":"elm.json","title":self.title,"message":[self.message]})
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
            Problem::new(
                "MISSING FIELD",
                format!("elm.json requires the {name} field."),
            )
        })
}
fn string<'a>(value: &'a Value, context: &str) -> Result<&'a str, Problem> {
    value
        .as_str()
        .ok_or_else(|| Problem::new("EXPECTING STRING", format!("{context} must be a string.")))
}
fn version(value: &Value, context: &str) -> Result<Version, Problem> {
    Version::parse(string(value, context)?).map_err(|e| Problem::new("PROBLEM WITH VERSION", e))
}
fn constraint(value: &Value, context: &str) -> Result<Constraint, Problem> {
    Constraint::parse(string(value, context)?)
        .map_err(|e| Problem::new("PROBLEM WITH CONSTRAINT", e))
}
fn dependencies(value: &Value, constraints: bool, context: &str) -> Result<(), Problem> {
    for (name, value) in value
        .as_object()
        .ok_or_else(|| Problem::new("EXPECTING OBJECT", format!("{context} must be an object.")))?
    {
        if !valid_name(name) {
            return Err(Problem::new(
                "PROBLEM WITH DEPENDENCY NAME",
                format!("Invalid dependency name: {name}"),
            ));
        }
        if constraints {
            constraint(value, name)?;
        } else {
            version(value, name)?;
        }
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
    validate_inner(config, root, true)
}

fn validate_inner(config: &Value, root: &Path, project: bool) -> Result<(), Problem> {
    match string(field(config, "type")?, "type")? {
        "application" => {
            version(field(config, "elm-version")?, "elm-version")?;
            let dirs = field(config, "source-directories")?
                .as_array()
                .ok_or_else(|| {
                    Problem::new("EXPECTING ARRAY", "source-directories must be an array.")
                })?;
            if dirs.is_empty() {
                return Err(Problem::new(
                    "NO SOURCE DIRECTORIES",
                    "source-directories must contain at least one directory.",
                ));
            }
            for dir in dirs {
                string(dir, "source-directories entry")?;
            }
            for section in ["dependencies", "test-dependencies"] {
                for category in ["direct", "indirect"] {
                    dependencies(
                        field(field(config, section)?, category)?,
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
                return Err(Problem::new(
                    if missing.len() == 1 {
                        "MISSING SOURCE DIRECTORY"
                    } else {
                        "MISSING SOURCE DIRECTORIES"
                    },
                    format!("Source directories do not exist: {}", missing.join(", ")),
                ));
            }
            let mut seen = BTreeSet::new();
            for dir in dirs {
                let text = dir.as_str().unwrap();
                let canonical = root
                    .join(text)
                    .canonicalize()
                    .map_err(|e| Problem::new("MISSING SOURCE DIRECTORY", e.to_string()))?;
                if !seen.insert(canonical) {
                    return Err(Problem::new(
                        "REDUNDANT SOURCE DIRECTORIES",
                        format!("Source directory listed more than once: {text}"),
                    ));
                }
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
            if !LICENSES.contains(&license) {
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
                        ));
                    }
                    modules_list(modules)?;
                }
            } else {
                modules_list(exposed)?;
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
    for name in value.as_array().ok_or_else(|| {
        Problem::new(
            "EXPECTING ARRAY",
            "Exposed modules must be an array of module names.",
        )
    })? {
        let name = string(name, "exposed module")?;
        if name.len() >= 256
            || !name.split('.').all(|part| {
                let mut chars = part.chars();
                chars.next().is_some_and(crate::unicode::is_upper)
                    && chars.all(|c| crate::unicode::is_alpha(c) || c.is_ascii_digit() || c == '_')
            })
        {
            return Err(Problem::new(
                "PROBLEM WITH MODULE NAME",
                format!("Invalid exposed module name: {name}"),
            ));
        }
    }
    Ok(())
}

// Historical OSI/SPDX whitelist from Elm 0.19.1 Elm/Licenses.hs.
// Upstream SHA-256: e0be94d304e97400542cb3d0d572aec3e89dc1be17b76dbb2a21d39ad8363e32; see LICENSE-ELM.
const LICENSES: &[&str] = &[
    "0BSD",
    "AAL",
    "AFL-1.1",
    "AFL-1.2",
    "AFL-2.0",
    "AFL-2.1",
    "AFL-3.0",
    "AGPL-3.0",
    "Apache-1.1",
    "Apache-2.0",
    "APL-1.0",
    "APSL-1.0",
    "APSL-1.1",
    "APSL-1.2",
    "APSL-2.0",
    "Artistic-1.0",
    "Artistic-1.0-cl8",
    "Artistic-1.0-Perl",
    "Artistic-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "CATOSL-1.1",
    "CDDL-1.0",
    "CECILL-2.1",
    "CNRI-Python",
    "CPAL-1.0",
    "CPL-1.0",
    "CUA-OPL-1.0",
    "ECL-1.0",
    "ECL-2.0",
    "EFL-1.0",
    "EFL-2.0",
    "Entessa",
    "EPL-1.0",
    "EUDatagrid",
    "EUPL-1.1",
    "Fair",
    "Frameworx-1.0",
    "GPL-2.0",
    "GPL-3.0",
    "HPND",
    "Intel",
    "IPA",
    "IPL-1.0",
    "ISC",
    "LGPL-2.0",
    "LGPL-2.1",
    "LGPL-3.0",
    "LiLiQ-P-1.1",
    "LiLiQ-R-1.1",
    "LiLiQ-Rplus-1.1",
    "LPL-1.0",
    "LPL-1.02",
    "LPPL-1.3c",
    "MirOS",
    "MIT",
    "Motosoto",
    "MPL-1.0",
    "MPL-1.1",
    "MPL-2.0",
    "MPL-2.0-no-copyleft-exception",
    "MS-PL",
    "MS-RL",
    "Multics",
    "NASA-1.3",
    "Naumen",
    "NCSA",
    "NGPL",
    "Nokia",
    "NPOSL-3.0",
    "NTP",
    "OCLC-2.0",
    "OFL-1.1",
    "OGTSL",
    "OSET-PL-2.1",
    "OSL-1.0",
    "OSL-2.0",
    "OSL-2.1",
    "OSL-3.0",
    "PHP-3.0",
    "PostgreSQL",
    "Python-2.0",
    "QPL-1.0",
    "RPL-1.1",
    "RPL-1.5",
    "RPSL-1.0",
    "RSCPL",
    "SimPL-2.0",
    "SISSL",
    "Sleepycat",
    "SPL-1.0",
    "UPL-1.0",
    "VSL-1.0",
    "W3C",
    "Watcom-1.0",
    "Xnet",
    "Zlib",
    "ZPL-2.0",
];

/// Elm's Json.String and customString operate on source snippets, retaining
/// JSON escapes. Check JSON syntax first, then preserve those string contents.
pub fn decode(source: &str) -> Result<Value, String> {
    let _: Value = serde_json::from_str(source).map_err(|e| e.to_string())?;
    let bytes = source.as_bytes();
    let mut out = String::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'"' {
            let start = cursor;
            while cursor < bytes.len() && bytes[cursor] != b'"' {
                cursor += 1;
            }
            out.push_str(&source[start..cursor]);
        } else {
            let start = cursor + 1;
            cursor = start;
            while bytes[cursor] != b'"' {
                if bytes[cursor] == b'\\' {
                    cursor += 1;
                }
                cursor += 1;
            }
            out.push_str(
                &serde_json::to_string(&source[start..cursor]).map_err(|e| e.to_string())?,
            );
            cursor += 1;
        }
    }
    decode_named_fields(&out)
}

pub fn decode_bytes(bytes: &[u8]) -> Result<Value, String> {
    decode(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)
}

/// Json.Decode.field chooses the first occurrence. Dependency dictionaries use
/// Map.fromList instead, and must retain their distinct last-occurrence behavior.
struct JsonFields(Vec<(String, Box<serde_json::value::RawValue>)>);
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

fn decode_named_fields(source: &str) -> Result<Value, String> {
    if !source.trim_start().starts_with('{') {
        return serde_json::from_str(source).map_err(|e| e.to_string());
    }
    let fields = serde_json::from_str::<JsonFields>(source)
        .map_err(|e| e.to_string())?
        .first();
    let application = fields
        .get("type")
        .and_then(|raw| serde_json::from_str::<String>(raw.get()).ok())
        .is_some_and(|kind| kind == "application");
    let mut result = serde_json::Map::new();
    for (key, raw) in fields {
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
                    serde_json::from_str(raw.get()).map_err(|e| e.to_string())?
                };
                values.insert(name, value);
            }
            Value::Object(values)
        } else if !application && matches!(key.as_str(), "dependencies" | "test-dependencies") {
            dependency_dictionary(raw.get(), true)?
        } else if !application && key == "exposed-modules" {
            exposed_groups(raw.get())?
        } else {
            serde_json::from_str(raw.get()).map_err(|e| e.to_string())?
        };
        result.insert(key, value);
    }
    Ok(Value::Object(result))
}

impl JsonFields {
    fn first(self) -> std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> {
        let mut result = std::collections::BTreeMap::new();
        for (key, value) in self.0 {
            result.entry(key).or_insert(value);
        }
        result
    }
}

fn dependency_dictionary(source: &str, constraints: bool) -> Result<Value, String> {
    if !source.trim_start().starts_with('{') {
        return serde_json::from_str(source).map_err(|e| e.to_string());
    }
    let JsonFields(entries) = serde_json::from_str(source).map_err(|e| e.to_string())?;
    let mut result = serde_json::Map::new();
    for (name, raw) in entries {
        let value: Value = serde_json::from_str(raw.get()).map_err(|e| e.to_string())?;
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
        return serde_json::from_str(source).map_err(|e| e.to_string());
    }
    let JsonFields(groups) = serde_json::from_str(source).map_err(|e| e.to_string())?;
    let mut result = serde_json::Map::new();
    for (heading, raw) in groups {
        let modules: Value = serde_json::from_str(raw.get()).map_err(|e| e.to_string())?;
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
