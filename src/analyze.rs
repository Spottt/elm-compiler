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
#[derive(Debug, Default)]
pub struct ModuleTiming {
    pub module: String,
    pub parse_ms: f64,
    pub inference_ms: f64,
    pub interface_ms: f64,
    pub generation_ms: f64,
    pub compaction_ms: f64,
    pub total_ms: f64,
}
#[derive(Debug, Default)]
pub struct Report {
    /// Detached inferred schemes, aliases and source variable names for the
    /// REPL entry only.
    pub repl_types: Option<serde_json::Value>,
    pub repl_type_names: Option<crate::type_localizer::Localizer>,
    pub documentation: Vec<serde_json::Value>,
    pub module_timings: Vec<ModuleTiming>,
    pub field_layout_ms: f64,
    pub cached_type_modules: usize,
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
    pub generated: Vec<crate::module_codegen::Definition>,
    pub generation_errors: Vec<String>,
    pub main_export: Option<String>,
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
    let first = match project_pass_once(graph, stage, mode, type_cache) {
        Ok(report) => return Ok(report),
        Err(error) => error,
    };
    // Failed inference can leave partially unified nodes. Restart only the error
    // path on an independent graph; successful builds never clone or replay it.
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
            parse(&module.source).map(|_| ())
        };
        if let Err(error) = result {
            syntax_paths.push(module.path.to_string_lossy().into_owned());
            messages.push(crate::source_error::in_module(
                &module.name,
                format!("{}:{error}", module.path.display()),
            ));
        }
    }
    let mut error = if syntax_paths.is_empty() {
        first
    } else {
        remove_failed_modules(&mut remaining, &syntax_paths);
        match project_pass_once(&remaining, stage, mode, None) {
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
        match project_pass_once(&remaining, stage, mode, None) {
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
    !blocked.is_empty()
}

fn project_pass_once(
    graph: &Graph,
    stage: Stage,
    mode: crate::kernel::Mode,
    type_cache: Option<&crate::typed_cache::TypeCache>,
) -> Result<Report, String> {
    let mut symbols = crate::names::Symbols::default();
    let mut types = crate::types::Catalog::default();
    let mut engine = matches!(
        stage,
        Stage::DeclaredTypes | Stage::Check | Stage::Documentation | Stage::Generate | Stage::Repl
    )
    .then(|| crate::unify::Engine::new(crate::types::builtins(&mut symbols)));
    if stage == Stage::Repl {
        engine
            .as_mut()
            .expect("REPL type engine")
            .track_display_names();
    }
    let mut global_types = BTreeMap::new();
    let mut type_interfaces = BTreeMap::new();
    let mut name_interfaces = BTreeMap::<String, crate::names::Interface>::new();
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
    let mut documentation_errors = Vec::new();
    let mut blocked_documentation = BTreeSet::new();
    for module in &graph.modules {
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
            let mut env = crate::names::Environment::default();
            env.builtin_list(&mut symbols);
            if module.owner != "elm/core" {
                for (name, prefix, exposure) in default_imports() {
                    let from = format!("elm/core:{name}");
                    let interface = name_interfaces
                        .get(&from)
                        .ok_or_else(|| format!("missing name interface {from}"))?;
                    env.import(prefix, interface, &exposure, &symbols)?;
                }
            }
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
                    env.import(
                        import.alias.as_deref().unwrap_or(&import.name),
                        interface,
                        &import.exposing,
                        &symbols,
                    )?;
                }
            }
            let (interface, resolved) = crate::names::resolve(&ast, &id, env, &mut symbols)
                .map_err(|e| format!("{}: {e}", module.path.display()))?;
            if mode == crate::kernel::Mode::Production && id != "elm/core:Debug" {
                for binding in resolved.expressions.iter().flatten() {
                    if let crate::names::Binding::Global(symbol) = binding {
                        let symbol = symbols.get(*symbol);
                        if symbol.module.as_ref() == "elm/core:Debug" {
                            return Err(format!(
                                "{}: Debug.{} is not allowed in production output",
                                module.path.display(),
                                symbol.name
                            ));
                        }
                    }
                }
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
                    let inference_started = Instant::now();
                    let required = if type_cache.is_some() {
                        crate::typed_artifact::required_globals(&id, &ast, &interface, &symbols)?
                    } else {
                        BTreeSet::new()
                    };
                    let module_cache =
                        type_cache.and_then(|cache| cache.for_interfaces(&id, &type_interfaces));
                    let restored = module_cache
                        .as_ref()
                        .and_then(|cache| cache.load())
                        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                        .and_then(|artifact| {
                            let restored = crate::typed_artifact::import_selected(
                                &artifact, &id, &symbols, engine, &required,
                            )
                            .ok()?;
                            let fingerprint =
                                artifact.get("interface_fingerprint").and_then(|value| {
                                    serde_json::from_value::<[u8; 32]>(value.clone()).ok()
                                });
                            Some((restored, fingerprint))
                        });
                    let was_restored = restored.is_some();
                    let mut interface_fingerprint = None;
                    if let Some((restored, fingerprint)) = restored {
                        global_types.extend(restored);
                        report.cached_type_modules += 1;
                        interface_fingerprint = fingerprint;
                    } else {
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
                        .map_err(|e| format!("{}: {e}", module.path.display()))?;
                        report.inferred_expressions +=
                            inferred.expressions.iter().flatten().count();
                    }
                    let interface_started = Instant::now();
                    if type_cache.is_some() {
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
                        if let Some(fingerprint) = interface_fingerprint {
                            artifact["interface_fingerprint"] = serde_json::json!(fingerprint);
                        }
                        if let Ok(bytes) = serde_json::to_vec(&artifact) {
                            let _ = cache.store(&bytes);
                        }
                    }
                    timing.inference_ms = inference_started.elapsed().as_secs_f64() * 1000.0;
                    crate::coverage::check(&ast, &resolved, &symbols)
                        .map_err(|e| format!("{}: {e}", module.path.display()))?;
                    if ast
                        .declarations
                        .iter()
                        .any(|d| matches!(d, crate::ast::Declaration::Value { name: "main", .. }))
                    {
                        let main = symbols
                            .lookup(&id, "main", crate::names::Space::Value)
                            .ok_or("missing main symbol")?;
                        crate::entry::check_main(engine, &symbols, &global_types[&main])
                            .map_err(|e| format!("{}: {e}", module.path.display()))?;
                        if stage == Stage::Generate && graph.entries.contains(&id) {
                            report.link_roots.insert(main);
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
                                        report.link_roots.extend(converter.dependencies);
                                        converter.javascript
                                    };
                                    format!("{name}({decoder})(0)")
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
                            report
                                .main_export
                                .get_or_insert_with(String::new)
                                .push_str(&format!("_Platform_export({export});\n"));
                        }
                    }
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
                let generated = (|| {
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
                    crate::module_codegen::emit_with_ports(
                        &ast,
                        &id,
                        &resolved,
                        &symbols,
                        &layouts,
                        (mode, Some(&ports)),
                    )
                })();
                match generated {
                    Ok(definitions) => {
                        report.generated_modules += 1;
                        report.generated.extend(definitions);
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
            name_interfaces.insert(id.clone(), interface);
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
            types.compact(engine, &mut global_types);
            retained_types = engine.node_count();
            timing.compaction_ms = compaction_started.elapsed().as_secs_f64() * 1000.0;
        }
        timing.total_ms = started.elapsed().as_secs_f64() * 1000.0;
        report.module_timings.push(timing);
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
