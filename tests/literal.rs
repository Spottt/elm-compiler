use planexpo_elm::{
    kernel::Mode,
    lexer::{Kind, lex},
    literal::emit,
};
use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn emitted_literals_execute_with_elm_values_and_character_boxing() {
    let fixtures = [
        ("42", "42"),
        ("0xFF", "255"),
        ("1.25e2", "125"),
        (r#""a\n\t\"\\'b""#, r#""a\n\t\"\\'b""#),
        (
            "\"\"\"first\nsecond\r\n\"quoted\" end\"\"\"",
            "\"first\\nsecond\\r\\n\\\"quoted\\\" end\"",
        ),
        (r#""\u{1F642}""#, "\"🙂\""),
        (r#"'\u{0061}'"#, "\"a\""),
        (r#"'\''"#, "\"'\""),
        ("'🙂'", "\"🙂\""),
        ("\"café\"", "\"café\""),
    ];
    for mode in [Mode::Development, Mode::Production] {
        let mut expressions = Vec::new();
        let mut expected = Vec::new();
        for (raw, json) in fixtures {
            let tokens = lex(raw).unwrap();
            assert_eq!(tokens.len(), 1, "{raw}");
            let kind = tokens[0].kind;
            expressions.push(emit(kind, raw, mode).unwrap());
            let boxed = kind == Kind::Char && matches!(mode, Mode::Development);
            expected.push(serde_json::json!([
                serde_json::from_str::<serde_json::Value>(json).unwrap(),
                boxed
            ]));
        }
        let script = format!(
            "function _Utils_chr(c) {{ return new String(c); }}\nconsole.log(JSON.stringify([{}].map(x => [x, x instanceof String])));",
            expressions.join(",")
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
        let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual, serde_json::json!(expected));
    }
}

#[test]
fn integer_literals_follow_signed_64_bit_accumulation() {
    for (raw, expected) in [("18446744073709551616", "0"), ("18446744073709551617", "1"), ("18446744073709551615", "(-1)"), ("9223372036854775808", "(-9223372036854775808)"), ("0x10000000000000000", "0")] {
        assert_eq!(emit(Kind::Number, raw, Mode::Production).unwrap(), expected);
    }
    assert!(lex("0x8000000000000000").is_err());
    // Number.chompHex uses -1 as its stop sentinel, even when produced by overflow.
    let source = "0xFFFFFFFFFFFFFFFF";
    let tokens = lex(source).unwrap();
    assert_eq!(tokens.iter().map(|t| t.text(source)).collect::<Vec<_>>(), ["0xFFFFFFFFFFFFFFF", "F"]);
    assert!(lex("0x10000000000000000").is_ok());
}
