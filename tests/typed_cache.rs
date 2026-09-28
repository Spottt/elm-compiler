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
    for change in 0..7 {
        let mut graph = graph();
        let mut compiler = b"compiler".as_slice();
        let mut mode = Mode::Development;
        match change {
            0 => compiler = b"new compiler",
            1 => graph.manifests[0].1.push(' '),
            2 => graph.modules[4].source.push(' '),
            3 => mode = Mode::Production,
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
