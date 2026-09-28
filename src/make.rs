//! Conventional invocation for build tools; inspection commands remain separate.
use crate::make_cli::flag_value_error;
mod batch;
pub(crate) use batch::{run as run_worker_batch, run_stream as run_worker_batch_stream};
use planexpo_elm::kernel::Mode;
use std::{
    cell::Cell,
    env, fs,
    io::{IsTerminal, Write},
    path::PathBuf,
    rc::Rc,
};

// Opt-in developer tracing; default CLI and worker streams stay unchanged.
fn profile_phase<T>(enabled: bool, entries: &[PathBuf], phase: &str, run: impl FnOnce() -> T) -> T {
    if !enabled { return run(); }
    let start = std::time::Instant::now();
    let result = run();
    // Stderr is unbuffered. Serialize first, then issue one write: streaming a
    // JSON Value through eprintln makes many tiny writes and can block the
    // compiler while Webpack's JS thread is busy draining neither pipe.
    let line = format!("ELM_MAKE_PROFILE {}\n", serde_json::json!({"entries":entries,"phase":phase,"ms":start.elapsed().as_secs_f64()*1000.0}));
    let _ = std::io::stderr().write_all(line.as_bytes());
    result
}

// Private build-tool metadata; never changes the public CLI streams or result.
// The loader reads it only after the compiler exits. A failed write falls back
// to dependency discovery in the loader, not to a compilation failure.
fn write_loader_dependencies(graph: &planexpo_elm::project::Graph, cwd: &std::path::Path, destination: Option<&std::path::Path>) {
    let Some(destination) = destination else { return; };
    let files: Option<std::collections::BTreeSet<String>> = graph.manifests.iter()
        .map(|(path, _)| path)
        .chain(graph.modules.iter().map(|module| &module.path))
        .map(|path| cwd.join(path).to_str().map(str::to_owned))
        .collect();
    if let Some(files) = files
        && let Ok(bytes) = serde_json::to_vec(&serde_json::json!({"version":1,"files":files})) {
        let _ = fs::write(destination, bytes);
    }
}

fn remember_import_names(graph: &planexpo_elm::project::Graph, failed: &[String]) {
    let Some((owner, _)) = graph.entry.split_once(':') else { return; };
    let Some((manifest, _)) = graph.manifests.first() else { return; };
    let mut blocked = std::collections::BTreeSet::new();
    let mut names = std::collections::BTreeSet::new();
    for module in &graph.modules {
        let id = format!("{}:{}", module.owner, module.name);
        if failed.iter().any(|path| module.path == std::path::Path::new(path))
            || module.imports.iter().any(|import| blocked.contains(import)) {
            blocked.insert(id);
        } else if module.owner == owner && !module.kernel {
            names.insert(module.name.clone());
        }
    }
    let _ = planexpo_elm::import_history::remember(manifest, names);
}

fn remember_analysis(
    graph: &planexpo_elm::project::Graph,
    outcome: Result<planexpo_elm::analyze::Report, String>,
    enabled: bool,
) -> Result<planexpo_elm::analyze::Report, String> {
    if enabled {
        match &outcome {
            Ok(_) => remember_import_names(graph, &[]),
            Err(error) => {
                let report = crate::diagnostic::report(error);
                if report["type"] == "compile-errors" {
                    let failed: Vec<_> = report["errors"].as_array().into_iter().flatten()
                        .filter_map(|error| error["path"].as_str().map(str::to_owned)).collect();
                    if !failed.is_empty() { remember_import_names(graph, &failed); }
                }
            }
        }
    }
    outcome
}

// Keep the actual failing ancestor: create_dir_all only returns an error and
// loses which recursive directory creation failed.
fn create_output_directory(path: &std::path::Path) -> Result<(), (PathBuf, std::io::Error)> {
    let mut pending = vec![path.to_path_buf()];
    while let Some(current) = pending.pop() {
        if current.as_os_str().is_empty() {
            continue;
        }
        match fs::create_dir(&current) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && current.is_dir() => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(parent) = current.parent().filter(|p| !p.as_os_str().is_empty()) else {
                    return Err((current, error));
                };
                let parent = parent.to_path_buf();
                // An existing parent means the missing component cannot be
                // repaired by creating directories (e.g. a dangling symlink).
                if parent.is_dir() {
                    return Err((current, error));
                }
                pending.push(current);
                pending.push(parent);
            }
            Err(error) => return Err((current, error)),
        }
    }
    Ok(())
}

struct Options {
    entries: Vec<PathBuf>,
    docs: Option<PathBuf>,
    output: PathBuf,
    explicit_output: bool,
    json: bool,
    cache: bool,
    mode: Mode,
    clashing: bool,
    incremental: bool,
}

struct ProgressLine(bool);

impl Drop for ProgressLine {
    fn drop(&mut self) {
        if self.0 {
            // Leave stderr diagnostics on a fresh line when analysis fails.
            println!();
        }
    }
}

fn no_outline() -> String {
    format!(
        "ELM_DEPENDENCY_JSON:{}",
        serde_json::json!({
            "type":"error", "path":null, "title":"NO elm.json FILE",
            "message":["It looks like you are starting a new Elm project. Very exciting! Try running:\n\n    ",
                {"bold":false,"underline":false,"color":"GREEN","string":"elm init"},
                "\n\nIt will help you get set up. It is really simple!"]
        })
    )
}

fn missing_entry(path: &str) -> String {
    format!(
        "ELM_DEPENDENCY_JSON:{}",
        serde_json::json!({
            "type":"error", "path":null, "title":"FILE NOT FOUND",
            "message":["I cannot find this file:\n\n    ",
                {"bold":false,"underline":false,"color":"RED","string":path},
                "\n\nIs there a typo?\n\n",
                {"bold":false,"underline":true,"color":null,"string":"Note"},
                ": If you are just getting started, try working through the examples in the\nofficial guide https://guide.elm-lang.org to get an idea of the kinds of things\nthat typically go in a src/Main.elm file."]
        })
    )
}

fn take_switch(args: &mut Vec<String>, flag: &str) -> Result<bool, String> {
    let prefix = format!("{flag}=");
    let Some(index) = args
        .iter()
        .position(|arg| arg == flag || arg.starts_with(&prefix))
    else {
        return Ok(false);
    };
    let arg = args.remove(index);
    if arg.starts_with(&prefix) {
        return Err(crate::make_cli::boolean_value(&arg));
    }
    Ok(true)
}

fn take_value(args: &mut Vec<String>, flag: &str) -> Result<Option<String>, String> {
    let prefix = format!("{flag}=");
    let Some(index) = args
        .iter()
        .position(|arg| arg == flag || arg.starts_with(&prefix))
    else {
        return Ok(None);
    };
    let arg = args.remove(index);
    if let Some(value) = arg.strip_prefix(&prefix) {
        return Ok(Some(value.to_owned()));
    }
    if args.get(index).is_none_or(|arg| arg.starts_with('-')) {
        return Err(flag_value_error(flag, None));
    }
    Ok(Some(args.remove(index)))
}

fn clashing_flags() -> String {
    use serde_json::json;
    let red = |text| json!({"bold":false,"underline":false,"color":"RED","string":text});
    format!(
        "ELM_DEPENDENCY_JSON:{}",
        json!({
            "type":"error", "path":null, "title":"CLASHING FLAGS",
            "message":["I cannot compile with ", red("--optimize"), " and ", red("--debug"),
                " at the same time.\n\nI need to take away information to optimize things, and I need to add\ninformation to add the debugger. It is impossible to do both at once though!\nPick just one of those flags and it should work!"]
        })
    )
}

impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        // Terminal.Chomp consumes one occurrence of each declared flag in this
        // fixed order. Repeated flags remain for the unknown-flag check below.
        let mut args = args;
        let debug = take_switch(&mut args, "--debug")?;
        let optimize = take_switch(&mut args, "--optimize")?;
        let output = take_value(&mut args, "--output")?
            .map(|value| {
                if matches!(value.as_str(), "/dev/null" | "NUL" | "$null") {
                    Ok(PathBuf::from("/dev/null"))
                } else if (value.ends_with(".js") && value.len() > 3)
                    || (value.ends_with(".html") && value.len() > 5)
                {
                    Ok(PathBuf::from(value))
                } else {
                    Err(flag_value_error("--output", Some(&value)))
                }
            })
            .transpose()?;
        let json = match take_value(&mut args, "--report")? {
            Some(value) if value == "json" => true,
            Some(value) => return Err(flag_value_error("--report", Some(&value))),
            None => false,
        };
        let docs = take_value(&mut args, "--docs")?
            .map(|value| {
                if value.ends_with(".json") && value.len() > 5 {
                    Ok(PathBuf::from(value))
                } else {
                    Err(flag_value_error("--docs", Some(&value)))
                }
            })
            .transpose()?;
        let mut cache = true;
        let mut incremental = false;
        args.retain(|arg| match arg.as_str() {
            "--no-cache" => {
                cache = false;
                false
            }
            "--incremental" => {
                incremental = true;
                false
            }
            _ => true,
        });
        if let Some(flag) = args.iter().find(|arg| arg.starts_with('-')) {
            return Err(crate::make_cli::unknown(flag));
        }
        if let Some(path) = args.iter().find(|path| !path.ends_with(".elm")) {
            return Err(crate::make_cli::bad_file(path));
        }
        let entries: Vec<_> = args.into_iter().map(PathBuf::from).collect();
        let mode = if debug {
            Mode::Debug
        } else if optimize {
            Mode::Production
        } else {
            Mode::Development
        };
        let explicit_output = output.is_some();
        let output = output.unwrap_or_else(|| {
            PathBuf::from(if entries.len() == 1 {
                "index.html"
            } else {
                "elm.js"
            })
        });
        Ok(Self {
            entries,
            docs,
            output,
            explicit_output,
            json,
            cache,
            mode,
            clashing: debug && optimize,
            incremental,
        })
    }
}

/// Flag consumption can bring a value next to --report after an intervening
/// switch is removed. Reuse the parser when choosing the diagnostic protocol.
pub fn json_report(args: &[String]) -> bool {
    Options::parse(args.to_vec()).is_ok_and(|options| options.json)
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let mut progress_line = ProgressLine(false);
    let dependencies = env::var_os("PLANEXPO_ELM_DEPENDENCIES_FILE").map(PathBuf::from);
    let result = run_inner(args, &mut progress_line, false, dependencies.as_deref(), None);
    if let Err(error) = &result
        && progress_line.0
    {
        let report = crate::diagnostic::report(error);
        let count = report["errors"].as_array().map(Vec::len);
        // Elm writes the diagnostic before completing the progress line. This
        // order is visible when build tools merge stdout and stderr.
        eprintln!("{}", crate::diagnostic::terminal(error));
        match count {
            Some(1) => println!("\rDetected problems in 1 module."),
            Some(n) if n > 1 => println!("\rDetected problems in {n} modules."),
            _ => println!("\rDetected a problem."),
        }
        progress_line.0 = false;
        // Both streams are already rendered; main must not print it twice.
        return Err("ELM_CLI_RAW:".into());
    }
    result
}

pub fn run_worker(args: Vec<String>, dependencies: Option<&std::path::Path>) -> Result<(), String> {
    run_inner(args, &mut ProgressLine(false), true, dependencies, None)
}

fn run_inner(args: Vec<String>, progress_line: &mut ProgressLine, quiet: bool, dependencies: Option<&std::path::Path>, prepare: Option<&mut Option<batch::Prepared>>) -> Result<(), String> {
    let mut options = Options::parse(args)?;
    let profile_make = env::var("PLANEXPO_ELM_PROFILE_MAKE").as_deref() == Ok("1");
    let profile_entries = if profile_make { options.entries.clone() } else { Vec::new() };
    macro_rules! trace { ($name:expr, $body:expr) => { profile_phase(profile_make, &profile_entries, $name, || $body) }; }
    if quiet { options.json = true; }
    let _uncached = (!options.cache).then(|| planexpo_elm::session_cache::scope(false));
    let cwd = env::current_dir().map_err(|e| e.to_string())?;
    let manifest = cwd
        .ancestors()
        .map(|p| p.join("elm.json"))
        .find(|p| p.is_file())
        .ok_or_else(no_outline)?;
    if options.clashing {
        return Err(clashing_flags());
    }
    let config =
        planexpo_elm::outline::decode(&fs::read_to_string(&manifest).map_err(|e| e.to_string())?)?;
    planexpo_elm::outline::validate(&config, manifest.parent().unwrap())
        .map_err(|problem| problem.encode())?;
    let document_package = options.entries.is_empty() && options.docs.is_some();
    if options.entries.is_empty() {
        options.entries = planexpo_elm::project::exposed_entries(&manifest)?;
        options.output = PathBuf::from("/dev/null");
        options.explicit_output = true;
    }
    let home = env::var_os("ELM_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".elm")))
        .ok_or("ELM_HOME or HOME is required")?;
    let downloaded = Rc::new(Cell::new(false));
    let _downloads = if options.json {
        None
    } else {
        let downloaded = downloaded.clone();
        let ansi = std::io::stdout().is_terminal();
        Some(planexpo_elm::package_progress::subscribe(move |event| {
            use planexpo_elm::package_progress::Event;
            match event {
                Event::Started { .. } => {
                    if !downloaded.replace(true) {
                        println!("Starting downloads...\n");
                    }
                }
                Event::Finished {
                    name,
                    version,
                    success,
                } => {
                    let mark = match (cfg!(windows), success) {
                        (true, true) => "+",
                        (true, false) => "X",
                        (false, true) => "●",
                        (false, false) => "✗",
                    };
                    if ansi {
                        println!(
                            "  \x1b[{}m{mark}\x1b[0m {name} {version}",
                            if success { 92 } else { 91 }
                        );
                    } else {
                        println!("  {mark} {name} {version}");
                    }
                }
            }
            let _ = std::io::stdout().flush();
        }))
    };
    let resolution = trace!("package_resolution", planexpo_elm::package_resolution::resolve_for_build(&manifest, &home))?;
    let selected_packages = Some(resolution.selected);
    trace!("dependency_verification", planexpo_elm::dependency_build::verify_downloads_with_progress(
        &manifest,
        &home,
        selected_packages.as_ref().unwrap(),
        !options.json,
        downloaded.get(),
        &resolution.failures,
    ))?;
    let identity = if options.cache {
        planexpo_elm::cache::running_compiler_identity()
    } else {
        None
    };
    let mut cached = 0;
    let mut cached_types = 0;
    let mut compiled = 0;
    let mut generated_names = Vec::new();
    if !options.json {
        print!("Compiling ...");
        let _ = std::io::stdout().flush();
        progress_line.0 = true;
    }
    {
        // Arguments are relative to the invoking directory, not to elm.json.
        let entries: Vec<_> = options
            .entries
            .iter()
            .map(|entry| cwd.join(entry))
            .collect();
        for (entry, requested) in entries.iter().zip(&options.entries) {
            if !entry.exists() {
                return Err(missing_entry(&requested.to_string_lossy()));
            }
        }
        let mut graph = trace!("discovery", planexpo_elm::project::discover_many_selected(
            &manifest,
            &entries,
            &home,
            false,
            selected_packages.as_ref(),
        ))?;
        trace!("loader_dependencies", write_loader_dependencies(&graph, &cwd, dependencies));
        let owner = graph
            .entry
            .split_once(':')
            .ok_or("invalid entry identity")?
            .0
            .to_string();
        let count_compiled = |report: &planexpo_elm::analyze::Report| {
            report
                .compiled_modules
                .iter()
                .filter(|name| {
                    name.split_once(':')
                        .is_some_and(|(package, _)| package == owner)
                })
                .count()
        };
        let _progress = if options.json {
            None
        } else {
            let progress_owner = owner.clone();
            let done = Rc::new(Cell::new(0usize));
            Some(planexpo_elm::build_progress::subscribe(move |module| {
                if module
                    .split_once(':')
                    .is_some_and(|(package, _)| package == progress_owner)
                {
                    done.set(done.get() + 1);
                    print!("\rCompiling ({})", done.get());
                    let _ = std::io::stdout().flush();
                }
            }))
        };
        let mains = trace!("entry_detection", (|| -> Result<Vec<String>, String> {
            let mut mains = Vec::new();
            for module in &graph.modules {
                let canonical = format!("{}:{}", module.owner, module.name);
                if !graph.entries.contains(&canonical) {
                    continue;
                }
                let ast = match planexpo_elm::parser::parse(&module.source) {
                    Ok(ast) => ast,
                    Err(error) => {
                        // Main/output detection must not hide independent source errors.
                        let analysis = if document_package {
                            planexpo_elm::analyze::documentation(&graph)
                        } else {
                            planexpo_elm::analyze::check_types(&graph)
                        };
                        let analysis = remember_analysis(&graph, analysis, options.cache);
                        return Err(analysis
                            .err()
                            .unwrap_or_else(|| format!("{}:{error}", module.path.display())));
                    }
                };
                if ast.declarations.iter().any(|d| {
                    matches!(
                        d,
                        planexpo_elm::ast::Declaration::Value { name: "main", .. }
                    )
                }) {
                    mains.push(canonical);
                }
            }
            Ok(mains)
        })())?;
        if !options.explicit_output {
            options.output = match mains.len() {
                0 => PathBuf::from("/dev/null"),
                1 => PathBuf::from("index.html"),
                _ => PathBuf::from("elm.js"),
            };
        } else if options.output != std::path::Path::new("/dev/null")
            && (mains.len() != graph.entries.len()
                || (options
                    .output
                    .to_str()
                    .is_some_and(|path| path.ends_with(".html"))
                    && graph.entries.len() > 1))
        {
            // Elm reports source errors before checking whether the requested
            // output can be initialized. Preserve the same diagnostic as a
            // check-only invocation, even when an entry has no main value.
            let report = remember_analysis(&graph, planexpo_elm::analyze::check_types(&graph), options.cache)?;
            if !options.json {
                print!(
                    "\r{}",
                    success_message(count_compiled(&report), &[], &options.output)
                );
                progress_line.0 = false;
            }
            let missing = graph
                .entries
                .iter()
                .filter(|entry| !mains.contains(entry))
                .filter_map(|entry| entry.split_once(':').map(|(_, name)| name.to_owned()))
                .collect::<Vec<_>>();
            return Err(planexpo_elm::make_output::error(
                options
                    .output
                    .to_str()
                    .is_some_and(|path| path.ends_with(".html")),
                &missing,
                graph.entries.len(),
            ));
        }

        if document_package {
            let report = remember_analysis(&graph, planexpo_elm::analyze::documentation(&graph), options.cache)?;
            compiled = count_compiled(&report);
            let path = options
                .docs
                .as_ref()
                .ok_or("missing documentation output path")?;
            let bytes = serde_json::to_vec(&report.documentation).map_err(|e| e.to_string())?;
            fs::write(path, bytes).map_err(|error| {
                // Unlike bundle output failures, Elm leaves the compilation
                // progress line untouched when documentation writing fails.
                progress_line.0 = false;
                crate::diagnostic::output_io_error(path, "openBinaryFile", error)
            })?;
        } else if options.output == std::path::Path::new("/dev/null") {
            compiled = count_compiled(&remember_analysis(&graph, planexpo_elm::analyze::check_types(&graph), options.cache)?);
        } else {
            graph = if graph.entries.len() == 1 {
                trace!("complete_runtime", planexpo_elm::project::complete_runtime(graph))?
            } else {
                planexpo_elm::project::discover_many_selected(
                    &manifest, &entries, &home, true, selected_packages.as_ref(),
                )?
            };
            let source_digests = identity.as_ref().map(|_| trace!("source_digests", planexpo_elm::cache::SourceDigests::new(&graph)));
            let cache = identity.as_ref().map(|id| {
                trace!("output_key", planexpo_elm::cache::OutputCache::new_with_digests(
                    &manifest
                        .parent()
                        .unwrap()
                        .join("elm-stuff/planexpo-rust/v1"),
                    source_digests.as_ref().unwrap(),
                    id,
                    options.mode,
                ))
            });
            // Kept opt-in while measuring complete watcher sessions. Only
            // successful development JS can use the trivia-equivalent key.
            let mut trivia_cache = None;
            let reused_output = cache.as_ref().and_then(|c| trace!("output_cache_read", c.load())).or_else(|| {
                if options.incremental && options.cache
                    && options.mode == planexpo_elm::kernel::Mode::Development
                    && std::env::var("PLANEXPO_ELM_TRIVIA_CACHE").as_deref() == Ok("1") {
                    trivia_cache = identity.as_ref().and_then(|id| {
                        trace!("trivia_key", planexpo_elm::cache::OutputCache::new_for_trivia_with_digests(
                            &manifest.parent().unwrap().join("elm-stuff/planexpo-rust/v1"), source_digests.as_ref().unwrap(), id,
                        ))
                    });
                    trivia_cache.as_ref().and_then(|cache| trace!("trivia_cache_read", cache.load()))
                } else { None }
            });
            let javascript = if let Some(output) = reused_output {
                if options.cache { trace!("import_history", remember_import_names(&graph, &[])); }
                cached += 1;
                output
            } else {
                let diagnostic_cache = options.incremental.then(|| cache.as_ref().map(|cache| cache.diagnostic_slot())).flatten();
                if let Some(error) = diagnostic_cache.as_ref().and_then(|cache| cache.load())
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .filter(|error| planexpo_elm::docs_diagnostic::report_encoded(error)
                        .is_some_and(|report| report["type"] == "compile-errors")) {
                    return Err(error);
                }
                // Batch preflight follows the ordinary package, entry, output
                // and diagnostic-cache checks. Only a cache miss for development
                // JS is deferred; all other requests retain ordinary behavior.
                if let Some(prepare) = prepare
                    && options.incremental && options.cache
                    && options.mode == Mode::Development && graph.entries.len() == 1
                    && mains.len() == 1 && options.output.extension().is_some_and(|ext| ext == "js")
                    && let Some(identity) = &identity
                {
                    *prepare = Some(batch::Prepared { graph: graph.clone(), output: options.output.clone(), manifest: manifest.clone(), identity: identity.clone() });
                    return Ok(());
                }
                let type_cache = if options.incremental {
                    identity.as_ref().and_then(|id| {
                        trace!("type_cache_setup", planexpo_elm::typed_cache::TypeCache::new_with_digests(
                            &manifest
                                .parent()
                                .unwrap()
                                .join("elm-stuff/planexpo-rust/types-v1"),
                            source_digests.as_ref().unwrap(),
                            id,
                            options.mode,
                        ))
                        .ok()
                    })
                } else {
                    None
                };
                let outcome = trace!("analysis", if let Some(cache) = &type_cache {
                    planexpo_elm::analyze::generate_modules_cached(&graph, options.mode, cache)
                } else {
                    planexpo_elm::analyze::generate_modules_in_mode(&graph, options.mode)
                });
                let report = trace!("import_history", remember_analysis(&graph, outcome, options.cache)).inspect_err(|error| {
                    if let Some(cache) = &diagnostic_cache
                        && planexpo_elm::docs_diagnostic::report_encoded(error)
                            .is_some_and(|report| report["type"] == "compile-errors") {
                        let _ = cache.store(error.as_bytes());
                    }
                })?;
                compiled = count_compiled(&report);
                cached_types += report.cached_type_modules;
                let output = trace!("linking", planexpo_elm::linker::assemble(&graph, report))?.into_bytes();
                if let Some(cache) = cache {
                    // A full/read-only cache must never prevent compilation.
                    if let Err(error) = trace!("output_cache_store", cache.store_with_trivia(&output, trivia_cache.as_ref()))
                        && !options.json
                    {
                        eprintln!("Output cache unavailable: {error}");
                    }
                }
                output
            };
            let mut output_error = |path: &std::path::Path, error, operation| {
                // Analysis succeeded; a destination failure must not be presented
                // as a source error or claim that the bundle was written.
                if !options.json {
                    if compiled == 0 {
                        print!("\r             ");
                    }
                    print!("\r{}", success_message(compiled, &[], &options.output));
                    progress_line.0 = false;
                }
                crate::diagnostic::output_io_error(path, operation, error)
            };
            if let Some(parent) = options
                .output
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
            {
                create_output_directory(parent)
                    .map_err(|(path, e)| output_error(&path, e, "createDirectory"))?;
            }
            let output = if options
                .output
                .to_str()
                .is_some_and(|path| path.ends_with(".html"))
            {
                let module = mains[0]
                    .split_once(':')
                    .map(|(_, name)| name)
                    .ok_or("invalid HTML entry identity")?;
                planexpo_elm::html::render(
                    module,
                    &String::from_utf8(javascript).map_err(|e| e.to_string())?,
                )
                .into_bytes()
            } else {
                javascript
            };
            generated_names = graph
                .entries
                .iter()
                .filter(|entry| mains.contains(entry))
                .filter_map(|entry| entry.split_once(':').map(|(_, name)| name.to_string()))
                .collect();
            trace!("output_write", fs::write(&options.output, output))
                .map_err(|e| output_error(&options.output, e, "openBinaryFile"))?;
        }
    }
    if !options.json {
        if progress_line.0 {
            if compiled == 0 {
                print!("\r             ");
            }
            print!("\r");
            progress_line.0 = false;
        }
        print!(
            "{}",
            success_message(compiled, &generated_names, &options.output)
        );
        if env::var("PLANEXPO_ELM_STATS").as_deref() == Ok("1") {
            if options.incremental {
                println!("Reused types for {cached_types} module(s).");
            }
            println!("Output cache: ({cached} cached).");
        }
    }
    Ok(())
}

fn success_message(compiled: usize, names: &[String], output: &std::path::Path) -> String {
    let mut message = match compiled {
        0 => "Success!\n".to_string(),
        1 => "Success! Compiled 1 module.\n".to_string(),
        n => format!("Success! Compiled {n} modules.\n"),
    };
    if names.is_empty() {
        return message;
    }
    message.push('\n');
    let width = 3 + names
        .iter()
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(0);
    let (bar, top, middle, bottom) = if cfg!(windows) {
        ('-', '+', '+', '+')
    } else {
        ('─', '┬', '┤', '┘')
    };
    for (i, name) in names.iter().enumerate() {
        message.push_str(&format!(
            "    {name} {}",
            bar.to_string().repeat(width - name.chars().count())
        ));
        if names.len() == 1 {
            message.push_str(&format!("> {}\n", output.display()));
        } else if i == 0 {
            message.push_str(&format!("{top}{bar}{bar}> {}\n", output.display()));
        } else {
            message.push(if i + 1 == names.len() { bottom } else { middle });
            message.push('\n');
        }
    }
    message.push('\n');
    message
}

#[cfg(test)]
mod tests {
    use super::Options;
    #[test]
    fn debugger_mode_and_optimize_conflict() {
        assert_eq!(
            Options::parse(vec!["--debug".into()]).unwrap().mode,
            super::Mode::Debug
        );
        for flags in [["--debug", "--optimize"], ["--optimize", "--debug"]] {
            assert!(
                Options::parse(flags.into_iter().map(str::to_string).collect())
                    .unwrap()
                    .clashing
            );
        }
    }
    #[test]
    fn documentation_path_flags() {
        for args in [vec!["--docs=docs.json"], vec!["--docs", "nested/docs.json"]] {
            let options = Options::parse(args.into_iter().map(str::to_string).collect()).unwrap();
            assert!(options.docs.is_some());
        }
        for args in [
            vec!["--docs"],
            vec!["--docs="],
            vec!["--docs=docs.txt"],
            vec!["--docs=.json"],
            vec!["--docs=a.json", "--docs=b.json"],
        ] {
            assert!(Options::parse(args.into_iter().map(str::to_string).collect()).is_err());
        }
    }
    #[test]
    fn success_summary_counts_modules_and_formats_outputs() {
        let path = std::path::Path::new("elm.js");
        assert_eq!(super::success_message(0, &[], path), "Success!\n");
        assert_eq!(
            super::success_message(1, &[], path),
            "Success! Compiled 1 module.\n"
        );
        assert_eq!(
            super::success_message(3, &[], path),
            "Success! Compiled 3 modules.\n"
        );
        if !cfg!(windows) {
            assert_eq!(
                super::success_message(1, &["Main".into()], path),
                "Success! Compiled 1 module.\n\n    Main ───> elm.js\n\n"
            );
            assert_eq!(
                super::success_message(0, &["Long.Main".into(), "B".into(), "C".into()], path),
                "Success!\n\n    Long.Main ───┬──> elm.js\n    B ───────────┤\n    C ───────────┘\n\n"
            );
        }
    }
}
