use planexpo_elm::html::render;

#[test]
fn closes_only_the_html_shell_script_and_preserves_nested_entry_names() {
    let html = render(
        "Nested.Main",
        "var text = '</ScRiPt><script>bad()</script>';",
    );
    assert_eq!(html.to_lowercase().matches("</script>").count(), 1);
    assert!(html.contains("'<\\/ScRiPt><script>bad()<\\/script>'"));
    assert!(html.contains("Elm[\"Nested\"][\"Main\"].init"));
    assert!(html.contains("Initialization Error"));
}
