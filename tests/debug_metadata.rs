use planexpo_elm::{
    debug_metadata,
    names::{self, Environment, Symbols},
    parser::parse,
    types::{self, Catalog},
    unify::{Engine, Term},
};
use serde_json::{Value, json};

fn metadata(source: &str, compact: bool) -> Value {
    let mut symbols = Symbols::default();
    let builtins = types::builtins(&mut symbols);
    let mut engine = Engine::new(builtins);
    engine.track_debug_types();
    let ast = parse(source).unwrap();
    let mut environment = Environment::default();
    environment.builtin_list(&mut symbols);
    let (interface, resolved) =
        names::resolve(&ast, "author/project:Main", environment, &mut symbols).unwrap();
    let mut catalog = Catalog::default();
    catalog
        .register_type_definitions(
            &ast,
            &resolved,
            &symbols,
            "author/project:Main",
            &mut engine,
        )
        .unwrap();
    if compact {
        catalog.compact(&mut engine, &mut Default::default());
    }
    let message = engine.term(Term::Named(interface.types["Msg"], vec![]));
    debug_metadata::extract(&mut engine, &catalog, &symbols, message).unwrap()
}

#[test]
fn recursive_messages_include_transitive_aliases_but_not_unrelated_types() {
    let source = "type alias Payload a = { value : a }\ntype Chain a = End | Link a (Chain a)\ntype Msg = Got (Payload (Chain ())) | Again Msg\ntype Unused = Unused";
    let expected = json!({"versions":{"elm":"0.19.1"},"types":{
        "message":"Main.Msg",
        "aliases":{"Main.Payload":{"args":["a"],"type":"{ value : a }"}},
        "unions":{
            "Main.Msg":{"args":[],"tags":{"Got":["Main.Payload (Main.Chain ())"],"Again":["Main.Msg"]}},
            "Main.Chain":{"args":["a"],"tags":{"End":[],"Link":["a","Main.Chain a"]}}
        }
    }});
    assert_eq!(metadata(source, false), expected);
    assert_eq!(metadata(source, true), expected);
}

#[test]
fn alias_bodies_add_hidden_dependencies_and_phantom_parameters_survive() {
    let source = "type Hidden = Hidden\ntype alias Payload a = { hidden : Hidden, value : a }\ntype Phantom original = Phantom\ntype Msg = Got (Payload ()) (Phantom ())";
    let value = metadata(source, true);
    assert_eq!(
        value["types"]["aliases"]["Main.Payload"],
        json!({"args":["a"],"type":"{ hidden : Main.Hidden, value : a }"})
    );
    assert_eq!(
        value["types"]["unions"]["Main.Hidden"],
        json!({"args":[],"tags":{"Hidden":[]}})
    );
    assert_eq!(
        value["types"]["unions"]["Main.Phantom"],
        json!({"args":["original"],"tags":{"Phantom":[]}})
    );
}

#[test]
fn list_metadata_has_no_runtime_constructors_and_types_stay_on_one_line() {
    let source = "type Msg = Got (List ((), (), ())) { aVeryLongFieldNameThatUsesSpace : (), anotherLongFieldNameThatUsesSpace : (), lastLongFieldNameThatUsesSpace : () }";
    let value = metadata(source, false);
    assert_eq!(
        value["types"]["unions"]["List.List"],
        json!({"args":["a"],"tags":{}})
    );
    let fields = value["types"]["unions"]["Main.Msg"]["tags"]["Got"]
        .as_array()
        .unwrap();
    assert_eq!(fields[0], "List.List ( (), (), () )");
    assert!(fields[1].as_str().unwrap().len() > 80);
    assert!(!fields[1].as_str().unwrap().contains('\n'));
}

#[test]
fn record_order_survives_alias_substitution_and_compaction() {
    let source =
        "type alias Payload a = { z : a, a : () }\ntype Msg = Got (Payload ()) { z : (), a : () }";
    let value = metadata(source, true);
    assert_eq!(
        value["types"]["aliases"]["Main.Payload"]["type"],
        "{ z : a, a : () }"
    );
    assert_eq!(
        value["types"]["unions"]["Main.Msg"]["tags"]["Got"][1],
        "{ z : (), a : () }"
    );
}

#[test]
fn record_order_cache_roundtrip_validates_before_mutation() {
    let mut symbols = Symbols::default();
    let mut engine = Engine::new(types::builtins(&mut symbols));
    engine.track_debug_types();
    let graph = json!({"version":1,"nodes":[["unit"],["record",{"a":0,"z":0},null]],"roots":[1],"record_field_order":{"1":["z","a"]}});
    let roots = engine
        .import_types(&graph, |_| Err("unexpected symbol".into()))
        .unwrap();
    let roundtrip = engine
        .export_types(&roots, |_| Err("unexpected symbol".into()))
        .unwrap();
    let root = roundtrip["roots"][0].as_u64().unwrap().to_string();
    assert_eq!(roundtrip["record_field_order"][root], json!(["z", "a"]));
    for (node, order) in [
        ("1", json!(["a", "a"])),
        ("1", json!(["a"])),
        ("0", json!([])),
        ("99", json!([])),
    ] {
        let mut corrupt = graph.clone();
        corrupt["record_field_order"] = json!({node:order});
        let before = engine.node_count();
        assert!(
            engine
                .import_types(&corrupt, |_| Err("unexpected symbol".into()))
                .is_err()
        );
        assert_eq!(engine.node_count(), before);
    }
}
