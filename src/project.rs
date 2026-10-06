//! Dependency discovery for applications and locally cached package projects.
//! Uses the existing package cache read-only; no downloads, shell invocation or
//! cached Elm interfaces. Package identities remain distinct across modules.
use crate::module::header;
mod snapshot;
mod source;
pub use source::Source;
pub(crate) use snapshot::source_digest as scoped_source_digest;
pub use snapshot::{Scope as SnapshotScope, scope as snapshot_scope};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
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
#[derive(Clone, PartialEq, Eq)]
struct Package {
    root: PathBuf,
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
    pub source: Source,
}
#[derive(Clone)]
pub struct Graph {
    /// Discovery failures retained for the normal independent-module error pass.
    pub import_errors: Vec<Value>,
    pub entry: String,
    /// Canonical roots exported by this compilation, without duplicates.
    pub entries: Vec<String>,
    pub modules: Vec<Module>,
    pub manifests: Vec<(PathBuf, String)>,
}
type Candidate = (String, PathBuf, bool);
#[derive(Default)]
struct DiscoveryProfile {
    modules: usize,
    read_ms: f64,
    header_ms: f64,
    providers_ms: f64,
    local_total_ms: f64,
}
impl DiscoveryProfile {
    fn enabled() -> Option<Self> {
        (std::env::var("PLANEXPO_ELM_PROFILE_DISCOVERY").as_deref() == Ok("1")).then(Self::default)
    }
    fn emit(&self) {
        eprintln!("ELM_DISCOVERY_PROFILE {}", serde_json::json!({"modules": self.modules, "read_ms": self.read_ms, "header_ms": self.header_ms, "providers_ms": self.providers_ms, "local_total_ms": self.local_total_ms}));
    }
}
// Timers are opt-in and exclude recursive visits from each module's local work.
macro_rules! discovery_time {
    ($loader:expr, $phase:ident, $body:expr) => {{
        let started = $loader.profile.as_ref().map(|_| std::time::Instant::now());
        let result = $body;
        if let Some(started) = started {
            $loader.profile.as_mut().unwrap().$phase += started.elapsed().as_secs_f64() * 1000.0;
        }
        result
    }};
}

struct Loader {
    profile: Option<DiscoveryProfile>,
    resolution: Option<usize>,
    candidates: BTreeMap<(String, String), Vec<Candidate>>,
    import_problems: Vec<crate::import_diagnostic::ImportProblems>,
    recover_lexical: bool,
    packages: BTreeMap<String, Package>,
    visiting: BTreeSet<String>,
    done: BTreeSet<String>,
    modules: Vec<Module>,
}
/// Reads an Elm module. Since 0.19.2 a file that is not UTF-8 is reported at its first bad
/// byte, unless an ordinary syntax error comes first as it would for the official lexer.
fn read_module(path: &Path) -> Result<Source, String> {
    match snapshot::read_to_string(path) {
        Ok(source) => Ok(source),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidData && crate::edition::reports_encoding() => {
            let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
            match crate::edition::decode_source(bytes) {
                Ok(source) => Ok(source.into()),
                Err((lossy, line, column)) => {
                    let earlier_syntax_error = crate::parser::parse(&lossy).err().is_some_and(|error| {
                        let mut parts = error.splitn(3, ':');
                        let position = (parts.next().and_then(|s| s.parse().ok()), parts.next().and_then(|s| s.parse().ok()));
                        matches!(position, (Some(l), Some(c)) if (l, c) <= (line, column))
                    });
                    if earlier_syntax_error {
                        Ok(lossy.into())
                    } else {
                        Err(format!("{}:{line}:{column}:{line}:{column}: {}", path.display(), crate::edition::NOT_UTF8))
                    }
                }
            }
        }
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}
fn json(path: &Path, inputs: &mut Vec<(PathBuf, String)>) -> Result<Value, String> {
    let s = snapshot::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let value = crate::outline::decode(&s).map_err(|e| format!("{}: {e}", path.display()))?;
    inputs.push((path.to_owned(), s.to_string()));
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
    fn import_reports(&self) -> Result<Vec<Value>, String> {
        let mut errors = Vec::new();
        for failure in &self.import_problems {
            let mut known: BTreeSet<String> = self.modules.iter()
                .filter(|module| module.owner == failure.owner)
                .map(|module| module.name.clone()).collect();
            known.extend(crate::import_history::known(&self.packages[&failure.owner].root.join("elm.json")));
            for dependency in &self.packages[&failure.owner].dependencies {
                known.extend(self.packages[dependency].exposed.iter().cloned());
            }
            for other in &self.import_problems {
                if other.owner == failure.owner {
                    known.extend(other.missing.iter().cloned());
                    known.extend(other.ambiguous.keys().cloned());
                }
            }
            if failure.owner != "elm/core" {
                for name in DEFAULTS { known.remove(*name); }
            }
            errors.push(failure.report(&known)?);
        }
        crate::edition::sort_module_reports(&mut errors);
        Ok(errors)
    }
    // A loader is confined to one discovery pass. Repeated imports share their
    // complete provider list, including ambiguities; no filesystem result is
    // retained across compilations, even inside a persistent worker.
    fn candidates(&mut self, owner: &str, name: &str) -> Result<Vec<Candidate>, String> {
        let key = (owner.to_owned(), name.to_owned());
        if let Some(found) = self.candidates.get(&key) { return Ok(found.clone()); }
        if let Some(found) = snapshot::candidates(self.resolution, &key) {
            self.candidates.insert(key, found.clone()); return Ok(found);
        }
        let found = self.find_candidates(owner, name)?;
        snapshot::remember_candidates(self.resolution, key.clone(), &found);
        self.candidates.insert(key, found.clone());
        Ok(found)
    }
    fn find_candidates(&self, owner: &str, name: &str) -> Result<Vec<Candidate>, String> {
        let package = &self.packages[owner];
        let relative = name.replace('.', "/") + ".elm";
        let mut found = Vec::new();
        for root in &package.roots {
            let path = root.join(&relative);
            if snapshot::is_file(&path) {
                found.push((owner.to_string(), path, false));
            }
        }
        if name.starts_with("Elm.Kernel.") && is_kernel_package(owner) {
            for root in &package.roots {
                let path = root.join(name.replace('.', "/") + ".js");
                if snapshot::is_file(&path) {
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
                    if snapshot::is_file(&path) {
                        found.push((dep.clone(), path, true));
                    }
                }
            }
            if candidate.exposed.contains(name) {
                for root in &candidate.roots {
                    let path = root.join(&relative);
                    if snapshot::is_file(&path) {
                        found.push((dep.clone(), path, false));
                    }
                }
            }
        }
        Ok(found)
    }
    fn resolve(&mut self, owner: &str, name: &str) -> Result<(String, PathBuf, bool), String> {
        let mut found = self.candidates(owner, name)?;
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
                                if snapshot::is_file(&path) {
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
        let local_started = self.profile.as_ref().map(|_| std::time::Instant::now());
        let source = discovery_time!(self, read_ms, read_module(&path))?;
        let mut dependencies = BTreeMap::new();
        let token_count;
        if kernel {
            token_count = 0;
        } else {
            let (h, count, broken_body) = match discovery_time!(self, header_ms, discovery_header_at(Some(&path), &source, self.recover_lexical)) {
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
            let mut missing = BTreeSet::new();
            let mut ambiguous = BTreeMap::new();
            for import in h.imports.iter().filter(|_| !broken_body) {
                let mut found = discovery_time!(self, providers_ms, self.candidates(&owner, &import.name))?;
                let (dep_owner, dep_path, kernel) = match found.len() {
                    1 => found.remove(0),
                    0 => {
                        missing.insert(import.name.clone());
                        continue;
                    }
                    _ => {
                        let (local, mut foreign): (Vec<_>, Vec<_>) = found.into_iter().partition(|(package, _, _)| package == &owner);
                        // Details builds its foreign-name list by prepending each
                        // dependency while traversing the ordered package map.
                        foreign.reverse();
                        use crate::import_diagnostic::Ambiguity;
                        let problem = if local.len() > 1 {
                            let root = &self.packages[&owner].root;
                            Ambiguity::Local(local.into_iter().map(|(_, path, _)| path.strip_prefix(root).unwrap_or(&path).to_path_buf()).collect())
                        } else if let Some((_, path, _)) = local.into_iter().next() {
                            Ambiguity::LocalForeign(path, foreign[0].0.clone())
                        } else {
                            Ambiguity::Foreign(foreign.into_iter().map(|(package, _, _)| package).collect())
                        };
                        ambiguous.insert(import.name.clone(), problem);
                        continue;
                    }
                };
                dependencies.insert(
                    format!("{dep_owner}:{}", import.name),
                    (dep_owner, import.name.clone(), dep_path, kernel),
                );
            }
            if !missing.is_empty() || !ambiguous.is_empty() {
                self.import_problems.push(crate::import_diagnostic::ImportProblems {
                    owner: owner.clone(), module: name.clone(), path: path.clone(),
                    source: source.to_string(), missing, ambiguous,
                });
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
        if let Some(started) = local_started {
            let profile = self.profile.as_mut().unwrap();
            profile.modules += 1;
            profile.local_total_ms += started.elapsed().as_secs_f64() * 1000.0;
        }
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
    discover_many_with_runtime(manifest, &[entry.to_path_buf()], elm_home, true, None, false)
}
fn discover_entries_with_runtime(
    manifest: &Path,
    entries: &[PathBuf],
    elm_home: &Path,
    runtime: bool,
    selected_packages: Option<&BTreeMap<String, String>>,
    recover_lexical: bool,
) -> Result<Graph, String> {
    let manifest = snapshot::canonicalize(manifest).map_err(|e| e.to_string())?;
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
            if !crate::package_solver::Version::APPLICATION.contains(&elm) {
                return Err(crate::dependency_error::application_version(elm));
            }
            crate::edition::select(elm);
            let roots = config["source-directories"]
                .as_array()
                .ok_or("missing source-directories")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(|s| root.join(s))
                        .ok_or_else(|| "invalid source-directory".to_string())
                        .and_then(|path| {
                            snapshot::canonicalize(&path)
                                .map_err(|error| format!("{}: {error}", path.display()))
                        })
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
            crate::edition::select(crate::package_solver::Version::ELM);
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
                root: base.clone(),
                roots: vec![base.join("src")],
                exposed: exposed(&package["exposed-modules"])?,
                dependencies: names(&package["dependencies"])?,
            },
        );
    }
    packages.insert(
        owner.clone(),
        Package {
            root: root.to_path_buf(),
            roots,
            exposed: own_exposed,
            dependencies: direct,
        },
    );
    let mut loader = Loader {
        profile: DiscoveryProfile::enabled(),
        resolution: snapshot::resolution(&packages),
        candidates: BTreeMap::new(),
        import_problems: Vec::new(),
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
    let mut entry_ids = Vec::new();
    for entry in entries {
        let path = root.join(entry);
        let source = read_module(&path)?;
        let name = match discovery_header_at(Some(&path), &source, recover_lexical) {
            Ok((header, _, _)) => {
                if recover_lexical && !header.explicit {
                    entry_name_from_path(&path, &loader.packages[&owner].roots).unwrap_or_else(|| header.name.clone())
                } else {
                    header.name.clone()
                }
            }
            Err(error) => {
                let inferred = recover_lexical
                    .then(|| entry_name_from_path(&path, &loader.packages[&owner].roots))
                    .flatten();
                inferred.ok_or_else(|| format!("{}:{error}", path.display()))?
            }
        };
        drop(source);
        let id = format!("{owner}:{name}");
        if entry_ids.contains(&id) {
            return Err(format!("duplicate entry source: {}", entry.display()));
        }
        entry_ids.push(id);
        loader.visit(owner.clone(), name, path, false)?;
    }
    let entry = entry_ids.first().ok_or("make needs at least one Elm source")?.clone();
    let import_errors = loader.import_reports()?;
    if !recover_lexical && !import_errors.is_empty() {
        return Err(crate::docs_diagnostic::encode(&serde_json::json!({"type":"compile-errors","errors":import_errors})));
    }
    if runtime {
        loader.runtime_dependencies()?;
    }
    if let Some(profile) = &loader.profile { profile.emit(); }
    Ok(Graph {
        import_errors,
        entries: entry_ids,
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
/// Complete a single-entry source snapshot with kernel runtime dependencies.
/// Reuses the already-read source/manifests instead of rediscovering them.
pub fn complete_runtime(mut graph: Graph) -> Result<Graph, String> {
    if graph.entries.len() != 1 {
        return Err("runtime completion requires one entry".into());
    }
    let mut packages = BTreeMap::new();
    for (path, source) in &graph.manifests {
        let config = crate::outline::decode(source)?;
        let root = path.parent().ok_or("missing manifest directory")?;
        let (owner, package) = if config["type"] == "application" {
            let roots = config["source-directories"].as_array()
                .ok_or("missing source-directories")?.iter().map(|value| {
                    let name = value.as_str().ok_or("invalid source-directory")?;
                    snapshot::canonicalize(&root.join(name)).map_err(|error| error.to_string())
                }).collect::<Result<Vec<_>, String>>()?;
            ("application".to_owned(), Package {
                root: root.to_path_buf(),
                roots, exposed: BTreeSet::new(),
                dependencies: names(&config["dependencies"]["direct"])?,
            })
        } else {
            let owner = config["name"].as_str().ok_or("missing package name")?.to_owned();
            (owner, Package {
                root: root.to_path_buf(),
                roots: vec![root.join("src")],
                exposed: exposed(&config["exposed-modules"])?,
                dependencies: names(&config["dependencies"])?,
            })
        };
        packages.insert(owner, package);
    }
    let mut loader = Loader {
        profile: DiscoveryProfile::enabled(),
        resolution: snapshot::resolution(&packages),
        candidates: BTreeMap::new(),
        import_problems: Vec::new(),
        recover_lexical: true,
        packages,
        visiting: BTreeSet::new(),
        done: graph.modules.iter().map(|m| format!("{}:{}", m.owner, m.name)).collect(),
        modules: std::mem::take(&mut graph.modules),
    };
    loader.runtime_dependencies()?;
    if let Some(profile) = &loader.profile { profile.emit(); }
    graph.modules = loader.modules;
    Ok(graph)
}

fn discover_many_with_runtime(
    manifest: &Path,
    entries: &[PathBuf],
    elm_home: &Path,
    runtime: bool,
    selected_packages: Option<&BTreeMap<String, String>>,
    recover_lexical: bool,
) -> Result<Graph, String> {
    discover_entries_with_runtime(manifest, entries, elm_home, runtime, selected_packages, recover_lexical)
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
#[cfg(test)]
fn discovery_header(source: &str, recover_lexical: bool) -> Result<(std::rc::Rc<crate::module::Header>, usize, bool), String> {
    discovery_header_at(None, source, recover_lexical)
}
fn discovery_header_at(
    path: Option<&Path>,
    source: &str,
    recover_lexical: bool,
) -> Result<(std::rc::Rc<crate::module::Header>, usize, bool), String> {
    crate::session_cache::discovery_header(source, recover_lexical, path.and_then(|p| snapshot::source_digest(p, source)), || {
        let (tokens, lexical_error) = crate::lexer::lex_prefix(source);
        let parsed = header(source, &tokens);
        match (parsed, lexical_error) {
            (Ok(header), None) => Ok((std::rc::Rc::new(header), tokens.len(), false)),
            (Ok(header), Some(_)) if recover_lexical && header.body_start < tokens.len() => {
                Ok((std::rc::Rc::new(header), tokens.len(), true))
            }
            (_, Some(error)) => Err(crate::module::prefer_header_error(source, &tokens, error)),
            (Err(error), None) => Err(error),
        }
    })
}

/// Only use a unique identity implied by configured source directories.
fn entry_name_from_path(path: &Path, roots: &[PathBuf]) -> Option<String> {
    let path = snapshot::canonicalize(path).ok()?;
    let mut names = BTreeSet::new();
    for root in roots {
        let root = snapshot::canonicalize(root).ok()?;
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
    fn cached_headers_preserve_full_source_errors_and_recovery_policy() {
        let _session = crate::session_cache::scope(true);
        let valid = "module A exposing (..)\nimport B exposing (value)\nvalue = 1\n";
        let broken = "module A exposing (..)\nimport B exposing (value)\nvalue = \"unfinished";
        for source in [valid, broken, "module A exposing (\"unfinished", valid] {
            for recover in [true, false, true, false] {
                let expected = {
                    let _disabled = crate::session_cache::scope(false);
                    discovery_header(source, recover)
                };
                assert_eq!(discovery_header(source, recover), expected);
                assert_eq!(discovery_header(source, recover), expected);
            }
        }
        assert!(discovery_header(broken, true).unwrap().2);
        assert!(discovery_header(broken, false).is_err());
        let renamed = valid.replace("import B", "import C");
        assert_eq!(discovery_header(&renamed, true).unwrap().0.imports[0].name, "C");
        let stats = crate::session_cache::statistics();
        assert!(stats["hits"][3].as_u64().unwrap() > 0);
        assert!(stats["hits"][4].as_u64().unwrap() > 0);
    }

    #[test]
    fn discovery_reuses_immutable_header_allocations() {
        let _session = crate::session_cache::scope(true);
        let source = "module A exposing (value)\nimport B as Alias exposing (Thing(..), helper)\nvalue = 1\n";
        let first = discovery_header(source, true).unwrap().0;
        let second = discovery_header(source, true).unwrap().0;
        assert!(std::rc::Rc::ptr_eq(&first, &second));
        assert_eq!(second.imports[0].alias.as_deref(), Some("Alias"));
        let changed = discovery_header(&source.replace("import B", "import C"), true).unwrap().0;
        assert!(!std::rc::Rc::ptr_eq(&first, &changed));
        assert_eq!(first.imports[0].name, "B");
        assert_eq!(changed.imports[0].name, "C");
    }

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
