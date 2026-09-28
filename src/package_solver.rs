//! Resolve package constraints against sources already present in ELM_HOME.
//! Elm's solver selects the first pending package alphabetically and tries its
//! versions newest first, backtracking when transitive constraints conflict.
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub [u16; 3]);
impl Version {
    pub const ELM: Self = Self([0, 19, 1]);
    /// Application manifests accepted by this compiler. Elm 0.19.2 is a patch
    /// release of the same language, so projects pinned to either compile unchanged.
    pub const APPLICATION: [Self; 2] = [Self([0, 19, 1]), Self([0, 19, 2])];
    pub fn parse(text: &str) -> Result<Self, String> {
        let parts = text.split('.').collect::<Vec<_>>();
        if parts.len() != 3
            || parts.iter().any(|p| {
                p.is_empty()
                    || p.len() > 1 && p.starts_with('0')
                    || !p.bytes().all(|c| c.is_ascii_digit())
            })
        {
            return Err(format!("invalid Elm version {text}"));
        }
        let mut result = [0; 3];
        for (slot, part) in result.iter_mut().zip(parts) {
            // Elm.Version.chompWord16 accumulates with Word16 arithmetic.
            *slot = part.bytes().fold(0u16, |total, digit| {
                total.wrapping_mul(10).wrapping_add(u16::from(digit - b'0'))
            });
        }
        Ok(Self(result))
    }
}
impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0[0], self.0[1], self.0[2])
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Constraint {
    lower: Version,
    upper: Version,
    lower_inclusive: bool,
    upper_inclusive: bool,
}
impl Constraint {
    pub fn parse(text: &str) -> Result<Self, String> {
        let parts = text.split(' ').collect::<Vec<_>>();
        let [lower, lo, "v", hi, upper] = parts.as_slice() else {
            return Err(format!("invalid Elm constraint {text}"));
        };
        if !matches!(*lo, "<" | "<=") || !matches!(*hi, "<" | "<=") {
            return Err(format!("invalid Elm constraint {text}"));
        }
        let result = Self {
            lower: Version::parse(lower)?,
            upper: Version::parse(upper)?,
            lower_inclusive: *lo == "<=",
            upper_inclusive: *hi == "<=",
        };
        if result.lower >= result.upper {
            return Err(format!("invalid Elm constraint {text}"));
        }
        Ok(result)
    }
    pub fn contains(self, version: Version) -> bool {
        (version > self.lower || self.lower_inclusive && version == self.lower)
            && (version < self.upper || self.upper_inclusive && version == self.upper)
    }
}
impl std::fmt::Display for Constraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} v {} {}",
            self.lower,
            if self.lower_inclusive { "<=" } else { "<" },
            if self.upper_inclusive { "<=" } else { "<" },
            self.upper
        )
    }
}

pub fn valid_name(name: &str) -> bool {
    name_error_offset(name).is_none()
}

/// Byte position reported by Elm.Package.parser, including the author/project
/// asymmetry: a dash before the slash terminates a valid author component.
pub(crate) fn name_error_offset(name: &str) -> Option<usize> {
    fn component(bytes: &[u8], start: usize, author: bool) -> Result<usize, usize> {
        let good = |byte: u8| {
            if author {
                byte.is_ascii_alphanumeric()
            } else {
                byte.is_ascii_lowercase() || byte.is_ascii_digit()
            }
        };
        let Some(&first) = bytes.get(start) else {
            return Err(start);
        };
        if !(if author {
            first.is_ascii_alphanumeric()
        } else {
            first.is_ascii_lowercase()
        }) {
            return Err(start);
        }
        let mut cursor = start + 1;
        let mut dash = false;
        while let Some(&byte) = bytes.get(cursor) {
            if good(byte) {
                dash = false;
            } else if byte == b'-' {
                if dash {
                    return Err(cursor);
                }
                dash = true;
            } else {
                break;
            }
            cursor += 1;
        }
        if (cursor == bytes.len() && dash) || cursor - start >= 256 {
            Err(cursor)
        } else {
            Ok(cursor)
        }
    }
    let bytes = name.as_bytes();
    let author = match component(bytes, 0, true) {
        Ok(end) => end,
        Err(at) => return Some(at),
    };
    if bytes.get(author) != Some(&b'/') {
        return Some(author);
    }
    match component(bytes, author + 1, false) {
        Ok(end) if end == bytes.len() => None,
        Ok(end) | Err(end) => Some(end),
    }
}

type Requirements = BTreeMap<String, Vec<Constraint>>;
type Metadata = (Constraint, Requirements);
fn constraints(value: &Value) -> Result<Requirements, String> {
    let mut result = BTreeMap::new();
    for (name, text) in value.as_object().ok_or("expected package dependencies")? {
        if !valid_name(name) {
            return Err(format!("invalid package name {name}"));
        }
        result.insert(
            name.clone(),
            vec![Constraint::parse(
                text.as_str().ok_or("expected version constraint")?,
            )?],
        );
    }
    Ok(result)
}
/// Only normal dependencies are made visible to source modules. Test dependencies
/// nevertheless participate in solving, as they do in Elm's Details.verifyPkg.
pub fn solve(
    home: &Path,
    dependencies: &Value,
    tests: &Value,
) -> Result<BTreeMap<String, String>, String> {
    solve_impl(home, dependencies, tests, None)
}
pub fn solve_online(
    home: &Path,
    dependencies: &Value,
    tests: &Value,
    network: &crate::package_network::PackageNetwork,
) -> Result<BTreeMap<String, String>, String> {
    solve_impl(home, dependencies, tests, Some(network))
}

/// Plan an application dependency addition, following Deps.Solver.addToApp.
/// Preserve every existing version first, then release indirect pins, then
/// permit patch, minor, and finally major upgrades. This does not write files
/// or download package sources: callers must confirm and verify the plan.
pub fn add_to_application(
    home: &Path,
    application: &Value,
    package: &str,
    network: Option<&crate::package_network::PackageNetwork>,
) -> Result<Value, String> {
    if application["type"] != "application" || !valid_name(package) {
        return Err("expected an application and a valid package name".into());
    }
    fn pins(value: &Value) -> Result<BTreeMap<String, Version>, String> {
        value
            .as_object()
            .ok_or("expected dependency object")?
            .iter()
            .map(|(name, version)| {
                if !valid_name(name) {
                    return Err(format!("invalid package name {name}"));
                }
                Ok((
                    name.clone(),
                    Version::parse(version.as_str().ok_or("expected version")?)?,
                ))
            })
            .collect()
    }
    let direct = pins(&application["dependencies"]["direct"])?;
    let indirect = pins(&application["dependencies"]["indirect"])?;
    let test_direct = pins(&application["test-dependencies"]["direct"])?;
    let test_indirect = pins(&application["test-dependencies"]["indirect"])?;
    let all_direct: BTreeMap<_, _> = test_direct
        .iter()
        .chain(&direct)
        .map(|(name, version)| (name.clone(), *version))
        .collect();
    let all: BTreeMap<_, _> = test_indirect
        .iter()
        .chain(&indirect)
        .chain(&all_direct)
        .map(|(name, version)| (name.clone(), *version))
        .collect();
    let anything = Constraint {
        lower: Version([1, 0, 0]),
        upper: Version([u16::MAX, 0, 0]),
        lower_inclusive: true,
        upper_inclusive: true,
    };
    let mut solver = Solver {
        network,
        home,
        registry: crate::registry::Registry::read_cached(home)?,
        cache: home.join("0.19.1/packages"),
        versions: BTreeMap::new(),
        manifests: BTreeMap::new(),
    };
    let mut solution = None;
    for attempt in 0..5 {
        let existing = if attempt == 0 { &all } else { &all_direct };
        let mut pending: Requirements = existing
            .iter()
            .map(|(name, version)| {
                let constraint = match attempt {
                    0 | 1 => Constraint {
                        lower: *version,
                        upper: *version,
                        lower_inclusive: true,
                        upper_inclusive: true,
                    },
                    2 => Constraint {
                        lower: *version,
                        upper: Version([version.0[0], version.0[1].wrapping_add(1), 0]),
                        lower_inclusive: true,
                        upper_inclusive: false,
                    },
                    3 => Constraint {
                        lower: *version,
                        upper: Version([version.0[0].wrapping_add(1), 0, 0]),
                        lower_inclusive: true,
                        upper_inclusive: false,
                    },
                    _ => anything,
                };
                (name.clone(), vec![constraint])
            })
            .collect();
        pending.insert(package.into(), vec![anything]);
        if let Some(selected) = solver.search(pending, BTreeMap::new())? {
            solution = Some(selected);
            break;
        }
    }
    let selected =
        solution.ok_or_else(|| crate::dependency_error::no_solution(network.is_some()))?;
    let runtime_direct: BTreeMap<_, _> = selected
        .iter()
        .filter(|(name, _)| name.as_str() == package || direct.contains_key(*name))
        .map(|(name, version)| (name.clone(), version.to_string()))
        .collect();
    let mut runtime = std::collections::BTreeSet::new();
    let mut pending: Vec<_> = runtime_direct.keys().cloned().collect();
    while let Some(name) = pending.pop() {
        if !runtime.insert(name.clone()) {
            continue;
        }
        let version = *selected
            .get(&name)
            .ok_or("incomplete dependency solution")?;
        let (_, dependencies) = solver
            .metadata(&name, version)?
            .ok_or("missing solved package metadata")?;
        pending.extend(dependencies.into_keys());
    }
    let runtime_indirect: BTreeMap<_, _> = selected
        .iter()
        .filter(|(name, _)| runtime.contains(*name) && !runtime_direct.contains_key(*name))
        .map(|(name, version)| (name.clone(), version.to_string()))
        .collect();
    let tests_direct: BTreeMap<_, _> = selected
        .iter()
        .filter(|(name, _)| name.as_str() != package && test_direct.contains_key(*name))
        .map(|(name, version)| (name.clone(), version.to_string()))
        .collect();
    let tests_indirect: BTreeMap<_, _> = selected
        .iter()
        .filter(|(name, _)| !runtime.contains(*name) && !tests_direct.contains_key(*name))
        .map(|(name, version)| (name.clone(), version.to_string()))
        .collect();
    let mut updated = application.clone();
    updated["dependencies"] =
        serde_json::json!({"direct":runtime_direct,"indirect":runtime_indirect});
    updated["test-dependencies"] =
        serde_json::json!({"direct":tests_direct,"indirect":tests_indirect});
    Ok(updated)
}

pub fn validate_dependencies(dependencies: &Value, tests: &Value) -> Result<(), String> {
    requirements(dependencies, tests).map(|_| ())
}
fn requirements(dependencies: &Value, tests: &Value) -> Result<Requirements, String> {
    let mut pending = constraints(dependencies)?;
    for (name, constraint) in constraints(tests)? {
        if pending.insert(name.clone(), constraint).is_some() {
            return Err(crate::dependency_error::hand_edited());
        }
    }
    Ok(pending)
}
fn solve_impl(
    home: &Path,
    dependencies: &Value,
    tests: &Value,
    network: Option<&crate::package_network::PackageNetwork>,
) -> Result<BTreeMap<String, String>, String> {
    solve_requirements(home, requirements(dependencies, tests)?, network)
}

pub(crate) fn solve_pinned(
    home: &Path,
    pins: &BTreeMap<String, Version>,
    network: Option<&crate::package_network::PackageNetwork>,
) -> Result<BTreeMap<String, String>, String> {
    let pending = pins
        .iter()
        .map(|(name, version)| {
            (
                name.clone(),
                vec![Constraint {
                    lower: *version,
                    upper: *version,
                    lower_inclusive: true,
                    upper_inclusive: true,
                }],
            )
        })
        .collect();
    solve_requirements(home, pending, network)
}

fn solve_requirements(
    home: &Path,
    pending: Requirements,
    network: Option<&crate::package_network::PackageNetwork>,
) -> Result<BTreeMap<String, String>, String> {
    let registry = crate::registry::Registry::read_cached(home)?;
    let mut solver = Solver {
        network,
        home,
        registry,
        cache: home.join("0.19.1/packages"),
        versions: BTreeMap::new(),
        manifests: BTreeMap::new(),
    };
    let result = solver
        .search(pending, BTreeMap::new())?
        .ok_or_else(|| crate::dependency_error::no_solution(network.is_some()))?;
    Ok(result
        .into_iter()
        .map(|(name, v)| (name, v.to_string()))
        .collect())
}
struct Solver<'a> {
    network: Option<&'a crate::package_network::PackageNetwork>,
    home: &'a Path,
    registry: Option<crate::registry::Registry>,
    cache: std::path::PathBuf,
    versions: BTreeMap<String, Vec<Version>>,
    manifests: BTreeMap<(String, Version), Option<Metadata>>,
}
impl Solver<'_> {
    fn available(&mut self, name: &str) -> Result<Vec<Version>, String> {
        if let Some(versions) = self.versions.get(name) {
            return Ok(versions.clone());
        }
        if let Some(registry) = &self.registry {
            return Ok(registry.versions(name).unwrap_or_default().to_vec());
        }
        let mut versions = Vec::new();
        match fs::read_dir(self.cache.join(name)) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry.map_err(|e| e.to_string())?;
                    if entry.file_type().map_err(|e| e.to_string())?.is_dir()
                        && let Some(text) = entry.file_name().to_str()
                        && let Ok(version) = Version::parse(text)
                    {
                        versions.push(version);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        versions.sort_unstable_by(|a, b| b.cmp(a));
        self.versions.insert(name.into(), versions.clone());
        Ok(versions)
    }
    fn metadata(&mut self, name: &str, version: Version) -> Result<Option<Metadata>, String> {
        let key = (name.to_owned(), version);
        if let Some(info) = self.manifests.get(&key) {
            return Ok(info.clone());
        }
        let root = self.cache.join(name).join(version.to_string());
        let config = if let Some(network) = self.network {
            network.metadata(self.home, name, version)?
        } else {
            let Some(config) = crate::package_network::cached_metadata(self.home, name, version)?
            else {
                self.manifests.insert(key, None);
                return Ok(None);
            };
            if !root.join("src").is_dir() {
                self.manifests.insert(key, None);
                return Ok(None);
            }
            config
        };
        let info = (
            Constraint::parse(
                config["elm-version"]
                    .as_str()
                    .ok_or("package missing elm-version constraint")?,
            )?,
            constraints(&config["dependencies"])?,
        );
        self.manifests.insert(key, Some(info.clone()));
        Ok(Some(info))
    }
    fn search(
        &mut self,
        mut pending: Requirements,
        solved: BTreeMap<String, Version>,
    ) -> Result<Option<BTreeMap<String, Version>>, String> {
        let Some(name) = pending
            .keys()
            .min_by(|a, b| crate::registry::compare_names(a, b))
            .cloned()
        else {
            return Ok(Some(solved));
        };
        let required = pending.remove(&name).unwrap();
        for version in self.available(&name)? {
            if !required.iter().all(|c| c.contains(version)) {
                continue;
            }
            let Some((elm, dependencies)) = self.metadata(&name, version)? else {
                continue;
            };
            if !elm.contains(Version::ELM) {
                continue;
            }
            let mut next = pending.clone();
            let mut chosen = solved.clone();
            chosen.insert(name.clone(), version);
            let mut valid = true;
            for (dependency, constraints) in dependencies {
                if let Some(version) = chosen.get(&dependency) {
                    if !constraints.iter().all(|c| c.contains(*version)) {
                        valid = false;
                        break;
                    }
                } else {
                    next.entry(dependency).or_default().extend(constraints);
                }
            }
            if valid && let Some(solution) = self.search(next, chosen)? {
                return Ok(Some(solution));
            }
        }
        Ok(None)
    }
}
