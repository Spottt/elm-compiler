//! REPL backend probe. Accepts explicit type text or --infer-structural for the
//! incomplete inferred type renderer; this is not an interactive CLI.
use std::{env, fs, path::Path};
fn main() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    let [
        home,
        manifest,
        entry,
        binding,
        annotation,
        output,
        options @ ..,
    ] = args.as_slice()
    else {
        return Err(
            "expected ELM_HOME, elm.json, entry, binding, type text, output.js [--ansi]".into(),
        );
    };
    if !options.is_empty() && options != ["--ansi"] {
        return Err("only --ansi is supported".into());
    }
    let graph =
        planexpo_elm::project::discover(Path::new(manifest), Path::new(entry), Path::new(home))?;
    let report = planexpo_elm::analyze::generate_for_repl(&graph)?;
    let inferred;
    let annotation = if annotation == "--infer-structural" {
        let artifact = report.repl_types.as_ref().ok_or("missing inferred types")?;
        let entry = artifact["entries"]
            .as_array()
            .ok_or("missing schemes")?
            .iter()
            .find(|entry| entry["name"] == binding.as_str())
            .ok_or("missing binding type")?;
        inferred = planexpo_elm::repl_type::render(
            &artifact["graph"],
            entry["root"].as_u64().ok_or("missing scheme root")? as usize,
            report
                .repl_type_names
                .as_ref()
                .ok_or("missing type localizer")?,
        )?;
        &inferred
    } else {
        annotation
    };
    let script = planexpo_elm::linker::assemble_for_repl(
        &graph,
        report,
        binding,
        annotation,
        !options.is_empty(),
    )?;
    fs::write(output, script).map_err(|e| e.to_string())
}
