use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn repl_commands_run_without_a_project_and_do_not_create_user_files() {
    let directory = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_planexpo-elm"))
        .args(["repl", "--no-colors"])
        .current_dir(directory.path())
        .env("ELM_HOME", directory.path().join("home"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b":help\n:reset\n:quit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Valid commands include:"));
    assert!(stdout.ends_with("> <reset>\n> "));
    assert!(!directory.path().join("elm.json").exists());
}
