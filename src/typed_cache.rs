//! Inferred-module artifacts keyed by own source and dependency interfaces,
//! with transitive source keys as a conservative fallback. Kernel cycles are
//! runtime edges; all kernel sources conservatively invalidate all type entries.
use crate::{cache::{OutputCache, SourceDigests}, kernel::Mode, project::Graph};
use sha2::{Digest, Sha256};
mod text_edits;
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

pub struct TypeCache {
    artifacts: BTreeMap<String, OutputCache>,
    semantic: BTreeMap<String, (Sha256, Vec<String>)>,
    text_semantic: BTreeMap<String, (Sha256, Vec<String>)>,
    source_keys: BTreeMap<String, [u8; 32]>,
    directory: PathBuf,
    mode: String,
    diagnostics: bool,
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
        Self::new_with_digests(directory, &SourceDigests::new(graph), compiler, mode)
    }
    pub fn new_with_digests(directory: &Path, sources: &SourceDigests<'_>, compiler: &[u8], mode: Mode) -> Result<Self, String> {
        Self::with_text_edits(directory, sources, compiler, mode, std::env::var("PLANEXPO_ELM_TEXT_TYPE_CACHE").as_deref() == Ok("1"))
    }
    fn with_text_edits(directory: &Path, sources: &SourceDigests<'_>, compiler: &[u8], mode: Mode, text_edits: bool) -> Result<Self, String> {
        let graph = sources.graph();
        let mut base = Sha256::new();
        add(&mut base, b"planexpo-typed-modules-source-digests-blake3-v3");
        add(&mut base, compiler);
        let mode = match mode {
            Mode::Development => "development",
            Mode::Debug => "debug",
            Mode::Production => "production",
        };
        add(&mut base, mode.as_bytes());
        for (path, source) in &graph.manifests {
            add(&mut base, path.as_os_str().as_encoded_bytes());
            add(&mut base, source.as_bytes());
        }
        let kernels: BTreeMap<_, _> = graph
            .modules
            .iter().enumerate()
            .filter(|(_, m)| m.kernel)
            .map(|(i, m)| (format!("{}:{}", m.owner, m.name), (i, m)))
            .collect();
        for (id, (index, module)) in &kernels {
            add(&mut base, id.as_bytes());
            add(&mut base, module.path.as_os_str().as_encoded_bytes());
            add(&mut base, sources.key(*index));
            for import in &module.imports {
                add(&mut base, import.as_bytes());
            }
        }
        let mut keys = BTreeMap::<String, [u8; 32]>::new();
        let mut artifacts = BTreeMap::new();
        let mut semantic = BTreeMap::new();
        let mut text_semantic = BTreeMap::new();
        let text_edits = mode == "development" && text_edits;
        for (index, module) in graph.modules.iter().enumerate().filter(|(_, m)| !m.kernel) {
            let id = format!("{}:{}", module.owner, module.name);
            let mut hash = base.clone();
            add(&mut hash, id.as_bytes());
            add(&mut hash, module.path.as_os_str().as_encoded_bytes());
            if text_edits {
                text_semantic.insert(id.clone(), (hash.clone(), module.imports.iter().filter(|import| !kernels.contains_key(*import)).cloned().collect()));
            }
            add(&mut hash, sources.key(index));
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
            text_semantic,
            source_keys: keys,
            directory: directory.into(),
            mode: mode.into(),
            diagnostics: false,
        })
    }
    /// Display metadata (aliases and source variable names) must never be read
    /// from compact artifacts. Source-transitive keys intentionally invalidate
    /// more broadly than public API fingerprints: names can change without a
    /// semantic type change and still affect the exact diagnostic.
    pub fn for_diagnostics(&self) -> Self {
        let directory = self.directory.join("diagnostics-v1");
        let artifacts = self.source_keys.iter().map(|(id, key)| {
            (id.clone(), OutputCache::artifact(&directory, format!("{}:{id}", self.mode).as_bytes(), *key))
        }).collect();
        Self {
            artifacts,
            semantic: BTreeMap::new(),
            text_semantic: BTreeMap::new(),
            source_keys: self.source_keys.clone(),
            directory,
            mode: self.mode.clone(),
            diagnostics: true,
        }
    }
    /// Display artifacts use transitive source keys, so public type hashes
    /// cannot improve their reuse and need not be computed.
    pub fn uses_interface_fingerprints(&self) -> bool {
        !self.diagnostics
    }
    /// Opt-in inference reuse for parsed expression-string changes. Generated
    /// code, exact-source validation proofs and diagnostics keep their own keys.
    pub(crate) fn for_text_edits(&self, module: &str, ast: &crate::ast::Syntax<'_>, interfaces: &BTreeMap<String, [u8; 32]>) -> Option<OutputCache> {
        let (base, imports) = self.text_semantic.get(module)?;
        let mut hash = base.clone();
        add(&mut hash, &text_edits::identity(ast)?);
        for import in imports {
            add(&mut hash, import.as_bytes());
            add(&mut hash, interfaces.get(import).or_else(|| self.source_keys.get(import))?);
        }
        Some(OutputCache::artifact(&self.directory.join("expression-text-v1"), module.as_bytes(), hash.finalize().into()))
    }
    /// Dependencies are already checked or restored in topological order.
    /// Missing interface hashes conservatively use transitive source keys.
    pub fn for_interfaces(
        &self,
        module: &str,
        interfaces: &BTreeMap<String, [u8; 32]>,
    ) -> Option<OutputCache> {
        if self.diagnostics {
            return Some(OutputCache::artifact(
                &self.directory,
                format!("{}:{module}", self.mode).as_bytes(),
                *self.source_keys.get(module)?,
            ));
        }
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
    /// Development code generation depends on the full transitive source graph.
    /// Use separate slots so cached inference and generated code never overwrite each other.
    pub fn generated(&self, module: &str) -> Option<OutputCache> {
        if self.diagnostics || self.mode != "development" { return None; }
        Some(OutputCache::artifact(
            &self.directory.join("generated-v1"), module.as_bytes(), *self.source_keys.get(module)?,
        ))
    }
    /// Development output names constructors and fields symbolically; it does
    /// not inline imported implementations or use production constructor tags.
    /// Own source plus dependency types therefore suffice, except that operator
    /// references also depend on their private implementation's symbol name.
    /// Name resolution and entry validation still run before reuse. Coverage may
    /// reuse an explicit success proof from a verified typed artifact.
    pub fn generated_for_interfaces(
        &self,
        module: &str,
        interfaces: &BTreeMap<String, [u8; 32]>,
        names: &BTreeMap<String, std::rc::Rc<crate::names::Interface>>,
        symbols: &crate::names::Symbols,
    ) -> Option<OutputCache> {
        if self.diagnostics || self.mode != "development" { return None; }
        let (base, imports) = self.semantic.get(module)?;
        let mut hash = base.clone();
        add(&mut hash, b"generated-public-interface-v1");
        for import in imports {
            add(&mut hash, import.as_bytes());
            if let Some((signature, names)) = interfaces.get(import).zip(names.get(import)) {
                add(&mut hash, b"interface");
                add(&mut hash, signature);
                for (operator, target) in &names.operators {
                    let target = symbols.get(*target);
                    add(&mut hash, operator.as_bytes());
                    add(&mut hash, target.module.as_bytes());
                    add(&mut hash, target.name.as_bytes());
                }
            } else {
                add(&mut hash, b"source");
                add(&mut hash, self.source_keys.get(import)?);
            }
        }
        Some(OutputCache::artifact(
            &self.directory.join("generated-interfaces-v1"),
            module.as_bytes(), hash.finalize().into(),
        ))
    }
    /// Full source identity for in-process proofs of successful analysis.
    /// Display replays and other compilation modes cannot reuse these proofs.
    pub(crate) fn validation_source_key(&self, module: &str) -> Option<[u8; 32]> {
        if self.diagnostics || self.mode != "development" { return None; }
        self.source_keys.get(module).copied()
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
