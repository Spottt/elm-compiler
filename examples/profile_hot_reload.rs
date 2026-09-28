//! Developer-only profiler for repeated analysis and linking in one session.
//! No make/output cache or Webpack: timings identify compiler work, not HMR latency.
use std::{io::{self, BufRead, Write}, path::PathBuf, time::Instant};
use planexpo_elm::{analyze, cache, kernel, linker, project, session_cache, typed_cache};
use sha2::{Digest, Sha256};

fn main() -> Result<(), String> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 4 { return Err("usage: profile_hot_reload manifest entry elm_home compiler; stdin: JSON changed-path arrays".into()); }
    let identity = cache::compiler_identity(&args[3]).map_err(|error| error.to_string())?;
    let directory = args[0].parent().ok_or("manifest needs parent")?.join("elm-stuff/planexpo-rust/types-v1");
    let _session = session_cache::scope(true);
    for line in io::stdin().lock().lines() {
        let input: serde_json::Value = serde_json::from_str(&line.map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
        let (entry, changed) = if input.is_array() {
            (args[1].clone(), input)
        } else {
            (PathBuf::from(input.get("entry").and_then(serde_json::Value::as_str).ok_or("request needs entry")?),
                input.get("changed").cloned().ok_or("request needs changed paths")?)
        };
        let changed: Vec<PathBuf> = serde_json::from_value(changed).map_err(|error| error.to_string())?;
        session_cache::set_changed_paths(changed);
        let start = Instant::now();
        let graph = project::discover(&args[0], &entry, &args[2])?;
        let discovery_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start_cache = Instant::now();
        let types = typed_cache::TypeCache::new(&directory, &graph, &identity, kernel::Mode::Development)?;
        let cache_setup_ms = start_cache.elapsed().as_secs_f64() * 1000.0;
        let start_analysis = Instant::now();
        let report = analyze::generate_modules_cached(&graph, kernel::Mode::Development, &types)?;
        let analysis_ms = start_analysis.elapsed().as_secs_f64() * 1000.0;
        let modules: Vec<_> = report.module_timings.iter().map(|m| serde_json::json!({
            "module":m.module,"parse_ms":m.parse_ms,"preparation_ms":m.preparation_ms,
            "inference_ms":m.inference_ms,"validation_ms":m.validation_ms,
            "type_restore_ms":m.type_restore_ms,
            "type_load_ms":m.type_load_ms,"type_decode_ms":m.type_decode_ms,"type_import_ms":m.type_import_ms,"type_check_ms":m.type_check_ms,
            "type_artifact_ms":m.type_artifact_ms,"interface_ms":m.interface_ms,
            "generation_ms":m.generation_ms,"compaction_ms":m.compaction_ms,
            "finalization_ms":m.finalization_ms,"total_ms":m.total_ms,
        })).collect();
        let cached_type_modules = report.cached_type_modules;
        let compiled_modules = report.compiled_modules.clone();
        let resumed_modules = report.resumed_modules;
        let cached_generated_modules = report.cached_generated_modules;
        let start_linking = Instant::now();
        let output = linker::assemble(&graph, report)?;
        let linking_ms = start_linking.elapsed().as_secs_f64() * 1000.0;
        println!("{}", serde_json::json!({"entry":entry,"cached_type_modules":cached_type_modules,"compiled_modules":compiled_modules,"discovery_ms":discovery_ms,"cache_setup_ms":cache_setup_ms,
            "analysis_ms":analysis_ms,"linking_ms":linking_ms,"total_ms":start.elapsed().as_secs_f64()*1000.0,
            "resumed_modules":resumed_modules,"cached_generated_modules":cached_generated_modules,"output_bytes":output.len(),
            "output_sha256":format!("{:x}",Sha256::digest(output.as_bytes())),"modules":modules,
            "cache":session_cache::statistics(),"analysis":analyze::worker_statistics()}));
        io::stdout().flush().map_err(|error|error.to_string())?;
    }
    Ok(())
}
