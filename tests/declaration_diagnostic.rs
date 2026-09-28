use planexpo_elm::{declaration_diagnostic, parser};
use std::path::Path;

#[test]
fn declaration_errors_keep_keyword_ranges_and_stray_points() {
    for (body, title, width) in [
        ("", "WEIRD DECLARATION", 0),
        ("then = 1", "RESERVED WORD", 4),
        ("]", "STRAY SQUARE BRACKET", 0),
        ("123", "WEIRD DECLARATION", 0),
    ] {
        let source = format!("module Main exposing (..)\n{body}");
        assert!(
            parser::parse(&source)
                .unwrap_err()
                .starts_with("2:1: declaration start;")
        );
        let report =
            declaration_diagnostic::report(&source, "Main", Path::new("Main.elm"), 2, 1).unwrap();
        let problem = &report["errors"][0]["problems"][0];
        assert_eq!(problem["title"], title);
        assert_eq!(problem["region"]["end"]["column"], 1 + width);
        if title == "WEIRD DECLARATION" {
            assert!(problem["message"].as_array().unwrap().iter().any(|chunk| {
                chunk
                    .as_str()
                    .is_some_and(|text| text.contains("\n    \n    "))
            }));
        }
    }
}

#[test]
fn conditional_examples_and_initial_expressions_keep_their_own_advice() {
    for (body, title) in [
        ("if True then 1 else 2", "RESERVED WORD"),
        ("case 1 of", "RESERVED WORD"),
        ("[1]", "UNEXPECTED SYMBOL"),
    ] {
        let source = format!("module Main exposing (..)\n{body}");
        let report =
            declaration_diagnostic::report(&source, "Main", Path::new("Main.elm"), 2, 1).unwrap();
        let problem = &report["errors"][0]["problems"][0];
        assert_eq!(problem["title"], title);
        let message = problem["message"].to_string();
        if body.starts_with("case") {
            assert!(message.contains("BLUE"));
            assert!(message.contains("getWidth"));
        } else if body.starts_with("if") {
            assert!(message.contains("Abraham Lincoln"));
            assert!(message.contains("reviewPowerLevel"));
        } else {
            assert!(message.contains("If this is not supposed to be a declaration"));
        }
    }
}

#[test]
fn capitalized_names_suggest_one_lowercase_scalar_without_changing_the_rest() {
    for (name, expected) in [
        ("İstanbul", "istanbul"),
        ("ǅelta", "ǆelta"),
        ("NAME", "nAME"),
    ] {
        let source = format!("module Main exposing (..)\n{name} = 1");
        let report =
            declaration_diagnostic::report(&source, "Main", Path::new("Main.elm"), 2, 1).unwrap();
        let problem = &report["errors"][0]["problems"][0];
        assert_eq!(problem["title"], "UNEXPECTED CAPITAL LETTER");
        let suggestion = problem["message"]
            .as_array()
            .unwrap()
            .iter()
            .find(|chunk| chunk["color"] == "GREEN")
            .unwrap();
        assert_eq!(suggestion["string"], expected);
    }
}
