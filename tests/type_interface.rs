use planexpo_elm::{
    fixity, infer,
    names::{self, Environment, Symbols},
    parser::parse,
    type_interface,
    types::{self, Catalog, SourceTypes},
    unify::Engine,
};
use std::collections::BTreeMap;

fn fingerprint(source: &str) -> [u8; 32] {
    let ast = parse(source).unwrap();
    let mut symbols = Symbols::default();
    let mut engine = Engine::new(types::builtins(&mut symbols));
    let mut env = Environment::default();
    env.builtin_list(&mut symbols);
    let (interface, resolved) = names::resolve(&ast, "test:Api", env, &mut symbols).unwrap();
    let mut catalog = Catalog::default();
    catalog
        .register(&ast, &resolved, &symbols, "test:Api", &mut engine)
        .unwrap();
    let mut globals = BTreeMap::new();
    infer::check(
        SourceTypes {
            ast: &ast,
            resolved: &resolved,
            symbols: &symbols,
        },
        "test:Api",
        &catalog,
        &mut engine,
        &mut globals,
    )
    .unwrap();
    type_interface::fingerprint(
        &interface,
        &fixity::declared(&ast).unwrap(),
        &symbols,
        &catalog,
        &mut engine,
        &globals,
    )
    .unwrap()
}

#[test]
fn implementation_private_values_and_declaration_order_do_not_invalidate_public_types() {
    let original = fingerprint(
        "module Api exposing (identity, Box(..))\ntype Box a = Box a\nidentity x = x\nprivate = ()\n",
    );
    let edited = fingerprint(
        "module Api exposing (identity, Box(..))\nprivate x = (x,x)\nidentity y = (\\z -> z) y\ntype Box b = Box b\n",
    );
    assert_eq!(original, edited);
    assert_ne!(
        original,
        fingerprint(
            "module Api exposing (identity, Box(..))\ntype Box a = Box a\nidentity x = (x,x)\nprivate = ()\n"
        )
    );
}

#[test]
fn alias_body_parameter_order_and_exposure_are_part_of_interface() {
    let header = "module Api exposing (Pair)\n";
    let a = fingerprint(&format!("{header}type alias Pair a b = (a,b)\n"));
    assert_eq!(
        a,
        fingerprint(&format!("{header}type alias Pair x y = (x,y)\n"))
    );
    assert_ne!(
        a,
        fingerprint(&format!("{header}type alias Pair a b = (b,a)\n"))
    );
    assert_ne!(
        a,
        fingerprint(&format!(
            "{header}type alias Pair a b = {{ first : a, second : b }}\n"
        ))
    );
    assert_ne!(
        a,
        fingerprint("module Api exposing (Pair)\ntype Pair a b = Pair a b\n")
    );
    assert_ne!(
        fingerprint("module Api exposing (Box)\ntype Box a = Box a\n"),
        fingerprint("module Api exposing (Box(..))\ntype Box a = Box a\n")
    );
}

#[test]
fn constructor_names_arguments_and_arity_are_part_of_interface() {
    let original = fingerprint("module Api exposing (Box(..))\ntype Box a = Box a\n");
    for source in [
        "module Api exposing (Box(..))\ntype Box a = Wrap a\n",
        "module Api exposing (Box(..))\ntype Box a = Box (a,a)\n",
        "module Api exposing (Box(..))\ntype Box a = Box a a\n",
        "module Api exposing (Box(..))\ntype Box a = Box a | Empty\n",
    ] {
        assert_ne!(original, fingerprint(source));
    }
}

#[test]
fn operator_precedence_associativity_and_signature_are_part_of_interface() {
    let source = "module Api exposing ((|=))\ninfix left 4 (|=) = choose\nchoose a b = a\n";
    let original = fingerprint(source);
    assert_ne!(original, fingerprint(&source.replace("left 4", "left 5")));
    assert_ne!(original, fingerprint(&source.replace("left 4", "right 4")));
    assert_ne!(original, fingerprint(&source.replace("= a\n", "= b\n")));
    assert_eq!(original, fingerprint(&source.replace("choose", "select")));
}

#[test]
fn inference_of_private_record_access_does_not_change_constructor_interface() {
    let source = "module Api exposing (Box(..))\ntype Box = Box { field : (), other : () }\n";
    assert_eq!(
        fingerprint(source),
        fingerprint(&format!("{source}private (Box r) = r.field\n"))
    );
}

#[test]
fn pruning_private_globals_preserves_polymorphic_exports_and_operator_implementations() {
    use planexpo_elm::{
        module::Exposing,
        names::{Space, SymbolKind},
    };
    let ast = parse("module Api exposing (identity, (|=), Box(..))\ninfix left 4 (|=) = choose\ntype Box a = Box a\nidentity x = x\nchoose a b = a\nprivate x = (x,x)\n").unwrap();
    let mut symbols = Symbols::default();
    let mut engine = Engine::new(types::builtins(&mut symbols));
    let mut env = Environment::default();
    env.builtin_list(&mut symbols);
    let (interface, resolved) = names::resolve(&ast, "test:Api", env, &mut symbols).unwrap();
    let mut catalog = Catalog::default();
    catalog
        .register(&ast, &resolved, &symbols, "test:Api", &mut engine)
        .unwrap();
    let mut globals = BTreeMap::new();
    infer::check(
        SourceTypes {
            ast: &ast,
            resolved: &resolved,
            symbols: &symbols,
        },
        "test:Api",
        &catalog,
        &mut engine,
        &mut globals,
    )
    .unwrap();
    let private = symbols.lookup("test:Api", "private", Space::Value).unwrap();
    let foreign = symbols.intern("test:Other", "value", Space::Value, SymbolKind::Value);
    globals.insert(foreign, globals[&interface.values["identity"]].clone());
    assert!(globals.contains_key(&private));
    let operators = fixity::declared(&ast).unwrap();
    let before = type_interface::fingerprint(
        &interface,
        &operators,
        &symbols,
        &catalog,
        &mut engine,
        &globals,
    )
    .unwrap();
    type_interface::retain_public_globals("test:Api", &interface, &symbols, &mut globals);
    assert!(!globals.contains_key(&private));
    assert!(globals.contains_key(&foreign));
    assert!(globals.contains_key(&interface.operators["|="]));
    catalog.compact(&mut engine, &mut globals);
    assert_eq!(
        before,
        type_interface::fingerprint(
            &interface,
            &operators,
            &symbols,
            &catalog,
            &mut engine,
            &globals
        )
        .unwrap()
    );
    let mut client =
        parse("first = Api.identity ()\nsecond = Api.identity []\nselected = Api.Box () |= []\n")
            .unwrap();
    fixity::resolve(&mut client, &operators).unwrap();
    let mut env = Environment::default();
    env.builtin_list(&mut symbols);
    env.import("Api", &interface, &Exposing::All, &symbols)
        .unwrap();
    let (_, resolved) = names::resolve(&client, "test:Client", env, &mut symbols).unwrap();
    infer::check(
        SourceTypes {
            ast: &client,
            resolved: &resolved,
            symbols: &symbols,
        },
        "test:Client",
        &catalog,
        &mut engine,
        &mut globals,
    )
    .unwrap();
}
