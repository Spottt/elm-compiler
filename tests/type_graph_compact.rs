use planexpo_elm::type_graph_compact::compact;
use serde_json::json;
#[test]
fn shares_closed_structures_but_never_independent_type_variables() {
    let graph = json!({"version":1,"nodes":[
        ["variable",1,"any"],["variable",1,"any"],
        ["unit"],["unit"],
        ["record",{"field":2},null],["record",{"field":3},null],
        ["function",0,4],["function",1,5]
    ],"roots":[0,1,4,5,6,7]});
    let result = compact(&graph).unwrap();
    assert_eq!(result["nodes"].as_array().unwrap().len(), 6);
    assert_ne!(result["roots"][0], result["roots"][1]);
    assert_eq!(result["roots"][2], result["roots"][3]);
    assert_ne!(result["roots"][4], result["roots"][5]);
    let mut symbols = planexpo_elm::names::Symbols::default();
    let mut engine = planexpo_elm::unify::Engine::new(planexpo_elm::types::builtins(&mut symbols));
    let roots = engine
        .import_types(&result, |_| Err("unexpected symbol".into()))
        .unwrap();
    engine.unify(roots[0], roots[2]).unwrap();
    assert!(engine.structure(roots[1]).is_none());
}
#[test]
fn malformed_recursive_and_display_graphs_keep_the_original_representation() {
    for graph in [
        json!({"version":1,"nodes":[["function",0,0]],"roots":[0]}),
        json!({"version":1,"nodes":[["tuple",[99]]],"roots":[0]}),
        json!({"version":1,"nodes":[["unit"]],"roots":[0],"record_field_order":{}}),
        json!({"version":1,"nodes":[["unit"]],"roots":[0],"variable_names":{}}),
    ] {
        assert!(compact(&graph).is_none());
    }
}
#[test]
fn deep_closed_graphs_do_not_use_the_call_stack() {
    let mut nodes = vec![json!(["unit"])];
    for n in 0..10000 {
        nodes.push(json!(["tuple", [n, n]]));
    }
    let result = compact(&json!({"version":1,"nodes":nodes,"roots":[10000]})).unwrap();
    assert_eq!(result["nodes"].as_array().unwrap().len(), 10001);
}
