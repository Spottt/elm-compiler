use planexpo_elm::{
    ast::Span,
    source_error::{locate, locate_slice},
};

#[test]
fn borrowed_identifier_uses_its_occurrence_and_unicode_columns() {
    let source = "é = value\nλ = value";
    let start = source.rfind("value").unwrap();
    assert_eq!(
        locate_slice(source, &source[start..], "unknown name value".into()),
        "2:5:2:10: unknown name value"
    );
}

#[test]
fn unrelated_text_does_not_invent_a_region() {
    let source = String::from("value = value");
    let other = String::from("value");
    assert_eq!(locate_slice(&source, &other, "failure".into()), "failure");
}

#[test]
fn inner_region_survives_outer_context() {
    let source = "value = missing";
    let inner = locate_slice(source, &source[8..], "unknown name missing".into());
    assert_eq!(
        locate(source, Span { start: 0, end: 15 }, inner.clone()),
        inner
    );
}

#[test]
fn multiple_source_failures_roundtrip_without_losing_individual_regions() {
    use planexpo_elm::source_error::{batch, batch_messages};
    let messages = vec![
        "/tmp/Alpha.elm:2:3:2:8: unknown name absent".into(),
        "ELM_DOCS_JSON:{\"type\":\"compile-errors\"}".into(),
    ];
    let encoded = batch(messages.clone());
    assert_eq!(batch_messages(&encoded), Some(messages));
    assert_eq!(batch(vec!["single error".into()]), "single error");
    assert_eq!(batch_messages("ELM_SOURCE_ERRORS:invalid"), None);
}

#[test]
fn discovered_module_names_survive_an_unparseable_header() {
    use planexpo_elm::source_error::{in_module, module_message};
    let encoded = in_module(
        "Alpha.Inner",
        "/project/Alpha/Inner.elm:2:1: missing exposed name".into(),
    );
    assert_eq!(
        module_message(&encoded),
        Some((
            "Alpha.Inner".into(),
            "/project/Alpha/Inner.elm:2:1: missing exposed name".into()
        ))
    );
    assert!(module_message("ELM_SOURCE_MODULE:invalid").is_none());
}
