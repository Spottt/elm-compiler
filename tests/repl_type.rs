use planexpo_elm::{parser::parse, repl_type::render, type_localizer::Localizer};
use serde_json::json;

#[test]
fn source_names_reserve_fresh_names_and_disambiguate_in_elm_order() {
    let names = Localizer::from_header(
        &parse("module Repl exposing (..)\nx = ()\n").unwrap().header,
        "application",
    );
    let graph = json!({"nodes":[["variable",0,"any"],["variable",0,"any"],["variable",0,"any"],["tuple",[0,1,2]]],"roots":[3],"variable_names":{"1":"a","2":"a"}});
    assert_eq!(render(&graph, 0, &names).unwrap(), "( b, a1, a )");
}

#[test]
fn structural_types_preserve_sharing_precedence_and_constraints() {
    let names = Localizer::from_header(
        &parse("module Repl exposing (..)\nx = ()\n").unwrap().header,
        "application",
    );
    let graph = json!({"nodes":[["variable",0,"any"],["function",0,0],["function",1,1],["variable",0,"number"],["variable",0,"number"],["tuple",[3,4,0]]],"roots":[2,5]});
    assert_eq!(render(&graph, 0, &names).unwrap(), "(a -> a) -> a -> a");
    assert_eq!(render(&graph, 1, &names).unwrap(), "( number, number1, a )");
}

#[test]
fn structural_records_flatten_rows_and_localize_named_arguments() {
    let names = Localizer::from_header(
        &parse("module Repl exposing (..)\nimport Dict as D\nx = ()\n")
            .unwrap()
            .header,
        "application",
    );
    let graph = json!({"nodes":[["unit"],["record",{"z":0},null],["record",{"a":0},1],["named","[\"elm/core:Dict\",\"Dict\"]",[0,2]],["named","[\"elm/core:List\",\"List\"]",[3]]],"roots":[4]});
    assert_eq!(
        render(&graph, 0, &names).unwrap(),
        "List (D.Dict () { a : (), z : () })"
    );
}

#[test]
fn cyclic_and_missing_type_nodes_fail_without_truncating_output() {
    let names = Localizer::from_header(
        &parse("module Repl exposing (..)\nx = ()\n").unwrap().header,
        "application",
    );
    assert!(render(&json!({"nodes":[["function",0,0]],"roots":[0]}), 0, &names).is_err());
    assert!(render(&json!({"nodes":[],"roots":[42]}), 0, &names).is_err());
}
