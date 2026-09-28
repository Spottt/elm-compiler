use planexpo_elm::{dependency_error, diff_diagnostic, package_network::DocumentationError, package_solver::Version};

#[test]
fn invalid_response_preview_uses_bytes_for_threshold_and_characters_for_truncation() {
    // Reporting.Exit uses ByteString.length <= 76, but take 73 on decoded chars.
    for (body, preview, whole) in [
        (String::new(), String::new(), true),
        ("x".repeat(76), "x".repeat(76), true),
        ("x".repeat(77), format!("{}...", "x".repeat(73)), false),
        ("é".repeat(40), format!("{}...", "é".repeat(40)), false),
    ] {
        let url = "https://package.elm-lang.org/all-packages";
        for (encoded, title) in [
            (diff_diagnostic::registry_data(url, body.as_bytes()), "PROBLEM UPDATING PACKAGE LIST"),
            (diff_diagnostic::documentation(Version([1, 0, 0]), DocumentationError::InvalidData { url: url.into(), body: body.as_bytes().to_vec() }), "PROBLEM LOADING DOCS"),
        ] {
            let report = dependency_error::report_encoded(&encoded).unwrap();
            assert_eq!(report["title"], title);
            let chunks = report["message"].as_array().unwrap();
            let colored: Vec<_> = chunks.iter().filter(|chunk| chunk["color"] == "yellow").collect();
            assert_eq!(colored.len(), 2);
            assert_eq!(colored[0]["string"], url);
            assert_eq!(colored[1]["string"], preview);
            let rendered = dependency_error::terminal_report(&report).unwrap();
            assert!(rendered.contains(&format!("{} bytes", body.len())));
            assert!(rendered.contains(if whole { "whole thing:" } else { "beginning:" }));
        }
    }
}
