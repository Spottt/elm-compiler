use planexpo_elm::{lexer::lex, module::header};
#[test]
fn imports_are_not_inferred_from_comments_or_strings() {
    let s = "port module Admin.Main exposing (main, Thing(..))\n{- import Fake -}\nimport Html.Attributes as A exposing (class)\nimport Json.Decode\nmain = \"import Wrong\"";
    let ts = lex(s).unwrap();
    let h = header(s, &ts).unwrap();
    assert_eq!(h.name, "Admin.Main");
    assert_eq!(h.imports.len(), 2);
    assert_eq!(h.imports[0].name, "Html.Attributes");
    assert_eq!(h.imports[0].alias.as_deref(), Some("A"));
    assert_eq!(ts[h.body_start].text(s), "main");
}
#[test]
fn effect_header_and_missing_parentheses() {
    let s = "effect module Task where { command = MyCmd } exposing (Task, map)\nimport Platform\n";
    assert_eq!(header(s, &lex(s).unwrap()).unwrap().name, "Task");
    let bad = "module Bad exposing (x";
    assert!(header(bad, &lex(bad).unwrap()).is_err());
}

#[test]
fn retains_exposures_and_effect_manager_types() {
    use planexpo_elm::module::{Effects, Exposed, Exposing};
    let s = "effect module Fx where { subscription = MySub, command = MyCmd } exposing (T(..), f, (+))\nimport List exposing ((::))\n";
    let h = header(s, &lex(s).unwrap()).unwrap();
    assert_eq!(
        h.effects,
        Effects::Manager {
            command: Some("MyCmd".into()),
            subscription: Some("MySub".into())
        }
    );
    let Exposing::Explicit(items) = h.exposing else {
        panic!()
    };
    assert_eq!(
        items,
        vec![
            Exposed::Type {
                name: "T".into(),
                constructors: true
            },
            Exposed::Value("f".into()),
            Exposed::Operator("+".into())
        ]
    );
}
#[test]
fn rejects_invalid_exposures_and_module_names() {
    for s in [
        "module X exposing ()",
        "module X exposing (x,)",
        "module X exposing (T(C))",
        "module X exposing ((=))",
        "module X . Y exposing (..)",
        "module X exposing (..) import A",
        "module X exposing (..)\nimport A as B.C",
        "effect module X where { foo = Bar } exposing (..)",
    ] {
        assert!(header(s, &lex(s).unwrap()).is_err(), "accepted {s}");
    }
}

#[test]
fn incomplete_headers_keep_eof_or_pre_indentation_coordinates() {
    for (source, row, column) in [
        ("module", 1, 7),
        ("module État exposing (", 1, 23),
        ("module A exposing ((", 1, 21),
        // Parse.Space reports before the newline when indentation fails.
        ("module A exposing (\n", 1, 20),
    ] {
        let tokens = planexpo_elm::lexer::lex(source).unwrap();
        let error = planexpo_elm::module::header(source, &tokens).unwrap_err();
        assert!(error.starts_with(&format!("{row}:{column}:")), "{error}");
    }
}

#[test]
fn exposing_layout_and_operator_adjacency_follow_elm_grammar() {
    for listing in [
        "(\nx)",
        "(x\n)",
        "(x,\ny)",
        "(x\n, y)",
        "(\n..)",
        "(..\n)",
        "(T\n(..))",
        "(T(\n..))",
        "(T(..\n))",
        "(T(..)\n)",
        "(( +))",
        "((+ ))",
        "(({--}+))",
        "((+{--}))",
    ] {
        for prefix in [
            "module Main exposing ",
            "module Main exposing (..)\nimport Basics exposing ",
        ] {
            let source = format!("{prefix}{listing}\nvalue = 1\n");
            assert!(header(&source, &lex(&source).unwrap()).is_err(), "{source}");
        }
    }
    for listing in [
        "(\n    x\n    )",
        "(x\n    , y)",
        "(T (\n    ..\n    ))",
        "(\n    ..\n    )",
        "((+))",
        "( {- comment -} (+) {- comment -} )",
    ] {
        let source = format!("module Main exposing {listing}\nvalue = 1\n");
        assert!(header(&source, &lex(&source).unwrap()).is_ok(), "{source}");
    }
}

#[test]
fn imports_require_indented_continuations_and_fresh_line_endings() {
    for import in [
        "import\nBasics\nvalue = 1\n",
        "import Basics as\nB\nvalue = 1\n",
        "import Basics exposing\n(..)\nvalue = 1\n",
        "import Basics",
        "import Basics as B",
        "import Basics exposing (..)",
        "import Basics\n  ",
        "import Basics -- comment",
    ] {
        let source = format!("module Main exposing (..)\n{import}");
        assert!(header(&source, &lex(&source).unwrap()).is_err(), "{source}");
    }
    let source =
        "module Main exposing (..)\nimport\n    Basics\n    as B\n    exposing (..)\nvalue = 1\n";
    assert_eq!(
        header(source, &lex(source).unwrap()).unwrap().imports[0]
            .alias
            .as_deref(),
        Some("B")
    );
}

#[test]
fn exposing_wildcard_consumes_two_literal_dots_before_reporting_extra_symbols() {
    for suffix in [".", "....", "+", "->"] {
        let source = format!("module Main exposing (..{suffix})\nvalue = 1\n");
        assert_eq!(
            header(&source, &lex(&source).unwrap()).unwrap_err(),
            "1:25: exposing 1 end"
        );
        let source = format!("module Main exposing (..)\nimport Basics exposing (..{suffix})\nvalue = 1\n");
        assert_eq!(
            header(&source, &lex(&source).unwrap()).unwrap_err(),
            "2:27: exposing 2 end"
        );
    }
}

#[test]
fn effect_headers_require_indentation_between_all_components() {
    let words = ["effect", "module", "Main", "where", "{", "command", "=", "Cmd", ",", "subscription", "=", "Sub", "}", "exposing", "(..)"];
    for split in 1..words.len() {
        let source = format!("{}\n{}\nvalue = 1", words[..split].join(" "), words[split..].join(" "));
        assert!(header(&source, &lex(&source).unwrap()).is_err(), "{source}");
        let source = format!("{}\n    {}\nvalue = 1", words[..split].join(" "), words[split..].join(" "));
        assert!(header(&source, &lex(&source).unwrap()).is_ok(), "{source}");
    }
}

#[test]
fn malformed_literals_do_not_mask_header_syntax_errors() {
    for literal in ["01", "0x", "\"unfinished", "'ab'", "\"\\u{1}\"", "\"\\q\""] {
        for source in [format!("module main exposing (..)\nvalue = {literal}"), format!("module {literal}")] {
            let error = planexpo_elm::parser::parse(&source).unwrap_err();
            assert!(error.contains("expected module name"), "{source}: {error}");
        }
        let source = format!("module Main exposing (..)\nvalue = {literal}");
        let error = planexpo_elm::parser::parse(&source).unwrap_err();
        assert!(!error.contains("expected module name"), "{source}: {error}");
    }
}
