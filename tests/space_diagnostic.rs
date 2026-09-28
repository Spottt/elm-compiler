use planexpo_elm::space_diagnostic::report;
use std::path::Path;

#[test]
fn comments_highlight_two_columns_but_tabs_are_points() {
    for (source, kind, title, width) in [
        (
            "{- unclosed",
            "unterminated block comment",
            "ENDLESS COMMENT",
            2,
        ),
        ("\t", "tabs are not allowed in Elm layout", "NO TABS", 0),
    ] {
        let value = report(source, "Main", Path::new("Main.elm"), 1, 1, kind).unwrap();
        let problem = &value["errors"][0]["problems"][0];
        assert_eq!(problem["title"], title);
        assert_eq!(problem["region"]["end"]["column"], 1 + width);
    }
}
