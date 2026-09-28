use planexpo_elm::{package_network::PackageNetwork, package_solver::Version};
use sha1::{Digest, Sha1};
use std::{
    io::{Cursor, Read, Write},
    net::TcpListener,
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
    server_prepared(bytes, bad_hash, None)
}
fn server_prepared(
    bytes: Vec<u8>,
    bad_hash: bool,
    project: Option<&str>,
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
    responses.extend([
        (
            "/packages/author/pkg/1.0.0/endpoint.json",
            endpoint.into_bytes(),
        ),
        ("/archive.zip", bytes),
    ]);
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
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
}
#[test]
fn bad_hash_bad_zip_and_path_traversal_never_replace_existing_metadata() {
    for (bytes, bad_hash, message) in [
        (archive(None), true, "hash"),
        (b"not a zip".to_vec(), false, "archive"),
        (archive(Some("repo-1.0.0/src/../../escape")), false, "path"),
    ] {
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
        let (url, handle) = server_prepared(archive(None), false, Some(project));
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
