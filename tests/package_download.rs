use planexpo_elm::{
    package_network::PackageNetwork,
    package_progress::{self, Event},
    package_solver::Version,
};
use sha1::{Digest, Sha1};
use std::{
    cell::RefCell,
    io::{Cursor, Read, Write},
    net::TcpListener,
    rc::Rc,
    thread,
};
use zip::{ZipWriter, write::SimpleFileOptions};

fn archive(extra: Option<&str>) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    zip.add_directory("repo-1.0.0/", options).unwrap();
    for (name, body) in [
        (
            "elm.json",
            r#"{"type":"package","name":"author/pkg","summary":"Test package","license":"BSD-3-Clause","exposed-modules":[],"test-dependencies":{},"version":"1.0.0","elm-version":"0.19.0 <= v < 0.20.0","dependencies":{}}"#,
        ),
        ("src/Main.elm", "module Main exposing (value)\nvalue = 1\n"),
        ("README.md", "readme"),
        ("LICENSE", "license"),
        ("tests/Test.elm", "ignored"),
    ] {
        zip.start_file(format!("repo-1.0.0/{name}"), options)
            .unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    if let Some(name) = extra {
        zip.start_file(name, options).unwrap();
        zip.write_all(b"bad").unwrap();
    }
    zip.finish().unwrap().into_inner()
}
fn server(bytes: Vec<u8>, bad_hash: bool) -> (String, thread::JoinHandle<()>) {
    server_prepared(bytes, bad_hash, None, None)
}
fn server_prepared(
    bytes: Vec<u8>,
    bad_hash: bool,
    project: Option<&str>,
    endpoint_override: Option<Vec<u8>>,
) -> (String, thread::JoinHandle<()>) {
    server_encoded(bytes, bad_hash, project, endpoint_override, false)
}
fn server_encoded(
    bytes: Vec<u8>,
    bad_hash: bool,
    project: Option<&str>,
    endpoint_override: Option<Vec<u8>>,
    gzip: bool,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hash = if bad_hash {
        "0".repeat(40)
    } else {
        format!("{:x}", Sha1::digest(&bytes))
    };
    let endpoint = serde_json::json!({"url":format!("{url}/archive.zip"),"hash":hash}).to_string();
    let mut responses = Vec::new();
    if project.is_some() {
        responses.push((
            "/all-packages",
            if project == Some("package") {
                br#"{"author/pkg":["1.0.0","1.1.0"]}"#.to_vec()
            } else {
                br#"{"author/pkg":["1.0.0"]}"#.to_vec()
            },
        ));
    }
    if project == Some("package") {
        responses.push((
            "/packages/author/pkg/1.1.0/elm.json",
            br#"{"type":"package","name":"author/pkg","summary":"Test package","license":"BSD-3-Clause","version":"1.0.0","exposed-modules":[],"test-dependencies":{},"elm-version":"0.20.0 <= v < 0.21.0","dependencies":{}}"#
                .to_vec(),
        ));
        responses.push((
            "/packages/author/pkg/1.0.0/elm.json",
            br#"{"type":"package","name":"author/pkg","summary":"Test package","license":"BSD-3-Clause","version":"1.0.0","exposed-modules":[],"test-dependencies":{},"elm-version":"0.19.0 <= v < 0.20.0","dependencies":{}}"#
                .to_vec(),
        ));
    }
    let malformed_endpoint = endpoint_override.is_some();
    responses.extend([
        (
            "/packages/author/pkg/1.0.0/endpoint.json",
            endpoint_override.unwrap_or_else(|| endpoint.into_bytes()),
        ),
        ("/archive.zip", bytes),
    ]);
    if malformed_endpoint {
        responses.pop();
    }
    let handle = thread::spawn(move || {
        for (path, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut b = [0];
                stream.read_exact(&mut b).unwrap();
                request.push(b[0]);
            }
            assert!(String::from_utf8(request).unwrap().starts_with(&format!(
                "{} {path} HTTP/1.1\r\n",
                if path == "/all-packages" {
                    "POST"
                } else {
                    "GET"
                }
            )));
            let body = if gzip {
                let mut encoder =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(&body).unwrap();
                encoder.finish().unwrap()
            } else {
                body
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n",
                if gzip {
                    "Content-Encoding: gzip\r\n"
                } else {
                    ""
                },
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    (url, handle)
}
#[test]
fn verified_sources_are_published_with_the_official_file_filter_and_reused() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed = events.clone();
    let _subscription = package_progress::subscribe(move |event| observed.borrow_mut().push(event));
    let home = tempfile::tempdir().unwrap();
    let (url, handle) = server(archive(None), false);
    let network = PackageNetwork::new(&url).unwrap();
    let root = network
        .package(home.path(), "author/pkg", Version([1, 0, 0]))
        .unwrap();
    handle.join().unwrap();
    assert!(root.join("src/Main.elm").is_file());
    assert!(root.join("LICENSE").is_file());
    assert!(root.join("README.md").is_file());
    assert!(!root.join("tests").exists());
    // The server is now gone. A complete source cache needs no HTTP request.
    assert_eq!(
        network
            .package(home.path(), "author/pkg", Version([1, 0, 0]))
            .unwrap(),
        root
    );
    assert_eq!(
        *events.borrow(),
        vec![
            Event::Started {
                name: "author/pkg".into(),
                version: Version([1, 0, 0])
            },
            Event::Finished {
                name: "author/pkg".into(),
                version: Version([1, 0, 0]),
                success: true
            },
        ]
    );
}
#[test]
fn bad_hash_bad_zip_and_path_traversal_never_replace_existing_metadata() {
    for (bytes, bad_hash, message) in [
        (archive(None), true, "hash"),
        (b"not a zip".to_vec(), false, "unable to unzip"),
        (archive(Some("repo-1.0.0/src/../../escape")), false, "path"),
    ] {
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        let _subscription =
            package_progress::subscribe(move |event| observed.borrow_mut().push(event));
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("0.19.1/packages/author/pkg/1.0.0");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("elm.json"), "existing metadata").unwrap();
        let (url, handle) = server(bytes, bad_hash);
        let error = PackageNetwork::new(&url)
            .unwrap()
            .package(home.path(), "author/pkg", Version([1, 0, 0]))
            .unwrap_err();
        handle.join().unwrap();
        assert!(error.contains(message), "{error}");
        assert_eq!(
            std::fs::read_to_string(root.join("elm.json")).unwrap(),
            "existing metadata"
        );
        assert!(!root.join("src").exists());
        assert!(!root.parent().unwrap().join("escape").exists());
        assert_eq!(
            *events.borrow(),
            vec![
                Event::Started {
                    name: "author/pkg".into(),
                    version: Version([1, 0, 0])
                },
                Event::Finished {
                    name: "author/pkg".into(),
                    version: Version([1, 0, 0]),
                    success: false
                },
            ]
        );
    }
}

#[test]
fn concurrent_downloads_publish_once_and_replace_metadata_only_cache() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("0.19.1/packages/author/pkg/1.0.0");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("elm.json"), "metadata only").unwrap();
    let (url, handle) = server(archive(None), false);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers = (0..2)
        .map(|_| {
            let path = home.path().to_owned();
            let network = PackageNetwork::new(&url).unwrap();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                network
                    .package(&path, "author/pkg", Version([1, 0, 0]))
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        assert_eq!(worker.join().unwrap(), root);
    }
    handle.join().unwrap();
    assert!(root.join("src/Main.elm").is_file());
    assert!(
        std::fs::read_to_string(root.join("elm.json"))
            .unwrap()
            .contains("author/pkg")
    );
    assert_eq!(
        std::fs::read_dir(root.parent().unwrap()).unwrap().count(),
        1
    );
}

#[test]
fn symlinks_and_incomplete_archives_are_rejected_without_publishing() {
    for symlink in [false, true] {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        zip.add_directory("repo/", options).unwrap();
        zip.add_directory("repo/src/", options).unwrap();
        if symlink {
            zip.add_symlink("repo/src/escape", "../../../outside", options)
                .unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let home = tempfile::tempdir().unwrap();
        let (url, handle) = server(bytes, false);
        let error = PackageNetwork::new(&url)
            .unwrap()
            .package(home.path(), "author/pkg", Version([1, 0, 0]))
            .unwrap_err();
        handle.join().unwrap();
        assert!(
            error.contains(if symlink {
                "symbolic link"
            } else {
                "missing elm.json"
            }),
            "{error}"
        );
        assert!(
            !home
                .path()
                .join("0.19.1/packages/author/pkg/1.0.0")
                .exists()
        );
    }
}

#[test]
fn missing_application_and_package_dependencies_are_acquired_then_work_offline() {
    for project in ["application", "package"] {
        let home = tempfile::tempdir().unwrap();
        let (url, handle) = server_prepared(archive(None), false, Some(project), None);
        let config = if project == "application" {
            serde_json::json!({"type":"application","dependencies":{"direct":{"author/pkg":"1.0.0"},"indirect":{}}})
        } else {
            serde_json::json!({"type":"package","dependencies":{"author/pkg":"1.0.0 <= v < 2.0.0"},"test-dependencies":{}})
        };
        planexpo_elm::dependencies::prepare_with(&config, home.path(), || {
            PackageNetwork::new(&url)
        })
        .unwrap();
        handle.join().unwrap();
        if project == "package" {
            assert!(
                home.path()
                    .join("0.19.1/packages/author/pkg/1.1.0/elm.json")
                    .is_file()
            );
            assert!(
                !home
                    .path()
                    .join("0.19.1/packages/author/pkg/1.1.0/src")
                    .exists()
            );
        }
        assert!(
            home.path()
                .join("0.19.1/packages/author/pkg/1.0.0/src/Main.elm")
                .is_file()
        );
        planexpo_elm::dependencies::prepare_with(&config, home.path(), || {
            panic!("warm cache contacted network")
        })
        .unwrap();
    }
}
#[test]
fn malformed_dependency_identities_fail_before_network_or_cache_changes() {
    for name in ["../evil", "author/UPPER", "author/a--b"] {
        let home = tempfile::tempdir().unwrap();
        let config = serde_json::json!({"type":"application","dependencies":{"direct":{name:"1.0.0"},"indirect":{}}});
        assert!(
            planexpo_elm::dependencies::prepare_with(&config, home.path(), || panic!(
                "invalid request contacted network"
            ))
            .is_err()
        );
        assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
    }
}

#[test]
fn changed_archive_hash_has_the_official_structured_diagnostic() {
    let bytes = archive(None);
    let actual = format!("{:x}", Sha1::digest(&bytes));
    let expected = "0".repeat(40);
    let home = tempfile::tempdir().unwrap();
    let (url, handle) = server(bytes, true);
    let error = PackageNetwork::new(&url)
        .unwrap()
        .package(home.path(), "author/pkg", Version([1, 0, 0]))
        .unwrap_err();
    handle.join().unwrap();
    let report = planexpo_elm::dependency_error::report_encoded(&error)
        .expect("archive hash failure must be a structured Elm report");
    assert_eq!(
        report,
        serde_json::json!({
            "type": "error", "path": null, "title": "CORRUPT PACKAGE DATA",
            "message": [
                "I downloaded the source code for author/pkg 1.0.0 from:\n\n    ",
                {"bold": false, "underline": false, "color": "yellow", "string": format!("{url}/archive.zip")},
                format!("\n\nBut it looks like the hash of the archive has changed since publication:\n\n  Expected: {expected}\n    Actual: {actual}\n\nThis usually means that the package author moved the version tag, so report it\nto them and see if that is the issue. Folks on Elm slack can probably help as\nwell.")
            ]
        })
    );
    let terminal = planexpo_elm::dependency_error::terminal_report(&report).unwrap();
    assert!(terminal.contains("-- CORRUPT PACKAGE DATA "));
    assert!(terminal.contains(&format!("  Expected: {expected}\n    Actual: {actual}")));
    assert!(
        !home
            .path()
            .join("0.19.1/packages/author/pkg/1.0.0")
            .exists()
    );
}

#[test]
fn unreadable_zip_takes_precedence_over_a_wrong_hash() {
    for bad_hash in [false, true] {
        let home = tempfile::tempdir().unwrap();
        let (url, handle) = server(b"not a zip".to_vec(), bad_hash);
        let error = PackageNetwork::new(&url)
            .unwrap()
            .package(home.path(), "author/pkg", Version([1, 0, 0]))
            .unwrap_err();
        handle.join().unwrap();
        let report = planexpo_elm::dependency_error::report_encoded(&error).unwrap();
        assert_eq!(report["title"], "PROBLEM DOWNLOADING PACKAGE");
        assert_eq!(report["path"], serde_json::Value::Null);
        assert_eq!(
            report["message"][0],
            "I downloaded the source code for author/pkg 1.0.0 from:\n\n    "
        );
        assert_eq!(report["message"][1]["string"], format!("{url}/archive.zip"));
        assert_eq!(report["message"][1]["color"], "yellow");
        assert!(
            report["message"][2]
                .as_str()
                .unwrap()
                .starts_with("\n\nBut I was unable to unzip the data.")
        );
        assert!(
            !home
                .path()
                .join("0.19.1/packages/author/pkg/1.0.0")
                .exists()
        );
    }
}

#[test]
fn malformed_download_metadata_has_an_endpoint_diagnostic() {
    for body in [
        "not json",
        "{}",
        r#"{"url":12,"hash":"abc"}"#,
        r#"{"url":"https://example.org/a","hash":12}"#,
    ] {
        let home = tempfile::tempdir().unwrap();
        let (url, handle) =
            server_prepared(Vec::new(), false, None, Some(body.as_bytes().to_vec()));
        let error = PackageNetwork::new(&url)
            .unwrap()
            .package(home.path(), "author/pkg", Version([1, 0, 0]))
            .unwrap_err();
        handle.join().unwrap();
        let report = planexpo_elm::dependency_error::report_encoded(&error).unwrap();
        assert_eq!(report["title"], "PROBLEM DOWNLOADING PACKAGE");
        assert_eq!(
            report["message"][0],
            "I need to find the latest download link for author/pkg 1.0.0, but I ran into\ncorrupted information from:\n\n    "
        );
        assert_eq!(
            report["message"][1]["string"],
            format!("{url}/packages/author/pkg/1.0.0/endpoint.json")
        );
        assert!(
            report["message"][2]
                .as_str()
                .unwrap()
                .starts_with("\n\nIs something weird with your internet connection.")
        );
        assert!(
            !home
                .path()
                .join("0.19.1/packages/author/pkg/1.0.0")
                .exists()
        );
    }
}

#[test]
fn interrupted_endpoint_response_has_a_structured_transport_diagnostic() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        // Close before returning any HTTP response headers.
    });
    let home = tempfile::tempdir().unwrap();
    let error = PackageNetwork::new(&url)
        .unwrap()
        .package(home.path(), "author/pkg", Version([1, 0, 0]))
        .unwrap_err();
    worker.join().unwrap();
    let report = planexpo_elm::dependency_error::report_encoded(&error).unwrap();
    assert_eq!(report["title"], "PROBLEM DOWNLOADING PACKAGE");
    assert_eq!(report["path"], serde_json::Value::Null);
    assert_eq!(
        report["message"][1]["string"],
        format!("{url}/packages/author/pkg/1.0.0/endpoint.json")
    );
    let detail = report["message"][2].as_str().unwrap();
    assert!(
        detail.starts_with(
            "\n\nBut my HTTP library is giving me the following error message:\n\n    "
        )
    );
    assert!(detail.contains("connection"), "{detail}");
    assert!(detail.contains("Are you somewhere with a slow internet connection?"));
    assert!(
        !home
            .path()
            .join("0.19.1/packages/author/pkg/1.0.0")
            .exists()
    );
}

#[test]
fn truncated_endpoint_reports_expected_and_received_body_lengths() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 94\r\nConnection: close\r\n\r\n{}")
            .unwrap();
    });
    let home = tempfile::tempdir().unwrap();
    let error = PackageNetwork::new(&url)
        .unwrap()
        .package(home.path(), "author/pkg", Version([1, 0, 0]))
        .unwrap_err();
    worker.join().unwrap();
    let report = planexpo_elm::dependency_error::report_encoded(&error).unwrap();
    assert!(
        report["message"][2]
            .as_str()
            .unwrap()
            .contains("ResponseBodyTooShort 94 2"),
        "{report}"
    );
    assert!(
        !home
            .path()
            .join("0.19.1/packages/author/pkg/1.0.0")
            .exists()
    );
}

#[test]
fn chunked_endpoint_errors_preserve_the_failed_stage() {
    for (wire, expected) in [
        ("a\r\n{}", "InvalidChunkHeaders"),
        ("2\r\n{}\r\n", "IncompleteHeaders"),
        ("2\r\n{}", "IncompleteHeaders"),
        ("2\r\n{}\r", "IncompleteHeaders"),
        ("NOTHEX\r\n{}\r\n0\r\n\r\n", "InvalidChunkHeaders"),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            stream.write_all(format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{wire}").as_bytes()).unwrap();
        });
        let home = tempfile::tempdir().unwrap();
        let error = PackageNetwork::new(&url)
            .unwrap()
            .package(home.path(), "author/pkg", Version([1, 0, 0]))
            .unwrap_err();
        worker.join().unwrap();
        let report = planexpo_elm::dependency_error::report_encoded(&error).unwrap();
        assert!(
            report["message"][2].as_str().unwrap().contains(expected),
            "{report}"
        );
        assert!(
            !home
                .path()
                .join("0.19.1/packages/author/pkg/1.0.0")
                .exists()
        );
    }
}

#[test]
fn gzip_metadata_and_archive_are_decoded_before_hash_verification() {
    let home = tempfile::tempdir().unwrap();
    let (url, handle) = server_encoded(archive(None), false, None, None, true);
    let root = PackageNetwork::new(&url)
        .unwrap()
        .package(home.path(), "author/pkg", Version([1, 0, 0]))
        .unwrap();
    handle.join().unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("src/Main.elm")).unwrap(),
        "module Main exposing (value)\nvalue = 1\n"
    );
}

#[test]
fn corrupt_gzip_endpoint_uses_the_official_zlib_diagnostic() {
    for corruption in ["header", "crc", "size", "deflate"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"{}").unwrap();
        let mut body = encoder.finish().unwrap();
        match corruption {
            "header" => body[0] ^= 255,
            "crc" => {
                let offset = body.len() - 8;
                body[offset] ^= 255;
            }
            "size" => {
                let offset = body.len() - 4;
                body[offset] ^= 255;
            }
            "deflate" => body[10] |= 6,
            _ => unreachable!(),
        }
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
            stream.write_all(&body).unwrap();
        });
        let home = tempfile::tempdir().unwrap();
        let error = PackageNetwork::new(&url)
            .unwrap()
            .package(home.path(), "author/pkg", Version([1, 0, 0]))
            .unwrap_err();
        worker.join().unwrap();
        let report = planexpo_elm::dependency_error::report_encoded(&error).unwrap();
        assert!(
            report["message"][2]
                .as_str()
                .unwrap()
                .contains("HttpZlibException (ZlibException (-3))"),
            "{report}"
        );
        assert!(
            !home
                .path()
                .join("0.19.1/packages/author/pkg/1.0.0")
                .exists()
        );
    }
}
