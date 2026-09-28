use planexpo_elm::{header_diagnostic, lexer, module};

#[test]
fn module_name_failures_keep_their_header_context_and_dot_position() {
    for (source, expected) in [
        ("module lower exposing (..)", "1:8: expected module name"),
        (
            "port module lower exposing (..)",
            "1:13: expected port module name",
        ),
        (
            "module Alpha. lower exposing (..)",
            "1:14: expected module name",
        ),
        ("module Alpha.", "1:14: expected module name"),
    ] {
        assert_eq!(
            module::header(source, &lexer::lex(source).unwrap()).unwrap_err(),
            expected
        );
    }
    for source in [
        "module Alpha .Beta exposing (..)",
        "module\nlower exposing (..)",
    ] {
        let error = module::header(source, &lexer::lex(source).unwrap()).unwrap_err();
        assert!(!error.ends_with("expected module name"));
    }
}

#[test]
fn module_examples_keep_official_styles_and_port_variants() {
    for port in [false, true] {
        let kind = if port {
            "expected port module name"
        } else {
            "expected module name"
        };
        let report = header_diagnostic::report(
            "module lower",
            "Main",
            std::path::Path::new("Main.elm"),
            1,
            8,
            kind,
        )
        .unwrap();
        let problem = &report["errors"][0]["problems"][0];
        assert_eq!(problem["region"]["start"], problem["region"]["end"]);
        assert_eq!(problem["title"], "EXPECTING MODULE NAME");
        let message = problem["message"].to_string();
        assert!(message.contains("CYAN"));
        assert_eq!(message.contains("WebSockets"), port);
        assert_eq!(message.contains("Html.Attributes"), !port);
    }
}

#[test]
fn header_indentation_reports_the_position_before_whitespace() {
    for (source, expected) in [
        (
            "module\nMain exposing (..)",
            "1:7: unfinished module declaration",
        ),
        (
            "module Main\nexposing (..)",
            "1:12: unfinished module declaration",
        ),
        (
            "module Main exposing\n(..)",
            "1:21: unfinished module declaration",
        ),
        (
            "port\nmodule Main exposing (..)",
            "1:5: unfinished port module declaration",
        ),
        ("port wrong", "1:6: unfinished port module declaration"),
    ] {
        assert_eq!(
            module::header(source, &lexer::lex(source).unwrap()).unwrap_err(),
            expected
        );
    }
    let source =
        "port\n    module\n    Main\n    exposing\n    (..)\nport send : String -> Cmd msg\n";
    assert!(module::header(source, &lexer::lex(source).unwrap()).is_ok());
}

#[test]
fn carriage_returns_in_layout_do_not_shift_header_errors() {
    for (source, expected) in [
        ("module\r", "1:7: expected module name"),
        ("module\n\r", "1:7: unfinished module declaration"),
        ("module {- café\r -}\r", "1:19: expected module name"),
        ("module -- café\r", "1:16: expected module name"),
        ("module \rmain", "1:8: expected module name"),
        (
            "module\n\rMain exposing (..)",
            "1:7: unfinished module declaration",
        ),
    ] {
        assert_eq!(
            module::header(source, &lexer::lex(source).unwrap()).unwrap_err(),
            expected
        );
    }
}

#[test]
fn unfinished_exposing_keeps_pre_whitespace_points_and_declaration_context() {
    for (source, expected) in [
        (
            "module Main exposing (\nvalue)",
            "1:23: exposing 1 indent-value",
        ),
        (
            "module Main exposing (value\n)",
            "1:28: exposing 1 indent-end",
        ),
        ("module Main exposing (value other)", "1:29: exposing 1 end"),
        (
            "module Main exposing (..)\nimport Basics exposing (\nidentity)",
            "2:25: exposing 2 indent-value",
        ),
    ] {
        assert_eq!(
            module::header(source, &lexer::lex(source).unwrap()).unwrap_err(),
            expected
        );
    }
    let source = "module Main exposing (\n    value\n    other)";
    let report = header_diagnostic::report(
        source,
        "Main",
        std::path::Path::new("Main.elm"),
        3,
        5,
        "exposing 1 end",
    )
    .unwrap();
    let problem = &report["errors"][0]["problems"][0];
    assert_eq!(problem["title"], "UNFINISHED EXPOSING");
    assert_eq!(problem["region"]["start"]["line"], 3);
    let message = problem["message"].to_string();
    assert!(message.contains("1| module Main"));
    assert!(message.contains("2|     value"));
    assert!(message.contains("3|     other"));
}

#[test]
fn custom_type_exposing_errors_preserve_privacy_context() {
    for (source, expected) in [
        ("module Main exposing (T(C))", "1:25: exposing 1 privacy"),
        ("module Main exposing (T(...))", "1:27: exposing 1 privacy"),
        ("module Main exposing (T(\n..))", "1:25: exposing 1 privacy"),
        ("module Main exposing (T(..\n))", "1:27: exposing 1 privacy"),
        ("module Main exposing (T(", "1:25: exposing 1 privacy"),
    ] {
        assert_eq!(
            module::header(source, &lexer::lex(source).unwrap()).unwrap_err(),
            expected
        );
    }
    let source = "module Main exposing (T(C))";
    let report = header_diagnostic::report(
        source,
        "Main",
        std::path::Path::new("Main.elm"),
        1,
        25,
        "exposing 1 privacy",
    )
    .unwrap();
    let problem = &report["errors"][0]["problems"][0];
    assert_eq!(problem["title"], "PROBLEM EXPOSING CUSTOM TYPE VARIANTS");
    let examples: Vec<_> = problem["message"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|chunk| chunk["color"] == "yellow")
        .map(|chunk| chunk["string"].as_str().unwrap())
        .collect();
    assert_eq!(examples, ["Status(..)", "Entity(..)"]);
}

#[test]
fn exposed_operator_errors_point_before_unconsumed_layout() {
    for (source, expected) in [
        ("module Main exposing (( +))", "1:24: exposing 1 operator"),
        (
            "module Main exposing ((+ ))",
            "1:25: exposing 1 operator-close",
        ),
        (
            "module Main exposing ((+\n",
            "1:25: exposing 1 operator-close",
        ),
        ("module Main exposing ((=))", "1:24: exposing 1 reserved-="),
        (
            "module Main exposing ((->))",
            "1:24: exposing 1 reserved-->",
        ),
    ] {
        assert_eq!(
            module::header(source, &lexer::lex(source).unwrap()).unwrap_err(),
            expected
        );
    }
    // Parse.Symbol.operator excludes '.', but '..' is not a reserved operator.
    let source = "module Main exposing ((..))";
    let header = module::header(source, &lexer::lex(source).unwrap()).unwrap();
    assert_eq!(
        header.exposing,
        module::Exposing::Explicit(vec![module::Exposed::Operator("..".into())])
    );
}

#[test]
fn exposed_values_distinguish_reserved_words_and_bare_operators() {
    for (source, column, title, width) in [
        ("module Main exposing value", 22, "PROBLEM IN EXPOSING", 0),
        ("module Main exposing (if)", 23, "RESERVED WORD", 2),
        ("module Main exposing (|>)", 23, "UNEXPECTED SYMBOL", 2),
        ("module Main exposing ()", 23, "PROBLEM IN EXPOSING", 0),
    ] {
        let error = module::header(source, &lexer::lex(source).unwrap()).unwrap_err();
        let kind = error.splitn(3, ':').nth(2).unwrap().trim();
        let report = header_diagnostic::report(
            source,
            "Main",
            std::path::Path::new("Main.elm"),
            1,
            column,
            kind,
        )
        .unwrap();
        let problem = &report["errors"][0]["problems"][0];
        assert_eq!(problem["title"], title);
        assert_eq!(problem["region"]["start"]["column"], column);
        assert_eq!(problem["region"]["end"]["column"], column + width);
        if title == "UNEXPECTED SYMBOL" {
            assert_eq!(problem["message"].as_array().unwrap().last().unwrap(), "");
        }
    }
}

#[test]
fn imports_distinguish_name_alias_and_indentation_errors() {
    for (body, expected) in [
        ("import basics", "2:8: import name"),
        ("import Html. Attributes", "2:13: import name"),
        ("import Html .Attributes", "2:13: import end"),
        ("import Basics as b", "2:18: import alias"),
        ("import\nBasics", "2:7: import end"),
        ("import Basics exposing\n(..)", "2:23: import exposed-list"),
        ("import Basics", "2:14: import end"),
    ] {
        let source = format!("module Main exposing (..)\n{body}");
        assert_eq!(
            module::header(&source, &lexer::lex(&source).unwrap()).unwrap_err(),
            expected
        );
    }
}
