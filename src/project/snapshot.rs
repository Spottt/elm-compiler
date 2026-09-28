//! File observations retained only during one explicitly scoped worker batch.
//! No timestamp keys or persistent filesystem memoization.
use std::{cell::RefCell, collections::{HashMap, BTreeMap}, fs, io, path::{Path, PathBuf}, rc::Rc};
const SOURCE_LIMIT: usize = 64 * 1024 * 1024;
const RESOLUTION_LIMIT: usize = 8 * 1024 * 1024;
const PATH_LIMIT: usize = 65_536;
#[derive(Default)]
struct Snapshot {
    sources: HashMap<PathBuf, Source>,
    files: HashMap<PathBuf, bool>,
    canonical: HashMap<PathBuf, PathBuf>,
    bytes: usize,
    resolutions: Vec<Resolution>,
    resolution_bytes: usize,
}
struct Source { text: super::Source, digest: Option<[u8; 32]> }
struct Resolution {
    packages: BTreeMap<String, super::Package>,
    candidates: HashMap<(String, String), Vec<super::Candidate>>,
}
thread_local! { static ACTIVE: RefCell<Option<Snapshot>> = const { RefCell::new(None) }; }
/// Drop before processing the next batch. The Rc marker makes this guard
/// non-Send, so its thread-local state cannot be restored from another thread.
pub struct Scope { previous: Option<Snapshot>, _thread: std::marker::PhantomData<Rc<()>> }
pub fn scope() -> Scope {
    Scope { previous: ACTIVE.with(|active| active.replace(Some(Snapshot::default()))), _thread: std::marker::PhantomData }
}
impl Drop for Scope {
    fn drop(&mut self) { ACTIVE.with(|active| active.replace(self.previous.take())); }
}
// An identical package universe is required: owner names alone do not bind
// source roots, direct dependencies, exposed names or the selected versions.
pub(super) fn resolution(packages: &BTreeMap<String, super::Package>) -> Option<usize> {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut(); let cache = active.as_mut()?;
        if let Some(index) = cache.resolutions.iter().position(|r| &r.packages == packages) { return Some(index); }
        let bytes: usize = packages.iter().map(|(name, p)| 256 + name.len() + p.root.as_os_str().len()
            + p.roots.iter().map(|s| 64 + s.as_os_str().len()).sum::<usize>()
            + p.exposed.iter().chain(&p.dependencies).map(|s| 64 + s.len()).sum::<usize>()).sum();
        if cache.resolutions.len() >= 16 || bytes > RESOLUTION_LIMIT.saturating_sub(cache.resolution_bytes) { return None; }
        cache.resolution_bytes += bytes;
        cache.resolutions.push(Resolution { packages: packages.clone(), candidates: HashMap::new() });
        Some(cache.resolutions.len() - 1)
    })
}
pub(super) fn candidates(resolution: Option<usize>, key: &(String, String)) -> Option<Vec<super::Candidate>> {
    ACTIVE.with(|active| active.borrow().as_ref()?.resolutions.get(resolution?)?.candidates.get(key).cloned())
}
pub(super) fn remember_candidates(resolution: Option<usize>, key: (String, String), found: &[super::Candidate]) {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut(); let Some(cache) = active.as_mut() else { return; };
        let Some(resolution) = resolution.and_then(|index| cache.resolutions.get_mut(index)) else { return; };
        let bytes = 192 + key.0.len() + key.1.len() + found.iter().map(|(owner, path, _)| 96 + owner.len() + path.as_os_str().len()).sum::<usize>();
        if resolution.candidates.len() >= PATH_LIMIT || bytes > RESOLUTION_LIMIT.saturating_sub(cache.resolution_bytes) || resolution.candidates.contains_key(&key) { return; }
        cache.resolution_bytes += bytes;
        resolution.candidates.insert(key, found.to_vec());
    });
}
fn absolute(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() { Some(path.to_owned()) }
    else { std::env::current_dir().ok().map(|cwd| cwd.join(path)) }
}
// Only reuse a digest after comparing the graph's complete bytes with this
// batch's immutable file observation. A modified graph or uncached file falls
// back to hashing; paths and timestamps alone can never authorize reuse.
pub(crate) fn source_digest(path: &Path, source: &str) -> Option<[u8; 32]> {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut(); let cache = active.as_mut()?;
        let observed = cache.sources.get_mut(&absolute(path)?)?;
        if observed.text.as_str() != source { return None; }
        Some(*observed.digest.get_or_insert_with(|| crate::cache::content_digest(source.as_bytes())))
    })
}
pub(super) fn read_to_string(path: &Path) -> io::Result<super::Source> {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(cache) = active.as_mut() else { return fs::read_to_string(path).map(Into::into); };
        let Some(key) = absolute(path) else { return fs::read_to_string(path).map(Into::into); };
        if let Some(source) = cache.sources.get(&key) { return Ok(source.text.clone()); }
        let source: super::Source = fs::read_to_string(path)?.into();
        if cache.sources.len() < PATH_LIMIT && source.len() <= SOURCE_LIMIT.saturating_sub(cache.bytes) {
            cache.bytes += source.len();
            cache.sources.insert(key, Source { text: source.clone(), digest: None });
        }
        Ok(source)
    })
}
pub(super) fn is_file(path: &Path) -> bool {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(cache) = active.as_mut() else { return path.is_file(); };
        let Some(key) = absolute(path) else { return path.is_file(); };
        if let Some(value) = cache.files.get(&key) { return *value; }
        let value = path.is_file();
        if cache.files.len() < PATH_LIMIT { cache.files.insert(key, value); }
        value
    })
}
pub(super) fn canonicalize(path: &Path) -> io::Result<PathBuf> {
    ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        let Some(cache) = active.as_mut() else { return path.canonicalize(); };
        let Some(key) = absolute(path) else { return path.canonicalize(); };
        if let Some(value) = cache.canonical.get(&key) { return Ok(value.clone()); }
        let value = path.canonicalize()?;
        if cache.canonical.len() < PATH_LIMIT { cache.canonical.insert(key, value.clone()); }
        Ok(value)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_share_reads_but_never_survive_their_scope() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("source.elm");
        fs::write(&file, "old").unwrap();
        {
            let _scope = scope();
            assert_eq!(read_to_string(&file).unwrap(), "old");
            assert!(is_file(&file));
            fs::write(&file, "new").unwrap(); // same-size change
            assert_eq!(read_to_string(&file).unwrap(), "old");
            { let _nested = scope(); assert_eq!(read_to_string(&file).unwrap(), "new"); }
            assert_eq!(read_to_string(&file).unwrap(), "old");
        }
        assert_eq!(read_to_string(&file).unwrap(), "new");
        fs::remove_file(&file).unwrap();
        { let _scope = scope(); assert!(!is_file(&file)); assert!(read_to_string(&file).is_err()); }
        fs::write(&file, "fix").unwrap();
        { let _scope = scope(); assert!(is_file(&file)); assert_eq!(read_to_string(&file).unwrap(), "fix"); }
    }
    #[test]
    fn cached_sources_do_not_alias_mutable_graph_copies() {
        let directory = tempfile::tempdir().unwrap(); let file = directory.path().join("source");
        fs::write(&file, "source").unwrap();
        let _scope = scope();
        let mut first = read_to_string(&file).unwrap();
        let shared = read_to_string(&file).unwrap();
        assert_eq!(first.as_ptr(), shared.as_ptr());
        first.clear();
        assert_eq!(shared, "source");
        assert_eq!(read_to_string(&file).unwrap(), "source");
        ACTIVE.with(|active| assert_eq!(active.borrow().as_ref().unwrap().bytes, 6));
    }
}

#[cfg(test)]
mod resolution_tests {
    use super::*;
    #[test]
    fn shared_candidates_require_the_same_package_universe_and_batch() {
        let packages: BTreeMap<_, _> = [("app".into(), super::super::Package {
            root: "/app".into(), roots: vec!["/app/src".into()], exposed: Default::default(), dependencies: Default::default(),
        })].into_iter().collect();
        let key = ("app".into(), "Shared".into());
        let _scope = scope();
        let id = resolution(&packages); assert!(id.is_some());
        remember_candidates(id, key.clone(), &[("app".into(), "/app/src/Shared.elm".into(), false)]);
        assert_eq!(candidates(resolution(&packages), &key).unwrap().len(), 1);
        for field in 0..4 {
            let mut changed = packages.clone(); let package = changed.get_mut("app").unwrap();
            match field {
                0 => package.root = "/other".into(),
                1 => package.roots.push("/other/src".into()),
                2 => { package.exposed.insert("Shared".into()); },
                _ => { package.dependencies.insert("other/provider".into()); },
            }
            assert!(candidates(resolution(&changed), &key).is_none());
        }
        drop(_scope);
        let _scope = scope(); assert!(candidates(resolution(&packages), &key).is_none());
    }
}

#[cfg(test)]
mod digest_tests {
    use super::*;
    #[test]
    fn source_digest_never_reuses_a_path_for_different_bytes() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("Main.elm");
        fs::write(&path, "old").unwrap();
        let _scope = scope();
        assert!(source_digest(&path, "old").is_none()); // not observed yet
        assert_eq!(read_to_string(&path).unwrap(), "old");
        let digest = source_digest(&path, "old").unwrap();
        assert_eq!(digest, crate::cache::content_digest(b"old"));
        assert!(source_digest(&path, "new").is_none()); // same-size graph mutation
        fs::write(&path, "new").unwrap();
        assert_eq!(source_digest(&path, "old"), Some(digest)); // batch observation remains frozen
        drop(_scope); let _scope = scope();
        assert!(source_digest(&path, "old").is_none());
        assert_eq!(read_to_string(&path).unwrap(), "new");
        assert!(source_digest(&path, "old").is_none());
        assert_eq!(source_digest(&path, "new").unwrap(), crate::cache::content_digest(b"new"));
    }
}
