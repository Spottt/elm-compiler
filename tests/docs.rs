use planexpo_elm::{
    docs::{Error, NameProblem, parse_overview, validate_names},
    lexer::lex_with_docs,
    parser::parse_with_docs,
};

fn names(text: &str) -> Result<Vec<String>, Error> {
    let source = format!("{{-|{text}-}}");
    let (_, comments) = lex_with_docs(&source).map_err(|_| Error::Syntax {
        offset: 0,
        column: 0,
        message: "invalid documentation comment",
    })?;
    Ok(parse_overview(&source, comments[0])?
        .into_iter()
        .map(|n| n.name.to_string())
        .collect())
}

#[test]
fn overview_finds_inline_markers_but_respects_identifier_boundaries() {
    assert_eq!(
        names("prose @docs value, Choice\nmore @docs other\n").unwrap(),
        ["value", "Choice", "other"]
    );
    assert_eq!(
        names("@docsValue @docs_ @docs2 @docsé\n@docs value").unwrap(),
        ["value"]
    );
}

#[test]
fn overview_accepts_unicode_names_operators_and_nested_spacing() {
    assert_eq!(
        names("@docs café, État, (|>), (..)\n").unwrap(),
        ["café", "État", "|>", ".."]
    );
    assert_eq!(
        names("@docs {- nested {- block -} -} value, -- text\n other\n").unwrap(),
        ["value", "other"]
    );
}

#[test]
fn comma_at_column_one_is_prose_but_indented_comma_continues() {
    assert_eq!(names("@docs value\n, other\n").unwrap(), ["value"]);
    assert_eq!(
        names("@docs value\n , other\n").unwrap(),
        ["value", "other"]
    );
}

#[test]
fn malformed_docs_names_and_spacing_are_rejected() {
    assert!(names("@docs² ignored\n@docs value").is_err());
    for text in [
        "@docs",
        "@docs value,",
        "@docs if",
        "@docs _value",
        "@docs (+ )",
        "@docs (|)",
        "@docs (->)",
        "@docs (.)",
        "@docs (=)",
        "@docs (:)",
        "@docs\tvalue",
    ] {
        assert!(names(text).is_err(), "{text}");
    }
}

#[test]
fn name_spans_are_utf8_offsets_in_original_source() {
    let source = "module Main exposing (café)\n{-| 😀 @docs café -}\ncafé = 1\n";
    let (_, comments) = lex_with_docs(source).unwrap();
    let names = parse_overview(source, comments[0]).unwrap();
    assert_eq!(
        &source[names[0].span.start as usize..names[0].span.end as usize],
        "café"
    );
}

#[test]
fn checks_missing_extra_and_duplicate_names_together() {
    let source = "module Main exposing (value, other)\n{-| @docs value, value, extra -}\nvalue = 1\nother = 2\n";
    let (ast, docs) = parse_with_docs(source).unwrap();
    let Error::Names(problems) = validate_names(&ast, &docs).unwrap_err() else {
        panic!()
    };
    assert!(matches!(&problems[0], NameProblem::OnlyInDocs { name, .. } if name == "extra"));
    assert!(matches!(&problems[1], NameProblem::OnlyInExports { name } if name == "other"));
    assert!(
        matches!(&problems[2], NameProblem::Duplicate { name, spans, .. } if name == "value" && spans.len() == 2)
    );
}

#[test]
fn explicit_exports_and_module_overview_are_required() {
    for (source, implicit) in [
        ("module Main exposing (..)\nvalue = 1\n", true),
        ("module Main exposing (value)\nvalue = 1\n", false),
    ] {
        let (ast, docs) = parse_with_docs(source).unwrap();
        assert!(matches!(validate_names(&ast, &docs), Err(Error::ImplicitExposing)) == implicit);
        if !implicit {
            assert!(matches!(
                validate_names(&ast, &docs),
                Err(Error::MissingOverview)
            ));
        }
    }
    let source = "module Main exposing (value, Choice(..))\n{-| @docs Choice, value -}\nvalue = 1\ntype Choice = A\n";
    let (ast, docs) = parse_with_docs(source).unwrap();
    assert!(validate_names(&ast, &docs).is_ok());
}

fn definitions(source: &str) -> Result<Vec<planexpo_elm::docs::DocumentedExport>, Error> {
    let (ast, docs) = parse_with_docs(source).unwrap();
    planexpo_elm::docs::validate(&ast, &docs)
}

#[test]
fn exported_values_require_annotations_before_comments() {
    use planexpo_elm::docs::DefinitionProblem;
    let header = "module Main exposing (value)\n{-| @docs value -}\nimport String\n";
    for (body, expected) in [
        (
            "value = 1\n",
            DefinitionProblem::NoAnnotation {
                name: "value".into(),
            },
        ),
        (
            "{-| Value. -}\nvalue = 1\n",
            DefinitionProblem::NoAnnotation {
                name: "value".into(),
            },
        ),
        (
            "value : Int\nvalue = 1\n",
            DefinitionProblem::NoComment {
                name: "value".into(),
            },
        ),
    ] {
        assert_eq!(
            definitions(&format!("{header}{body}")),
            Err(Error::Definitions(vec![expected]))
        );
    }
    let exports = definitions(&format!("{header}{{-|-}}\nvalue : Int\nvalue = 1\n")).unwrap();
    assert_eq!(exports.len(), 1);
    assert!(exports[0].annotation.is_some());
}

#[test]
fn private_declarations_do_not_require_docs_or_annotations() {
    let source = "module Main exposing (value)\n{-| @docs value -}\n{-| Value. -}\nvalue : Int\nvalue = private\nprivate = 1\ntype Hidden = Hidden\n";
    let exports = definitions(source).unwrap();
    assert_eq!(
        exports.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        ["value"]
    );
}

#[test]
fn aliases_and_unions_require_comments_but_no_value_annotation() {
    use planexpo_elm::docs::DefinitionProblem;
    let source = "module Main exposing (Choice(..), Row)\n{-| @docs Choice, Row -}\nimport String\ntype Choice = A | B\ntype alias Row = { x : Int }\n";
    assert_eq!(
        definitions(source),
        Err(Error::Definitions(vec![
            DefinitionProblem::NoComment {
                name: "Choice".into()
            },
            DefinitionProblem::NoComment { name: "Row".into() }
        ]))
    );
    let source = source
        .replace("type Choice", "{-| Choice. -}\ntype Choice")
        .replace("type alias Row", "{-| Row. -}\ntype alias Row");
    let exports = definitions(&source).unwrap();
    assert!(exports[0].constructors);
    assert!(exports.iter().all(|e| e.annotation.is_none()));
}

#[test]
fn operators_use_the_implementation_annotation_and_comment() {
    use planexpo_elm::docs::DefinitionProblem;
    let source = "module Main exposing ((<+>))\n{-| @docs (<+>) -}\ninfix left 5 (<+>) = combine\n{-| Adds. -}\ncombine : Int -> Int -> Int\ncombine a b = a + b\n";
    let exports = definitions(source).unwrap();
    assert_eq!(exports[0].name, "<+>");
    assert_eq!(exports[0].comment.text(source), " Adds. ");
    assert!(exports[0].annotation.is_some());
    assert_eq!(
        definitions(&source.replace("combine : Int -> Int -> Int\n", "")),
        Err(Error::Definitions(vec![DefinitionProblem::NoAnnotation {
            name: "combine".into()
        }]))
    );
    assert_eq!(
        definitions(&source.replace("{-| Adds. -}\n", "")),
        Err(Error::Definitions(vec![DefinitionProblem::NoComment {
            name: "combine".into()
        }]))
    );
}

#[test]
fn ports_have_an_annotation_and_still_need_a_comment() {
    let source = "port module Main exposing (send)\n{-| @docs send -}\n{-| Sends. -}\nport send : String -> Cmd msg\n";
    assert!(definitions(source).unwrap()[0].annotation.is_some());
    assert!(matches!(
        definitions(&source.replace("{-| Sends. -}\n", "")),
        Err(Error::Definitions(_))
    ));
}

#[test]
fn name_validation_precedes_definition_validation() {
    assert!(matches!(
        definitions("module Main exposing (value)\n{-| prose -}\nvalue = 1\n"),
        Err(Error::Names(_))
    ));
}

#[test]
fn names_preserve_columns_after_ignored_carriage_returns() {
    let source = "{-|@docs\r value,\r café, (<+>)-}";
    let (_, comments) = lex_with_docs(source).unwrap();
    let parsed = parse_overview(source, comments[0]).unwrap();
    assert_eq!(
        parsed.iter().map(|n| n.column).collect::<Vec<_>>(),
        [10, 17, 23]
    );
    assert_eq!(
        parsed.iter().map(|n| n.name).collect::<Vec<_>>(),
        ["value", "café", "<+>"]
    );
}
