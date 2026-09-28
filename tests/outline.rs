use planexpo_elm::outline::{decode, validate};
use serde_json::json;
fn package() -> serde_json::Value {
    json!({"type":"package","name":"author/pkg","summary":"","license":"BSD-3-Clause","version":"1.0.0","exposed-modules":[],"dependencies":{"elm/core":"1.0.0 <= v < 2.0.0"},"test-dependencies":{},"elm-version":"0.19.0 <= v < 0.20.0"})
}
#[test]
fn package_required_fields_and_historical_bounds_are_checked() {
    let root = tempfile::tempdir().unwrap();
    let base = package();
    validate(&base, root.path()).unwrap();
    for field in [
        "name",
        "summary",
        "license",
        "version",
        "exposed-modules",
        "dependencies",
        "test-dependencies",
        "elm-version",
    ] {
        let mut config = base.clone();
        config.as_object_mut().unwrap().remove(field);
        assert!(validate(&config, root.path()).is_err(), "{field}");
    }
    for (field, value) in [
        ("summary", json!("x".repeat(80))),
        ("license", json!("UNLICENSED")),
        ("exposed-modules", json!(["Main.bad"])),
        ("exposed-modules", json!({"x".repeat(20):[]})),
    ] {
        let mut config = base.clone();
        config[field] = value;
        assert!(validate(&config, root.path()).is_err(), "{field}");
    }
    let mut config = base;
    config["summary"] = json!("é".repeat(39));
    validate(&config, root.path()).unwrap();
    config["summary"] = json!("é".repeat(40));
    assert!(validate(&config, root.path()).is_err());
}
#[test]
fn source_directories_are_nonempty_existing_and_canonically_distinct() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    let mut config = json!({"type":"application","elm-version":"0.19.1","source-directories":["src"],"dependencies":{"direct":{"elm/core":"1.0.5","elm/json":"1.1.3"},"indirect":{}},"test-dependencies":{"direct":{},"indirect":{}}});
    validate(&config, root.path()).unwrap();
    for dirs in [
        json!([]),
        json!(["missing"]),
        json!(["src", "./src"]),
        json!([42]),
    ] {
        config["source-directories"] = dirs;
        assert!(validate(&config, root.path()).is_err());
    }
}
#[test]
fn json_source_snippets_preserve_escapes_in_values_and_keys() {
    let value = decode(r#"{"summary":"\u00e9\n\"text\"","na\u006de":"author/pkg"}"#).unwrap();
    assert_eq!(value["summary"], r#"\u00e9\n\"text\""#);
    assert!(value.get("name").is_none());
    for bad in [r#"{"a":"\q"}"#, r#"{"a":"unfinished}"#] {
        assert!(decode(bad).is_err());
    }
}

#[test]
fn diagnostics_distinguish_missing_fields_wrong_types_and_empty_arrays() {
    let root = tempfile::tempdir().unwrap();
    for (config, title) in [
        (json!({}), "MISSING FIELD"),
        (json!({"type":null}), "EXPECTING STRING"),
        (json!({"type":"unknown"}), "UNEXPECTED TYPE"),
        (
            json!({"type":"application","elm-version":"0.19.1","source-directories":null}),
            "EXPECTING ARRAY",
        ),
        (
            json!({"type":"application","elm-version":"0.19.1","source-directories":[]}),
            "NO SOURCE DIRECTORIES",
        ),
    ] {
        let problem = validate(&config, root.path()).unwrap_err();
        assert_eq!(problem.title, title);
        let report = planexpo_elm::outline::report_encoded(&problem.encode()).unwrap();
        assert_eq!(report["path"], "elm.json");
        assert_eq!(report["title"], title);
        assert_eq!(report["type"], "error");
    }
}

#[test]
fn cli_renders_manifest_diagnostics_as_json_and_readable_terminal_text() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("elm.json"), "{}").unwrap();
    for report_json in [false, true] {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_planexpo-elm"));
        command.current_dir(root.path()).args(["make", "Main.elm"]);
        if report_json {
            command.arg("--report=json");
        }
        let result = command.output().unwrap();
        assert!(!result.status.success());
        let stderr = String::from_utf8(result.stderr).unwrap();
        assert!(!stderr.contains("ELM_OUTLINE_JSON:"));
        if report_json {
            let report: serde_json::Value = serde_json::from_str(&stderr).unwrap();
            assert_eq!(report["title"], "MISSING FIELD");
            assert_eq!(report["path"], "elm.json");
        } else {
            assert!(stderr.contains("MISSING FIELD"));
            assert!(stderr.contains("1| {}"));
            assert!(
                stderr.contains("I was expecting to run into an OBJECT with a \"type\" field.")
            );
        }
    }
}

#[test]
fn overlong_group_keys_keep_the_official_oneof_tie_diagnostic() {
    let root = tempfile::tempdir().unwrap();
    let mut config = package();
    config["exposed-modules"] = json!({"x".repeat(20):["Main"]});
    assert_eq!(
        validate(&config, root.path()).unwrap_err().title,
        "EXPECTING ARRAY"
    );
}

#[test]
fn named_fields_use_first_occurrence_but_dependency_dictionaries_use_last() {
    let value = decode(r#"{"type":"application","type":"package","elm-version":"0.19.1","elm-version":"0.18.0","dependencies":{"direct":{"elm/core":"1.0.0","elm/core":"1.0.5"},"direct":{"elm/core":"9.9.9"}},"dependencies":{}}"#).unwrap();
    assert_eq!(value["type"], "application");
    assert_eq!(value["elm-version"], "0.19.1");
    assert_eq!(value["dependencies"]["direct"]["elm/core"], "1.0.5");
}

#[test]
fn invalid_dependency_occurrences_cannot_be_hidden_by_later_valid_values() {
    for raw in [
        r#"{"elm/core":"latest","elm/core":"1.0.0 <= v < 2.0.0"}"#,
        r#"{"elm/core":"1.0.0 <= v < 2.0.0","elm/core":null,"elm/core":"1.0.0 <= v < 2.0.0"}"#,
    ] {
        let mut config = package();
        config.as_object_mut().unwrap().remove("dependencies");
        let source = config.to_string();
        let source = format!("{},\"dependencies\":{raw}}}", &source[..source.len() - 1]);
        let decoded = decode(&source).unwrap();
        assert!(validate(&decoded, std::path::Path::new(".")).is_err());
        let mut missing_name = decoded;
        missing_name.as_object_mut().unwrap().remove("name");
        assert_eq!(
            validate(&missing_name, std::path::Path::new("."))
                .unwrap_err()
                .title,
            "MISSING FIELD"
        );
    }
}

#[test]
fn repeated_exposed_groups_keep_every_module_and_invalid_occurrence() {
    let base = package().to_string();
    for (groups, valid) in [
        (r#"{"Public":["Main"],"Public":["Other"]}"#, true),
        (r#"{"Public":["bad"],"Public":["Main"]}"#, false),
    ] {
        let source = base.replace(
            "\"exposed-modules\":[]",
            &format!("\"exposed-modules\":{groups}"),
        );
        let config = decode(&source).unwrap();
        assert_eq!(validate(&config, std::path::Path::new(".")).is_ok(), valid);
        if valid {
            assert_eq!(
                config["exposed-modules"]["Public"],
                json!(["Main", "Other"])
            );
        }
    }
}

#[test]
fn dependency_errors_keep_source_order_across_distinct_keys() {
    let base = package().to_string();
    let source = base.replace(
        "\"dependencies\":{\"elm/core\":\"1.0.0 <= v < 2.0.0\"}",
        "\"dependencies\":{\"z/pkg\":\"latest\",\"broken\":\"1.0.0 <= v < 2.0.0\"}",
    );
    let config = decode(&source).unwrap();
    assert_eq!(
        validate(&config, std::path::Path::new("."))
            .unwrap_err()
            .title,
        "PROBLEM WITH CONSTRAINT"
    );
}

#[test]
fn json_numbers_follow_elm_integer_lexing_and_machine_overflow() {
    let value = decode(r#"{"a":9223372036854775808,"b":18446744073709551616,"c":42}"#).unwrap();
    assert_eq!(value["a"], i64::MIN);
    assert_eq!(value["b"], 0);
    assert_eq!(value["c"], 42);
    for (source, title) in [
        (r#"{"a":-1}"#, "EXPECTING A VALUE"),
        (r#"{"a":1.5}"#, "UNEXPECTED NUMBER"),
        (r#"{"a":1e2}"#, "UNEXPECTED NUMBER"),
        (r#"{"a":01}"#, "BAD NUMBER"),
        (r#"{"a":0e2}"#, "UNFINISHED OBJECT"),
    ] {
        let error = decode(source).unwrap_err();
        let report = planexpo_elm::outline::report_encoded(&error).unwrap();
        assert_eq!(report["title"], title, "{source}");
    }
}

#[test]
fn json_unicode_escapes_are_raw_snippets_even_without_surrogate_pairs() {
    let value = decode(r#"{"a":"\ud800","b":"\udfff","c":"\u0000","d":"\ud800\udfff"}"#).unwrap();
    assert_eq!(value["a"], r#"\ud800"#);
    assert_eq!(value["b"], r#"\udfff"#);
    assert_eq!(value["c"], r#"\u0000"#);
    assert_eq!(value["d"], r#"\ud800\udfff"#);
}

#[test]
fn ignored_outline_extensions_are_validated_without_recursive_values() {
    let nested = format!("{}0{}", "[".repeat(10_000), "]".repeat(10_000));
    for kind in ["application", "package"] {
        let value = decode(&format!(r#"{{"type":"{kind}","extension":{nested}}}"#)).unwrap();
        assert_eq!(value, json!({"type": kind}));
    }
    let value = decode(&format!(r#"{{"type":"application","dependencies":{{"direct":{{}},"indirect":{{}},"extension":{nested}}}}}"#)).unwrap();
    assert_eq!(value["dependencies"], json!({"direct":{},"indirect":{}}));
    let bad = format!(
        r#"{{"type":"application","extension":{}?{}}}"#,
        "[".repeat(10_000),
        "]".repeat(10_000)
    );
    let report = planexpo_elm::outline::report_encoded(&decode(&bad).unwrap_err()).unwrap();
    assert_eq!(report["title"], "EXPECTING A VALUE");
}

#[test]
fn deeply_nested_wrong_types_reach_outline_validation() {
    let nested = format!("{}0{}", "[".repeat(10_000), "]".repeat(10_000));
    let root = tempfile::tempdir().unwrap();
    for field in ["summary", "version", "type"] {
        let mut config = package();
        config[field] = json!("PLACEHOLDER");
        let source = config.to_string().replace("\"PLACEHOLDER\"", &nested);
        let value = decode(&source).unwrap();
        assert_eq!(
            validate(&value, root.path()).unwrap_err().title,
            "EXPECTING STRING",
            "{field}"
        );
    }
    let mut config = package();
    config["exposed-modules"] = json!(["PLACEHOLDER"]);
    let source = config.to_string().replace("\"PLACEHOLDER\"", &nested);
    assert_eq!(
        validate(&decode(&source).unwrap(), root.path())
            .unwrap_err()
            .title,
        "EXPECTING STRING"
    );
    let value = decode(&nested).unwrap();
    assert_eq!(
        validate(&value, root.path()).unwrap_err().title,
        "EXPECTING OBJECT"
    );
}
