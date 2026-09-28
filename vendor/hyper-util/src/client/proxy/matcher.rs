//! Proxy matchers
//!
//! This module contains different matchers to configure rules for when a proxy
//! should be used, and if so, with what arguments.
//!
//! A [`Matcher`] can be constructed either using environment variables, or
//! a [`Matcher::builder()`].
//!
//! Once constructed, the `Matcher` can be asked if it intercepts a `Uri` by
//! calling [`Matcher::intercept()`].
//!
//! An [`Intercept`] includes the destination for the proxy, and any parsed
//! authentication to be used.

use std::fmt;
use std::net::IpAddr;

use http::header::HeaderValue;
use ipnet::IpNet;
use percent_encoding::percent_decode_str;

#[cfg(docsrs)]
pub use self::builder::IntoValue;
#[cfg(not(docsrs))]
use self::builder::IntoValue;

/// A proxy matcher, usually built from environment variables.
pub struct Matcher {
    http: Option<Intercept>,
    https: Option<Intercept>,
    no: NoProxy,
}

/// A matched proxy,
///
/// This is returned by a matcher if a proxy should be used.
#[derive(Clone)]
pub struct Intercept {
    uri: http::Uri,
    auth: Auth,
}

/// A builder to create a [`Matcher`].
///
/// Construct with [`Matcher::builder()`].
#[derive(Default)]
pub struct Builder {
    #[cfg(feature = "elm-proxy-environment")]
    elm_environment: bool,
    is_cgi: bool,
    all: String,
    http: String,
    https: String,
    no: String,
}

#[derive(Clone)]
enum Auth {
    Empty,
    Basic(http::header::HeaderValue),
    Raw(String, String),
}

/// A filter for proxy matchers.
///
/// This type is based off the `NO_PROXY` rules used by curl.
#[derive(Clone, Debug, Default)]
struct NoProxy {
    ips: IpMatcher,
    domains: DomainMatcher,
    elm_domains: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default)]
struct DomainMatcher(Vec<String>);

#[derive(Clone, Debug, Default)]
struct IpMatcher(Vec<Ip>);

#[derive(Clone, Debug)]
enum Ip {
    Address(IpAddr),
    Network(IpNet),
}

#[cfg(feature = "elm-no-proxy")]
/// Look up a lowercase proxy variable name in an environment snapshot.
/// An exact key wins; otherwise the last ASCII case-insensitive key is used.
pub fn elm_proxy_environment_value<'a>(
    environment: &'a [(String, String)],
    name: &str,
) -> Option<&'a str> {
    environment
        .iter()
        .find(|(key, _)| key == name)
        .or_else(|| {
            environment
                .iter()
                .rev()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
        })
        .map(|(_, value)| value.as_str())
}

// ===== impl Matcher =====

impl Matcher {
    fn environment_no_proxy(matcher: Self) -> Self {
        #[cfg(feature = "elm-no-proxy")]
        {
            let mut matcher = matcher;
            let environment = std::env::vars_os()
                .filter_map(|(key, value)| {
                    let key = key.into_string().ok()?;
                    if !key.eq_ignore_ascii_case("no_proxy") {
                        return None;
                    }
                    Some((key, value.into_string().ok()?))
                })
                .collect::<Vec<_>>();
            if let Some(value) = elm_proxy_environment_value(&environment, "no_proxy") {
                matcher.no = NoProxy::from_elm_string(value);
            }
            matcher
        }
        #[cfg(not(feature = "elm-no-proxy"))]
        matcher
    }

    /// Create a matcher reading the current environment variables.
    ///
    /// This checks for values in the following variables, treating them the
    /// same as curl does:
    ///
    /// - `ALL_PROXY`/`all_proxy`
    /// - `HTTPS_PROXY`/`https_proxy`
    /// - `HTTP_PROXY`/`http_proxy`
    /// - `NO_PROXY`/`no_proxy`
    pub fn from_env() -> Self {
        Self::environment_no_proxy(Builder::from_env().build())
    }

    /// Create a matcher from the environment or system.
    ///
    /// This checks the same environment variables as `from_env()`, and if not
    /// set, checks the system configuration for values for the OS.
    ///
    /// This constructor is always available, but if the `client-proxy-system`
    /// feature is enabled, it will check more configuration. Use this
    /// constructor if you want to allow users to optionally enable more, or
    /// use `from_env` if you do not want the values to change based on an
    /// enabled feature.
    pub fn from_system() -> Self {
        Self::environment_no_proxy(Builder::from_system().build())
    }

    /// Start a builder to configure a matcher.
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Check if the destination should be intercepted by a proxy.
    ///
    /// If the proxy rules match the destination, a new `Uri` will be returned
    /// to connect to.
    pub fn intercept(&self, dst: &http::Uri) -> Option<Intercept> {
        // TODO(perf): don't need to check `no` if below doesn't match...
        if self.no.contains(dst.host()?) {
            return None;
        }

        match dst.scheme_str() {
            Some("http") => self.http.clone(),
            Some("https") => self.https.clone(),
            _ => None,
        }
    }
}

impl fmt::Debug for Matcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut b = f.debug_struct("Matcher");

        if let Some(ref http) = self.http {
            b.field("http", http);
        }

        if let Some(ref https) = self.https {
            b.field("https", https);
        }

        if !self.no.is_empty() {
            b.field("no", &self.no);
        }
        b.finish()
    }
}

// ===== impl Intercept =====

impl Intercept {
    /// Get the `http::Uri` for the target proxy.
    pub fn uri(&self) -> &http::Uri {
        &self.uri
    }

    /// Get any configured basic authorization.
    ///
    /// This should usually be used with a `Proxy-Authorization` header, to
    /// send in Basic format.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use hyper_util::client::proxy::matcher::Matcher;
    /// # let uri = http::Uri::from_static("https://hyper.rs");
    /// let m = Matcher::builder()
    ///     .all("https://Aladdin:opensesame@localhost:8887")
    ///     .build();
    ///
    /// let proxy = m.intercept(&uri).expect("example");
    /// let auth = proxy.basic_auth().expect("example");
    /// assert_eq!(auth, "Basic QWxhZGRpbjpvcGVuc2VzYW1l");
    /// ```
    pub fn basic_auth(&self) -> Option<&HeaderValue> {
        if let Auth::Basic(ref val) = self.auth {
            Some(val)
        } else {
            None
        }
    }

    /// Get any configured raw authorization.
    ///
    /// If not detected as another scheme, this is the username and password
    /// that should be sent with whatever protocol the proxy handshake uses.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use hyper_util::client::proxy::matcher::Matcher;
    /// # let uri = http::Uri::from_static("https://hyper.rs");
    /// let m = Matcher::builder()
    ///     .all("socks5h://Aladdin:opensesame@localhost:8887")
    ///     .build();
    ///
    /// let proxy = m.intercept(&uri).expect("example");
    /// let auth = proxy.raw_auth().expect("example");
    /// assert_eq!(auth, ("Aladdin", "opensesame"));
    /// ```
    pub fn raw_auth(&self) -> Option<(&str, &str)> {
        if let Auth::Raw(ref u, ref p) = self.auth {
            Some((u.as_str(), p.as_str()))
        } else {
            None
        }
    }
}

impl fmt::Debug for Intercept {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Intercept")
            .field("uri", &self.uri)
            // dont output auth, its sensitive
            .finish()
    }
}

// ===== impl Builder =====

impl Builder {
    #[cfg(feature = "elm-proxy-environment")]
    fn from_elm_environment(environment: &[(String, String)], is_cgi: bool) -> Self {
        let value = |name| {
            elm_proxy_environment_value(environment, name)
                .unwrap_or_default()
                .to_owned()
        };
        Self {
            elm_environment: true,
            is_cgi,
            all: String::new(),
            http: value("http_proxy"),
            https: value("https_proxy"),
            no: value("no_proxy"),
        }
    }

    fn from_env() -> Self {
        #[cfg(feature = "elm-proxy-environment")]
        {
            let environment = std::env::vars_os()
                .filter_map(|(key, value)| {
                    let key = key.into_string().ok()?;
                    if !["http_proxy", "https_proxy", "no_proxy"]
                        .iter()
                        .any(|name| key.eq_ignore_ascii_case(name))
                    {
                        return None;
                    }
                    Some((key, value.into_string().ok()?))
                })
                .collect::<Vec<_>>();
            return Self::from_elm_environment(
                &environment,
                std::env::var_os("REQUEST_METHOD").is_some(),
            );
        }
        #[cfg(not(feature = "elm-proxy-environment"))]
        Builder {
            is_cgi: std::env::var_os("REQUEST_METHOD").is_some(),
            all: get_first_env(&["ALL_PROXY", "all_proxy"]),
            http: get_first_env(&["HTTP_PROXY", "http_proxy"]),
            https: get_first_env(&["HTTPS_PROXY", "https_proxy"]),
            no: get_first_env(&["NO_PROXY", "no_proxy"]),
        }
    }

    fn from_system() -> Self {
        #[allow(unused_mut)]
        let mut builder = Self::from_env();

        #[cfg(all(feature = "client-proxy-system", target_os = "macos"))]
        mac::with_system(&mut builder);

        #[cfg(all(feature = "client-proxy-system", windows))]
        win::with_system(&mut builder);

        builder
    }

    /// Set the target proxy for all destinations.
    pub fn all<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.all = val.into_value();
        self
    }

    /// Set the target proxy for HTTP destinations.
    pub fn http<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.http = val.into_value();
        self
    }

    /// Set the target proxy for HTTPS destinations.
    pub fn https<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.https = val.into_value();
        self
    }

    /// Set the "no" proxy filter.
    ///
    /// The rules are as follows:
    /// * Entries are expected to be comma-separated (whitespace between entries is ignored)
    /// * IP addresses (both IPv4 and IPv6) are allowed, as are optional subnet masks (by adding /size,
    ///   for example "`192.168.1.0/24`").
    /// * An entry "`*`" matches all hostnames (this is the only wildcard allowed)
    /// * Any other entry is considered a domain name (and may contain a leading dot, for example `google.com`
    ///   and `.google.com` are equivalent) and would match both that domain AND all subdomains.
    ///
    /// For example, if `"NO_PROXY=google.com, 192.168.1.0/24"` was set, all of the following would match
    /// (and therefore would bypass the proxy):
    /// * `http://google.com/`
    /// * `http://www.google.com/`
    /// * `http://192.168.1.42/`
    ///
    /// The URL `http://notgoogle.com/` would not match.
    pub fn no<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.no = val.into_value();
        self
    }

    /// Construct a [`Matcher`] using the configured values.
    pub fn build(self) -> Matcher {
        if self.is_cgi {
            return Matcher {
                http: None,
                https: None,
                no: NoProxy::empty(),
            };
        }

        let parse = |value: &str| {
            let proxy = parse_env_uri(value);
            #[cfg(feature = "elm-proxy-environment")]
            let proxy = proxy.map(|mut proxy| {
                if self.elm_environment {
                    proxy.auth = elm_proxy_auth(value);
                }
                proxy
            });
            proxy
        };
        let all = parse(&self.all);

        Matcher {
            http: parse(&self.http).or_else(|| all.clone()),
            https: parse(&self.https).or(all),
            no: NoProxy::from_string(&self.no),
        }
    }
}

#[cfg(not(feature = "elm-proxy-environment"))]
fn get_first_env(names: &[&str]) -> String {
    for name in names {
        if let Ok(val) = std::env::var(name) {
            return val;
        }
    }

    String::new()
}

fn parse_env_uri(val: &str) -> Option<Intercept> {
    use std::borrow::Cow;

    let uri = val.parse::<http::Uri>().ok()?;
    let mut builder = http::Uri::builder();
    let mut is_httpish = false;
    let mut auth = Auth::Empty;

    builder = builder.scheme(match uri.scheme() {
        Some(s) => {
            if s == &http::uri::Scheme::HTTP || s == &http::uri::Scheme::HTTPS {
                is_httpish = true;
                s.clone()
            } else if matches!(s.as_str(), "socks4" | "socks4a" | "socks5" | "socks5h") {
                s.clone()
            } else {
                // can't use this proxy scheme
                return None;
            }
        }
        // if no scheme provided, assume they meant 'http'
        None => {
            is_httpish = true;
            http::uri::Scheme::HTTP
        }
    });

    let authority = uri.authority()?;

    if let Some((userinfo, host_port)) = authority.as_str().split_once('@') {
        let (user, pass) = match userinfo.split_once(':') {
            Some((user, pass)) => (user, Some(pass)),
            None => (userinfo, None),
        };
        let user = percent_decode_str(user).decode_utf8_lossy();
        let pass = pass.map(|pass| percent_decode_str(pass).decode_utf8_lossy());
        if is_httpish {
            auth = Auth::Basic(encode_basic_auth(&user, pass.as_deref()));
        } else {
            auth = Auth::Raw(
                user.into_owned(),
                pass.map_or_else(String::new, Cow::into_owned),
            );
        }
        builder = builder.authority(host_port);
    } else {
        builder = builder.authority(authority.clone());
    }

    // removing any path, but we MUST specify one or the builder errors
    builder = builder.path_and_query("/");

    let dst = builder.build().ok()?;

    Some(Intercept { uri: dst, auth })
}

#[cfg(feature = "elm-proxy-environment")]
fn elm_proxy_auth(value: &str) -> Auth {
    use base64::{Engine as _, prelude::BASE64_STANDARD};

    let Some(uri) = value.parse::<http::Uri>().ok() else {
        return Auth::Empty;
    };
    let Some(authority) = uri.authority() else {
        return Auth::Empty;
    };
    let Some((userinfo, _)) = authority.as_str().split_once('@') else {
        return Auth::Empty;
    };
    if !userinfo.contains(':') {
        return Auth::Empty;
    }
    // http-client extracts a literal colon before decoding, then uses
    // network-uri's UTF-8 unescaping followed by ByteString.Char8.pack.
    // Char8 preserves only the low byte of each Unicode scalar value.
    let bytes = elm_unescape_char8(userinfo);
    let mut header = HeaderValue::from_str(&format!("Basic {}", BASE64_STANDARD.encode(bytes)))
        .expect("base64 is always valid HeaderValue");
    header.set_sensitive(true);
    Auth::Basic(header)
}

// network-uri consumes a complete escaped sequence before validating its
// scalar value, including obsolete five/six-byte forms. Rust's lossy decoder
// instead replaces invalid lead/continuation bytes separately.
#[cfg(feature = "elm-proxy-environment")]
fn elm_unescape_char8(value: &str) -> Vec<u8> {
    fn escaped(value: &str, offset: usize) -> Option<u8> {
        let bytes = value.as_bytes().get(offset..offset + 3)?;
        if bytes[0] != b'%' {
            return None;
        }
        let digit = |byte: u8| (byte as char).to_digit(16);
        Some((digit(bytes[1])? * 16 + digit(bytes[2])?) as u8)
    }
    let mut output = Vec::with_capacity(value.len());
    let mut offset = 0;
    while offset < value.len() {
        let Some(first) = escaped(value, offset) else {
            let character = value[offset..].chars().next().unwrap();
            output.push(character as u32 as u8);
            offset += character.len_utf8();
            continue;
        };
        offset += 3;
        let (remaining, mask, minimum) = match first {
            0..=0x7f => {
                output.push(first);
                continue;
            }
            0xc0..=0xdf => (1, 0x1f, 0x80),
            0xe0..=0xef => (2, 0x0f, 0x800),
            0xf0..=0xf7 => (3, 0x07, 0x10000),
            0xf8..=0xfb => (4, 0x03, 0x200000),
            0xfc..=0xfd => (5, 0x01, 0x4000000),
            _ => {
                output.push(0xfd);
                continue;
            }
        };
        let mut scalar = u32::from(first & mask);
        let mut consumed = 0;
        while consumed < remaining {
            let Some(next) = escaped(value, offset).filter(|byte| byte & 0xc0 == 0x80) else {
                break;
            };
            scalar = (scalar << 6) | u32::from(next & 0x3f);
            offset += 3;
            consumed += 1;
        }
        let valid = consumed == remaining
            && scalar >= minimum
            && scalar <= 0x10ffff
            && !(0xd800..=0xdfff).contains(&scalar)
            && !(0xfffe..=0xffff).contains(&scalar);
        output.push(if valid { scalar as u8 } else { 0xfd });
    }
    output
}

fn encode_basic_auth(user: &str, pass: Option<&str>) -> HeaderValue {
    use base64::prelude::BASE64_STANDARD;
    use base64::write::EncoderWriter;
    use std::io::Write;

    let mut buf = b"Basic ".to_vec();
    {
        let mut encoder = EncoderWriter::new(&mut buf, &BASE64_STANDARD);
        let _ = write!(encoder, "{user}:");
        if let Some(password) = pass {
            let _ = write!(encoder, "{password}");
        }
    }
    let mut header = HeaderValue::from_bytes(&buf).expect("base64 is always valid HeaderValue");
    header.set_sensitive(true);
    header
}

impl NoProxy {
    #[cfg(feature = "elm-no-proxy")]
    fn from_elm_string(value: &str) -> Self {
        let suffixes = value
            .split(',')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let part = part.trim_start_matches(' ').to_ascii_lowercase();
                if part.starts_with('.') {
                    part
                } else {
                    format!(".{part}")
                }
            })
            .collect();
        Self {
            elm_domains: Some(suffixes),
            ..Self::empty()
        }
    }

    /*
    fn from_env() -> NoProxy {
        let raw = std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .unwrap_or_default();

        Self::from_string(&raw)
    }
    */

    fn empty() -> NoProxy {
        NoProxy {
            ips: IpMatcher(Vec::new()),
            domains: DomainMatcher(Vec::new()),
            elm_domains: None,
        }
    }

    /// Returns a new no-proxy configuration based on a `no_proxy` string (or `None` if no variables
    /// are set)
    /// The rules are as follows:
    /// * The environment variable `NO_PROXY` is checked, if it is not set, `no_proxy` is checked
    /// * If neither environment variable is set, `None` is returned
    /// * Entries are expected to be comma-separated (whitespace between entries is ignored)
    /// * IP addresses (both IPv4 and IPv6) are allowed, as are optional subnet masks (by adding /size,
    ///   for example "`192.168.1.0/24`").
    /// * An entry "`*`" matches all hostnames (this is the only wildcard allowed)
    /// * Any other entry is considered a domain name (and may contain a leading dot, for example `google.com`
    ///   and `.google.com` are equivalent) and would match both that domain AND all subdomains.
    ///
    /// For example, if `"NO_PROXY=google.com, 192.168.1.0/24"` was set, all of the following would match
    /// (and therefore would bypass the proxy):
    /// * `http://google.com/`
    /// * `http://www.google.com/`
    /// * `http://192.168.1.42/`
    ///
    /// The URL `http://notgoogle.com/` would not match.
    pub fn from_string(no_proxy_list: &str) -> Self {
        let mut ips = Vec::new();
        let mut domains = Vec::new();
        let parts = no_proxy_list.split(',').map(str::trim);
        for part in parts {
            match part.parse::<IpNet>() {
                // If we can parse an IP net or address, then use it, otherwise, assume it is a domain
                Ok(ip) => ips.push(Ip::Network(ip)),
                Err(_) => match part.parse::<IpAddr>() {
                    Ok(addr) => ips.push(Ip::Address(addr)),
                    Err(_) => {
                        if !part.trim().is_empty() {
                            domains.push(part.to_owned())
                        }
                    }
                },
            }
        }
        NoProxy {
            ips: IpMatcher(ips),
            domains: DomainMatcher(domains),
            elm_domains: None,
        }
    }

    /// Return true if this matches the host (domain or IP).
    pub fn contains(&self, host: &str) -> bool {
        if let Some(suffixes) = &self.elm_domains {
            let host = host.to_ascii_lowercase();
            let host = if host.starts_with('.') {
                host
            } else {
                format!(".{host}")
            };
            return suffixes.iter().any(|suffix| host.ends_with(suffix));
        }
        // According to RFC3986, raw IPv6 hosts will be wrapped in []. So we need to strip those off
        // the end in order to parse correctly
        let host = if host.starts_with('[') {
            let x: &[_] = &['[', ']'];
            host.trim_matches(x)
        } else {
            host
        };
        match host.parse::<IpAddr>() {
            // If we can parse an IP addr, then use it, otherwise, assume it is a domain
            Ok(ip) => self.ips.contains(ip),
            Err(_) => self.domains.contains(host),
        }
    }

    fn is_empty(&self) -> bool {
        self.elm_domains.as_ref().map_or_else(
            || self.ips.0.is_empty() && self.domains.0.is_empty(),
            |domains| domains.is_empty(),
        )
    }
}

impl IpMatcher {
    fn contains(&self, addr: IpAddr) -> bool {
        for ip in &self.0 {
            match ip {
                Ip::Address(address) => {
                    if &addr == address {
                        return true;
                    }
                }
                Ip::Network(net) => {
                    if net.contains(&addr) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl DomainMatcher {
    // The following links may be useful to understand the origin of these rules:
    // * https://curl.se/libcurl/c/CURLOPT_NOPROXY.html
    // * https://github.com/curl/curl/issues/1208
    fn contains(&self, domain: &str) -> bool {
        let domain_len = domain.len();
        for d in &self.0 {
            if d.eq_ignore_ascii_case(domain)
                || d.strip_prefix('.')
                    .map_or(false, |s| s.eq_ignore_ascii_case(domain))
            {
                return true;
            } else if domain
                .get(domain_len.saturating_sub(d.len())..)
                .map_or(false, |s| s.eq_ignore_ascii_case(d))
            {
                if d.starts_with('.') {
                    // If the first character of d is a dot, that means the first character of domain
                    // must also be a dot, so we are looking at a subdomain of d and that matches
                    return true;
                } else if domain.as_bytes().get(domain_len - d.len() - 1) == Some(&b'.') {
                    // Given that d is a prefix of domain, if the prior character in domain is a dot
                    // then that means we must be matching a subdomain of d, and that matches
                    return true;
                }
            } else if d == "*" {
                return true;
            }
        }
        false
    }
}

mod builder {
    /// A type that can used as a `Builder` value.
    ///
    /// Private and sealed, only visible in docs.
    pub trait IntoValue {
        #[doc(hidden)]
        fn into_value(self) -> String;
    }

    impl IntoValue for String {
        #[doc(hidden)]
        fn into_value(self) -> String {
            self
        }
    }

    impl IntoValue for &String {
        #[doc(hidden)]
        fn into_value(self) -> String {
            self.into()
        }
    }

    impl IntoValue for &str {
        #[doc(hidden)]
        fn into_value(self) -> String {
            self.into()
        }
    }
}

#[cfg(feature = "client-proxy-system")]
#[cfg(target_os = "macos")]
mod mac {
    use system_configuration::core_foundation::base::CFType;
    use system_configuration::core_foundation::dictionary::CFDictionary;
    use system_configuration::core_foundation::number::CFNumber;
    use system_configuration::core_foundation::string::{CFString, CFStringRef};
    use system_configuration::dynamic_store::SCDynamicStoreBuilder;
    use system_configuration::sys::schema_definitions::{
        kSCPropNetProxiesHTTPEnable, kSCPropNetProxiesHTTPPort, kSCPropNetProxiesHTTPProxy,
        kSCPropNetProxiesHTTPSEnable, kSCPropNetProxiesHTTPSPort, kSCPropNetProxiesHTTPSProxy,
    };

    pub(super) fn with_system(builder: &mut super::Builder) {
        let store = if let Some(store) = SCDynamicStoreBuilder::new("hyper-util").build() {
            store
        } else {
            return;
        };

        let proxies_map = if let Some(proxies_map) = store.get_proxies() {
            proxies_map
        } else {
            return;
        };

        if builder.http.is_empty() {
            let http_proxy_config = parse_setting_from_dynamic_store(
                &proxies_map,
                unsafe { kSCPropNetProxiesHTTPEnable },
                unsafe { kSCPropNetProxiesHTTPProxy },
                unsafe { kSCPropNetProxiesHTTPPort },
            );
            if let Some(http) = http_proxy_config {
                builder.http = http;
            }
        }

        if builder.https.is_empty() {
            let https_proxy_config = parse_setting_from_dynamic_store(
                &proxies_map,
                unsafe { kSCPropNetProxiesHTTPSEnable },
                unsafe { kSCPropNetProxiesHTTPSProxy },
                unsafe { kSCPropNetProxiesHTTPSPort },
            );

            if let Some(https) = https_proxy_config {
                builder.https = https;
            }
        }
    }

    fn parse_setting_from_dynamic_store(
        proxies_map: &CFDictionary<CFString, CFType>,
        enabled_key: CFStringRef,
        host_key: CFStringRef,
        port_key: CFStringRef,
    ) -> Option<String> {
        let proxy_enabled = proxies_map
            .find(enabled_key)
            .and_then(|flag| flag.downcast::<CFNumber>())
            .and_then(|flag| flag.to_i32())
            .unwrap_or(0)
            == 1;

        if proxy_enabled {
            let proxy_host = proxies_map
                .find(host_key)
                .and_then(|host| host.downcast::<CFString>())
                .map(|host| host.to_string());
            let proxy_port = proxies_map
                .find(port_key)
                .and_then(|port| port.downcast::<CFNumber>())
                .and_then(|port| port.to_i32());

            return match (proxy_host, proxy_port) {
                (Some(proxy_host), Some(proxy_port)) => Some(format!("{proxy_host}:{proxy_port}")),
                (Some(proxy_host), None) => Some(proxy_host),
                (None, Some(_)) => None,
                (None, None) => None,
            };
        }

        None
    }
}

#[cfg(feature = "client-proxy-system")]
#[cfg(windows)]
mod win {
    pub(super) fn with_system(builder: &mut super::Builder) {
        let settings = if let Ok(settings) = windows_registry::CURRENT_USER
            .open("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        {
            settings
        } else {
            return;
        };

        if settings.get_u32("ProxyEnable").unwrap_or(0) == 0 {
            return;
        }

        if let Ok(val) = settings.get_string("ProxyServer") {
            if builder.http.is_empty() {
                builder.http = val.clone();
            }
            if builder.https.is_empty() {
                builder.https = val;
            }
        }

        if builder.no.is_empty() {
            if let Ok(val) = settings.get_string("ProxyOverride") {
                builder.no = val
                    .split(';')
                    .map(|s| s.trim())
                    .collect::<Vec<&str>>()
                    .join(",")
                    .replace("*.", "");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_matcher() {
        let domains = vec![".foo.bar".into(), "bar.foo".into()];
        let matcher = DomainMatcher(domains);

        // domains match with leading `.`
        assert!(matcher.contains("foo.bar"));
        assert!(matcher.contains("FOO.BAR"));

        // subdomains match with leading `.`
        assert!(matcher.contains("www.foo.bar"));
        assert!(matcher.contains("WWW.FOO.BAR"));

        // domains match with no leading `.`
        assert!(matcher.contains("bar.foo"));
        assert!(matcher.contains("Bar.foo"));

        // subdomains match with no leading `.`
        assert!(matcher.contains("www.bar.foo"));
        assert!(matcher.contains("WWW.BAR.FOO"));

        // non-subdomain string prefixes don't match
        assert!(!matcher.contains("notfoo.bar"));
        assert!(!matcher.contains("notbar.foo"));
    }

    #[cfg(feature = "elm-proxy-environment")]
    #[test]
    fn elm_proxy_auth_matches_reference_userinfo_encoding() {
        for (userinfo, expected) in [
            ("us%FFr:p%FFss", Some("Basic dXP9cjpw/XNz")),
            ("us%C3%A9r:p%C3%A9ss", Some("Basic dXPpcjpw6XNz")),
            ("u%C0%80:pass", Some("Basic df06cGFzcw==")),
            ("u%E0%80%80:pass", Some("Basic df06cGFzcw==")),
            ("u%F0%80%80%80:pass", Some("Basic df06cGFzcw==")),
            ("u%ED%A0%80:pass", Some("Basic df06cGFzcw==")),
            ("u%F4%90%80%80:pass", Some("Basic df06cGFzcw==")),
            ("user", None),
            (":pass", Some("Basic OnBhc3M=")),
        ] {
            let value = format!("http://{userinfo}@proxy.test:80");
            let environment = vec![("http_proxy".to_owned(), value)];
            let matcher = super::Builder::from_elm_environment(&environment, false).build();
            let proxy = matcher
                .intercept(&"http://archive.test/".parse().unwrap())
                .unwrap();
            assert_eq!(
                proxy.basic_auth().map(|value| value.to_str().unwrap()),
                expected,
                "{userinfo}"
            );
        }
        // Explicit proxy builders retain the upstream authentication contract.
        let matcher = super::Builder::default()
            .http("http://user@proxy.test:80")
            .build();
        let proxy = matcher
            .intercept(&"http://archive.test/".parse().unwrap())
            .unwrap();
        assert_eq!(proxy.basic_auth().unwrap(), "Basic dXNlcjo=");
    }

    #[cfg(feature = "elm-proxy-environment")]
    #[test]
    fn elm_proxy_environment_uses_protocol_variables_without_all_fallback() {
        for (pairs, http, https) in [
            (vec![("HtTp_PrOxY", "http://proxy.test:80")], true, false),
            (vec![("HtTpS_PrOxY", "http://proxy.test:80")], false, true),
            (
                vec![("HTTP_PROXY", "http://proxy.test:80"), ("http_proxy", "")],
                false,
                false,
            ),
            (
                vec![("HTTP_PROXY", ""), ("http_proxy", "http://proxy.test:80")],
                true,
                false,
            ),
            (vec![("ALL_PROXY", "http://proxy.test:80")], false, false),
        ] {
            let environment = pairs
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect::<Vec<_>>();
            let matcher = super::Builder::from_elm_environment(&environment, false).build();
            assert_eq!(
                matcher
                    .intercept(&"http://archive.test/".parse().unwrap())
                    .is_some(),
                http
            );
            assert_eq!(
                matcher
                    .intercept(&"https://archive.test/".parse().unwrap())
                    .is_some(),
                https
            );
        }
    }

    #[cfg(feature = "elm-no-proxy")]
    #[test]
    fn elm_no_proxy_environment_case_and_priority() {
        for (pairs, expected) in [
            (vec![("No_PrOxY", "mixed")], Some("mixed")),
            (vec![("NO_PROXY", "upper")], Some("upper")),
            (vec![("NO_PROXY", "upper"), ("no_proxy", "")], Some("")),
            (
                vec![("no_proxy", "lower"), ("NO_PROXY", "upper")],
                Some("lower"),
            ),
            (
                vec![("NO_PROXY", "upper"), ("No_Proxy", "last")],
                Some("last"),
            ),
            (vec![], None),
        ] {
            let environment = pairs
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect::<Vec<_>>();
            assert_eq!(
                super::elm_proxy_environment_value(&environment, "no_proxy"),
                expected
            );
        }
    }

    #[cfg(feature = "elm-no-proxy")]
    #[test]
    fn elm_no_proxy_suffixes_match_reference_rules() {
        for (list, host, matches) in [
            ("127.0.0.1", "127.0.0.1", true),
            (" 127.0.0.1", "127.0.0.1", true),
            ("127.0.0.1 ", "127.0.0.1", false),
            ("127.0.0.0/8", "127.0.0.1", false),
            (".127.0.0.1", "127.0.0.1", true),
            ("::1", "[::1]", false),
            ("[::1]", "[::1]", true),
            ("example.test", "sub.example.test", true),
            ("example.test", "notexample.test", false),
            ("EXAMPLE.TEST", "sub.example.test", true),
        ] {
            assert_eq!(
                NoProxy::from_elm_string(list).contains(host),
                matches,
                "{list:?} / {host:?}"
            );
        }
    }

    #[test]
    fn test_no_proxy_wildcard() {
        let no_proxy = NoProxy::from_string("*");
        assert!(no_proxy.contains("any.where"));
    }

    #[test]
    fn test_no_proxy_ip_ranges() {
        let no_proxy =
            NoProxy::from_string(".foo.bar, bar.baz,10.42.1.1/24,::1,10.124.7.8,2001::/17");

        let should_not_match = [
            // random url, not in no_proxy
            "hyper.rs",
            // make sure that random non-subdomain string prefixes don't match
            "notfoo.bar",
            // make sure that random non-subdomain string prefixes don't match
            "notbar.baz",
            // ipv4 address out of range
            "10.43.1.1",
            // ipv4 address out of range
            "10.124.7.7",
            // ipv6 address out of range
            "[ffff:db8:a0b:12f0::1]",
            // ipv6 address out of range
            "[2005:db8:a0b:12f0::1]",
        ];

        for host in &should_not_match {
            assert!(!no_proxy.contains(host), "should not contain {host:?}");
        }

        let should_match = [
            // make sure subdomains (with leading .) match
            "hello.foo.bar",
            // make sure exact matches (without leading .) match (also makes sure spaces between entries work)
            "bar.baz",
            // make sure subdomains (without leading . in no_proxy) match
            "foo.bar.baz",
            // make sure subdomains (without leading . in no_proxy) match - this differs from cURL
            "foo.bar",
            // ipv4 address match within range
            "10.42.1.100",
            // ipv6 address exact match
            "[::1]",
            // ipv6 address match within range
            "[2001:db8:a0b:12f0::1]",
            // ipv4 address exact match
            "10.124.7.8",
        ];

        for host in &should_match {
            assert!(no_proxy.contains(host), "should contain {host:?}");
        }
    }

    macro_rules! p {
        ($($n:ident = $v:expr,)*) => ({Builder {
            $($n: $v.into(),)*
            ..Builder::default()
        }.build()});
    }

    fn intercept(p: &Matcher, u: &str) -> Intercept {
        p.intercept(&u.parse().unwrap()).unwrap()
    }

    #[test]
    fn test_all_proxy() {
        let p = p! {
            all = "http://om.nom",
        };

        assert_eq!("http://om.nom", intercept(&p, "http://example.com").uri());

        assert_eq!("http://om.nom", intercept(&p, "https://example.com").uri());
    }

    #[test]
    fn test_specific_overrides_all() {
        let p = p! {
            all = "http://no.pe",
            http = "http://y.ep",
        };

        assert_eq!("http://no.pe", intercept(&p, "https://example.com").uri());

        // the http rule is "more specific" than the all rule
        assert_eq!("http://y.ep", intercept(&p, "http://example.com").uri());
    }

    #[test]
    fn test_parse_no_scheme_defaults_to_http() {
        let p = p! {
            https = "y.ep",
            http = "127.0.0.1:8887",
        };

        assert_eq!(intercept(&p, "https://example.local").uri(), "http://y.ep");
        assert_eq!(
            intercept(&p, "http://example.local").uri(),
            "http://127.0.0.1:8887"
        );
    }

    #[test]
    fn test_parse_http_auth() {
        let p = p! {
            all = "http://Aladdin:opensesame@y.ep",
        };

        let proxy = intercept(&p, "https://example.local");
        assert_eq!(proxy.uri(), "http://y.ep");
        assert_eq!(
            proxy.basic_auth().expect("basic_auth"),
            "Basic QWxhZGRpbjpvcGVuc2VzYW1l"
        );
    }

    #[test]
    fn test_parse_http_auth_without_password() {
        let p = p! {
            all = "http://Aladdin@y.ep",
        };
        let proxy = intercept(&p, "https://example.local");
        assert_eq!(proxy.uri(), "http://y.ep");
        assert_eq!(
            proxy.basic_auth().expect("basic_auth"),
            "Basic QWxhZGRpbjo="
        );
    }

    #[test]
    fn test_parse_http_auth_without_scheme() {
        let p = p! {
            all = "Aladdin:opensesame@y.ep",
        };

        let proxy = intercept(&p, "https://example.local");
        assert_eq!(proxy.uri(), "http://y.ep");
        assert_eq!(
            proxy.basic_auth().expect("basic_auth"),
            "Basic QWxhZGRpbjpvcGVuc2VzYW1l"
        );
    }

    #[test]
    fn test_dont_parse_http_when_is_cgi() {
        let mut builder = Matcher::builder();
        builder.is_cgi = true;
        builder.http = "http://never.gonna.let.you.go".into();
        let m = builder.build();

        assert!(m.intercept(&"http://rick.roll".parse().unwrap()).is_none());
    }

    #[test]
    fn test_domain_matcher_case_insensitive() {
        let domains = vec![".foo.bar".into()];
        let matcher = DomainMatcher(domains);

        assert!(matcher.contains("foo.bar"));
        assert!(matcher.contains("FOO.BAR"));
        assert!(matcher.contains("Foo.Bar"));

        assert!(matcher.contains("www.foo.bar"));
        assert!(matcher.contains("WWW.FOO.BAR"));
        assert!(matcher.contains("Www.Foo.Bar"));
    }

    #[test]
    fn test_no_proxy_case_insensitive() {
        let p = p! {
            all = "http://proxy.local",
            no = ".example.com",
        };

        // should bypass proxy (case insensitive match)
        assert!(
            p.intercept(&"http://example.com".parse().unwrap())
                .is_none()
        );
        assert!(
            p.intercept(&"http://EXAMPLE.COM".parse().unwrap())
                .is_none()
        );
        assert!(
            p.intercept(&"http://Example.com".parse().unwrap())
                .is_none()
        );

        // subdomain should bypass proxy (case insensitive match)
        assert!(
            p.intercept(&"http://www.example.com".parse().unwrap())
                .is_none()
        );
        assert!(
            p.intercept(&"http://WWW.EXAMPLE.COM".parse().unwrap())
                .is_none()
        );
        assert!(
            p.intercept(&"http://Www.Example.Com".parse().unwrap())
                .is_none()
        );
    }
}
