use planexpo_elm::{
    kernel::Mode,
    project::{Graph, Module},
    typed_cache::TypeCache,
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "elm-typed-cache-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn module(name: &str, imports: &[&str], kernel: bool) -> Module {
    Module {
        missing_header: None,
        owner: "test".into(),
        name: name.into(),
        path: format!("/src/{name}").into(),
        imports: imports.iter().map(|s| s.to_string()).collect(),
        bytes: 1,
        tokens: 1,
        kernel,
        source: "x".into(),
    }
}
fn graph() -> Graph {
    Graph {
        import_errors: Vec::new(),
        entry: "test:C".into(),
        entries: vec!["test:C".into()],
        manifests: vec![("/elm.json".into(), "manifest".into())],
        modules: vec![
            module("A", &["test:Kernel"], false),
            module("B", &["test:A"], false),
            module("C", &["test:B"], false),
            module("D", &[], false),
            module("Kernel", &["test:A"], true),
        ],
    }
}
#[test]
fn interface_keys_reuse_dependents_after_implementation_edits_but_not_api_edits() {
    use std::collections::BTreeMap;
    let dir = Directory::new();
    let signatures = BTreeMap::from([("test:A".into(), [1; 32]), ("test:B".into(), [2; 32])]);
    let original = TypeCache::new(&dir.0, &graph(), b"compiler", Mode::Development).unwrap();
    for module in ["test:A", "test:B", "test:C"] {
        original
            .for_interfaces(module, &signatures)
            .unwrap()
            .store(b"artifact")
            .unwrap();
    }
    let mut edited = graph();
    edited.modules[0].source = "new implementation".into();
    let cache = TypeCache::new(&dir.0, &edited, b"compiler", Mode::Development).unwrap();
    assert!(
        cache
            .for_interfaces("test:A", &signatures)
            .unwrap()
            .load()
            .is_none()
    );
    for module in ["test:B", "test:C"] {
        assert_eq!(
            cache
                .for_interfaces(module, &signatures)
                .unwrap()
                .load()
                .unwrap(),
            b"artifact"
        );
    }
    let mut changed_types = signatures.clone();
    changed_types.insert("test:A".into(), [3; 32]);
    assert!(
        cache
            .for_interfaces("test:B", &changed_types)
            .unwrap()
            .load()
            .is_none()
    );
    // B can keep its public interface after rechecking, so C remains reusable.
    assert!(
        cache
            .for_interfaces("test:C", &changed_types)
            .unwrap()
            .load()
            .is_some()
    );
    changed_types.insert("test:B".into(), [4; 32]);
    assert!(
        cache
            .for_interfaces("test:C", &changed_types)
            .unwrap()
            .load()
            .is_none()
    );
    // Without a portable signature, transitive source keys provide a fallback.
    let empty = BTreeMap::new();
    original
        .for_interfaces("test:C", &empty)
        .unwrap()
        .store(b"fallback")
        .unwrap();
    assert!(
        cache
            .for_interfaces("test:C", &empty)
            .unwrap()
            .load()
            .is_none()
    );
}
#[test]
fn transitive_changes_invalidate_dependents_but_keep_unrelated_modules() {
    let dir = Directory::new();
    let original = TypeCache::new(&dir.0, &graph(), b"compiler", Mode::Development).unwrap();
    for name in ["A", "B", "C", "D"] {
        original
            .store(&format!("test:{name}"), &json!({"name":name}))
            .unwrap();
    }
    let mut changed = graph();
    changed.modules[0].source = "y".into();
    let cache = TypeCache::new(&dir.0, &changed, b"compiler", Mode::Development).unwrap();
    for name in ["A", "B", "C"] {
        assert!(cache.load(&format!("test:{name}")).is_none());
    }
    assert_eq!(cache.load("test:D").unwrap(), json!({"name":"D"}));
    changed = graph();
    changed.modules.swap(2, 3);
    let reordered = TypeCache::new(&dir.0, &changed, b"compiler", Mode::Development).unwrap();
    for name in ["A", "B", "C", "D"] {
        assert!(reordered.load(&format!("test:{name}")).is_some());
    }
    cache.store("test:A", &json!({"changed":true})).unwrap();
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 4);
}
#[test]
fn compiler_manifest_kernel_resolution_and_mode_changes_cannot_reuse_stale_types() {
    let dir = Directory::new();
    let cache = TypeCache::new(&dir.0, &graph(), b"compiler", Mode::Development).unwrap();
    cache.store("test:C", &json!({"ok":true})).unwrap();
    for change in 0..8 {
        let mut graph = graph();
        let mut compiler = b"compiler".as_slice();
        let mut mode = Mode::Development;
        match change {
            0 => compiler = b"new compiler",
            1 => graph.manifests[0].1.push(' '),
            2 => graph.modules[4].source.push(' '),
            3 => mode = Mode::Production,
            7 => mode = Mode::Debug,
            4 => graph.modules[0].path = "/other/A".into(),
            5 => graph.modules[1].imports.push("test:Kernel".into()),
            _ => graph.manifests[0].0 = "/other/elm.json".into(),
        }
        assert!(
            TypeCache::new(&dir.0, &graph, compiler, mode)
                .unwrap()
                .load("test:C")
                .is_none(),
            "change {change}"
        );
    }
    assert!(cache.load("test:C").is_some());
    let mut invalid = graph();
    invalid.modules.swap(0, 1);
    assert!(TypeCache::new(&dir.0, &invalid, b"compiler", Mode::Development).is_err());
}
#[test]
fn truncated_and_corrupt_artifacts_are_cache_misses() {
    let dir = Directory::new();
    let cache = TypeCache::new(&dir.0, &graph(), b"compiler", Mode::Development).unwrap();
    cache.store("test:A", &json!({"ok":true})).unwrap();
    let path = fs::read_dir(&dir.0)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut bytes = fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::write(&path, &bytes).unwrap();
    assert!(cache.load("test:A").is_none());
    fs::write(&path, &bytes[..10]).unwrap();
    assert!(cache.load("test:A").is_none());
    assert!(cache.load("test:Unknown").is_none());
    assert!(cache.store("test:Unknown", &json!({})).is_err());
}

#[test]
fn generated_cache_invalidates_transitive_edits_and_preserves_independent_modules() {
    let dir = Directory::new();
    let original = TypeCache::new(&dir.0, &graph(), b"compiler", Mode::Development).unwrap();
    for name in ["test:A", "test:B", "test:C", "test:D"] {
        original.generated(name).unwrap().store(b"generated").unwrap();
    }
    let mut edited = graph();
    edited.modules[0].source = "new implementation".into();
    let changed = TypeCache::new(&dir.0, &edited, b"compiler", Mode::Development).unwrap();
    for name in ["test:A", "test:B", "test:C"] { assert!(changed.generated(name).unwrap().load().is_none()); }
    assert_eq!(changed.generated("test:D").unwrap().load(), Some(b"generated".to_vec()));
    assert!(TypeCache::new(&dir.0, &graph(), b"compiler", Mode::Production).unwrap().generated("test:C").is_none());
    assert!(TypeCache::new(&dir.0, &graph(), b"new compiler", Mode::Development).unwrap().generated("test:D").unwrap().load().is_none());
}

#[test]
fn diagnostic_types_are_isolated_and_invalidate_on_transitive_source_edits() {
    use std::collections::BTreeMap;
    let dir = Directory::new();
    let signatures = BTreeMap::from([("test:A".into(), [1; 32]), ("test:B".into(), [2; 32])]);
    let normal = TypeCache::new(&dir.0, &graph(), b"compiler", Mode::Development).unwrap();
    let diagnostics = normal.for_diagnostics();
    normal.for_interfaces("test:C", &signatures).unwrap().store(b"compact types").unwrap();
    assert!(diagnostics.for_interfaces("test:C", &signatures).unwrap().load().is_none());
    for module in ["test:A", "test:B", "test:C", "test:D"] {
        diagnostics.for_interfaces(module, &signatures).unwrap().store(b"types with display names").unwrap();
    }
    assert_eq!(normal.for_interfaces("test:C", &signatures).unwrap().load().unwrap(), b"compact types");
    let mut edited = graph();
    // Public types may be equal while source variable/alias names in diagnostics differ.
    edited.modules[0].source = "same signature, different source names".into();
    let after = TypeCache::new(&dir.0, &edited, b"compiler", Mode::Development).unwrap().for_diagnostics();
    for module in ["test:A", "test:B", "test:C"] {
        assert!(after.for_interfaces(module, &signatures).unwrap().load().is_none(), "{module}");
    }
    assert_eq!(after.for_interfaces("test:D", &signatures).unwrap().load().unwrap(), b"types with display names");
    assert!(after.generated("test:D").is_none());
}

#[test]
fn generated_interface_cache_reuses_bodies_but_tracks_operator_targets() {
    use planexpo_elm::names::{Interface, Space, SymbolKind, Symbols};
    use std::{collections::BTreeMap, rc::Rc};
    let dir = Directory::new();
    let signatures = BTreeMap::from([("test:A".into(), [1; 32]), ("test:B".into(), [2; 32])]);
    let mut symbols = Symbols::default();
    let choose = symbols.intern("test:A", "choose", Space::Value, SymbolKind::Value);
    let select = symbols.intern("test:A", "select", Space::Value, SymbolKind::Value);
    let mut names = BTreeMap::from([
        ("test:A".into(), Rc::new(Interface::default())),
        ("test:B".into(), Rc::new(Interface::default())),
    ]);
    Rc::make_mut(names.get_mut("test:A").unwrap()).operators.insert("|=".into(), choose);
    let original = TypeCache::new(&dir.0, &graph(), b"compiler", Mode::Development).unwrap();
    original.generated_for_interfaces("test:B", &signatures, &names, &symbols).unwrap().store(b"javascript").unwrap();
    let mut edited = graph(); edited.modules[0].source = "different body".into();
    let changed = TypeCache::new(&dir.0, &edited, b"compiler", Mode::Development).unwrap();
    assert_eq!(changed.generated_for_interfaces("test:B", &signatures, &names, &symbols).unwrap().load(), Some(b"javascript".to_vec()));
    // Both private implementations still exist and have the same type. The
    // public operator now refers to the other implementation, so remapping
    // dependency IDs alone would silently keep the wrong JavaScript reference.
    Rc::make_mut(names.get_mut("test:A").unwrap()).operators.insert("|=".into(), select);
    assert!(changed.generated_for_interfaces("test:B", &signatures, &names, &symbols).unwrap().load().is_none());
    Rc::make_mut(names.get_mut("test:A").unwrap()).operators.insert("|=".into(), choose);
    let mut changed_types = signatures.clone(); changed_types.insert("test:A".into(), [3; 32]);
    assert!(changed.generated_for_interfaces("test:B", &changed_types, &names, &symbols).unwrap().load().is_none());
    edited.modules[1].source = "new own body".into();
    let own = TypeCache::new(&dir.0, &edited, b"compiler", Mode::Development).unwrap();
    assert!(own.generated_for_interfaces("test:B", &signatures, &names, &symbols).unwrap().load().is_none());
    for mode in [Mode::Debug, Mode::Production] {
        assert!(TypeCache::new(&dir.0, &graph(), b"compiler", mode).unwrap().generated_for_interfaces("test:B", &signatures, &names, &symbols).is_none());
    }
    assert!(original.for_diagnostics().generated_for_interfaces("test:B", &signatures, &names, &symbols).is_none());
    let missing = BTreeMap::new();
    original.generated_for_interfaces("test:B", &missing, &names, &symbols).unwrap().store(b"fallback").unwrap();
    assert!(changed.generated_for_interfaces("test:B", &missing, &names, &symbols).unwrap().load().is_none());
    // The older conservative slots cannot overwrite the new artifact.
    original.generated("test:B").unwrap().store(b"legacy").unwrap();
    assert_eq!(original.generated_for_interfaces("test:B", &missing, &names, &symbols).unwrap().load(), Some(b"fallback".to_vec()));
}

#[test]
fn shared_source_digests_preserve_transitive_validation_keys() {
    use planexpo_elm::cache::SourceDigests;
    let dir = Directory::new();
    let original = graph();
    let ordinary = TypeCache::new(&dir.0, &original, b"compiler", Mode::Development).unwrap();
    let shared = TypeCache::new_with_digests(&dir.0, &SourceDigests::new(&original), b"compiler", Mode::Development).unwrap();
    for id in ["test:A", "test:B", "test:C", "test:D"] {
        ordinary.generated(id).unwrap().store(id.as_bytes()).unwrap();
        assert_eq!(shared.generated(id).unwrap().load().unwrap(), id.as_bytes());
    }
    for kernel in [false, true] {
        let mut changed = graph();
        changed.modules[if kernel { 4 } else { 0 }].source = "y".into();
        let edited = TypeCache::new_with_digests(&dir.0, &SourceDigests::new(&changed), b"compiler", Mode::Development).unwrap();
        for id in ["test:A", "test:B", "test:C"] {
            assert!(edited.generated(id).unwrap().load().is_none());
        }
        assert_eq!(edited.generated("test:D").unwrap().load().is_some(), !kernel);
    }
}
