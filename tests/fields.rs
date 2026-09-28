use planexpo_elm::{
    fields::Fields,
    project::{Graph, Module},
};

#[test]
fn production_fields_cover_every_source_form_and_kernel_references() {
    let source = "module Main exposing (..)\nrecord = { frequent = 1, rare = 2 }\nread value = value.frequent\naccessor = .frequent\ntype alias Only = { typeOnly : Int }\nextract {patternOnly} = patternOnly\n";
    let mut graph = Graph {
        entry: "application:Main".into(),
        entries: vec!["application:Main".into()],
        manifests: vec![],
        modules: vec![
            Module {
                missing_header: None,
                owner: "application".into(),
                name: "Main".into(),
                path: "Main.elm".into(),
                imports: vec![],
                bytes: source.len(),
                tokens: 0,
                kernel: false,
                source: source.into(),
            },
            Module {
                missing_header: None,
                owner: "elm/core".into(),
                name: "Elm.Kernel.Probe".into(),
                path: "Probe.js".into(),
                imports: vec![],
                bytes: 0,
                tokens: 0,
                kernel: true,
                source: "/*\n*/\nvar _Probe_value = { __$kernelOnly: 1 };".into(),
            },
        ],
    };
    let fields = Fields::production(&graph).unwrap();
    assert_eq!(fields.name("frequent").unwrap(), "a");
    let names = ["frequent", "rare", "typeOnly", "patternOnly", "kernelOnly"];
    let mapped: std::collections::BTreeSet<_> = names
        .iter()
        .map(|name| fields.name(name).unwrap())
        .collect();
    assert_eq!(mapped.len(), names.len());
    assert!(mapped.iter().all(|name| name.len() == 1));
    assert!(fields.name("missing").is_err());
    graph.modules.reverse();
    let reordered = Fields::production(&graph).unwrap();
    for name in names {
        assert_eq!(fields.name(name), reordered.name(name));
    }
    assert_eq!(Fields::default().name("return").unwrap(), "_return");
}
