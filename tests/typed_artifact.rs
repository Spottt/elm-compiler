use planexpo_elm::{
    infer,
    module::Exposing,
    names::{self, Environment, Interface, Space, SymbolKind, Symbols},
    parser::parse,
    typed_artifact,
    types::{self, Catalog, SourceTypes},
    unify::{Engine, Scheme},
};
use std::collections::BTreeMap;
const SOURCE: &str = "module Cache exposing (..)\ntype Box a = Box a\nidentity x = x\nget r = r.value\nwrap x = Box x\n";
fn setup(noise: bool) -> (Symbols, Engine, Catalog, Interface) {
    let ast = parse(SOURCE).unwrap();
    let mut symbols = Symbols::default();
    if noise {
        symbols.intern(
            "other:Noise",
            "Value",
            Space::Type,
            SymbolKind::Type {
                arity: 0,
                alias: false,
            },
        );
    }
    let mut engine = Engine::new(types::builtins(&mut symbols));
    let mut env = Environment::default();
    env.builtin_list(&mut symbols);
    let (interface, resolved) = names::resolve(&ast, "test:Cache", env, &mut symbols).unwrap();
    let mut catalog = Catalog::default();
    catalog
        .register(&ast, &resolved, &symbols, "test:Cache", &mut engine)
        .unwrap();
    (symbols, engine, catalog, interface)
}
fn artifact() -> serde_json::Value {
    let (mut symbols, mut engine, catalog, _) = setup(false);
    let ast = parse(SOURCE).unwrap();
    let mut env = Environment::default();
    env.builtin_list(&mut symbols);
    let (_, resolved) = names::resolve(&ast, "test:Cache", env, &mut symbols).unwrap();
    let mut globals = BTreeMap::new();
    infer::check(
        SourceTypes {
            ast: &ast,
            resolved: &resolved,
            symbols: &symbols,
        },
        "test:Cache",
        &catalog,
        &mut engine,
        &mut globals,
    )
    .unwrap();
    let value = typed_artifact::export("test:Cache", &symbols, &mut engine, &globals).unwrap();
    serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap()
}
#[test]
fn restored_module_has_the_same_public_interface_fingerprint() {
    let artifact = artifact();
    let mut fingerprints = Vec::new();
    for noise in [false, true] {
        let (mut symbols, mut engine, catalog, interface) = setup(noise);
        let globals = if noise {
            typed_artifact::import(&artifact, "test:Cache", &symbols, &mut engine).unwrap()
        } else {
            let ast = parse(SOURCE).unwrap();
            let mut env = Environment::default();
            env.builtin_list(&mut symbols);
            let (_, resolved) = names::resolve(&ast, "test:Cache", env, &mut symbols).unwrap();
            let mut globals = BTreeMap::new();
            infer::check(
                SourceTypes {
                    ast: &ast,
                    resolved: &resolved,
                    symbols: &symbols,
                },
                "test:Cache",
                &catalog,
                &mut engine,
                &mut globals,
            )
            .unwrap();
            globals
        };
        fingerprints.push(
            planexpo_elm::type_interface::fingerprint(
                &interface,
                &BTreeMap::new(),
                &symbols,
                &catalog,
                &mut engine,
                &globals,
            )
            .unwrap(),
        );
    }
    assert_eq!(fingerprints[0], fingerprints[1]);
}
#[test]
fn cached_signatures_typecheck_real_dependents_after_symbol_reordering() {
    let artifact = artifact();
    for (source, valid) in [
        (
            "x = (Cache.identity (), Cache.identity [])\ny = Cache.get {value=Cache.wrap (), extra=[]}",
            true,
        ),
        ("x = Cache.get {other=()}", false),
        ("x = (Cache.identity ()) ()", false),
    ] {
        let (mut symbols, mut engine, catalog, interface) = setup(true);
        let mut globals =
            typed_artifact::import(&artifact, "test:Cache", &symbols, &mut engine).unwrap();
        assert_eq!(globals.len(), 3);
        let ast = parse(source).unwrap();
        let mut env = Environment::default();
        env.builtin_list(&mut symbols);
        env.import("Cache", &interface, &Exposing::All, &symbols)
            .unwrap();
        let (_, resolved) = names::resolve(&ast, "test:Client", env, &mut symbols).unwrap();
        let result = infer::check(
            SourceTypes {
                ast: &ast,
                resolved: &resolved,
                symbols: &symbols,
            },
            "test:Client",
            &catalog,
            &mut engine,
            &mut globals,
        );
        assert_eq!(result.is_ok(), valid, "{source}: {:?}", result.err());
    }
}
#[test]
fn corrupt_module_metadata_is_rejected_before_mutating_types() {
    let artifact = artifact();
    for change in 0..6 {
        let mut bad = artifact.clone();
        match change {
            0 => bad["module"] = serde_json::json!("other:Cache"),
            1 => bad["entries"][0]["name"] = serde_json::json!("missing"),
            2 => bad["entries"][0]["root"] = serde_json::json!(999999),
            3 => bad["entries"][0]["quantified"] = serde_json::json!([999999]),
            4 => {
                let duplicate = bad["entries"][0].clone();
                bad["entries"].as_array_mut().unwrap().push(duplicate);
            }
            _ => {
                bad["entries"].as_array_mut().unwrap().pop();
            }
        }
        let (symbols, mut engine, _, _) = setup(true);
        let before = engine.node_count();
        assert!(typed_artifact::import(&bad, "test:Cache", &symbols, &mut engine).is_err());
        assert_eq!(engine.node_count(), before);
    }
}
#[test]
fn artifacts_exclude_globals_from_other_modules() {
    let (mut symbols, mut engine, _, _) = setup(false);
    let id = symbols.intern("other:Module", "secret", Space::Value, SymbolKind::Value);
    let root = engine.variable(1, planexpo_elm::unify::Constraint::Any);
    let globals = BTreeMap::from([(
        id,
        Scheme {
            root,
            quantified: Default::default(),
        },
    )]);
    let artifact = typed_artifact::export("test:Cache", &symbols, &mut engine, &globals).unwrap();
    assert!(artifact["entries"].as_array().unwrap().is_empty());
    assert!(artifact["graph"]["nodes"].as_array().unwrap().is_empty());
}

#[test]
fn open_schemes_are_not_detached_from_their_original_environment() {
    let (symbols, mut engine, _, _) = setup(false);
    let id = symbols
        .lookup("test:Cache", "identity", Space::Value)
        .unwrap();
    let root = engine.variable(0, planexpo_elm::unify::Constraint::Any);
    let globals = BTreeMap::from([(
        id,
        Scheme {
            root,
            quantified: Default::default(),
        },
    )]);
    assert!(typed_artifact::export("test:Cache", &symbols, &mut engine, &globals).is_err());
}

#[test]
fn selected_artifacts_omit_private_schemes_and_validate_the_exact_required_set() {
    let full = artifact();
    let (symbols, mut engine, _, _) = setup(false);
    let globals = typed_artifact::import(&full, "test:Cache", &symbols, &mut engine).unwrap();
    let identity = symbols
        .lookup("test:Cache", "identity", Space::Value)
        .unwrap();
    let wrap = symbols.lookup("test:Cache", "wrap", Space::Value).unwrap();
    let required = std::collections::BTreeSet::from([identity, wrap]);
    let selected =
        typed_artifact::export_selected("test:Cache", &symbols, &mut engine, &globals, &required)
            .unwrap();
    assert_eq!(selected["entries"].as_array().unwrap().len(), 2);
    assert!(
        selected["graph"]["nodes"].as_array().unwrap().len()
            < full["graph"]["nodes"].as_array().unwrap().len()
    );
    let restored =
        typed_artifact::import_selected(&selected, "test:Cache", &symbols, &mut engine, &required)
            .unwrap();
    assert_eq!(
        restored
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        required
    );
    for invalid in [
        &full,
        &serde_json::json!({"version":1,"module":"test:Cache","entries":[],"graph":selected["graph"]}),
    ] {
        let count = engine.node_count();
        assert!(
            typed_artifact::import_selected(
                invalid,
                "test:Cache",
                &symbols,
                &mut engine,
                &required
            )
            .is_err()
        );
        assert_eq!(engine.node_count(), count);
    }
    let missing = BTreeMap::from([(identity, globals[&identity].clone())]);
    assert!(
        typed_artifact::export_selected("test:Cache", &symbols, &mut engine, &missing, &required)
            .is_err()
    );
    let private = symbols.lookup("test:Cache", "get", Space::Value).unwrap();
    let mut private_open = globals.clone();
    private_open.insert(
        private,
        Scheme {
            root: engine.variable(0, planexpo_elm::unify::Constraint::Any),
            quantified: Default::default(),
        },
    );
    assert!(typed_artifact::export("test:Cache", &symbols, &mut engine, &private_open).is_err());
    assert!(
        typed_artifact::export_selected(
            "test:Cache",
            &symbols,
            &mut engine,
            &private_open,
            &required
        )
        .is_ok()
    );
}

#[test]
fn selection_keeps_unexposed_main_ports_and_operator_implementations() {
    let ast = parse("port module Cache exposing ((|=))\ninfix left 4 (|=) = choose\nchoose x y = x\nmain = ()\nprivate = ()\nport outgoing : ()\n").unwrap();
    let mut symbols = Symbols::default();
    let (interface, _) =
        names::resolve(&ast, "test:Cache", Environment::default(), &mut symbols).unwrap();
    let required =
        typed_artifact::required_globals("test:Cache", &ast, &interface, &symbols).unwrap();
    let names: std::collections::BTreeSet<_> = required
        .iter()
        .map(|id| symbols.get(*id).name.as_ref())
        .collect();
    assert_eq!(
        names,
        std::collections::BTreeSet::from(["choose", "main", "outgoing"])
    );
}
