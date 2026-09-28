use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_planexpo-elm"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn identifies_its_own_version_and_documents_make() {
    let result = cli(&["--version"]);
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout).unwrap().trim(),
        env!("CARGO_PKG_VERSION")
    );
    let result = cli(&["--help"]);
    assert!(result.status.success());
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("make <entry.elm>")
    );
}

#[test]
fn invalid_invocations_fail_with_machine_readable_errors() {
    for args in [
        vec!["make", "Main.elm", "--debug", "--report=json"],
        vec!["make", "Main.elm", "--output=out.txt", "--report", "json"],
        vec![
            "make",
            "A.elm",
            "B.elm",
            "--output=out.html",
            "--report=json",
        ],
        vec!["make", "Main.elm", "--output", "--report=json"],
        vec![
            "make",
            "Main.elm",
            "--output=a.js",
            "--output=b.js",
            "--report=json",
        ],
    ] {
        let result = cli(&args);
        assert!(!result.status.success(), "{args:?}");
        assert!(result.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&result.stderr).unwrap();
        assert_eq!(error["type"], "error");
        assert!(!error["message"][0].as_str().unwrap().is_empty());
    }
}

#[test]
fn source_errors_have_a_file_and_real_region_in_json() {
    let directory =
        std::env::temp_dir().join(format!("elm-rust-diagnostic-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("Main with spaces.elm");
    std::fs::write(&path, "module Main exposing (..)\nbad =\n").unwrap();
    let result = cli(&["parse", path.to_str().unwrap(), "--report=json"]);
    let error: serde_json::Value = serde_json::from_slice(&result.stderr).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(!result.status.success());
    assert_eq!(error["type"], "compile-errors");
    assert_eq!(error["errors"][0]["path"], path.to_str().unwrap());
    assert_eq!(error["errors"][0]["name"], "Main");
    assert_eq!(
        error["errors"][0]["problems"][0]["region"]["start"]["line"],
        3
    );
    assert_eq!(
        error["errors"][0]["problems"][0]["region"]["start"]["column"],
        1
    );
}
