//! Elm 0.19.1 publication protocol. Endpoints are explicit for isolated tests;
//! production callers use `official`, never environment-provided URLs.
use crate::package_solver::{Version, valid_name};
use reqwest::blocking::{Client, Response, multipart};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::{io::Read, path::Path, time::Duration};
#[derive(Debug)]
pub enum Error {
    Http {
        url: String,
        status: Option<u16>,
        detail: String,
    },
    TagData {
        url: String,
        body: Vec<u8>,
    },
    Archive {
        url: String,
        detail: String,
    },
    Io(String),
}
pub struct PublicationNetwork {
    client: Client,
    api: String,
    github: String,
    registry: String,
}
pub struct Archive {
    pub sha1: String,
    url: String,
    bytes: Vec<u8>,
}
impl Archive {
    /// Extract into a fresh directory owned by the current publication attempt.
    pub fn extract(&self, destination: &Path) -> Result<(), Error> {
        crate::package_archive::extract(&self.bytes, destination).map_err(|detail| Error::Archive {
            url: self.url.clone(),
            detail,
        })
    }
}
impl PublicationNetwork {
    pub fn official() -> Result<Self, String> {
        Self::with_endpoints(
            "https://api.github.com",
            "https://github.com",
            crate::package_network::PACKAGE_SERVER,
        )
    }
    pub fn with_endpoints(api: &str, github: &str, registry: &str) -> Result<Self, String> {
        fn base(value: &str) -> Result<String, String> {
            let url = reqwest::Url::parse(value).map_err(|e| e.to_string())?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err("invalid publication endpoint".into());
            }
            Ok(value.trim_end_matches('/').to_owned())
        }
        crate::proxy_environment::validate()?;
        let client = Client::builder()
            .user_agent("elm/0.19.1")
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            api: base(api)?,
            github: base(github)?,
            registry: base(registry)?,
        })
    }
    pub fn tag(&self, name: &str, version: Version) -> Result<String, Error> {
        package_name(name)?;
        let url = format!("{}/repos/{name}/git/refs/tags/{version}", self.api);
        let response = self
            .client
            .get(&url)
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(Duration::from_secs(30))
            .send()
            .map_err(|e| http_error(&url, e))?;
        let bytes = read_response(&url, response, 1024 * 1024)?;
        serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|v| v["object"]["sha"].as_str().map(str::to_owned))
            .ok_or(Error::TagData { url, body: bytes })
    }
    pub fn archive(&self, name: &str, version: Version) -> Result<Archive, Error> {
        package_name(name)?;
        let url = format!("{}/{name}/zipball/{version}/", self.github);
        let response = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(30))
            .send()
            .map_err(|e| http_error(&url, e))?;
        let bytes = read_response(&url, response, 64 * 1024 * 1024)?;
        let sha1 = format!("{:x}", Sha1::digest(&bytes));
        Ok(Archive { sha1, url, bytes })
    }
    /// Register only after local and downloaded builds, version and Git checks.
    /// This is the sole mutating network operation in the publication pipeline.
    pub fn register(
        &self,
        root: &Path,
        name: &str,
        version: Version,
        commit: &str,
        docs: &Value,
        archive: &Archive,
    ) -> Result<(), Error> {
        package_name(name)?;
        let mut url = reqwest::Url::parse(&format!("{}/register", self.registry))
            .map_err(|e| Error::Io(e.to_string()))?;
        url.query_pairs_mut()
            .append_pair("name", name)
            .append_pair("version", &version.to_string())
            .append_pair("commit-hash", commit);
        let url = url.to_string();
        let form = multipart::Form::new()
            .file("elm.json", root.join("elm.json"))
            .map_err(|e| Error::Io(e.to_string()))?
            .part(
                "docs.json",
                multipart::Part::bytes(
                    serde_json::to_vec(docs).map_err(|e| Error::Io(e.to_string()))?,
                )
                .file_name("docs.json")
                .mime_str("application/json")
                .map_err(|e| Error::Io(e.to_string()))?,
            )
            .file("README.md", root.join("README.md"))
            .map_err(|e| Error::Io(e.to_string()))?
            .text("github-hash", archive.sha1.clone());
        // The official upload intentionally has no overall response timeout.
        self.client
            .post(&url)
            .multipart(form)
            .send()
            .and_then(Response::error_for_status)
            .map_err(|e| http_error(&url, e))?;
        Ok(())
    }
}
fn package_name(name: &str) -> Result<(), Error> {
    if valid_name(name) {
        Ok(())
    } else {
        Err(Error::Io(format!("invalid package name {name}")))
    }
}
fn http_error(url: &str, error: reqwest::Error) -> Error {
    Error::Http {
        url: url.into(),
        status: error.status().map(|s| s.as_u16()),
        detail: error.to_string(),
    }
}
fn read_response(url: &str, response: Response, limit: u64) -> Result<Vec<u8>, Error> {
    let response = response
        .error_for_status()
        .map_err(|e| http_error(url, e))?;
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::Http {
            url: url.into(),
            status: None,
            detail: e.to_string(),
        })?;
    if bytes.len() as u64 > limit {
        return Err(Error::Http {
            url: url.into(),
            status: None,
            detail: "response size limit exceeded".into(),
        });
    }
    Ok(bytes)
}
