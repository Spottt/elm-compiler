use planexpo_elm::{
    cache::OutputCache,
    project::{Graph, Module},
};
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
            "elm-rust-output-cache-{}-{}",
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
fn graph() -> Graph {
    Graph {
        entry: "application:Main".into(),
        entries: vec!["application:Main".into()],
        manifests: vec![("/project/elm.json".into(), "manifest".into())],
        modules: vec![Module {
            missing_header: None,
            owner: "application".into(),
            name: "Main".into(),
            path: "/project/Main.elm".into(),
            imports: vec!["elm/core:Basics".into()],
            bytes: 3,
            tokens: 1,
            kernel: false,
            source: "abc".into(),
        }],
    }
}
#[test]
fn source_manifest_resolution_and_compiler_changes_invalidate_outputs() {
    let dir = Directory::new();
    let original = graph();
    let cache = OutputCache::new(&dir.0, &original, b"compiler1");
    assert!(cache.load().is_none());
    cache.store(b"javascript").unwrap();
    assert_eq!(cache.load().unwrap(), b"javascript");
    assert!(
        OutputCache::new(&dir.0, &original, b"compiler2")
            .load()
            .is_none()
    );
    for change in 0..8 {
        let mut changed = graph();
        match change {
            0 => changed.modules[0].source = "xyz".into(), // Same byte length.
            1 => changed.manifests[0].1.push(' '),
            2 => changed.modules[0].path = "/other/Main.elm".into(),
            3 => changed.modules[0]
                .imports
                .push("elm/json:Json.Decode".into()),
            4 => changed.modules[0].kernel = true,
            5 => changed.modules[0].owner = "other/package".into(),
            6 => changed.entry = "application:Other".into(),
            _ => changed.entries.push("application:Other".into()),
        }
        assert!(
            OutputCache::new(&dir.0, &changed, b"compiler1")
                .load()
                .is_none(),
            "change {change}"
        );
    }
    assert_eq!(cache.load().unwrap(), b"javascript");
}
#[test]
fn corrupt_or_truncated_artifacts_are_misses_and_replacement_is_bounded() {
    let dir = Directory::new();
    let cache = OutputCache::new(&dir.0, &graph(), b"compiler");
    cache.store(b"output").unwrap();
    let path = fs::read_dir(&dir.0)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original = fs::read(&path).unwrap();
    for offset in [0, original.len() / 2, original.len() - 1] {
        let mut corrupt = original.clone();
        corrupt[offset] ^= 1;
        fs::write(&path, corrupt).unwrap();
        assert!(cache.load().is_none());
    }
    fs::write(&path, &original[..original.len() - 1]).unwrap();
    assert!(cache.load().is_none());
    cache.store(b"replacement").unwrap();
    assert_eq!(cache.load().unwrap(), b"replacement");
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
}
#[test]
fn simultaneous_writers_publish_complete_artifacts() {
    let dir = Directory::new();
    std::thread::scope(|scope| {
        for value in 0..8 {
            let directory = &dir.0;
            scope.spawn(move || {
                let cache = OutputCache::new(directory, &graph(), b"compiler");
                cache.store(&vec![value; 100_000]).unwrap();
                let loaded = cache.load().expect("a complete atomic publication");
                assert_eq!(loaded.len(), 100_000);
                assert!(loaded.iter().all(|&byte| byte == loaded[0]));
            });
        }
    });
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[test]
fn production_and_development_outputs_have_independent_slots_and_keys() {
    use planexpo_elm::kernel::Mode;
    let dir = Directory::new();
    let graph = graph();
    let dev = OutputCache::new_in_mode(&dir.0, &graph, b"compiler", Mode::Development);
    let prod = OutputCache::new_in_mode(&dir.0, &graph, b"compiler", Mode::Production);
    dev.store(b"development").unwrap();
    assert!(prod.load().is_none());
    prod.store(b"production").unwrap();
    assert_eq!(dev.load().unwrap(), b"development");
    assert_eq!(prod.load().unwrap(), b"production");
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 2);
    assert_eq!(
        OutputCache::new(&dir.0, &graph, b"compiler")
            .load()
            .unwrap(),
        b"development"
    );
}
