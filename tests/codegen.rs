use planexpo_elm::{
    ast::{Declaration, Expr},
    codegen::{Reference, expression},
    kernel::Mode,
    parser::parse,
};
use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn generated_composites_execute_with_curried_and_kernel_calls() {
    let fixtures = [
        ("(1, 2, 3)", serde_json::json!({"a":1,"b":2,"c":3})),
        ("[1, 2, 3]", serde_json::json!([1, 2, 3])),
        ("if yes then 7 else fail ()", serde_json::json!(7)),
        ("add 3 4", serde_json::json!(7)),
        ("(add 3) 4", serde_json::json!(7)),
        ("kernel 3 4", serde_json::json!(7)),
        ("(.value) {value=9}", serde_json::json!(9)),
        ("{value=9}.value", serde_json::json!(9)),
        (
            "{ record | value = 5 }",
            serde_json::json!({"renamed_value":5,"untouched":2}),
        ),
        ("-42", serde_json::json!(-42)),
    ];
    let mut generated = Vec::new();
    let mut expected = Vec::new();
    for (body, result) in fixtures {
        let source = format!("value = {body}");
        let ast = parse(&source).unwrap();
        let Declaration::Value { body, .. } = ast.declarations[0] else {
            panic!()
        };
        generated.push(
            expression(
                &ast,
                body,
                Mode::Production,
                |id| {
                    let name = match &ast.expressions[id.0 as usize].kind {
                        Expr::Var(name) => *name,
                        Expr::Record {
                            base: Some(name), ..
                        } => *name,
                        _ => return Err("unexpected reference".into()),
                    };
                    Ok(Reference {
                        name: format!("${name}"),
                        kernel: name == "kernel",
                    })
                },
                |name| Ok(format!("renamed_{name}")),
            )
            .unwrap(),
        );
        expected.push(result);
    }
    // Runtime shims implement the same call/record conventions; this test
    // exercises generated control flow and application, not kernel correctness.
    let runtime = r#"
function A2(f,a,b) { return f.a === 2 ? f.f(a,b) : f(a)(b); }
function _Utils_Tuple3(a,b,c) { return {a:a,b:b,c:c}; }
function _List_fromArray(a) { return a; }
function _Utils_update(old,fields) { return Object.assign({},old,fields); }
var $yes=true, $record={renamed_value:1,untouched:2};
function $fail() { throw new Error('unselected branch evaluated'); }
function $add(a) { return function(b) { return a+b; }; }
function $kernel(a) { return function(b) { return a+b; }; }
"#;
    let script = format!(
        "{runtime}\nconsole.log(JSON.stringify([{}])); if ($record.renamed_value !== 1) throw new Error('record mutated');",
        generated.join(",")
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
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::json!(expected)
    );
}
#[test]
fn emission_does_not_recurse_on_deep_expression_arenas() {
    use planexpo_elm::ast::{ExprId, Node, Span};
    let mut ast = parse("x=0").unwrap();
    let mut root = ExprId(0);
    for _ in 0..20_000 {
        let id = ExprId(ast.expressions.len() as u32);
        ast.expressions.push(Node {
            span: Span { start: 0, end: 0 },
            kind: Expr::Negate(root),
        });
        root = id;
    }
    let js = expression(
        &ast,
        root,
        Mode::Production,
        |_| Err("unexpected reference".into()),
        |n| Ok(n.into()),
    )
    .unwrap();
    assert_eq!(js.len(), 60_001);
}

#[test]
fn production_arithmetic_uses_canonical_identities_and_preserves_numeric_semantics() {
    for (name, operator) in [("add", "+"), ("sub", "-"), ("mul", "*"), ("fdiv", "/")] {
        for binary in [false, true] {
            for mode in [Mode::Development, Mode::Production] {
                for canonical in [false, true] {
                    let mut ast = parse("value = op left right").unwrap();
                    let Declaration::Value { body, .. } = ast.declarations[0] else {
                        panic!()
                    };
                    if binary {
                        let Expr::Call(_, ref args) = ast.expressions[body.0 as usize].kind else {
                            panic!()
                        };
                        ast.expressions[body.0 as usize].kind = Expr::Binary("+", args[0], args[1]);
                    }
                    let global = if canonical {
                        format!("$elm$core$Basics${name}")
                    } else {
                        format!("$author$project$Basics${name}")
                    };
                    let js = expression(
                        &ast,
                        body,
                        mode,
                        |id| {
                            Ok(Reference {
                                name: match ast.expressions[id.0 as usize].kind {
                                    Expr::Var("left") => "left()".into(),
                                    Expr::Var("right") => "right()".into(),
                                    _ => global.clone(),
                                },
                                kernel: false,
                            })
                        },
                        |name| Ok(name.into()),
                    )
                    .unwrap();
                    assert_eq!(
                        js.contains("A2("),
                        !(canonical && mode == Mode::Production),
                        "{js}"
                    );
                    let script = format!(
                        r#"
function A2(f,a,b){{return f(a)(b);}}
var {global}=a=>b=>a {operator} b;
var trace=[],a,b;
function left(){{trace.push('l');return a;}}
function right(){{trace.push('r');return b;}}
for(var pair of [[7,3],[-7,3],[1.25,-2.5],[0,-0],[-0,-0],[1,0],[0,0],[Infinity,-Infinity],[NaN,2],[2147483647,2],[9007199254740991,2]]){{
[a,b]=pair;trace=[];
var actual={js};
if(!Object.is(actual,a {operator} b)||trace.join('')!=='lr')throw Error('numeric semantics/order');
}}
"#
                    );
                    let result = Command::new("node").args(["-e", &script]).output().unwrap();
                    assert!(
                        result.status.success(),
                        "{}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                }
            }
        }
    }
}
#[test]
fn record_fields_follow_elm_name_order_before_field_renaming() {
    let ast = parse("value = { z = mark \"z\", a = mark \"a\" }").unwrap();
    let Declaration::Value { body, .. } = ast.declarations[0] else {
        panic!()
    };
    for mode in [Mode::Development, Mode::Production] {
        let generated = expression(
            &ast,
            body,
            mode,
            |_| {
                Ok(Reference {
                    name: "$mark".into(),
                    kernel: false,
                })
            },
            |name| Ok(if name == "a" { "zz" } else { "aa" }.into()),
        )
        .unwrap();
        let script = format!(
            "var seen=[];function $mark(x){{seen.push(x);return x;}}var value={generated};console.log(JSON.stringify([seen,Object.keys(value)]));"
        );
        let output = Command::new("node").args(["-e", &script]).output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            serde_json::json!([["a", "z"], ["zz", "aa"]])
        );
    }
}
