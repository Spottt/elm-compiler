use planexpo_elm::{
    coverage,
    names::{self, Environment, Symbols},
    parser::parse,
};
fn check(source: &str) -> Result<(), String> {
    let ast = parse(source)?;
    let mut symbols = Symbols::default();
    let mut env = Environment::default();
    env.builtin_list(&mut symbols);
    let (_, resolved) = names::resolve(&ast, "test:Main", env, &mut symbols)?;
    coverage::check(&ast, &resolved, &symbols)
}
#[test]
fn checks_all_combinations_of_nested_constructor_patterns() {
    let prefix = "type Bit = Zero | One\n";
    check(&format!(
        "{prefix}f x = case x of\n    (Zero,_) -> ()\n    (_,Zero) -> ()\n    (One,One) -> ()"
    ))
    .unwrap();
    assert!(
        check(&format!(
            "{prefix}f x = case x of\n    (Zero,_) -> ()\n    (One,One) -> ()"
        ))
        .unwrap_err()
        .contains("missing patterns")
    );
    assert!(
        check(&format!(
            "{prefix}f x = case x of\n    (Zero,_) -> ()\n    (One,_) -> ()\n    (_,One) -> ()"
        ))
        .unwrap_err()
        .contains("redundant")
    );
}
#[test]
fn checks_lists_alias_patterns_and_infinite_literal_domains() {
    check("f xs = case xs of\n    [] -> ()\n    (a :: rest) as whole -> ()").unwrap();
    assert!(check("f xs = case xs of\n    [] -> ()\n    [a] -> ()").is_err());
    check("f x = case x of\n    1 -> ()\n    _ -> ()").unwrap();
    assert!(check("f x = case x of\n    1 -> ()\n    2 -> ()").is_err());
}
#[test]
fn checks_function_lambda_and_destructuring_patterns() {
    check("type Box a = Box a\nf (Box a) = a\ng {field} = field").unwrap();
    assert!(check("type Choice = A | B\nf A = ()").is_err());
    assert!(check("f = \\[a] -> a").is_err());
    assert!(check("f xs = let [a] = xs in a").is_err());
}
#[test]
fn canonical_escaped_literals_are_redundant() {
    for source in [
        "f x = case x of\n    '\\u{000061}' -> ()\n    '\\u{0061}' -> ()\n    _ -> ()",
        "f x = case x of\n    \"\\u{000061}\" -> ()\n    \"\\u{0061}\" -> ()\n    _ -> ()",
        "f x = case x of\n    10 -> ()\n    0x0A -> ()\n    _ -> ()",
    ] {
        assert!(check(source).unwrap_err().contains("redundant"));
    }
}

#[test]
fn raw_and_unicode_escaped_patterns_remain_distinct_like_elm() {
    check("f x = case x of\n    'a' -> ()\n    '\\u{0061}' -> ()\n    _ -> ()").unwrap();
    check("f x = case x of\n    \"a\" -> ()\n    \"\\u{0061}\" -> ()\n    _ -> ()").unwrap();
}

#[test]
fn wrapped_integer_patterns_share_their_canonical_value() {
    check("f x = case x of\n    18446744073709551616 -> ()\n    _ -> ()").unwrap();
    for literal in ["18446744073709551616", "0x10000000000000000"] {
        let source = format!("f x = case x of\n    {literal} -> ()\n    0 -> ()\n    _ -> ()");
        assert!(
            check(&source).unwrap_err().contains("redundant"),
            "{source}"
        );
    }
}

#[test]
fn unrelated_unions_do_not_change_nested_coverage() {
    let unused = format!(
        "type Unused = {}\n",
        (0..1000)
            .map(|n| format!("Unused{n}"))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    for branches in [
        "Box A -> ()",
        "Box A -> ()\n    Box B -> ()",
        "_ -> ()\n    Box A -> ()",
    ] {
        let source = format!(
            "type Choice = A | B\ntype Box = Box Choice\nf x = case x of\n    {branches}\n"
        );
        assert_eq!(check(&source), check(&format!("{source}{unused}")));
    }
}
