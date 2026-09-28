use planexpo_elm::{
    ast::{Declaration, Expr},
    codegen::{self, Reference},
    js_names::Layouts,
    kernel::Mode,
    module_codegen,
    names::{self, Environment, Symbols},
    parser::parse,
};
use std::process::Command;
#[test]
fn optimized_primitives_preserve_evaluation_and_structural_semantics() {
    let mut outputs = Vec::new();
    for source in [
        "eq (left ()) (right ())",
        "neq 2 3",
        "idiv -7 2",
        "idiv 7 0",
        "lt 2 3",
        "ge 3 3",
        "append (left ()) (right ())",
        "truncate -2.9",
        "not yes",
        "toFloat 7",
        "[]",
    ] {
        let source = format!("value = {source}");
        let ast = parse(&source).unwrap();
        let Declaration::Value { body, .. } = ast.declarations[0] else {
            panic!()
        };
        let js = codegen::expression(
            &ast,
            body,
            Mode::Production,
            |id| {
                let Expr::Var(name) = &ast.expressions[id.0 as usize].kind else {
                    panic!()
                };
                Ok(Reference {
                    name: if ["left", "right", "yes"].contains(name) {
                        format!("${name}")
                    } else {
                        format!("$elm$core$Basics${name}")
                    },
                    kernel: false,
                })
            },
            |name| Ok(name.into()),
        )
        .unwrap();
        assert!(!js.contains("A2("));
        outputs.push(js);
    }
    let script = format!(
        r#"
var events=[], $yes=true, _List_Nil={{$:0}};
function $left(){{events.push('left');return [1,2];}}
function $right(){{events.push('right');return [1,2];}}
function _Utils_eq(a,b){{return JSON.stringify(a)===JSON.stringify(b);}}
function _Utils_cmp(a,b){{return a<b?-1:a>b?1:0;}}
function _Utils_ap(a,b){{return a.concat(b);}}
console.log(JSON.stringify([{}]));
if(events.join(',')!=='left,right,left,right')throw Error('evaluation order');
"#,
        outputs.join(",")
    );
    let result = Command::new("node").args(["-e", &script]).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap(),
        serde_json::json!([true,true,-3,0,true,true,[1,2,1,2],-2,false,7,{"$":0}])
    );
}
fn generate(source: &str, mode: Mode) -> Result<String, String> {
    let ast = parse(source).map_err(|e| e.to_string())?;
    let mut symbols = Symbols::default();
    let (_, resolved) = names::resolve(
        &ast,
        "application:Main",
        Environment::default(),
        &mut symbols,
    )?;
    let mut layouts = Layouts::default();
    layouts.register(&ast, "application:Main", &symbols)?;
    Ok(module_codegen::emit(
        &ast,
        "application:Main",
        &resolved,
        &symbols,
        &layouts,
        mode,
    )?
    .into_iter()
    .map(|d| d.javascript)
    .collect())
}
#[test]
fn checked_cases_use_defaults_switches_and_numeric_enums() {
    let source = "module Main exposing (..)\ntype Color = Red | Green | Blue\nchoose c = case c of\n Red -> 1\n Green -> 2\n Blue -> 3\ntext s = case s of\n \"x\" -> Red\n \"y\" -> Green\n _ -> Blue\n";
    let js = generate(source, Mode::Production).unwrap();
    assert!(js.contains("switch("));
    assert!(!js.contains("unreachable Elm pattern"));
    assert!(js.contains("$Main$Red=0;"));
    let script = format!(
        "{js}\nif($author$project$Main$choose($author$project$Main$text('y'))!==2||$author$project$Main$choose($author$project$Main$text('z'))!==3)throw Error('case');"
    );
    let result = Command::new("node").args(["-e", &script]).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        generate(source, Mode::Development)
            .unwrap()
            .contains("({$:\"Red\"})")
    );
    assert!(
        generate(
            "module Main exposing (..)\ntype Color = Red | Blue\nchoose c = case c of\n Red -> 1\n",
            Mode::Production
        )
        .is_err()
    );
}

#[test]
fn pipes_flatten_partial_calls_and_string_append_keeps_list_fallback() {
    for (source, expected) in [
        ("apR 7 (add 3)", "10"),
        ("apL (add 3) 7", "10"),
        ("append \"a\" \"b\"", "\"ab\""),
        ("append [1] [2]", "[1,2]"),
        ("append (append [1] [2]) (append [3] [4])", "[1,2,3,4]"),
    ] {
        let source = format!("value = {source}");
        let ast = parse(&source).unwrap();
        let Declaration::Value { body, .. } = ast.declarations[0] else {
            panic!()
        };
        let js = codegen::expression(
            &ast,
            body,
            Mode::Production,
            |id| {
                let Expr::Var(name) = &ast.expressions[id.0 as usize].kind else {
                    panic!()
                };
                Ok(Reference {
                    name: format!("$elm$core$Basics${name}"),
                    kernel: false,
                })
            },
            |n| Ok(n.into()),
        )
        .unwrap();
        assert!(!js.contains("$Basics$ap"));
        let script = format!(
            "function _List_fromArray(a){{return a;}} function _Utils_ap(a,b){{return a.concat(b);}} console.log(JSON.stringify({js}));"
        );
        let out = Command::new("node").args(["-e", &script]).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), expected);
    }
}
#[test]
fn single_field_unions_unbox_and_enum_cases_switch_in_production_only() {
    let source = "module Main exposing (..)\ntype Wrap a = Wrap a\ntype Choice = A | B | C\nunpack (Wrap x) = x\nchoose x = case x of\n A -> 1\n B -> 2\n C -> 3\nresult = unpack (Wrap (choose B))\n";
    let js = generate(source, Mode::Production).unwrap();
    assert!(js.contains("switch("));
    assert!(js.contains("function(value){return value;}"));
    let out=Command::new("node").args(["-e",&format!("{js}\nif($author$project$Main$result!==2||$author$project$Main$Wrap(17)!==17)throw Error('unboxing');")]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        generate(source, Mode::Development)
            .unwrap()
            .contains("return {$:\"Wrap\",a:c0}")
    );
}

#[test]
fn nested_case_prefixes_are_shared_without_changing_branch_priority() {
    let source = "module Main exposing (..)\ntype Inner = I Int | J Int\ntype Outer = Box Inner | Empty\npick x = case x of\n Box (I 0) -> 10\n Box (I _) -> 20\n Box (J _) -> 30\n Empty -> 40\n";
    // Provide the nominal Int used by the isolated declaration fixture.
    let ast = parse(source).unwrap();
    let mut symbols = Symbols::default();
    let mut env = Environment::default();
    let mut basics = names::Interface::default();
    let id = symbols.intern(
        "elm/core:Basics",
        "Int",
        names::Space::Type,
        names::SymbolKind::Type {
            arity: 0,
            alias: false,
        },
    );
    basics.types.insert("Int".into(), id);
    env.import(
        "Basics",
        &basics,
        &planexpo_elm::module::Exposing::All,
        &symbols,
    )
    .unwrap();
    let (_, resolved) = names::resolve(&ast, "application:Main", env, &mut symbols).unwrap();
    let mut layouts = Layouts::default();
    layouts
        .register(&ast, "application:Main", &symbols)
        .unwrap();
    let js: String = module_codegen::emit(
        &ast,
        "application:Main",
        &resolved,
        &symbols,
        &layouts,
        Mode::Production,
    )
    .unwrap()
    .into_iter()
    .map(|d| d.javascript)
    .collect();
    assert_eq!(js.matches(".a.$===0").count(), 1, "{js}");
    let out=Command::new("node").args(["-e",&format!("{js}\nvar p=$author$project$Main$pick,b=$author$project$Main$Box,i=$author$project$Main$I,j=$author$project$Main$J;if(JSON.stringify([p(b(i(0))),p(b(i(4))),p(b(j(0))),p($author$project$Main$Empty)])!=='[10,20,30,40]')throw Error('priority');")]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn string_chains_flatten_and_preserve_each_operand_once_in_order() {
    let source = "value = append (left ()) (append \"middle\" (right ()))";
    let ast = parse(source).unwrap();
    let Declaration::Value { body, .. } = ast.declarations[0] else {
        panic!()
    };
    let js = codegen::expression(
        &ast,
        body,
        Mode::Production,
        |id| {
            let Expr::Var(name) = &ast.expressions[id.0 as usize].kind else {
                panic!()
            };
            Ok(Reference {
                name: if *name == "append" {
                    "$elm$core$Basics$append".into()
                } else {
                    format!("${name}")
                },
                kernel: false,
            })
        },
        |n| Ok(n.into()),
    )
    .unwrap();
    assert!(!js.contains("_Utils_ap"));
    let out=Command::new("node").args(["-e",&format!("var seen=[];function $left(){{seen.push(1);return 'L';}}function $right(){{seen.push(2);return 'R';}}if(({js})!=='LmiddleR'||seen.join(',')!=='1,2')throw Error('concat order');")]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn enum_references_are_literals_and_do_not_keep_constructor_dependencies() {
    let ast = parse("module Main exposing (..)\ntype Color = Red | Blue\nvalue = Blue\n").unwrap();
    let mut symbols = Symbols::default();
    let (_, resolved) = names::resolve(
        &ast,
        "application:Main",
        Environment::default(),
        &mut symbols,
    )
    .unwrap();
    let mut layouts = Layouts::default();
    layouts
        .register(&ast, "application:Main", &symbols)
        .unwrap();
    let defs = module_codegen::emit(
        &ast,
        "application:Main",
        &resolved,
        &symbols,
        &layouts,
        Mode::Production,
    )
    .unwrap();
    let value = defs
        .iter()
        .find(|d| symbols.get(d.symbol).name.as_ref() == "value")
        .unwrap();
    assert!(value.javascript.contains("$value=1;"));
    assert!(value.dependencies.is_empty());
    let root = value.symbol;
    assert_eq!(
        planexpo_elm::linker::reachable(defs, std::collections::BTreeSet::from([root])).len(),
        1
    );
}

#[test]
fn simple_case_on_stable_value_needs_no_extra_function_and_keeps_lazy_branches() {
    let source = "module Main exposing (..)\ntype Choice = A | B\nresult x = (case x of\n A -> 1\n B -> 2\n , 7)\n";
    let js = generate(source, Mode::Production).unwrap();
    assert_eq!(js.matches("function(").count(), 1, "{js}");
    let script = format!(
        "function _Utils_Tuple2(a,b){{return {{a:a,b:b}};}}{js}\nif($author$project$Main$result(0).a!==1||$author$project$Main$result(1).a!==2)throw Error('case');"
    );
    let out = Command::new("node").args(["-e", &script]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
