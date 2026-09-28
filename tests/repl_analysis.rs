use planexpo_elm::{
    analyze,
    project::{Graph, Module},
};

fn graph(source: &str) -> Graph {
    Graph {
        import_errors: Vec::new(),
        entry: "elm/core:Repl".into(),
        entries: vec!["elm/core:Repl".into()],
        manifests: vec![],
        modules: vec![Module {
            missing_header: None,
            owner: "elm/core".into(),
            name: "Repl".into(),
            path: "/src/Repl.elm".into(),
            imports: vec![],
            bytes: source.len(),
            tokens: 0,
            kernel: false,
            source: source.into(),
        }],
    }
}

#[test]
fn repl_preserves_private_inferred_schemes_without_changing_normal_generation() {
    let graph = graph(
        "module Repl exposing (exposed)\nexposed = ()\nidentity x = x\nvalue = identity ()\n",
    );
    let normal = analyze::generate_modules(&graph).unwrap();
    assert!(normal.repl_types.is_none());
    assert!(normal.repl_type_names.is_none());
    let report = analyze::generate_for_repl(&graph).unwrap();
    assert!(report.generation_errors.is_empty());
    assert_eq!(
        report
            .repl_type_names
            .as_ref()
            .unwrap()
            .name("elm/core:Repl", "Private"),
        "Private"
    );
    let artifact = report.repl_types.unwrap();
    let entries = artifact["entries"].as_array().unwrap();
    assert!(entries.iter().any(|entry| entry["name"] == "identity"));
    let value = entries
        .iter()
        .find(|entry| entry["name"] == "value")
        .unwrap();
    let root = artifact["graph"]["roots"][value["root"].as_u64().unwrap() as usize]
        .as_u64()
        .unwrap() as usize;
    assert_eq!(
        artifact["graph"]["nodes"][root],
        serde_json::json!(["unit"])
    );
    assert_eq!(report.generated.len(), normal.generated.len());
}

#[test]
fn repl_still_rejects_invalid_main_like_elm_official() {
    assert!(analyze::generate_for_repl(&graph("module Repl exposing (..)\nmain = ()\n")).is_err());
}

#[test]
fn alias_of_a_parameter_remains_transparent_in_annotated_functions() {
    let graph = graph(
        "module Repl exposing (..)\ntype alias Identity item = item\nidentity : Identity item -> item\nidentity x = x\n",
    );
    let normal = analyze::generate_modules(&graph).unwrap();
    assert!(normal.generation_errors.is_empty());
    let repl = analyze::generate_for_repl(&graph).unwrap();
    assert!(repl.generation_errors.is_empty());
}

#[test]
fn repl_preserves_annotation_variable_names_through_inference() {
    let report = analyze::generate_for_repl(&graph(
        "module Repl exposing (..)\nidentity : message -> message\nidentity x = x\nvalue = identity\nkeep : { row | field : item } -> { row | field : item }\nkeep x = x\nrecord = keep\n",
    )).unwrap();
    let artifact = report.repl_types.as_ref().unwrap();
    for (name, expected) in [
        ("value", "message -> message"),
        ("record", "{ row | field : item } -> { row | field : item }"),
    ] {
        let entry = artifact["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == name)
            .unwrap();
        let actual = planexpo_elm::repl_type::render(
            &artifact["graph"],
            entry["root"].as_u64().unwrap() as usize,
            report.repl_type_names.as_ref().unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }
}
