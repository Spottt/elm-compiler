use planexpo_elm::{
    names::{self, Binding, Environment, Space, SymbolKind, Symbols},
    parser::parse,
};
fn standalone(source: &str) -> Result<names::Resolved, String> {
    let ast = parse(source)?;
    let mut symbols = Symbols::default();
    let mut env = Environment::default();
    env.builtin_list(&mut symbols);
    names::resolve(&ast, "test:Main", env, &mut symbols).map(|(_, r)| r)
}
#[test]
fn binds_recursive_let_and_distinct_branch_locals() {
    let r=standalone("f n =\n    let\n        walk x = walk x\n    in\n    case n of\n        [] -> (\\x -> walk x) n\n        h :: t -> walk t\n").unwrap();
    assert_eq!(r.definitions.len(), 1);
    assert!(
        r.expressions
            .iter()
            .flatten()
            .any(|b| matches!(b, Binding::Local(_)))
    );
}
#[test]
fn rejects_unknown_names_shadowing_and_duplicate_patterns() {
    for s in [
        "f x = missing",
        "f x = \\x -> x",
        "f x x = x",
        "f x = let f y = y in x",
        "f (a,a) = a",
        "f x = {a=x,a=x}",
    ] {
        assert!(standalone(s).is_err(), "accepted {s}");
    }
}
#[test]
fn constructors_and_type_arity_are_checked() {
    standalone("type Choice a = One a | None\nf (One x) = x\n").unwrap();
    for s in [
        "type Choice a = One a\nf One = 1",
        "type Choice a = One a\nf (One x y) = x",
        "type alias X = Unknown",
        "type alias X a = List",
        "type alias X = {a : missing}",
    ] {
        assert!(standalone(s).is_err(), "accepted {s}");
    }
}
#[test]
fn qualified_alias_does_not_leave_original_module_name_visible() {
    let mut symbols = Symbols::default();
    let id = symbols.intern(
        "author/pkg:Original",
        "value",
        Space::Value,
        SymbolKind::Value,
    );
    let foreign = names::Interface {
        values: [("value".into(), id)].into(),
        ..Default::default()
    };
    let mut env = Environment::default();
    env.import(
        "Alias",
        &foreign,
        &planexpo_elm::module::Exposing::Explicit(vec![]),
        &symbols,
    )
    .unwrap();
    let ast = parse("x = Alias.value").unwrap();
    let (_, r) = names::resolve(&ast, "test:Main", env, &mut symbols).unwrap();
    assert!(r.expressions.contains(&Some(Binding::Global(id))));
    let mut env = Environment::default();
    env.import(
        "Alias",
        &foreign,
        &planexpo_elm::module::Exposing::Explicit(vec![]),
        &symbols,
    )
    .unwrap();
    assert!(
        names::resolve(
            &parse("x = Original.value").unwrap(),
            "test:Other",
            env,
            &mut symbols
        )
        .is_err()
    );
}

#[test]
fn rejects_recursive_aliases_but_accepts_recursive_unions() {
    standalone("type Tree a = Branch (List (Tree a)) | Leaf a").unwrap();
    for s in [
        "type alias A = A",
        "type alias A = List B\ntype alias B = A",
        "type alias A a = ()",
    ] {
        let error = standalone(s).unwrap_err();
        assert!(
            error.contains("recursive") || error.contains("unused type parameter"),
            "{error}"
        );
    }
}
#[test]
fn opaque_constructors_stay_private_and_record_aliases_cannot_be_patterns() {
    use planexpo_elm::module::{Exposed, Exposing};
    let mut symbols = Symbols::default();
    let a =
        parse("module A exposing (Secret, Row)\ntype Secret = Secret\ntype alias Row = { x : () }")
            .unwrap();
    let (interface, _) =
        names::resolve(&a, "author/pkg:A", Environment::default(), &mut symbols).unwrap();
    assert!(!interface.constructors.contains_key("Secret"));
    assert!(interface.constructors.contains_key("Row"));
    let exposure = Exposing::Explicit(vec![
        Exposed::Type {
            name: "Secret".into(),
            constructors: true,
        },
        Exposed::Type {
            name: "Row".into(),
            constructors: false,
        },
    ]);
    for text in ["x = A.Secret", "f (Row x) = x"] {
        let mut env = Environment::default();
        env.import("A", &interface, &exposure, &symbols).unwrap();
        assert!(names::resolve(&parse(text).unwrap(), "test:Main", env, &mut symbols).is_err());
    }
}
#[test]
fn application_code_cannot_access_kernel_internals() {
    assert!(
        standalone("x = Elm.Kernel.Bytes.width")
            .unwrap_err()
            .contains("unknown name")
    );
    let mut symbols = Symbols::default();
    names::resolve(
        &parse("x = Elm.Kernel.Bytes.width").unwrap(),
        "elm/bytes:Decode",
        Environment::default(),
        &mut symbols,
    )
    .unwrap();
}

#[test]
fn titlecase_constructor_references_resolve_like_uppercase() {
    for name in ["ǅelta", "ǈambda", "ǋame", "ǲeta"] {
        standalone(&format!(
            "type Box = {name}\nvalue = {name}\nf {name} = ()\n"
        ))
        .unwrap();
    }
}

#[test]
fn duplicate_regions_follow_official_binding_and_constructor_order() {
    for (source, region) in [
        ("f x x = x", "1:3:1:4:"),
        ("f (x as x) = x", "1:4:1:5:"),
        ("type Thing = One\ntype Other = One", "1:14:1:17:"),
        ("type Thing = One | One", "1:20:1:23:"),
    ] {
        let error = standalone(source).unwrap_err();
        assert!(error.starts_with(region), "{source}: {error}");
    }
}

#[test]
fn symbol_lookup_preserves_module_and_namespace_identity() {
    let mut symbols = Symbols::default();
    let value = symbols.intern("pkg:A", "Same", Space::Value, SymbolKind::Value);
    let other = symbols.intern("pkg:B", "Same", Space::Value, SymbolKind::Value);
    assert_ne!(value, other);
    assert_eq!(symbols.lookup("pkg:A", "Same", Space::Value), Some(value));
    assert_eq!(symbols.lookup("pkg:B", "Same", Space::Value), Some(other));
    assert_eq!(symbols.lookup("pkg:A", "Same", Space::Type), None);
    assert_eq!(symbols.lookup("pkg:A", "Unknown", Space::Value), None);
    assert_eq!(symbols.lookup("pkg:Unknown", "Same", Space::Value), None);
    assert_eq!(symbols.lookup("Same", "pkg:A", Space::Value), None);
    assert_eq!(symbols.intern("pkg:A", "Same", Space::Value, SymbolKind::Value), value);
    assert_eq!(symbols.entries.len(), 2);
}

#[test]
fn repeated_imports_stay_unique_and_ambiguity_is_order_independent() {
    use planexpo_elm::module::Exposing;
    let mut symbols = Symbols::default();
    let mut interfaces = Vec::new();
    for module in ["A", "B", "C"] {
        let id = symbols.intern(&format!("test:{module}"), "value", Space::Value, SymbolKind::Value);
        interfaces.push(names::Interface { values: [("value".into(), id)].into(), ..Default::default() });
    }
    let mut env = Environment::default();
    for _ in 0..3 {
        env.import("Alias", &interfaces[0], &Exposing::All, &symbols).unwrap();
    }
    let (_, resolved) = names::resolve(&parse("x = (value, Alias.value)").unwrap(), "test:Main", env.clone(), &mut symbols).unwrap();
    assert!(resolved.expressions.contains(&Some(Binding::Global(interfaces[0].values["value"]))));
    let mut errors = Vec::new();
    for order in [[1, 2, 1], [2, 1, 2]] {
        let mut ambiguous = env.clone();
        for index in order {
            ambiguous.import("Alias", &interfaces[index], &Exposing::All, &symbols).unwrap();
        }
        errors.push(names::resolve(&parse("x = Alias.value").unwrap(), "test:Main", ambiguous, &mut symbols).err().unwrap());
    }
    assert_eq!(errors[0], errors[1]);
    assert!(errors[0].contains("A.value") && errors[0].contains("B.value") && errors[0].contains("C.value"));
    // Extending a cloned environment cannot change the original singleton.
    assert!(names::resolve(&parse("x = Alias.value").unwrap(), "test:Main", env, &mut symbols).is_ok());
}
