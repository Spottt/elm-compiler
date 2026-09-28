use planexpo_elm::{
    package_solver::Version,
    publish_git::{Error, Git},
};
use std::{path::Path, process::Command, sync::Mutex};
// A concurrent spawn can inherit the temporary script's open writer until exec,
// causing ETXTBSY even after this test closes its own descriptor.
static PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
#[test]
fn tags_and_changes_follow_official_git_commands() {
    let _guard = PROCESS_TEST_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "--quiet"]);
    std::fs::write(root.join("elm.json"), "{}\n").unwrap();
    git(root, &["add", "elm.json"]);
    git(
        root,
        &[
            "-c",
            "user.name=Publish Test",
            "-c",
            "user.email=publish@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
    );
    let commit = git(root, &["rev-parse", "HEAD"]);
    let checks = Git::find(std::env::var_os("PATH").as_deref()).unwrap();
    let version = Version([1, 0, 0]);
    assert_eq!(
        checks.verify_tag(root, version),
        Err(Error::MissingTag(version))
    );
    git(root, &["tag", "1.0.0"]);
    assert_eq!(checks.verify_tag(root, version), Ok(()));
    assert_eq!(checks.verify_changes(root, &commit, version), Ok(()));
    // Haskell's diff-index deliberately ignores untracked files.
    std::fs::write(root.join("untracked"), "local").unwrap();
    assert_eq!(checks.verify_changes(root, &commit, version), Ok(()));
    std::fs::write(root.join("elm.json"), "{\"modified\":true}\n").unwrap();
    assert_eq!(
        checks.verify_changes(root, &commit, version),
        Err(Error::LocalChanges(version))
    );
    git(root, &["add", "elm.json"]);
    assert_eq!(
        checks.verify_changes(root, &commit, version),
        Err(Error::LocalChanges(version))
    );
    git(root, &["checkout", "HEAD", "--", "elm.json"]);
    assert_eq!(checks.verify_changes(root, &commit, version), Ok(()));
    assert_eq!(
        checks.verify_changes(root, "missing-commit", version),
        Err(Error::LocalChanges(version))
    );
}
#[test]
fn absent_git_is_distinct_from_a_missing_tag() {
    assert!(matches!(Git::find(None), Err(Error::MissingGit)));
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        Git::find(Some(dir.path().as_os_str())),
        Err(Error::MissingGit)
    ));
}
#[cfg(unix)]
#[test]
fn path_search_skips_non_executable_files() {
    let _guard = PROCESS_TEST_LOCK.lock().unwrap();
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("git");
    std::fs::write(&file, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(
        Git::find(Some(dir.path().as_os_str())),
        Err(Error::MissingGit)
    ));
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    let checks = Git::find(Some(dir.path().as_os_str())).unwrap();
    assert_eq!(
        checks.verify_tag(dir.path(), Version([1, 0, 0])),
        Err(Error::MissingTag(Version([1, 0, 0])))
    );
}
