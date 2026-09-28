use planexpo_elm::{
    entry,
    names::{Space, SymbolKind, Symbols},
    types,
    unify::{Constraint, Engine, Scheme, Term, Ty},
};
use std::collections::BTreeSet;
fn named(e: &mut Engine, s: &mut Symbols, module: &str, name: &str, args: Vec<Ty>) -> Ty {
    let id = s.intern(
        module,
        name,
        Space::Type,
        SymbolKind::Type {
            arity: args.len(),
            alias: false,
        },
    );
    e.term(Term::Named(id, args))
}
fn scheme(root: Ty) -> Scheme {
    Scheme {
        root,
        quantified: BTreeSet::new(),
    }
}
#[test]
fn accepts_only_canonical_main_types() {
    let mut s = Symbols::default();
    let mut e = Engine::new(types::builtins(&mut s));
    let msg = e.variable(1, Constraint::Any);
    let node = named(
        &mut e,
        &mut s,
        "elm/virtual-dom:VirtualDom",
        "Node",
        vec![msg],
    );
    entry::check_main(&mut e, &s, &scheme(node)).unwrap();
    let fake = named(&mut e, &mut s, "app:Main", "Node", vec![msg]);
    assert!(entry::check_main(&mut e, &s, &scheme(fake)).is_err());
    let integer = named(&mut e, &mut s, "elm/core:Basics", "Int", vec![]);
    assert!(entry::check_main(&mut e, &s, &scheme(integer)).is_err());
}
#[test]
fn checks_nested_flags_and_rejects_functions_and_open_records() {
    let mut s = Symbols::default();
    let mut e = Engine::new(types::builtins(&mut s));
    let unit = e.term(Term::Unit);
    let function = e.term(Term::Function(unit, unit));
    let variable = e.variable(1, Constraint::Any);
    let open = e.term(Term::Record {
        fields: Default::default(),
        extension: Some(variable),
    });
    for flags in [function, open, variable] {
        let program = named(
            &mut e,
            &mut s,
            "elm/core:Platform",
            "Program",
            vec![flags, unit, unit],
        );
        assert!(entry::check_main(&mut e, &s, &scheme(program)).is_err());
    }
    let int = named(&mut e, &mut s, "elm/core:Basics", "Int", vec![]);
    let list = named(&mut e, &mut s, "elm/core:List", "List", vec![int]);
    let flags = e.term(Term::Record {
        fields: [("items".into(), list)].into(),
        extension: None,
    });
    let program = named(
        &mut e,
        &mut s,
        "elm/core:Platform",
        "Program",
        vec![flags, unit, unit],
    );
    entry::check_main(&mut e, &s, &scheme(program)).unwrap();
}

#[test]
fn port_signatures_require_matching_polymorphic_messages() {
    let mut s = Symbols::default();
    let mut e = Engine::new(types::builtins(&mut s));
    let unit = e.term(Term::Unit);
    let msg = e.variable(1, Constraint::Any);
    let other = e.variable(1, Constraint::Any);
    let cmd = named(&mut e, &mut s, "elm/core:Platform.Cmd", "Cmd", vec![msg]);
    let sub = named(&mut e, &mut s, "elm/core:Platform.Sub", "Sub", vec![msg]);
    let outgoing = e.term(Term::Function(unit, cmd));
    assert_eq!(
        entry::check_port(&mut e, &s, outgoing).unwrap().direction,
        entry::Direction::Outgoing
    );
    let callback = e.term(Term::Function(unit, msg));
    let incoming = e.term(Term::Function(callback, sub));
    assert_eq!(
        entry::check_port(&mut e, &s, incoming).unwrap().direction,
        entry::Direction::Incoming
    );
    let wrong_callback = e.term(Term::Function(unit, other));
    let wrong = e.term(Term::Function(wrong_callback, sub));
    assert!(entry::check_port(&mut e, &s, wrong).is_err());
    assert!(entry::check_port(&mut e, &s, cmd).is_err());
    let extra = e.term(Term::Function(unit, outgoing));
    assert!(entry::check_port(&mut e, &s, extra).is_err());
    let concrete_cmd = named(&mut e, &mut s, "elm/core:Platform.Cmd", "Cmd", vec![unit]);
    let concrete = e.term(Term::Function(unit, concrete_cmd));
    assert!(entry::check_port(&mut e, &s, concrete).is_err());
    let function_payload = e.term(Term::Function(callback, cmd));
    assert!(entry::check_port(&mut e, &s, function_payload).is_err());
}

#[test]
fn module_headers_enforce_port_and_effect_manager_boundaries() {
    use planexpo_elm::parser::parse;
    let port = parse("port module P exposing (..)\nport out : () -> Cmd msg").unwrap();
    entry::check_module(&port, "application").unwrap();
    assert!(entry::check_module(&port, "author/pkg").is_err());
    assert!(
        entry::check_module(
            &parse("module P exposing (..)\nport out : () -> Cmd msg").unwrap(),
            "application"
        )
        .is_err()
    );
    assert!(
        entry::check_module(
            &parse("port module P exposing (..)\nx=()").unwrap(),
            "application"
        )
        .is_err()
    );
    // A leading `port` is parsed as the start of a port-module header by Elm.
    assert!(parse("port out : () -> Cmd msg").is_err());
    let implicit = parse("value = ()\nport out : () -> Cmd msg").unwrap();
    assert!(!implicit.header.explicit);
    entry::check_module(&implicit, "application").unwrap();
    let manager=parse("effect module M where { command = Task } exposing (..)\ntype Task a = Task\ninit=()\nonEffects=()\nonSelfMsg=()\ncmdMap=()").unwrap();
    entry::check_module(&manager, "elm/core").unwrap();
    assert!(entry::check_module(&manager, "application").is_err());
    let missing = parse(
        "effect module M where { command = Task } exposing (..)\ntype Task a = Task\ninit=()",
    )
    .unwrap();
    assert!(entry::check_module(&missing, "elm/core").is_err());
}

#[test]
fn operator_declarations_require_a_kernel_package_even_without_a_header() {
    use planexpo_elm::parser::parse;
    for header in ["", "module Operators exposing ((|=))\n"] {
        let source = format!("{header}infix left 4 (|=) = combine\ncombine a b = a\n");
        let ast = parse(&source).unwrap();
        for owner in ["application", "author/package", "not-elm/core"] {
            assert!(
                entry::check_module(&ast, owner)
                    .unwrap_err()
                    .contains("operator declarations")
            );
        }
        for owner in ["elm/core", "elm/json", "elm-explorations/test"] {
            entry::check_module(&ast, owner).unwrap();
        }
    }
}
