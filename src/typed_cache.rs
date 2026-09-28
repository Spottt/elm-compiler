//! Inferred-module artifacts keyed by own source and dependency interfaces,
//! with transitive source keys as a conservative fallback. Kernel cycles are
//! runtime edges; all kernel sources conservatively invalidate all type entries.
use crate::{cache::OutputCache, kernel::Mode, project::Graph};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

pub struct TypeCache {
    artifacts: BTreeMap<String, OutputCache>,
    semantic: BTreeMap<String, (Sha256, Vec<String>)>,
    source_keys: BTreeMap<String, [u8; 32]>,
    directory: PathBuf,
    mode: String,
}
fn add(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value);
}
impl TypeCache {
    pub fn new(
        directory: &Path,
        graph: &Graph,
        compiler: &[u8],
        mode: Mode,
    ) -> Result<Self, String> {
        let mut base = Sha256::new();
        add(&mut base, b"planexpo-typed-modules-v1");
        add(&mut base, compiler);
        let mode = match mode {
            Mode::Development => "development",
            Mode::Production => "production",
        };
        add(&mut base, mode.as_bytes());
        for (path, source) in &graph.manifests {
            add(&mut base, path.as_os_str().as_encoded_bytes());
            add(&mut base, source.as_bytes());
        }
        let kernels: BTreeMap<_, _> = graph
            .modules
            .iter()
            .filter(|m| m.kernel)
            .map(|m| (format!("{}:{}", m.owner, m.name), m))
            .collect();
        for (id, module) in &kernels {
            add(&mut base, id.as_bytes());
            add(&mut base, module.path.as_os_str().as_encoded_bytes());
            add(&mut base, module.source.as_bytes());
            for import in &module.imports {
                add(&mut base, import.as_bytes());
            }
        }
        let mut keys = BTreeMap::<String, [u8; 32]>::new();
        let mut artifacts = BTreeMap::new();
        let mut semantic = BTreeMap::new();
        for module in graph.modules.iter().filter(|m| !m.kernel) {
            let id = format!("{}:{}", module.owner, module.name);
            let mut hash = base.clone();
            add(&mut hash, id.as_bytes());
            add(&mut hash, module.path.as_os_str().as_encoded_bytes());
            add(&mut hash, module.source.as_bytes());
            semantic.insert(
                id.clone(),
                (
                    hash.clone(),
                    module
                        .imports
                        .iter()
                        .filter(|import| !kernels.contains_key(*import))
                        .cloned()
                        .collect(),
                ),
            );
            for import in &module.imports {
                add(&mut hash, import.as_bytes());
                if !kernels.contains_key(import) {
                    add(
                        &mut hash,
                        keys.get(import).ok_or_else(|| {
                            format!("type cache dependency {import} must precede {id}")
                        })?,
                    );
                }
            }
            let key: [u8; 32] = hash.finalize().into();
            let artifact = OutputCache::artifact(directory, format!("{mode}:{id}").as_bytes(), key);
            if keys.insert(id.clone(), key).is_some() {
                return Err(format!("duplicate type cache module {id}"));
            }
            artifacts.insert(id, artifact);
        }
        Ok(Self {
            artifacts,
            semantic,
            source_keys: keys,
            directory: directory.into(),
            mode: mode.into(),
        })
    }
    /// Dependencies are already checked or restored in topological order.
    /// Missing interface hashes conservatively use transitive source keys.
    pub fn for_interfaces(
        &self,
        module: &str,
        interfaces: &BTreeMap<String, [u8; 32]>,
    ) -> Option<OutputCache> {
        let (base, imports) = self.semantic.get(module)?;
        let mut hash = base.clone();
        add(&mut hash, b"public-interface-v1");
        for import in imports {
            add(&mut hash, import.as_bytes());
            add(
                &mut hash,
                interfaces
                    .get(import)
                    .or_else(|| self.source_keys.get(import))?,
            );
        }
        Some(OutputCache::artifact(
            &self.directory,
            format!("{}:{module}", self.mode).as_bytes(),
            hash.finalize().into(),
        ))
    }
    pub fn load(&self, module: &str) -> Option<serde_json::Value> {
        let bytes = self.artifacts.get(module)?.load()?;
        serde_json::from_slice(&bytes).ok()
    }
    pub fn store(&self, module: &str, artifact: &serde_json::Value) -> io::Result<()> {
        let cache = self
            .artifacts
            .get(module)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "unknown type cache module"))?;
        cache.store(&serde_json::to_vec(artifact)?)
    }
}
