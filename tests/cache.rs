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
        import_errors: Vec::new(),
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
    let debug = OutputCache::new_in_mode(&dir.0, &graph, b"compiler", Mode::Debug);
    assert!(debug.load().is_none());
    debug.store(b"debugger").unwrap();
    assert_eq!(debug.load().unwrap(), b"debugger");
    assert_eq!(dev.load().unwrap(), b"development");
    assert_eq!(prod.load().unwrap(), b"production");
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 3);
    assert_eq!(
        OutputCache::new(&dir.0, &graph, b"compiler")
            .load()
            .unwrap(),
        b"development"
    );
}

#[test]
fn diagnostic_slot_is_separate_and_invalidates_with_compiler_and_source() {
    let dir = Directory::new();
    let original = graph();
    let output = OutputCache::new(&dir.0, &original, b"compiler1");
    let diagnostic = output.diagnostic_slot();
    output.store(b"valid bundle").unwrap();
    diagnostic.store(b"source error").unwrap();
    assert_eq!(output.load().unwrap(), b"valid bundle");
    assert_eq!(diagnostic.load().unwrap(), b"source error");
    assert!(OutputCache::new(&dir.0, &original, b"compiler2").diagnostic_slot().load().is_none());
    let mut changed = graph();
    changed.modules[0].source.push(' ');
    assert!(OutputCache::new(&dir.0, &changed, b"compiler1").diagnostic_slot().load().is_none());
    changed = graph();
    changed.manifests[0].1.push(' ');
    assert!(OutputCache::new(&dir.0, &changed, b"compiler1").diagnostic_slot().load().is_none());
    assert!(OutputCache::new_in_mode(&dir.0, &original, b"compiler1", planexpo_elm::kernel::Mode::Production).diagnostic_slot().load().is_none());
}

#[test]
fn trivia_output_reuse_preserves_token_coordinates_and_rejects_invalid_syntax() {
    let dir = Directory::new();
    let mut original = graph();
    original.modules[0].source = "module Main exposing (main)\nmain = 42\n".into();
    let cache = OutputCache::new_for_trivia(&dir.0, &original, b"compiler").unwrap();
    cache.store(b"verified output").unwrap();
    for suffix in ["\n-- a comment\n", "\n{- nested {- comment -} -}\n", "\n\n"] {
        let mut changed = original.clone();
        changed.modules[0].source.push_str(suffix);
        assert_eq!(OutputCache::new_for_trivia(&dir.0, &changed, b"compiler").unwrap().load(), Some(b"verified output".to_vec()));
    }
    for source in [
        "module Main exposing (main)\nmain = 43\n",
        "module Main exposing (main)\n\nmain = 42\n",
        "module Main exposing (main)\nmain = \"--42\"\n",
    ] {
        let mut changed = original.clone(); changed.modules[0].source = source.into();
        assert!(OutputCache::new_for_trivia(&dir.0, &changed, b"compiler").unwrap().load().is_none());
    }
    for suffix in ["\n{- unclosed", "\nbroken =", "\nother : Int\n"] {
        let mut changed = original.clone(); changed.modules[0].source.push_str(suffix);
        assert!(OutputCache::new_for_trivia(&dir.0, &changed, b"compiler").and_then(|cache| cache.load()).is_none());
    }
    assert!(OutputCache::new_for_trivia(&dir.0, &original, b"other compiler").unwrap().load().is_none());
    assert!(OutputCache::new(&dir.0, &original, b"compiler").load().is_none(), "the exact-output namespace stays separate");
    assert!(OutputCache::new(&dir.0, &original, b"compiler").diagnostic_slot().load().is_none());
}

#[test]
fn trivia_key_keeps_kernel_sources_and_graph_resolution_exact() {
    let dir = Directory::new();
    let mut original = graph();
    original.modules[0].source = "module Main exposing (main)\nmain = 42\n".into();
    let mut kernel = original.modules[0].clone();
    kernel.kernel = true; kernel.name = "Elm.Kernel.Test".into(); kernel.source = "function x() { return 1; }".into();
    original.modules.push(kernel);
    OutputCache::new_for_trivia(&dir.0, &original, b"compiler").unwrap().store(b"output").unwrap();
    let mut changed = original.clone(); changed.modules[1].source.push_str(" // comment");
    assert!(OutputCache::new_for_trivia(&dir.0, &changed, b"compiler").unwrap().load().is_none());
    let mut changed = original.clone(); changed.modules[0].path = "/moved/Main.elm".into();
    assert!(OutputCache::new_for_trivia(&dir.0, &changed, b"compiler").unwrap().load().is_none());
    let mut changed = original.clone(); changed.manifests[0].1.push(' ');
    assert!(OutputCache::new_for_trivia(&dir.0, &changed, b"compiler").unwrap().load().is_none());
}

#[test]
fn trivia_lookup_validates_syntax_even_with_a_matching_payload() {
    let dir = Directory::new();
    let mut invalid = graph();
    invalid.modules[0].source = "module Main exposing (main)\nmain = 42\nbroken : Int\n".into();
    let candidate = OutputCache::new_for_trivia(&dir.0, &invalid, b"compiler").unwrap();
    candidate.store(b"not a proof of valid syntax").unwrap();
    assert!(candidate.load().is_none());
}

#[test]
fn exact_and_trivia_keys_share_one_verified_payload() {
    let dir = Directory::new();
    let mut graph = graph();
    graph.modules[0].source = "module Main exposing (main)\nmain = 42\n".into();
    let exact = OutputCache::new(&dir.0, &graph, b"compiler");
    let trivia = OutputCache::new_for_trivia(&dir.0, &graph, b"compiler").unwrap();
    let output = vec![b'x'; 1024 * 1024];
    exact.store_with_trivia(&output, Some(&trivia)).unwrap();
    assert_eq!(exact.load().unwrap(), output);
    assert_eq!(trivia.load().unwrap(), output);
    let files = fs::read_dir(&dir.0).unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    assert_eq!(files.len(), 2);
    let total: u64 = files.iter().map(|path| fs::metadata(path).unwrap().len()).sum();
    assert!(total < output.len() as u64 + 512, "the second key must not duplicate the bundle");
    exact.store(b"another build").unwrap();
    assert!(trivia.load().is_none(), "a stale reference cannot reuse a different build");
    exact.store_with_trivia(&output, Some(&trivia)).unwrap();
    let payload = files.iter().find(|path| path.extension().unwrap() == "cache").unwrap();
    let mut corrupted = fs::read(payload).unwrap();
    *corrupted.last_mut().unwrap() ^= 1;
    fs::write(payload, corrupted).unwrap();
    assert!(exact.load().is_none());
    assert!(trivia.load().is_none(), "the referenced payload is still checksum-verified");
}

#[test]
fn concurrent_paired_cache_writers_never_cross_wire_sources() {
    let dir = Directory::new();
    std::thread::scope(|scope| {
        for value in [42, 43] {
            let directory = &dir.0;
            scope.spawn(move || {
                let mut graph = graph();
                graph.modules[0].source = format!("module Main exposing (main)\nmain = {value}\n").into();
                let exact = OutputCache::new(directory, &graph, b"compiler");
                let trivia = OutputCache::new_for_trivia(directory, &graph, b"compiler").unwrap();
                let output = vec![value as u8; 8192];
                for _ in 0..30 {
                    exact.store_with_trivia(&output, Some(&trivia)).unwrap();
                    if let Some(actual) = trivia.load() { assert_eq!(actual, output); }
                }
            });
        }
    });
}

#[test]
fn shared_source_digests_use_the_same_verified_output_keys() {
    use planexpo_elm::{cache::SourceDigests, kernel::Mode};
    let dir = Directory::new();
    let original = graph();
    let digests = SourceDigests::new(&original);
    OutputCache::new_with_digests(&dir.0, &digests, b"compiler", Mode::Development)
        .store(b"shared output").unwrap();
    assert_eq!(OutputCache::new(&dir.0, &original, b"compiler").load().unwrap(), b"shared output");
    for edit in 0..3 {
        let mut changed = graph();
        match edit {
            0 => changed.modules[0].source = "abd".into(),
            1 => changed.modules[0].source.insert(0, ' '),
            _ => changed.modules[0].path = "/moved/Main.elm".into(),
        }
        assert!(OutputCache::new_with_digests(&dir.0, &SourceDigests::new(&changed), b"compiler", Mode::Development).load().is_none());
    }
}

#[test]
fn worker_memory_outputs_keep_source_identity_and_fresh_trivia_validation() {
    let dir = Directory::new();
    let mut original = graph();
    original.modules[0].source = "module Main exposing (main)\nmain = 42\n".into();
    let exact = OutputCache::new(&dir.0, &original, b"compiler");
    {
        let _scope = planexpo_elm::cache::output_memory_scope(true);
        let trivia = OutputCache::new_for_trivia(&dir.0, &original, b"compiler").unwrap();
        exact.store_with_trivia(b"output", Some(&trivia)).unwrap();
        assert_eq!(exact.load().unwrap(), b"output");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0, "worker output does not write a second disk bundle");
        assert!(exact.diagnostic_slot().load().is_none());
        for change in 0..5 {
            let mut changed = original.clone();
            match change {
                0 => changed.modules[0].source.push_str("\n-- comment\n"),
                1 => changed.modules[0].source = changed.modules[0].source.replace("42", "43").into(),
                2 => changed.modules[0].imports.push("other:Provider".into()),
                3 => changed.modules[0].path = "moved/Main.elm".into(),
                _ => changed.manifests[0].1.push('!'),
            }
            assert!(OutputCache::new(&dir.0, &changed, b"compiler").load().is_none());
            assert_eq!(OutputCache::new_for_trivia(&dir.0, &changed, b"compiler").unwrap().load().is_some(), change == 0);
        }
        assert!(OutputCache::new(&dir.0, &original, b"another compiler").load().is_none());
        let mut invalid = original.clone();
        invalid.modules[0].source.push_str("\nother : Int\n");
        let invalid_exact = OutputCache::new(&dir.0, &invalid, b"compiler");
        let invalid_trivia = OutputCache::new_for_trivia(&dir.0, &invalid, b"compiler").unwrap();
        // Even an injected matching trivia key cannot bypass fresh parsing.
        invalid_exact.store_with_trivia(b"bad", Some(&invalid_trivia)).unwrap();
        assert!(invalid_trivia.load().is_none());
    }
    assert!(exact.load().is_none(), "worker lifetime ends the ephemeral cache");
}

#[test]
fn disk_writers_replace_an_existing_worker_memory_generation() {
    let dir = Directory::new(); let mut source = graph();
    source.modules[0].source = "module Main exposing (main)\nmain = 42\n".into();
    let exact = OutputCache::new(&dir.0, &source, b"compiler");
    let trivia = OutputCache::new_for_trivia(&dir.0, &source, b"compiler").unwrap();
    let _scope = planexpo_elm::cache::output_memory_scope(true);
    exact.store_with_trivia(b"memory", Some(&trivia)).unwrap();
    exact.store(b"disk").unwrap();
    assert_eq!(exact.load().unwrap(), b"disk");
    assert!(trivia.load().is_none());
    exact.store_with_trivia(b"memory", Some(&trivia)).unwrap();
    trivia.store(b"other").unwrap();
    assert_eq!(trivia.load().unwrap(), b"other");
    assert!(exact.load().is_none());
}
