use planexpo_elm::{
    js_names::Layouts,
    kernel::Mode,
    module_codegen,
    names::{self, Environment, Symbols},
    parser::parse,
};
use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn production_functions_keep_partial_application_captures_and_argument_patterns() {
    for arity in [2, 3, 9, 10, 100] {
        let args = (0..arity)
            .map(|i| format!("a{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let source = format!(
            "module Main exposing (..)\npick {args} = a{}\nouter captured first second = let inner a b = captured in inner first second\nlambda = \\a b -> a\nselect (first,second) {{value}} = value\n",
            arity - 1
        );
        let ast = parse(&source).unwrap();
        let mut symbols = Symbols::default();
        let (_, resolved) = names::resolve(
            &ast,
            "application:Main",
            Environment::default(),
            &mut symbols,
        )
        .unwrap();
        let layouts = Layouts::default();
        let definitions = module_codegen::emit(
            &ast,
            "application:Main",
            &resolved,
            &symbols,
            &layouts,
            Mode::Production,
        )
        .unwrap();
        assert_eq!(definitions[0].javascript.matches("function(").count(), 1);
        let mut script = planexpo_elm::linker::helpers();
        for definition in definitions {
            script.push_str(&definition.javascript);
        }
        script.push_str(&format!(r#"
var partial=$author$project$Main$pick;
for(var i=0;i<{};i++)partial=partial(i);
if(partial(71)!==71||partial(99)!==99)throw Error('partial reuse');
if(A3($author$project$Main$outer,71,1,2)!==71||A3($author$project$Main$outer,99,1,2)!==99)throw Error('local capture');
if(A2($author$project$Main$lambda,17,18)!==17)throw Error('lambda');
if(A2($author$project$Main$select,{{a:1,b:2}},{{value:11}})!==11)throw Error('argument patterns');
"#,arity-1));
        let result = Command::new("node").args(["-e", &script]).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
#[test]
fn wide_union_constructors_and_patterns_are_shallow_and_preserve_field_order() {
    for arity in [27, 54, 260, 2200] {
        let arguments = vec!["a"; arity].join(" ");
        let patterns = (0..arity)
            .map(|i| format!("v{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let source = format!(
            "module Main exposing (..)\ntype Wide a = Wide {arguments}\nlast (Wide {patterns}) = v{}\n",
            arity - 1
        );
        let ast = parse(&source).unwrap();
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
        for mode in [Mode::Development, Mode::Production] {
            let definitions = module_codegen::emit(
                &ast,
                "application:Main",
                &resolved,
                &symbols,
                &layouts,
                mode,
            )
            .unwrap();
            assert_eq!(definitions[0].javascript.matches("function(").count(), 1);
            let fields = (0..arity)
                .map(planexpo_elm::js_names::kernel_field)
                .collect::<Vec<_>>();
            let mut script = planexpo_elm::linker::helpers();
            for definition in definitions {
                script.push_str(&definition.javascript);
            }
            script.push_str(&format!(r#"
var fields={};
var prefix=$author$project$Main$Wide;
for(var i=0;i<{};i++)prefix=prefix(i);
var first=prefix(71),second=prefix(99);
for(var i=0;i<{};i++)if(first[fields[i]]!==i||second[fields[i]]!==i)throw Error('field order');
if($author$project$Main$last(first)!==71||$author$project$Main$last(second)!==99)throw Error('pattern or partial reuse');
"#, serde_json::to_string(&fields).unwrap(), arity-1, arity-1));
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
            let result = child.wait_with_output().unwrap();
            assert!(
                result.status.success(),
                "arity {arity}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}
#[test]
fn complete_module_definitions_execute_in_dependency_order() {
    let source = r#"module Main exposing (..)
output = unwrap made
made = Box ((Pair 7) "second")
unwrap (Box record) = record
type Box a = Box a
type alias Pair = { z : Int, a : String }
"#;
    let ast = parse(source).unwrap();
    let mut symbols = Symbols::default();
    let mut environment = Environment::default();
    // The code generator consumes resolved identities, independent of inference.
    // Supply only the two nominal types needed for this isolated module fixture.
    let mut basics = names::Interface::default();
    for name in ["Int", "String"] {
        let id = symbols.intern(
            "elm/core:Basics",
            name,
            names::Space::Type,
            names::SymbolKind::Type {
                arity: 0,
                alias: false,
            },
        );
        basics.types.insert(name.into(), id);
    }
    environment
        .import(
            "Basics",
            &basics,
            &planexpo_elm::module::Exposing::All,
            &symbols,
        )
        .unwrap();
    let (_, resolved) =
        names::resolve(&ast, "application:Main", environment, &mut symbols).unwrap();
    let mut layouts = Layouts::default();
    layouts
        .register(&ast, "application:Main", &symbols)
        .unwrap();
    for mode in [Mode::Development, Mode::Production] {
        let definitions = module_codegen::emit(
            &ast,
            "application:Main",
            &resolved,
            &symbols,
            &layouts,
            mode,
        )
        .unwrap();
        assert_eq!(definitions.len(), 5);
        let ordered = module_codegen::order(definitions).unwrap();
        assert_eq!(
            symbols.get(ordered.last().unwrap().symbol).name.as_ref(),
            "output"
        );
        let mut script = planexpo_elm::linker::helpers();
        script.extend(ordered.iter().map(|d| d.javascript.as_str()));
        script.push_str("console.log(JSON.stringify($author$project$Main$output));");
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
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            serde_json::json!({"z":7,"a":"second"})
        );
    }
}

#[test]
fn effect_registration_is_deferred_and_implicit_leaves_share_the_manager() {
    for (clause, command, subscription) in [
        ("command = Cmd", true, false),
        ("subscription = Sub", false, true),
        ("command = Cmd, subscription = Sub", true, true),
    ] {
        let source = format!(
            "effect module Manager where {{ {clause} }} exposing (..)\ntype Cmd a = Cmd a\ntype Sub a = Sub a\ninit=7\nonEffects router effects state = state\nonSelfMsg router message state = state\ncmdMap f value = value\nsubMap f value = value"
        );
        let ast = parse(&source).unwrap();
        let mut symbols = Symbols::default();
        let (_, resolved) = names::resolve(
            &ast,
            "elm/test:Manager",
            Environment::default(),
            &mut symbols,
        )
        .unwrap();
        let mut layouts = Layouts::default();
        layouts
            .register(&ast, "elm/test:Manager", &symbols)
            .unwrap();
        let ordered = module_codegen::order(
            module_codegen::emit(
                &ast,
                "elm/test:Manager",
                &resolved,
                &symbols,
                &layouts,
                Mode::Development,
            )
            .unwrap(),
        )
        .unwrap();
        let registrations: Vec<_> = ordered
            .iter()
            .filter_map(|d| d.registration.as_ref())
            .collect();
        assert_eq!(
            registrations.len(),
            usize::from(command) + usize::from(subscription)
        );
        assert_eq!(
            registrations[0].dependencies.len(),
            3 + usize::from(command) + usize::from(subscription)
        );
        let mut script = String::from(
            "var _Platform_effectManagers={}; function _Platform_leaf(home){return function(value){return {home:home,value:value};};} function _Platform_createManager(init,onEffects,onSelfMsg,cmdMap,subMap){return {init:init,onEffects:onEffects,onSelfMsg:onSelfMsg,cmdMap:cmdMap,subMap:subMap};}\n",
        );
        for definition in &ordered {
            script.push_str(&definition.javascript);
        }
        script.push_str("if(Object.keys(_Platform_effectManagers).length)throw new Error('early registration');\n");
        script.push_str(&registrations[0].javascript);
        script.push_str(&format!("var m=_Platform_effectManagers.Manager;if(m.init!==7 || Boolean(m.cmdMap)!=={command} || Boolean(m.subMap)!=={subscription})throw new Error('wrong lifecycle slots');\n"));
        for (name, enabled) in [("command", command), ("subscription", subscription)] {
            if enabled {
                script.push_str(&format!("var leaf=$elm$test$Manager${name}(9);if(leaf.home!=='Manager'||leaf.value!==9)throw new Error('bad leaf');\n"));
            }
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
}

#[test]
fn recursive_values_preserve_deferred_references_and_cached_identity() {
    let source = "module Main exposing (..)\nleft = { next = \\_ -> right }\nright = { next = \\_ -> left }\nresult = left.next 0";
    let ast = parse(source).unwrap();
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
    let definitions = module_codegen::emit(
        &ast,
        "application:Main",
        &resolved,
        &symbols,
        &layouts,
        Mode::Development,
    )
    .unwrap();
    assert_eq!(
        definitions
            .iter()
            .filter(|d| d.cycle_initialization.is_some())
            .count(),
        2
    );
    let ordered = module_codegen::order(definitions).unwrap();
    let mut script = ordered
        .iter()
        .map(|d| d.javascript.as_str())
        .collect::<String>();
    script.push_str("if($author$project$Main$result!==$author$project$Main$right || $author$project$Main$right.next(0)!==$author$project$Main$left)throw Error('lost recursive value identity');");
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
fn large_record_constructor_is_shallow_and_partial_applications_are_reusable() {
    let fields = (0..2200)
        .map(|i| format!("f{i}:()"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!("module Main exposing (..)\ntype alias Large = {{ {fields} }}");
    let ast = parse(&source).unwrap();
    let mut symbols = Symbols::default();
    let (_, resolved) = names::resolve(
        &ast,
        "application:Main",
        Environment::default(),
        &mut symbols,
    )
    .unwrap();
    let layouts = Layouts::default();
    let definitions = module_codegen::emit(
        &ast,
        "application:Main",
        &resolved,
        &symbols,
        &layouts,
        Mode::Development,
    )
    .unwrap();
    assert_eq!(definitions[0].javascript.matches("function(").count(), 1);
    let mut script = planexpo_elm::linker::helpers();
    script.push_str(&definitions[0].javascript);
    script.push_str("var prefix=$author$project$Main$Large;for(var i=0;i<2199;i++)prefix=prefix(i);var a=prefix(7),b=prefix(9);if(a.f0!==0||a.f2198!==2198||a.f2199!==7||b.f2199!==9)throw Error('constructor order or sharing');");
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
fn production_return_branches_avoid_iifes_and_preserve_selected_closures() {
    let source = r#"module Main exposing (..)
choose condition input =
    if condition then
        case input of
            (first, second) ->
                let
                    saved = first
                in
                \_ -> saved
    else
        case input of
            (first, second) ->
                let
                    saved = second
                in
                \_ -> saved
"#;
    let ast = parse(source).unwrap();
    let mut symbols = Symbols::default();
    let (_, resolved) = names::resolve(
        &ast,
        "application:Main",
        Environment::default(),
        &mut symbols,
    )
    .unwrap();
    let layouts = Layouts::default();
    let definitions = module_codegen::emit(
        &ast,
        "application:Main",
        &resolved,
        &symbols,
        &layouts,
        Mode::Production,
    )
    .unwrap();
    let js = &definitions[0].javascript;
    // One exported function and the two returned closures: no case/let IIFEs.
    assert_eq!(js.matches("function(").count(), 3, "{js}");
    assert!(!js.contains("while(true)"));
    let mut script = planexpo_elm::linker::helpers();
    script.push_str(js);
    script.push_str(
        r#"
var choose=$author$project$Main$choose;
var partial=choose(true), a=partial({a:11,b:12}), b=partial({a:21,b:22});
var c=A2(choose,false,{a:31,b:32});
if(a(0)!==11||b(0)!==21||c(0)!==32||a(0)!==11)throw Error('branch capture');
"#,
    );
    let result = Command::new("node").args(["-e", &script]).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn standalone_generation_rejects_missing_and_redundant_cases() {
    for (branches, expected) in [
        ("A -> 1", "missing patterns"),
        ("_ -> 1\n        A -> 2", "redundant"),
    ] {
        let source = format!(
            "module Main exposing (..)\ntype Choice = A | B\nf x =\n    case x of\n        {branches}\n"
        );
        let ast = parse(&source).unwrap();
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
        for mode in [Mode::Development, Mode::Production, Mode::Debug] {
            let error = module_codegen::emit(
                &ast,
                "application:Main",
                &resolved,
                &symbols,
                &layouts,
                mode,
            )
            .unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }
}
