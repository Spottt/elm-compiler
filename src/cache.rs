//! Verified whole-program output cache. This is not a cache of typed modules.
//! The graph owns the exact bytes consumed by the compiler, including kernels.
use crate::{kernel::Mode, project::Graph};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
mod output_memory;
pub use output_memory::{scope as output_memory_scope, Scope as OutputMemoryScope};
const MAGIC: &[u8] = b"PLANEXPO-ELM-CACHE-1\n";
static SERIAL: AtomicU64 = AtomicU64::new(0);
/// Exact byte identities shared by graph and decoded-session caches.
/// Persistent artifact checksums and published identities remain SHA-256.
pub(crate) fn content_digest(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

/// Fresh content digests tied to an immutable graph. Private fields prevent a
/// caller from pairing digests with different sources, including same-size edits.
pub struct SourceDigests<'graph> {
    graph: &'graph Graph,
    keys: Vec<[u8; 32]>,
}
impl<'graph> SourceDigests<'graph> {
    pub fn new(graph: &'graph Graph) -> Self {
        Self { graph, keys: graph.modules.iter().map(|m| crate::project::scoped_source_digest(&m.path, &m.source).unwrap_or_else(|| content_digest(m.source.as_bytes()))).collect() }
    }
    pub(crate) fn graph(&self) -> &'graph Graph { self.graph }
    pub(crate) fn key(&self, index: usize) -> &[u8; 32] { &self.keys[index] }
}
pub struct OutputCache {
    whole_output: bool,
    path: PathBuf,
    key: [u8; 32],
}
pub struct TriviaCache<'graph> {
    output: OutputCache,
    graph: &'graph Graph,
}
impl TriviaCache<'_> {
    pub fn load(&self) -> Option<Vec<u8>> {
        let output = output_memory::load(&self.output.path.with_extension("cache"), &self.output.key, true).or_else(|| {
            let expected: [u8; 32] = self.output.load()?.try_into().ok()?;
            OutputCache::load_verified(&self.output.path.with_extension("cache"), None, Some(&expected))
        })?;
        // Do the extra parse only when an equivalent successful output exists.
        // Real code edits miss the key and use the normal compiler's validation.
        for module in self.graph.modules.iter().filter(|module| !module.kernel) {
            crate::parser::check_syntax(&module.source).ok()?;
        }
        Some(output)
    }
    pub fn store(&self, output: &[u8]) -> io::Result<()> {
        let digest: [u8; 32] = Sha256::digest(output).into();
        let payload = OutputCache { whole_output: true, path: self.output.path.with_extension("cache"), key: self.output.key };
        payload.store_verified(output, &digest)?;
        self.output.store(&digest)
    }
}
fn add(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
impl OutputCache {
    pub(crate) fn artifact(directory: &Path, slot: &[u8], key: [u8; 32]) -> Self {
        Self {
            whole_output: false,
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
        Self::new_with_digests(directory, &SourceDigests::new(graph), compiler, mode)
    }
    pub fn new_with_digests(directory: &Path, sources: &SourceDigests<'_>, compiler: &[u8], mode: Mode) -> Self {
        Self::with_source_keys(directory, sources.graph, compiler, mode, &sources.keys, b"source-digests-blake3-v2")
    }
    /// Optional successful-output cache for trivia-only edits. Keep every token's
    /// spelling and coordinates (including Debug.todo locations), and validate
    /// the fresh syntax before reusing output. Diagnostics never use this key.
    pub fn new_for_trivia<'graph>(directory: &Path, graph: &'graph Graph, compiler: &[u8]) -> Option<TriviaCache<'graph>> {
        Self::new_for_trivia_with_digests(directory, &SourceDigests::new(graph), compiler)
    }
    pub fn new_for_trivia_with_digests<'graph>(directory: &Path, sources: &SourceDigests<'graph>, compiler: &[u8]) -> Option<TriviaCache<'graph>> {
        let graph = sources.graph;
        if !graph.import_errors.is_empty() || graph.modules.iter().any(|module| module.missing_header.is_some()) {
            return None;
        }
        let keys = graph.modules.iter().enumerate().map(|(index, module)| {
            if module.kernel { return Some(*sources.key(index)); }
            crate::session_cache::generated_tokens_with_digest(sources.key(index), || {
                let tokens = crate::lexer::lex(&module.source).ok()?;
                let mut hash = Sha256::new();
                for token in tokens {
                    add(&mut hash, &[token.kind as u8]);
                    add(&mut hash, &token.row.to_le_bytes());
                    add(&mut hash, &token.column.to_le_bytes());
                    add(&mut hash, token.text(&module.source).as_bytes());
                }
                Some(hash.finalize().into())
            })
        }).collect::<Option<Vec<[u8; 32]>>>()?;
        let mut cache = Self::with_source_keys(directory, graph, compiler, Mode::Development, &keys, b"token-coordinates-reference-v2");
        cache.path.set_extension("trivia");
        cache.whole_output = false;
        Some(TriviaCache { output: cache, graph })
    }
    fn with_source_keys(directory: &Path, graph: &Graph, compiler: &[u8], mode: Mode, source_keys: &[[u8; 32]], domain: &[u8]) -> Self {
        let mode = match mode {
            Mode::Development => "development-javascript",
            Mode::Debug => "debug-javascript",
            Mode::Production => "production-javascript",
        };
        let mut hash = Sha256::new();
        add(&mut hash, MAGIC);
        add(&mut hash, domain);
        add(&mut hash, mode.as_bytes());
        add(&mut hash, compiler);
        if !graph.import_errors.is_empty() {
            // Import suggestions can change after another entry builds, without
            // changing this graph's source files. Do not replay a stale report.
            add(&mut hash, &serde_json::to_vec(&graph.import_errors).expect("import reports serialize"));
        }
        let entries = serde_json::to_vec(&graph.entries).expect("entry names serialize");
        add(&mut hash, graph.entry.as_bytes());
        add(&mut hash, &entries);
        add(&mut hash, &(graph.manifests.len() as u64).to_le_bytes());
        for (path, source) in &graph.manifests {
            add(&mut hash, path.as_os_str().as_encoded_bytes());
            add(&mut hash, source.as_bytes());
        }
        add(&mut hash, &(graph.modules.len() as u64).to_le_bytes());
        for (index, module) in graph.modules.iter().enumerate() {
            add(&mut hash, module.owner.as_bytes());
            add(&mut hash, module.name.as_bytes());
            add(&mut hash, module.path.as_os_str().as_encoded_bytes());
            add(&mut hash, &[module.kernel as u8]);
            add(&mut hash, &source_keys[index]);
            add(&mut hash, &(module.imports.len() as u64).to_le_bytes());
            for import in &module.imports {
                add(&mut hash, import.as_bytes());
            }
        }
        // Keep only the latest output per entry and mode, bounding disk usage as sources
        // are edited instead of accumulating all historical bundles.
        let slot = format!("{:x}-{mode}", Sha256::digest(&entries));
        Self {
            whole_output: true,
            path: directory.join(format!("{slot}.cache")),
            key: hash.finalize().into(),
        }
    }
    /// Separate slot with the same complete graph/compiler identity. Diagnostics
    /// must never be mistaken for a successful JavaScript output.
    pub fn diagnostic_slot(&self) -> Self {
        Self { whole_output: false, path: self.path.with_extension("diagnostic"), key: self.key }
    }
    pub fn load(&self) -> Option<Vec<u8>> {
        if self.whole_output && let Some(bytes) = output_memory::load(&self.path, &self.key, false) { return Some(bytes); }
        Self::load_verified(&self.path, Some(&self.key), None)
    }
    fn load_verified(path: &Path, key: Option<&[u8; 32]>, expected: Option<&[u8; 32]>) -> Option<Vec<u8>> {
        let mut file = fs::File::open(path).ok()?;
        let mut header = [0u8; MAGIC.len() + 64];
        file.read_exact(&mut header).ok()?;
        if !header.starts_with(MAGIC) {
            return None;
        }
        if key.is_some_and(|key| header[MAGIC.len()..MAGIC.len() + 32] != *key)
            || expected.is_some_and(|digest| header[MAGIC.len() + 32..] != *digest) {
            return None;
        }
        // A stale key is common on each edit. Reject it before reading and
        // allocating a large payload; hits read directly into the final buffer.
        let mut bytes = Vec::new();
        let length = usize::try_from(file.metadata().ok()?.len()).ok()?.saturating_sub(header.len());
        bytes.try_reserve_exact(length).ok()?;
        file.read_to_end(&mut bytes).ok()?;
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        if header[MAGIC.len() + 32..] != digest {
            return None;
        }
        Some(bytes)
    }
    pub fn store(&self, output: &[u8]) -> io::Result<()> {
        self.store_verified(output, &Sha256::digest(output).into())
    }
    /// Both cache keys share one payload and one payload checksum calculation.
    /// Concurrent replacement can cause a miss, never an unchecked reuse.
    pub fn store_with_trivia(&self, output: &[u8], trivia: Option<&TriviaCache<'_>>) -> io::Result<()> {
        let trivia_key = trivia.filter(|t| t.output.path.with_extension("cache") == self.path).map(|t| t.output.key);
        if self.whole_output && output_memory::store(&self.path, self.key, trivia_key, output) { return Ok(()); }
        let digest: [u8; 32] = Sha256::digest(output).into();
        self.store_verified(output, &digest)?;
        if let Some(trivia) = trivia
            && trivia.output.path.with_extension("cache") == self.path {
            let _ = trivia.output.store(&digest);
        }
        Ok(())
    }
    fn store_verified(&self, output: &[u8], digest: &[u8; 32]) -> io::Result<()> {
        // Other public writers must replace any older in-memory generation too.
        output_memory::invalidate(&self.path);
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
            file.write_all(digest)?;
            file.write_all(output)?;
            // Cache entries are disposable and verified by key and payload digest
            // on load. Atomic publication is needed; crash durability is not.
            // Avoid forcing every module artifact to disk on the critical path.
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

// Bind caches to the running image, even if its file is replaced while a worker lives.
pub fn running_compiler_identity() -> Option<Vec<u8>> {
    static IDENTITY: std::sync::OnceLock<Option<Vec<u8>>> = std::sync::OnceLock::new();
    IDENTITY.get_or_init(|| std::env::current_exe().ok()
        .and_then(|path| compiler_identity(&path).ok())).clone()
}
