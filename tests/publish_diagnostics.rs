use planexpo_elm::{
    api_diff::Magnitude, dependency_error, package_publish::Problem, package_solver::Version,
    publish_diagnostic,
};
fn v(major: u32, minor: u32, patch: u32) -> Version {
    Version([major, minor, patch])
}
#[test]
fn version_diagnostics_preserve_actual_and_stated_versions() {
    let cases = [
        (
            Problem::NotInitialVersion(v(2, 0, 0)),
            "INVALID VERSION",
            None,
            vec!["2.0.0", "1.0.0"],
        ),
        (
            Problem::AlreadyPublished(v(1, 2, 3)),
            "ALREADY PUBLISHED",
            None,
            vec!["1.2.3", "elm bump"],
        ),
        (
            Problem::InvalidBump {
                stated: v(5, 0, 0),
                latest: v(2, 1, 0),
            },
            "INVALID VERSION",
            Some("elm.json"),
            vec!["5.0.0", "2.1.0", "elm bump"],
        ),
        (
            Problem::BadBump {
                old: v(1, 0, 0),
                stated: v(1, 0, 1),
                stated_magnitude: Magnitude::Patch,
                required: v(2, 0, 0),
                actual_magnitude: Magnitude::Major,
            },
            "INVALID VERSION",
            Some("elm.json"),
            vec!["1.0.1", "PATCH", "MAJOR", "2.0.0", "elm diff 1.0.0"],
        ),
    ];
    for (problem, title, path, fragments) in cases {
        let encoded = publish_diagnostic::problem(&problem).unwrap();
        let report = dependency_error::report_encoded(&encoded).unwrap();
        assert_eq!(report["title"], title);
        assert_eq!(report["path"].as_str(), path);
        let rendered = dependency_error::terminal_report(&report).unwrap();
        for fragment in fragments {
            assert!(rendered.contains(fragment), "{rendered}");
        }
        assert!(
            report["message"]
                .as_array()
                .unwrap()
                .iter()
                .any(|chunk| chunk["color"] == "GREEN")
        );
    }
}
#[test]
fn contextual_errors_are_not_misreported_as_package_validation() {
    for problem in [
        Problem::Io("permission denied".into()),
        Problem::InvalidDocumentation("bad json".into()),
    ] {
        assert!(publish_diagnostic::problem(&problem).is_none());
    }
    let report = dependency_error::report_encoded(&publish_diagnostic::no_outline()).unwrap();
    assert_eq!(report["title"], "PUBLISH WHAT?");
    assert!(report["path"].is_null());
}
