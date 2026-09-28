use planexpo_elm::{package_network::PackageNetwork, registry::Registry};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

fn server(responses: Vec<(&'static str, u16, &'static str)>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        for (path, status, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let request = String::from_utf8(request).unwrap();
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("content-length: 0\r\n"),
                "{request}"
            );
            assert!(
                request.starts_with(&format!("POST {path} HTTP/1.1\r\n")),
                "{request}"
            );
            write!(stream, "HTTP/1.1 {status} Response\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    (url, handle)
}

#[test]
fn fetch_update_and_empty_update_use_official_routes_and_binary_cache() {
    let home = tempfile::tempdir().unwrap();
    let (url, handle) = server(vec![
        ("/all-packages", 200, r#"{"elm/core":["1.0.0"]}"#),
        (
            "/all-packages/since/1",
            200,
            r#"["elm/core@1.0.5","elm/json@1.0.0"]"#,
        ),
        ("/all-packages/since/3", 200, "[]"),
    ]);
    let network = PackageNetwork::new(&url).unwrap();
    let first = network.registry(home.path()).unwrap();
    assert!(first.online);
    assert_eq!(first.registry.count(), 1);
    let next = network.registry(home.path()).unwrap();
    assert!(next.online);
    assert_eq!(next.registry.count(), 3);
    assert_eq!(
        Registry::read_cached(home.path()).unwrap().unwrap(),
        next.registry
    );
    let path = home.path().join("0.19.1/packages/registry.dat");
    let modified = path.metadata().unwrap().modified().unwrap();
    assert_eq!(
        network.registry(home.path()).unwrap().registry,
        next.registry
    );
    assert_eq!(path.metadata().unwrap().modified().unwrap(), modified);
    handle.join().unwrap();
}

#[test]
fn bad_updates_preserve_the_cache_and_fall_back_offline() {
    let home = tempfile::tempdir().unwrap();
    let cache = home.path().join("0.19.1/packages");
    std::fs::create_dir_all(&cache).unwrap();
    let original = Registry::from_json(&json!({"elm/core":["1.0.5"]}))
        .unwrap()
        .encode()
        .unwrap();
    std::fs::write(cache.join("registry.dat"), &original).unwrap();
    let (url, handle) = server(vec![
        ("/all-packages/since/1", 503, "unavailable"),
        (
            "/all-packages/since/1",
            200,
            r#"["elm/core@2.0.0","broken"]"#,
        ),
    ]);
    let network = PackageNetwork::new(&url).unwrap();
    for _ in 0..2 {
        let result = network.registry(home.path()).unwrap();
        assert!(!result.online);
        assert_eq!(result.registry.count(), 1);
        assert_eq!(std::fs::read(cache.join("registry.dat")).unwrap(), original);
    }
    handle.join().unwrap();
    assert!(!network.registry(home.path()).unwrap().online);
}

#[test]
fn failed_initial_fetch_does_not_publish_a_registry() {
    let home = tempfile::tempdir().unwrap();
    let (url, handle) = server(vec![("/all-packages", 200, r#"{"elm/core":[]}"#)]);
    assert!(
        PackageNetwork::new(&url)
            .unwrap()
            .registry(home.path())
            .is_err()
    );
    assert!(Registry::read_cached(home.path()).unwrap().is_none());
    handle.join().unwrap();
}

#[test]
fn simultaneous_syncs_read_the_cache_only_after_obtaining_the_lock() {
    let home = tempfile::tempdir().unwrap();
    let (url, server) = server(vec![
        ("/all-packages", 200, r#"{"elm/core":["1.0.0"]}"#),
        ("/all-packages/since/1", 200, r#"["elm/core@1.0.5"]"#),
    ]);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let clients = (0..2)
        .map(|_| {
            let network = PackageNetwork::new(&url).unwrap();
            let path = home.path().to_owned();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                let result = network.registry(&path).unwrap();
                assert!(result.online);
                result.registry.count()
            })
        })
        .collect::<Vec<_>>();
    let mut counts = clients
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect::<Vec<_>>();
    counts.sort();
    assert_eq!(counts, vec![1, 2]);
    assert_eq!(
        Registry::read_cached(home.path()).unwrap().unwrap().count(),
        2
    );
    server.join().unwrap();
}

#[test]
fn corrupt_registry_is_refetched_and_failed_repair_preserves_original_bytes() {
    for bytes in [b"".as_slice(), &[0u8], &[0u8; 8]] {
        let home = tempfile::tempdir().unwrap();
        let cache = home.path().join("0.19.1/packages");
        std::fs::create_dir_all(&cache).unwrap();
        let path = cache.join("registry.dat");
        std::fs::write(&path, bytes).unwrap();
        let (url, handle) = server(vec![
            ("/all-packages", 503, "unavailable"),
            ("/all-packages", 200, r#"{"elm/core":["1.0.5"]}"#),
        ]);
        let network = PackageNetwork::new(&url).unwrap();
        assert!(network.registry(home.path()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let repaired = network.registry(home.path()).unwrap();
        assert!(repaired.online);
        assert_eq!(repaired.registry.count(), 1);
        assert_eq!(
            Registry::read_cached(home.path()).unwrap().unwrap(),
            repaired.registry
        );
        handle.join().unwrap();
    }
}
