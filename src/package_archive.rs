//! Verified package archive extraction. Elm keeps only src/, elm.json, README.md
//! and LICENSE, and removes the single GitHub archive root directory.
use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Read, Write},
    path::{Component, Path},
};

pub(crate) fn extract(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("invalid package archive: {e}"))?;
    let mut root = None;
    let mut seen = BTreeSet::new();
    let mut expanded = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("invalid archive entry: {e}"))?;
        let name = entry.name().to_owned();
        if name.contains('\\')
            || name.contains('\0')
            || name.split('/').any(|part| matches!(part, "." | ".."))
            || Path::new(&name)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(format!("invalid archive path: {name}"));
        }
        let (top, relative) = name
            .split_once('/')
            .ok_or("archive has no root directory")?;
        if top.is_empty() || root.as_ref().is_some_and(|r| r != top) {
            return Err("archive contains multiple roots".into());
        }
        root.get_or_insert_with(|| top.to_owned());
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(format!("archive contains a symbolic link: {name}"));
        }
        if !(relative.starts_with("src/")
            || matches!(relative, "elm.json" | "README.md" | "LICENSE"))
        {
            continue;
        }
        let path = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            continue;
        }
        if !seen.insert(relative.to_owned()) {
            return Err(format!("duplicate archive path: {name}"));
        }
        const MAX_EXPANDED: u64 = 256 * 1024 * 1024;
        expanded = expanded
            .checked_add(entry.size())
            .ok_or("archive size overflow")?;
        if expanded > MAX_EXPANDED {
            return Err("package archive exceeds 256 MiB expanded".into());
        }
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let expected = entry.size();
        let copied = std::io::copy(&mut entry.by_ref().take(expected + 1), &mut file)
            .map_err(|e| format!("invalid archive content: {e}"))?;
        if copied != expected {
            return Err("archive entry size mismatch".into());
        }
        file.flush().map_err(|e| e.to_string())?;
    }
    if !destination.join("elm.json").is_file() || !destination.join("src").is_dir() {
        return Err("package archive is missing elm.json or src".into());
    }
    Ok(())
}
