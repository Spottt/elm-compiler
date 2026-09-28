use planexpo_elm::kernel::{Chunk, parse};
#[test]
fn preserves_javascript_and_resolves_import_tags() {
    let source = "/*\nimport Json.Decode as D exposing (decodeValue)\nimport Elm.Kernel.Utils exposing (Tuple2)\n*/\nvar a = __D_decodeValue; var b = __Utils_Tuple2;";
    let content = parse(source).unwrap();
    assert_eq!(
        content.chunks,
        vec![
            Chunk::JavaScript("\nvar a = "),
            Chunk::ElmVariable {
                module: "Json.Decode".into(),
                name: "decodeValue".into()
            },
            Chunk::JavaScript("; var b = "),
            Chunk::KernelVariable {
                module: "Utils".into(),
                name: "Tuple2".into()
            },
            Chunk::JavaScript(";")
        ]
    );
}
#[test]
fn numbers_fields_and_enums_by_first_occurrence_and_group() {
    let content = parse("/*\n*/__$model __z __a __z __0B __0A __0B __1A __DEBUG __PROD").unwrap();
    let tags: Vec<_> = content
        .chunks
        .into_iter()
        .filter(|c| !matches!(c, Chunk::JavaScript(_)))
        .collect();
    assert_eq!(
        tags,
        vec![
            Chunk::ElmField("model"),
            Chunk::JsField(0),
            Chunk::JsField(1),
            Chunk::JsField(0),
            Chunk::JsEnum(0),
            Chunk::JsEnum(1),
            Chunk::JsEnum(0),
            Chunk::JsEnum(0),
            Chunk::Debug,
            Chunk::Prod
        ]
    );
}
#[test]
fn rejects_unknown_tags_and_invalid_import_tables() {
    for source in [
        "/*\n*/__Missing_name",
        "/*\nimport Elm.Kernel.X as X exposing (f)\n*/",
        "/*\nimport Json.Decode exposing (f)\n*/",
        "/*\nimport Basics exposing (..)\n*/",
        "/*\nimport Maybe exposing (Maybe(..))\n*/",
        "/*\nvalue=1\n*/",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
}

#[test]
fn rendered_kernel_executes_both_build_modes_and_preserves_field_layout() {
    use planexpo_elm::kernel::{Mode, render};
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let content = parse("/*\nimport Source exposing (value)\nimport Elm.Kernel.Utils exposing (identity)\n*/\nvar _Mode__DEBUG = 7;\nvar _Mode__PROD = 9;\nvar data = { __$field: __Source_value, __slot: __0One };\nconsole.log(JSON.stringify([_Mode, _Utils_identity(data.__$field), data.__slot, __0Two]));").unwrap();
    for (mode, expected) in [
        (Mode::Development, "[7,42,0,1]"),
        (Mode::Production, "[9,42,0,1]"),
    ] {
        let js = render(
            &content,
            mode,
            |module, name| {
                assert_eq!((module, name), ("Source", "value"));
                Ok("$author$project$Source$value".into())
            },
            |name| {
                assert_eq!(name, "field");
                Ok("renamed".into())
            },
            |index| format!("slot{index}"),
        )
        .unwrap();
        let mut process = Command::new("node")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Node is required for JavaScript execution tests");
        process.stdin.take().unwrap().write_all(format!("var $author$project$Source$value = 42; function _Utils_identity(x) {{ return x; }}\n{js}").as_bytes()).unwrap();
        let result = process.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), expected);
    }
    assert!(
        render(
            &content,
            Mode::Development,
            |_, _| Err("unresolved global".into()),
            |n| Ok(n.into()),
            |i| i.to_string()
        )
        .is_err()
    );
}
