//! Inspection helper for differential documentation tests; not a make --docs implementation.
fn main() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or("expected an Elm source path")?;
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if std::env::args().any(|arg| {
        arg == "--associated" || arg == "--validate-names" || arg == "--validate-definitions"
    }) {
        let (ast, docs) = planexpo_elm::parser::parse_with_docs(&source)?;
        if std::env::args().any(|arg| arg == "--validate-names") {
            planexpo_elm::docs::validate_names(&ast, &docs)
                .map_err(|error| format!("{error:?}"))?;
        }
        if std::env::args().any(|arg| arg == "--validate-definitions") {
            let exports =
                planexpo_elm::docs::validate(&ast, &docs).map_err(|error| format!("{error:?}"))?;
            let declarations = exports
                .iter()
                .map(|export| {
                    (
                        export.name.as_str(),
                        export.comment.normalized_text(&source),
                    )
                })
                .collect::<std::collections::BTreeMap<_, _>>();
            println!(
                "{}",
                serde_json::json!({ "overview": docs.overview.map(|comment| comment.normalized_text(&source)), "declarations": declarations })
            );
            return Ok(());
        }
        let declarations = docs
            .declarations
            .iter()
            .map(|(name, comment)| (*name, comment.normalized_text(&source)))
            .collect::<std::collections::BTreeMap<_, _>>();
        println!(
            "{}",
            serde_json::json!({
                "overview": docs.overview.map(|comment| comment.normalized_text(&source)),
                "declarations": declarations
            })
        );
        return Ok(());
    }
    let (_, comments) = planexpo_elm::lexer::lex_with_docs(&source)?;
    let texts = comments
        .iter()
        .map(|comment| comment.normalized_text(&source))
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string(&texts).map_err(|e| e.to_string())?
    );
    Ok(())
}
