use planexpo_elm::dependencies::prepare_with;
use serde_json::{Value, json};
fn package(home: &std::path::Path, name: &str, elm: &str, deps: Value) {
    let root = home.join("0.19.1/packages").join(name).join("1.0.0");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("elm.json"),
        json!({"type":"package","name":name,"summary":"Test package","license":"BSD-3-Clause","version":"1.0.0","exposed-modules":[],"test-dependencies":{},"elm-version":elm,"dependencies":deps}).to_string(),
    )
    .unwrap();
}
fn config(direct: Value, indirect: Value, test_direct: Value, test_indirect: Value) -> Value {
    json!({"type":"application","dependencies":{"direct":direct,"indirect":indirect},"test-dependencies":{"direct":test_direct,"indirect":test_indirect}})
}
#[test]
fn only_equal_indirect_and_test_direct_duplicates_are_allowed() {
    let home = tempfile::tempdir().unwrap();
    package(home.path(), "author/pkg", "0.19.0 <= v < 0.20.0", json!({}));
    for a in 0..4 {
        for b in a + 1..4 {
            let mut parts = [json!({}), json!({}), json!({}), json!({})];
            parts[a] = json!({"author/pkg":"1.0.0"});
            parts[b] = parts[a].clone();
            let c = config(
                parts[0].clone(),
                parts[1].clone(),
                parts[2].clone(),
                parts[3].clone(),
            );
            let result = prepare_with(&c, home.path(), || {
                panic!("cached manifest validation must not use network")
            });
            assert_eq!(
                result.is_ok(),
                a == 1 && b == 2,
                "sections {a},{b}: {result:?}"
            );
        }
    }
}
#[test]
fn cached_sources_do_not_hide_missing_or_incompatible_transitive_dependencies() {
    for (declared, required, elm, valid) in [
        (false, "1.0.0 <= v < 2.0.0", "0.19.0 <= v < 0.20.0", false),
        (true, "2.0.0 <= v < 3.0.0", "0.19.0 <= v < 0.20.0", false),
        (true, "1.0.0 <= v < 2.0.0", "0.20.0 <= v < 0.21.0", false),
        (true, "1.0.0 <= v < 2.0.0", "0.19.0 <= v < 0.20.0", true),
    ] {
        let home = tempfile::tempdir().unwrap();
        package(
            home.path(),
            "author/parent",
            elm,
            json!({"author/child":required}),
        );
        package(
            home.path(),
            "author/child",
            "0.19.0 <= v < 0.20.0",
            json!({}),
        );
        let c = config(
            json!({"author/parent":"1.0.0"}),
            if declared {
                json!({"author/child":"1.0.0"})
            } else {
                json!({})
            },
            json!({}),
            json!({}),
        );
        let result = prepare_with(&c, home.path(), || {
            panic!("cached manifest validation must not use network")
        });
        assert_eq!(result.is_ok(), valid, "{result:?}");
    }
}

#[test]
fn application_rejects_and_removes_corrupt_cached_metadata() {
    let home = tempfile::tempdir().unwrap();
    package(home.path(), "author/pkg", "0.19.0 <= v < 0.20.0", json!({}));
    let path = home
        .path()
        .join("0.19.1/packages/author/pkg/1.0.0/elm.json");
    let mut value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["license"] = json!("invalid");
    std::fs::write(&path, value.to_string()).unwrap();
    let c = config(
        json!({"author/pkg":"1.0.0"}),
        json!({}),
        json!({}),
        json!({}),
    );
    assert!(prepare_with(&c, home.path(), || panic!("must fail without network")).is_err());
    assert!(!path.exists());
}

#[test]
fn corrupt_package_metadata_does_not_trigger_an_immediate_network_retry() {
    let home = tempfile::tempdir().unwrap();
    package(home.path(), "author/pkg", "0.19.0 <= v < 0.20.0", json!({}));
    let path = home
        .path()
        .join("0.19.1/packages/author/pkg/1.0.0/elm.json");
    std::fs::write(&path, "invalid json").unwrap();
    let config = json!({"type":"package","dependencies":{"author/pkg":"1.0.0 <= v < 2.0.0"},"test-dependencies":{}});
    let error = prepare_with(&config, home.path(), || {
        panic!("corrupt cache must fail this attempt")
    })
    .unwrap_err();
    let report = planexpo_elm::dependency_error::report_encoded(&error).unwrap();
    assert_eq!(report["title"], "PROBLEM SOLVING PACKAGE CONSTRAINTS");
    assert!(!path.exists());
}
