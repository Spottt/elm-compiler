use planexpo_elm::{package_solver::Version, registry::Registry};
use serde_json::json;
#[test]
fn official_binary_layout_sorts_authors_before_projects_and_uses_word16_escape() {
    let registry = Registry::from_json(&json!({"a-b/c":["255.256.257"],"a/b":["1.0.0"]})).unwrap();
    // Data.Binary: Int64 count, Int64 Map length, byte-length strings,
    // Version, Int64 previous-version count. Author "a" precedes "a-b".
    let expected: Vec<u8> = [
        &[0, 0, 0, 0, 0, 0, 0, 2][..],
        &[0, 0, 0, 0, 0, 0, 0, 2],
        &[1, b'a', 1, b'b', 1, 0, 0],
        &[0, 0, 0, 0, 0, 0, 0, 0],
        &[3, b'a', b'-', b'b', 1, b'c', 255, 0, 255, 1, 0, 1, 1],
        &[0, 0, 0, 0, 0, 0, 0, 0],
    ]
    .concat();
    assert_eq!(registry.encode().unwrap(), expected);
    assert_eq!(Registry::decode(&expected).unwrap(), registry);
    for n in 0..expected.len() {
        assert!(
            Registry::decode(&expected[..n]).is_err(),
            "accepted prefix {n}"
        );
    }
}
#[test]
fn full_registry_and_incremental_updates_keep_official_order() {
    let registry = Registry::from_json(&json!({"author/pkg":["1.0.0","1.10.0","1.2.0"]})).unwrap();
    assert_eq!(
        registry.versions("author/pkg").unwrap(),
        &[Version([1, 10, 0]), Version([1, 2, 0]), Version([1, 0, 0])]
    );
    let updated = registry
        .updated(&json!([
            "author/pkg@2.0.0",
            "author/other@1.0.0",
            "author/pkg@1.11.0"
        ]))
        .unwrap();
    assert_eq!(updated.count(), 6);
    assert_eq!(updated.package_count(), 2);
    assert_eq!(
        updated.versions("author/pkg").unwrap()[..2],
        [Version([2, 0, 0]), Version([1, 11, 0])]
    );
    assert_eq!(
        Registry::decode(&updated.encode().unwrap()).unwrap(),
        updated
    );
    assert!(
        registry
            .updated(&json!(["author/pkg@2.0.0", "broken"]))
            .is_err()
    );
    assert_eq!(registry.count(), 3);
}
#[test]
fn malformed_registry_data_is_rejected() {
    for value in [
        json!([]),
        json!({"author/pkg":[]}),
        json!({"../pkg":["1.0.0"]}),
        json!({"author/pkg":["latest"]}),
    ] {
        assert!(Registry::from_json(&value).is_err());
    }
    let original = Registry::from_json(&json!({"author/pkg":["1.0.0"]}))
        .unwrap()
        .encode()
        .unwrap();
    let mut data = original.clone();
    data.extend_from_slice(&[0]);
    assert!(Registry::decode(&data).is_err());
    let mut data = original.clone();
    data[0] = 255;
    assert!(Registry::decode(&data).is_err());
    let mut data = original;
    data[7] = 2;
    assert!(Registry::decode(&data).is_err());
}

#[test]
fn registry_json_uses_the_official_package_name_grammar() {
    for name in [
        "Elm/core",
        "7team/a1",
        "a-b/c-d",
        &format!("{}/a", "a".repeat(255)),
    ] {
        assert!(
            Registry::from_json(&json!({name:["1.0.0"]})).is_ok(),
            "{name}"
        );
    }
    for name in [
        "a/Core",
        "a/1core",
        "-a/core",
        "a-/core",
        "a--b/core",
        "a/-core",
        "a/core-",
        "a/co--re",
        "a/co_re",
        "a/coré",
        "a/b/c",
        &format!("{}/a", "a".repeat(256)),
    ] {
        assert!(
            Registry::from_json(&json!({name:["1.0.0"]})).is_err(),
            "{name}"
        );
        let registry = Registry::from_json(&json!({})).unwrap();
        assert!(
            registry.updated(&json!([format!("{name}@1.0.0")])).is_err(),
            "{name}"
        );
    }
}
#[test]
fn suggestions_use_restricted_transpositions_and_official_author_preference() {
    let registry = planexpo_elm::registry::Registry::from_json(&serde_json::json!({
        "ab/ca":["1.0.0"],"ab/ac":["1.0.0"],"ab/ccc":["1.0.0"],"ab/abc":["1.0.0"]
    }))
    .unwrap();
    assert_eq!(
        registry.nearby_names("ab/ca"),
        ["ab/ca", "ab/ac", "ab/ccc", "ab/abc"]
    );
    let registry = planexpo_elm::registry::Registry::from_json(&serde_json::json!({
        "elm/json":["1.0.0"],"elm/core":["1.0.0"],"elm-explorations/test":["1.0.0"],
        "x/test":["1.0.0"],"z/test":["1.0.0"]
    }))
    .unwrap();
    assert_eq!(
        registry.nearby_names("random/test"),
        ["elm-explorations/test", "elm/core", "elm/json", "x/test"]
    );
}
