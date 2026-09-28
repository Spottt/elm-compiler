use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    process::{Command, Stdio},
};

fn fixture() -> (tempfile::TempDir, tempfile::TempDir, String) {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let cache = home.path().join("0.19.1/packages");
    for name in ["elm/core", "elm/json", "author/new"] {
        let path = cache.join(name).join("1.0.0");
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(path.join("elm.json"), json!({"type":"package","name":name,"summary":"CLI test package","license":"BSD-3-Clause","version":"1.0.0","elm-version":"0.19.0 <= v < 0.20.0","exposed-modules":[],"dependencies":{},"test-dependencies":{}}).to_string()).unwrap();
    }
    let registry = planexpo_elm::registry::Registry::from_json(
        &json!({"elm/core":["1.0.0"],"elm/json":["1.0.0"],"author/new":["1.0.0"]}),
    )
    .unwrap();
    fs::write(cache.join("registry.dat"), registry.encode().unwrap()).unwrap();
    let original = json!({"type":"application","elm-version":"0.19.1","source-directories":["src"],"dependencies":{"direct":{"elm/core":"1.0.0","elm/json":"1.0.0"},"indirect":{}},"test-dependencies":{"direct":{},"indirect":{}}}).to_string();
    fs::write(root.path().join("elm.json"), &original).unwrap();
    fs::create_dir_all(root.path().join("src/nested")).unwrap();
    (root, home, original)
}

fn command(root: &std::path::Path, home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planexpo-elm"));
    cmd.args(["install", "author/new"])
        .current_dir(root)
        .env("ELM_HOME", home);
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
    cmd.env("no_proxy", "").env("NO_PROXY", "");
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

#[test]
fn declining_from_subdirectory_preserves_manifest_bytes() {
    let (root, home, original) = fixture();
    let mut child = command(&root.path().join("src/nested"), home.path())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"N\nno\nn\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Must type 'y' for yes or 'n' for no: ")
    );
    assert_eq!(
        fs::read_to_string(root.path().join("elm.json")).unwrap(),
        original
    );
    assert!(!root.path().join("src/nested/elm.json").exists());
}

#[test]
fn concurrent_manifest_edit_is_not_overwritten_after_confirmation() {
    let (root, home, original) = fixture();
    let mut child = command(root.path(), home.path()).spawn().unwrap();
    let mut prompt = Vec::new();
    while !prompt.ends_with(b"[Y/n]: ") {
        let mut byte = [0];
        assert_eq!(child.stdout.as_mut().unwrap().read(&mut byte).unwrap(), 1);
        prompt.push(byte[0]);
    }
    let edited = format!("{original}\n\n");
    fs::write(root.path().join("elm.json"), &edited).unwrap();
    child.stdin.take().unwrap().write_all(b"y\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("elm.json changed during installation"),
        "{output:?}"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("elm.json")).unwrap(),
        edited
    );
}

#[test]
fn confirmation_creates_no_project_source_or_build_artifact() {
    let (root, home, _) = fixture();
    fs::write(root.path().join("src/Unfinished.elm"), "this is not Elm").unwrap();
    let mut child = command(root.path(), home.path()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("elm.json")).unwrap()).unwrap();
    assert_eq!(config["dependencies"]["direct"]["author/new"], "1.0.0");
    assert_eq!(
        fs::read_to_string(root.path().join("src/Unfinished.elm")).unwrap(),
        "this is not Elm"
    );
    assert!(!root.path().join("elm.js").exists());
}

#[cfg(unix)]
#[test]
fn installation_preserves_manifest_symlink_and_target_permissions() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let (root, home, original) = fixture();
    let storage = tempfile::tempdir().unwrap();
    let target = storage.path().join("project.json");
    fs::write(&target, original).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    let link = root.path().join("elm.json");
    fs::remove_file(&link).unwrap();
    symlink(&target, &link).unwrap();
    let mut child = command(root.path(), home.path()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"y\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let config: serde_json::Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    assert_eq!(config["dependencies"]["direct"]["author/new"], "1.0.0");
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o640
    );
}
