//! Experimental whole-project assembly; entry-point exports are added separately.
use crate::{analyze::Report, js_names, kernel, module_codegen, project::Graph};
use std::{borrow::Borrow, collections::{BTreeMap, BTreeSet, HashMap, HashSet}};
pub fn helpers() -> String {
    // Persistent argument lists permit reusing a partially applied constructor
    // without copying the prefix for each additional argument.
    let mut out = String::from(
        r#"
function _Rust_curry(arity,fun){
  function step(reverse,count){
    return function(value){
      var values={a:value,b:reverse};
      if(count+1<arity)return step(values,count+1);
      var args=new Array(arity);
      for(var i=arity;i--;values=values.b)args[i]=values.a;
      return fun.apply(null,args);
    };
  }
  return step(null,0);
}
"#,
    );
    for arity in 2..=9 {
        let args = (0..arity).map(|i| format!("a{i}")).collect::<Vec<_>>();
        let mut curried = String::new();
        for arg in &args {
            curried.push_str(&format!("function({arg}){{return "));
        }
        curried.push_str(&format!("fun({})", args.join(",")));
        for _ in &args {
            curried.push_str(";}");
        }
        out.push_str(&format!("function F{arity}(fun){{var wrapper={curried};wrapper.a={arity};wrapper.f=fun;return wrapper;}}\n"));
        let applied = args.iter().map(|a| format!("({a})")).collect::<String>();
        out.push_str(&format!(
            "function A{arity}(fun,{args}){{return fun.a==={arity}?fun.f({args}):fun{applied};}}\n",
            args = args.join(",")
        ));
    }
    out
}
/// Link one entry from a successful shared analysis. `graph` must be that
/// entry's complete discovery snapshot, drawn from the same source revision.
/// Its module order and kernels preserve ordinary single-entry output ordering.
pub fn assemble_entry(graph: &Graph, report: Report) -> Result<String, String> {
    assemble_entry_shared(graph, &report)
}

/// Reuse analysis metadata across entry bundles. Only the cheap shared-definition
/// vector and entry roots are copied; cycle initialization remains copy-on-write.
pub fn assemble_entry_shared(graph: &Graph, report: &Report) -> Result<String, String> {
    if graph.entries.len() != 1 || graph.entries[0] != graph.entry {
        return Err("entry linking requires one entry".into());
    }
    if report.mode != kernel::Mode::Development {
        return Err("entry linking currently requires development mode".into());
    }
    let entry = report.entry_outputs.get(&graph.entry)
        .ok_or_else(|| format!("missing analyzed entry {}", graph.entry))?;
    let mut generated = report.generated.clone();
    let order: BTreeMap<_, _> = graph.modules.iter().enumerate()
        .map(|(index, module)| (format!("{}:{}", module.owner, module.name), index)).collect();
    let ranks: HashMap<_, _> = report.link_names.iter()
        .filter_map(|((module, _), symbol)| order.get(module).map(|rank| (*symbol, *rank))).collect();
    generated.sort_by_key(|definition| ranks.get(&definition.symbol).copied());
    assemble_parts(graph, report, generated, entry.roots.clone(), Some(&entry.javascript))
}

pub fn assemble(graph: &Graph, mut report: Report) -> Result<String, String> {
    let generated = std::mem::take(&mut report.generated);
    let roots = std::mem::take(&mut report.link_roots);
    assemble_parts(graph, &report, generated, roots, report.main_export.as_deref())
}

fn assemble_parts(
    graph: &Graph,
    report: &Report,
    generated: Vec<std::rc::Rc<module_codegen::Definition>>,
    mut roots: BTreeSet<crate::names::SymbolId>,
    export: Option<&str>,
) -> Result<String, String> {
    if !report.generation_errors.is_empty() {
        return Err(report.generation_errors.join("\n"));
    }
    let mut out = String::from("(function(scope){\n'use strict';\n");
    out.push_str(&helpers());
    let mut kernel_modules: Vec<_> = graph
        .modules
        .iter()
        .filter(|module| module.kernel)
        .collect();
    if report.mode == kernel::Mode::Debug {
        // Browser chooses its implementation at initialization time. The
        // debugger's functions only use other kernels when called, so defining
        // them first makes Browser.element/document select their wrappers.
        kernel_modules.sort_by_key(|module| !module.name.ends_with("Kernel.Debugger"));
    }
    for module in kernel_modules {
        if !module.kernel {
            continue;
        }
        let content = kernel::parse(&module.source)?;
        let rendered = kernel::render(
            &content,
            report.mode,
            |home, name| {
                let canonical = module
                    .imports
                    .iter()
                    .find(|id| id.ends_with(&format!(":{home}")))
                    .ok_or_else(|| format!("missing kernel import {home}"))?;
                if canonical == "elm/core:Basics" && matches!(name, "True" | "False") {
                    return Ok(name.to_lowercase());
                }
                let id = report
                    .link_names
                    .get(&(canonical.clone(), name.into()))
                    .ok_or_else(|| format!("missing kernel Elm symbol {canonical}.{name}"))?;
                roots.insert(*id);
                js_names::global(canonical, name)
            },
            |name| report.fields.name(name),
            crate::js_names::kernel_field,
        )?;
        out.push_str(&rendered);
        out.push('\n');
    }
    let definitions = module_codegen::order_shared(reachable_definitions(generated, roots))?;
    for definition in &definitions {
        out.push_str(&definition.javascript);
    }
    let mut registered = BTreeSet::new();
    for registration in definitions.iter().filter_map(|d| d.registration.as_ref()) {
        if registered.insert(&registration.javascript) {
            out.push_str(&registration.javascript);
        }
    }
    if let Some(export) = export {
        out.push_str(export);
    }
    // Match Elm's script/CommonJS contract: the enclosing module's `this`
    // is its exports object under require/Webpack, and the window in a script.
    // elm-hot also recognizes this exact IIFE shape before injecting hooks.
    out.push_str("\n}(this));\n");
    Ok(out)
}

/// Assemble evaluation of a checked binding, using Elm's Debug renderer and
/// Node output protocol. The annotation is supplied by the caller; this linker
/// does not infer or localize type names. See Generate.JavaScript.generateForRepl
/// in Elm 0.19.1 for the exception handler and value/type line-break rules.
pub fn assemble_for_repl(
    graph: &Graph,
    mut report: Report,
    binding: &str,
    annotation: &str,
    ansi: bool,
) -> Result<String, String> {
    if report.mode != kernel::Mode::Development {
        return Err("REPL evaluation requires development representations".into());
    }
    let value = *report
        .link_names
        .get(&(graph.entry.clone(), binding.into()))
        .ok_or_else(|| format!("missing REPL value {}.{binding}", graph.entry))?;
    let debug = *report
        .link_names
        .get(&("elm/core:Debug".into(), "toString".into()))
        .ok_or("REPL evaluation requires elm/core Debug.toString")?;
    report.link_roots = BTreeSet::from([value, debug]);
    let value = js_names::global(&graph.entry, binding)?;
    let annotation = serde_json::to_string(annotation).map_err(|e| e.to_string())?;
    report.main_export = Some(format!(
        r#"
var _value = _Debug_toAnsiString({ansi}, {value});
var _type = {annotation};
function _print(t) {{ console.log(_value + ({ansi} ? '\x1b[90m' + t + '\x1b[0m' : t)); }}
if (_value.length + 3 + _type.length >= 80 || _type.indexOf('\n') >= 0) {{
    _print('\n    : ' + _type.split('\n').join('\n      '));
}} else {{
    _print(' : ' + _type);
}}
"#
    ));
    let body = assemble(graph, report)?;
    Ok(format!(
        "process.on('uncaughtException', function(err) {{ process.stderr.write(err.toString() + '\\n'); process.exit(1); }});\n{body}"
    ))
}

/// Keep transitive value dependencies and callbacks of reachable effect leaves.
/// Kernel references and the main's flags decoder are added to roots by assembly.
pub fn reachable(
    definitions: Vec<module_codegen::Definition>,
    roots: BTreeSet<crate::names::SymbolId>,
) -> Vec<module_codegen::Definition> {
    reachable_definitions(definitions, roots)
}

fn reachable_definitions<D: Borrow<module_codegen::Definition>>(
    definitions: Vec<D>,
    roots: BTreeSet<crate::names::SymbolId>,
) -> Vec<D> {
    let index: HashMap<_, _> = definitions.iter().map(|d| {
        let definition: &module_codegen::Definition = d.borrow();
        (definition.symbol, definition)
    }).collect();
    // Lookup tables are never iterated: emission retains the input order.
    let mut seen = HashSet::with_capacity(definitions.len());
    let mut pending: Vec<_> = roots.into_iter().collect();
    while let Some(symbol) = pending.pop() {
        if !seen.insert(symbol) {
            continue;
        }
        if let Some(definition) = index.get(&symbol) {
            pending.extend(&definition.dependencies);
            if let Some(registration) = &definition.registration {
                pending.extend(&registration.dependencies);
            }
        }
    }
    definitions
        .into_iter()
        .filter(|d| seen.contains(&Borrow::<module_codegen::Definition>::borrow(d).symbol))
        .collect()
}
