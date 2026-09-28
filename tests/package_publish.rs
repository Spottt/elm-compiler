use planexpo_elm::{
    api_diff::Magnitude,
    package_bump::Candidate,
    package_publish::{self as publish, Problem, VersionPlan},
    package_solver::Version,
};
use serde_json::{Value, json};
fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}
fn outline() -> Value {
    json!({"type":"package", "summary":"Useful tools", "exposed-modules":["Example"]})
}
fn docs(values: Value) -> Value {
    json!([{"name":"Example","comment":"","unions":[],"aliases":[],"values":values,"binops":[]}])
}

#[test]
fn description_checks_follow_official_priority_and_default_summary() {
    let mut p = outline();
    p["type"] = json!("application");
    p["exposed-modules"] = json!([]);
    p["summary"] = json!("");
    assert_eq!(publish::check_description(&p), Err(Problem::Application));
    p["type"] = json!("package");
    assert_eq!(
        publish::check_description(&p),
        Err(Problem::NoExposedModules)
    );
    p["exposed-modules"] = json!({"Empty":[],"API":["Example"]});
    for summary in [
        "",
        "helpful summary of your project, less than 80 characters",
    ] {
        p["summary"] = json!(summary);
        assert_eq!(publish::check_description(&p), Err(Problem::NoSummary));
    }
    for summary in [
        " ",
        "A helpful summary of your project, less than 80 characters",
        "Useful tools",
    ] {
        p["summary"] = json!(summary);
        assert_eq!(publish::check_description(&p), Ok(()));
    }
    for exposed in [json!([]), json!({}), json!({"Empty":[]})] {
        p["exposed-modules"] = exposed;
        assert_eq!(
            publish::check_description(&p),
            Err(Problem::NoExposedModules)
        );
    }
}

#[test]
fn readme_uses_byte_size_and_license_may_be_empty() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert_eq!(publish::check_readme(root), Err(Problem::NoReadme));
    assert_eq!(publish::check_license(root), Err(Problem::NoLicense));
    // Elm uses the host filesystem's case semantics (macOS/Windows commonly
    // resolve readme.md and README.md to the same file).
    std::fs::write(root.join("case-probe"), "").unwrap();
    let case_insensitive = root.join("CASE-PROBE").exists();
    std::fs::write(root.join("readme.md"), "x".repeat(300)).unwrap();
    assert_eq!(
        publish::check_readme(root),
        if case_insensitive { Ok(()) } else { Err(Problem::NoReadme) }
    );
    for (contents, result) in [
        ("x".repeat(299), Err(Problem::ShortReadme)),
        ("é".repeat(150), Ok(())),
        ("x".repeat(301), Ok(())),
    ] {
        std::fs::write(root.join("README.md"), contents).unwrap();
        assert_eq!(publish::check_readme(root), result);
    }
    std::fs::write(root.join("LICENSE"), "").unwrap();
    assert_eq!(publish::check_license(root), Ok(()));
}

#[test]
fn directories_are_not_publication_documents() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("README.md")).unwrap();
    std::fs::create_dir(dir.path().join("LICENSE")).unwrap();
    assert_eq!(publish::check_readme(dir.path()), Err(Problem::NoReadme));
    assert_eq!(publish::check_license(dir.path()), Err(Problem::NoLicense));
}

#[test]
fn initial_existing_and_skipped_versions_are_distinct() {
    assert_eq!(
        publish::version_plan(v("1.0.0"), &[]),
        Ok(VersionPlan::Initial)
    );
    for s in ["0.1.0", "1.0.1", "2.0.0"] {
        assert_eq!(
            publish::version_plan(v(s), &[]),
            Err(Problem::NotInitialVersion(v(s)))
        );
    }
    let published = [v("2.0.0"), v("1.0.0"), v("1.0.1")];
    assert_eq!(
        publish::version_plan(v("1.0.0"), &published),
        Err(Problem::AlreadyPublished(v("1.0.0")))
    );
    assert_eq!(
        publish::version_plan(v("1.0.3"), &published),
        Err(Problem::InvalidBump {
            stated: v("1.0.3"),
            latest: v("2.0.0")
        })
    );
    for (new, old, magnitude) in [
        ("1.0.2", "1.0.1", Magnitude::Patch),
        ("1.1.0", "1.0.1", Magnitude::Minor),
        ("3.0.0", "2.0.0", Magnitude::Major),
    ] {
        assert_eq!(
            publish::version_plan(v(new), &published),
            Ok(VersionPlan::Compare(Candidate {
                old: v(old),
                new: v(new),
                magnitude
            }))
        );
    }
}

#[test]
fn api_changes_require_exact_magnitude_even_for_overstated_versions() {
    let old = docs(json!([{"name":"identity","comment":"","type":"a -> a"}]));
    let mut added = old.clone();
    added[0]["values"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"answer","comment":"","type":"Int"}));
    let removed = docs(json!([]));
    for (new_docs, required, actual) in [
        (&old, "1.0.1", Magnitude::Patch),
        (&added, "1.1.0", Magnitude::Minor),
        (&removed, "2.0.0", Magnitude::Major),
    ] {
        for (stated, magnitude) in [
            ("1.0.1", Magnitude::Patch),
            ("1.1.0", Magnitude::Minor),
            ("2.0.0", Magnitude::Major),
        ] {
            let candidate = Candidate {
                old: v("1.0.0"),
                new: v(stated),
                magnitude,
            };
            let expected = if stated == required {
                Ok(())
            } else {
                Err(Problem::BadBump {
                    old: candidate.old,
                    stated: candidate.new,
                    stated_magnitude: magnitude,
                    required: v(required),
                    actual_magnitude: actual,
                })
            };
            assert_eq!(publish::check_api(&candidate, &old, new_docs), expected);
        }
    }
    let candidate = Candidate {
        old: v("1.0.0"),
        new: v("1.0.1"),
        magnitude: Magnitude::Patch,
    };
    assert!(matches!(
        publish::check_api(&candidate, &json!({}), &old),
        Err(Problem::InvalidDocumentation(_))
    ));
}
