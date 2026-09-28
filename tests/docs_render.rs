use planexpo_elm::{
    docs,
    names::{self, Environment, Symbols},
    parser::parse_with_docs,
};

fn render(source: &str) -> serde_json::Value {
    let (ast, comments) = parse_with_docs(source).unwrap();
    let mut symbols = Symbols::default();
    let mut env = Environment::default();
    env.builtin_list(&mut symbols);
    let (_, resolved) = names::resolve(&ast, "author/pkg:Main", env, &mut symbols).unwrap();
    docs::to_json(&ast, &comments, &resolved, &symbols).unwrap()
}

#[test]
fn renders_aliases_unions_records_tuples_and_function_precedence() {
    let json = render(
        "module Main exposing (Choice(..), Row, Signature, Closed)\n{-| @docs Choice, Row, Signature, Closed -}\n{-| Choice. -}\ntype Choice a = None | Some a | Apply (a -> a)\n{-| Row. -}\ntype alias Row r a = { r | z : (a, ()), a : List a }\n{-| Signature. -}\ntype alias Signature a = (a -> a) -> List (List a) -> Choice (a -> a)\n{-| Closed. -}\ntype Closed = Hidden\n",
    );
    assert_eq!(
        json["aliases"][0]["type"],
        "{ r | z : ( a, () ), a : List.List a }"
    );
    assert_eq!(
        json["aliases"][1]["type"],
        "(a -> a) -> List.List (List.List a) -> Main.Choice (a -> a)"
    );
    assert_eq!(
        json["unions"][0]["cases"],
        serde_json::json!([["None", []], ["Some", ["a"]], ["Apply", ["a -> a"]]])
    );
    assert_eq!(json["unions"][1]["cases"], serde_json::json!([]));
}

#[test]
fn retains_alias_names_in_signatures_and_emits_empty_record() {
    let json = render(
        "module Main exposing (Empty, Wrapped, identity)\n{-| @docs Empty, Wrapped, identity -}\n{-| Empty. -}\ntype alias Empty = {}\n{-| Wrapped. -}\ntype alias Wrapped a = { value : a }\n{-| Identity. -}\nidentity : Wrapped a -> Wrapped a\nidentity x = x\n",
    );
    assert_eq!(json["aliases"][0]["type"], "{}");
    assert_eq!(
        json["values"][0]["type"],
        "Main.Wrapped a -> Main.Wrapped a"
    );
    assert_eq!(json["name"], "Main");
}
