use planexpo_elm::project::discover;
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    home: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "elm-rs-graph-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        // macOS temp directories may be reached through /var -> /private/var.
        let root = root.canonicalize().unwrap();
        let home = root.join("cache");
        let f = Self { root, home };
        let defaults = [
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
        f.package("elm/core", &defaults, &[]);
        for name in defaults {
            f.package_module("elm/core", name, "stub = ()");
        }
        f.manifest(&["src"], &["elm/core"]);
        f
    }
    fn manifest(&self, roots: &[&str], direct: &[&str]) {
        let deps: serde_json::Map<String, serde_json::Value> = direct
            .iter()
            .map(|n| (n.to_string(), json!("1.0.0")))
            .collect();
        fs::write(self.root.join("elm.json"), json!({"type":"application","elm-version":"0.19.1","source-directories":roots,"dependencies":{"direct":deps,"indirect":{}}}).to_string()).unwrap();
    }
    fn package(&self, name: &str, exposed: &[&str], dependencies: &[&str]) {
        let root = self.home.join("0.19.1/packages").join(name).join("1.0.0");
        fs::create_dir_all(root.join("src")).unwrap();
        let deps: serde_json::Map<String, serde_json::Value> = dependencies
            .iter()
            .map(|n| (n.to_string(), json!("1.0.0 <= v < 2.0.0")))
            .collect();
        fs::write(
            root.join("elm.json"),
            json!({"type":"package","name":name,"summary":"Test package","license":"BSD-3-Clause","version":"1.0.0","test-dependencies":{},"elm-version":"0.19.0 <= v < 0.20.0","exposed-modules":exposed,"dependencies":deps}).to_string(),
        )
        .unwrap();
    }
    fn package_module(&self, package: &str, name: &str, body: &str) {
        let path = self
            .home
            .join("0.19.1/packages")
            .join(package)
            .join("1.0.0/src")
            .join(name.replace('.', "/") + ".elm");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("module {name} exposing (..)\n{body}\n")).unwrap();
    }
    fn module(&self, name: &str, body: &str) {
        fs::write(
            self.root.join("src").join(format!("{name}.elm")),
            format!("module {name} exposing (..)\n{body}\n"),
        )
        .unwrap();
    }
    fn graph(&self) -> Result<planexpo_elm::project::Graph, String> {
        discover(
            &self.root.join("elm.json"),
            std::path::Path::new("src/Main.elm"),
            &self.home,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn graph_keeps_the_source_snapshot_consumed_by_later_compiler_phases() {
    let f = Fixture::new();
    f.package_module(
        "elm/core",
        "List",
        "infix right 5 (::) = cons\ncons a b = b",
    );
    f.module("Main", "answer = ()");
    let before = f.graph().unwrap();
    f.module("Main", "answer = []");
    let after = f.graph().unwrap();
    assert!(
        before
            .modules
            .iter()
            .find(|m| m.name == "Main")
            .unwrap()
            .source
            .contains("answer = ()")
    );
    assert!(
        after
            .modules
            .iter()
            .find(|m| m.name == "Main")
            .unwrap()
            .source
            .contains("answer = []")
    );
    fs::remove_file(f.root.join("src/Main.elm")).unwrap();
    planexpo_elm::analyze::syntax_and_operators(&before).unwrap();
    assert_eq!(before.manifests.len(), 2);
}
#[test]
fn dependencies_precede_importers_and_defaults_are_included_once() {
    let f = Fixture::new();
    f.module("Main", "import Child\nmain = Child.value");
    f.module("Child", "value = 1");
    let g = f.graph().unwrap();
    assert_eq!(g.modules.len(), 13);
    assert_eq!(g.modules.last().unwrap().name, "Main");
    let child = g.modules.iter().position(|m| m.name == "Child").unwrap();
    assert!(child < g.modules.len() - 1);
    assert_eq!(g.modules.last().unwrap().imports.len(), 12);
}
#[test]
fn rejects_cycles_missing_modules_and_duplicate_roots() {
    let f = Fixture::new();
    f.module("Main", "import Child");
    let report = planexpo_elm::docs_diagnostic::report_encoded(&f.graph().err().unwrap()).unwrap();
    assert_eq!(report["errors"][0]["problems"][0]["title"], "MODULE NOT FOUND");
    f.module("Child", "import Main");
    assert!(f.graph().err().unwrap().contains("cyclic"));
    f.module("Child", "value = 1");
    f.manifest(&["src", "src"], &["elm/core"]);
    let report = planexpo_elm::docs_diagnostic::report_encoded(&f.graph().err().unwrap()).unwrap();
    assert_eq!(report["errors"][0]["problems"][0]["title"], "AMBIGUOUS IMPORT");
}
#[test]
fn package_private_modules_are_visible_only_within_package() {
    let f = Fixture::new();
    f.package("author/pkg", &["Public"], &["elm/core"]);
    f.package_module(
        "author/pkg",
        "Public",
        "import Private\nvalue = Private.value",
    );
    f.package_module("author/pkg", "Private", "value = 1");
    f.manifest(&["src"], &["elm/core", "author/pkg"]);
    f.module("Main", "import Public");
    assert!(
        f.graph()
            .unwrap()
            .modules
            .iter()
            .any(|m| m.name == "Private")
    );
    f.module("Main", "import Private");
    let report = planexpo_elm::docs_diagnostic::report_encoded(&f.graph().err().unwrap()).unwrap();
    assert_eq!(report["errors"][0]["problems"][0]["title"], "MODULE NOT FOUND");
}

#[test]
fn runtime_kernel_cycles_and_elm_back_edges_are_discovered_once() {
    let f = Fixture::new();
    f.module("Main", "value = ()");
    f.package_module("elm/core", "Basics", "import Elm.Kernel.First");
    f.package_module("elm/core", "Hidden", "import Basics\nvalue=()");
    let kernel = f.home.join("0.19.1/packages/elm/core/1.0.0/src/Elm/Kernel");
    fs::create_dir_all(&kernel).unwrap();
    fs::write(kernel.join("First.js"), "/*\nimport Elm.Kernel.Second exposing (run)\nimport Hidden exposing (value)\n*/\nvar _First_run = __Second_run;").unwrap();
    fs::write(kernel.join("Second.js"), "/*\nimport Elm.Kernel.First exposing (run)\n*/\nvar _Second_run = function() { return __First_run(); };").unwrap();
    let graph = f.graph().unwrap();
    let checked = planexpo_elm::project::discover_many_for_check(
        &f.root.join("elm.json"), &[f.root.join("src/Main.elm")], &f.home,
    ).unwrap();
    let snapshot = checked.modules.iter().find(|m| m.name == "Main").unwrap().source.clone();
    f.module("Main", "changed = 42");
    let completed = planexpo_elm::project::complete_runtime(checked).unwrap();
    let normalized_modules = |modules: &[planexpo_elm::project::Module]| {
        let mut modules = modules.to_vec();
        for module in &mut modules {
            module.path = module.path.canonicalize().unwrap();
        }
        format!("{modules:?}")
    };
    assert_eq!(normalized_modules(&completed.modules), normalized_modules(&graph.modules));
    assert_eq!(completed.manifests, graph.manifests);
    assert_eq!(completed.entries, graph.entries);
    assert_eq!(completed.modules.iter().find(|m| m.name == "Main").unwrap().source, snapshot);
    assert_eq!(graph.modules.iter().filter(|m| m.kernel).count(), 2);
    assert_eq!(
        graph.modules.iter().filter(|m| m.name == "Hidden").count(),
        1
    );
    let first = graph
        .modules
        .iter()
        .find(|m| m.name == "Elm.Kernel.First")
        .unwrap();
    assert_eq!(
        first.imports,
        ["elm/core:Elm.Kernel.Second", "elm/core:Hidden"]
    );
    let second = graph
        .modules
        .iter()
        .find(|m| m.name == "Elm.Kernel.Second")
        .unwrap();
    assert_eq!(second.imports, ["elm/core:Elm.Kernel.First"]);
}

#[test]
fn multiple_entries_share_dependencies_and_preserve_topological_order() {
    let fixture = Fixture::new();
    fixture.module("Shared", "value = 1");
    fixture.module("Main", "import Shared\nvalue = Shared.value");
    fixture.module("Other", "import Shared\nvalue = Shared.value");
    let entries = vec!["src/Main.elm".into(), "src/Other.elm".into()];
    let graph = planexpo_elm::project::discover_many(
        &fixture.root.join("elm.json"),
        &entries,
        &fixture.home,
    )
    .unwrap();
    assert_eq!(graph.entries, ["application:Main", "application:Other"]);
    let names: Vec<_> = graph
        .modules
        .iter()
        .map(|module| module.name.as_str())
        .collect();
    assert_eq!(names.iter().filter(|name| **name == "Shared").count(), 1);
    let shared = names.iter().position(|name| *name == "Shared").unwrap();
    for entry in ["Main", "Other"] {
        assert!(shared < names.iter().position(|name| *name == entry).unwrap());
    }
    let duplicate = vec!["src/Main.elm".into(), "src/./Main.elm".into()];
    assert!(
        planexpo_elm::project::discover_many(
            &fixture.root.join("elm.json"),
            &duplicate,
            &fixture.home,
        )
        .err()
        .unwrap()
        .contains("duplicate entry")
    );
}

#[test]
fn entry_lexical_errors_keep_the_source_path() {
    let f = Fixture::new();
    f.module("Main", "shader = [glsl||]");
    let error = f.graph().err().unwrap();
    assert!(
        // Canonical path: macOS temporary directories live behind the /var -> /private/var symlink.
        error.starts_with(&format!(
            "{}:2:10:",
            f.root
                .join("src/Main.elm")
                .canonicalize()
                .unwrap()
                .display()
        )),
        "{error}"
    );
}

#[test]
fn package_roots_keep_their_real_owner_and_exposed_entries() {
    let f = Fixture::new();
    f.module("Main", "value = 1");
    fs::write(f.root.join("elm.json"), json!({"type":"package","name":"author/library","elm-version":"0.19.0 <= v < 0.20.0","exposed-modules":{"Public":["Main"]},"dependencies":{"elm/core":"1.0.0 <= v < 2.0.0"},"test-dependencies":{}}).to_string()).unwrap();
    let entries = planexpo_elm::project::exposed_entries(&f.root.join("elm.json")).unwrap();
    assert_eq!(entries, vec![f.root.join("src/Main.elm")]);
    let graph =
        planexpo_elm::project::discover_many_for_check(&f.root.join("elm.json"), &entries, &f.home)
            .unwrap();
    let completed = planexpo_elm::project::complete_runtime(graph.clone()).unwrap();
    let direct = planexpo_elm::project::discover_many(&f.root.join("elm.json"), &entries, &f.home).unwrap();
    assert_eq!(format!("{:?}", completed.modules), format!("{:?}", direct.modules));
    assert_eq!(completed.manifests, direct.manifests);
    assert_eq!(graph.entry, "author/library:Main");
    assert!(
        graph
            .modules
            .iter()
            .any(|m| m.owner == "author/library" && m.name == "Main")
    );
}

#[test]
fn make_recovers_an_invalid_header_using_an_unambiguous_source_path() {
    let f = Fixture::new();
    let path = f.root.join("src/Alpha/Inner.elm");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "module Alpha.Inner exposing (\n").unwrap();
    let graph = planexpo_elm::project::discover_many_selected(
        &f.root.join("elm.json"),
        std::slice::from_ref(&path),
        &f.home,
        false,
        None,
    )
    .unwrap();
    assert!(graph.entries[0].ends_with(":Alpha.Inner"));
    let module = graph.modules.iter().find(|m| m.path == path).unwrap();
    assert_eq!(module.name, "Alpha.Inner");
    assert!(module.imports.is_empty());
    assert!(planexpo_elm::parser::parse(&module.source).is_err());
    assert!(discover(&f.root.join("elm.json"), &path, &f.home).is_err());
    f.manifest(&["src", "src/Alpha"], &["elm/core"]);
    assert!(
        planexpo_elm::project::discover_many_selected(
            &f.root.join("elm.json"),
            &[path],
            &f.home,
            false,
            None,
        )
        .is_err()
    );
}

#[test]
fn discovery_preserves_header_errors_before_later_invalid_whitespace() {
    let f = Fixture::new();
    fs::write(
        f.root.join("src/Main.elm"),
        "module lower exposing (..)\nvalue =\t1",
    )
    .unwrap();
    let error = f.graph().err().unwrap();
    assert!(error.contains("1:8: expected module name"), "{error}");
}

#[test]
fn import_resolution_is_fresh_after_provider_add_remove_and_manifest_edits() {
    for snapshot in [false, true] {
    let f = Fixture::new();
    let graph = || {
        let _snapshot = snapshot.then(planexpo_elm::project::snapshot_scope);
        f.graph()
    };
    let _worker = planexpo_elm::session_cache::scope(true);
    f.module("Shared", "value = 1");
    f.module("Left", "import Shared\nvalue = Shared.value");
    f.module("Right", "import Shared\nvalue = Shared.value");
    f.module("Main", "import Left\nimport Right\nvalue = (Left.value, Right.value)");
    assert!(graph().unwrap().import_errors.is_empty());
    fs::remove_file(f.root.join("src/Shared.elm")).unwrap();
    assert!(graph().err().unwrap().contains("MODULE NOT FOUND"));
    f.module("Shared", "value = 2");
    assert!(graph().unwrap().import_errors.is_empty());
    fs::create_dir_all(f.root.join("other")).unwrap();
    fs::write(f.root.join("other/Shared.elm"), "module Shared exposing (value)\nvalue = 3\n").unwrap();
    f.manifest(&["src", "other"], &["elm/core"]);
    assert!(graph().err().unwrap().contains("AMBIGUOUS IMPORT"));
    f.manifest(&["src"], &["elm/core"]);
    assert!(graph().unwrap().import_errors.is_empty());
    }
}

#[test]
fn separate_entry_discovery_in_a_batch_preserves_graphs_and_project_boundaries() {
    let local = Fixture::new();
    local.module("Shared", "value = 1");
    let foreign = Fixture::new();
    foreign.package("author/provider", &["Shared"], &["elm/core"]);
    foreign.package_module("author/provider", "Shared", "value = 2");
    foreign.manifest(&["src"], &["elm/core", "author/provider"]);
    for f in [&local, &foreign] {
        f.module("Main", "import Shared\nmain = Shared.value");
        f.module("Other", "import Shared\nother = Shared.value");
    }
    let entries = [&local, &foreign].into_iter().flat_map(|f| ["Main", "Other"].map(|name| (f, name))).collect::<Vec<_>>();
    let discover_entry = |(f, name): &(&Fixture, &str)| discover(&f.root.join("elm.json"), &PathBuf::from(format!("src/{name}.elm")), &f.home).unwrap();
    let expected: Vec<_> = entries.iter().map(discover_entry).collect();
    let _scope = planexpo_elm::project::snapshot_scope();
    // Alternating projects and repeating entries exercises shared resolution
    // without changing each graph's dependency order or source ownership.
    for index in [0, 2, 1, 3, 0, 3] {
        let actual = discover_entry(&entries[index]); let expected = &expected[index];
        assert_eq!(actual.entries, expected.entries);
        assert_eq!(actual.manifests, expected.manifests);
        assert_eq!(actual.import_errors, expected.import_errors);
        assert_eq!(format!("{:?}", actual.modules), format!("{:?}", expected.modules));
    }
}

#[test]
fn batch_source_digests_do_not_trust_a_graph_mutated_after_discovery() {
    let f = Fixture::new(); f.module("Main", "main = 1");
    let directory = tempfile::tempdir().unwrap();
    let _scope = planexpo_elm::project::snapshot_scope();
    let original = f.graph().unwrap();
    let cache = planexpo_elm::cache::OutputCache::new(directory.path(), &original, b"compiler");
    cache.store(b"original bundle").unwrap();
    let mut changed = original.clone();
    let main = changed.modules.iter_mut().find(|m| m.name == "Main").unwrap();
    main.source = main.source.replace("main = 1", "main = 2").into();
    assert!(planexpo_elm::cache::OutputCache::new(directory.path(), &changed, b"compiler").load().is_none());
    assert_eq!(cache.load().unwrap(), b"original bundle");
}

#[test]
fn foreign_provider_index_preserves_ambiguities_and_exposure_changes() {
    let f = Fixture::new();
    for name in ["author/a", "author/b"] {
        f.package(name, &["Shared"], &["elm/core"]);
        f.package_module(name, "Shared", "value = 1");
    }
    let _worker = planexpo_elm::session_cache::scope(true);
    f.module("Main", "import Shared\nvalue = Shared.value");
    f.manifest(&["src"], &["elm/core", "author/a", "author/b"]);
    let problem = f.graph().err().unwrap();
    assert!(problem.contains("AMBIGUOUS IMPORT"));
    assert!(problem.contains("author/a") && problem.contains("author/b"));
    // Still present on disk, but private: it must stop being a provider.
    f.package("author/b", &[], &["elm/core"]);
    let graph = f.graph().unwrap();
    let shared = graph.modules.iter().find(|m| m.name == "Shared").unwrap();
    assert_eq!(shared.owner, "author/a");
    // A local provider does not hide the remaining foreign ambiguity.
    f.module("Shared", "value = 2");
    assert!(f.graph().err().unwrap().contains("AMBIGUOUS IMPORT"));
    f.manifest(&["src"], &["elm/core"]);
    assert_eq!(f.graph().unwrap().modules.iter().find(|m| m.name == "Shared").unwrap().owner, "application");
}

#[test]
fn exposed_provider_index_does_not_make_indirect_packages_direct_imports() {
    let f = Fixture::new();
    f.package("author/a", &["Public"], &["elm/core", "author/b"]);
    f.package("author/b", &["Shared"], &["elm/core"]);
    f.package_module("author/a", "Public", "import Shared\nvalue = Shared.value");
    f.package_module("author/b", "Shared", "value = 1");
    fs::write(f.root.join("elm.json"), json!({
        "type":"application", "elm-version":"0.19.1", "source-directories":["src"],
        "dependencies":{"direct":{"elm/core":"1.0.0","author/a":"1.0.0"},"indirect":{"author/b":"1.0.0"}},
        "test-dependencies":{"direct":{},"indirect":{}}
    }).to_string()).unwrap();
    f.module("Main", "import Public\nvalue = Public.value");
    assert!(f.graph().is_ok());
    f.module("Main", "import Public\nimport Shared\nvalue = Shared.value");
    assert!(f.graph().err().unwrap().contains("MODULE NOT FOUND"));
}
