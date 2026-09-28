use planexpo_elm::api_diff::{Magnitude, diff, equivalent_type};
use serde_json::{Value, json};
fn module(values: Value) -> Value {
    json!([{"name":"Example","comment":"","unions":[],"aliases":[],"values":values,"binops":[]}])
}
fn value(name: &str, tipe: &str) -> Value {
    json!({"name":name,"comment":"","type":tipe})
}
#[test]
fn alpha_renaming_is_bijective_and_preserves_parameter_positions() {
    let cases = [
        ("a -> a", "b -> b", true),
        ("a -> a", "a -> b", false),
        ("a -> b", "c -> c", false),
        ("number -> number", "comparable -> comparable", true),
        ("comparable -> comparable", "number -> number", false),
        ("appendable -> appendable", "a -> a", true),
        ("a -> a", "appendable -> appendable", false),
        (
            "compappend -> compappend",
            "comparable -> comparable",
            false,
        ),
        ("number1 -> number1", "number2 -> number2", true),
    ];
    for (old, new, expected) in cases {
        assert_eq!(
            equivalent_type(old, new, &[], &[]).unwrap(),
            expected,
            "{old} / {new}"
        );
    }
    assert!(
        !equivalent_type(
            "a -> b",
            "x -> y",
            &["a".into(), "b".into()],
            &["y".into(), "x".into()]
        )
        .unwrap()
    );
}
#[test]
fn structural_types_cover_records_tuples_and_legacy_qualifications() {
    for (old, new, expected) in [
        (
            "{ row | a : Int, b : List x }",
            "{ rest | b : List y, a : Basics.Int }",
            true,
        ),
        ("{ a : Int }", "{ row | a : Int }", false),
        ("{ a : Int }", "{ b : Int }", false),
        ("Time", "Time.Posix", false),
        ("Posix", "Time.Posix", true),
        ("A.Token", "B.Token", true),
        ("(a, b)", "(x, y, z)", false),
        ("()", "()", true),
    ] {
        assert_eq!(
            equivalent_type(old, new, &[], &[]).unwrap(),
            expected,
            "{old} / {new}"
        );
    }
    assert!(equivalent_type("List (", "Int", &[], &[]).is_err());
}
#[test]
fn package_magnitude_and_change_inventory_match_api_changes() {
    let old = module(json!([value("id", "a -> a")]));
    let mut new = module(json!([value("id", "b -> b")]));
    new[0]["comment"] = json!("new documentation");
    assert_eq!(diff(&old, &new).unwrap().magnitude(), Magnitude::Patch);
    new[0]["values"]
        .as_array_mut()
        .unwrap()
        .push(value("extra", "Int"));
    let changes = diff(&old, &new).unwrap();
    assert_eq!(changes.magnitude(), Magnitude::Minor);
    assert!(
        changes.changed["Example"]
            .values
            .added
            .contains_key("extra")
    );
    assert_eq!(diff(&new, &old).unwrap().magnitude(), Magnitude::Major);
    assert_eq!(diff(&json!([]), &old).unwrap().added, vec!["Example"]);
    assert_eq!(
        diff(&old, &json!([])).unwrap().magnitude(),
        Magnitude::Major
    );
}
#[test]
fn union_constructor_order_and_operator_fixity_are_public_api() {
    let mut old = module(json!([]));
    old[0]["unions"] =
        json!([{"name":"Choice","comment":"","args":["a"],"cases":[["One",["a"]],["Two",[]]]}]);
    let mut new = old.clone();
    new[0]["unions"][0]["cases"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_eq!(diff(&old, &new).unwrap().magnitude(), Magnitude::Major);
    old[0]["binops"] = json!([{"name":"%%","comment":"","type":"a -> a -> a","associativity":"left","precedence":5}]);
    new = old.clone();
    new[0]["binops"][0]["precedence"] = json!(6);
    assert_eq!(diff(&old, &new).unwrap().magnitude(), Magnitude::Major);
    new = old.clone();
    new[0]["unions"][0]["comment"] = json!("updated");
    assert_eq!(diff(&old, &new).unwrap().magnitude(), Magnitude::Patch);
}

#[test]
fn bumps_reset_lower_components_and_malformed_added_types_are_rejected() {
    use planexpo_elm::package_solver::Version;
    let v = Version([2, 3, 4]);
    assert_eq!(Magnitude::Patch.bump(v), Version([2, 3, 5]));
    assert_eq!(Magnitude::Minor.bump(v), Version([2, 4, 0]));
    assert_eq!(Magnitude::Major.bump(v), Version([3, 0, 0]));
    assert!(diff(&json!([]), &module(json!([value("broken", "List (")]))).is_err());
    assert!(diff(&json!({}), &json!([])).is_err());
}

#[test]
fn diff_union_without_parameters_preserves_official_header_spacing() {
    for (cases, expected) in [
        (json!([]), "type NoParameters "),
        (json!([["Only", []]]), "type NoParameters  = Only"),
        (json!([["First", []], ["Second", []]]), "type NoParameters  = First | Second"),
    ] {
        let old = module(json!([]));
        let mut new = old.clone();
        new[0]["unions"] = json!([{"name":"NoParameters","comment":"","args":[],"cases":cases}]);
        let rendered = planexpo_elm::api_diff_render::render(&diff(&old, &new).unwrap()).unwrap();
        assert_eq!(rendered, format!("This is a MINOR change.\n\n---- Example - MINOR ----\n\n    Added:\n        {expected}\n\n\n"));
    }
}
