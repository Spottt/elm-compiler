//! Package server transport. Registry synchronization follows Deps.Registry and
//! Deps.Solver.initEnv: failed updates may use an existing registry offline.
use crate::{
    package_solver::{Version, valid_name},
    registry::Registry,
};
use reqwest::blocking::Client;
use sha1::{Digest, Sha1};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    time::Duration,
};

pub const PACKAGE_SERVER: &str = "https://package.elm-lang.org";
const MAX_REGISTRY_BYTES: u64 = 64 * 1024 * 1024;

pub struct PackageNetwork {
    client: Client,
    server: String,
}
pub struct RegistryState {
    pub registry: Registry,
    /// A failed update with a valid cache must also disable metadata downloads.
    pub online: bool,
}
impl PackageNetwork {
    /// The explicit server argument permits isolated protocol tests. Production
    /// callers should use PACKAGE_SERVER; no environment override is installed.
    pub fn new(server: &str) -> Result<Self, String> {
        let url = reqwest::Url::parse(server).map_err(|e| e.to_string())?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("invalid package server URL".into());
        }
        let client = Client::builder()
            .user_agent("elm/0.19.1")
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            server: server.trim_end_matches('/').into(),
        })
    }
    pub fn registry(&self, home: &Path) -> Result<RegistryState, String> {
        let directory = home.join("0.19.1/packages");
        fs::create_dir_all(&directory).map_err(|e| format!("{}: {e}", directory.display()))?;
        // Same path and exclusive OS lock as Stuff.withRegistryLock. The file
        // remains after release, like Elm's lock; process exit releases the lock.
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("lock"))
            .map_err(|e| e.to_string())?;
        lock.lock()
            .map_err(|e| format!("cannot lock package cache: {e}"))?;
        let cached = Registry::read_cached(home)?;
        let route = match &cached {
            Some(registry) => format!("/all-packages/since/{}", registry.count()),
            None => "/all-packages".into(),
        };
        let downloaded = self.post_registry(&route).and_then(|value| match &cached {
            Some(registry) => registry.updated(&value),
            None => Registry::from_json(&value),
        });
        let registry = match downloaded {
            Ok(registry) => registry,
            Err(error) => {
                return match cached {
                    Some(registry) => Ok(RegistryState {
                        registry,
                        online: false,
                    }),
                    None => Err(crate::dependency_error::registry_problem(&error)),
                };
            }
        };
        if cached.as_ref() != Some(&registry) {
            // Never replace a valid registry with a partial HTTP body or a
            // partially written binary. The temporary file is on the same FS.
            let mut temp =
                tempfile::NamedTempFile::new_in(&directory).map_err(|e| e.to_string())?;
            temp.write_all(&registry.encode()?)
                .map_err(|e| e.to_string())?;
            temp.as_file().sync_all().map_err(|e| e.to_string())?;
            temp.persist(directory.join("registry.dat"))
                .map_err(|e| e.to_string())?;
        }
        Ok(RegistryState {
            registry,
            online: true,
        })
    }
    /// The solver needs only metadata while backtracking, not every archive.
    pub fn metadata(
        &self,
        home: &Path,
        name: &str,
        version: Version,
    ) -> Result<serde_json::Value, String> {
        if !valid_name(name) {
            return Err(format!("invalid package name {name}"));
        }
        let cache = home.join("0.19.1/packages");
        fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(cache.join("lock"))
            .map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
        let root = cache.join(name).join(version.to_string());
        let path = root.join("elm.json");
        if let Some(config) = read_metadata_path(&path, name, version)? {
            return Ok(config);
        }
        let url = format!("{}/packages/{name}/{version}/elm.json", self.server);
        let bytes = self.get_bytes(&url, 1024 * 1024)?;
        let config: serde_json::Value = crate::outline::decode_bytes(&bytes)
            .map_err(|e| format!("invalid package metadata: {e}"))?;
        crate::outline::validate_package_metadata(&config)
            .map_err(|e| format!("invalid package metadata: {}", e.message))?;
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let mut temp = tempfile::NamedTempFile::new_in(&root).map_err(|e| e.to_string())?;
        temp.write_all(&bytes).map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        temp.persist(path).map_err(|e| e.to_string())?;
        Ok(config)
    }
    /// Acquire a package source tree. A failed download never modifies the
    /// existing tree; the shared package lock also serializes publication.
    pub fn package(
        &self,
        home: &Path,
        name: &str,
        version: Version,
    ) -> Result<std::path::PathBuf, String> {
        if !valid_name(name) {
            return Err(format!("invalid package name {name}"));
        }
        let cache = home.join("0.19.1/packages");
        fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(cache.join("lock"))
            .map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
        let destination = cache.join(name).join(version.to_string());
        if destination.join("src").is_dir() && destination.join("elm.json").is_file() {
            return Ok(destination);
        }
        let endpoint_url = format!("{}/packages/{name}/{version}/endpoint.json", self.server);
        let endpoint: serde_json::Value =
            serde_json::from_slice(&self.get_bytes(&endpoint_url, 1024 * 1024)?)
                .map_err(|e| format!("invalid package endpoint: {e}"))?;
        let url = endpoint["url"]
            .as_str()
            .ok_or("package endpoint missing URL")?;
        let hash = endpoint["hash"]
            .as_str()
            .ok_or("package endpoint missing hash")?;
        if hash.len() != 40
            || !hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err("invalid package archive hash".into());
        }
        let bytes = self.get_bytes(url, 64 * 1024 * 1024)?;
        let actual = format!("{:x}", Sha1::digest(&bytes));
        if actual != hash {
            return Err(format!(
                "package archive hash mismatch: expected {hash}, received {actual}"
            ));
        }
        let parent = destination.parent().unwrap();
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let staging = tempfile::tempdir_in(parent).map_err(|e| e.to_string())?;
        let tree = staging.path().join("source");
        fs::create_dir(&tree).map_err(|e| e.to_string())?;
        crate::package_archive::extract(&bytes, &tree)?;
        // The solver may have cached only elm.json. Keep it until all source
        // files are validated, and restore it if publication fails.
        let backup = staging.path().join("previous");
        let existed = destination.exists();
        if existed {
            fs::rename(&destination, &backup).map_err(|e| e.to_string())?;
        }
        if let Err(error) = fs::rename(&tree, &destination) {
            if existed && let Err(restore) = fs::rename(&backup, &destination) {
                let saved = staging.keep();
                return Err(format!(
                    "package publication failed: {error}; restore failed: {restore}; original retained in {}",
                    saved.display()
                ));
            }
            return Err(format!("package publication failed: {error}"));
        }
        Ok(destination)
    }
    fn get_bytes(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let parsed = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
        if !matches!(parsed.scheme(), "https" | "http") {
            return Err("unsupported package URL scheme".into());
        }
        let response = self
            .client
            .get(parsed)
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("{url}: {e}"))?;
        let mut bytes = Vec::new();
        response
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > limit {
            return Err(format!("{url}: response size limit exceeded"));
        }
        Ok(bytes)
    }
    fn post_registry(&self, route: &str) -> Result<serde_json::Value, String> {
        let url = format!("{}{route}", self.server);
        let response = self
            .client
            .post(&url)
            .header(reqwest::header::CONTENT_LENGTH, 0)
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("{url}: {e}"))?;
        let mut bytes = Vec::new();
        response
            .take(MAX_REGISTRY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{url}: {e}"))?;
        if bytes.len() as u64 > MAX_REGISTRY_BYTES {
            return Err(format!("{url}: registry response exceeds 64 MiB"));
        }
        serde_json::from_slice(&bytes).map_err(|e| format!("{url}: invalid registry response: {e}"))
    }
}

/// Read and validate a cached outline under the same lock as downloads.
/// Like Deps.Solver, invalid data removes only elm.json and fails this attempt.
pub(crate) fn cached_metadata(
    home: &Path,
    name: &str,
    version: Version,
) -> Result<Option<serde_json::Value>, String> {
    let cache = home.join("0.19.1/packages");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(cache.join("lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    read_metadata_path(
        &cache.join(name).join(version.to_string()).join("elm.json"),
        name,
        version,
    )
}

fn read_metadata_path(
    path: &Path,
    name: &str,
    version: Version,
) -> Result<Option<serde_json::Value>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let decoded = crate::outline::decode_bytes(&bytes).and_then(|config| {
        crate::outline::validate_package_metadata(&config).map_err(|e| e.message)?;
        Ok(config)
    });
    match decoded {
        Ok(config) => Ok(Some(config)),
        Err(problem) => {
            fs::remove_file(path).map_err(|e| {
                format!(
                    "{}: invalid metadata ({problem}); cannot remove: {e}",
                    path.display()
                )
            })?;
            Err(crate::dependency_error::bad_cache(name, version))
        }
    }
}
