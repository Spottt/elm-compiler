//! Verified whole-program output cache. This is not a cache of typed modules.
//! The graph owns the exact bytes consumed by the compiler, including kernels.
use crate::{kernel::Mode, project::Graph};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const MAGIC: &[u8] = b"PLANEXPO-ELM-CACHE-1\n";
static SERIAL: AtomicU64 = AtomicU64::new(0);
pub struct OutputCache {
    path: PathBuf,
    key: [u8; 32],
}
fn add(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
impl OutputCache {
    pub(crate) fn artifact(directory: &Path, slot: &[u8], key: [u8; 32]) -> Self {
        Self {
            path: directory.join(format!("{:x}.cache", Sha256::digest(slot))),
            key,
        }
    }
    /// Compiler identity is its content digest, not version or mtime: local
    /// compiler changes invalidate outputs before a new version is released.
    pub fn new(directory: &Path, graph: &Graph, compiler: &[u8]) -> Self {
        Self::new_in_mode(directory, graph, compiler, Mode::Development)
    }
    pub fn new_in_mode(directory: &Path, graph: &Graph, compiler: &[u8], mode: Mode) -> Self {
        let mode = match mode {
            Mode::Development => "development-javascript",
            Mode::Production => "production-javascript",
        };
        let mut hash = Sha256::new();
        add(&mut hash, MAGIC);
        add(&mut hash, mode.as_bytes());
        add(&mut hash, compiler);
        let entries = serde_json::to_vec(&graph.entries).expect("entry names serialize");
        add(&mut hash, graph.entry.as_bytes());
        add(&mut hash, &entries);
        add(&mut hash, &(graph.manifests.len() as u64).to_le_bytes());
        for (path, source) in &graph.manifests {
            add(&mut hash, path.as_os_str().as_encoded_bytes());
            add(&mut hash, source.as_bytes());
        }
        add(&mut hash, &(graph.modules.len() as u64).to_le_bytes());
        for module in &graph.modules {
            add(&mut hash, module.owner.as_bytes());
            add(&mut hash, module.name.as_bytes());
            add(&mut hash, module.path.as_os_str().as_encoded_bytes());
            add(&mut hash, &[module.kernel as u8]);
            add(&mut hash, module.source.as_bytes());
            add(&mut hash, &(module.imports.len() as u64).to_le_bytes());
            for import in &module.imports {
                add(&mut hash, import.as_bytes());
            }
        }
        // Keep only the latest output per entry and mode, bounding disk usage as sources
        // are edited instead of accumulating all historical bundles.
        let slot = format!("{:x}-{mode}", Sha256::digest(&entries));
        Self {
            path: directory.join(format!("{slot}.cache")),
            key: hash.finalize().into(),
        }
    }
    pub fn load(&self) -> Option<Vec<u8>> {
        let mut bytes = fs::read(&self.path).ok()?;
        let header = MAGIC.len() + 64;
        if bytes.len() < header || !bytes.starts_with(MAGIC) {
            return None;
        }
        if bytes[MAGIC.len()..MAGIC.len() + 32] != self.key {
            return None;
        }
        let output = &bytes[header..];
        let digest: [u8; 32] = Sha256::digest(output).into();
        if bytes[MAGIC.len() + 32..header] != digest {
            return None;
        }
        // Reuse the buffer rather than holding two copies of a large bundle.
        bytes.drain(..header);
        Some(bytes)
    }
    pub fn store(&self, output: &[u8]) -> io::Result<()> {
        let parent = self.path.parent().expect("cache path has a directory");
        fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(
            ".{}-{}.tmp",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| {
            file.write_all(MAGIC)?;
            file.write_all(&self.key)?;
            file.write_all(&Sha256::digest(output))?;
            file.write_all(output)?;
            file.sync_all()?;
            drop(file);
            // Concurrent readers see only complete generations, without a
            // shared lock or writes to Haskell's artifact directory.
            fs::rename(&temporary, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
pub fn compiler_identity(path: &Path) -> io::Result<Vec<u8>> {
    Ok(Sha256::digest(fs::read(path)?).to_vec())
}
