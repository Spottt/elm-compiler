use serde_json::json;
use std::{
    fs,
    io::Write,
    process::{Command, Output, Stdio},
};

fn init(root: &std::path::Path, home: &std::path::Path, input: &str) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planexpo-elm"));
    cmd.arg("init").current_dir(root).env("ELM_HOME", home);
    for key in [
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
    ] {
        cmd.env(key, "http://127.0.0.1:9");
    }
    cmd.env("NO_PROXY", "").env("no_proxy", "");
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn decline_reprompts_and_does_not_touch_project_or_cache() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("cache");
    let result = init(root.path(), &home, "N\nno\nn\n");
    assert!(result.status.success(), "{:?}", result);
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("Must type 'y' for yes or 'n' for no: ")
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn existing_project_is_never_overwritten() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("elm.json"), "existing content").unwrap();
    let result = init(root.path(), &root.path().join("cache"), "");
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("EXISTING PROJECT"));
    assert_eq!(
        fs::read_to_string(root.path().join("elm.json")).unwrap(),
        "existing content"
    );
}

#[test]
fn creates_application_from_cached_solution_and_preserves_sources() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let cache = home.path().join("0.19.1/packages");
    for name in ["elm/core", "elm/browser", "elm/html", "elm/json"] {
        let dir = cache.join(name).join("1.0.0");
        fs::create_dir_all(dir.join("src")).unwrap();
        let deps = if name == "elm/browser" {
            json!({"elm/json":"1.0.0 <= v < 2.0.0"})
        } else {
            json!({})
        };
        fs::write(dir.join("elm.json"), json!({"type":"package","name":name,"summary":"Test package","license":"BSD-3-Clause","version":"1.0.0","exposed-modules":[],"test-dependencies":{},"elm-version":"0.19.0 <= v < 0.20.0","dependencies":deps}).to_string()).unwrap();
    }
    let registry = planexpo_elm::registry::Registry::from_json(&json!({"elm/core":["1.0.0"],"elm/browser":["1.0.0"],"elm/html":["1.0.0"],"elm/json":["1.0.0"]})).unwrap();
    fs::write(cache.join("registry.dat"), registry.encode().unwrap()).unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/keep.txt"), "keep").unwrap();
    let result = init(root.path(), home.path(), "\n");
    assert!(result.status.success(), "{:?}", result);
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("elm.json")).unwrap()).unwrap();
    assert_eq!(
        config,
        json!({"type":"application","source-directories":["src"],"elm-version":"0.19.1","dependencies":{"direct":{"elm/core":"1.0.0","elm/browser":"1.0.0","elm/html":"1.0.0"},"indirect":{"elm/json":"1.0.0"}},"test-dependencies":{"direct":{},"indirect":{}}})
    );
    assert_eq!(
        fs::read_to_string(root.path().join("src/keep.txt")).unwrap(),
        "keep"
    );
}
