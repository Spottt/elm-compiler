use planexpo_elm::{lexer::lex, module::header};
use std::{env, fs, process::ExitCode, time::Instant};
mod diagnostic;
mod init;
mod install;
mod make;
mod repl;
mod repl_completion;
mod repl_history;
#[cfg(unix)]
mod repl_evaluation;
fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args
        .next()
        .ok_or("usage: planexpo-elm scan <source.elm>...")?;
    if command == "--version" {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if matches!(command.as_str(), "--help" | "help") {
        println!(
            "planexpo-elm {} — experimental Elm 0.19.1 compiler\ninit (create an Elm application)\nrepl [--no-colors] [--interpreter <path>] (interactive session)\ninstall <author/package> (add a dependency)\nmake <entry.elm> [--output <output.js|index.html>] [--report=json] [--no-cache] [--optimize] [--incremental]\nmake <source.elm>... --output <output.js|/dev/null>\nmake [--docs <docs.json>] (package exposed modules)\nInspection: scan, parse, graph, operators, names, declarations, check, codegen-check, link-js\nProfiling: profile <elm.json> <entry.elm> (production, no output cache)\nExperimental production runtime: link-js-prod <elm.json> <entry.elm> <output.js>",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if command == "init" {
        return init::run(args.collect());
    }
    if command == "install" {
        return install::run(args.collect());
    }
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
        let incremental_profile = if command == "profile" {
            match args.next().as_deref() {
                None => false,
                Some("--incremental") => true,
                Some(_) => return Err("profile only supports --incremental".into()),
            }
        } else {
            false
        };
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
                let mode = planexpo_elm::kernel::Mode::Production;
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
                            "compaction_ms": m.compaction_ms, "total_ms": m.total_ms,
                        })
                    })
                    .collect();
                let field_layout_ms = report.field_layout_ms;
                let cached_type_modules = report.cached_type_modules;
                let linking_started = Instant::now();
                let javascript = planexpo_elm::linker::assemble(&graph, report)?;
                println!(
                    "{}",
                    serde_json::json!({
                        "mode":"Production", "field_layout_ms":field_layout_ms,
                        "cached_type_modules":cached_type_modules,
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
        return Err("unknown command; use --help for available commands".into());
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
            let args: Vec<_> = env::args().collect();
            if args.get(1).is_none_or(|command| command != "repl")
                && (args.iter().any(|s| s == "--report=json")
                    || args
                        .windows(2)
                        .any(|s| s[0] == "--report" && s[1] == "json"))
            {
                eprintln!("{}", diagnostic::report(&e));
            } else {
                eprintln!("{}", diagnostic::terminal(&e));
            }
            ExitCode::FAILURE
        }
    }
}
