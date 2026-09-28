//! Full planning/application through a local registry and archive server.
use planexpo_elm::{installation, package_network::PackageNetwork};
use serde_json::json;
use sha1::{Digest, Sha1};
use std::{
    fs,
    io::{Cursor, Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};
use zip::{ZipWriter, write::SimpleFileOptions};

fn metadata(name: &str) -> serde_json::Value {
    json!({"type":"package","name":name,"summary":"Online installation fixture","license":"BSD-3-Clause",
        "version":"1.0.0","elm-version":"0.19.0 <= v < 0.20.0","exposed-modules":[],
        "dependencies":{},"test-dependencies":{}})
}

fn server(failure: &str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let manifest = metadata("author/new").to_string();
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (path, body) in [
        ("elm.json", manifest.as_str()),
        (
            "src/Value.elm",
            "module Value exposing (value)\nvalue = 42\n",
        ),
    ] {
        zip.start_file(format!("repo/{path}"), SimpleFileOptions::default())
            .unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    let archive = if failure == "invalid-archive" {
        b"not a zip file".to_vec()
    } else {
        zip.finish().unwrap().into_inner()
    };
    let hash = if failure == "hash" {
        "0".repeat(40)
    } else {
        format!("{:x}", Sha1::digest(&archive))
    };
    let endpoint = json!({"url":format!("{url}/archive.zip"),"hash":hash}).to_string();
    let mut responses = vec![
        (
            "POST",
            "/all-packages",
            200,
            br#"{"elm/core":["1.0.0"],"elm/json":["1.0.0"],"author/new":["1.0.0"]}"#.to_vec(),
        ),
        (
            "GET",
            "/packages/author/new/1.0.0/elm.json",
            200,
            manifest.into_bytes(),
        ),
        (
            "GET",
            "/packages/author/new/1.0.0/endpoint.json",
            if failure == "endpoint" { 503 } else { 200 },
            endpoint.into_bytes(),
        ),
    ];
    if failure != "endpoint" {
        responses.push(("GET", "/archive.zip", 200, archive));
    }
    let handle = thread::spawn(move || {
        for (method, path, status, body) in responses {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("missing request {method} {path}: {e}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            assert!(
                String::from_utf8(request)
                    .unwrap()
                    .starts_with(&format!("{method} {path} HTTP/1.1\r\n"))
            );
            write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    (url, handle)
}

#[test]
fn online_installation_verifies_archives_before_changing_the_project() {
    for failure in ["", "hash", "endpoint", "invalid-archive"] {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        for name in ["elm/core", "elm/json"] {
            let root = home.path().join("0.19.1/packages").join(name).join("1.0.0");
            fs::create_dir_all(root.join("src")).unwrap();
            fs::write(root.join("elm.json"), metadata(name).to_string()).unwrap();
        }
        let outline = json!({"type":"application","elm-version":"0.19.1","source-directories":["src"],
            "dependencies":{"direct":{"elm/core":"1.0.0","elm/json":"1.0.0"},"indirect":{}},
            "test-dependencies":{"direct":{},"indirect":{}}});
        let original = format!("{}\n\n", outline);
        let path = project.path().join("elm.json");
        fs::write(&path, &original).unwrap();
        let (url, handle) = server(failure);
        let network = PackageNetwork::new(&url).unwrap();
        assert!(network.registry(home.path()).unwrap().online);
        let plan = installation::plan(home.path(), &outline, "author/new", Some(&network)).unwrap();
        let package = home.path().join("0.19.1/packages/author/new/1.0.0");
        assert!(package.join("elm.json").exists());
        assert!(
            !package.join("src").exists(),
            "planning downloaded sources before confirmation"
        );
        let result = installation::apply(
            home.path(),
            &path,
            original.as_bytes(),
            &plan,
            Some(&network),
        );
        handle.join().unwrap();
        if failure.is_empty() {
            result.unwrap();
            let saved: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(saved, plan.outline);
            assert!(package.join("src/Value.elm").exists());
        } else {
            let error = result.expect_err(failure);
            let expected = match failure {
                "hash" => "package archive hash mismatch",
                "endpoint" => "503",
                "invalid-archive" => "invalid package archive",
                _ => unreachable!(),
            };
            assert!(error.contains(expected), "{failure}: {error}");
            assert_eq!(fs::read_to_string(&path).unwrap(), original);
            assert!(!package.join("src").exists());
            assert_eq!(
                fs::read_to_string(package.join("elm.json")).unwrap(),
                metadata("author/new").to_string()
            );
        }
    }
}
