//! Validate HTTP manager proxy settings before creating network clients.

pub(crate) fn validate() -> Result<(), String> {
    let environment = std::env::vars_os()
        .filter_map(|(key, value)| {
            let key = key.into_string().ok()?;
            if !["http_proxy", "https_proxy"]
                .iter()
                .any(|name| key.eq_ignore_ascii_case(name))
            {
                return None;
            }
            Some((key, value.into_string().ok()?))
        })
        .collect::<Vec<_>>();
    for name in ["http_proxy", "https_proxy"] {
        if let Some(value) =
            hyper_util::client::proxy::matcher::elm_proxy_environment_value(&environment, name)
            && !value.is_empty()
            && !valid_proxy(value)
        {
            return Err(format!(
                "ELM_CLI_RAW:elm: HttpExceptionContentWrapper {{unHttpExceptionContentWrapper = InvalidProxyEnvironmentVariable {} {}}}\n",
                crate::package_network::haskell_text(name),
                crate::package_network::haskell_text(value)
            ));
        }
    }
    Ok(())
}

fn valid_proxy(value: &str) -> bool {
    let bytes = value.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'%'
            && (!bytes.get(index + 1).is_some_and(u8::is_ascii_hexdigit)
                || !bytes.get(index + 2).is_some_and(u8::is_ascii_hexdigit))
        {
            return false;
        }
    }
    if value.contains('#') || value.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return false;
    }
    let candidate = if value.starts_with("http:") {
        value.to_owned()
    } else {
        format!("http://{value}")
    };
    let Ok(uri) = candidate.parse::<hyper::http::Uri>() else {
        return false;
    };
    if uri.scheme_str() != Some("http") || uri.query().is_some() || !matches!(uri.path(), "" | "/")
    {
        return false;
    }
    let Some(authority) = uri.authority() else {
        return false;
    };
    // URI userinfo permits an encoded @, but only one literal delimiter.
    if authority
        .as_str()
        .bytes()
        .filter(|byte| *byte == b'@')
        .count()
        > 1
    {
        return false;
    }
    let host_port = authority.as_str().rsplit('@').next().unwrap_or_default();
    let port = if host_port.starts_with('[') {
        let Some((_, rest)) = host_port.split_once(']') else {
            return false;
        };
        if rest.is_empty() {
            return true;
        }
        rest.strip_prefix(':')
    } else {
        host_port.rsplit_once(':').map(|(_, port)| port)
    };
    port.is_none_or(|port| !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_uri_validation_retains_reference_restrictions() {
        for value in [
            "not a proxy",
            "http://127.0.0.1:wrong",
            "http://proxy/path",
            "http://proxy?x=y",
            "http://proxy#fragment",
            "ftp://proxy",
            "http://proxy:",
            "http://user%GG@proxy",
            "http://user%4@proxy",
            "http://user%@proxy",
            "http://user@name@proxy",
        ] {
            assert!(!valid_proxy(value), "must reject {value:?}");
        }
        for value in [
            "proxy",
            "proxy:8080",
            "http://proxy",
            "http://proxy/",
            "http://user:p%40ss@proxy:80/",
            "http://us%65r:p%4ass@proxy/",
            "http://user%2525@proxy/",
            "http://[::1]:80",
            "http://proxy:65536",
        ] {
            assert!(valid_proxy(value), "must accept {value:?}");
        }
    }
}
