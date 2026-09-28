use planexpo_elm::{
    ast::{Declaration, Pattern, PatternId},
    codegen::expression_with_definitions,
    kernel::Mode,
    names::{self, Environment, Symbols},
    parser::parse,
    pattern_codegen::{self, Names},
};
use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn resolved_patterns_lambdas_and_cases_execute_in_both_modes() {
    let fixtures = [
        ("let\n    x = y\n    y = 7\nin x", 7),
        ("let\n    x = f 9\n    f a = a\nin x", 9),
        ("let\n    x = a\n    (a,b) = (8,9)\nin x", 8),
        (
            "let\n    f xs = case xs of\n        [] -> 4\n        h :: t -> g t\n    g xs = f xs\nin f [1,2]",
            4,
        ),
        (
            "let\n    f x =\n        let\n            g y = x\n        in g\nin (f 6) 7",
            6,
        ),
        ("(\\x -> x) 4", 4),
        ("((\\x -> \\y -> x) 4) 9", 4),
        ("(\\(x,y) -> y) (3,7)", 7),
        ("(\\{value} -> value) {value=8}", 8),
        (
            "case [4,5] of\n    [] -> 0\n    [x] -> x\n    x :: rest -> x",
            4,
        ),
        (
            "case [[1],[]] of\n    [[x],[y]] -> y\n    [a,[]] -> 8\n    _ -> 0",
            8,
        ),
        ("case (2,7) of\n    ((x,y) as pair) -> (.b) {b=y}", 7),
        ("case Box 9 of\n    Box x -> x", 9),
        ("case 'a' of\n    'a' -> 3\n    _ -> 0", 3),
    ];
    for mode in [Mode::Development, Mode::Production] {
        let mut emitted = Vec::new();
        let mut constructor = String::new();
        let mut expected = Vec::new();
        for (body, result) in fixtures {
            let indented = body
                .lines()
                .map(|line| format!("    {line}"))
                .collect::<Vec<_>>()
                .join("\n");
            let source = format!("type Box a = Box a\nresultValue =\n{indented}");
            let ast = parse(&source).unwrap();
            let mut symbols = Symbols::default();
            let mut env = Environment::default();
            env.builtin_list(&mut symbols);
            let (_, resolved) =
                names::resolve(&ast, "application:Main", env, &mut symbols).unwrap();
            let Declaration::Value { body, .. } = ast.declarations[1] else {
                panic!()
            };
            let mut layouts = planexpo_elm::js_names::Layouts::default();
            layouts
                .register(&ast, "application:Main", &symbols)
                .unwrap();
            let mut naming = planexpo_elm::js_names::Naming {
                resolved: &resolved,
                symbols: &symbols,
                layouts: &layouts,
                mode,
            };
            constructor = layouts
                .constructor_expression(
                    symbols
                        .lookup(
                            "application:Main",
                            "Box",
                            planexpo_elm::names::Space::Constructor,
                        )
                        .unwrap(),
                    &symbols,
                    mode,
                )
                .unwrap();
            let references = planexpo_elm::js_names::Naming {
                resolved: &resolved,
                symbols: &symbols,
                layouts: &layouts,
                mode,
            };
            emitted.push(
                expression_with_definitions(
                    &ast,
                    body,
                    mode,
                    |id| references.reference(id),
                    |n| Ok(n.into()),
                    |id, value| pattern_codegen::plan(&ast, id, value, mode, &mut naming),
                    |id| {
                        planexpo_elm::codegen::local_definitions(&ast, &resolved, id, |local| {
                            format!("$l{}", local.0)
                        })
                    },
                )
                .unwrap(),
            );
            expected.push(result);
        }
        let (nil, cons) = if matches!(mode, Mode::Development) {
            ("'[]'", "'::'")
        } else {
            ("0", "1")
        };
        let runtime = format!(
            "function A2(f,a,b){{return f(a)(b);}} function _Utils_Tuple2(a,b){{return {{a:a,b:b}};}} function _Utils_chr(c){{return new String(c);}} var $author$project$Main$Box={constructor}; var _List_Nil={{$:{nil}}}; function _List_fromArray(a){{var r={{$:{nil}}};for(var i=a.length;i--;)r={{$:{cons},a:a[i],b:r}};return r;}}"
        );
        let script = format!(
            "{}{runtime}\nconsole.log(JSON.stringify([{}]));",
            planexpo_elm::linker::helpers(),
            emitted.join(",")
        );
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
            serde_json::from_slice::<Vec<i32>>(&output.stdout).unwrap(),
            expected
        );
    }
}
#[test]
fn constructor_layout_must_cover_every_argument() {
    struct Bad;
    impl Names for Bad {
        fn local(&mut self, _: PatternId, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn field(&mut self, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn constructor(&mut self, _: PatternId, _: &str) -> Result<(String, Vec<String>), String> {
            Ok(("true".into(), vec![]))
        }
    }
    let ast = parse("f (Box x) = x").unwrap();
    let id = ast
        .patterns
        .iter()
        .position(|p| matches!(p.kind, Pattern::Constructor(..)))
        .unwrap();
    assert!(
        pattern_codegen::plan(
            &ast,
            PatternId(id as u32),
            "$x",
            Mode::Development,
            &mut Bad
        )
        .is_err()
    );
}
