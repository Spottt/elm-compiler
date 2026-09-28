//! Dependency discovery for applications and locally cached package projects.
//! Uses the existing package cache read-only; no downloads, shell invocation or
//! cached Elm interfaces. Package identities remain distinct across modules.
use crate::module::header;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
const DEFAULTS: &[&str] = &[
    "Basics",
    "Debug",
    "List",
    "Maybe",
    "Result",
    "String",
    "Char",
    "Tuple",
    "Platform",
    "Platform.Cmd",
    "Platform.Sub",
];
#[derive(Clone)]
struct Package {
    roots: Vec<PathBuf>,
    exposed: BTreeSet<String>,
    dependencies: BTreeSet<String>,
}
#[derive(Debug, Clone)]
pub struct Module {
    /// Expected name for a source-root file lacking an explicit module header.
    pub missing_header: Option<String>,
    pub owner: String,
    pub name: String,
    pub path: PathBuf,
    pub imports: Vec<String>,
    pub bytes: usize,
    pub tokens: usize,
    pub kernel: bool,
    /// Immutable source shared by discovery, inference and generation.
    pub source: String,
}
#[derive(Clone)]
pub struct Graph {
    pub entry: String,
    /// Canonical roots exported by this compilation, without duplicates.
    pub entries: Vec<String>,
    pub modules: Vec<Module>,
    pub manifests: Vec<(PathBuf, String)>,
}
struct Loader {
    recover_lexical: bool,
    packages: BTreeMap<String, Package>,
    visiting: BTreeSet<String>,
    done: BTreeSet<String>,
    modules: Vec<Module>,
}
fn json(path: &Path, inputs: &mut Vec<(PathBuf, String)>) -> Result<Value, String> {
    let s = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let value = crate::outline::decode(&s).map_err(|e| format!("{}: {e}", path.display()))?;
    inputs.push((path.to_owned(), s));
    Ok(value)
}
fn names(value: &Value) -> Result<BTreeSet<String>, String> {
    Ok(value
        .as_object()
        .ok_or("expected dependency object")?
        .keys()
        .cloned()
        .collect())
}
fn exposed(value: &Value) -> Result<BTreeSet<String>, String> {
    fn add(value: &Value, out: &mut BTreeSet<String>) -> Result<(), String> {
        for name in value.as_array().ok_or("expected exposed module array")? {
            out.insert(
                name.as_str()
                    .ok_or("expected exposed module name")?
                    .to_string(),
            );
        }
        Ok(())
    }
    let mut out = BTreeSet::new();
    if let Some(groups) = value.as_object() {
        for group in groups.values() {
            add(group, &mut out)?;
        }
    } else {
        add(value, &mut out)?;
    }
    Ok(out)
}
impl Loader {
    fn resolve(&self, owner: &str, name: &str) -> Result<(String, PathBuf, bool), String> {
        let package = &self.packages[owner];
        let relative = name.replace('.', "/") + ".elm";
        let mut found = Vec::new();
        for root in &package.roots {
            let path = root.join(&relative);
            if path.is_file() {
                found.push((owner.to_string(), path, false));
            }
        }
        if name.starts_with("Elm.Kernel.") && is_kernel_package(owner) {
            for root in &package.roots {
                let path = root.join(name.replace('.', "/") + ".js");
                if path.is_file() {
                    found.push((owner.to_string(), path, true));
                }
            }
        }
        for dep in &package.dependencies {
            let candidate = self
                .packages
                .get(dep)
                .ok_or_else(|| format!("missing dependency {dep} of {owner}"))?;
            if name.starts_with("Elm.Kernel.") && is_kernel_package(owner) {
                for root in &candidate.roots {
                    let path = root.join(name.replace('.', "/") + ".js");
                    if path.is_file() {
                        found.push((dep.clone(), path, true));
                    }
                }
            }
            if candidate.exposed.contains(name) {
                for root in &candidate.roots {
                    let path = root.join(&relative);
                    if path.is_file() {
                        found.push((dep.clone(), path, false));
                    }
                }
            }
        }
        match found.len() {
            0 => Err(format!("{owner}: cannot resolve import {name}")),
            1 => Ok(found.remove(0)),
            _ => Err(format!("{owner}: ambiguous import {name}: {found:?}")),
        }
    }
    // Kernel edges are runtime dependencies and may form cycles (including
    // back-edges to Elm). Discover them after the acyclic Elm import traversal.
    fn runtime_dependencies(&mut self) -> Result<(), String> {
        let mut index = 0;
        while index < self.modules.len() {
            if !self.modules[index].kernel {
                index += 1;
                continue;
            }
            let owner = self.modules[index].owner.clone();
            let path = self.modules[index].path.clone();
            let source = &self.modules[index].source;
            let content =
                crate::kernel::parse(source).map_err(|e| format!("{}: {e}", path.display()))?;
            let imports = content.imports;
            let mut dependencies = BTreeSet::new();
            for import in imports {
                let (dep_owner, dep_path, kernel) = if import.name.starts_with("Elm.Kernel.") {
                    // Kernel names are linked globally, including references
                    // from core to Json whose package depends on core itself.
                    let mut candidates = Vec::new();
                    for (package, info) in &self.packages {
                        if is_kernel_package(package) {
                            for root in &info.roots {
                                let path = root.join(import.name.replace('.', "/") + ".js");
                                if path.is_file() {
                                    candidates.push((package.clone(), path, true));
                                }
                            }
                        }
                    }
                    if candidates.len() != 1 {
                        return Err(format!(
                            "kernel import {} has {} providers",
                            import.name,
                            candidates.len()
                        ));
                    }
                    candidates.remove(0)
                } else {
                    self.resolve(&owner, &import.name)?
                };
                dependencies.insert(format!("{dep_owner}:{}", import.name));
                self.visit(dep_owner, import.name, dep_path, kernel)?;
            }
            self.modules[index].imports = dependencies.into_iter().collect();
            index += 1;
        }
        Ok(())
    }
    fn visit(
        &mut self,
        owner: String,
        name: String,
        path: PathBuf,
        kernel: bool,
    ) -> Result<(), String> {
        let id = format!("{owner}:{name}");
        if self.done.contains(&id) {
            return Ok(());
        }
        if !self.visiting.insert(id.clone()) {
            return Err(format!("cyclic import at {id}"));
        }
        let source = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut dependencies = BTreeMap::new();
        let token_count;
        if kernel {
            token_count = 0;
        } else {
            let (h, count, broken_body) = match discovery_header(&source, self.recover_lexical) {
                Ok(header) => header,
                Err(_) if self.recover_lexical => {
                    // Preserve invalid source for the analysis error collector.
                    // Its identity is known from the resolved import or entry path.
                    self.visiting.remove(&id);
                    self.done.insert(id);
                    self.modules.push(Module {
                        missing_header: None,
                        owner,
                        name,
                        path,
                        imports: Vec::new(),
                        bytes: source.len(),
                        tokens: 0,
                        kernel: false,
                        source,
                    });
                    return Ok(());
                }
                Err(error) => return Err(format!("{}:{error}", path.display())),
            };
            token_count = count;
            if !h.explicit
                && let Some(expected) = entry_name_from_path(&path, &self.packages[&owner].roots)
            {
                self.visiting.remove(&id);
                self.done.insert(id);
                self.modules.push(Module {
                    missing_header: Some(expected),
                    owner,
                    name,
                    path,
                    imports: Vec::new(),
                    bytes: source.len(),
                    tokens: count,
                    kernel: false,
                    source,
                });
                return Ok(());
            }
            if h.name != name {
                return Err(format!(
                    "{}: expected module {name}, found {}",
                    path.display(),
                    h.name
                ));
            }
            // Elm does not resolve imports of a module whose source is malformed.
            for import in h.imports.into_iter().filter(|_| !broken_body) {
                let (dep_owner, dep_path, kernel) = self.resolve(&owner, &import.name)?;
                dependencies.insert(
                    format!("{dep_owner}:{}", import.name),
                    (dep_owner, import.name, dep_path, kernel),
                );
            }
            if owner != "elm/core" {
                let core = self.packages.get("elm/core").ok_or("missing elm/core")?;
                for name in DEFAULTS {
                    let path = core.roots[0].join(name.replace('.', "/") + ".elm");
                    dependencies.insert(
                        format!("elm/core:{name}"),
                        ("elm/core".into(), name.to_string(), path, false),
                    );
                }
            }
        }
        let imports = dependencies.keys().cloned().collect();
        // Tokens are released above. Keep one immutable copy of each source so
        // later phases and the cache fingerprint always see identical bytes.
        let bytes = source.len();
        for (_, (dep_owner, dep_name, dep_path, dep_kernel)) in dependencies {
            self.visit(dep_owner, dep_name, dep_path, dep_kernel)?;
        }
        self.visiting.remove(&id);
        self.done.insert(id);
        self.modules.push(Module {
            missing_header: None,
            owner,
            name,
            path,
            imports,
            bytes,
            tokens: token_count,
            kernel,
            source,
        });
        Ok(())
    }
}

pub fn discover(manifest: &Path, entry: &Path, elm_home: &Path) -> Result<Graph, String> {
    discover_with_runtime(manifest, entry, elm_home, true, None, false)
}
fn discover_with_runtime(
    manifest: &Path,
    entry: &Path,
    elm_home: &Path,
    runtime: bool,
    selected_packages: Option<&BTreeMap<String, String>>,
    recover_lexical: bool,
) -> Result<Graph, String> {
    let manifest = manifest.canonicalize().map_err(|e| e.to_string())?;
    let root = manifest.parent().ok_or("missing project directory")?;
    let mut manifests = Vec::new();
    let config = json(&manifest, &mut manifests)?;
    let (owner, roots, direct, selected, own_exposed) = match config["type"].as_str() {
        Some("application") => {
            let elm = crate::package_solver::Version::parse(
                config["elm-version"]
                    .as_str()
                    .ok_or("missing elm-version")?,
            )?;
            if elm != crate::package_solver::Version::ELM {
                return Err(crate::dependency_error::application_version(elm));
            }
            let roots = config["source-directories"]
                .as_array()
                .ok_or("missing source-directories")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(|s| root.join(s))
                        .ok_or_else(|| "invalid source-directory".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut selected = BTreeMap::new();
            for category in ["direct", "indirect"] {
                for (name, version) in config["dependencies"][category]
                    .as_object()
                    .ok_or("missing dependencies")?
                {
                    selected.insert(
                        name.clone(),
                        crate::package_solver::Version::parse(
                            version.as_str().ok_or("invalid dependency version")?,
                        )?
                        .to_string(),
                    );
                }
            }
            if selected_packages.is_none()
                && let Some(registry) = crate::registry::Registry::read_cached(elm_home)?
            {
                for (name, text) in &selected {
                    let version = crate::package_solver::Version::parse(text)?;
                    if !registry
                        .versions(name)
                        .is_some_and(|versions| versions.contains(&version))
                    {
                        return Err(format!(
                            "package {name}@{version} is not in the cached registry"
                        ));
                    }
                }
            }
            (
                "application".to_owned(),
                roots,
                names(&config["dependencies"]["direct"])?,
                selected,
                BTreeSet::new(),
            )
        }
        Some("package") => {
            let owner = config["name"]
                .as_str()
                .filter(|name| crate::package_solver::valid_name(name))
                .ok_or("invalid package name")?
                .to_owned();
            let elm = crate::package_solver::Constraint::parse(
                config["elm-version"]
                    .as_str()
                    .ok_or("missing elm-version")?,
            )?;
            if !elm.contains(crate::package_solver::Version::ELM) {
                return Err(crate::dependency_error::package_version(elm));
            }
            let selected = match selected_packages {
                Some(selected) => selected.clone(),
                None => crate::package_solver::solve(
                    elm_home,
                    &config["dependencies"],
                    &config["test-dependencies"],
                )?,
            };
            (
                owner,
                vec![root.join("src")],
                names(&config["dependencies"])?,
                selected,
                exposed(&config["exposed-modules"])?,
            )
        }
        _ => return Err("elm.json type must be application or package".into()),
    };
    let mut packages = BTreeMap::new();
    for (name, version) in &selected {
        if name.split('/').count() != 2
            || name.split('/').any(|s| s.is_empty() || s == "..")
            || !version.chars().all(|c| c.is_ascii_digit() || c == '.')
        {
            return Err(format!("invalid package identity {name}@{version}"));
        }
        let base = elm_home.join("0.19.1/packages").join(name).join(version);
        let package = json(&base.join("elm.json"), &mut manifests)?;
        packages.insert(
            name.clone(),
            Package {
                roots: vec![base.join("src")],
                exposed: exposed(&package["exposed-modules"])?,
                dependencies: names(&package["dependencies"])?,
            },
        );
    }
    packages.insert(
        owner.clone(),
        Package {
            roots,
            exposed: own_exposed,
            dependencies: direct,
        },
    );
    let path = root.join(entry);
    let source = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = match discovery_header(&source, recover_lexical) {
        Ok((header, _, _)) => {
            if recover_lexical && !header.explicit {
                entry_name_from_path(&path, &packages[&owner].roots).unwrap_or(header.name)
            } else {
                header.name
            }
        }
        Err(error) => {
            let inferred = recover_lexical
                .then(|| entry_name_from_path(&path, &packages[&owner].roots))
                .flatten();
            inferred.ok_or_else(|| format!("{}:{error}", path.display()))?
        }
    };
    drop(source);
    let mut loader = Loader {
        recover_lexical,
        packages,
        visiting: BTreeSet::new(),
        done: BTreeSet::new(),
        modules: Vec::new(),
    };
    // JSON converters are compiler-introduced dependencies for flags/ports.
    if let Some(json) = loader.packages.get("elm/json").cloned() {
        for module in ["Json.Decode", "Json.Encode"] {
            let path = json.roots[0].join(module.replace('.', "/") + ".elm");
            loader.visit("elm/json".into(), module.into(), path, false)?;
        }
    }
    let entry = format!("{owner}:{name}");
    loader.visit(owner, name, path, false)?;
    if runtime {
        loader.runtime_dependencies()?;
    }
    Ok(Graph {
        entries: vec![entry.clone()],
        entry,
        modules: loader.modules,
        manifests,
    })
}

fn is_kernel_package(owner: &str) -> bool {
    owner.starts_with("elm/") || owner.starts_with("elm-explorations/")
}

/// Combine dependency-first graphs, sharing modules and runtime registrations.
pub fn discover_many(
    manifest: &Path,
    entries: &[PathBuf],
    elm_home: &Path,
) -> Result<Graph, String> {
    discover_many_with_runtime(manifest, entries, elm_home, true, None, false)
}
pub fn discover_many_for_check(
    manifest: &Path,
    entries: &[PathBuf],
    elm_home: &Path,
) -> Result<Graph, String> {
    discover_many_with_runtime(manifest, entries, elm_home, false, None, false)
}
/// Use the exact package selection prepared for this make invocation.
pub fn discover_many_selected(
    manifest: &Path,
    entries: &[PathBuf],
    elm_home: &Path,
    runtime: bool,
    selected: Option<&BTreeMap<String, String>>,
) -> Result<Graph, String> {
    discover_many_with_runtime(manifest, entries, elm_home, runtime, selected, true)
}
fn discover_many_with_runtime(
    manifest: &Path,
    entries: &[PathBuf],
    elm_home: &Path,
    runtime: bool,
    selected_packages: Option<&BTreeMap<String, String>>,
    recover_lexical: bool,
) -> Result<Graph, String> {
    let mut inputs = entries.iter();
    let first = inputs.next().ok_or("make needs at least one Elm source")?;
    let mut graph = discover_with_runtime(
        manifest,
        first,
        elm_home,
        runtime,
        selected_packages,
        recover_lexical,
    )?;
    let mut seen: BTreeSet<_> = graph
        .modules
        .iter()
        .map(|m| (m.owner.clone(), m.name.clone()))
        .collect();
    let mut manifests: BTreeSet<_> = graph.manifests.iter().map(|(p, _)| p.clone()).collect();
    for entry in inputs {
        let next = discover_with_runtime(
            manifest,
            entry,
            elm_home,
            runtime,
            selected_packages,
            recover_lexical,
        )?;
        if graph.entries.contains(&next.entry) {
            return Err(format!("duplicate entry source: {}", entry.display()));
        }
        graph.entries.push(next.entry);
        for module in next.modules {
            if seen.insert((module.owner.clone(), module.name.clone())) {
                graph.modules.push(module);
            }
        }
        for (path, source) in next.manifests {
            if manifests.insert(path.clone()) {
                graph.manifests.push((path, source));
            }
        }
    }
    Ok(graph)
}

/// Package `make` with no file arguments checks all exposed modules.
pub fn exposed_entries(manifest: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = dependency_exposed_entries(manifest)?;
    if entries.is_empty() {
        return Err("package must expose at least one module".into());
    }
    Ok(entries)
}

pub(crate) fn dependency_exposed_entries(manifest: &Path) -> Result<Vec<PathBuf>, String> {
    let config = json(manifest, &mut Vec::new())?;
    if config["type"] != "package" {
        return Err("make needs at least one Elm source in an application".into());
    }
    let root = manifest.parent().ok_or("missing project directory")?;
    let entries = exposed(&config["exposed-modules"])?;
    Ok(entries
        .into_iter()
        .map(|name| root.join("src").join(name.replace('.', "/") + ".elm"))
        .collect())
}

/// Recover only when a complete header is followed by at least one body token.
/// A lexical failure in the header/import list must remain a discovery error.
fn discovery_header(
    source: &str,
    recover_lexical: bool,
) -> Result<(crate::module::Header, usize, bool), String> {
    let (tokens, lexical_error) = crate::lexer::lex_prefix(source);
    let parsed = header(source, &tokens);
    match (parsed, lexical_error) {
        (Ok(header), None) => Ok((header, tokens.len(), false)),
        (Ok(header), Some(_)) if recover_lexical && header.body_start < tokens.len() => {
            Ok((header, tokens.len(), true))
        }
        (_, Some(error)) => Err(crate::module::prefer_header_error(source, &tokens, error)),
        (Err(error), None) => Err(error),
    }
}

/// Only use a unique identity implied by configured source directories.
fn entry_name_from_path(path: &Path, roots: &[PathBuf]) -> Option<String> {
    let path = path.canonicalize().ok()?;
    let mut names = BTreeSet::new();
    for root in roots {
        let root = root.canonicalize().ok()?;
        if let Ok(relative) = path.strip_prefix(root) {
            let stem = relative.with_extension("");
            let parts: Option<Vec<_>> = stem
                .components()
                .map(|part| {
                    let text = part.as_os_str().to_str()?;
                    let mut chars = text.chars();
                    (chars.next().is_some_and(crate::unicode::is_upper)
                        && chars
                            .all(|c| crate::unicode::is_alpha(c) || c.is_ascii_digit() || c == '_'))
                    .then(|| text.to_owned())
                })
                .collect();
            if let Some(parts) = parts {
                names.insert(parts.join("."));
            }
        }
    }
    (names.len() == 1)
        .then(|| names.into_iter().next())
        .flatten()
}

#[cfg(test)]
mod discovery_recovery_tests {
    use super::discovery_header;

    #[test]
    fn recover_only_a_completed_header_followed_by_a_body() {
        let (header, _, broken) = discovery_header(
            "module A exposing (..)\nimport B\nvalue = \"unfinished",
            true,
        )
        .unwrap();
        assert!(broken);
        assert_eq!(header.name, "A");
        assert_eq!(header.imports[0].name, "B");
        for source in [
            "module A exposing (\"unfinished",
            "module A exposing (..)\nimport \"unfinished",
            "module A exposing (..)\n\"unfinished",
        ] {
            assert!(discovery_header(source, true).is_err());
        }
    }
}
