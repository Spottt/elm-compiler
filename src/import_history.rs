//! Previously checked local module names, used only for import suggestions.
//! Elm resets this history whenever the elm.json modification time changes.
use crate::cache::OutputCache;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, io, path::{Path, PathBuf}, time::UNIX_EPOCH};

struct Store { directory: PathBuf, artifact: OutputCache }
impl Store {
    fn new(manifest: &Path) -> io::Result<Self> {
        let root = manifest.parent().ok_or_else(|| io::Error::other("manifest has no directory"))?;
        let modified = fs::metadata(manifest)?.modified()?.duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?.as_nanos();
        let mut hash = Sha256::new();
        hash.update(b"local-import-history-v1");
        hash.update(modified.to_le_bytes());
        hash.update(fs::read(manifest)?);
        let directory = root.join("elm-stuff/planexpo-rust/import-history-v1");
        let artifact = OutputCache::artifact(&directory, b"local-modules", hash.finalize().into());
        Ok(Self { directory, artifact })
    }
    fn load(&self) -> BTreeSet<String> {
        self.artifact.load().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }
}
pub fn known(manifest: &Path) -> BTreeSet<String> {
    Store::new(manifest).map(|store| store.load()).unwrap_or_default()
}
/// Cache failures must not change whether the source compiles. The caller may
/// ignore this result; atomic checked artifacts prevent partial-name histories.
pub fn remember(manifest: &Path, names: BTreeSet<String>) -> io::Result<()> {
    if names.is_empty() { return Ok(()); }
    let store = Store::new(manifest)?;
    if names.is_subset(&store.load()) { return Ok(()); }
    fs::create_dir_all(&store.directory)?;
    let lock = fs::OpenOptions::new().create(true).truncate(false).write(true)
        .open(store.directory.join("history.lock"))?;
    lock.lock()?;
    let mut previous = store.load();
    if names.is_subset(&previous) { return Ok(()); }
    previous.extend(names);
    store.artifact.store(&serde_json::to_vec(&previous).map_err(io::Error::other)?)
}
