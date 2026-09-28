use planexpo_elm::installation::{PlanKind, plan};
use serde_json::{Value, json};

fn application() -> Value {
    json!({"type":"application","source-directories":["src"],"elm-version":"0.19.1",
        "dependencies":{"direct":{"elm/core":"1.0.5"},"indirect":{"elm/json":"1.1.3"}},
        "test-dependencies":{"direct":{"author/tests":"2.1.0"},"indirect":{"author/helper":"1.0.0"}}})
}

#[test]
fn existing_and_promoted_dependencies_do_not_resolve_or_change_versions() {
    let home = tempfile::tempdir().unwrap();
    let original = application();
    let installed = plan(home.path(), &original, "elm/core", None).unwrap();
    assert_eq!(installed.kind, PlanKind::AlreadyInstalled);
    assert_eq!(installed.outline, original);
    for (name, section, field, kind) in [
        (
            "elm/json",
            "dependencies",
            "indirect",
            PlanKind::PromoteIndirect,
        ),
        (
            "author/tests",
            "test-dependencies",
            "direct",
            PlanKind::PromoteTest,
        ),
        (
            "author/helper",
            "test-dependencies",
            "indirect",
            PlanKind::PromoteTest,
        ),
    ] {
        let result = plan(home.path(), &original, name, None).unwrap();
        assert_eq!(result.kind, kind);
        let mut expected = original.clone();
        let version = expected[section][field]
            .as_object_mut()
            .unwrap()
            .remove(name)
            .unwrap();
        expected["dependencies"]["direct"][name] = version;
        assert_eq!(result.outline, expected);
        assert!(result.changes.is_empty());
    }
    assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
}

fn package() -> Value {
    json!({"type":"package","name":"author/project","summary":"Test fixture","license":"BSD-3-Clause",
        "version":"1.0.0","elm-version":"0.19.0 <= v < 0.20.0","exposed-modules":[],
        "dependencies":{"elm/core":"1.0.0 <= v < 2.0.0"},
        "test-dependencies":{"author/tests":"2.0.0 <= v < 3.0.0"}})
}

#[test]
fn package_promotions_preserve_the_original_constraint() {
    let home = tempfile::tempdir().unwrap();
    let original = package();
    let installed = plan(home.path(), &original, "elm/core", None).unwrap();
    assert_eq!(installed.kind, PlanKind::AlreadyInstalled);
    let result = plan(home.path(), &original, "author/tests", None).unwrap();
    assert_eq!(result.kind, PlanKind::PromoteTest);
    assert_eq!(
        result.outline["dependencies"]["author/tests"],
        "2.0.0 <= v < 3.0.0"
    );
    assert!(
        result.outline["test-dependencies"]
            .as_object()
            .unwrap()
            .is_empty()
    );
    assert!(result.changes.is_empty());
}

#[test]
fn adding_to_package_keeps_existing_constraints_and_excludes_transitives() {
    use std::fs;
    let home = tempfile::tempdir().unwrap();
    for (name, version, dependencies) in [
        ("elm/core", "1.0.5", json!({})),
        ("author/tests", "2.1.0", json!({})),
        ("author/transitive", "1.0.0", json!({})),
        (
            "author/new",
            "3.2.1",
            json!({"author/transitive":"1.0.0 <= v < 2.0.0"}),
        ),
    ] {
        let root = home.path().join("0.19.1/packages").join(name).join(version);
        fs::create_dir_all(root.join("src")).unwrap();
        let mut config = package();
        config["name"] = json!(name);
        config["version"] = json!(version);
        config["dependencies"] = dependencies;
        config["test-dependencies"] = json!({});
        fs::write(root.join("elm.json"), config.to_string()).unwrap();
    }
    let original = package();
    let result = plan(home.path(), &original, "author/new", None).unwrap();
    assert_eq!(result.kind, PlanKind::Changes);
    assert_eq!(
        result.outline["dependencies"],
        json!({"elm/core":"1.0.0 <= v < 2.0.0", "author/new":"3.2.1 <= v < 4.0.0"})
    );
    assert_eq!(
        result.outline["test-dependencies"],
        original["test-dependencies"]
    );
    assert_eq!(result.changes.len(), 1);
}
#[test]
fn corrupt_package_metadata_keeps_its_solver_diagnostic() {
    let home = tempfile::tempdir().unwrap();
    let cache = home.path().join("0.19.1/packages/author/new/1.0.0");
    std::fs::create_dir_all(cache.join("src")).unwrap();
    std::fs::write(cache.join("elm.json"), "not json").unwrap();
    let error = plan(home.path(), &package(), "author/new", None).unwrap_err();
    let report = planexpo_elm::dependency_error::report_encoded(&error).unwrap();
    assert_eq!(report["title"], "PROBLEM SOLVING PACKAGE CONSTRAINTS");
}
