use planexpo_elm::{package_network::PackageNetwork, package_solver::Version};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

#[test]
fn invalid_metadata_is_never_published_to_the_cache() {
    for body in [
        "not json",
        r#"{"type":"application"}"#,
        r#"{"type":"package","elm-version":"0.19.0\u0020<= v < 0.20.0","dependencies":{}}"#,
        r#"{"type":"package","elm-version":"0.19.0 <= v < 0.20.0","dependencies":{"elm/core":"1.0.0  <= v < 2.0.0"}}"#,
        r#"{"type":"package","elm-version":"latest","dependencies":{}}"#,
        r#"{"type":"package","elm-version":"0.19.0 <= v < 0.20.0","dependencies":{"../evil":"1.0.0 <= v < 2.0.0"}}"#,
    ] {
        // Keep every unrelated field valid so each malformed value exercises
        // its own validation rather than failing on missing package fields.
        let body = if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
            if value["type"] == "package" {
                let mut defaults = valid_metadata();
                for key in value.as_object().unwrap().keys() {
                    defaults.as_object_mut().unwrap().remove(key);
                }
                let defaults = defaults.to_string();
                format!("{},{}", &defaults[..defaults.len() - 1], &body[1..])
            } else {
                body.to_string()
            }
        } else {
            body.to_string()
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
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
            assert!(
                String::from_utf8(request)
                    .unwrap()
                    .starts_with("GET /packages/author/pkg/1.0.0/elm.json HTTP/1.1\r\n")
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let home = tempfile::tempdir().unwrap();
        assert!(
            PackageNetwork::new(&url)
                .unwrap()
                .metadata(home.path(), "author/pkg", Version([1, 0, 0]))
                .is_err()
        );
        handle.join().unwrap();
        assert!(
            !home
                .path()
                .join("0.19.1/packages/author/pkg/1.0.0/elm.json")
                .exists()
        );
    }
}
#[test]
fn cached_metadata_does_not_require_sources_or_a_live_server() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("0.19.1/packages/author/pkg/1.0.0");
    std::fs::create_dir_all(&root).unwrap();
    let config = valid_metadata();
    std::fs::write(root.join("elm.json"), config.to_string()).unwrap();
    let network = PackageNetwork::new("http://127.0.0.1:9").unwrap();
    assert_eq!(
        network
            .metadata(home.path(), "author/pkg", Version([1, 0, 0]))
            .unwrap(),
        config
    );
    assert!(!root.join("src").exists());
}

fn valid_metadata() -> serde_json::Value {
    serde_json::json!({"type":"package", "name":"author/pkg", "summary":"Test package",
        "license":"BSD-3-Clause", "version":"1.0.0", "exposed-modules":[],
        "elm-version":"0.19.0 <= v < 0.20.0", "dependencies":{}, "test-dependencies":{}})
}

#[test]
fn cached_metadata_validates_the_entire_package_outline() {
    for field in [
        "name",
        "summary",
        "license",
        "version",
        "exposed-modules",
        "test-dependencies",
    ] {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("0.19.1/packages/author/pkg/1.0.0");
        std::fs::create_dir_all(&root).unwrap();
        let mut config = valid_metadata();
        config.as_object_mut().unwrap().remove(field);
        std::fs::write(root.join("elm.json"), config.to_string()).unwrap();
        let network = PackageNetwork::new("http://127.0.0.1:9").unwrap();
        assert!(
            network
                .metadata(home.path(), "author/pkg", Version([1, 0, 0]))
                .is_err(),
            "accepted missing {field}"
        );
    }
}

#[test]
fn cached_metadata_rejects_invalid_outline_fields() {
    for (field, value) in [
        ("name", serde_json::json!("invalid")),
        ("summary", serde_json::json!("x".repeat(80))),
        ("license", serde_json::json!("Unknown-License")),
        ("version", serde_json::json!("latest")),
        ("exposed-modules", serde_json::json!(["lowercase"])),
        (
            "test-dependencies",
            serde_json::json!({"elm/core":"latest"}),
        ),
    ] {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("0.19.1/packages/author/pkg/1.0.0");
        std::fs::create_dir_all(&root).unwrap();
        let mut config = valid_metadata();
        config[field] = value;
        std::fs::write(root.join("elm.json"), config.to_string()).unwrap();
        let network = PackageNetwork::new("http://127.0.0.1:9").unwrap();
        assert!(
            network
                .metadata(home.path(), "author/pkg", Version([1, 0, 0]))
                .is_err(),
            "accepted invalid {field}"
        );
    }
}

#[test]
fn corrupt_cached_json_is_removed_without_fetching_or_removing_sources() {
    for bytes in [b"not json".as_slice(), &[255u8, 254], b"{}"] {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("0.19.1/packages/author/pkg/1.0.0");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/Keep.elm"), "keep").unwrap();
        std::fs::write(root.join("elm.json"), bytes).unwrap();
        let network = PackageNetwork::new("http://127.0.0.1:9").unwrap();
        assert!(
            network
                .metadata(home.path(), "author/pkg", Version([1, 0, 0]))
                .is_err()
        );
        assert!(!root.join("elm.json").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("src/Keep.elm")).unwrap(),
            "keep"
        );
    }
}
