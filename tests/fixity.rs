use planexpo_elm::{
    ast::{Declaration, Expr, ExprId, Syntax},
    fixity::{self, Associativity as A, Fixity, Table},
    parser::parse,
};
fn table() -> Table {
    [
        ("+", 6, A::Left),
        ("-", 6, A::Left),
        ("*", 7, A::Left),
        ("::", 5, A::Right),
        ("==", 4, A::Non),
        ("<|", 0, A::Right),
        ("|>", 0, A::Left),
    ]
    .into_iter()
    .map(|(name, precedence, associativity)| {
        (
            name.into(),
            Fixity {
                precedence,
                associativity,
            },
        )
    })
    .collect()
}
fn body(ast: &Syntax<'_>) -> ExprId {
    match ast.declarations[0] {
        Declaration::Value { body, .. } => body,
        _ => panic!(),
    }
}
#[test]
fn multiplication_binds_more_tightly_and_subtraction_is_left_associative() {
    let mut ast = parse("x = 1 - 2 - 3 * 4").unwrap();
    fixity::resolve(&mut ast, &table()).unwrap();
    let Expr::Binary("-", left, right) = ast.expressions[body(&ast).0 as usize].kind else {
        panic!()
    };
    assert!(matches!(
        ast.expressions[left.0 as usize].kind,
        Expr::Binary("-", _, _)
    ));
    assert!(matches!(
        ast.expressions[right.0 as usize].kind,
        Expr::Binary("*", _, _)
    ));
}
#[test]
fn cons_is_right_associative() {
    let mut ast = parse("x = 1 :: 2 :: []").unwrap();
    fixity::resolve(&mut ast, &table()).unwrap();
    let Expr::Binary("::", _, right) = ast.expressions[body(&ast).0 as usize].kind else {
        panic!()
    };
    assert!(matches!(
        ast.expressions[right.0 as usize].kind,
        Expr::Binary("::", _, _)
    ));
}
#[test]
fn rejects_ambiguous_and_unknown_operators_but_respects_parentheses() {
    for text in ["x = 1 == 2 == 3", "x = f <| g |> h", "x = 1 %% 2"] {
        assert!(fixity::resolve(&mut parse(text).unwrap(), &table()).is_err());
    }
    let mut ast = parse("x = (1 == 2) == False").unwrap();
    fixity::resolve(&mut ast, &table()).unwrap();
}
#[test]
fn long_right_associative_chains_resolve_and_drop_without_recursing() {
    let source = format!("x = {}[]", "1 :: ".repeat(20000));
    let mut ast = parse(&source).unwrap();
    fixity::resolve(&mut ast, &table()).unwrap();
    assert_eq!(
        ast.expressions
            .iter()
            .filter(|e| matches!(e.kind, Expr::Binary(_, _, _)))
            .count(),
        20000
    );
}

#[test]
fn operator_values_also_require_a_binding() {
    assert!(fixity::resolve(&mut parse("x = (%%)").unwrap(), &table()).is_err());
}

#[test]
fn infix_exports_do_not_create_local_operator_bindings() {
    let source =
        "module A exposing ((+++))\ninfix left 3 (+++) = add\nadd a b = a\nvalue = 1 +++ 2\n";
    let mut ast = parse(source).unwrap();
    assert!(fixity::declared(&ast).unwrap().contains_key("+++"));
    assert!(fixity::resolve(&mut ast, &Table::new()).is_err());
}
