//! Compare separate development bundles with one shared analysis, byte for byte.
//! No worker or Webpack integration is enabled by this developer check.
use std::{path::PathBuf, time::Instant};
use planexpo_elm::{analyze, linker, project};
use sha2::{Digest, Sha256};
fn main() -> Result<(), String> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() < 4 { return Err("usage: check_grouped_entries manifest elm_home entry entry...".into()); }
    let start = Instant::now();
    let graph = project::discover_many(&args[0], &args[2..], &args[1])?;
    let report = analyze::generate_modules(&graph)?;
    let shared_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut rows = Vec::new();
    for entry in &args[2..] {
        let single = project::discover(&args[0], entry, &args[1])?;
        let start = Instant::now();
        let grouped = linker::assemble_entry(&single, report.clone())?;
        let link_ms = start.elapsed().as_secs_f64() * 1000.0;
        let independent = linker::assemble(&single, analyze::generate_modules(&single)?)?;
        if grouped != independent {
            let offset = grouped.bytes().zip(independent.bytes()).position(|(a,b)| a != b)
                .unwrap_or(grouped.len().min(independent.len()));
            return Err(format!("{} differs at byte {offset}: grouped={} bytes, separate={} bytes", entry.display(), grouped.len(), independent.len()));
        }
        rows.push(serde_json::json!({"entry":entry,"bytes":grouped.len(),"sha256":format!("{:x}",Sha256::digest(grouped.as_bytes())),"clone_and_link_ms":link_ms}));
    }
    println!("{}", serde_json::json!({"identical":true,"shared_analysis_ms":shared_ms,"entries":rows}));
    Ok(())
}
