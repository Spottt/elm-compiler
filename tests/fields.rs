use planexpo_elm::{
    fields::Fields,
    project::{Graph, Module},
};

#[test]
fn production_fields_cover_every_source_form_and_kernel_references() {
    let source = "module Main exposing (..)\nrecord = { frequent = 1, rare = 2 }\nread value = value.frequent\naccessor = .frequent\ntype alias Only = { typeOnly : Int }\nextract {patternOnly} = patternOnly\n";
    let mut graph = Graph {
        import_errors: Vec::new(),
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

#[test]
fn development_field_escaping_preserves_the_javascript_and_elm_helper_abi() {
    let reserved = "do if in NaN int for new try var let null true eval byte char goto long case else this void with enum false final float short break catch throw while class const super yield double native throws delete return switch typeof export import public static boolean default finally extends package private Infinity abstract volatile function continue debugger undefined arguments transient interface protected instanceof implements synchronized F2 F3 F4 F5 F6 F7 F8 F9 A2 A3 A4 A5 A6 A7 A8 A9";
    let mut names: Vec<String> = reserved.split_whitespace().flat_map(|word| {
        [word.into(), format!("{word}_"), format!("_{word}"), word.to_uppercase()]
    }).collect();
    for first in b'A'..=b'Z' {
        for second in b'0'..=b'9' {
            names.push(String::from_utf8(vec![first, second]).unwrap());
        }
    }
    names.extend(["", "customerName", "étiquette", "F22", "A10", "a2"].map(str::to_owned));
    for name in names {
        let expected = if reserved.split_whitespace().any(|word| word == name) {
            format!("_{name}")
        } else { name.clone() };
        assert_eq!(Fields::default().name(&name).unwrap(), expected, "{name}");
    }
}
