//! Compile a REPL candidate without committing session state. The entry path
//! belongs to the caller's temporary directory, never a user source file.
use crate::{analyze, linker, project, repl_type};
use std::{fs, path::Path};

pub fn compile(
    manifest: &Path,
    home: &Path,
    entry: &Path,
    source: &str,
    binding: Option<&str>,
    ansi: bool,
) -> Result<Option<String>, String> {
    fs::write(entry, source).map_err(|e| e.to_string())?;
    let selected = crate::package_resolution::resolve(manifest, home)?;
    crate::dependency_build::verify(manifest, home, &selected)?;
    let graph = project::discover(manifest, entry, home)?;
    let report = analyze::generate_for_repl(&graph)?;
    if !report.generation_errors.is_empty() {
        return Err(report.generation_errors.join("\n"));
    }
    let Some(binding) = binding else {
        return Ok(None);
    };
    let artifact = report.repl_types.as_ref().ok_or("missing inferred types")?;
    let scheme = artifact["entries"]
        .as_array()
        .ok_or("missing schemes")?
        .iter()
        .find(|e| e["name"] == binding)
        .ok_or("missing binding scheme")?;
    let annotation = repl_type::render(
        &artifact["graph"],
        scheme["root"].as_u64().ok_or("missing root")? as usize,
        report.repl_type_names.as_ref().ok_or("missing names")?,
    )?;
    linker::assemble_for_repl(&graph, report, binding, &annotation, ansi).map(Some)
}
