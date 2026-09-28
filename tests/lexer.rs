use planexpo_elm::lexer::{Kind, lex};

#[test]
fn nested_comments_preserve_layout_and_utf8_offsets() {
    let s = "{- outer {- inner -} -}\nélève = \"é\" -- ignored\n  next";
    let tokens = lex(s).unwrap();
    assert_eq!(
        tokens.iter().map(|t| t.text(s)).collect::<Vec<_>>(),
        ["élève", "=", "\"é\"", "next"]
    );
    assert_eq!((tokens[0].row, tokens[0].column), (2, 1));
    assert_eq!((tokens[1].row, tokens[1].column), (2, 7));
    assert_eq!((tokens[3].row, tokens[3].column), (3, 3));
}

#[test]
fn literals_and_operators() {
    let s = r#"x = (0xAF, 12.5e-2, -4) |> f """line
{- not comment -} "quoted""" 'é' '\u{1F600}' [glsl|void main() {}|]"#;
    let ts = lex(s).unwrap();
    assert_eq!(
        ts.iter()
            .filter(|t| t.kind == Kind::Number)
            .map(|t| t.text(s))
            .collect::<Vec<_>>(),
        ["0xAF", "12.5e-2", "4"]
    );
    assert_eq!(ts.iter().filter(|t| t.kind == Kind::Char).count(), 2);
    assert_eq!(ts.last().unwrap().kind, Kind::Shader);
}

#[test]
fn invalid_literals_are_rejected() {
    for s in [
        "{- unclosed",
        "\"unclosed",
        "'ab'",
        "'a",
        "01",
        "0x",
        "12foo",
        "1.",
        "1e+",
        "\"\\q\"",
        "\"\\u{110000}\"",
        "\tname",
    ] {
        assert!(lex(s).is_err(), "accepted {s:?}");
    }
}

#[test]
fn module_names_and_field_access_keep_dots_separate() {
    let s = "Html.Attributes.class model.name .field";
    let ts = lex(s).unwrap();
    assert_eq!(
        ts.iter().map(|t| t.text(s)).collect::<Vec<_>>(),
        [
            "Html",
            ".",
            "Attributes",
            ".",
            "class",
            "model",
            ".",
            "name",
            ".",
            "field"
        ]
    );
}

#[test]
fn identifiers_follow_elm_haskell_categories() {
    assert_eq!(lex("ǅelta").unwrap()[0].kind, Kind::Upper);
    for name in ["ª", "ⅰ", "Ⓐ", "a\u{0345}", "a\u{2160}", "\u{1C90}"] {
        assert!(lex(name).is_err(), "unexpected identifier: {name}");
    }
    for name in ["élève", "a中", "aª", "aǅ", "a_0"] {
        assert!(lex(name).is_ok(), "valid identifier: {name}");
    }
}

#[test]
fn documentation_comments_preserve_content_without_changing_tokens() {
    let source = "module Main exposing (value)\r\n{-| Résumé\r\n@docs value\n-}\n{- ordinary {-| ignored -} -}\n{-| Value {- nested -} docs. -}\nvalue = \"{-| not a doc -}\" -- {-| ignored\n";
    let (tokens, docs) = planexpo_elm::lexer::lex_with_docs(source).unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].text(source), " Résumé\r\n@docs value\n");
    assert_eq!(docs[0].normalized_text(source), " Résumé\n@docs value\n");
    assert_eq!((docs[0].row, docs[0].column), (2, 1));
    assert_eq!(docs[1].text(source), " Value {- nested -} docs. ");
    let plain = lex(source).unwrap();
    assert_eq!(
        tokens
            .iter()
            .map(|t| (t.kind, t.start, t.end, t.row, t.column))
            .collect::<Vec<_>>(),
        plain
            .iter()
            .map(|t| (t.kind, t.start, t.end, t.row, t.column))
            .collect::<Vec<_>>()
    );
}

#[test]
fn documentation_markers_in_literals_and_nested_comments_are_not_collected() {
    let source = "{- outer {-| nested -} -}\nx = \"\"\"{-| string -}\"\"\"\ny = [glsl|/* {-| shader -} */ void main() {}|]\n{-|-}";
    let (_, docs) = planexpo_elm::lexer::lex_with_docs(source).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].text(source), "");
    for source in ["{-|", "{-| {- nested -}"] {
        assert!(planexpo_elm::lexer::lex_with_docs(source).is_err());
    }
}

#[test]
fn tabs_in_block_comments_are_rejected_but_not_in_line_comments() {
    for source in [
        "{- tab\there -}\nx = 1",
        "{-| tab\there -}\nx = 1",
        "{- outer {- inner\t -} -}\nx = 1",
    ] {
        assert!(lex(source).is_err(), "{source}");
        assert!(
            planexpo_elm::lexer::lex_with_docs(source).is_err(),
            "{source}"
        );
    }
    assert!(lex("-- tab\there\nx = 1").is_ok());
    assert!(lex("x = \"tab\there\"").is_ok());
}

#[test]
fn partial_lexing_retains_the_error_and_only_complete_tokens() {
    let source = "module Broken exposing (..)\nvalue = \"unfinished";
    let (tokens, failure) = planexpo_elm::lexer::lex_prefix(source);
    assert_eq!(failure, planexpo_elm::lexer::lex(source).err());
    assert!(failure.is_some());
    assert_eq!(tokens.last().unwrap().text(source), "=");
    let complete = "module Good exposing (..)\nvalue = 1";
    let (prefix, failure) = planexpo_elm::lexer::lex_prefix(complete);
    assert!(failure.is_none());
    let full = planexpo_elm::lexer::lex(complete).unwrap();
    let fields = |tokens: &[planexpo_elm::lexer::Token]| {
        tokens
            .iter()
            .map(|t| (t.kind, t.start, t.end, t.row, t.column))
            .collect::<Vec<_>>()
    };
    assert_eq!(fields(&prefix), fields(&full));
}

#[test]
fn carriage_returns_are_zero_width_only_in_layout() {
    let source = "\rx\r =\r 1\r\n\ry = {- café\r -} z";
    let tokens = lex(source).unwrap();
    assert_eq!((tokens[0].row, tokens[0].column), (1, 1));
    assert_eq!((tokens[1].row, tokens[1].column), (1, 3));
    assert_eq!((tokens[2].row, tokens[2].column), (1, 5));
    assert_eq!((tokens[3].row, tokens[3].column), (2, 1));
    assert_eq!((tokens[5].row, tokens[5].column), (2, 17));
}

#[test]
fn unclosed_comments_point_at_outer_opening_and_bare_openers_remain_tokens() {
    for (source, expected) in [
        ("{- unclosed", "1:1: unterminated block comment"),
        (
            "\n  {- outer {- inner -}",
            "2:3: unterminated block comment",
        ),
        ("{-| documentation", "1:1: unterminated block comment"),
    ] {
        assert_eq!(lex(source).unwrap_err(), expected);
    }
    let source = "{-";
    let tokens = lex(source).unwrap();
    assert_eq!(
        tokens
            .iter()
            .map(|token| token.text(source))
            .collect::<Vec<_>>(),
        ["{", "-"]
    );
}

#[test]
fn number_errors_point_at_dot_or_exponent_sign() {
    assert!(lex("123.").unwrap_err().starts_with("1:4:"));
    assert!(lex("1e+").unwrap_err().starts_with("1:3:"));
    assert!(lex("1e-").unwrap_err().starts_with("1:3:"));
    let source = "1e2foo";
    let tokens = lex(source).unwrap();
    assert_eq!(tokens.iter().map(|t| t.text(source)).collect::<Vec<_>>(), ["1e2", "foo"]);
}

#[test]
fn quoted_errors_retain_opening_or_line_end_positions() {
    assert_eq!(lex("\"ab\n").unwrap_err(), "1:4: literal endless single");
    assert_eq!(lex("\"\"\"ab\n").unwrap_err(), "1:1: literal endless multi");
    assert_eq!(lex("'ab'").unwrap_err(), "1:1: literal char width 4");
    assert_eq!(lex("'a\n").unwrap_err(), "1:3: literal endless char");
    assert_eq!(lex("\"\\q\"").unwrap_err(), "1:2: literal unknown escape");
    assert!(lex("\"a\rb\"").is_ok());
    assert!(lex("'\r'").is_ok());
    let source = "\"\"\"a\rb\"\"\" next";
    assert_eq!(lex(source).unwrap()[1].column, 10);
}

#[test]
fn unicode_escape_errors_preserve_kind_width_and_code() {
    assert_eq!(lex("\"\\u0041\"").unwrap_err(), "1:2: literal unicode format 2");
    assert_eq!(lex("\"\\u{}\"").unwrap_err(), "1:2: literal unicode code 4");
    assert_eq!(lex("\"\\u{1}\"").unwrap_err(), "1:2: literal unicode length 5 1 1");
    assert_eq!(lex("\"\\u{110000}\"").unwrap_err(), "1:2: literal unicode code 10");
    assert_eq!(lex("\"\\u{0000000}\"").unwrap_err(), "1:2: literal unicode length 11 7 0");
    for source in ["\"\\u{0000}\"", "'\\u{D800}'", "\"\\u{10FFFF}\""] {
        assert!(lex(source).is_ok(), "{source}");
    }
}
