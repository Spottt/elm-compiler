use planexpo_elm::js_names::{field, global};
#[test]
fn kernel_fields_match_elms_public_runtime_layout_without_collisions() {
    use planexpo_elm::js_names::kernel_field;
    for (index, name) in [
        (0, "a"),
        (2, "c"),
        (4, "e"),
        (25, "z"),
        (26, "A"),
        (51, "Z"),
        (52, "_"),
        (53, "aa"),
        (116, "a9"),
        (117, "ba"),
        (3506, "aaa"),
    ] {
        assert_eq!(kernel_field(index), name);
    }
    let fields = (0..10_000)
        .map(kernel_field)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(fields.len(), 10_000);
    for reserved in ["do", "if", "in", "for", "var", "let", "$", "int", "NaN"] {
        assert!(!fields.contains(reserved), "{reserved}");
    }
}
#[test]
fn global_names_preserve_package_and_module_identity() {
    assert_eq!(
        global("application:Admin.Main", "view").unwrap(),
        "$author$project$Admin$Main$view"
    );
    assert_eq!(
        global("elm-explorations/web-gl:WebGL.Settings", "blend").unwrap(),
        "$elm_explorations$web_gl$WebGL$Settings$blend"
    );
    assert_eq!(field("return"), "_return");
    assert_eq!(field("A2"), "_A2");
    assert_eq!(field("value"), "value");
    assert!(global("unknown", "value").is_err());
}

#[test]
fn constructors_support_test_runner_instrumentation_and_partial_application() {
    use planexpo_elm::{js_names::Layouts, kernel::Mode, names, parser};
    let ast = parser::parse("module Main exposing (..)\ntype Test a = UnitTest a | Labeled a a\n")
        .unwrap();
    let mut symbols = names::Symbols::default();
    names::resolve(
        &ast,
        "application:Main",
        names::Environment::default(),
        &mut symbols,
    )
    .unwrap();
    let mut layouts = Layouts::default();
    layouts
        .register(&ast, "application:Main", &symbols)
        .unwrap();
    let mut script =
        String::from("function F2(f){return function(a){return function(b){return f(a,b);};};}\n");
    for name in ["UnitTest", "Labeled"] {
        let id = symbols
            .lookup("application:Main", name, names::Space::Constructor)
            .unwrap();
        let expression = layouts
            .constructor_expression(id, &symbols, Mode::Development)
            .unwrap();
        // elm-test inserts a symbol immediately after this matched object opening.
        script.push_str(&format!("var {name}={expression};\n"));
    }
    let source = serde_json::to_string(&script).unwrap();
    script.push_str(&format!(r#"
var source={source};
var pattern=/^var\s+(UnitTest|Labeled)\s*=\s*(?:\w+\(\s*)?function\s*\([\w, ]*\)\s*\{{\s*return *\{{/gm;
if(Array.from(source.matchAll(pattern)).length!==2)throw Error('test runner cannot instrument constructors');
if(UnitTest(3).a!==3)throw Error('unary constructor');
var partial=Labeled('label'), first=partial(1), second=partial(2);
if(first.a!=='label'||first.b!==1||second.b!==2)throw Error('partial application reuse');
"#));
    let result = std::process::Command::new("node")
        .args(["-e", &script])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
