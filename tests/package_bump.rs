use planexpo_elm::{
    api_diff::Magnitude,
    package_bump::{bumpable_versions, change_version, possibilities},
    package_solver::Version,
};
use serde_json::json;
fn v(text: &str) -> Version {
    Version::parse(text).unwrap()
}
#[test]
fn release_candidates_allow_backports_only_from_last_releases() {
    let published = ["2.0.1", "1.1.1", "1.0.0", "1.0.1", "1.1.0", "2.0.0"].map(v);
    let candidates = possibilities(&published);
    let actual: Vec<_> = candidates
        .iter()
        .map(|c| (c.old.to_string(), c.new.to_string(), c.magnitude))
        .collect();
    let expected = [
        ("2.0.1", "3.0.0", Magnitude::Major),
        ("1.1.1", "1.2.0", Magnitude::Minor),
        ("2.0.1", "2.1.0", Magnitude::Minor),
        ("1.0.1", "1.0.2", Magnitude::Patch),
        ("1.1.1", "1.1.2", Magnitude::Patch),
        ("2.0.1", "2.0.2", Magnitude::Patch),
    ]
    .map(|(a, b, m)| (a.into(), b.into(), m));
    assert_eq!(actual, expected);
    assert_eq!(
        bumpable_versions(&published),
        ["1.0.1", "1.1.1", "2.0.1"].map(v)
    );
    assert!(possibilities(&[]).is_empty());
    assert_eq!(bumpable_versions(&[v("1.0.0")]), [v("1.0.0")]);
}
fn package() -> Vec<u8> {
    serde_json::to_vec(&json!({"type":"package","name":"author/package","summary":"Package version update fixture","license":"BSD-3-Clause","version":"1.0.0","exposed-modules":{"API":["Example"]},"elm-version":"0.19.0 <= v < 0.20.0","dependencies":{"elm/core":"1.0.0 <= v < 2.0.0"},"test-dependencies":{}})).unwrap()
}
#[test]
fn approved_update_preserves_fields_and_refuses_intervening_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("elm.json");
    let original = package();
    std::fs::write(&path, &original).unwrap();
    change_version(&path, &original, v("2.0.0")).unwrap();
    let after = std::fs::read(&path).unwrap();
    let mut expected: serde_json::Value = serde_json::from_slice(&original).unwrap();
    expected["version"] = json!("2.0.0");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&after).unwrap(),
        expected
    );
    assert!(after.starts_with(b"{\n    \"type\": \"package\",\n    \"name\":"));
    assert!(after.ends_with(b"\n"));
    assert!(
        change_version(&path, &original, v("3.0.0"))
            .unwrap_err()
            .contains("changed")
    );
    assert_eq!(std::fs::read(&path).unwrap(), after);
}
#[cfg(unix)]
#[test]
fn version_update_preserves_manifest_symlink_and_file_permissions() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("actual.json");
    let link = dir.path().join("elm.json");
    let original = package();
    std::fs::write(&target, &original).unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
    symlink(&target, &link).unwrap();
    change_version(&link, &original, v("1.1.0")).unwrap();
    assert!(link.is_symlink());
    assert_eq!(
        std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(&target).unwrap()).unwrap()["version"],
        "1.1.0"
    );
}

#[test]
fn writer_keeps_summary_escapes_group_order_and_elm_dependency_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("elm.json");
    let mut outline: serde_json::Value = serde_json::from_slice(&package()).unwrap();
    outline["dependencies"] = json!({"elm/core":"1.0.0 <= v < 2.0.0", "aa/zz":"1.0.0 <= v < 2.0.0", "aa-bb/yy":"1.0.0 <= v < 2.0.0"});
    let original = serde_json::to_string(&outline)
        .unwrap()
        .replace("Package version update fixture", r"R\u00e9sum\u00e9")
        .replace(
            r#"{"API":["Example"]}"#,
            r#"{"Zed":["Zed"],"Alpha":["Alpha"]}"#,
        );
    std::fs::write(&path, &original).unwrap();
    change_version(&path, original.as_bytes(), v("1.0.1")).unwrap();
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains(r#""summary": "R\u00e9sum\u00e9""#));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&after).unwrap()["summary"],
        "Résumé"
    );
    assert!(after.find("\"Zed\":").unwrap() < after.find("\"Alpha\":").unwrap());
    assert!(after.find("\"aa/zz\":").unwrap() < after.find("\"aa-bb/yy\":").unwrap());
}

#[test]
fn approved_update_serializes_parsed_constraints_with_original_bound_inclusivity() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("elm.json");
    let mut outline: serde_json::Value = serde_json::from_slice(&package()).unwrap();
    outline["elm-version"] = json!("65536.19.0 <= v < 65536.20.0");
    outline["dependencies"]["elm/core"] = json!("65537.0.0 <= v < 65538.0.0");
    outline["test-dependencies"] = json!({"aa/bb":"65537.0.0 < v <= 65538.0.0"});
    let original = serde_json::to_vec(&outline).unwrap();
    std::fs::write(&path, &original).unwrap();
    change_version(&path, &original, v("1.0.1")).unwrap();
    let result: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(result["elm-version"], "0.19.0 <= v < 0.20.0");
    assert_eq!(result["dependencies"]["elm/core"], "1.0.0 <= v < 2.0.0");
    assert_eq!(result["test-dependencies"]["aa/bb"], "1.0.0 < v <= 2.0.0");
}
