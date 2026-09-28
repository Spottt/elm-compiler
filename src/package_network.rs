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

#[derive(Debug)]
pub enum DocumentationError {
    CorruptCache,
    InvalidData { url: String, body: Vec<u8> },
    Other(String),
}
impl From<String> for DocumentationError {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}
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
        crate::proxy_environment::validate()?;
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
        self.registry_with_policy(home, None)
    }
    /// Commands comparing published APIs must never fall back to a stale registry.
    pub fn latest_registry(&self, home: &Path) -> Result<Registry, String> {
        self.latest_registry_with_context(
            home,
            "I need the latest list of published packages before I do this diff",
        )
    }
    pub fn latest_registry_with_context(
        &self,
        home: &Path,
        context: &str,
    ) -> Result<Registry, String> {
        self.registry_with_policy(home, Some(context))
            .map(|state| state.registry)
    }
    fn registry_with_policy(
        &self,
        home: &Path,
        require_latest: Option<&str>,
    ) -> Result<RegistryState, String> {
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
        let url = format!("{}{route}", self.server);
        let downloaded = self
            .post_registry(&route)
            .map_err(|error| (error, None))
            .and_then(|bytes| {
                let parsed = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("{url}: invalid registry response: {e}"))
                    .and_then(|value| match &cached {
                        Some(registry) => registry.updated(&value),
                        None => Registry::from_json(&value),
                    });
                parsed.map_err(|error| (error, Some(bytes)))
            });
        let registry = match downloaded {
            Ok(registry) => registry,
            Err((error, body)) => {
                if let Some(context) = require_latest {
                    return Err(match body {
                        Some(body) => crate::diff_diagnostic::invalid_response(
                            "PROBLEM UPDATING PACKAGE LIST",
                            context,
                            &url,
                            &body,
                        ),
                        None => crate::diff_diagnostic::registry_context(context, &url, &error),
                    });
                }
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
    /// Read the documented public API, following Deps.Diff.getDocs cache rules.
    pub fn documentation(
        &self,
        home: &Path,
        name: &str,
        version: Version,
    ) -> Result<serde_json::Value, DocumentationError> {
        if !valid_name(name) {
            return Err(format!("invalid package name {name}").into());
        }
        let root = home
            .join("0.19.1/packages")
            .join(name)
            .join(version.to_string());
        let path = root.join("docs.json");
        let decode = |bytes: &[u8]| -> Result<serde_json::Value, String> {
            let value = serde_json::from_slice(bytes)
                .map_err(|e| format!("invalid package documentation: {e}"))?;
            crate::api_diff::diff(&value, &serde_json::json!([]))?;
            Ok(value)
        };
        match fs::read(&path) {
            Ok(bytes) => match decode(&bytes) {
                Ok(value) => return Ok(value),
                Err(_) => {
                    fs::remove_file(&path).map_err(|e| e.to_string())?;
                    return Err(DocumentationError::CorruptCache);
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string().into()),
        }
        let url = format!("{}/packages/{name}/{version}/docs.json", self.server);
        let bytes = self.get_bytes(&url, MAX_REGISTRY_BYTES)?;
        let value = decode(&bytes).map_err(|_| DocumentationError::InvalidData {
            url,
            body: bytes.clone(),
        })?;
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(&root).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(&path).map_err(|e| e.to_string())?;
        Ok(value)
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
        let mut download = crate::package_progress::Download::start(name, version);
        let endpoint_url = format!("{}/packages/{name}/{version}/endpoint.json", self.server);
        let bad_endpoint =
            || crate::dependency_error::bad_download_content(name, version, &endpoint_url, true);
        let endpoint: serde_json::Value = serde_json::from_slice(&self.get_bytes_with_context(
            &endpoint_url,
            1024 * 1024,
            Some(&format!(
                "I need to find the latest download link for {name} {version}"
            )),
        )?)
        .map_err(|_| bad_endpoint())?;
        let url = endpoint["url"].as_str().ok_or_else(bad_endpoint)?;
        let hash = endpoint["hash"].as_str().ok_or_else(bad_endpoint)?;
        let bytes = self.get_bytes_with_context(
            url,
            64 * 1024 * 1024,
            Some(&format!(
                "I was trying to download the source code for {name} {version}"
            )),
        )?;
        // Http.getArchive decodes the ZIP before invoking the hash comparison.
        // Keep malformed content ahead of hash mismatch when both are wrong.
        zip::ZipArchive::new(std::io::Cursor::new(&bytes)).map_err(|_| {
            crate::dependency_error::bad_download_content(name, version, url, false)
        })?;
        let actual = format!("{:x}", Sha1::digest(&bytes));
        if actual != hash {
            return Err(crate::dependency_error::bad_archive_hash(
                name, version, url, hash, &actual,
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
        download.succeeded();
        Ok(destination)
    }
    fn get_bytes(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        self.get_bytes_with_context(url, limit, None)
    }
    fn get_bytes_with_context(
        &self,
        url: &str,
        limit: u64,
        context: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let parsed = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
        if !matches!(parsed.scheme(), "https" | "http") {
            return Err("unsupported package URL scheme".into());
        }
        let response = self
            .client
            .get(parsed)
            .send()
            .map_err(|error| match context {
                Some(context) => crate::dependency_error::download_transport(
                    context,
                    url,
                    &transport_details(&error),
                ),
                None => format!("{url}: {error}"),
            })?;
        let status = response.status();
        if (status.is_client_error() || status.is_server_error())
            && let Some(context) = context
        {
            let reason = response
                .extensions()
                .get::<hyper::ext::ReasonPhrase>()
                .map(|reason| String::from_utf8_lossy(reason.as_bytes()).into_owned())
                .unwrap_or_else(|| status.canonical_reason().unwrap_or("").to_owned());
            return Err(crate::dependency_error::download_status(
                context,
                url,
                status.as_u16(),
                &reason,
            ));
        }
        let response = response
            .error_for_status()
            .map_err(|e| format!("{url}: {e}"))?;
        let gzip = response
            .extensions()
            .get::<tower_http::decompression::GzipDecoded>()
            .is_some();
        let expected_length = response.content_length();
        let chunked = response
            .headers()
            .get_all(reqwest::header::TRANSFER_ENCODING)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .last()
            .is_some_and(|coding| coding.trim().eq_ignore_ascii_case("chunked"));
        let mut bytes = Vec::new();
        response
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .or_else(|error| {
                if gzip && gzip_decoder_eof(&error) {
                    Ok(bytes.len())
                } else {
                    Err(error)
                }
            })
            .map_err(|error| match context {
                Some(context) => crate::dependency_error::download_transport(
                    context,
                    url,
                    &body_transport_details(&error, expected_length, bytes.len(), chunked, gzip),
                ),
                None => error.to_string(),
            })?;
        if bytes.len() as u64 > limit {
            return Err(format!("{url}: response size limit exceeded"));
        }
        Ok(bytes)
    }
    fn post_registry(&self, route: &str) -> Result<Vec<u8>, String> {
        let url = format!("{}{route}", self.server);
        let response = self
            .client
            .post(&url)
            .header(reqwest::header::CONTENT_LENGTH, 0)
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("{url}: {e}"))?;
        let gzip = response
            .extensions()
            .get::<tower_http::decompression::GzipDecoded>()
            .is_some();
        let mut bytes = Vec::new();
        response
            .take(MAX_REGISTRY_BYTES + 1)
            .read_to_end(&mut bytes)
            .or_else(|error| {
                if gzip && gzip_decoder_eof(&error) {
                    Ok(bytes.len())
                } else {
                    Err(error)
                }
            })
            .map_err(|e| format!("{url}: {e}"))?;
        if bytes.len() as u64 > MAX_REGISTRY_BYTES {
            return Err(format!("{url}: registry response exceeds 64 MiB"));
        }
        Ok(bytes)
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

// http-client's streaming inflater accepts EOF before the gzip footer. Only
// swallow the compression decoder's EOF, never HTTP framing or TLS truncation.
fn gzip_decoder_eof(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut cause = Some(error);
    while let Some(error) = cause {
        if let Some(io) = error.downcast_ref::<std::io::Error>()
            && io.kind() == std::io::ErrorKind::UnexpectedEof
            && io.to_string() == "unexpected end of file"
        {
            return true;
        }
        cause = error.source();
    }
    false
}

/// Reproduce http-client's counted short-body error only for hyper's explicit
/// HTTP framing EOF. TLS truncation, timeouts and other I/O failures retain
/// their own causes even if a Content-Length was present.
fn body_transport_details(
    error: &std::io::Error,
    expected: Option<u64>,
    received: usize,
    chunked: bool,
    gzip: bool,
) -> String {
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(error) = cause {
        if gzip {
            let deflate = error.downcast_ref::<flate2::DecompressError>().or_else(|| {
                error
                    .downcast_ref::<std::io::Error>()
                    .and_then(std::io::Error::get_ref)
                    .and_then(|inner| inner.downcast_ref::<flate2::DecompressError>())
            });
            if deflate.is_some_and(|error| error.needs_dictionary().is_none()) {
                return "HttpZlibException (ZlibException (-3))".to_owned();
            }
        }
        // hyper 1.9's framing error types are private. Check both the I/O
        // kind and exact message, with differential transport regressions.
        if let Some(io) = error.downcast_ref::<std::io::Error>() {
            let message = io.to_string();
            if gzip
                && io.kind() == std::io::ErrorKind::InvalidData
                && matches!(
                    message.as_str(),
                    "Invalid gzip header"
                        | "CRC computed does not match"
                        | "CRC computed for header does not match"
                        | "amount of bytes read does not match"
                )
            {
                return "HttpZlibException (ZlibException (-3))".to_owned();
            }
            if io.kind() == std::io::ErrorKind::UnexpectedEof {
                if message == "end of file before message length reached" {
                    if chunked {
                        return "InvalidChunkHeaders".to_owned();
                    }
                    if let Some(expected) =
                        expected.filter(|expected| (received as u64) < *expected)
                    {
                        return format!("ResponseBodyTooShort {expected} {received}");
                    }
                } else if chunked && message == "unexpected EOF during chunk size line" {
                    return "IncompleteHeaders".to_owned();
                }
            } else if chunked
                && io.kind() == std::io::ErrorKind::InvalidInput
                && message == "Invalid chunk size line: missing size digit"
            {
                return "InvalidChunkHeaders".to_owned();
            }
        }
        cause = error.source();
    }
    transport_details(error)
}

/// Include nested causes: reqwest's top-level display only says that sending
/// failed, while the actionable connection/TLS/proxy cause is in source().
fn transport_details(error: &(dyn std::error::Error + 'static)) -> String {
    let mut cause = Some(error);
    while let Some(error) = cause {
        #[cfg(target_os = "linux")]
        if let Some(address) = error.downcast_ref::<hyper_util::client::legacy::connect::ElmBracketedHostError>() {
            return format!("ConnectionFailure Network.Socket.getAddrInfo (called with preferred socket type/protocol: AddrInfo {{addrFlags = [AI_ADDRCONFIG], addrFamily = AF_UNSPEC, addrSocketType = Stream, addrProtocol = 0, addrAddress = <assumed to be undefined>, addrCanonName = <assumed to be undefined>}}, host name: Just {}, service name: Just {}): does not exist (Try again)", haskell_text(&address.host), haskell_text(&address.port.to_string()));
        }
        #[cfg(target_os = "linux")]
        if let Some(connect) =
            error.downcast_ref::<hyper_util::client::legacy::connect::TcpConnectError>()
            && connect.cause.kind() == std::io::ErrorKind::ConnectionRefused
        {
            return format!(
                "ConnectionFailure Network.Socket.connect: <socket: {}>: does not exist (Connection refused)",
                connect.socket
            );
        }

        let tls = error.downcast_ref::<rustls::Error>().or_else(|| {
            error
                .downcast_ref::<std::io::Error>()
                .and_then(std::io::Error::get_ref)
                .and_then(|inner| inner.downcast_ref::<rustls::Error>())
        });
        if let Some(rustls::Error::InvalidCertificate(certificate)) = tls {
            let report = match certificate {
                rustls::CertificateError::Expired
                | rustls::CertificateError::ExpiredContext { .. }
                | rustls::CertificateError::NotValidYet
                | rustls::CertificateError::NotValidYetContext { .. } => {
                    // Elm's TLS library reports both validity-window failures
                    // with this same expired-certificate message and alert.
                    Some(("certificate has expired".to_owned(), "CertificateExpired"))
                }
                rustls::CertificateError::BadSignature => Some((
                    "certificate rejected: [InvalidSignature SignatureInvalid]".to_owned(),
                    "CertificateUnknown",
                )),
                rustls::CertificateError::UnknownIssuer => {
                    Some(("certificate has unknown CA".to_owned(), "UnknownCa"))
                }
                rustls::CertificateError::NotValidForNameContext { expected, .. } => Some((
                    format!(
                        "certificate rejected: [NameMismatch {}]",
                        haskell_bytes(expected.to_str().as_bytes())
                    ),
                    "CertificateUnknown",
                )),
                _ => None,
            };
            if let Some((reason, alert)) = report {
                return format!(
                    "InternalException (HandshakeFailed (Error_Protocol ({},True,{alert})))",
                    haskell_bytes(reason.as_bytes())
                );
            }
        }

        if let Some(hyper_util::client::legacy::connect::proxy::TunnelError::ProxyRejected {
            host,
            port,
            status,
            reason,
        }) = error.downcast_ref::<hyper_util::client::legacy::connect::proxy::TunnelError>()
        {
            return format!(
                "ProxyConnectException {} {} (Status {{statusCode = {}, statusMessage = {}}})",
                haskell_bytes(host.as_bytes()),
                port,
                status,
                haskell_bytes(reason),
            );
        }
        if let Some(tunnel) =
            error.downcast_ref::<hyper_util::client::legacy::connect::proxy::TunnelError>()
        {
            use hyper_util::client::legacy::connect::proxy::TunnelError;
            match tunnel {
                TunnelError::InvalidStatusLine(line) => {
                    return format!("InvalidStatusLine {}", haskell_bytes(line));
                }
                TunnelError::ProxyHeadersTooLong => return "OverlongHeaders".to_owned(),
                TunnelError::TunnelNoResponse => return "NoResponseDataReceived".to_owned(),
                TunnelError::TunnelUnexpectedEof => return "IncompleteHeaders".to_owned(),
                _ => {}
            }
        }
        cause = error.source();
    }
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(error) = source {
        text.push_str(": ");
        text.push_str(&error.to_string());
        source = error.source();
    }
    text
}

/// Haskell Text/String literals escape Unicode scalar values, not UTF-8 bytes.
pub(crate) fn haskell_text(value: &str) -> String {
    haskell_quoted(value.chars().map(u32::from))
}

/// Haskell ByteString literals escape each byte independently.
fn haskell_bytes(bytes: &[u8]) -> String {
    haskell_quoted(bytes.iter().copied().map(u32::from))
}

fn haskell_quoted(codes: impl Iterator<Item = u32>) -> String {
    const CONTROL: [&str; 32] = [
        "NUL", "SOH", "STX", "ETX", "EOT", "ENQ", "ACK", "a", "b", "t", "n", "v", "f", "r", "SO",
        "SI", "DLE", "DC1", "DC2", "DC3", "DC4", "NAK", "SYN", "ETB", "CAN", "EM", "SUB", "ESC",
        "FS", "GS", "RS", "US",
    ];
    let mut text = String::from("\"");
    let mut codes = codes.peekable();
    while let Some(code) = codes.next() {
        match code {
            34 => text.push_str("\\\""),
            92 => text.push_str("\\\\"),
            0..=31 => {
                text.push('\\');
                text.push_str(CONTROL[code as usize]);
                if code == 14 && codes.peek() == Some(&72) {
                    text.push_str("\\&");
                }
            }
            32..=126 => text.push(char::from(code as u8)),
            127 => text.push_str("\\DEL"),
            _ => {
                text.push('\\');
                text.push_str(&code.to_string());
                if codes.peek().is_some_and(|next| (48..=57).contains(next)) {
                    text.push_str("\\&");
                }
            }
        }
    }
    text.push('"');
    text
}

#[cfg(test)]
mod proxy_tests {
    use super::{body_transport_details, gzip_decoder_eof, haskell_bytes, transport_details};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    #[test]
    fn rejected_connect_preserves_status_reason_and_destination() {
        for (status, reason, fragmented) in [
            (502, "Bad Gateway", false),
            (407, "Proxy Authentication Required", true),
            (403, "Custom \"quoted\" rejection", true),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let proxy = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.extend(byte);
                }
                assert!(request.starts_with(b"CONNECT example.invalid:8443 HTTP/1.1\r\n"));
                let response = format!("HTTP/1.1 {status} {reason}\r\n\r\n");
                if fragmented {
                    // Deliver an incomplete protocol/status line before the rest.
                    stream.write_all(&response.as_bytes()[..9]).unwrap();
                    thread::sleep(Duration::from_millis(20));
                    stream.write_all(&response.as_bytes()[9..]).unwrap();
                } else {
                    stream.write_all(response.as_bytes()).unwrap();
                }
            });
            let client = reqwest::blocking::Client::builder()
                .proxy(reqwest::Proxy::all(format!("http://{address}")).unwrap())
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap();
            let error = client
                .get("https://example.invalid:8443/archive.zip")
                .send()
                .unwrap_err();
            proxy.join().unwrap();
            assert_eq!(
                transport_details(&error),
                format!(
                    "ProxyConnectException \"example.invalid\" 8443 (Status {{statusCode = {status}, statusMessage = {}}})",
                    haskell_bytes(reason.as_bytes())
                )
            );
        }
    }

    #[test]
    fn disconnected_proxy_distinguishes_empty_and_incomplete_response() {
        for (response, expected) in [
            ("", "NoResponseDataReceived"),
            ("HTTP/1.1 20", "IncompleteHeaders"),
            ("HTTP/1.1 200 OK\r\n", "IncompleteHeaders"),
            ("HTTP/1.1 502 Bad Gateway\r\n", "IncompleteHeaders"),
            ("HTTP/1.1 100 Continue\r\n\r\n", "NoResponseDataReceived"),
            (
                "HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 50",
                "IncompleteHeaders",
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let proxy = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.extend(byte);
                }
                stream.write_all(response.as_bytes()).unwrap();
            });
            let client = reqwest::blocking::Client::builder()
                .proxy(reqwest::Proxy::all(format!("http://{address}")).unwrap())
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap();
            let error = client
                .get("https://example.invalid/archive.zip")
                .send()
                .unwrap_err();
            proxy.join().unwrap();
            assert_eq!(transport_details(&error), expected);
        }
    }

    #[test]
    fn proxy_status_line_diagnostics_match_http_client() {
        for (response, expected) in [
            (
                "HTTP/1.1 502 Bad Gateway\n\n",
                "ProxyConnectException \"example.invalid\" 443 (Status {statusCode = 502, statusMessage = \"Bad Gateway\"})",
            ),
            (
                "\r\n\r\n\r\nHTTP/1.1 502 Bad Gateway\r\n\r\n",
                "ProxyConnectException \"example.invalid\" 443 (Status {statusCode = 502, statusMessage = \"Bad Gateway\"})",
            ),
            (
                "\r\n\r\n\r\n\r\nHTTP/1.1 502 Bad Gateway\r\n\r\n",
                "InvalidStatusLine \"\"",
            ),
            ("NOT HTTP\r\n\r\n", "InvalidStatusLine \"NOT HTTP\""),
            (
                "HTTP/1.1 xyz Broken\r\n\r\n",
                "InvalidStatusLine \"HTTP/1.1 xyz Broken\"",
            ),
            (
                "HTTP/7.9 502 Bad Gateway\r\n\r\n",
                "ProxyConnectException \"example.invalid\" 443 (Status {statusCode = 502, statusMessage = \"Bad Gateway\"})",
            ),
            (
                "HTTP/1.1   502   Bad Gateway\r\n\r\n",
                "ProxyConnectException \"example.invalid\" 443 (Status {statusCode = 502, statusMessage = \"Bad Gateway\"})",
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let proxy = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.extend(byte);
                }
                stream.write_all(response.as_bytes()).unwrap();
            });
            let client = reqwest::blocking::Client::builder()
                .proxy(reqwest::Proxy::all(format!("http://{address}")).unwrap())
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap();
            let error = client
                .get("https://example.invalid/archive.zip")
                .send()
                .unwrap_err();
            proxy.join().unwrap();
            assert_eq!(transport_details(&error), expected);
        }
    }

    #[test]
    fn proxy_header_count_and_streamed_size_match_http_client() {
        for (headers, expected) in [
            ("X: value\r\n".repeat(100), "OverlongHeaders"),
            (
                ("X: ".to_owned() + &"v".repeat(200) + "\r\n").repeat(80),
                "ProxyConnectException \"example.invalid\" 443 (Status {statusCode = 502, statusMessage = \"Bad Gateway\"})",
            ),
            (
                "ignored line\r\n".repeat(110),
                "ProxyConnectException \"example.invalid\" 443 (Status {statusCode = 502, statusMessage = \"Bad Gateway\"})",
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let proxy = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.extend(byte);
                }
                stream
                    .write_all(format!("HTTP/1.1 502 Bad Gateway\r\n{headers}\r\n").as_bytes())
                    .unwrap();
            });
            let client = reqwest::blocking::Client::builder()
                .proxy(reqwest::Proxy::all(format!("http://{address}")).unwrap())
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap();
            let error = client
                .get("https://example.invalid/archive.zip")
                .send()
                .unwrap_err();
            proxy.join().unwrap();
            assert_eq!(transport_details(&error), expected);
        }
    }

    #[test]
    fn tls_truncation_is_not_relabelled_as_http_body_length_failure() {
        let error = std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "peer closed connection without sending TLS close_notify",
        );
        assert_eq!(
            body_transport_details(&error, Some(94), 47, false, false),
            transport_details(&error)
        );
    }

    #[test]
    fn gzip_eof_tolerance_does_not_swallow_http_or_tls_truncation() {
        for message in [
            "end of file before message length reached",
            "unexpected EOF during chunk size line",
            "peer closed connection without sending TLS close_notify",
        ] {
            let error = std::io::Error::new(std::io::ErrorKind::UnexpectedEof, message);
            assert!(!gzip_decoder_eof(&error));
        }
    }

    #[test]
    fn gzip_header_checksum_failure_uses_zlib_diagnostic_only_for_gzip() {
        let error = std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "CRC computed for header does not match",
        );
        assert_eq!(
            body_transport_details(&error, None, 0, false, true),
            "HttpZlibException (ZlibException (-3))"
        );
        assert_eq!(
            body_transport_details(&error, None, 0, false, false),
            transport_details(&error)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn refused_connection_diagnostic_uses_the_actual_socket() {
        let error = hyper_util::client::legacy::connect::TcpConnectError {
            socket: 37,
            cause: std::io::Error::from_raw_os_error(111),
        };
        assert_eq!(
            transport_details(&error),
            "ConnectionFailure Network.Socket.connect: <socket: 37>: does not exist (Connection refused)"
        );
    }

    #[test]
    fn typed_certificate_errors_use_official_diagnostics() {
        let expired = r#"InternalException (HandshakeFailed (Error_Protocol ("certificate has expired",True,CertificateExpired)))"#;
        let time = rustls::pki_types::UnixTime::since_unix_epoch(Duration::from_secs(100));
        for (certificate, expected) in [
            (
                rustls::CertificateError::BadSignature,
                r#"InternalException (HandshakeFailed (Error_Protocol ("certificate rejected: [InvalidSignature SignatureInvalid]",True,CertificateUnknown)))"#,
            ),
            (rustls::CertificateError::Expired, expired),
            (rustls::CertificateError::NotValidYet, expired),
            (
                rustls::CertificateError::ExpiredContext {
                    time,
                    not_after: time,
                },
                expired,
            ),
            (
                rustls::CertificateError::NotValidYetContext {
                    time,
                    not_before: time,
                },
                expired,
            ),
            (
                rustls::CertificateError::UnknownIssuer,
                r#"InternalException (HandshakeFailed (Error_Protocol ("certificate has unknown CA",True,UnknownCa)))"#,
            ),
            (
                rustls::CertificateError::NotValidForNameContext {
                    expected: rustls::pki_types::ServerName::try_from("archive.test").unwrap(),
                    presented: vec!["package.elm-lang.org".to_owned()],
                },
                r#"InternalException (HandshakeFailed (Error_Protocol ("certificate rejected: [NameMismatch \"archive.test\"]",True,CertificateUnknown)))"#,
            ),
        ] {
            let error = std::io::Error::other(rustls::Error::InvalidCertificate(certificate));
            assert_eq!(transport_details(&error), expected);
        }
    }

    #[test]
    fn text_escapes_use_unicode_code_points_instead_of_utf8_bytes() {
        assert_eq!(super::haskell_text("é漢😀1"), r#""\233\28450\128512\&1""#);
        assert_eq!(super::haskell_text("\x0eH\n\"\\"), r#""\SO\&H\n\"\\""#);
    }

    #[test]
    fn byte_string_escapes_are_unambiguous() {
        assert_eq!(
            haskell_bytes(b"a\"\\\n\x0eH\xff1\x7f"),
            r#""a\"\\\n\SO\&H\255\&1\DEL""#
        );
    }
}
