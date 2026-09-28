use planexpo_elm::shader::{Shader, Type};

#[test]
fn shader_interfaces_match_single_declaration_rules() {
    let shader = Shader::parse("[glsl|attribute vec3 position; uniform highp float time; varying vec2 uv; uniform float ignoredA, ignoredB; uniform bool ignoredBool; void main() {}|]").unwrap();
    assert_eq!(shader.attributes.get("position"), Some(&Type::Vec3));
    assert_eq!(shader.uniforms.get("time"), Some(&Type::Float));
    assert_eq!(shader.varyings.get("uv"), Some(&Type::Vec2));
    assert_eq!(shader.uniforms.len(), 1);
}
#[test]
fn shader_parser_rejects_invalid_trailing_input() {
    for source in [
        "",
        "// comment only",
        "#version 100\nvoid main() {}",
        "void main() {} garbage",
        "void main( {",
        "uniform float time",
        "void main() { float a = ; }",
    ] {
        assert!(
            Shader::parse(&format!("[glsl|{source}|]")).is_err(),
            "accepted {source}"
        );
    }
}
#[test]
fn shader_source_and_field_translation_survive_emission() {
    let shader = Shader::parse(
        "[glsl|// 'quoted' \\ slash\r\nuniform float time; attribute vec3 position; void main() {}|]",
    )
    .unwrap();
    let output = shader
        .emit(&mut |name| Ok(format!("short_{name}")))
        .unwrap();
    assert!(output.contains("\"time\":\"short_time\""));
    assert!(output.contains("\"position\":\"short_position\""));
    assert!(!shader.source.contains('\r'));
}

#[test]
fn invariant_qualified_varyings_are_not_extracted_by_elm() {
    let shader = Shader::parse("[glsl|invariant varying float value;|]").unwrap();
    assert!(shader.varyings.is_empty());
}

#[test]
fn legacy_reserved_words_are_rejected_only_as_complete_tokens() {
    for source in [
        "double value;",
        "dvec3 value;",
        "image2D value;",
        "float row_major;",
    ] {
        assert!(
            Shader::parse(&format!("[glsl|{source}|]")).is_err(),
            "accepted {source}"
        );
    }
    for source in [
        "float doubleValue;",
        "float _double;",
        "// double\nvoid main() {}",
        "void main() { /* row_major */ }",
    ] {
        Shader::parse(&format!("[glsl|{source}|]")).unwrap();
    }
}

#[test]
fn consecutive_underscores_are_reserved_in_shader_identifiers() {
    for name in ["__value", "value__", "a__b"] {
        for source in [
            format!("float {name};"),
            format!("void main() {{ float value = {name}; }}"),
            format!("struct Light {{ float {name}; }};"),
            format!("void {name}() {{}}"),
        ] {
            assert!(
                Shader::parse(&format!("[glsl|{source}|]")).is_err(),
                "accepted {source}"
            );
        }
        Shader::parse(&format!("[glsl|void main() {{ /* {name} */ }}|]")).unwrap();
    }
    for name in ["_value", "value_", "a_b"] {
        Shader::parse(&format!("[glsl|float {name};|]")).unwrap();
    }
}

#[test]
fn legacy_glsl_names_and_numbers() {
    for source in [
        "Foo value;",
        "float buffer;",
        "float shared;",
        "float café;",
        "float x = 1.0f;",
    ] {
        Shader::parse(&format!("[glsl|{source}|]")).unwrap();
    }
    for source in ["float x = 1.0lf;", "float x = 1.0LF;"] {
        assert!(Shader::parse(&format!("[glsl|{source}|]")).is_err());
    }
}

#[test]
fn historical_glsl_acceptance_corpus() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("upstream/language-glsl-0.3.0/cases.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let source = case["shader"].as_str().unwrap();
        let result = Shader::parse(&format!("[glsl|{source}|]"));
        assert_eq!(
            result.is_ok(),
            case["accepted"].as_bool().unwrap(),
            "{}: {source}: {result:?}",
            case["group"]
        );
    }
}
