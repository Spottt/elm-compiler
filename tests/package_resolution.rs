use planexpo_elm::{
    package_network::PackageNetwork, package_resolution::resolve_with, registry::Registry,
};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    thread,
};
fn cache(home: &Path, versions: &[&str]) {
    let packages = home.join("0.19.1/packages");
    fs::create_dir_all(&packages).unwrap();
    fs::write(
        packages.join("registry.dat"),
        Registry::from_json(&json!({"author/pkg":versions}))
            .unwrap()
            .encode()
            .unwrap(),
    )
    .unwrap();
    for version in versions {
        let root = packages.join("author/pkg").join(version);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("elm.json"),
            json!({"type":"package","name":"author/pkg","summary":"Test package","license":"BSD-3-Clause","version":version,"exposed-modules":[],"test-dependencies":{},"elm-version":"0.19.0 <= v < 0.20.0","dependencies":{}})
                .to_string(),
        )
        .unwrap();
    }
}
fn server(count: usize) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
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
                .starts_with(&format!("POST /all-packages/since/{count} HTTP/1.1\r\n"))
        );
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]"
        )
        .unwrap();
    });
    (url, handle)
}
fn manifest(root: &Path) -> std::path::PathBuf {
    let path = root.join("elm.json");
    fs::write(&path,json!({"type":"package","elm-version":"0.19.0 <= v < 0.20.0","dependencies":{"author/pkg":"1.0.0 <= v < 2.0.0"},"test-dependencies":{}}).to_string()).unwrap();
    path
}
#[test]
fn selected_versions_stay_stable_until_the_project_manifest_changes() {
    let project = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let path = manifest(project.path());
    cache(home.path(), &["1.0.0"]);
    let (url, h) = server(1);
    let first = resolve_with(&path, home.path(), || PackageNetwork::new(&url)).unwrap();
    h.join().unwrap();
    assert_eq!(first["author/pkg"], "1.0.0");
    cache(home.path(), &["1.0.0", "1.1.0"]);
    assert_eq!(
        resolve_with(&path, home.path(), || panic!(
            "unchanged project contacted network"
        ))
        .unwrap(),
        first
    );
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"\n")
        .unwrap();
    let (url, h) = server(2);
    assert_eq!(
        resolve_with(&path, home.path(), || PackageNetwork::new(&url)).unwrap()["author/pkg"],
        "1.1.0"
    );
    h.join().unwrap();
}
#[test]
fn corrupt_resolution_is_rebuilt_and_offline_registry_fallback_still_works() {
    let project = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let path = manifest(project.path());
    cache(home.path(), &["1.0.0"]);
    let make_network = || PackageNetwork::new("http://127.0.0.1:9");
    let expected = resolve_with(&path, home.path(), make_network).unwrap();
    let saved = project
        .path()
        .join("elm-stuff/planexpo-rust/dependencies-v1.json");
    fs::write(&saved, "truncated JSON").unwrap();
    assert_eq!(
        resolve_with(&path, home.path(), make_network).unwrap(),
        expected
    );
    let mut tampered: serde_json::Value =
        serde_json::from_slice(&fs::read(&saved).unwrap()).unwrap();
    tampered["selected"] = json!({});
    tampered["metadata"] = json!({});
    fs::write(&saved, tampered.to_string()).unwrap();
    assert_eq!(
        resolve_with(&path, home.path(), make_network).unwrap(),
        expected
    );
    let other_home = tempfile::tempdir().unwrap();
    cache(other_home.path(), &["1.1.0"]);
    assert_eq!(
        resolve_with(&path, other_home.path(), make_network).unwrap()["author/pkg"],
        "1.1.0"
    );
}

#[test]
fn applications_refresh_on_first_build_and_manifest_change_but_not_source_rebuilds() {
    let project = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    cache(home.path(), &["1.0.0"]);
    let path = project.path().join("elm.json");
    fs::write(
        &path,
        json!({"type":"application","elm-version":"0.19.1",
        "dependencies":{"direct":{"author/pkg":"1.0.0"},"indirect":{}},
        "test-dependencies":{"direct":{},"indirect":{}}})
        .to_string(),
    )
    .unwrap();
    let (url, h) = server(1);
    let first = resolve_with(&path, home.path(), || PackageNetwork::new(&url)).unwrap();
    h.join().unwrap();
    assert_eq!(first["author/pkg"], "1.0.0");
    fs::remove_file(home.path().join("0.19.1/packages/registry.dat")).unwrap();
    assert_eq!(
        resolve_with(&path, home.path(), || panic!(
            "unchanged application contacted network"
        ))
        .unwrap(),
        first
    );
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"\n")
        .unwrap();
    let saved = project
        .path()
        .join("elm-stuff/planexpo-rust/dependencies-v1.json");
    let previous = fs::read(&saved).unwrap();
    assert!(
        resolve_with(&path, home.path(), || PackageNetwork::new(
            "http://127.0.0.1:9"
        ))
        .is_err()
    );
    assert_eq!(fs::read(&saved).unwrap(), previous);
    cache(home.path(), &["1.0.0"]);
    let (url, h) = server(1);
    assert_eq!(
        resolve_with(&path, home.path(), || PackageNetwork::new(&url)).unwrap(),
        first
    );
    h.join().unwrap();
}

#[test]
fn new_application_rejects_unregistered_cached_versions_offline() {
    let project = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    cache(home.path(), &["1.0.0"]);
    let registry = Registry::from_json(&json!({})).unwrap();
    fs::write(
        home.path().join("0.19.1/packages/registry.dat"),
        registry.encode().unwrap(),
    )
    .unwrap();
    let path = project.path().join("elm.json");
    fs::write(&path,json!({"type":"application","elm-version":"0.19.1",
        "dependencies":{"direct":{"author/pkg":"1.0.0"},"indirect":{}},"test-dependencies":{"direct":{},"indirect":{}}}).to_string()).unwrap();
    let error = resolve_with(&path, home.path(), || {
        PackageNetwork::new("http://127.0.0.1:9")
    })
    .unwrap_err();
    assert_eq!(
        planexpo_elm::dependency_error::report_encoded(&error).unwrap()["title"],
        "TROUBLE VERIFYING DEPENDENCIES"
    );
    assert!(
        !project
            .path()
            .join("elm-stuff/planexpo-rust/dependencies-v1.json")
            .exists()
    );
}

#[test]
fn selections_from_before_complete_metadata_validation_are_revalidated() {
    let project = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let path = manifest(project.path());
    cache(home.path(), &["1.0.0"]);
    let (url, h) = server(1);
    let selected = resolve_with(&path, home.path(), || PackageNetwork::new(&url)).unwrap();
    h.join().unwrap();
    let saved_path = project
        .path()
        .join("elm-stuff/planexpo-rust/dependencies-v1.json");
    let mut saved: serde_json::Value =
        serde_json::from_slice(&fs::read(&saved_path).unwrap()).unwrap();
    for old_schema in [1, 2, 3, 4] {
        saved["key"]["schema"] = json!(old_schema);
        fs::write(&saved_path, saved.to_string()).unwrap();
        let mut contacted = false;
        let (url, h) = server(1);
        let actual = resolve_with(&path, home.path(), || {
            contacted = true;
            PackageNetwork::new(&url)
        })
        .unwrap();
        assert!(
            contacted,
            "old dependency selections bypassed metadata validation"
        );
        h.join().unwrap();
        assert_eq!(actual, selected);
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(&saved_path).unwrap()).unwrap();
        assert_eq!(saved["key"]["schema"], 5);
    }
}

#[test]
fn unsatisfiable_constraints_distinguish_online_and_offline_registry() {
    for online in [false, true] {
        let project = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        cache(home.path(), &["1.0.0"]);
        let path = manifest(project.path());
        let mut config: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        config["dependencies"]["author/pkg"] = json!("2.0.0 <= v < 3.0.0");
        fs::write(&path, config.to_string()).unwrap();
        let (url, handle) = if online {
            let (url, handle) = server(1);
            (url, Some(handle))
        } else {
            ("http://127.0.0.1:9".into(), None)
        };
        let error = resolve_with(&path, home.path(), || PackageNetwork::new(&url)).unwrap_err();
        if let Some(handle) = handle {
            handle.join().unwrap();
        }
        let report = planexpo_elm::dependency_error::report_encoded(&error).unwrap();
        assert_eq!(
            report["title"],
            if online {
                "INCOMPATIBLE DEPENDENCIES"
            } else {
                "TROUBLE VERIFYING DEPENDENCIES"
            }
        );
        assert_eq!(report["path"], "elm.json");
    }
}
