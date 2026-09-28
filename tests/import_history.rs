use planexpo_elm::import_history::{known, remember};
use std::{
    collections::BTreeSet,
    fs,
    sync::{Arc, Barrier},
    time::Duration,
};

fn names(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn concurrent_builds_preserve_all_successful_module_names() {
    let project = tempfile::tempdir().unwrap();
    let manifest = project.path().join("elm.json");
    fs::write(&manifest, "{}").unwrap();
    let gate = Arc::new(Barrier::new(8));
    std::thread::scope(|scope| {
        for index in 0..8 {
            let gate = gate.clone();
            let manifest = &manifest;
            scope.spawn(move || {
                gate.wait();
                remember(manifest, names(&[&format!("Module{index}")])).unwrap();
            });
        }
    });
    assert_eq!(
        known(&manifest),
        (0..8).map(|index| format!("Module{index}")).collect()
    );
}

#[test]
fn touched_manifest_starts_a_new_history_without_restoring_old_names() {
    let project = tempfile::tempdir().unwrap();
    let manifest = project.path().join("elm.json");
    fs::write(&manifest, "{}").unwrap();
    remember(&manifest, names(&["Old"])).unwrap();
    let modified = fs::metadata(&manifest).unwrap().modified().unwrap();
    fs::File::options()
        .write(true)
        .open(&manifest)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified + Duration::from_secs(2)))
        .unwrap();
    assert!(known(&manifest).is_empty());
    remember(&manifest, names(&["New"])).unwrap();
    assert_eq!(known(&manifest), names(&["New"]));
}

#[test]
fn truncated_history_is_a_cache_miss_and_can_be_replaced() {
    let project = tempfile::tempdir().unwrap();
    let manifest = project.path().join("elm.json");
    fs::write(&manifest, "{}").unwrap();
    remember(&manifest, names(&["Old"])).unwrap();
    let directory = project
        .path()
        .join("elm-stuff/planexpo-rust/import-history-v1");
    let artifact = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.file_name().unwrap() != "history.lock")
        .unwrap();
    fs::write(artifact, b"incomplete").unwrap();
    assert!(known(&manifest).is_empty());
    remember(&manifest, names(&["New"])).unwrap();
    assert_eq!(known(&manifest), names(&["New"]));
}

#[test]
fn unavailable_cache_returns_no_history_and_a_recoverable_write_error() {
    let project = tempfile::tempdir().unwrap();
    let manifest = project.path().join("elm.json");
    fs::write(&manifest, "{}").unwrap();
    fs::write(
        project.path().join("elm-stuff"),
        "a file blocks the cache directory",
    )
    .unwrap();
    assert!(known(&manifest).is_empty());
    assert!(remember(&manifest, names(&["Widget"])).is_err());
    assert!(known(&manifest).is_empty());
}
