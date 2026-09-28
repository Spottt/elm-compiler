//! Conventional invocation for build tools; inspection commands remain separate.
use planexpo_elm::kernel::Mode;
use std::{env, fs, path::PathBuf};

struct Options {
    entries: Vec<PathBuf>,
    docs: Option<PathBuf>,
    output: PathBuf,
    explicit_output: bool,
    json: bool,
    cache: bool,
    mode: Mode,
    incremental: bool,
}

impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut entries = Vec::new();
        let mut output = None;
        let mut docs = None;
        let mut json = false;
        let mut cache = true;
        let mut incremental = false;
        let mut mode = Mode::Development;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            if arg == "--output" || arg.starts_with("--output=") {
                let value = arg
                    .strip_prefix("--output=")
                    .map(str::to_owned)
                    .or_else(|| args.next())
                    .ok_or("--output needs a path")?;
                if value.is_empty() || value.starts_with("--") {
                    return Err("--output needs a path".into());
                }
                if output.replace(PathBuf::from(value)).is_some() {
                    return Err("--output was provided more than once".into());
                }
            } else if arg == "--docs" || arg.starts_with("--docs=") {
                let value = arg
                    .strip_prefix("--docs=")
                    .map(str::to_owned)
                    .or_else(|| args.next())
                    .ok_or("--docs needs a JSON path")?;
                if !value.ends_with(".json") || value.len() <= 5 || value.starts_with("--") {
                    return Err("--docs needs a JSON (.json) path".into());
                }
                if docs.replace(PathBuf::from(value)).is_some() {
                    return Err("--docs was provided more than once".into());
                }
            } else if arg == "--report" || arg.starts_with("--report=") {
                let value = arg
                    .strip_prefix("--report=")
                    .map(str::to_owned)
                    .or_else(|| args.next())
                    .ok_or("--report needs a format")?;
                if value != "json" {
                    return Err("only --report=json is supported".into());
                }
                json = true;
            } else if arg == "--no-cache" {
                cache = false;
            } else if arg == "--incremental" {
                incremental = true;
            } else if arg == "--optimize" {
                mode = Mode::Production;
            } else if arg.starts_with('-') {
                return Err(format!("unsupported make option: {arg}"));
            } else {
                entries.push(PathBuf::from(arg));
            }
        }
        let explicit_output = output.is_some();
        let output = output.unwrap_or_else(|| {
            PathBuf::from(if entries.len() == 1 {
                "index.html"
            } else {
                "elm.js"
            })
        });
        if output != std::path::Path::new("/dev/null") {
            if output.extension().is_some_and(|s| s == "html") && entries.len() > 1 {
                return Err("HTML output requires exactly one entry".into());
            }
            if output.extension().is_none_or(|s| s != "js" && s != "html") {
                return Err("output must be JavaScript (.js), HTML (.html) or /dev/null".into());
            }
        }
        Ok(Self {
            entries,
            docs,
            output,
            explicit_output,
            json,
            cache,
            mode,
            incremental,
        })
    }
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let mut options = Options::parse(args)?;
    let cwd = env::current_dir().map_err(|e| e.to_string())?;
    let manifest = cwd
        .ancestors()
        .map(|p| p.join("elm.json"))
        .find(|p| p.is_file())
        .ok_or("no elm.json found in this directory or its parents")?;
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
    let selected_packages = Some(planexpo_elm::package_resolution::resolve(&manifest, &home)?);
    let identity = if options.cache {
        env::current_exe()
            .ok()
            .and_then(|p| planexpo_elm::cache::compiler_identity(&p).ok())
    } else {
        None
    };
    let mut cached = 0;
    let mut cached_types = 0;
    {
        // Arguments are relative to the invoking directory, not to elm.json.
        let entries: Vec<_> = options
            .entries
            .iter()
            .map(|entry| cwd.join(entry))
            .collect();
        let mut graph = planexpo_elm::project::discover_many_selected(
            &manifest,
            &entries,
            &home,
            false,
            selected_packages.as_ref(),
        )?;
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
        if !options.explicit_output {
            options.output = match mains.len() {
                0 => PathBuf::from("/dev/null"),
                1 => PathBuf::from("index.html"),
                _ => PathBuf::from("elm.js"),
            };
        } else if options.output != std::path::Path::new("/dev/null")
            && mains.len() != graph.entries.len()
        {
            // Elm reports source errors before checking whether the requested
            // output can be initialized. Preserve the same diagnostic as a
            // check-only invocation, even when an entry has no main value.
            planexpo_elm::analyze::check_types(&graph)?;
            return Err("output requires a main value in every entry module".into());
        }

        if document_package {
            let report = planexpo_elm::analyze::documentation(&graph)?;
            let path = options
                .docs
                .as_ref()
                .ok_or("missing documentation output path")?;
            let bytes = serde_json::to_vec(&report.documentation).map_err(|e| e.to_string())?;
            fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        } else if options.output == std::path::Path::new("/dev/null") {
            planexpo_elm::analyze::check_types(&graph)?;
        } else {
            graph = planexpo_elm::project::discover_many_selected(
                &manifest,
                &entries,
                &home,
                true,
                selected_packages.as_ref(),
            )?;
            let cache = identity.as_ref().map(|id| {
                planexpo_elm::cache::OutputCache::new_in_mode(
                    &manifest
                        .parent()
                        .unwrap()
                        .join("elm-stuff/planexpo-rust/v1"),
                    &graph,
                    id,
                    options.mode,
                )
            });
            let javascript = if let Some(output) = cache.as_ref().and_then(|c| c.load()) {
                cached += 1;
                output
            } else {
                let type_cache = if options.incremental {
                    identity.as_ref().and_then(|id| {
                        planexpo_elm::typed_cache::TypeCache::new(
                            &manifest
                                .parent()
                                .unwrap()
                                .join("elm-stuff/planexpo-rust/types-v1"),
                            &graph,
                            id,
                            options.mode,
                        )
                        .ok()
                    })
                } else {
                    None
                };
                let report = if let Some(cache) = &type_cache {
                    planexpo_elm::analyze::generate_modules_cached(&graph, options.mode, cache)?
                } else {
                    planexpo_elm::analyze::generate_modules_in_mode(&graph, options.mode)?
                };
                cached_types += report.cached_type_modules;
                let output = planexpo_elm::linker::assemble(&graph, report)?.into_bytes();
                if let Some(cache) = cache {
                    // A full/read-only cache must never prevent compilation.
                    if let Err(error) = cache.store(&output)
                        && !options.json
                    {
                        eprintln!("Output cache unavailable: {error}");
                    }
                }
                output
            };
            if let Some(parent) = options
                .output
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
            {
                fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            let output = if options.output.extension().is_some_and(|s| s == "html") {
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
            fs::write(&options.output, output)
                .map_err(|e| format!("{}: {e}", options.output.display()))?;
        }
    }
    if !options.json {
        if options.incremental {
            eprintln!("Reused types for {cached_types} module(s).");
        }
        eprintln!(
            "Success! Compiled {} entry point(s) with the experimental Rust compiler ({cached} cached).",
            options.entries.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Options;
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
}
