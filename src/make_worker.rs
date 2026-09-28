//! Line-framed JSON protocol for an owned development compiler process
//! (`--make-worker`, documented in docs/worker.md).
//! Fixed cwd/environment; EOF releases the process and all bounded caches.
use std::io::{BufRead, Write};
use serde_json::{Value, json};

fn respond(value: Value) -> Result<(), String> {
    let mut output = std::io::stdout().lock();
    serde_json::to_writer(&mut output, &value).map_err(|e| e.to_string())?;
    writeln!(output).and_then(|_| output.flush()).map_err(|e| e.to_string())
}
pub(crate) struct Request {
    pub arguments: Vec<String>,
    pub dependencies: Option<std::path::PathBuf>,
    pub changed: Vec<std::path::PathBuf>,
}
fn parse(value: &Value) -> Result<Request, String> {
    let args = value.get("arguments").and_then(Value::as_array)
        .and_then(|values| values.iter().map(|v| v.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>())
        .ok_or("invalid worker arguments")?;
    let dependencies = match value.get("dependencies") {
        Some(Value::String(path)) => Some(std::path::PathBuf::from(path)),
        None | Some(Value::Null) => None,
        _ => return Err("invalid worker dependency path".into()),
    };
    let changed = match value.get("changed") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(paths)) => paths.iter().map(|path| path.as_str().map(std::path::PathBuf::from))
            .collect::<Option<Vec<_>>>().ok_or("invalid worker changed paths")?,
        _ => return Err("invalid worker changed paths".into()),
    };
    Ok(Request { arguments: args, dependencies, changed })
}
fn request(value: &Value) -> Result<(), String> {
    let request = parse(value)?;
    planexpo_elm::session_cache::set_changed_paths(request.changed);
    crate::make::run_worker(request.arguments, request.dependencies.as_deref())
}
fn batch(value: &Value) -> Result<(Vec<Result<(), String>>, usize), String> {
    let values = value.as_array().filter(|items| !items.is_empty() && items.len() <= 32)
        .ok_or("worker batch must contain 1 to 32 requests")?;
    // Invalid items retain their position and never prevent valid neighbors
    // from completing. They use the ordinary path; batches are not recursive.
    match values.iter().map(parse).collect::<Result<Vec<_>, _>>() {
        Ok(requests) => Ok(crate::make::run_worker_batch(&requests)),
        Err(_) => Ok((values.iter().map(request).collect(), 0)),
    }
}
fn batch_stream(value: &Value) -> Result<Value, String> {
    let values = value.as_array().filter(|items| !items.is_empty() && items.len() <= 32)
        .ok_or("worker batch must contain 1 to 32 requests")?;
    let mut ready = |index, result: &Result<(), String>| respond(json!({"protocol":1,"index":index,"result":outcome(result.clone())}));
    let shared = match values.iter().map(parse).collect::<Result<Vec<_>, _>>() {
        Ok(requests) => crate::make::run_worker_batch_stream(&requests, &mut ready)?.1,
        Err(_) => {
            for (index, value) in values.iter().enumerate() { ready(index, &request(value))?; }
            0
        }
    };
    Ok(json!({"protocol":1,"batch_done":true,"count":values.len(),"shared_analysis_entries":shared}))
}
fn outcome(result: Result<(), String>) -> Value {
    let (error, report) = match &result {
        Ok(()) => (None, None),
        Err(error) => {
            let terminal = if let Some(raw) = error.strip_prefix("ELM_CLI_RAW:") {
                crate::diagnostic::proxy_cli_error(raw, "make", false).unwrap_or_else(|| raw.to_owned())
            } else { format!("{}\n", crate::diagnostic::terminal(error)) };
            (Some(terminal), Some(crate::diagnostic::report(error)))
        }
    };
    json!({"protocol":1,"ok":result.is_ok(),"error":error,"report":report,"cache":planexpo_elm::session_cache::statistics(),"analysis":planexpo_elm::analyze::worker_statistics()})
}
pub fn run() -> Result<(), String> {
    let _cache = planexpo_elm::session_cache::scope(true);
    let _outputs = planexpo_elm::cache::output_memory_scope(std::env::var("PLANEXPO_ELM_MEMORY_OUTPUT_CACHE").as_deref() == Ok("1"));
    let _identity = planexpo_elm::cache::running_compiler_identity();
    respond(json!({"protocol":1,"ready":true,"capabilities":{"batch":true,"streaming_batch":true},"executable":std::env::current_exe().ok()}))?;
    let profile = std::env::var("PLANEXPO_ELM_PROFILE_WORKER").as_deref() == Ok("1")
        || std::env::var("PLANEXPO_ELM_PROFILE_MAKE").as_deref() == Ok("1");
    let input = std::io::stdin();
    for line in input.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        let started = profile.then(std::time::Instant::now);
        let parsed = serde_json::from_str::<Value>(&line).map_err(|e| e.to_string());
        let mut response = match parsed {
            Ok(value) if value.get("batch").is_some() && value.get("stream").and_then(Value::as_bool) == Some(true) => batch_stream(&value["batch"]).unwrap_or_else(|error| outcome(Err(error))),
            Ok(value) if value.get("batch").is_some() => match batch(&value["batch"]) {
                Ok((results, shared)) => json!({"protocol":1,"results":results.into_iter().map(outcome).collect::<Vec<_>>(),"shared_analysis_entries":shared}),
                Err(error) => outcome(Err(error)),
            },
            value => outcome(value.and_then(|value| request(&value))),
        };
        // Developer-only timing: include discovery, analysis, output I/O and
        // diagnostic rendering. The JS observer can distinguish this from
        // delayed delivery while Webpack occupies its event loop.
        if let Some(started) = started {
            response["timing"] = json!({"request_ms":started.elapsed().as_secs_f64()*1000.0});
        }
        respond(response)?;
    }
    Ok(())
}
