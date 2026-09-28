use planexpo_elm::linker::helpers;
use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn all_call_helpers_support_full_and_partial_application() {
    let mut script = helpers();
    for arity in 2..=9 {
        let args = (1..=arity).map(|i| i.to_string()).collect::<Vec<_>>();
        let sum = arity * (arity + 1) / 2;
        script.push_str(&format!("var f=F{arity}(function(){{return Array.from(arguments).reduce((a,b)=>a+b,0);}});if(A{arity}(f,{})!=={sum})throw Error('full application');\n",args.join(",")));
        let applied = args.iter().map(|a| format!("({a})")).collect::<String>();
        script.push_str(&format!(
            "if(f{applied}!=={sum})throw Error('partial application');\n"
        ));
        let plain = (0..arity)
            .map(|i| format!("function(a{i}){{return "))
            .collect::<String>()
            + &(0..arity)
                .map(|i| format!("a{i}"))
                .collect::<Vec<_>>()
                .join("+")
            + &";}".repeat(arity);
        script.push_str(&format!("var plain={plain};if(A{arity}(plain,{})!=={sum})throw Error('plain closure fallback');\n",args.join(",")));
    }
    let mut child = Command::new("node")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn reachability_keeps_effect_callbacks_but_drops_unused_ports_and_values() {
    use planexpo_elm::{
        linker::reachable,
        module_codegen::{Definition, Registration},
        names::SymbolId,
    };
    use std::collections::BTreeSet;
    let def = |id, deps: &[u32], registration| Definition {
        symbol: SymbolId(id),
        javascript: String::new(),
        dependencies: deps.iter().map(|i| SymbolId(*i)).collect(),
        function: true,
        registration,
        cycle_initialization: None,
    };
    let definitions = vec![
        def(0, &[1], None),
        def(
            1,
            &[],
            Some(Registration {
                javascript: "register".into(),
                dependencies: BTreeSet::from([SymbolId(2)]),
            }),
        ),
        def(2, &[3], None),
        def(3, &[2], None),
        def(4, &[5], None),
        def(5, &[], None),
    ];
    let retained = reachable(definitions, BTreeSet::from([SymbolId(0)]));
    assert_eq!(
        retained.iter().map(|d| d.symbol.0).collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
}
#[test]
fn repl_linking_keeps_the_selected_value_and_drops_unrelated_roots() {
    use planexpo_elm::{
        analyze::Report, linker::assemble_for_repl, module_codegen::Definition, names::SymbolId,
        project::Graph,
    };
    use std::collections::BTreeSet;
    let graph = Graph {
        entry: "application:Repl".into(),
        entries: vec!["application:Repl".into()],
        modules: vec![],
        manifests: vec![],
    };
    let mut report = Report::default();
    report
        .link_names
        .insert((graph.entry.clone(), "value".into()), SymbolId(1));
    report
        .link_names
        .insert(("elm/core:Debug".into(), "toString".into()), SymbolId(0));
    report.link_roots.insert(SymbolId(2));
    report.main_export = Some("throw Error('old main export ran');".into());
    for (symbol, javascript) in [
        (
            0,
            "function _Debug_toAnsiString(ansi,value){return String(value);}",
        ),
        (1, "var $author$project$Repl$value=42;"),
        (2, "throw Error('unused value ran');"),
    ] {
        report.generated.push(Definition {
            symbol: SymbolId(symbol),
            javascript: javascript.into(),
            dependencies: BTreeSet::new(),
            function: symbol == 0,
            registration: None,
            cycle_initialization: None,
        });
    }
    let script = assemble_for_repl(&graph, report, "value", "number", false).unwrap();
    let output = Command::new("node").args(["-e", &script]).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42 : number\n");
    assert!(output.stderr.is_empty());
}
