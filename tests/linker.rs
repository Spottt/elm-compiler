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
        import_errors: Vec::new(),
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
        }.into());
    }
    let script = assemble_for_repl(&graph, report, "value", "number", false).unwrap();
    let output = Command::new("node").args(["-e", &script]).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42 : number\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn grouped_entry_linking_keeps_only_its_exports_and_flag_dependencies() {
    use planexpo_elm::{analyze::{EntryOutput, Report}, linker::assemble_entry, module_codegen::Definition, names::SymbolId, project::Graph};
    use std::{collections::BTreeSet, rc::Rc};
    let graph = Graph { entry: "application:A".into(), entries: vec!["application:A".into()], modules: vec![], manifests: vec![], import_errors: vec![] };
    let definition = |id, javascript: &str| Rc::new(Definition { symbol: SymbolId(id), javascript: javascript.into(), dependencies: BTreeSet::new(), function: true, registration: None, cycle_initialization: None });
    let mut report = Report {
        generated: vec![definition(1, "var mainA=1;\n"), definition(2, "var decodeFlags=2;\n"), definition(3, "var mainB=3;\n")],
        link_roots: [SymbolId(1), SymbolId(2), SymbolId(3)].into(),
        main_export: Some("scope.A=mainA;scope.B=mainB;\n".into()),
        ..Report::default()
    };
    report.entry_outputs.insert("application:A".into(), EntryOutput { roots: [SymbolId(1), SymbolId(2)].into(), javascript: "scope.A=mainA+decodeFlags;\n".into() });
    report.entry_outputs.insert("application:B".into(), EntryOutput { roots: [SymbolId(3)].into(), javascript: "scope.B=mainB;\n".into() });
    let output = assemble_entry(&graph, report.clone()).unwrap();
    assert!(output.contains("var mainA=1;"));
    assert!(output.contains("var decodeFlags=2;"));
    assert!(output.contains("scope.A=mainA+decodeFlags;"));
    assert!(!output.contains("mainB"));
    let mut other = graph.clone(); other.entry = "application:B".into(); other.entries = vec![other.entry.clone()];
    let output = assemble_entry(&other, report.clone()).unwrap();
    assert!(output.contains("scope.B=mainB;"));
    assert!(!output.contains("mainA")); assert!(!output.contains("decodeFlags"));
    other.entries.push(graph.entry.clone());
    assert!(assemble_entry(&other, report.clone()).is_err());
    let mut debug = report.clone(); debug.mode = planexpo_elm::kernel::Mode::Debug;
    assert!(assemble_entry(&graph, debug).is_err());
    report.entry_outputs.clear();
    assert!(assemble_entry(&graph, report).is_err());
}

#[test]
fn sparse_symbol_reachability_preserves_input_order_across_hash_seeds() {
    use planexpo_elm::{linker::reachable, module_codegen::{Definition, Registration}, names::SymbolId};
    use std::collections::BTreeSet;
    // A reachable cycle, an effect registration into another cycle, an external
    // kernel symbol, and a disconnected cycle. IDs deliberately remain sparse.
    let id = |n: u32| SymbolId(n * 100_003);
    let definitions: Vec<_> = (0..64u32).rev().map(|n| Definition {
        symbol: id(n),
        javascript: format!("var v{n}=0;"),
        dependencies: [id(if n < 16 { (n + 1) % 16 } else if n < 32 { 16 + (n + 1) % 16 } else { 32 + (n + 1) % 32 })].into(),
        function: true,
        registration: (n == 7).then(|| Registration {
            javascript: "register();".into(),
            dependencies: [id(20), SymbolId(u32::MAX)].into(),
        }),
        cycle_initialization: None,
    }).collect();
    let expected: Vec<_> = (0..32).rev().map(id).collect();
    for _ in 0..16 {
        let retained = reachable(definitions.clone(), BTreeSet::from([id(0)]));
        assert_eq!(retained.iter().map(|d| d.symbol).collect::<Vec<_>>(), expected);
    }
}

#[test]
fn borrowed_entry_linking_does_not_consume_exports_or_cycle_initializers() {
    use planexpo_elm::{analyze::{EntryOutput, Report}, linker::assemble_entry_shared, module_codegen::Definition, names::SymbolId, project::Graph};
    let graph = Graph { entry: "application:A".into(), entries: vec!["application:A".into()], modules: vec![], manifests: vec![], import_errors: vec![] };
    let mut report = Report::default();
    report.generated.push(Definition {
        symbol: SymbolId(1), javascript: "var value;\n".into(), dependencies: [SymbolId(1)].into(),
        function: false, registration: None, cycle_initialization: Some("value=42;\n".into()),
    }.into());
    report.entry_outputs.insert(graph.entry.clone(), EntryOutput { roots: [SymbolId(1)].into(), javascript: "scope.result=value;\n".into() });
    let first = assemble_entry_shared(&graph, &report).unwrap();
    let second = assemble_entry_shared(&graph, &report).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.matches("value=42;").count(), 1);
    assert!(first.contains("scope.result=value;"));
    assert_eq!(report.generated[0].javascript, "var value;\n");
    assert_eq!(report.generated[0].cycle_initialization.as_deref(), Some("value=42;\n"));
    assert_eq!(report.entry_outputs.len(), 1);
}
