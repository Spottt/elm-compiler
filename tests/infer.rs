use planexpo_elm::{
    infer,
    names::{self, Environment, Interface, Symbols},
    parser::parse,
    types::{self, Catalog},
    unify::Engine,
};
use std::collections::BTreeMap;
fn check(source: &str) -> Result<(), String> {
    let ast = parse(source)?;
    let mut symbols = Symbols::default();
    let mut engine = Engine::new(types::builtins(&mut symbols));
    let mut env = Environment::default();
    env.builtin_list(&mut symbols);
    let core = Interface::default();
    env.import(
        "Core",
        &core,
        &planexpo_elm::module::Exposing::All,
        &symbols,
    )?;
    let (_, resolved) = names::resolve(&ast, "test:Main", env, &mut symbols)?;
    let mut catalog = Catalog::default();
    catalog.register_type_definitions(&ast, &resolved, &symbols, "test:Main", &mut engine)?;
    infer::check(
        types::SourceTypes {
            ast: &ast,
            resolved: &resolved,
            symbols: &symbols,
        },
        "test:Main",
        &catalog,
        &mut engine,
        &mut BTreeMap::new(),
    )?;
    Ok(())
}
#[test]
fn functions_are_polymorphic_in_dependency_order() {
    check("pair = (identity (), identity [])\nidentity x = x").unwrap();
    check("f arg = let id x = x in (id arg, id [])").unwrap();
    check("f x = g x\ng y = f y").unwrap();
    assert!(check("bad x = x x").is_err());
}
#[test]
fn records_patterns_and_updates_generate_constraints() {
    check("get r = r.value\nx = (get { value=() }, get { value=[] })").unwrap();
    check("f {value} = value\ng r = { r | value = () }").unwrap();
    check("f xs = case xs of\n    [] -> ()\n    h :: t -> h").unwrap();
    assert!(check("f r = (r.value, r.value ())\nx = f {value=()}").is_err());
    assert!(check("f r = { r | value = () }\nx = f {}").is_err());
    assert!(check("f x = case x of\n    [] -> ()\n    h :: t -> []").is_err());
}
#[test]
fn annotations_are_checked_and_local_annotations_share_outer_type_variables() {
    check("f : a -> a\nf x = x").unwrap();
    assert!(check("f : a -> a\nf x = ()").is_err());
    check("f : a -> a\nf x = let g : a -> a\n          g y = x\n      in g x").unwrap();
    assert!(check("f : a -> a\nf x = let g : a -> a\n          g y = ()\n      in g x").is_err());
}
#[test]
fn unused_top_level_annotations_are_still_checked_by_inference() {
    check("unused : a -> a\nunused x = x\nvalue = ()").unwrap();
    assert!(check("unused : () -> ()\nunused x = []\nvalue = ()").is_err());
    assert!(check("type Box a = Box a\nunused : Box () ()\nunused = Box ()\nvalue = ()").is_err());
}
#[test]
fn user_constructors_and_destructuring_are_typed() {
    check("type Box a = Box a\nunbox (Box value) = value\nx = unbox (Box ())").unwrap();
    check("type alias Pair a = { right : a, left : () }\nx = Pair [] ()").unwrap();
    check("f x = let (a,b) = (x,[]) in (a,b)").unwrap();
    assert!(check("type Box a = Box a\nunbox (Box value) = value\nx = unbox ()").is_err());
}

#[test]
fn accessing_a_field_preserves_the_rigid_record_extension() {
    check("f : { row | a : (), b : () } -> ()\nf r = let value = r.a in value").unwrap();
}

#[test]
fn recursive_calls_respect_the_current_annotation() {
    assert!(check("f : a -> b\nf x = f [x]").is_err());
    check("f : a -> b\nf x = f x").unwrap();
}

#[test]
fn mixed_recursive_annotations_do_not_escape_into_inferred_schemes() {
    check("f x = g x\ng : a -> ()\ng y = f [y]\nvalue = (f [()], f [[]])").unwrap();
    check("g : a -> ()\ng y = f [y]\nf x = g x\nvalue = (f [()], f [[]])").unwrap();
    assert!(check("f x = g x\ng : a -> a\ng y = f [y]\nvalue = f [()]").is_err());
}

#[test]
fn captured_function_parameters_stay_monomorphic_inside_recursive_groups() {
    let source = "outer z =\n    let\n        f x = g z\n        g y =\n            let\n                a = z ()\n                b = z []\n            in\n            f [y]\n    in\n    f []";
    assert!(
        check(source)
            .unwrap_err()
            .contains("type constructor mismatch")
    );
}

#[test]
fn local_value_cycles_are_rejected_even_when_unused_or_through_functions() {
    for source in [
        "f z =\n    let\n        x = y\n        y = x\n    in\n    z",
        "f z =\n    let\n        x = g ()\n        g _ = x\n    in\n    z",
        "f z =\n    let\n        x = g\n        g y = x y\n    in\n    z",
        "f z =\n    let\n        (x,y) = (y,x)\n    in\n    z",
    ] {
        let error = check(source).expect_err(source);
        assert!(error.contains("cyclic local value"), "{error}");
    }
    check("f z =\n    let\n        x a = y a\n        y a = x a\n    in\n    z").unwrap();
    check("f z =\n    let\n        x = y\n        y = z\n    in\n    x").unwrap();
}

#[test]
fn unannotated_recursive_groups_preserve_intermediate_polymorphism() {
    for source in [
        "f x = g x\ng y = f [y]",
        "g y = f [y]\nf x = g x",
        "f x = g x\ng {value} = f {value=[value]}",
        "f x = g x\ng (a,b) = f ([a],b)",
        "outer input =\n    let\n        f x = g x\n        g y = f [y]\n    in\n    f input",
    ] {
        check(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
    for source in [
        "f x = f [x]",
        "f x = g [x]\ng y = f y",
        "f x = g x\ng y = (f [y], y ())\nvalue = f ()",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}

#[test]
fn direct_global_value_cycles_are_rejected_before_inference() {
    for source in [
        "x = x",
        "x = y\ny = x",
        "x = let unused = x in ()",
        "x = (\\ignored -> ()) x",
        "x = case () of\n    _ -> x",
        "x = { x | value = () }",
    ] {
        let error = check(source).expect_err(source);
        assert!(error.contains("cyclic global value"), "{source}: {error}");
    }
}

#[test]
fn delayed_global_dependencies_are_not_direct_value_cycles() {
    for source in [
        "type Ring = Ring (() -> Ring)\na = Ring (\\_ -> b)\nb = Ring (\\_ -> a)",
        "x = f ()\nf arg = x",
        "x = let f arg = x in ()",
        "f = \\arg -> f arg",
    ] {
        check(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}

#[test]
fn type_errors_show_the_conflicting_types() {
    let error = check("bad = [(), []]").unwrap_err();
    assert!(error.contains("Cannot unify"), "{error}");
    assert!(error.contains("()"), "{error}");
    assert!(error.contains("List"), "{error}");
    let error = check("type Color = Red\ntype Size = Small\nbad = [Red, Small]").unwrap_err();
    assert!(error.contains("Color"), "{error}");
    assert!(error.contains("Size"), "{error}");
}

#[test]
fn record_errors_name_the_missing_fields() {
    let error = check("get r = r.amount\nbad = get { amuont = () }").unwrap_err();
    assert!(error.contains("amount"), "{error}");
    assert!(error.contains("amuont"), "{error}");
}

#[test]
fn type_error_rendering_stays_bounded_for_large_and_recursive_types() {
    let fields = (0..500)
        .map(|i| format!("field{i} = ()"))
        .collect::<Vec<_>>()
        .join(", ");
    let error = check(&format!("bad = [{{ {fields} }}, ()]")).unwrap_err();
    assert!(error.contains('…'), "{error}");
    assert!(error.len() < 2000, "diagnostic length: {}", error.len());
    let error = check("bad x = x x").unwrap_err();
    assert!(error.contains("infinite type"), "{error}");
    assert!(error.contains("Cannot unify"), "{error}");
    assert!(error.len() < 2000);
}

#[test]
fn type_error_points_to_the_inner_expression_with_unicode_columns() {
    let source = "-- café\nf x = (\"é\", [(), []])\n";
    let error = check(source).unwrap_err();
    let start = source.find("[(), []]").unwrap();
    let line_start = source.find("f x").unwrap();
    let column = source[line_start..start].chars().count() + 1;
    assert!(
        error.starts_with(&format!("2:{column}:2:{}:", column + 8)),
        "{error}"
    );
    assert!(error.contains("Cannot unify these types"), "{error}");
    assert!(error.contains("in `f`"), "{error}");
}

#[test]
fn annotation_mismatch_has_the_real_body_region() {
    let error = check("f : a -> a\nf x = ()\n").unwrap_err();
    assert!(error.starts_with("2:7:2:9:"), "{error}");
}

#[test]
fn incompatible_pattern_items_point_to_the_pattern() {
    let error = check("f [(), []] = ()\n").unwrap_err();
    assert!(error.starts_with("1:3:1:11:"), "{error}");
}

#[test]
fn local_destructuring_mismatch_points_to_its_value() {
    let error = check("f =\n    let\n        (a, b) = ()\n    in\n    a\n").unwrap_err();
    assert!(error.starts_with("3:18:3:20:"), "{error}");
    assert!(error.contains("in `destructuring`"), "{error}");
}
