//! Port of the parse/print agreement properties in upstream 0.18 Test.Property.
//! Independent generated trees are printed as Elm 0.19 syntax, then compared to
//! the actual parser tree. Archived Haskell sources remain byte-for-byte intact.
use planexpo_elm::{
    ast::{Declaration, Pattern, PatternId, Syntax, Type, TypeId},
    parser::parse,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tree {
    Atom(String),
    Named(String, Vec<Tree>),
    Record(Option<String>, Vec<(String, Tree)>),
    Arrow(Box<Tree>, Box<Tree>),
    Alias(Box<Tree>, String),
}
struct Generator(u64);
impl Generator {
    fn next(&mut self, n: u64) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 32) % n) as usize
    }
    fn ty(&mut self, depth: usize) -> Tree {
        let choice = self.next(if depth == 0 { 2 } else { 5 });
        match choice {
            0 => Tree::Atom(format!("a{}", self.next(19))),
            1 => Tree::Named(format!("Type{}", self.next(19)), vec![]),
            2 => Tree::Named("Box".into(), vec![self.ty(depth - 1), self.ty(depth - 1)]),
            3 => Tree::Arrow(Box::new(self.ty(depth - 1)), Box::new(self.ty(depth - 1))),
            _ => {
                let extension = (self.next(2) == 0).then(|| "row".into());
                let count = 1 + self.next(3);
                Tree::Record(
                    extension,
                    (0..count)
                        .map(|i| (format!("field{i}"), self.ty(depth - 1)))
                        .collect(),
                )
            }
        }
    }
    fn pattern(&mut self, depth: usize) -> Tree {
        match self.next(if depth == 0 { 3 } else { 6 }) {
            0 => Tree::Atom("_".into()),
            1 => Tree::Atom(format!("value{}", self.next(19))),
            2 => Tree::Atom(["42", "'é'", "\"a\\nβ\"", "0", "'\\u{1F642}'"][self.next(5)].into()),
            3 => Tree::Named(
                "Ctor".into(),
                vec![self.pattern(depth - 1), self.pattern(depth - 1)],
            ),
            4 => Tree::Alias(
                Box::new(self.pattern(depth - 1)),
                format!("alias{}", self.next(19)),
            ),
            _ => Tree::Record(
                None,
                (0..1 + self.next(3))
                    .map(|i| (format!("field{i}"), Tree::Atom("_".into())))
                    .collect(),
            ),
        }
    }
}
fn print(tree: &Tree, pattern: bool) -> String {
    match tree {
        Tree::Atom(name) => name.clone(),
        Tree::Named(name, args) => {
            let mut out = name.clone();
            for arg in args {
                out.push_str(&format!(" ({})", print(arg, pattern)));
            }
            out
        }
        Tree::Arrow(a, b) => format!("({}) -> ({})", print(a, false), print(b, false)),
        Tree::Alias(value, name) => format!("({}) as {name}", print(value, true)),
        Tree::Record(extension, fields) => {
            let prefix = extension
                .as_ref()
                .map(|s| format!("{s} | "))
                .unwrap_or_default();
            let fields = fields
                .iter()
                .map(|(name, value)| {
                    if pattern {
                        name.clone()
                    } else {
                        format!("{name} : {}", print(value, false))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {prefix}{fields} }}")
        }
    }
}
fn parsed_type(ast: &Syntax<'_>, id: TypeId) -> Tree {
    match &ast.types[id.0 as usize].kind {
        Type::Var(name) => Tree::Atom((*name).into()),
        Type::Constructor(name, args) => Tree::Named(
            (*name).into(),
            args.iter().map(|id| parsed_type(ast, *id)).collect(),
        ),
        Type::Function(a, b) => Tree::Arrow(
            Box::new(parsed_type(ast, *a)),
            Box::new(parsed_type(ast, *b)),
        ),
        Type::Record { extension, fields } => Tree::Record(
            extension.map(str::to_owned),
            fields
                .iter()
                .map(|(name, id)| ((*name).into(), parsed_type(ast, *id)))
                .collect(),
        ),
        other => panic!("unexpected generated type {other:?}"),
    }
}
fn parsed_pattern(ast: &Syntax<'_>, id: PatternId) -> Tree {
    match &ast.patterns[id.0 as usize].kind {
        Pattern::Wildcard => Tree::Atom("_".into()),
        Pattern::Var(name) | Pattern::Literal(_, name) => Tree::Atom((*name).into()),
        Pattern::Constructor(name, args) => Tree::Named(
            (*name).into(),
            args.iter().map(|id| parsed_pattern(ast, *id)).collect(),
        ),
        Pattern::Alias(value, name) => {
            Tree::Alias(Box::new(parsed_pattern(ast, *value)), (*name).into())
        }
        Pattern::Record(fields) => Tree::Record(
            None,
            fields
                .iter()
                .map(|name| ((*name).into(), Tree::Atom("_".into())))
                .collect(),
        ),
        other => panic!("unexpected generated pattern {other:?}"),
    }
}
fn assert_pattern(tree: &Tree) {
    let text = print(tree, true);
    let source = format!("f ({text}) = ()\n");
    let ast = parse(&source).unwrap_or_else(|e| panic!("{text}: {e}"));
    let Declaration::Value { arguments, .. } = &ast.declarations[0] else {
        panic!()
    };
    assert_eq!(parsed_pattern(&ast, arguments[0]), *tree, "{text}");
}
#[test]
fn upstream_generated_pattern_parse_print_agreement() {
    let mut generator = Generator(0x0180_0019);
    for i in 0..2048 {
        assert_pattern(&generator.pattern(i % 5));
    }
}
#[test]
fn upstream_generated_type_parse_print_agreement() {
    let mut generator = Generator(0x0180_0019);
    for i in 0..2048 {
        let expected = generator.ty(i % 5);
        let text = print(&expected, false);
        let source = format!("value : {text}\nvalue = ()\n");
        let ast = parse(&source).unwrap_or_else(|e| panic!("{text}: {e}"));
        let Declaration::Annotation { ty, .. } = ast.declarations[0] else {
            panic!()
        };
        assert_eq!(parsed_type(&ast, ty), expected, "{text}");
    }
}
#[test]
fn upstream_long_pattern_regression_adapted_to_019_identifiers() {
    // Elm 0.19 removed apostrophes in identifiers. The fixture shape and field
    // names are retained, replacing only the three legacy apostrophes by `_`.
    let fields = [
        "q7yclkcm7k_ikstrczv_",
        "wQRv6gKsvvkjw4b5F",
        "c9_eFfhk9FTvsMnwF_D",
        "yqxhEkHvRFwZ",
        "o",
        "nbUlCn3y3NnkVoxhW",
        "iJ0MNy3KZ_lrs",
        "ug",
        "sHHsX",
        "mRKs9d",
        "o2KiCX5_ZRzHJfRi8",
    ];
    assert_pattern(&Tree::Named(
        "I".into(),
        vec![
            Tree::Atom("'+'".into()),
            Tree::Record(
                None,
                fields
                    .into_iter()
                    .map(|s| (s.into(), Tree::Atom("_".into())))
                    .collect(),
            ),
            Tree::Atom("su_BrrbPUK6I33Eq".into()),
        ],
    ));
}

#[test]
fn upstream_generated_literal_parse_print_agreement() {
    use planexpo_elm::{ast::Expr, kernel::Mode, literal::emit};
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    fn quote(value: &str, delimiter: char) -> String {
        let mut out = String::from(delimiter);
        for c in value.chars() {
            if c == delimiter || c == '\\' {
                out.push('\\');
                out.push(c);
            } else if c.is_control() || c == '\u{2028}' || c == '\u{2029}' {
                out.push_str(&format!("\\u{{{:04X}}}", c as u32));
            } else {
                out.push(c);
            }
        }
        out.push(delimiter);
        out
    }
    let mut generator = Generator(0x0180_0019);
    let mut fixtures = Vec::new();
    for i in 0..512 {
        let mut value = String::new();
        for _ in 0..i % 13 {
            loop {
                if let Some(c) = char::from_u32(generator.next(0x110000) as u32) {
                    value.push(c);
                    break;
                }
            }
        }
        fixtures.push((quote(&value, '"'), serde_json::json!(value)));
        let c = ['\0', '\n', '\r', '\t', '\'', '"', '\\', 'é', '🙂'][i % 9];
        fixtures.push((
            quote(&c.to_string(), '\''),
            serde_json::json!(c.to_string()),
        ));
        let number = (generator.next(1_000_000) as f64 - 500_000.0) / 8.0;
        fixtures.push((number.to_string(), serde_json::json!(number)));
    }
    let mut emitted = Vec::new();
    let mut expected = Vec::new();
    for (raw, value) in fixtures {
        let source = format!("value = {raw}\n");
        let ast = parse(&source).unwrap_or_else(|error| panic!("{raw}: {error}"));
        let Declaration::Value { body, .. } = ast.declarations[0] else {
            panic!()
        };
        let (negative, id) = match ast.expressions[body.0 as usize].kind {
            Expr::Negate(id) => (true, id),
            _ => (false, body),
        };
        let Expr::Literal(kind, raw) = ast.expressions[id.0 as usize].kind else {
            panic!()
        };
        let js = emit(kind, raw, Mode::Production).unwrap();
        emitted.push(if negative { format!("-({js})") } else { js });
        expected.push(value);
    }
    let script = format!("console.log(JSON.stringify([{}]));", emitted.join(","));
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
    let actual: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    for (i, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
            assert_eq!(a, b, "literal {i}");
        } else {
            assert_eq!(actual, expected, "literal {i}");
        }
    }
    assert_eq!(actual.len(), expected.len());
}
