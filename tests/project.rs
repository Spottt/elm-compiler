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
    assert!(f.graph().err().unwrap().contains("cannot resolve"));
    f.module("Child", "import Main");
    assert!(f.graph().err().unwrap().contains("cyclic"));
    f.module("Child", "value = 1");
    f.manifest(&["src", "src"], &["elm/core"]);
    assert!(f.graph().err().unwrap().contains("ambiguous"));
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
    assert!(f.graph().err().unwrap().contains("cannot resolve"));
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
        error.starts_with(&format!("{}:2:10:", f.root.join("src/Main.elm").display())),
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
