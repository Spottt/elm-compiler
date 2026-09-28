//! Project syntax, name resolution, declared types and experimental expression
//! inference, pattern coverage, main validation and code generation.
use crate::{
    ast::Syntax,
    fixity::{self, Table},
    module::{Exposed, Exposing},
    parser::parse,
    project::Graph,
};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
mod prefix;
mod verified;
#[derive(Debug, Default, Clone)]
pub struct ModuleTiming {
    pub module: String,
    pub parse_ms: f64,
    pub preparation_ms: f64,
    pub inference_ms: f64,
    pub type_restore_ms: f64,
    pub type_load_ms: f64,
    pub type_decode_ms: f64,
    pub type_import_ms: f64,
    pub type_check_ms: f64,
    pub type_artifact_ms: f64,
    pub validation_ms: f64,
    pub finalization_ms: f64,
    pub interface_ms: f64,
    pub generation_ms: f64,
    pub compaction_ms: f64,
    pub total_ms: f64,
}
#[derive(Debug, Default, Clone)]
pub struct EntryOutput {
    pub roots: BTreeSet<crate::names::SymbolId>,
    pub javascript: String,
}
#[derive(Debug, Default, Clone)]
pub struct Report {
    pub resumed_modules: usize,
    pub cached_generated_modules: usize,
    pub debug_modules: BTreeSet<String>,
    /// Detached inferred schemes, aliases and source variable names for the
    /// REPL entry only.
    pub repl_types: Option<serde_json::Value>,
    pub repl_type_names: Option<crate::type_localizer::Localizer>,
    pub documentation: Vec<serde_json::Value>,
    pub module_timings: Vec<ModuleTiming>,
    pub field_layout_ms: f64,
    pub cached_type_modules: usize,
    /// Canonical modules checked afresh rather than restored from typed artifacts.
    pub compiled_modules: Vec<String>,
    pub mode: crate::kernel::Mode,
    pub fields: crate::fields::Fields,
    pub elm_modules: usize,
    pub declarations: usize,
    pub expressions: usize,
    pub patterns: usize,
    pub types: usize,
    pub symbols: usize,
    pub references: usize,
    pub locals: usize,
    pub type_nodes: usize,
    pub aliases: usize,
    pub constructors: usize,
    pub annotations: usize,
    pub ports: usize,
    pub inferred_expressions: usize,
    pub generated_modules: usize,
    pub generated: Vec<std::rc::Rc<crate::module_codegen::Definition>>,
    pub generation_errors: Vec<String>,
    pub main_export: Option<String>,
    pub entry_outputs: BTreeMap<String, EntryOutput>,
    pub link_roots: BTreeSet<crate::names::SymbolId>,
    pub link_names: BTreeMap<(String, String), crate::names::SymbolId>,
}
fn add(
    scope: &mut Table,
    origins: &mut BTreeMap<String, BTreeSet<String>>,
    foreign: &Table,
    from: &str,
    exposing: &Exposing,
) -> Result<(), String> {
    let names: Vec<&str> = match exposing {
        Exposing::All => foreign.keys().map(String::as_str).collect(),
        Exposing::Explicit(items) => items
            .iter()
            .filter_map(|x| match x {
                Exposed::Operator(name) => Some(name.as_str()),
                _ => None,
            })
            .collect(),
    };
    for name in names {
        let fixity = foreign
            .get(name)
            .ok_or_else(|| format!("{from} does not expose operator {name}"))?;
        if matches!(exposing, Exposing::Explicit(_)) {
            // Elm's explicit operator exposure overwrites earlier imports;
            // exposing (..) instead accumulates candidates until use.
            origins.insert(name.into(), BTreeSet::from([from.into()]));
        } else {
            origins.entry(name.into()).or_default().insert(from.into());
        }
        scope.insert(name.into(), *fixity);
    }
    Ok(())
}
fn exports(ast: &Syntax<'_>, local: Table) -> Result<Table, String> {
    match &ast.header.exposing {
        Exposing::All => Ok(local),
        Exposing::Explicit(items) => {
            let mut out = Table::new();
            for item in items {
                if let Exposed::Operator(name) = item {
                    out.insert(
                        name.clone(),
                        *local
                            .get(name)
                            .ok_or_else(|| format!("exported operator {name} is not declared"))?,
                    );
                }
            }
            Ok(out)
        }
    }
}
pub fn syntax_and_operators(graph: &Graph) -> Result<Report, String> {
    project_pass(graph, Stage::Operators)
}
pub fn resolve_names(graph: &Graph) -> Result<Report, String> {
    project_pass(graph, Stage::Names)
}
pub fn declared_types(graph: &Graph) -> Result<Report, String> {
    project_pass(graph, Stage::DeclaredTypes)
}
pub fn check_types(graph: &Graph) -> Result<Report, String> {
    project_pass(graph, Stage::Check)
}
/// Check all modules and generate documentation for the graph's entry modules.
pub fn documentation(graph: &Graph) -> Result<Report, String> {
    let mut report = project_pass(graph, Stage::Documentation)?;
    report
        .documentation
        .sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(report)
}
/// Generate a REPL module while retaining its private inferred bindings for
/// subsequent type display. Ordinary builds do not allocate this snapshot.
pub fn generate_for_repl(graph: &Graph) -> Result<Report, String> {
    if graph.entries.len() != 1 || graph.entries[0] != graph.entry {
        return Err("REPL compilation requires exactly one entry module".into());
    }
    project_pass(graph, Stage::Repl)
}
pub fn generate_modules(graph: &Graph) -> Result<Report, String> {
    project_pass(graph, Stage::Generate)
}
pub fn generate_modules_in_mode(
    graph: &Graph,
    mode: crate::kernel::Mode,
) -> Result<Report, String> {
    project_pass_in_mode(graph, Stage::Generate, mode, None)
}
pub fn generate_modules_cached(
    graph: &Graph,
    mode: crate::kernel::Mode,
    cache: &crate::typed_cache::TypeCache,
) -> Result<Report, String> {
    project_pass_in_mode(graph, Stage::Generate, mode, Some(cache))
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Operators,
    Names,
    DeclaredTypes,
    Check,
    Documentation,
    Generate,
    Repl,
}
fn project_pass(graph: &Graph, stage: Stage) -> Result<Report, String> {
    project_pass_in_mode(graph, stage, crate::kernel::Mode::Development, None)
}
fn project_pass_in_mode(
    graph: &Graph,
    stage: Stage,
    mode: crate::kernel::Mode,
    type_cache: Option<&crate::typed_cache::TypeCache>,
) -> Result<Report, String> {
    let proofs = if stage == Stage::Generate && mode == crate::kernel::Mode::Development {
        type_cache.and_then(|cache| verified::prepare(graph, cache))
    } else { None };
    let analyze = |graph: &Graph, output: bool| {
        if let Some(error) = proofs.as_ref().and_then(|proofs| proofs.cached_failure(graph)) { return Err(error); }
        let result = project_pass_once(graph, stage, mode, type_cache, output);
        if let Err(error) = &result && let Some(proofs) = &proofs { proofs.remember_failure(graph, error); }
        result
    };
    // Developer-only phase trace; ordinary CLI/worker diagnostics stay unchanged.
    let profile_errors = std::env::var_os("PLANEXPO_ELM_PROFILE_ERRORS").is_some();
    let initial_started = Instant::now();
    let initial = analyze(graph, true);
    if profile_errors && initial.is_err() {
        eprintln!("ELM_ERROR_PROFILE {}", serde_json::json!({"entries":graph.entries,"phase":"initial_analysis","ms":initial_started.elapsed().as_secs_f64()*1000.0,"modules":graph.modules.len()}));
    }
    let first = match initial {
        Ok(report) => {
            return if report.debug_modules.is_empty() {
                if report.generation_errors.is_empty() && let Some(proofs) = &proofs {
                    proofs.remember();
                }
                Ok(report)
            } else {
                Err(crate::make_output::debug_remnants(&report.debug_modules))
            };
        }
        Err(error) => error,
    };
    // Failed inference can leave partially unified nodes. Restart only the error
    // path on an independent graph; successful builds never clone or replay it.
    // Retained modules have unchanged dependencies after blocked importers are
    // removed, so their verified type artifacts remain valid on the fresh engine.
    let syntax_started = Instant::now();
    let mut remaining = graph.clone();
    let mut messages = Vec::new();
    let mut syntax_paths = Vec::new();
    // Elm parses modules before checking dependency outcomes. A malformed
    // importer still has its own syntax error even when its dependency failed.
    for module in &remaining.modules {
        if module.kernel {
            continue;
        }
        let result = if stage == Stage::Documentation
            && remaining
                .entries
                .contains(&format!("{}:{}", module.owner, module.name))
        {
            crate::parser::parse_with_docs(&module.source).map(|_| ())
        } else {
            crate::parser::check_syntax(&module.source)
        };
        if let Err(error) = result {
            syntax_paths.push(module.path.to_string_lossy().into_owned());
            messages.push(crate::source_error::in_module(
                &module.name,
                format!("{}:{error}", module.path.display()),
            ));
        }
    }
    if profile_errors {
        eprintln!("ELM_ERROR_PROFILE {}", serde_json::json!({"entries":graph.entries,"phase":"syntax_scan_with_graph_clone","ms":syntax_started.elapsed().as_secs_f64()*1000.0,"modules":graph.modules.len()}));
    }
    let replay = |remaining: &Graph| {
        let started = Instant::now();
        let result = analyze(remaining, false);
        if profile_errors {
            eprintln!("ELM_ERROR_PROFILE {}", serde_json::json!({"entries":graph.entries,"phase":"error_collection_replay","ms":started.elapsed().as_secs_f64()*1000.0,"modules":remaining.modules.len()}));
        }
        result
    };
    let mut error = if syntax_paths.is_empty() {
        first
    } else {
        remove_failed_modules(&mut remaining, &syntax_paths);
        if proofs.as_ref().is_some_and(|proofs| proofs.can_skip(&remaining)) {
            return Err(crate::source_error::batch(messages));
        }
        match replay(&remaining) {
            Ok(_) => return Err(crate::source_error::batch(messages)),
            Err(error) => error,
        }
    };
    loop {
        let paths: Vec<String> =
            if let Some(report) = crate::docs_diagnostic::report_encoded(&error) {
                report["errors"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| entry["path"].as_str().map(str::to_owned))
                    .collect()
            } else {
                remaining
                    .modules
                    .iter()
                    .filter_map(|module| {
                        let path = module.path.to_string_lossy();
                        let detail = error.strip_prefix(&format!("{path}:"))?.trim_start();
                        let mut parts = detail.split(':');
                        parts.next()?.parse::<usize>().ok()?;
                        parts.next()?.parse::<usize>().ok()?;
                        Some(path.into_owned())
                    })
                    .collect()
            };
        if paths.is_empty() {
            // Unlocated project failures cannot safely be attributed to a module.
            return Err(error);
        }
        if !remove_failed_modules(&mut remaining, &paths) {
            return Err(error);
        }
        messages.push(error);
        if proofs.as_ref().is_some_and(|proofs| proofs.can_skip(&remaining)) {
            return Err(crate::source_error::batch(messages));
        }
        match replay(&remaining) {
            Ok(_) => return Err(crate::source_error::batch(messages)),
            Err(next) => error = next,
        }
    }
}

fn remove_failed_modules(graph: &mut Graph, paths: &[String]) -> bool {
    let mut blocked = BTreeSet::new();
    graph.modules.retain(|module| {
        let failed = paths
            .iter()
            .any(|path| module.path == std::path::Path::new(path))
            || module.imports.iter().any(|id| blocked.contains(id));
        if failed {
            blocked.insert(format!("{}:{}", module.owner, module.name));
        }
        !failed
    });
    graph.entries.retain(|id| !blocked.contains(id));
    graph.import_errors.retain(|error| {
        error["path"].as_str().is_none_or(|path| {
            graph.modules.iter().any(|module| module.path == std::path::Path::new(path))
        })
    });
    !blocked.is_empty()
}

fn project_pass_once(
    graph: &Graph,
    stage: Stage,
    mode: crate::kernel::Mode,
    type_cache: Option<&crate::typed_cache::TypeCache>,
    generate_output: bool,
) -> Result<Report, String> {
    if !graph.import_errors.is_empty() {
        return Err(crate::docs_diagnostic::encode(&serde_json::json!({
            "type":"compile-errors", "errors":graph.import_errors
        })));
    }
    let result = project_pass_with_display(graph, stage, mode, type_cache, false, generate_output);
    if stage != Stage::Repl
        && mode != crate::kernel::Mode::Debug
        && (result.as_ref().err().is_some_and(|error| {
            error.contains("\"kind\":\"annotation_mismatch\"")
                || error.contains("Cannot unify these types:")
        }) || result
            .as_ref()
            .err()
            .and_then(|error| crate::docs_diagnostic::report_encoded(error))
            .is_some_and(|report| {
                report["errors"].as_array().is_some_and(|errors| {
                    errors.iter().any(|error| {
                        error["problems"].as_array().is_some_and(|problems| {
                            problems.iter().any(|problem| {
                                problem["title"] == "BAD MAIN TYPE" || problem["title"] == "BAD FLAGS"
                            })
                        })
                    })
                })
            }))
    {
        // Successful builds keep compact types. Error replay has a separate,
        // source-transitive cache retaining aliases and source variable names;
        // subsequent edits must not repeat inference of unchanged dependencies.
        let diagnostics = type_cache.map(crate::typed_cache::TypeCache::for_diagnostics);
        project_pass_with_display(graph, stage, mode, diagnostics.as_ref(), true, false)?;
    }
    result
}

fn project_pass_with_display(
    graph: &Graph,
    stage: Stage,
    mode: crate::kernel::Mode,
    type_cache: Option<&crate::typed_cache::TypeCache>,
    preserve_display: bool,
    generate_output: bool,
) -> Result<Report, String> {
    let profile_analysis = std::env::var("PLANEXPO_ELM_PROFILE_ANALYSIS").as_deref() == Ok("1");
    let pass_started = Instant::now();
    let mut symbols = crate::names::Symbols::default();
    let mut types = crate::types::Catalog::default();
    let mut engine = matches!(
        stage,
        Stage::DeclaredTypes | Stage::Check | Stage::Documentation | Stage::Generate | Stage::Repl
    )
    .then(|| crate::unify::Engine::new(crate::types::builtins(&mut symbols)));
    if stage == Stage::Repl || preserve_display {
        engine
            .as_mut()
            .expect("REPL type engine")
            .track_display_names();
    }
    if mode == crate::kernel::Mode::Debug {
        engine
            .as_mut()
            .ok_or("debug mode requires type inference")?
            .track_debug_types();
    }
    let mut global_types = BTreeMap::new();
    let mut type_interfaces = BTreeMap::new();
    let mut name_interfaces = BTreeMap::<String, std::rc::Rc<crate::names::Interface>>::new();
    let mut implicit_environment: Option<crate::names::Environment> = None;
    let mut interfaces = BTreeMap::<String, Table>::new();
    let fields_started = Instant::now();
    let fields = if mode == crate::kernel::Mode::Production {
        crate::fields::Fields::production(graph)?
    } else {
        crate::fields::Fields::default()
    };
    let mut report = Report {
        field_layout_ms: fields_started.elapsed().as_secs_f64() * 1000.0,
        mode,
        fields: fields.clone(),
        ..Report::default()
    };
    let mut layouts = crate::js_names::Layouts::default();
    layouts.fields = fields;
    let mut retained_types = 0;
    let checkpoint_started = Instant::now();
    let mut continuation = if stage == Stage::Generate
        && mode == crate::kernel::Mode::Development && !preserve_display && type_cache.is_some() {
        prefix::prepare(graph, generate_output)
    } else { None };
    if profile_analysis {
        eprintln!("ELM_ANALYSIS_PROFILE {}", serde_json::json!({"phase":"checkpoint", "ms":checkpoint_started.elapsed().as_secs_f64()*1000.0, "primary":generate_output, "entries":graph.entries}));
    }
    let start = continuation.as_ref().map_or(0, |plan| plan.start);
    if let Some(state) = continuation.as_mut().and_then(|plan| plan.restored.take()) {
        symbols = state.symbols; types = state.types; engine = state.engine;
        global_types = state.global_types; type_interfaces = state.type_interfaces;
        name_interfaces = state.name_interfaces; implicit_environment = state.implicit_environment;
        interfaces = state.interfaces; report = state.report; layouts = state.layouts;
        retained_types = state.retained_types;
    }
    let mut documentation_errors = Vec::new();
    let mut blocked_documentation = BTreeSet::new();
    for (index, module) in graph.modules.iter().enumerate().skip(start) {
        if let Some(plan) = &continuation && plan.capture_at == Some(index) {
            plan.capture(index, prefix::State {
                symbols: symbols.clone(), types: types.clone(), engine: engine.clone(),
                global_types: global_types.clone(), type_interfaces: type_interfaces.clone(),
                name_interfaces: name_interfaces.clone(), implicit_environment: implicit_environment.clone(),
                interfaces: interfaces.clone(), report: report.clone(), layouts: layouts.clone(), retained_types,
            });
        }
        if continuation.as_ref().is_some_and(|plan| plan.skips(index)) { continue; }
        if module.kernel {
            continue;
        }
        if stage == Stage::Documentation
            && module
                .imports
                .iter()
                .any(|name| blocked_documentation.contains(name))
        {
            blocked_documentation.insert(format!("{}:{}", module.owner, module.name));
            continue;
        }
        if let Some(expected) = &module.missing_header {
            return Err(crate::docs_diagnostic::encode(
                &crate::header_diagnostic::missing_name(expected, &module.path),
            ));
        }
        let started = Instant::now();
        let mut timing = ModuleTiming {
            module: format!("{}:{}", module.owner, module.name),
            ..ModuleTiming::default()
        };
        let document = stage == Stage::Documentation
            && graph
                .entries
                .contains(&format!("{}:{}", module.owner, module.name));
        let (mut ast, docs) = if document {
            let (ast, docs) = crate::parser::parse_with_docs(&module.source)
                .map_err(|e| format!("{}:{e}", module.path.display()))?;
            (ast, Some(docs))
        } else {
            (
                parse(&module.source).map_err(|e| format!("{}:{e}", module.path.display()))?,
                None,
            )
        };
        timing.parse_ms = started.elapsed().as_secs_f64() * 1000.0;
        let preparation_started = Instant::now();
        if matches!(
            stage,
            Stage::Check | Stage::Documentation | Stage::Generate | Stage::Repl
        ) {
            crate::entry::check_module(&ast, &module.owner).map_err(|error| {
                crate::effect_diagnostic::report(&ast, &module.path, &error)
                    .map(|report| crate::docs_diagnostic::encode(&report))
                    .unwrap_or_else(|| format!("{}: {error}", module.path.display()))
            })?;
        }
        let id = format!("{}:{}", module.owner, module.name);
        let local = fixity::declared(&ast)?;
        let mut scope = Table::new();
        let mut origins = BTreeMap::new();
        if module.owner != "elm/core" {
            for (from, exposing) in [
                ("elm/core:Basics", Exposing::All),
                (
                    "elm/core:List",
                    Exposing::Explicit(vec![Exposed::Operator("::".into())]),
                ),
            ] {
                let foreign = interfaces
                    .get(from)
                    .ok_or_else(|| format!("missing implicit interface {from}"))?;
                add(&mut scope, &mut origins, foreign, from, &exposing)?;
            }
        }
        for import in &ast.header.imports {
            if import.name.starts_with("Elm.Kernel.") {
                continue;
            }
            let suffix = format!(":{}", import.name);
            let from = module
                .imports
                .iter()
                .find(|id| id.ends_with(&suffix))
                .ok_or_else(|| format!("missing import {} of {id}", import.name))?;
            let foreign = interfaces
                .get(from)
                .ok_or_else(|| format!("missing operator interface {from}"))?;
            add(&mut scope, &mut origins, foreign, from, &import.exposing)?;
        }
        // Infix declarations export function aliases; they do not introduce
        // operator bindings inside their defining module (e.g. elm/parser).
        for node in &ast.expressions {
            let used: Vec<&str> = match &node.kind {
                crate::ast::Expr::Operator(op) => vec![op],
                crate::ast::Expr::Binops(_, rest) => rest.iter().map(|(op, _)| *op).collect(),
                _ => Vec::new(),
            };
            for op in used {
                if origins.get(op).is_some_and(|sources| sources.len() > 1) {
                    let homes: Vec<String> = origins[op]
                        .iter()
                        .map(|id| {
                            id.split_once(':')
                                .map_or(id.as_str(), |(_, name)| name)
                                .to_owned()
                        })
                        .collect();
                    let choices: Vec<String> =
                        homes.iter().map(|home| format!("{home}.{op}")).collect();
                    let detail = crate::name_diagnostic::ambiguous(op, "operator", homes, &choices);
                    let located = if matches!(node.kind, crate::ast::Expr::Operator(_)) {
                        crate::source_error::locate(ast.source, node.span, detail)
                    } else {
                        crate::source_error::locate_slice(ast.source, op, detail)
                    };
                    return Err(format!("{}: {located}", module.path.display()));
                }
            }
        }
        fixity::resolve(&mut ast, &scope).map_err(|e| format!("{}: {e}", module.path.display()))?;
        if stage != Stage::Operators {
            let mut env = if module.owner == "elm/core" {
                let mut env = crate::names::Environment::default();
                env.builtin_list(&mut symbols);
                env
            } else {
                if implicit_environment.is_none() {
                    let mut env = crate::names::Environment::default();
                    env.builtin_list(&mut symbols);
                    for (name, prefix, exposure) in default_imports() {
                        let from = format!("elm/core:{name}");
                        let interface = name_interfaces
                            .get(&from)
                            .ok_or_else(|| format!("missing name interface {from}"))?;
                        env.import_shared(prefix, interface.clone(), &exposure, &symbols)?;
                    }
                    implicit_environment = Some(env);
                }
                // Symbols and core interfaces stay stable within this pass.
                // Explicit imports and local declarations modify only the clone.
                implicit_environment.as_ref().unwrap().clone()
            };
            for import in &ast.header.imports {
                let suffix = format!(":{}", import.name);
                let from = module
                    .imports
                    .iter()
                    .find(|id| id.ends_with(&suffix))
                    .ok_or_else(|| format!("missing import {}", import.name))?;
                if import.name.starts_with("Elm.Kernel.") {
                    if import.alias.is_some() {
                        return Err("kernel imports cannot be aliased".into());
                    }
                    env.kernel(&import.name, from);
                } else {
                    let interface = name_interfaces
                        .get(from)
                        .ok_or_else(|| format!("missing name interface {from}"))?;
                    env.import_shared(
                        import.alias.as_deref().unwrap_or(&import.name),
                        interface.clone(),
                        &import.exposing,
                        &symbols,
                    )?;
                }
            }
            let (interface, resolved) = crate::names::resolve(&ast, &id, env, &mut symbols)
                .map_err(|error| {
                    let messages =
                        crate::source_error::batch_messages(&error).unwrap_or_else(|| vec![error]);
                    crate::source_error::batch(
                        messages
                            .into_iter()
                            .map(|error| format!("{}: {error}", module.path.display()))
                            .collect(),
                    )
                })?;
            let has_debug_uses = mode == crate::kernel::Mode::Production
                && id != "elm/core:Debug"
                && resolved.expressions.iter().flatten().any(|binding| {
                    matches!(binding, crate::names::Binding::Global(symbol)
                        if symbols.get(*symbol).module.as_ref() == "elm/core:Debug")
                });
            if has_debug_uses {
                report.debug_modules.insert(module.name.clone());
            }
            report.references += resolved.expressions.iter().flatten().count()
                + resolved.types.iter().flatten().count()
                + resolved.constructors.iter().flatten().count();
            report.locals += resolved.locals.len();
            if let Some(engine) = &mut engine {
                let declarations = if stage == Stage::DeclaredTypes {
                    types
                        .register(&ast, &resolved, &symbols, &id, engine)
                        .map(|_| ())
                } else {
                    types.register_type_definitions(&ast, &resolved, &symbols, &id, engine)
                };
                declarations.map_err(|e| format!("{}: {e}", module.path.display()))?;
                report.annotations += ast
                    .declarations
                    .iter()
                    .filter(|d| matches!(d, crate::ast::Declaration::Annotation { .. }))
                    .count();
                report.ports += ast
                    .declarations
                    .iter()
                    .filter(|d| matches!(d, crate::ast::Declaration::Port { .. }))
                    .count();
                if matches!(
                    stage,
                    Stage::Check | Stage::Documentation | Stage::Generate | Stage::Repl
                ) {
                    timing.preparation_ms = preparation_started.elapsed().as_secs_f64() * 1000.0;
                    let inference_started = Instant::now();
                    let required = if type_cache.is_some() {
                        crate::typed_artifact::required_globals(&id, &ast, &interface, &symbols)?
                    } else {
                        BTreeSet::new()
                    };
                    let module_cache =
                        type_cache.and_then(|cache| cache.for_interfaces(&id, &type_interfaces));
                    let mut text_cache = None;
                    let mut restored_from_text = false;
                    let restored = module_cache
                        .as_ref()
                        .and_then(|cache| {
                            let start = Instant::now();
                            let result = cache.load().or_else(|| {
                                // Unchanged modules take the ordinary exact-key
                                // path without hashing or formatting their AST.
                                text_cache = (stage == Stage::Generate && !preserve_display)
                                    .then(|| type_cache.and_then(|cache| cache.for_text_edits(&id, &ast, &type_interfaces))).flatten();
                                let bytes = text_cache.as_ref()?.load()?;
                                restored_from_text = true;
                                Some(bytes)
                            });
                            timing.type_load_ms = start.elapsed().as_secs_f64() * 1000.0;
                            result
                        })
                        .and_then(|bytes| {
                            if crate::session_cache::use_prepared_types() {
                                let start = Instant::now();
                                let prepared = crate::session_cache::prepared_types(&bytes);
                                timing.type_decode_ms = start.elapsed().as_secs_f64() * 1000.0;
                                let prepared = prepared?;
                                let start = Instant::now();
                                let restored = prepared.import(&id, &symbols, engine, &required);
                                timing.type_import_ms = start.elapsed().as_secs_f64() * 1000.0;
                                let restored = restored.ok()?;
                                let fingerprint = prepared.metadata().get("interface_fingerprint")
                                    .and_then(|value| serde_json::from_value::<[u8; 32]>(value.clone()).ok());
                                let coverage_checked = prepared.metadata().get("coverage_checked").and_then(serde_json::Value::as_bool) == Some(true);
                                Some((restored, fingerprint, coverage_checked))
                            } else {
                                let start = Instant::now();
                                let artifact = crate::session_cache::json(&bytes);
                                timing.type_decode_ms = start.elapsed().as_secs_f64() * 1000.0;
                                let artifact = artifact?;
                                let start = Instant::now();
                                let restored = crate::typed_artifact::import_selected(
                                    &artifact, &id, &symbols, engine, &required,
                                );
                                timing.type_import_ms = start.elapsed().as_secs_f64() * 1000.0;
                                let restored = restored.ok()?;
                                let fingerprint = artifact.get("interface_fingerprint")
                                    .and_then(|value| serde_json::from_value::<[u8; 32]>(value.clone()).ok());
                                let coverage_checked = artifact.get("coverage_checked").and_then(serde_json::Value::as_bool) == Some(true);
                                Some((restored, fingerprint, coverage_checked))
                            }
                        });
                    timing.type_restore_ms = inference_started.elapsed().as_secs_f64() * 1000.0;
                    let was_restored = restored.is_some();
                    let mut interface_fingerprint = None;
                    let mut coverage_checked = false;
                    if let Some((restored, fingerprint, checked)) = restored {
                        // Alternate inference reuse never provides an exact-source
                        // coverage proof; validate the current patterns normally.
                        coverage_checked = checked && !restored_from_text;
                        global_types.extend(restored);
                        report.cached_type_modules += 1;
                        interface_fingerprint = fingerprint;
                    } else {
                        let type_check_started = Instant::now();
                        let inferred = crate::infer::check(
                            crate::types::SourceTypes {
                                ast: &ast,
                                resolved: &resolved,
                                symbols: &symbols,
                            },
                            &id,
                            &types,
                            engine,
                            &mut global_types,
                        )
                        .map_err(|e| {
                            if let Some(mut report) = crate::docs_diagnostic::report_encoded(&e) {
                                if let Some(errors) = report["errors"].as_array_mut() {
                                    for error in errors {
                                        if error["path"] == "" {
                                            error["path"] = serde_json::json!(module.path);
                                        }
                                    }
                                }
                                crate::docs_diagnostic::encode(&report)
                            } else {
                                format!("{}: {e}", module.path.display())
                            }
                        })?;
                        timing.type_check_ms = type_check_started.elapsed().as_secs_f64() * 1000.0;
                        report.compiled_modules.push(id.clone());
                        crate::build_progress::checked(&id);
                        report.inferred_expressions +=
                            inferred.expressions.iter().flatten().count();
                    }
                    // Type errors retain priority over coverage errors. Reject
                    // incomplete patterns before computing interface hashes or
                    // serializing types that cannot belong to a valid module.
                    let coverage_started = Instant::now();
                    // Successful artifact import verifies the exact module source and
                    // all dependency interfaces, including constructor exposure, names,
                    // arities and argument types. Only artifacts written after a
                    // successful coverage pass carry this proof. Keep display replays
                    // and nondevelopment analysis on the ordinary validation path.
                    if !(coverage_checked && stage == Stage::Generate
                        && mode == crate::kernel::Mode::Development && !preserve_display) {
                        crate::coverage::check_report(&ast, &resolved, &symbols, &module.path)
                            .map_err(|e| {
                                if profile_analysis {
                                    eprintln!("ELM_ANALYSIS_PROFILE {}", serde_json::json!({
                                        "phase":"coverage_failure", "module":id,
                                        "parse_ms":timing.parse_ms, "preparation_ms":timing.preparation_ms,
                                        "type_load_ms":timing.type_load_ms, "type_restore_ms":timing.type_restore_ms,
                                        "type_check_ms":timing.type_check_ms,
                                        "coverage_ms":coverage_started.elapsed().as_secs_f64()*1000.0,
                                        "module_ms":started.elapsed().as_secs_f64()*1000.0
                                    }));
                                }
                                if crate::docs_diagnostic::report_encoded(&e).is_some() {
                                    e
                                } else {
                                    format!("{}: {e}", module.path.display())
                                }
                            })?;
                    }
                    let coverage_ms = coverage_started.elapsed().as_secs_f64() * 1000.0;
                    let interface_started = Instant::now();
                    if type_cache.is_some_and(crate::typed_cache::TypeCache::uses_interface_fingerprints) {
                        // The verified artifact key includes this module's source,
                        // compiler, manifests and dependency interfaces. Once its
                        // types restore successfully, its public hash is reusable.
                        interface_fingerprint = interface_fingerprint.or_else(|| {
                            crate::type_interface::fingerprint(
                                &interface,
                                &local,
                                &symbols,
                                &types,
                                engine,
                                &global_types,
                            )
                            .ok()
                        });
                        if let Some(fingerprint) = interface_fingerprint {
                            type_interfaces.insert(id.clone(), fingerprint);
                        }
                    }
                    timing.interface_ms = interface_started.elapsed().as_secs_f64() * 1000.0;
                    let artifact_started = Instant::now();
                    if !was_restored
                        && let Some(cache) = &module_cache
                        && let Ok(mut artifact) = crate::typed_artifact::export_selected(
                            &id,
                            &symbols,
                            engine,
                            &global_types,
                            &required,
                        )
                    {
                        artifact["coverage_checked"] = serde_json::json!(true);
                        if let Some(fingerprint) = interface_fingerprint {
                            artifact["interface_fingerprint"] = serde_json::json!(fingerprint);
                        }
                        if let Ok(bytes) = serde_json::to_vec(&artifact) {
                            let _ = cache.store(&bytes);
                            if let Some(text_cache) = &text_cache { let _ = text_cache.store(&bytes); }
                        }
                    }
                    timing.type_artifact_ms = artifact_started.elapsed().as_secs_f64() * 1000.0;
                    timing.inference_ms = inference_started.elapsed().as_secs_f64() * 1000.0 - coverage_ms;
                    let validation_started = Instant::now();
                    if ast
                        .declarations
                        .iter()
                        .any(|d| matches!(d, crate::ast::Declaration::Value { name: "main", .. }))
                    {
                        let main = symbols
                            .lookup(&id, "main", crate::names::Space::Value)
                            .ok_or("missing main symbol")?;
                        crate::entry::check_main(engine, &symbols, &global_types[&main]).map_err(
                            |error| {
                                if error.starts_with("bad main type:") {
                                    // Re-lower a verified annotation only on the error path,
                                    // retaining its aliases and source variable names without
                                    // adding display metadata to successful compilations.
                                    let annotated =
                                        ast.declarations.iter().find_map(|declaration| {
                                            match declaration {
                                                crate::ast::Declaration::Annotation {
                                                    name: "main",
                                                    ty,
                                                } => Some(*ty),
                                                _ => None,
                                            }
                                        });
                                    let display_type = annotated
                                        .and_then(|ty| {
                                            engine.track_display_names();
                                            types
                                                .annotation(&ast, &resolved, &symbols, engine, ty)
                                                .ok()
                                                .map(|scheme| scheme.root)
                                        })
                                        .unwrap_or(global_types[&main].root);
                                    if let Some(report) = crate::main_diagnostic::bad_type(
                                        &ast,
                                        &module.owner,
                                        &module.path,
                                        engine,
                                        &symbols,
                                        display_type,
                                    ) {
                                        return report;
                                    }
                                }
                                if error.starts_with("bad main flags:")
                                    && let Some(report) = crate::main_diagnostic::bad_flags(
                                        &ast,
                                        &module.owner,
                                        &module.path,
                                        engine,
                                        &symbols,
                                        global_types[&main].root,
                                    )
                                {
                                    return report;
                                }
                                format!("{}: {error}", module.path.display())
                            },
                        )?;
                        if stage == Stage::Generate && graph.entries.contains(&id) {
                            let mut entry_roots = BTreeSet::from([main]);
                            let term = engine
                                .structure(global_types[&main].root)
                                .ok_or("missing main type")?;
                            let name = crate::js_names::global(&id, "main")?;
                            let expression = if let crate::unify::Term::Named(_, args) = &*term {
                                if args.len() == 3 {
                                    let flags = args[0];
                                    let decoder = if matches!(
                                        engine.structure(flags).as_deref(),
                                        Some(crate::unify::Term::Unit)
                                    ) {
                                        "_Json_succeed(_Utils_Tuple0)".into()
                                    } else {
                                        let converter = crate::port_codegen::converter_with_fields(
                                            engine,
                                            &symbols,
                                            flags,
                                            true,
                                            mode,
                                            &layouts.fields,
                                        )?;
                                        entry_roots.extend(converter.dependencies);
                                        converter.javascript
                                    };
                                    let metadata = if mode == crate::kernel::Mode::Debug {
                                        crate::debug_metadata::extract(
                                            engine, &types, &symbols, args[2],
                                        )?
                                        .to_string()
                                    } else {
                                        "0".into()
                                    };
                                    format!("{name}({decoder})({metadata})")
                                } else {
                                    format!("_VirtualDom_init({name})(0)(0)")
                                }
                            } else {
                                return Err("unexpected main type".into());
                            };
                            let mut export = format!("{{'init':{expression}}}");
                            for segment in module.name.split('.').rev() {
                                export = format!(
                                    "{{{}:{export}}}",
                                    serde_json::to_string(segment).unwrap()
                                );
                            }
                            let javascript = format!("_Platform_export({export});\n");
                            report.link_roots.extend(entry_roots.iter().copied());
                            report.main_export.get_or_insert_with(String::new).push_str(&javascript);
                            report.entry_outputs.insert(id.clone(), EntryOutput { roots: entry_roots, javascript });
                        }
                    }
                    timing.validation_ms = coverage_ms + validation_started.elapsed().as_secs_f64() * 1000.0;
                }
            }
            if let Some(docs) = &docs {
                if let Err(error) = crate::docs::validate(&ast, docs)
                    && let Some(report) = crate::docs_diagnostic::report(&ast, &error, &module.path)
                {
                    if let Some(errors) = report["errors"].as_array() {
                        documentation_errors.extend(errors.iter().cloned());
                    }
                    blocked_documentation.insert(id.clone());
                    continue;
                }
                report.documentation.push(
                    crate::docs::to_json(&ast, docs, &resolved, &symbols)
                        .map_err(|e| format!("{}: {e}", module.path.display()))?,
                );
            }
            if matches!(stage, Stage::Generate | Stage::Repl) {
                let generation_started = Instant::now();
                layouts.register(&ast, &id, &symbols)?;
                let generated_cache = if generate_output && !has_debug_uses {
                    type_cache.and_then(|cache| cache.generated_for_interfaces(
                        &id, &type_interfaces, &name_interfaces, &symbols,
                    ))
                } else { None };
                let cached_definitions = generated_cache.as_ref()
                    .and_then(|cache| cache.load())
                    .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                    .and_then(|value| crate::generated_cache::decode(&value, &symbols));
                let generated = if let Some(definitions) = cached_definitions {
                    report.cached_generated_modules += 1;
                    Ok(definitions)
                } else { (|| {
                    let mut ports = BTreeMap::new();
                    for declaration in &ast.declarations {
                        if let crate::ast::Declaration::Port { name, .. } = declaration {
                            let symbol = symbols
                                .lookup(&id, name, crate::names::Space::Value)
                                .ok_or("missing port identity")?;
                            let engine = engine.as_mut().ok_or("missing type engine")?;
                            let port = crate::entry::check_port(
                                engine,
                                &symbols,
                                global_types[&symbol].root,
                            )?;
                            let incoming = port.direction == crate::entry::Direction::Incoming;
                            ports.insert(
                                symbol,
                                (
                                    incoming,
                                    crate::port_codegen::converter_with_fields(
                                        engine,
                                        &symbols,
                                        port.payload,
                                        incoming,
                                        mode,
                                        &layouts.fields,
                                    )?,
                                ),
                            );
                        }
                    }
                    if generate_output && !has_debug_uses {
                        crate::module_codegen::emit_after_coverage(
                            &ast,
                            &id,
                            &resolved,
                            &symbols,
                            &layouts,
                            (mode, Some(&ports)),
                        )
                    } else {
                        // Error-only replays retain main/port validation above,
                        // but their generated definitions would be discarded.
                        Ok(Vec::new())
                    }
                })().inspect(|definitions| {
                    if let Some(cache) = &generated_cache
                        && let Ok(bytes) = serde_json::to_vec(&crate::generated_cache::encode(definitions, &symbols)) {
                        let _ = cache.store(&bytes);
                    }
                }) };
                match generated {
                    Ok(definitions) => {
                        report.generated_modules += 1;
                        report.generated.extend(definitions.into_iter().map(std::rc::Rc::new));
                    }
                    Err(error) => report
                        .generation_errors
                        .push(format!("{}: {error}", module.path.display())),
                }
                timing.generation_ms = generation_started.elapsed().as_secs_f64() * 1000.0;
            }
            if stage == Stage::Repl && id == graph.entry {
                report.repl_type_names = Some(crate::type_localizer::Localizer::from_header(
                    &ast.header,
                    &module.owner,
                ));
                report.repl_types = Some(crate::typed_artifact::export(
                    &id,
                    &symbols,
                    engine.as_mut().ok_or("missing REPL type engine")?,
                    &global_types,
                )?);
            }
            crate::type_interface::retain_public_globals(
                &id,
                &interface,
                &symbols,
                &mut global_types,
            );
            name_interfaces.insert(id.clone(), std::rc::Rc::new(interface));
        }
        report.elm_modules += 1;
        report.declarations += ast.declarations.len();
        report.expressions += ast.expressions.len();
        report.patterns += ast.patterns.len();
        report.types += ast.types.len();
        interfaces.insert(id, exports(&ast, local)?);
        if matches!(
            stage,
            Stage::Check | Stage::Documentation | Stage::Generate | Stage::Repl
        ) && let Some(engine) = &mut engine
            && engine.node_count() > retained_types + 100_000
        {
            let compaction_started = Instant::now();
            types.compact_for_analysis(engine, &mut global_types);
            retained_types = engine.node_count();
            timing.compaction_ms = compaction_started.elapsed().as_secs_f64() * 1000.0;
        }
        timing.total_ms = started.elapsed().as_secs_f64() * 1000.0;
        // interface_ms is nested within inference_ms, not a separate phase.
        timing.finalization_ms = (timing.total_ms
            - timing.parse_ms
            - timing.preparation_ms
            - timing.inference_ms
            - timing.validation_ms
            - timing.generation_ms
            - timing.compaction_ms)
            .max(0.0);
        if profile_analysis {
            eprintln!("ELM_ANALYSIS_PROFILE {}", serde_json::json!({
                "phase":"module", "module":timing.module, "total_ms":timing.total_ms,
                "parse_ms":timing.parse_ms, "preparation_ms":timing.preparation_ms,
                "inference_ms":timing.inference_ms, "type_restore_ms":timing.type_restore_ms,
                "type_load_ms":timing.type_load_ms, "type_decode_ms":timing.type_decode_ms,
                "type_import_ms":timing.type_import_ms, "type_check_ms":timing.type_check_ms,
                "type_artifact_ms":timing.type_artifact_ms, "validation_ms":timing.validation_ms,
                "finalization_ms":timing.finalization_ms, "generation_ms":timing.generation_ms,
                "compaction_ms":timing.compaction_ms
            }));
        }
        report.module_timings.push(timing);
    }
    // Reused symbols keep their identities, but definition order must follow
    // this graph so the linker emits the same bytes as a fresh compilation.
    if continuation.as_ref().is_some_and(|plan| plan.reordered) {
        let order: BTreeMap<_, _> = graph.modules.iter().enumerate()
            .map(|(index, module)| (format!("{}:{}", module.owner, module.name), index)).collect();
        report.generated.sort_by_key(|definition| order.get(symbols.get(definition.symbol).module.as_ref()).copied());
    }
    if matches!(stage, Stage::Generate | Stage::Repl) {
        for (index, symbol) in symbols.entries.iter().enumerate() {
            if !matches!(symbol.kind, crate::names::SymbolKind::Type { .. }) {
                report.link_names.insert(
                    (symbol.module.to_string(), symbol.name.to_string()),
                    crate::names::SymbolId(index as u32),
                );
            }
        }
    }
    if !documentation_errors.is_empty() {
        documentation_errors.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        return Err(crate::docs_diagnostic::encode(&serde_json::json!({
            "type": "compile-errors", "errors": documentation_errors
        })));
    }
    report.symbols = symbols.entries.len();
    report.type_nodes = engine.as_ref().map_or(0, |engine| engine.node_count());
    report.aliases = types.aliases.len();
    report.constructors = types.constructors.len();
    if profile_analysis {
        eprintln!("ELM_ANALYSIS_PROFILE {}", serde_json::json!({"phase":"complete", "ms":pass_started.elapsed().as_secs_f64()*1000.0, "resumed_modules":report.resumed_modules, "modules":report.module_timings.len()}));
    }
    Ok(report)
}
pub(crate) fn default_imports() -> Vec<(&'static str, &'static str, Exposing)> {
    let ty = |name: &str, constructors| {
        Exposing::Explicit(vec![Exposed::Type {
            name: name.into(),
            constructors,
        }])
    };
    vec![
        ("Basics", "Basics", Exposing::All),
        ("Debug", "Debug", Exposing::Explicit(vec![])),
        (
            "List",
            "List",
            Exposing::Explicit(vec![Exposed::Operator("::".into())]),
        ),
        ("Maybe", "Maybe", ty("Maybe", true)),
        ("Result", "Result", ty("Result", true)),
        ("String", "String", ty("String", false)),
        ("Char", "Char", ty("Char", false)),
        ("Tuple", "Tuple", Exposing::Explicit(vec![])),
        ("Platform", "Platform", ty("Program", false)),
        ("Platform.Cmd", "Cmd", ty("Cmd", false)),
        ("Platform.Sub", "Sub", ty("Sub", false)),
    ]
}

pub fn worker_statistics() -> serde_json::Value {
    let mut statistics = prefix::statistics();
    statistics["verified"] = verified::statistics();
    statistics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixity::{Associativity, Fixity};
    fn foreign(precedence: u8) -> Table {
        Table::from([(
            "+".into(),
            Fixity {
                precedence,
                associativity: Associativity::Left,
            },
        )])
    }
    #[test]
    fn explicit_operator_import_overwrites_but_open_imports_accumulate() {
        let mut scope = Table::new();
        let mut origins = BTreeMap::new();
        add(&mut scope, &mut origins, &foreign(6), "A", &Exposing::All).unwrap();
        add(&mut scope, &mut origins, &foreign(7), "B", &Exposing::All).unwrap();
        assert_eq!(origins["+"].len(), 2);
        let explicit = Exposing::Explicit(vec![Exposed::Operator("+".into())]);
        add(&mut scope, &mut origins, &foreign(8), "C", &explicit).unwrap();
        assert_eq!(origins["+"], BTreeSet::from(["C".into()]));
        assert_eq!(scope["+"].precedence, 8);
    }
    #[test]
    fn missing_explicit_operator_is_an_error() {
        assert!(
            add(
                &mut Table::new(),
                &mut BTreeMap::new(),
                &Table::new(),
                "A",
                &Exposing::Explicit(vec![Exposed::Operator("+".into())])
            )
            .is_err()
        );
    }
}
