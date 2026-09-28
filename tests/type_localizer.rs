use planexpo_elm::{parser::parse, type_localizer::Localizer};

fn localizer(source: &str) -> Localizer {
    Localizer::from_header(&parse(source).unwrap().header, "application")
}

#[test]
fn types_follow_import_exposure_and_aliases() {
    let names = localizer(
        "module Repl exposing (..)\nimport Json.Decode as D exposing (Decoder)\nimport Dict as Maps\nimport Set exposing (..)\nvalue = ()\n",
    );
    assert_eq!(names.name("elm/json:Json.Decode", "Decoder"), "Decoder");
    assert_eq!(names.name("elm/json:Json.Decode", "Error"), "D.Error");
    assert_eq!(names.name("elm/core:Dict", "Dict"), "Maps.Dict");
    assert_eq!(names.name("elm/core:Set", "Set"), "Set");
    assert_eq!(names.name("application:Repl", "Private"), "Private");
    assert_eq!(names.name("author/pkg:Other", "Type"), "Other.Type");
}

#[test]
fn explicit_imports_replace_default_exposure_for_display() {
    let names =
        localizer("module Repl exposing (..)\nimport Maybe as M\nimport List as L\nvalue = ()\n");
    assert_eq!(names.name("elm/core:Maybe", "Maybe"), "M.Maybe");
    assert_eq!(names.name("elm/core:List", "List"), "List");
    assert_eq!(names.name("author/pkg:List", "List"), "L.List");
    assert_eq!(names.name("elm/core:Basics", "Int"), "Int");
    assert_eq!(names.name("elm/core:Platform.Cmd", "Cmd"), "Cmd");
    assert_eq!(names.name("elm/core:Platform.Cmd", "Other"), "Cmd.Other");
}

#[test]
fn only_type_exposures_shorten_names_and_core_has_no_defaults() {
    let ast =
        parse("module Repl exposing (..)\nimport Thing as T exposing (value, Kind(..))\nx = ()\n")
            .unwrap();
    let names = Localizer::from_header(&ast.header, "elm/core");
    assert_eq!(names.name("author/pkg:Thing", "Kind"), "Kind");
    assert_eq!(names.name("author/pkg:Thing", "value"), "T.value");
    assert_eq!(names.name("elm/core:Basics", "Int"), "Basics.Int");
}
