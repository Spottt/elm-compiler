//! Read-only Git checks used by Elm 0.19.1 Publish.verifyTag/verifyNoChanges.
use crate::package_solver::Version;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    MissingGit,
    MissingTag(Version),
    LocalChanges(Version),
    Io(String),
}
pub struct Git {
    executable: PathBuf,
}
impl Git {
    pub fn find(path: Option<&OsStr>) -> Result<Self, Error> {
        let executable = if cfg!(windows) { "git.exe" } else { "git" };
        path.into_iter()
            .flat_map(std::env::split_paths)
            .map(|dir| dir.join(executable))
            .find(|candidate| executable_file(candidate))
            .map(|executable| Self {
                executable: executable.canonicalize().unwrap_or(executable),
            })
            .ok_or(Error::MissingGit)
    }
    fn succeeds(&self, root: &Path, args: &[&str]) -> Result<bool, Error> {
        Command::new(&self.executable)
            .current_dir(root)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .map_err(|error| Error::Io(error.to_string()))
    }
    pub fn verify_tag(&self, root: &Path, version: Version) -> Result<(), Error> {
        if self.succeeds(root, &["show", "--name-only", &version.to_string(), "--"])? {
            Ok(())
        } else {
            Err(Error::MissingTag(version))
        }
    }
    pub fn verify_changes(&self, root: &Path, commit: &str, version: Version) -> Result<(), Error> {
        if self.succeeds(root, &["diff-index", "--quiet", commit, "--"])? {
            Ok(())
        } else {
            Err(Error::LocalChanges(version))
        }
    }
}
fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
