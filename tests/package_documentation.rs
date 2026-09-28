use planexpo_elm::{package_network::{PackageNetwork, DocumentationError}, package_solver::Version};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};
#[test]
fn documentation_is_validated_cached_and_corruption_is_reported_before_retry() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
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
        assert!(
            String::from_utf8(request)
                .unwrap()
                .starts_with("GET /packages/author/package/1.0.0/docs.json HTTP/1.1")
        );
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]"
        )
        .unwrap();
    });
    let home = tempfile::tempdir().unwrap();
    let network = PackageNetwork::new(&url).unwrap();
    assert_eq!(
        network
            .documentation(home.path(), "author/package", Version([1, 0, 0]))
            .unwrap(),
        serde_json::json!([])
    );
    worker.join().unwrap();
    assert!(
        network
            .documentation(home.path(), "author/package", Version([1, 0, 0]))
            .is_ok()
    );
    let file = home
        .path()
        .join("0.19.1/packages/author/package/1.0.0/docs.json");
    std::fs::write(&file, "broken").unwrap();
    assert!(matches!(
        network.documentation(home.path(), "author/package", Version([1, 0, 0])).unwrap_err(),
        DocumentationError::CorruptCache
    ));
    assert!(!file.exists());
    assert!(
        network
            .documentation(home.path(), "../outside", Version([1, 0, 0]))
            .is_err()
    );
}

#[test]
fn invalid_download_is_not_written_to_the_cache() {
    for body in [
        "not json",
        "{}",
        r#"[{"name":"Example","comment":"","unions":[],"aliases":[],"binops":[],"values":[{"name":"bad","comment":"","type":"List ("}]}]"#,
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
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let home = tempfile::tempdir().unwrap();
        let error = PackageNetwork::new(&url)
            .unwrap()
            .documentation(home.path(), "author/package", Version([1, 0, 0]))
            .unwrap_err();
        match &error {
            DocumentationError::InvalidData { url: source, body: received } => {
                assert_eq!(source, &format!("{url}/packages/author/package/1.0.0/docs.json"));
                assert_eq!(received, body.as_bytes());
            }
            other => panic!("expected invalid response data, got {other:?}"),
        }
        let encoded = planexpo_elm::diff_diagnostic::documentation(Version([1, 0, 0]), error);
        let report = planexpo_elm::dependency_error::report_encoded(&encoded).unwrap();
        assert_eq!(report["title"], "PROBLEM LOADING DOCS");
        let terminal = planexpo_elm::dependency_error::terminal_report(&report).unwrap();
        assert!(terminal.contains(&format!("{} bytes", body.len())));
        assert!(terminal.contains(if body.len() <= 76 { "whole thing:" } else { "beginning:" }));
        worker.join().unwrap();
        assert!(
            !home
                .path()
                .join("0.19.1/packages/author/package/1.0.0/docs.json")
                .exists()
        );
    }
}
