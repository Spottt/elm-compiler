use std::{env, path::PathBuf};

/// Follows `getElmHome` in `builder/src/Stuff.hs` of the Haskell compiler (tag 0.19.1).
pub fn elm_home() -> Result<PathBuf, String> {
    if let Some(home) = env::var_os("ELM_HOME") {
        return Ok(PathBuf::from(home));
    }
    if cfg!(windows) {
        env::var_os("APPDATA")
            .map(|data| PathBuf::from(data).join("elm"))
            .ok_or_else(|| "ELM_HOME or APPDATA is required".to_string())
    } else {
        env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".elm"))
            .ok_or_else(|| "ELM_HOME or HOME is required".to_string())
    }
}
