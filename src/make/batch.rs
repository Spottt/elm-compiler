//! Shared development analysis behind the private worker batch protocol.
use super::*;
use crate::make_worker::Request;
use planexpo_elm::{analyze, cache, linker, project::Graph, typed_cache};
use std::{collections::{BTreeMap, BTreeSet}, path::Path};
pub(super) struct Prepared {
    pub graph: Graph,
    pub output: PathBuf,
    pub manifest: PathBuf,
    pub identity: Vec<u8>,
}
fn ordinary(request: &Request) -> Result<(), String> {
    planexpo_elm::session_cache::set_changed_paths(request.changed.clone());
    super::run_worker(request.arguments.clone(), request.dependencies.as_deref())
}
fn merge(items: &[Prepared]) -> Option<Graph> {
    let first = items.first()?;
    let mut graph = first.graph.clone();
    if !graph.import_errors.is_empty() { return None; }
    let mut modules: BTreeMap<_, _> = graph.modules.iter().enumerate()
        .map(|(i,m)| ((m.owner.clone(),m.name.clone()), i)).collect();
    for item in &items[1..] {
        if item.manifest != first.manifest || item.identity != first.identity
            || item.graph.manifests != first.graph.manifests || !item.graph.import_errors.is_empty() { return None; }
        for entry in &item.graph.entries { if !graph.entries.contains(entry) { graph.entries.push(entry.clone()); } }
        for module in &item.graph.modules {
            let key = (module.owner.clone(), module.name.clone());
            if let Some(index) = modules.get(&key) {
                let old = &graph.modules[*index];
                if old.source != module.source || old.path != module.path || old.kernel != module.kernel
                    || old.imports != module.imports || old.missing_header != module.missing_header { return None; }
            } else {
                modules.insert(key, graph.modules.len()); graph.modules.push(module.clone());
            }
        }
    }
    Some(graph)
}
fn shared(items: &[Prepared]) -> Option<Result<analyze::Report, String>> {
    let graph = merge(items)?;
    let first = &items[0];
    let directory = first.manifest.parent()?.join("elm-stuff/planexpo-rust/types-v1");
    let types = typed_cache::TypeCache::new(&directory, &graph, &first.identity, Mode::Development).ok()?;
    Some(analyze::generate_modules_cached(&graph, Mode::Development, &types))
}
// After a shared source error, keep each entry's diagnostic scope without
// repeating filesystem discovery, output-cache probes and package validation.
fn independent(item: &Prepared, profile: bool, entries: &[PathBuf]) -> Result<(), String> {
    let parent = item.manifest.parent().ok_or("missing manifest directory")?;
    let sources = cache::SourceDigests::new(&item.graph);
    let types = typed_cache::TypeCache::new_with_digests(&parent.join("elm-stuff/planexpo-rust/types-v1"), &sources, &item.identity, Mode::Development);
    let result = match types {
        Ok(types) => analyze::generate_modules_cached(&item.graph, Mode::Development, &types),
        Err(_) => analyze::generate_modules_in_mode(&item.graph, Mode::Development),
    };
    let report = remember_analysis(&item.graph, result, true).inspect_err(|error| {
        if planexpo_elm::docs_diagnostic::report_encoded(error).is_some_and(|r| r["type"] == "compile-errors") {
            let cache = cache::OutputCache::new_with_digests(&parent.join("elm-stuff/planexpo-rust/v1"), &sources, &item.identity, Mode::Development);
            let _ = cache.diagnostic_slot().store(error.as_bytes());
        }
    })?;
    finish(item, &report, profile, entries)
}
// Shared analysis completes error collection over the merged dependency graph.
// Only reuse located source diagnostics; unlocated/project failures keep the
// independent path. A clean entry still needs independent successful analysis.
fn unambiguous_paths(items: &[Prepared]) -> BTreeSet<PathBuf> {
    let mut owners = BTreeMap::new(); let mut ambiguous = BTreeSet::new();
    for module in items.iter().flat_map(|item| &item.graph.modules) {
        let path = module.path.as_path(); let identity = (module.owner.as_str(), module.name.as_str());
        if owners.insert(path, identity).is_some_and(|previous| previous != identity) { ambiguous.insert(path); }
    }
    owners.keys().filter(|path| !ambiguous.contains(**path)).map(|path| path.to_path_buf()).collect()
}
fn scoped_failure(report: &serde_json::Value, graph: &Graph, known_paths: &BTreeSet<PathBuf>) -> Option<String> {
    if report["type"] != "compile-errors" { return None; }
    let errors = report["errors"].as_array()?;
    if errors.is_empty() || errors.iter().any(|error| error["path"].as_str().is_none_or(|path| !known_paths.contains(Path::new(path)))) { return None; }
    let paths: BTreeSet<_> = graph.modules.iter().map(|module| module.path.as_path()).collect();
    let selected: Vec<_> = errors.iter().filter(|error| paths.contains(Path::new(error["path"].as_str().unwrap()))).cloned().collect();
    if selected.is_empty() { return None; }
    let mut scoped = report.clone(); scoped["errors"] = selected.into();
    Some(planexpo_elm::docs_diagnostic::encode(&scoped))
}
fn finish_failure(item: &Prepared, error: String) -> Result<(), String> {
    let _ = remember_analysis(&item.graph, Err(error.clone()), true);
    if let Some(parent) = item.manifest.parent() {
        let sources = cache::SourceDigests::new(&item.graph);
        let output = cache::OutputCache::new_with_digests(&parent.join("elm-stuff/planexpo-rust/v1"), &sources, &item.identity, Mode::Development);
        let _ = output.diagnostic_slot().store(error.as_bytes());
    }
    Err(error)
}
fn finish(item: &Prepared, report: &analyze::Report, profile: bool, entries: &[PathBuf]) -> Result<(), String> {
    let bytes = profile_phase(profile, entries, "batch_link", || linker::assemble_entry_shared(&item.graph, report))?.into_bytes();
    profile_phase(profile, entries, "batch_import_history", || remember_import_names(&item.graph, &[]));
    let directory = item.manifest.parent().ok_or("missing manifest directory")?.join("elm-stuff/planexpo-rust/v1");
    let sources = cache::SourceDigests::new(&item.graph);
    let output = cache::OutputCache::new_with_digests(&directory, &sources, &item.identity, Mode::Development);
    let trivia = (env::var("PLANEXPO_ELM_TRIVIA_CACHE").as_deref() == Ok("1"))
        .then(|| cache::OutputCache::new_for_trivia_with_digests(&directory, &sources, &item.identity)).flatten();
    // As with ordinary make, unavailable disposable caches do not fail a build.
    let _ = profile_phase(profile, entries, "batch_cache_store", || output.store_with_trivia(&bytes, trivia.as_ref()));
    if let Some(parent) = item.output.parent().filter(|p| !p.as_os_str().is_empty()) {
        create_output_directory(parent).map_err(|(path, error)| crate::diagnostic::output_io_error(&path, "createDirectory", error))?;
    }
    profile_phase(profile, entries, "batch_output_write", || fs::write(&item.output, bytes)).map_err(|error| crate::diagnostic::output_io_error(&item.output, "openBinaryFile", error))
}
fn eligible(requests: &[Request]) -> bool {
    let Ok(cwd) = env::current_dir() else { return false; };
    let mut destinations = BTreeSet::new();
    for request in requests {
        let Ok(options) = Options::parse(request.arguments.clone()) else { return false; };
        if !options.incremental || !options.cache || options.mode != Mode::Development
            || options.docs.is_some() || options.entries.len() != 1 || options.clashing
            || options.output.extension().is_none_or(|ext| ext != "js") { return false; }
        let output = cwd.join(&options.output);
        let Some(parent) = output.parent().and_then(|p| p.canonicalize().ok()) else { return false; };
        let Some(name) = output.file_name() else { return false; };
        let output = output.canonicalize().unwrap_or_else(|_| parent.join(name));
        if !destinations.insert(output) { return false; }
    }
    true
}
pub(crate) fn run(requests: &[Request]) -> (Vec<Result<(), String>>, usize) {
    run_stream(requests, &mut |_, _| Ok(())).expect("infallible batch observer")
}
pub(crate) fn run_stream(requests: &[Request], ready: &mut impl FnMut(usize, &Result<(), String>) -> Result<(), String>) -> Result<(Vec<Result<(), String>>, usize), String> {
    if requests.len() == 1 || !eligible(requests) {
        let mut results = Vec::with_capacity(requests.len());
        for (index, request) in requests.iter().enumerate() {
            let result = ordinary(request); ready(index, &result)?; results.push(result);
        }
        return Ok((results, 0));
    }
    let _snapshot = planexpo_elm::project::snapshot_scope();
    // Preflight and shared analysis are one logical request for cache aging.
    // These hints only place a checkpoint; fresh graph bytes still decide reuse.
    let changed: BTreeSet<_> = requests.iter().flat_map(|request| request.changed.iter().cloned()).collect();
    planexpo_elm::session_cache::set_changed_paths(changed.into_iter().collect());
    let profile = env::var("PLANEXPO_ELM_PROFILE_MAKE").as_deref() == Ok("1");
    let entries: Vec<PathBuf> = if profile { requests.iter().flat_map(|r| Options::parse(r.arguments.clone()).into_iter().flat_map(|o| o.entries)).collect() } else { Vec::new() };
    let mut results = Vec::with_capacity(requests.len());
    let mut prepared = Vec::new();
    let mut indices = Vec::new();
    for (index, request) in requests.iter().enumerate() {
        let mut item = None;
        results.push(profile_phase(profile, &entries, "batch_preflight", || run_inner(request.arguments.clone(), &mut ProgressLine(false), true, request.dependencies.as_deref(), Some(&mut item))));
        if let Some(item) = item { indices.push(index); prepared.push(item); }
        else { ready(index, &results[index])?; }
    }
    let mut shared_entries = 0;
    let report = if !prepared.is_empty() {
        profile_phase(profile, &entries, "batch_shared_analysis", || shared(&prepared))
    } else { None };
    let reusable = matches!(report, Some(Err(_)));
    let shared_failure = report.as_ref().and_then(|result| result.as_ref().err()).map(|error| crate::diagnostic::report(error));
    let known_paths = if shared_failure.is_some() { unambiguous_paths(&prepared) } else { BTreeSet::new() };
    if let Some(Ok(report)) = report {
        shared_entries = if prepared.len() > 1 { prepared.len() } else { 0 };
        for (index, item) in indices.into_iter().zip(prepared) {
            results[index] = profile_phase(profile, &entries, "batch_finish", || finish(&item, &report, profile, &entries));
            ready(index, &results[index])?;
        }
    } else {
        // Inconsistent snapshots still use ordinary make. Valid prepared graphs
        // can collect independent diagnostics directly after shared failure.
        for (index, item) in indices.into_iter().zip(prepared) {
            results[index] = profile_phase(profile, &entries, "batch_fallback", || {
                if let Some(error) = shared_failure.as_ref().and_then(|report| scoped_failure(report, &item.graph, &known_paths)) {
                    finish_failure(&item, error)
                } else if reusable { independent(&item, profile, &entries) } else { ordinary(&requests[index]) }
            });
            ready(index, &results[index])?;
        }
    }
    Ok((results, shared_entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use planexpo_elm::project::Module;
    fn prepared(entry: &str) -> Prepared {
        Prepared { manifest: "elm.json".into(), output: format!("{entry}.js").into(), identity: vec![1], graph: Graph {
            entry: format!("app:{entry}"), entries: vec![format!("app:{entry}")], manifests: vec![("elm.json".into(), "manifest".into())], import_errors: vec![],
            modules: ["Shared", entry].into_iter().map(|name| Module { owner:"app".into(), name:name.into(), source:format!("source {name}").into(), path:format!("src/{name}.elm").into(), imports:vec![], missing_header:None, kernel:false, bytes:0, tokens:0 }).collect(),
        } }
    }
    #[test]
    fn shared_analysis_requires_identical_common_source_snapshots() {
        let graph = merge(&[prepared("A"), prepared("B")]).unwrap();
        assert_eq!(graph.entries, ["app:A", "app:B"]);
        assert_eq!(graph.modules.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Shared", "A", "B"]);
        for change in 0..7 {
            let mut other = prepared("B");
            match change {
                0 => other.graph.modules[0].source.push('!'),
                1 => other.graph.modules[0].path = "moved/Shared.elm".into(),
                2 => other.graph.modules[0].imports.push("app:Other".into()),
                3 => other.graph.modules[0].kernel = true,
                4 => other.graph.modules[0].missing_header = Some("Shared".into()),
                5 => other.graph.manifests[0].1.push('!'),
                _ => other.identity.push(2),
            }
            assert!(merge(&[prepared("A"), other]).is_none(), "change {change}");
        }
    }
    #[test]
    fn shared_diagnostics_are_scoped_and_unknown_failures_fall_back() {
        let a = prepared("A"); let b = prepared("B");
        let known = merge(&[prepared("A"), prepared("B")]).unwrap().modules.into_iter().map(|module| module.path).collect();
        let report = serde_json::json!({"type":"compile-errors","errors":[
            {"path":"src/A.elm","name":"A","problems":[{"title":"A ERROR"}]},
            {"path":"src/Shared.elm","name":"Shared","problems":[{"title":"SHARED ERROR"}]}]});
        let a_report = planexpo_elm::docs_diagnostic::report_encoded(&scoped_failure(&report, &a.graph, &known).unwrap()).unwrap();
        let b_report = planexpo_elm::docs_diagnostic::report_encoded(&scoped_failure(&report, &b.graph, &known).unwrap()).unwrap();
        assert_eq!(a_report["errors"].as_array().unwrap().len(), 2);
        assert_eq!(b_report["errors"].as_array().unwrap().len(), 1);
        assert_eq!(b_report["errors"][0]["name"], "Shared");
        let mut only_a = report.clone(); only_a["errors"].as_array_mut().unwrap().pop();
        assert!(scoped_failure(&only_a, &b.graph, &known).is_none());
        for invalid in [serde_json::json!({"type":"error"}), serde_json::json!({"type":"compile-errors","errors":[]}), serde_json::json!({"type":"compile-errors","errors":[{"path":"unknown.elm"}]}), serde_json::json!({"type":"compile-errors","errors":[{}]})] {
            assert!(scoped_failure(&invalid, &a.graph, &known).is_none());
        }
    }

    #[test]
    fn source_paths_shared_by_distinct_package_identities_are_not_reused() {
        let mut other = prepared("B"); other.graph.modules[0].owner = "different/package".into();
        let known = unambiguous_paths(&[prepared("A"), other]);
        assert!(!known.contains(Path::new("src/Shared.elm")));
        assert!(known.contains(Path::new("src/A.elm")));
        assert!(unambiguous_paths(&[prepared("A"), prepared("B")]).contains(Path::new("src/Shared.elm")));
    }

}
