use planexpo_elm::package_solver::{Constraint, Version, solve};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Cache(PathBuf);
impl Cache {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "elm-solver-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn package(&self, name: &str, version: &str, elm: &str, dependencies: Value) {
        let root = self.0.join("0.19.1/packages").join(name).join(version);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("elm.json"),
            json!({"type":"package","name":name,"summary":"Test package","license":"BSD-3-Clause","version":version,"exposed-modules":[],"test-dependencies":{},"elm-version":elm,"dependencies":dependencies}).to_string(),
        )
        .unwrap();
    }
}
impl Drop for Cache {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
const ELM: &str = "0.19.0 <= v < 0.20.0";

fn application(direct: Value, indirect: Value, test_direct: Value, test_indirect: Value) -> Value {
    json!({"type":"application", "source-directories":["src"], "elm-version":"0.19.1",
        "dependencies":{"direct":direct,"indirect":indirect},
        "test-dependencies":{"direct":test_direct,"indirect":test_indirect}})
}

#[test]
fn installing_preserves_versions_before_relaxing_patch_minor_and_major() {
    use planexpo_elm::package_solver::add_to_application;
    for (required, expected) in [
        ("1.0.0 <= v < 4.0.0", "1.0.0"),
        ("1.0.1 <= v < 4.0.0", "1.0.2"),
        ("1.1.0 <= v < 4.0.0", "1.2.0"),
        ("2.0.0 <= v < 4.0.0", "3.0.0"),
    ] {
        let cache = Cache::new();
        for version in [
            "1.0.0", "1.0.1", "1.0.2", "1.1.0", "1.2.0", "2.0.0", "3.0.0",
        ] {
            cache.package("author/base", version, ELM, json!({}));
        }
        cache.package("author/new", "1.0.0", ELM, json!({"author/base":required}));
        let original = application(
            json!({"author/base":"1.0.0"}),
            json!({}),
            json!({}),
            json!({}),
        );
        let updated = add_to_application(&cache.0, &original, "author/new", None).unwrap();
        assert_eq!(
            updated["dependencies"]["direct"]["author/base"], expected,
            "{required}"
        );
        assert_eq!(updated["dependencies"]["direct"]["author/new"], "1.0.0");
        assert_eq!(original["dependencies"]["direct"]["author/base"], "1.0.0");
    }
}

#[test]
fn installing_unlocks_indirect_versions_before_direct_versions() {
    use planexpo_elm::package_solver::add_to_application;
    let cache = Cache::new();
    cache.package(
        "author/base",
        "1.0.0",
        ELM,
        json!({"author/shared":"1.0.0 <= v < 3.0.0"}),
    );
    cache.package(
        "author/base",
        "2.0.0",
        ELM,
        json!({"author/shared":"2.0.0 <= v < 3.0.0"}),
    );
    for version in ["1.0.0", "2.0.0"] {
        cache.package("author/shared", version, ELM, json!({}));
    }
    cache.package(
        "author/new",
        "1.0.0",
        ELM,
        json!({"author/shared":"2.0.0 <= v < 3.0.0"}),
    );
    let original = application(
        json!({"author/base":"1.0.0"}),
        json!({"author/shared":"1.0.0"}),
        json!({}),
        json!({}),
    );
    let updated = add_to_application(&cache.0, &original, "author/new", None).unwrap();
    assert_eq!(updated["dependencies"]["direct"]["author/base"], "1.0.0");
    assert_eq!(
        updated["dependencies"]["indirect"]["author/shared"],
        "2.0.0"
    );
}

#[test]
fn installing_splits_test_and_runtime_transitive_dependencies() {
    use planexpo_elm::package_solver::add_to_application;
    let cache = Cache::new();
    for name in ["author/runtime", "author/shared", "author/test-only"] {
        cache.package(name, "1.0.0", ELM, json!({}));
    }
    cache.package(
        "author/tests",
        "1.0.0",
        ELM,
        json!({"author/shared":"1.0.0 <= v < 2.0.0","author/test-only":"1.0.0 <= v < 2.0.0"}),
    );
    cache.package(
        "author/new",
        "1.0.0",
        ELM,
        json!({"author/runtime":"1.0.0 <= v < 2.0.0","author/shared":"1.0.0 <= v < 2.0.0"}),
    );
    let original = application(
        json!({}),
        json!({}),
        json!({"author/tests":"1.0.0"}),
        json!({"author/shared":"1.0.0","author/test-only":"1.0.0"}),
    );
    let updated = add_to_application(&cache.0, &original, "author/new", None).unwrap();
    assert_eq!(
        updated["dependencies"]["indirect"],
        json!({"author/runtime":"1.0.0","author/shared":"1.0.0"})
    );
    assert_eq!(
        updated["test-dependencies"]["direct"],
        json!({"author/tests":"1.0.0"})
    );
    assert_eq!(
        updated["test-dependencies"]["indirect"],
        json!({"author/test-only":"1.0.0"})
    );
}

#[test]
fn installing_backtracks_new_package_before_upgrading_existing_packages() {
    use planexpo_elm::package_solver::add_to_application;
    let cache = Cache::new();
    cache.package("author/base", "1.0.0", ELM, json!({}));
    cache.package("author/base", "2.0.0", ELM, json!({}));
    cache.package(
        "author/new",
        "1.0.0",
        ELM,
        json!({"author/base":"1.0.0 <= v < 2.0.0"}),
    );
    cache.package(
        "author/new",
        "2.0.0",
        ELM,
        json!({"author/base":"2.0.0 <= v < 3.0.0"}),
    );
    let original = application(
        json!({"author/base":"1.0.0"}),
        json!({}),
        json!({}),
        json!({}),
    );
    let updated = add_to_application(&cache.0, &original, "author/new", None).unwrap();
    assert_eq!(
        updated["dependencies"]["direct"],
        json!({"author/base":"1.0.0","author/new":"1.0.0"})
    );
}

#[test]
fn installing_can_downgrade_only_after_preserving_and_widening_attempts_fail() {
    use planexpo_elm::package_solver::add_to_application;
    let cache = Cache::new();
    cache.package("author/base", "1.0.0", ELM, json!({}));
    cache.package("author/base", "2.0.0", ELM, json!({}));
    cache.package(
        "author/new",
        "1.0.0",
        ELM,
        json!({"author/base":"1.0.0 <= v < 2.0.0"}),
    );
    let original = application(
        json!({"author/base":"2.0.0"}),
        json!({}),
        json!({}),
        json!({}),
    );
    let updated = add_to_application(&cache.0, &original, "author/new", None).unwrap();
    assert_eq!(updated["dependencies"]["direct"]["author/base"], "1.0.0");
}

#[test]
fn installing_discards_unreachable_indirect_packages_only_when_unlocking_is_needed() {
    use planexpo_elm::package_solver::add_to_application;
    let cache = Cache::new();
    cache.package(
        "author/base",
        "1.0.0",
        ELM,
        json!({"author/old":"1.0.0 <= v < 2.0.0"}),
    );
    cache.package("author/base", "1.0.1", ELM, json!({}));
    cache.package("author/old", "1.0.0", ELM, json!({}));
    cache.package(
        "author/new",
        "1.0.0",
        ELM,
        json!({"author/base":"1.0.1 <= v < 2.0.0"}),
    );
    let original = application(
        json!({"author/base":"1.0.0"}),
        json!({"author/old":"1.0.0"}),
        json!({}),
        json!({}),
    );
    let updated = add_to_application(&cache.0, &original, "author/new", None).unwrap();
    assert_eq!(updated["dependencies"]["indirect"], json!({}));
    assert_eq!(updated["test-dependencies"]["indirect"], json!({}));
}
#[test]
fn backtracks_when_a_later_dependency_contradicts_the_newest_choice() {
    let cache = Cache::new();
    cache.package(
        "author/alpha",
        "2.0.0",
        ELM,
        json!({"author/common":"2.0.0 <= v < 3.0.0"}),
    );
    cache.package(
        "author/alpha",
        "1.0.0",
        ELM,
        json!({"author/common":"1.0.0 <= v < 2.0.0"}),
    );
    cache.package(
        "author/beta",
        "1.0.0",
        ELM,
        json!({"author/common":"1.0.0 <= v < 2.0.0"}),
    );
    cache.package("author/common", "1.9.0", ELM, json!({}));
    cache.package("author/common", "1.10.0", ELM, json!({}));
    cache.package("author/common", "2.0.0", ELM, json!({}));
    let chosen = solve(
        &cache.0,
        &json!({"author/alpha":"1.0.0 <= v < 3.0.0","author/beta":"1.0.0 <= v < 2.0.0"}),
        &json!({}),
    )
    .unwrap();
    assert_eq!(chosen["author/alpha"], "1.0.0");
    assert_eq!(chosen["author/common"], "1.10.0");
}
#[test]
fn excludes_incompatible_elm_versions_and_honors_test_dependencies() {
    let cache = Cache::new();
    cache.package("author/alpha", "1.0.0", ELM, json!({}));
    cache.package("author/alpha", "2.0.0", "0.20.0 <= v < 0.21.0", json!({}));
    cache.package("author/tests", "1.0.0", ELM, json!({}));
    let chosen = solve(
        &cache.0,
        &json!({"author/alpha":"1.0.0 <= v < 3.0.0"}),
        &json!({"author/tests":"1.0.0 <= v < 2.0.0"}),
    )
    .unwrap();
    assert_eq!(chosen["author/alpha"], "1.0.0");
    assert!(chosen.contains_key("author/tests"));
    assert!(
        solve(
            &cache.0,
            &json!({"author/alpha":"2.0.0 <= v < 3.0.0"}),
            &json!({})
        )
        .is_err()
    );
}
#[test]
fn rejects_missing_sources_overlapping_test_dependencies_and_unsafe_names() {
    let cache = Cache::new();
    cache.package("author/alpha", "1.0.0", ELM, json!({}));
    let deps = json!({"author/alpha":"1.0.0 <= v < 2.0.0"});
    assert!(solve(&cache.0, &deps, &deps).is_err());
    fs::remove_dir(cache.0.join("0.19.1/packages/author/alpha/1.0.0/src")).unwrap();
    assert!(solve(&cache.0, &deps, &json!({})).is_err());
    assert!(
        solve(
            &cache.0,
            &json!({"../outside":"1.0.0 <= v < 2.0.0"}),
            &json!({})
        )
        .is_err()
    );
}
#[test]
fn constraint_bounds_and_versions_are_numeric() {
    let range = Constraint::parse("1.0.0 < v <= 2.0.0").unwrap();
    assert!(!range.contains(Version::parse("1.0.0").unwrap()));
    assert!(range.contains(Version::parse("2.0.0").unwrap()));
    assert!(!range.contains(Version::parse("2.0.1").unwrap()));
    for text in ["1.0.0", "2.0.0 <= v < 1.0.0", "1.0.0 > v < 2.0.0"] {
        assert!(Constraint::parse(text).is_err());
    }
}

#[test]
fn cached_versions_must_be_listed_in_an_existing_registry() {
    let cache = Cache::new();
    for version in ["1.0.0", "2.0.0"] {
        cache.package("author/pkg", version, ELM, json!({}));
    }
    let registry =
        planexpo_elm::registry::Registry::from_json(&json!({"author/pkg":["1.0.0"]})).unwrap();
    fs::write(
        cache.0.join("0.19.1/packages/registry.dat"),
        registry.encode().unwrap(),
    )
    .unwrap();
    let result = solve(
        &cache.0,
        &json!({"author/pkg":"1.0.0 <= v < 3.0.0"}),
        &json!({}),
    )
    .unwrap();
    assert_eq!(result["author/pkg"], "1.0.0");
    assert!(
        solve(
            &cache.0,
            &json!({"author/pkg":"2.0.0 <= v < 3.0.0"}),
            &json!({})
        )
        .is_err()
    );
}

#[test]
fn solver_uses_elm_author_project_order_when_backtracking() {
    let cache = Cache::new();
    for (package, latest_range, old_range) in [
        ("team/a", "2.0.0 <= v < 3.0.0", "1.0.0 <= v < 2.0.0"),
        ("team-extra/b", "1.0.0 <= v < 2.0.0", "2.0.0 <= v < 3.0.0"),
    ] {
        cache.package(package, "2.0.0", ELM, json!({"zz/common":latest_range}));
        cache.package(package, "1.0.0", ELM, json!({"zz/common":old_range}));
    }
    for version in ["1.0.0", "2.0.0"] {
        cache.package("zz/common", version, ELM, json!({}));
    }
    let result = solve(
        &cache.0,
        &json!({"team/a":"1.0.0 <= v < 3.0.0","team-extra/b":"1.0.0 <= v < 3.0.0"}),
        &json!({}),
    )
    .unwrap();
    assert_eq!(result["team/a"], "2.0.0");
    assert_eq!(result["team-extra/b"], "1.0.0");
}

#[test]
fn version_numbers_follow_the_official_word16_accumulator() {
    assert_eq!(
        Version::parse("65536.65537.131071").unwrap(),
        Version([0, 1, 65535])
    );
    assert!(Version::parse("01.0.0").is_err());
}

#[test]
fn constraints_require_exact_ascii_spaces_like_the_official_parser() {
    for lo in ["<", "<="] {
        for hi in ["<", "<="] {
            assert!(Constraint::parse(&format!("1.0.0 {lo} v {hi} 2.0.0")).is_ok());
        }
    }
    for text in [
        " 1.0.0 <= v < 2.0.0",
        "1.0.0 <= v < 2.0.0 ",
        "1.0.0  <= v < 2.0.0",
        "1.0.0 <=  v < 2.0.0",
        "1.0.0\t<= v < 2.0.0",
        "1.0.0 <=\nv < 2.0.0",
        "1.0.0\u{a0}<= v < 2.0.0",
        "1.0.0 <= v < 1.0.0",
    ] {
        assert!(Constraint::parse(text).is_err(), "accepted {text:?}");
    }
}

#[test]
fn offline_corrupt_metadata_fails_and_removes_only_the_manifest_even_without_sources() {
    for sources in [true, false] {
        let cache = Cache::new();
        cache.package("author/pkg", "1.0.0", ELM, json!({}));
        let root = cache.0.join("0.19.1/packages/author/pkg/1.0.0");
        fs::write(root.join("README.md"), "keep").unwrap();
        if !sources {
            fs::remove_dir(root.join("src")).unwrap();
        }
        let mut metadata: Value =
            serde_json::from_slice(&fs::read(root.join("elm.json")).unwrap()).unwrap();
        metadata.as_object_mut().unwrap().remove("license");
        fs::write(root.join("elm.json"), metadata.to_string()).unwrap();
        assert!(
            solve(
                &cache.0,
                &json!({"author/pkg":"1.0.0 <= v < 2.0.0"}),
                &json!({})
            )
            .is_err()
        );
        assert!(!root.join("elm.json").exists());
        assert_eq!(fs::read_to_string(root.join("README.md")).unwrap(), "keep");
        assert_eq!(root.join("src").exists(), sources);
    }
}
