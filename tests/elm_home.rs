use std::process::Command;

#[test]
fn package_cache_defaults_to_the_platform_location() {
    let project = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let application_data = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_planexpo-elm"))
        .args(["install", "notapackage"])
        .current_dir(project.path())
        .env_remove("ELM_HOME")
        .env("HOME", home.path())
        .env("APPDATA", application_data.path())
        .output()
        .unwrap();
    let (expected, ignored) = if cfg!(windows) {
        (
            application_data.path().join("elm"),
            home.path().join(".elm"),
        )
    } else {
        (
            home.path().join(".elm"),
            application_data.path().join("elm"),
        )
    };
    assert!(
        expected.join("0.19.1").join("packages").is_dir(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!ignored.exists());
}
