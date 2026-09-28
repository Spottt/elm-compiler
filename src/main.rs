use planexpo_elm::{lexer::lex, module::header};
use std::{env, fs, process::ExitCode, time::Instant};
mod diagnostic;
mod bump;
mod diff;
mod init;
mod install;
mod make;
mod make_worker;
mod make_cli;
mod global_cli;
mod publish;
mod reactor;
mod reactor_cli;
mod repl;
mod repl_completion;
#[cfg(unix)]
mod repl_evaluation;
mod repl_history;
fn run() -> Result<(), String> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    if global_cli::help_or_version(&arguments) { return Ok(()); }
    let mut args = arguments.into_iter();
    let command = args
        .next()
        .ok_or("usage: planexpo-elm scan <source.elm>...")?;
    if command == "--compiler-help" {
        println!(
            "planexpo-elm {} — experimental Elm 0.19.1 compiler\npublish (publish a package)\nbump (update the package version from API changes)\ndiff [<package>] [<old-version>] [<new-version>] (compare public APIs)\ninit (create an Elm application)\nrepl [--no-colors] [--interpreter <path>] (interactive session)\ninstall <author/package> (add a dependency)\nreactor [--port <number>] (browse and compile local files)\nmake <entry.elm> [--output <output.js|index.html>] [--report=json] [--no-cache] [--optimize|--debug] [--incremental]\nmake <source.elm>... --output <output.js|/dev/null>\nmake [--docs <docs.json>] (package exposed modules)\nInspection: scan, parse, graph, operators, names, declarations, check, codegen-check, link-js\nProfiling: profile <elm.json> <entry.elm> [--incremental] [--development] (no output cache)\nExperimental production runtime: link-js-prod <elm.json> <entry.elm> <output.js>",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if command == "--internal-make-worker" {
        if args.next().is_some() { return Err("unexpected worker argument".into()); }
        return make_worker::run();
    }
    if command == "publish" { return publish::run(args.collect()); }
    if command == "bump" { return bump::run(args.collect()); }
    if command == "diff" {
        return diff::run(args.collect());
    }
    if command == "init" {
        return init::run(args.collect());
    }
    if command == "install" {
        return install::run(args.collect());
    }
    if command == "reactor" { return reactor::run(args.collect()); }
    if command == "make" {
        return make::run(args.collect());
    }
    #[cfg(unix)]
    if command == "--internal-repl-compile" {
        let path = args.next().ok_or("missing REPL request file")?;
        return repl_evaluation::compile_request(std::path::Path::new(&path));
    }
    if command == "repl" {
        return repl::run(args.collect());
    }
    if matches!(
        command.as_str(),
        "graph"
            | "operators"
            | "names"
            | "declarations"
            | "check"
            | "codegen-check"
            | "link-js"
            | "link-js-prod"
            | "profile"
    ) {
        let manifest = args
            .next()
            .ok_or("graph requires elm.json and an entry path relative to its directory")?;
        let entry = args.next().ok_or("graph requires an entry path")?;
        let output = if matches!(command.as_str(), "link-js" | "link-js-prod") {
            Some(args.next().ok_or("link-js requires an output path")?)
        } else {
            None
        };
        let mut incremental_profile = false;
        let mut development_profile = false;
        if command == "profile" {
            for option in args.by_ref() {
                match option.as_str() {
                    "--incremental" if !incremental_profile => incremental_profile = true,
                    "--development" if !development_profile => development_profile = true,
                    _ => return Err("profile supports --incremental and --development once each".into()),
                }
            }
        }
        if args.next().is_some() {
            return Err("unexpected graph argument".into());
        }
        let home = env::var_os("ELM_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|p| std::path::PathBuf::from(p).join(".elm")))
            .ok_or("ELM_HOME or HOME is required")?;
        let start = Instant::now();
        let graph = planexpo_elm::project::discover(
            std::path::Path::new(&manifest),
            std::path::Path::new(&entry),
            &home,
        )?;
        if matches!(
            command.as_str(),
            "operators"
                | "names"
                | "declarations"
                | "check"
                | "codegen-check"
                | "link-js"
                | "link-js-prod"
                | "profile"
        ) {
            if command == "profile" {
                let mode = if development_profile {
                    planexpo_elm::kernel::Mode::Development
                } else {
                    planexpo_elm::kernel::Mode::Production
                };
                let report = if incremental_profile {
                    let executable = env::current_exe().map_err(|e| e.to_string())?;
                    let identity = planexpo_elm::cache::compiler_identity(&executable)
                        .map_err(|e| e.to_string())?;
                    let directory = std::path::Path::new(&manifest)
                        .parent()
                        .unwrap()
                        .join("elm-stuff/planexpo-rust/types-v1");
                    let cache = planexpo_elm::typed_cache::TypeCache::new(
                        &directory, &graph, &identity, mode,
                    )?;
                    planexpo_elm::analyze::generate_modules_cached(&graph, mode, &cache)?
                } else {
                    planexpo_elm::analyze::generate_modules_in_mode(&graph, mode)?
                };
                let modules: Vec<_> = report
                    .module_timings
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "module": m.module, "parse_ms": m.parse_ms,
                            "inference_ms": m.inference_ms, "generation_ms": m.generation_ms,
                            "interface_ms":m.interface_ms,
                            "preparation_ms":m.preparation_ms,
                            "validation_ms":m.validation_ms,
                            "finalization_ms":m.finalization_ms,
                            "compaction_ms": m.compaction_ms, "total_ms": m.total_ms,
                        })
                    })
                    .collect();
                let field_layout_ms = report.field_layout_ms;
                let cached_generated_modules = report.cached_generated_modules;
                let cached_type_modules = report.cached_type_modules;
                let linking_started = Instant::now();
                let javascript = planexpo_elm::linker::assemble(&graph, report)?;
                println!(
                    "{}",
                    serde_json::json!({
                        "mode":format!("{mode:?}"), "field_layout_ms":field_layout_ms,
                        "cached_type_modules":cached_type_modules, "cached_generated_modules":cached_generated_modules,
                        "linking_ms":linking_started.elapsed().as_secs_f64()*1000.0,
                        "elapsed_ms":start.elapsed().as_secs_f64()*1000.0,
                        "javascript_bytes":javascript.len(), "modules":modules,
                    })
                );
                return Ok(());
            }
            if matches!(command.as_str(), "link-js" | "link-js-prod") {
                let mode = if command == "link-js-prod" {
                    planexpo_elm::kernel::Mode::Production
                } else {
                    planexpo_elm::kernel::Mode::Development
                };
                let report = planexpo_elm::analyze::generate_modules_in_mode(&graph, mode)?;
                let javascript = planexpo_elm::linker::assemble(&graph, report)?;
                let output = output.unwrap();
                fs::write(&output, &javascript).map_err(|e| format!("{output}: {e}"))?;
                println!(
                    "{}",
                    serde_json::json!({"stage":"experimental-linking","mode":format!("{mode:?}"),"output":output,"bytes":javascript.len()})
                );
                return Ok(());
            }
            if command == "codegen-check" {
                let r = planexpo_elm::analyze::generate_modules(&graph)?;
                println!(
                    "{}",
                    serde_json::json!({"stage":"experimental-module-generation","elm_modules":r.elm_modules,"generated_modules":r.generated_modules,"definitions":r.generated.len(),"effect_registrations":r.generated.iter().filter(|d|d.registration.is_some()).count(),"javascript_bytes":r.generated.iter().map(|d|d.javascript.len()).sum::<usize>(),"errors":r.generation_errors,"elapsed_ms":start.elapsed().as_secs_f64()*1000.0})
                );
                return if r.generation_errors.is_empty() {
                    Ok(())
                } else {
                    Err("module generation is incomplete; see errors above".into())
                };
            }
            let r = if command == "check" {
                planexpo_elm::analyze::check_types(&graph)?
            } else if command == "declarations" {
                planexpo_elm::analyze::declared_types(&graph)?
            } else if command == "names" {
                planexpo_elm::analyze::resolve_names(&graph)?
            } else {
                planexpo_elm::analyze::syntax_and_operators(&graph)?
            };
            println!(
                "{}",
                serde_json::json!({"type_nodes":r.type_nodes,"aliases":r.aliases,"constructors":r.constructors,"annotations":r.annotations,"ports":r.ports,"inferred_expressions":r.inferred_expressions,"stage":if command == "check" {"experimental-type-inference"} else if command == "declarations" {"declared-types"} else if command == "names" {"name-resolution"} else {"syntax-and-operators"}, "symbols":r.symbols,"references":r.references,"locals":r.locals, "elm_modules":r.elm_modules,"declarations":r.declarations,"expressions":r.expressions,"patterns":r.patterns,"types":r.types,"elapsed_ms":start.elapsed().as_secs_f64()*1000.0})
            );
            return Ok(());
        }
        let nodes: Vec<_> = graph.modules.iter().map(|m| serde_json::json!({"owner":m.owner,"name":m.name,"path":m.path,"imports":m.imports,"bytes":m.bytes,"tokens":m.tokens,"kernel":m.kernel})).collect();
        println!(
            "{}",
            serde_json::json!({"stage":"dependency-discovery", "elapsed_ms":start.elapsed().as_secs_f64()*1000.0, "modules":nodes})
        );
        return Ok(());
    }
    if command != "scan" && command != "parse" {
        return Err(global_cli::unknown(&command));
    }
    let start = Instant::now();
    let mut files = 0;
    let mut bytes = 0;
    let mut tokens = 0;
    let mut imports = 0;
    let mut nodes = 0;
    for path in args {
        let source = fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
        let ts = lex(&source).map_err(|e| format!("{path}:{e}"))?;
        let h = header(&source, &ts).map_err(|e| format!("{path}: {e}"))?;
        if command == "parse" {
            let ast = planexpo_elm::parser::parse(&source).map_err(|e| format!("{path}:{e}"))?;
            nodes += ast.expressions.len() + ast.patterns.len() + ast.types.len();
        }
        files += 1;
        bytes += source.len();
        tokens += ts.len();
        imports += h.imports.len();
    }
    if files == 0 {
        return Err("scan requires at least one Elm source".into());
    }
    println!(
        "{}",
        serde_json::json!({"stage":if command == "parse" {"source-parse"} else {"lex-and-headers"}, "ast_nodes":nodes, "files":files,"bytes":bytes,"tokens":tokens,"explicit_imports":imports,"elapsed_ms":start.elapsed().as_secs_f64()*1000.0})
    );
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Terminal argument errors are already complete CLI documents.
            // In particular an unsupported --report flag must not enable JSON.
            if let Some(message) = e.strip_prefix("ELM_CLI_RAW:") {
                use std::io::IsTerminal;
                let rendered = diagnostic::proxy_cli_error(
                    message,
                    &env::args().nth(1).unwrap_or_default(),
                    std::io::stderr().is_terminal(),
                );
                eprint!("{}", rendered.as_deref().unwrap_or(message));
                return ExitCode::FAILURE;
            }
            let args: Vec<_> = env::args().collect();
            let json_report = if args.get(1).is_some_and(|command| command == "make") {
                make::json_report(&args[2..])
            } else {
                args.get(1).is_none_or(|command| command != "repl")
                    && (args.iter().any(|s| s == "--report=json")
                        || args.windows(2).any(|s| s[0] == "--report" && s[1] == "json"))
            };
            if json_report {
                eprintln!("{}", diagnostic::report(&e));
            } else {
                eprintln!("{}", diagnostic::terminal(&e));
            }
            ExitCode::FAILURE
        }
    }
}
