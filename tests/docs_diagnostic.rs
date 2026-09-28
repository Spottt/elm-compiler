use planexpo_elm::{docs, docs_diagnostic, parser::parse_with_docs};
#[test]
fn definition_reports_preserve_all_errors_and_roundtrip() {
    let source = "module Main exposing (alpha, beta)\n{-| @docs alpha, beta -}\nalpha = 1\nbeta : Int\nbeta = 2\n";
    let (ast, comments) = parse_with_docs(source).unwrap();
    let error = docs::validate(&ast, &comments).unwrap_err();
    let report =
        docs_diagnostic::definition_report(&ast, &error, std::path::Path::new("Main.elm")).unwrap();
    assert_eq!(report["errors"][0]["problems"].as_array().unwrap().len(), 2);
    assert_eq!(
        report["errors"][0]["problems"][0]["region"]["start"]["line"],
        3
    );
    assert_eq!(
        report["errors"][0]["problems"][1]["region"]["start"]["line"],
        1
    );
    assert_eq!(
        docs_diagnostic::report_encoded(&docs_diagnostic::encode(&report)),
        Some(report.clone())
    );
    let terminal = docs_diagnostic::terminal(&report);
    assert!(terminal.contains("NO TYPE ANNOTATION"));
    assert!(terminal.contains("NO DOCS"));
    assert!(!terminal.contains("ELM_DOCS_JSON"));
}
#[test]
fn operator_and_function_missing_comments_point_to_distinct_exports() {
    let source = "module Main exposing ((<+>), combine)\n{-| @docs (<+>), combine -}\ninfix left 5 (<+>) = combine\ncombine : a -> a\ncombine x = x\n";
    let (ast, comments) = parse_with_docs(source).unwrap();
    let error = docs::validate(&ast, &comments).unwrap_err();
    let report =
        docs_diagnostic::definition_report(&ast, &error, std::path::Path::new("Main.elm")).unwrap();
    let problems = report["errors"][0]["problems"].as_array().unwrap();
    assert_eq!(problems[0]["region"]["start"]["column"], 23);
    assert_eq!(problems[0]["region"]["end"]["column"], 28);
    assert_eq!(problems[1]["region"]["start"]["column"], 30);
}

#[test]
fn module_diagnostics_use_exposing_and_header_whitespace_regions() {
    for (source, title, start, end) in [
        (
            "module Main exposing (..)\nvalue = 1\n",
            "IMPLICIT EXPOSING",
            (1, 22),
            (1, 26),
        ),
        (
            "module Main exposing (value)\n{- ordinary -}\nimport String\nvalue = 1\n",
            "NO DOCS",
            (1, 29),
            (3, 1),
        ),
        (
            "module Main exposing\n    ( ..\n    )\nvalue = 1\n",
            "IMPLICIT EXPOSING",
            (2, 5),
            (3, 6),
        ),
    ] {
        let (ast, docs) = parse_with_docs(source).unwrap();
        let error = docs::validate(&ast, &docs).unwrap_err();
        let report =
            docs_diagnostic::report(&ast, &error, std::path::Path::new("Main.elm")).unwrap();
        let problem = &report["errors"][0]["problems"][0];
        assert_eq!(problem["title"], title);
        assert_eq!(
            problem["region"]["start"],
            serde_json::json!({"line":start.0,"column":start.1})
        );
        assert_eq!(
            problem["region"]["end"],
            serde_json::json!({"line":end.0,"column":end.1})
        );
    }
}

#[test]
fn list_errors_keep_missing_extra_and_duplicate_reports() {
    let source = "module Main exposing (value, other)\n{-| @docs value, value, extra -}\nvalue = 1\nother = 2\n";
    let (ast, comments) = parse_with_docs(source).unwrap();
    let error = docs::validate(&ast, &comments).unwrap_err();
    let report = docs_diagnostic::report(&ast, &error, std::path::Path::new("Main.elm")).unwrap();
    let problems = report["errors"][0]["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 3);
    assert_eq!(problems[0]["title"], "DOCS MISTAKE");
    assert_eq!(problems[1]["region"]["start"]["line"], 1);
    assert_eq!(problems[2]["title"], "DUPLICATE DOCS");
    assert_eq!(problems[2]["region"]["start"]["column"], 18);
    assert_eq!(problems[2]["region"]["end"]["column"], 23);
}

#[test]
fn syntax_errors_point_to_reserved_names_and_symbols() {
    for (overview, needle) in [
        ("@docs if", "if"),
        ("@docs (|)", "|)"),
        ("@docs (+ )", " )"),
        ("@docs value,", "-}"),
    ] {
        let source = format!("module Main exposing (value)\n{{-|{overview}-}}\nvalue = 1\n");
        let (ast, comments) = parse_with_docs(&source).unwrap();
        let error = docs::validate(&ast, &comments).unwrap_err();
        let report =
            docs_diagnostic::report(&ast, &error, std::path::Path::new("Main.elm")).unwrap();
        let problem = &report["errors"][0]["problems"][0];
        let offset = source.find(needle).unwrap();
        let column = source[..offset]
            .rsplit('\n')
            .next()
            .unwrap()
            .chars()
            .count()
            + 1;
        assert_eq!(problem["title"], "PROBLEM IN DOCS");
        assert_eq!(
            problem["region"]["start"],
            serde_json::json!({"line":2,"column":column})
        );
        assert_eq!(problem["region"]["start"], problem["region"]["end"]);
    }
}

#[test]
fn plain_terminal_uses_relative_paths_and_eighty_column_banners() {
    for path in [
        "src/Éléphant.elm".to_string(),
        format!("src/{}.elm", "x".repeat(90)),
    ] {
        let report = serde_json::json!({"errors":[{"path":format!("/project/{path}"),"problems":[
            {"title":"NO DOCS","message":["First."]},
            {"title":"DOCS MISTAKE","message":[{"string":"Second.","color":"RED"}]}
        ]}]});
        let actual = docs_diagnostic::terminal_in_root(&report, std::path::Path::new("/project"));
        let banners: Vec<_> = actual
            .lines()
            .filter(|line| line.starts_with("-- "))
            .collect();
        assert_eq!(banners.len(), 2);
        for banner in banners {
            assert!(banner.ends_with(&path));
            assert!(!banner.contains("/project/"));
            if path.len() < 80 {
                assert_eq!(banner.chars().count(), 80);
            } else {
                assert!(banner.contains(" - src/"));
            }
        }
        assert!(actual.contains("\n\nFirst.\n\n-- DOCS MISTAKE"));
        assert!(actual.ends_with("\n\nSecond.\n"));
        assert!(!actual.contains('\x1b'));
    }
}
