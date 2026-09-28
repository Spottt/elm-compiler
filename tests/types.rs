use planexpo_elm::{
    ast::Syntax,
    module::Exposing,
    names::{self, Environment, Interface, Resolved, Space, SymbolId, Symbols},
    parser::parse,
    types::{self, Catalog},
    unify::{Builtins, Engine, Term, Ty},
};
fn setup() -> (Symbols, Engine, Builtins) {
    let mut symbols = Symbols::default();
    let builtins = types::builtins(&mut symbols);
    (symbols, Engine::new(builtins), builtins)
}
fn resolve(
    ast: &Syntax<'_>,
    module: &str,
    symbols: &mut Symbols,
    builtins: Builtins,
) -> (Interface, Resolved) {
    let mut env = Environment::default();
    env.builtin_list(symbols);
    let core = Interface {
        types: [
            ("Int".into(), builtins.int),
            ("String".into(), builtins.string),
        ]
        .into(),
        ..Default::default()
    };
    env.import("Core", &core, &Exposing::All, symbols).unwrap();
    names::resolve(ast, module, env, symbols).unwrap()
}
fn function(e: &mut Engine, root: Ty) -> (Ty, Ty) {
    let t = e.structure(root).unwrap();
    let Term::Function(a, b) = &*t else {
        panic!("not a function: {t:?}")
    };
    (*a, *b)
}
fn named(e: &mut Engine, root: Ty, expected: SymbolId) {
    let t = e.structure(root).unwrap();
    assert!(matches!(&*t, Term::Named(id,_) if *id == expected), "{t:?}");
}
#[test]
fn catalog_compaction_keeps_alias_parameters_constructors_and_global_schemes() {
    let (mut symbols, mut engine, builtins) = setup();
    let ast =
        parse("type alias Wrapper a = { value : a }\ntype Box a = Box a\nid : b -> b\nid x = x")
            .unwrap();
    let (interface, resolved) = resolve(&ast, "test:Main", &mut symbols, builtins);
    let mut catalog = Catalog::default();
    let declarations = catalog
        .register(&ast, &resolved, &symbols, "test:Main", &mut engine)
        .unwrap();
    let mut globals = declarations.annotations;
    for _ in 0..1000 {
        engine.term(Term::Unit);
    }
    catalog.compact(&mut engine, &mut globals);
    assert!(engine.node_count() < 20);
    let wrapper = &catalog.aliases[&interface.types["Wrapper"]];
    let record = engine.structure(wrapper.root).unwrap();
    let Term::Record { fields, .. } = &*record else {
        panic!("not a record")
    };
    assert_eq!(wrapper.parameters.len(), 1);
    assert_eq!(
        engine.find(fields["value"]),
        engine.find(wrapper.parameters[0])
    );
    assert!(engine.structure(wrapper.parameters[0]).is_none());
    for scheme in [
        &globals[&interface.values["id"]],
        &catalog.constructors[&interface.constructors["Box"]],
    ] {
        let one = engine.instantiate(scheme, 1);
        let two = engine.instantiate(scheme, 1);
        let (a, _) = function(&mut engine, one);
        let (b, _) = function(&mut engine, two);
        let integer = engine.term(Term::Named(builtins.int, vec![]));
        let text = engine.term(Term::Named(builtins.string, vec![]));
        engine.unify(a, integer).unwrap();
        engine.unify(b, text).unwrap();
    }
}
#[test]
fn forward_aliases_expand_transparently_and_preserve_parameter_positions() {
    let (mut symbols, mut engine, builtins) = setup();
    let ast = parse("type alias Pair a b = Later b a\ntype alias Later a b = (a,b)\nx : Pair Int String\nx = ()").unwrap();
    let (interface, resolved) = resolve(&ast, "test:Main", &mut symbols, builtins);
    let mut catalog = Catalog::default();
    let declarations = catalog
        .register(&ast, &resolved, &symbols, "test:Main", &mut engine)
        .unwrap();
    let root = engine.instantiate(&declarations.annotations[&interface.values["x"]], 1);
    let t = engine.structure(root).unwrap();
    let Term::Tuple(args) = &*t else { panic!() };
    named(&mut engine, args[0], builtins.string);
    named(&mut engine, args[1], builtins.int);
}
#[test]
fn recursive_unions_remain_nominal_and_constructor_instances_are_independent() {
    let (mut symbols, mut engine, builtins) = setup();
    let ast = parse("type Tree a = Branch (List (Tree a)) | Leaf a").unwrap();
    let (interface, resolved) = resolve(&ast, "test:Main", &mut symbols, builtins);
    let mut catalog = Catalog::default();
    catalog
        .register(&ast, &resolved, &symbols, "test:Main", &mut engine)
        .unwrap();
    let scheme = &catalog.constructors[&interface.constructors["Leaf"]];
    let one = engine.instantiate(scheme, 1);
    let two = engine.instantiate(scheme, 1);
    let (a, result) = function(&mut engine, one);
    let (b, _) = function(&mut engine, two);
    let i = engine.term(Term::Named(builtins.int, vec![]));
    let s = engine.term(Term::Named(builtins.string, vec![]));
    engine.unify(a, i).unwrap();
    engine.unify(b, s).unwrap();
    named(&mut engine, result, interface.types["Tree"]);
    let branch = engine.instantiate(&catalog.constructors[&interface.constructors["Branch"]], 1);
    let (children, _) = function(&mut engine, branch);
    named(&mut engine, children, builtins.list);
}
#[test]
fn record_constructor_uses_source_field_order() {
    let (mut symbols, mut engine, builtins) = setup();
    let ast = parse("type alias Row = { z : Int, a : String }").unwrap();
    let (interface, resolved) = resolve(&ast, "test:Main", &mut symbols, builtins);
    let mut catalog = Catalog::default();
    catalog
        .register(&ast, &resolved, &symbols, "test:Main", &mut engine)
        .unwrap();
    let root = engine.instantiate(&catalog.constructors[&interface.constructors["Row"]], 1);
    let (first, rest) = function(&mut engine, root);
    let (second, result) = function(&mut engine, rest);
    named(&mut engine, first, builtins.int);
    named(&mut engine, second, builtins.string);
    assert!(matches!(
        &*engine.structure(result).unwrap(),
        Term::Record {
            extension: None,
            ..
        }
    ));
}
#[test]
fn annotation_variables_share_constraints_and_open_record_extensions() {
    let (mut symbols, mut engine, builtins) = setup();
    let ast = parse("f : { row | value : number1 } -> number1\nf x = x.value").unwrap();
    let (interface, resolved) = resolve(&ast, "test:Main", &mut symbols, builtins);
    let mut catalog = Catalog::default();
    let declarations = catalog
        .register(&ast, &resolved, &symbols, "test:Main", &mut engine)
        .unwrap();
    let scheme = &declarations.annotations[&interface.values["f"]];
    assert_eq!(scheme.quantified.len(), 2);
    let root = engine.instantiate(scheme, 1);
    let (arg, result) = function(&mut engine, root);
    let t = engine.structure(arg).unwrap();
    let Term::Record {
        fields,
        extension: Some(_),
    } = &*t
    else {
        panic!()
    };
    assert_eq!(engine.find(fields["value"]), engine.find(result));
    let s = engine.term(Term::Named(builtins.string, vec![]));
    assert!(engine.unify(result, s).is_err());
}
#[test]
fn imported_alias_templates_survive_dropping_their_source() {
    let (mut symbols, mut engine, builtins) = setup();
    let mut catalog = Catalog::default();
    let interface = {
        let source = String::from("module A exposing (Alias)\ntype alias Alias a = { value : a }");
        let ast = parse(&source).unwrap();
        let (interface, resolved) = resolve(&ast, "pkg:A", &mut symbols, builtins);
        catalog
            .register(&ast, &resolved, &symbols, "pkg:A", &mut engine)
            .unwrap();
        interface
    };
    let ast = parse("x : A.Alias ()\nx = ()").unwrap();
    let mut env = Environment::default();
    env.import("A", &interface, &Exposing::Explicit(vec![]), &symbols)
        .unwrap();
    let (interface, resolved) = names::resolve(&ast, "pkg:B", env, &mut symbols).unwrap();
    let declarations = catalog
        .register(&ast, &resolved, &symbols, "pkg:B", &mut engine)
        .unwrap();
    let root = declarations.annotations[&interface.values["x"]].root;
    let t = engine.structure(root).unwrap();
    let Term::Record { fields, .. } = &*t else {
        panic!()
    };
    assert!(matches!(
        &*engine.structure(fields["value"]).unwrap(),
        Term::Unit
    ));
    assert!(symbols.lookup("pkg:A", "Alias", Space::Type).is_some());
}
