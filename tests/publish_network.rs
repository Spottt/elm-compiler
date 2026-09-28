use planexpo_elm::{
    package_solver::Version,
    publish_network::{Error, PublicationNetwork},
};
use serde_json::json;
use sha1::{Digest, Sha1};
use std::{
    io::{Cursor, Write},
    thread,
    time::Duration,
};
struct Request {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
fn server(replies: Vec<(u16, Vec<u8>)>) -> (String, thread::JoinHandle<Vec<Request>>) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", server.server_addr());
    let thread = thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, body) in replies {
            let mut request = server
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .expect("missing protocol request");
            let mut bytes = Vec::new();
            request.as_reader().read_to_end(&mut bytes).unwrap();
            requests.push(Request {
                method: request.method().to_string(),
                url: request.url().into(),
                headers: request
                    .headers()
                    .iter()
                    .map(|h| (h.field.to_string(), h.value.to_string()))
                    .collect(),
                body: bytes,
            });
            request
                .respond(tiny_http::Response::from_data(body).with_status_code(status))
                .unwrap();
        }
        requests
    });
    (url, thread)
}
fn archive() -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    zip.add_directory("author-package-hash/", options).unwrap();
    for (name, content) in [
        ("elm.json", "{}"),
        (
            "src/Example.elm",
            "module Example exposing (answer)\nanswer = 42\n",
        ),
        ("README.md", "Package"),
        ("tests/Private.elm", "ignored"),
    ] {
        zip.start_file(format!("author-package-hash/{name}"), options)
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
#[test]
fn tag_archive_and_registration_follow_official_routes_and_payloads() {
    let zip = archive();
    let hash = format!("{:x}", Sha1::digest(&zip));
    let (url, handle) = server(vec![
        (200, br#"{"object":{"sha":"commit & value"}}"#.to_vec()),
        (200, zip),
        (200, b"registered".to_vec()),
    ]);
    let network = PublicationNetwork::with_endpoints(&url, &url, &url).unwrap();
    let version = Version([1, 2, 3]);
    let commit = network.tag("author/package", version).unwrap();
    assert_eq!(commit, "commit & value");
    let downloaded = network.archive("author/package", version).unwrap();
    assert_eq!(downloaded.sha1, hash);
    let extracted = tempfile::tempdir().unwrap();
    downloaded.extract(extracted.path()).unwrap();
    assert!(extracted.path().join("src/Example.elm").is_file());
    assert!(!extracted.path().join("tests").exists());
    let root = tempfile::tempdir().unwrap();
    let manifest = b"{\n  \"type\": \"package\"\n}\n";
    let readme = "Résumé du package\n";
    std::fs::write(root.path().join("elm.json"), manifest).unwrap();
    std::fs::write(root.path().join("README.md"), readme).unwrap();
    let docs = json!([{"name":"Example","comment":"Documentation"}]);
    network
        .register(
            root.path(),
            "author/package",
            version,
            &commit,
            &docs,
            &downloaded,
        )
        .unwrap();
    let requests = handle.join().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].url, "/repos/author/package/git/refs/tags/1.2.3");
    assert!(
        requests[0]
            .headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("accept") && v == "application/json")
    );
    assert_eq!(requests[1].method, "GET");
    assert_eq!(requests[1].url, "/author/package/zipball/1.2.3/");
    let registration = &requests[2];
    assert_eq!(registration.method, "POST");
    let parsed = reqwest::Url::parse(&format!("http://test{}", registration.url)).unwrap();
    assert_eq!(parsed.path(), "/register");
    assert_eq!(
        parsed.query_pairs().collect::<Vec<_>>(),
        vec![
            ("name".into(), "author/package".into()),
            ("version".into(), "1.2.3".into()),
            ("commit-hash".into(), commit.into())
        ]
    );
    let content_type = &registration
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .unwrap()
        .1;
    let boundary = content_type
        .strip_prefix("multipart/form-data; boundary=")
        .unwrap();
    let body = String::from_utf8(registration.body.clone()).unwrap();
    let parts: Vec<_> = body
        .split(&format!("--{boundary}"))
        .filter_map(|part| part.strip_prefix("\r\n"))
        .filter_map(|part| part.strip_suffix("\r\n"))
        .collect();
    assert_eq!(parts.len(), 4);
    for (part, name, filename, expected) in [
        (
            &parts[0],
            "elm.json",
            Some("elm.json"),
            String::from_utf8(manifest.to_vec()).unwrap(),
        ),
        (
            &parts[1],
            "docs.json",
            Some("docs.json"),
            serde_json::to_string(&docs).unwrap(),
        ),
        (&parts[2], "README.md", Some("README.md"), readme.into()),
        (&parts[3], "github-hash", None, hash),
    ] {
        let (headers, contents) = part.split_once("\r\n\r\n").unwrap();
        assert!(headers.contains(&format!("name=\"{name}\"")));
        assert_eq!(contents, expected);
        if let Some(filename) = filename {
            assert!(headers.contains(&format!("filename=\"{filename}\"")));
        } else {
            assert!(!headers.contains("filename="));
        }
    }
    assert!(requests.iter().all(|r| {
        r.headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("user-agent") && v == "elm/0.19.1")
    }));
}
#[test]
fn tag_http_errors_and_malformed_data_remain_distinguishable() {
    let malformed = br#"{"object":{"sha":123}}"#.to_vec();
    let (url, handle) = server(vec![(404, b"missing".to_vec()), (200, malformed.clone())]);
    let network = PublicationNetwork::with_endpoints(&url, &url, &url).unwrap();
    assert!(matches!(
        network.tag("author/package", Version([1, 0, 0])),
        Err(Error::Http {
            status: Some(404),
            ..
        })
    ));
    match network.tag("author/package", Version([1, 0, 0])) {
        Err(Error::TagData { body, .. }) => assert_eq!(body, malformed),
        other => panic!("{other:?}"),
    };
    assert_eq!(handle.join().unwrap().len(), 2);
}
#[test]
fn corrupt_archives_and_registration_failures_are_not_successes() {
    let (url, handle) = server(vec![
        (200, b"not a zip".to_vec()),
        (503, b"unavailable".to_vec()),
    ]);
    let network = PublicationNetwork::with_endpoints(&url, &url, &url).unwrap();
    let version = Version([1, 0, 0]);
    let downloaded = network.archive("author/package", version).unwrap();
    let root = tempfile::tempdir().unwrap();
    assert!(matches!(
        downloaded.extract(root.path()),
        Err(Error::Archive { .. })
    ));
    std::fs::write(root.path().join("elm.json"), "{}").unwrap();
    std::fs::write(root.path().join("README.md"), "Readme").unwrap();
    assert!(matches!(
        network.register(
            root.path(),
            "author/package",
            version,
            "commit",
            &json!([]),
            &downloaded
        ),
        Err(Error::Http {
            status: Some(503),
            ..
        })
    ));
    assert_eq!(handle.join().unwrap().len(), 2);
}
#[test]
fn endpoint_credentials_queries_and_non_http_schemes_are_rejected() {
    for endpoint in [
        "file:///tmp",
        "http://user:pass@localhost",
        "http://localhost?redirect=elsewhere",
        "http://localhost/#fragment",
    ] {
        assert!(
            PublicationNetwork::with_endpoints(endpoint, "http://localhost", "http://localhost")
                .is_err()
        );
    }
}
