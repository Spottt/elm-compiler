use planexpo_elm::{
    ast::{Declaration, Expr, Pattern, Type},
    parser::parse,
};

#[test]
fn preserves_application_and_binary_chain_for_later_fixity_resolution() {
    let ast = parse("module Main exposing (..)\nf x = g x + 2 * h 3\n").unwrap();
    let Declaration::Value { body, .. } = &ast.declarations[0] else {
        panic!()
    };
    let Expr::Binops(first, rest) = &ast.expressions[body.0 as usize].kind else {
        panic!()
    };
    assert!(matches!(
        ast.expressions[first.0 as usize].kind,
        Expr::Call(_, _)
    ));
    assert_eq!(rest.iter().map(|x| x.0).collect::<Vec<_>>(), ["+", "*"]);
}
#[test]
fn records_aliases_unions_and_right_associative_arrows() {
    let ast = parse("module Main exposing (..)\ntype alias Row r = { r | name : String, value : Maybe (List Int) }\ntype Result e a = Err e | Ok a\nf : Row r -> Int -> Int\nf row x = x\n").unwrap();
    assert_eq!(ast.declarations.len(), 4);
    let Declaration::Alias { ty, .. } = ast.declarations[0] else {
        panic!()
    };
    assert!(matches!(
        ast.types[ty.0 as usize].kind,
        Type::Record {
            extension: Some("r"),
            ..
        }
    ));
    let Declaration::Annotation { ty, .. } = ast.declarations[2] else {
        panic!()
    };
    let Type::Function(_, right) = ast.types[ty.0 as usize].kind else {
        panic!()
    };
    assert!(matches!(
        ast.types[right.0 as usize].kind,
        Type::Function(_, _)
    ));
}
#[test]
fn layout_nested_let_case_lambda_and_record_update() {
    let ast = parse(
        r#"module Main exposing (..)
f model values =
    let
        step : Int -> Int
        step x =
            if x > 0 then x else -x

        ( first, rest ) =
            ( 1, values )
    in
    case rest of
        [] ->
            { model | value = step first }

        (head :: tail) as nonempty ->
            List.map (\x -> x + head) nonempty

g = .value
"#,
    )
    .unwrap();
    assert_eq!(ast.declarations.len(), 2);
    assert!(
        ast.patterns
            .iter()
            .any(|p| matches!(p.kind, Pattern::Alias(_, "nonempty")))
    );
    assert!(
        ast.expressions
            .iter()
            .any(|e| matches!(e.kind, Expr::Case(_, _)))
    );
}
#[test]
fn minus_adjacency_distinguishes_negative_argument_and_subtraction() {
    let ast = parse("module Main exposing (..)\na = f -1\nb = f - 1\nc = f-1\n").unwrap();
    let bodies: Vec<_> = ast
        .declarations
        .iter()
        .map(|d| match d {
            Declaration::Value { body, .. } => &ast.expressions[body.0 as usize].kind,
            _ => panic!(),
        })
        .collect();
    assert!(matches!(bodies[0], Expr::Call(_, _)));
    assert!(matches!(bodies[1], Expr::Binops(_, _)));
    assert!(matches!(bodies[2], Expr::Binops(_, _)));
}
#[test]
fn rejects_missing_bodies_bad_layout_and_unmatched_delimiters() {
    for s in [
        "x =",
        "x = if True then 1",
        "x = [1,]",
        "x = (1,)",
        "x = { a = }",
        "x = let y = 1 in",
        "x = case 1 of",
        "x = \\ -> 1",
        "type alias X =",
        "f = 1\ng =",
        "x =\n1",
        "x = let\n y = 1\n  z = 2\n in y",
    ] {
        assert!(
            parse(&format!("module Main exposing (..)\n{s}\n")).is_err(),
            "accepted {s:?}"
        );
    }
}

#[test]
fn cons_alias_binds_to_whole_pattern() {
    let ast =
        parse("module Main exposing (..)\nf xs = case xs of\n    head :: tail as whole -> whole\n")
            .unwrap();
    let case = ast
        .expressions
        .iter()
        .find_map(|e| match &e.kind {
            Expr::Case(_, bs) => Some(bs),
            _ => None,
        })
        .unwrap();
    let Pattern::Alias(inner, "whole") = ast.patterns[case[0].0.0 as usize].kind else {
        panic!("alias must enclose cons")
    };
    assert!(matches!(
        ast.patterns[inner.0 as usize].kind,
        Pattern::Cons(_, _)
    ));
}
#[test]
fn rejects_floats_in_patterns_and_dangling_annotations() {
    for s in [
        "f 1.5 = 1",
        "f _named = 1",
        "x : Int",
        "x : Int\ny = 1",
        "x = - 1",
        "x = . field",
    ] {
        assert!(
            parse(&format!("module Main exposing (..)\n{s}\n")).is_err(),
            "accepted {s}"
        );
    }
}

#[test]
fn end_of_input_errors_use_real_unicode_source_coordinates() {
    for (source, position) in [
        ("broken =", "1:9:"),
        ("broken =\n", "2:1:"),
        ("module Main exposing (..)\r\nbroken =\r\n", "3:1:"),
        ("é = (", "1:6:"),
        ("broken =\n  (", "2:4:"),
    ] {
        let error = parse(source).unwrap_err();
        assert!(error.starts_with(position), "{source:?}: {error}");
    }
}

#[test]
fn attaches_documentation_to_module_and_declarations() {
    let source = "module Main exposing (value, Choice(..), Row)\n{-| Overview. -}\nimport String\n{- ordinary -}\n{-| Value. -}\nvalue : Int\nvalue = 1\n{-| Choice. -}\ntype Choice = A | B\n{-| Row. -}\ntype alias Row = { x : Int }\n";
    let (ast, docs) = planexpo_elm::parser::parse_with_docs(source).unwrap();
    assert_eq!(ast.declarations.len(), 4);
    assert_eq!(docs.overview.unwrap().text(source), " Overview. ");
    assert_eq!(docs.declarations.len(), 3);
    for (name, expected) in [
        ("value", " Value. "),
        ("Choice", " Choice. "),
        ("Row", " Row. "),
    ] {
        assert_eq!(docs.declarations[name].text(source), expected);
    }
}

#[test]
fn documentation_without_imports_distinguishes_overview_and_value() {
    let source = "module Main exposing (value)\n{-| Overview. -}\n{-| Value. -}\nvalue = 1\n";
    let (_, docs) = planexpo_elm::parser::parse_with_docs(source).unwrap();
    assert_eq!(docs.overview.unwrap().text(source), " Overview. ");
    assert_eq!(docs.declarations["value"].text(source), " Value. ");
    let source = "module Main exposing (value)\nvalue = 1\n";
    let (_, docs) = planexpo_elm::parser::parse_with_docs(source).unwrap();
    assert!(docs.overview.is_none());
    assert!(docs.declarations.is_empty());
}

#[test]
fn documentation_on_ports_and_unannotated_values() {
    let source = "port module Main exposing (send, value)\n{-| Overview. -}\n{-| Send. -}\nport send : String -> Cmd msg\n{-| Value. -}\nvalue = 1\n";
    let (_, docs) = planexpo_elm::parser::parse_with_docs(source).unwrap();
    assert_eq!(docs.declarations["send"].text(source), " Send. ");
    assert_eq!(docs.declarations["value"].text(source), " Value. ");
}

#[test]
fn documentation_cannot_float_inside_declarations_or_after_module() {
    for source in [
        "module Main exposing (value)\nvalue : Int\n{-| misplaced -}\nvalue = 1\n",
        "module Main exposing (value)\nvalue =\n    {-| misplaced -}\n    1\n",
        "module Main exposing (value)\nvalue = 1\n{-| trailing -}\n",
        "module Main exposing (value)\n{-| overview -}\n{-| first -}\n{-| second -}\nvalue = 1\n",
    ] {
        assert!(
            planexpo_elm::parser::parse_with_docs(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn modules_need_a_declaration_after_headers_and_imports() {
    for source in [
        "",
        "module Main exposing (..)\n",
        "module Main exposing (..)\nimport Basics\n",
        "module Main exposing (..)\n{- comment -}\n",
    ] {
        assert!(planexpo_elm::parser::parse(source).is_err(), "{source}");
    }
    assert!(
        planexpo_elm::parser::parse("module Main exposing (..)\nimport Basics\nvalue = 1\n")
            .is_ok()
    );
}

#[test]
fn known_header_errors_precede_later_space_failures_without_masking_required_layout() {
    for (source, expected) in [
        (
            "module lower exposing (..)\nvalue =\t1",
            "1:8: expected module name",
        ),
        (
            "module Main exposing (value other)\nvalue = 1 {- unfinished",
            "1:29: exposing 1 end",
        ),
        (
            "module Main exposing (\n\tvalue)",
            "2:1: tabs are not allowed in Elm layout",
        ),
        (
            "module Main exposing ((+\t))",
            "1:25: exposing 1 operator-close",
        ),
        (
            "module Main exposing (({- unfinished",
            "1:24: exposing 1 operator",
        ),
    ] {
        assert_eq!(planexpo_elm::parser::parse(source).unwrap_err(), expected);
        assert_eq!(
            planexpo_elm::parser::parse_with_docs(source).unwrap_err(),
            expected
        );
    }
}

#[test]
fn infix_prefix_requires_a_body_and_cannot_resume_after_regular_declarations() {
    for source in [
        "module Main exposing (..)\ninfix left 4 (<+>) = combine\n",
        "module Main exposing (..)\ninfix left 4 (<+>) = combine\ninfix right 5 (<*>) = multiply\n",
        "module Main exposing (..)\ncombine a b = a\ninfix left 4 (<+>) = combine\n",
        "module Main exposing (..)\ntype Box = Box\ninfix left 4 (<+>) = combine\ncombine a b = a\n",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
    assert!(
        parse("module Main exposing (..)\ninfix left 4 (<+>) = combine\ncombine a b = a\n").is_ok()
    );
    // Outside the operator prefix, `infix` remains an ordinary lower-case name.
    assert!(parse("module Main exposing (..)\nvalue = 1\ninfix x = x\n").is_ok());
}

#[test]
fn infix_syntax_rejects_spaced_parentheses_and_unindented_continuations() {
    for declaration in [
        "infix left 4 ( <+>) = combine",
        "infix left 4 (<+> ) = combine",
        "infix left 4 ({-x-}<+>) = combine",
        "infix\nleft 4 (<+>) = combine",
        "infix left\n4 (<+>) = combine",
        "infix left 4\n(<+>) = combine",
        "infix left 4 (<+>)\n= combine",
        "infix left 4 (<+>) =\ncombine",
    ] {
        let source = format!("module Main exposing (..)\n{declaration}\ncombine a b = a\n");
        assert!(parse(&source).is_err(), "{source}");
    }
}

#[test]
fn missing_pattern_after_parenthesis_retains_its_context() {
    for source in [
        "module Main exposing (..)\nf (+) = 1\n",
        "module Main exposing (..)\nf (if) = 1\n",
        "module Main exposing (..)\nf (\n    +) = 1\n",
    ] {
        assert!(
            parse(source)
                .unwrap_err()
                .contains("pattern parentheses open 2")
        );
    }
}

#[test]
fn unfinished_parenthesized_patterns_retain_context_at_the_closing_boundary() {
    for body in ["f (x y) = 1", "f (x,y z) = 1", "f (x + y) = 1", "f (x] = 1"] {
        let source = format!("module Main exposing (..)\n{body}\n");
        assert!(
            parse(&source)
                .unwrap_err()
                .contains("pattern parentheses end 2")
        );
    }
}

#[test]
fn parenthesized_patterns_require_indentation_at_each_boundary() {
    for body in [
        "f (\nx) = 1",
        "f (\n) = 1",
        "f (x\n) = 1",
        "f (x,\ny) = 1",
        "f (x,y\n) = 1",
    ] {
        let source = format!("module Main exposing (..)\n{body}");
        assert!(parse(&source).is_err(), "{source}");
    }
    for body in [
        "f (\n    x) = x",
        "f (x\n    ) = x",
        "f (x,\n    y\n    ) = x",
    ] {
        let source = format!("module Main exposing (..)\n{body}");
        assert!(parse(&source).is_ok(), "{source}");
    }
}

#[test]
fn missing_tuple_elements_distinguish_arguments_from_binding_patterns() {
    for (body, context) in [
        ("f (x,) = 1", "argument"),
        ("f (x,if) = 1", "argument"),
        ("f = let\n        (x,if) = (1,2)\n    in 1", "binding"),
        ("f x = case x of\n    (y,if) -> 1", "binding"),
    ] {
        let source = format!("module Main exposing (..)\n{body}");
        assert!(
            parse(&source)
                .unwrap_err()
                .contains(&format!("pattern start {context}"))
        );
    }
}

#[test]
fn list_patterns_check_indentation_relative_to_case_branches() {
    for body in ["f [\nx] = 1", "f [\n] = 1", "f [x\n] = 1", "f [x,\ny] = 1"] {
        let source = format!("module Main exposing (..)\n{body}");
        assert!(parse(&source).is_err(), "{source}");
    }
    assert!(
        parse(
            "module Main exposing (..)\nf xs = case xs of\n    [ x\n    , y\n    ] -> x\n    _ -> 0"
        )
        .is_err()
    );
    assert!(parse("module Main exposing (..)\nf xs = case xs of\n    [ x\n        , y\n        ] -> x\n    _ -> 0").is_ok());
}

#[test]
fn record_patterns_require_indented_fields_and_closing_braces() {
    for body in ["f {\nx} = 1", "f {\n} = 1", "f {x\n} = 1", "f {x,\ny} = 1"] {
        let source = format!("module Main exposing (..)\n{body}");
        assert!(parse(&source).is_err(), "{source}");
    }
    for body in ["f {} = 1", "f {x,\n    y\n    } = x"] {
        let source = format!("module Main exposing (..)\n{body}");
        assert!(parse(&source).is_ok(), "{source}");
    }
}

#[test]
fn named_wildcard_is_reported_at_its_start_by_the_pattern_parser() {
    for name in ["_value", "__", "_État", "_123"] {
        let source = format!("module Main exposing (..)\nf {name} = 1");
        assert!(parse(&source).unwrap_err().starts_with("2:3: pattern wildcard name"));
    }
    assert!(parse("module Main exposing (..)\nf _ = 1").is_ok());
}

#[test]
fn pattern_alias_names_require_indentation_after_as() {
    assert!(parse("module Main exposing (..)\nf (x as\nname) = 1").is_err());
    assert!(parse("module Main exposing (..)\nf (x as\n    name) = name").is_ok());
    assert!(parse("module Main exposing (..)\nf (x as Name) = 1").unwrap_err().contains("pattern alias name"));
}

#[test]
fn floating_patterns_report_the_entire_number() {
    for number in ["1.0", "1e3", "1.25e-3", "1E+3"] {
        let source = format!("module Main exposing (..)\nf {number} = 1");
        assert!(parse(&source).unwrap_err().starts_with(&format!("2:3: pattern float {};", number.len())));
    }
}

#[test]
fn cons_pattern_errors_preserve_indentation_and_binding_context() {
    assert!(parse("module Main exposing (..)\nf (x ::\nxs) = 1").unwrap_err().starts_with("2:8: pattern start indent 2"));
    assert!(parse("module Main exposing (..)\nf (x :: if) = 1").unwrap_err().contains("pattern start argument"));
    assert!(parse("module Main exposing (..)\nf x = case x of\n    y :: if -> 1").unwrap_err().contains("pattern start binding"));
}

#[test]
fn types_require_adjacent_unit_parentheses_and_indented_delimiters() {
    for typ in ["( )", "({-comment-})", "(\n    )", "(Int,Int\n)", "(Int\n,Int)", "{\nx:Int}", "{\n}", "{x:Int,\ny:Int}", "{x:Int\n}", "{x\n:Int}"] {
        let source = format!("module Main exposing (..)\ntype alias T = {typ}");
        assert!(parse(&source).is_err(), "{source}");
    }
    for typ in ["()", "(Int,Int\n    )", "{x:Int,\n    y:Int\n    }"] {
        let source = format!("module Main exposing (..)\ntype alias T = {typ}");
        assert!(parse(&source).is_ok(), "{source}");
    }
}

#[test]
fn expression_delimiters_require_layout_and_literal_operator_parentheses() {
    for expression in ["( )", "(\n    )", "( +)", "(+ )", "(1,2\n)", "(1\n,2)", "[\n]", "[1\n]", "{\n}", "{\nx=1}", "{x\n=1}", "{x=1,\ny=2}"] {
        let source = format!("module Main exposing (..)\nvalue = {expression}");
        assert!(parse(&source).is_err(), "{source}");
    }
    for expression in ["()", "(+)", "( -1)", "(1,2\n    )", "[\n    ]", "{x=1,\n    y=2\n    }"] {
        let source = format!("module Main exposing (..)\nvalue = {expression}");
        assert!(parse(&source).is_ok(), "{source}");
    }
}

#[test]
fn control_keywords_and_branch_arrows_require_their_context_indentation() {
    for expression in ["if True\nthen 1 else 2", "if True then 1\nelse 2", "\\item\n-> item", "case True\nof _ -> 1", "case True of _\n             -> 1", "let\n x = 1 in x", "let x = 1\nin x"] {
        let source = format!("module Main exposing (..)\nvalue = {expression}");
        assert!(parse(&source).is_err(), "{source}");
    }
    assert!(parse("module Main exposing (..)\nvalue =\n    let\n        x = 1\n    in x").is_ok());
}

#[test]
fn declarations_require_indented_names_separators_and_variant_bars() {
    for declaration in [
        "value\n= 1", "value x\n= x", "value\n: Int\nvalue = 1",
        "type\nT = T", "type\nalias T = Int", "type alias\nT = Int",
        "type T a\n= T a", "type alias T a\n= List a", "type T = A\n| B",
        "port\nsend : Int -> Cmd msg", "port send\n: Int -> Cmd msg",
        "value =\n    let\n        x\n        = 1\n    in x",
        "value =\n    let\n        (x,y)\n        = (1,2)\n    in x",
    ] {
        let source = format!("port module Main exposing (..)\n{declaration}");
        assert!(parse(&source).is_err(), "{source}");
    }
    for declaration in ["value\n    = 1", "type T = A\n    | B", "type alias\n    T = Int", "value =\n    let\n        (x,y)\n            = (1,2)\n    in x"] {
        let source = format!("module Main exposing (..)\n{declaration}");
        assert!(parse(&source).is_ok(), "{source}");
    }
}

#[test]
fn local_destructuring_uses_pattern_terms() {
    for pattern in ["Box x", "Box _", "(x,y) as pair", "{x} as record", "() as unit", "_ :: _"] {
        let source = format!("module Main exposing (..)\nvalue =\n    let\n        {pattern} = input\n    in 1");
        assert!(parse(&source).is_err(), "{source}");
    }
    for pattern in ["(Box x)", "((Box x) as box)", "((x,y) as pair)", "({x} as record)", "(() as unit)", "(_ :: _)"] {
        let source = format!("module Main exposing (..)\nvalue =\n    let\n        {pattern} = input\n    in 1");
        assert!(parse(&source).is_ok(), "{source}");
    }
    assert!(parse("module Main exposing (..)\nvalue = case input of\n    Box (Box x) as box -> x").is_ok());
}

#[test]
fn control_expressions_terminate_operator_chains() {
    for prefix in ["", "1 + ", "if True then 1 else ", "let x = 1 in ", "\\x -> "] {
        let source = format!("module Main exposing (..)\nvalue = {prefix}case True of\n    _ -> 2\n  + 3");
        assert!(parse(&source).is_err(), "{source}");
    }
    for expression in ["(case True of\n    _ -> 2\n  ) + 3", "1 + (case True of\n    _ -> 2\n  ) + 3", "1 + case True of\n    _ -> 2 + 3"] {
        let source = format!("module Main exposing (..)\nvalue = {expression}");
        assert!(parse(&source).is_ok(), "{source}");
    }
}

#[test]
fn earlier_body_errors_survive_later_lexical_failures() {
    for suffix in ["01", "0x", "\"unfinished", "'ab'", "\"\\u{1}\"", "\t", "{- unclosed"] {
        for (prefix, expected) in [("Capital = 1\nvalue = ", "declaration start"), ("f _name = ", "pattern wildcard name"), ("f 1.5 = ", "pattern float")] {
            let source = format!("module Main exposing (..)\n{prefix}{suffix}");
            let error = parse(&source).unwrap_err();
            assert!(error.contains(expected), "{source}: {error}");
            assert_eq!(planexpo_elm::parser::parse_with_docs(&source).unwrap_err(), error);
        }
        let source = format!("module Main exposing (..)\nvalue = {suffix}");
        assert_eq!(parse(&source).unwrap_err(), planexpo_elm::lexer::lex(&source).unwrap_err());
    }
}

#[test]
fn malformed_literal_boundaries_only_win_when_the_grammar_reads_them() {
    for literal in ["01", "0x", "\"unfinished", "'ab'", "\"\\u{1}\"", "\"\\q\""] {
        for (prefix, expected) in [("", "declaration start"), ("f {", "pattern record open"), ("f { x,", "pattern record field"), ("f (x as ", "pattern alias name")] {
            let source = format!("module Main exposing (..)\n{prefix}{literal}");
            let error = parse(&source).unwrap_err();
            assert!(error.contains(expected), "{source}: {error}");
        }
        for prefix in ["value = ", "f "] {
            let source = format!("module Main exposing (..)\n{prefix}{literal}");
            assert_eq!(parse(&source).unwrap_err(), planexpo_elm::lexer::lex(&source).unwrap_err());
        }
    }
}

#[test]
fn qualified_reserved_words_reach_name_resolution() {
    for word in ["if", "then", "else", "case", "of", "let", "in", "type", "module", "where", "import", "exposing", "as", "port"] {
        for prefix in ["Basics", "Example.Nested"] {
            let qualified = format!("{prefix}.{word}");
            let source = format!("module Main exposing (..)\nvalue = {qualified}\n");
            let ast = parse(&source).unwrap();
            let Declaration::Value { body, .. } = ast.declarations[0] else { panic!() };
            let Expr::Var(name) = ast.expressions[body.0 as usize].kind else { panic!() };
            assert_eq!(name, qualified);
        }
        assert!(parse(&format!("module Main exposing (..)\n{word} = 1\n")).is_err());
    }
}

#[test]
fn incomplete_expressions_at_eof_reject_without_panicking() {
    for fragment in ["(", "[", "{", "(1", "[1", "{ x = 1", "if", "if True", "if True then", "if True then 1 else", "case", "case 1 of", "case 1 of _", "let", "let x = 1", "\\x ->"] {
        for suffix in ["", " ", "\n", "\n    ", " {- comment -}"] {
            let source = format!("module Main exposing (..)\nvalue = {fragment}{suffix}");
            assert!(parse(&source).is_err(), "unexpectedly accepted: {source}");
        }
    }
}


#[test]
fn unary_minus_before_record_accessor_preserves_operator_functions_and_spans() {
    let source = "module Main exposing (..)\nvalue = -.name\noperator = (-.)\n";
    let ast = parse(source).unwrap();
    let Declaration::Value { body, .. } = ast.declarations[0] else { panic!() };
    let Expr::Negate(inner) = ast.expressions[body.0 as usize].kind else { panic!() };
    let accessor = &ast.expressions[inner.0 as usize];
    assert!(matches!(accessor.kind, Expr::Accessor("name")));
    assert_eq!(&source[accessor.span.start as usize..accessor.span.end as usize], ".name");
    let negation = &ast.expressions[body.0 as usize];
    assert_eq!(&source[negation.span.start as usize..negation.span.end as usize], "-.name");
    let Declaration::Value { body, .. } = ast.declarations[1] else { panic!() };
    assert!(matches!(ast.expressions[body.0 as usize].kind, Expr::Operator("-.")));
}
