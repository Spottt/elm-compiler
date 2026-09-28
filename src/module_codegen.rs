//! Generate independently linkable definitions from a resolved, checked module.
use crate::{
    ast::{Declaration, PatternId, Syntax, Type},
    codegen,
    infer::{components, expr_children},
    js_names::{self, Layouts, Naming},
    kernel::Mode,
    names::{Binding, Resolved, Space, SymbolId, Symbols},
    pattern_codegen,
};
use std::{borrow::Borrow, collections::{BTreeMap, BTreeSet, HashMap}, rc::Rc};
#[derive(Debug, Clone)]
pub struct Registration {
    pub javascript: String,
    pub dependencies: BTreeSet<SymbolId>,
}
#[derive(Debug, Clone)]
pub struct Definition {
    pub symbol: SymbolId,
    pub javascript: String,
    pub dependencies: BTreeSet<SymbolId>,
    pub function: bool,
    pub registration: Option<Registration>,
    pub cycle_initialization: Option<String>,
}
pub fn emit(
    ast: &Syntax<'_>,
    module: &str,
    resolved: &Resolved,
    symbols: &Symbols,
    layouts: &Layouts,
    mode: Mode,
) -> Result<Vec<Definition>, String> {
    emit_with_ports(ast, module, resolved, symbols, layouts, (mode, None))
}
pub type PortConverters = BTreeMap<SymbolId, (bool, crate::port_codegen::Converter)>;
pub fn emit_with_ports(
    ast: &Syntax<'_>,
    module: &str,
    resolved: &Resolved,
    symbols: &Symbols,
    layouts: &Layouts,
    options: (Mode, Option<&PortConverters>),
) -> Result<Vec<Definition>, String> {
    // Standalone callers must prove coverage before using a default branch.
    crate::coverage::check(ast, resolved, symbols)?;
    emit_after_coverage(ast, module, resolved, symbols, layouts, options)
}

/// Internal pipeline entry: the caller must have successfully checked coverage
/// for this exact AST and resolution. Analysis does this even on a type-cache hit.
pub(crate) fn emit_after_coverage(
    ast: &Syntax<'_>,
    module: &str,
    resolved: &Resolved,
    symbols: &Symbols,
    layouts: &Layouts,
    options: (Mode, Option<&PortConverters>),
) -> Result<Vec<Definition>, String> {
    let (mode, ports) = options;
    let final_patterns: BTreeSet<_> = ast
        .expressions
        .iter()
        .filter_map(|node| {
            if let crate::ast::Expr::Case(_, branches) = &node.kind {
                branches.last().map(|(id, _)| *id)
            } else {
                None
            }
        })
        .collect();
    let cyclic_values = cyclic_values(ast, module, resolved, symbols)?;
    let mut output = Vec::new();
    let references = Naming {
        symbols,
        resolved,
        layouts,
        mode,
    };
    for declaration in &ast.declarations {
        let mut add = |symbol, expression: String, dependencies, function| -> Result<(), String> {
            let name = references.symbol(symbol)?.name;
            // Bool constructors are emitted as primitive literals at references.
            if name != "true" && name != "false" {
                output.push(Definition {
                    symbol,
                    javascript: if cyclic_values.contains(&symbol) {
                        format!("function {name}$cycle(){{return {expression};}}\n")
                    } else {
                        format!("var {name}={expression};\n")
                    },
                    dependencies,
                    function,
                    registration: None,
                    cycle_initialization: cyclic_values.contains(&symbol).then(|| {
                        format!(
                            "var {name}={name}$cycle();{name}$cycle=function(){{return {name};}};\n"
                        )
                    }),
                });
            }
            Ok(())
        };
        match declaration {
            Declaration::Union { variants, .. } => {
                for (name, args) in variants {
                    let id = symbols
                        .lookup(module, name, Space::Constructor)
                        .ok_or("missing union constructor")?;
                    add(
                        id,
                        layouts.constructor_expression(id, symbols, mode)?,
                        BTreeSet::new(),
                        !args.is_empty(),
                    )?;
                }
            }
            Declaration::Alias { name, ty, .. } => {
                if let Type::Record {
                    extension: None,
                    fields,
                } = &ast.types[ty.0 as usize].kind
                {
                    let id = symbols
                        .lookup(module, name, Space::Constructor)
                        .ok_or("missing record constructor")?;
                    let mut expr = String::new();
                    if fields.len() > 9 {
                        let args = (0..fields.len())
                            .map(|i| format!("$r{i}"))
                            .collect::<Vec<_>>()
                            .join(",");
                        expr.push_str(&format!(
                            "_Rust_curry({},function({args}){{return ",
                            fields.len()
                        ));
                    } else {
                        for i in 0..fields.len() {
                            expr.push_str(&format!("(function($r{i}){{return "));
                        }
                    }
                    expr.push_str("({");
                    // Parameter positions follow the alias declaration, but
                    // canonical record properties are emitted in name order.
                    let mut properties: Vec<_> = fields.iter().enumerate().collect();
                    properties.sort_unstable_by_key(|(_, (name, _))| *name);
                    for (position, (i, (name, _))) in properties.into_iter().enumerate() {
                        if position > 0 {
                            expr.push(',');
                        }
                        expr.push_str(&format!(
                            "{}:$r{i}",
                            serde_json::to_string(&layouts.fields.name(name)?).unwrap()
                        ));
                    }
                    expr.push_str("})");
                    if fields.len() > 9 {
                        expr.push_str(";})");
                    } else {
                        for _ in fields {
                            expr.push_str(";})");
                        }
                    }
                    add(id, expr, BTreeSet::new(), !fields.is_empty())?;
                }
            }
            Declaration::Value {
                name,
                arguments,
                body,
            } => {
                let id = symbols
                    .lookup(module, name, Space::Value)
                    .ok_or("missing global definition")?;
                let generator = Expressions {
                    ast,
                    module,
                    resolved,
                    symbols,
                    layouts,
                    mode,
                    cyclic_values: &cyclic_values,
                    final_patterns: &final_patterns,
                };
                let expr = generator.function(Binding::Global(id), arguments, *body)?;
                let mut dependencies = BTreeSet::new();
                let mut pending = vec![*body];
                while let Some(id) = pending.pop() {
                    if let Some(Binding::Global(symbol)) = resolved.expressions[id.0 as usize]
                        && !layouts.inline_constant(symbol, mode)
                    {
                        dependencies.insert(symbol);
                    }
                    pending.extend(expr_children(&ast.expressions[id.0 as usize].kind));
                }
                add(id, expr, dependencies, !arguments.is_empty())?;
            }
            Declaration::Annotation { .. } | Declaration::Infix { .. } => {}
            Declaration::Port { name, .. } => {
                let id = symbols
                    .lookup(module, name, Space::Value)
                    .ok_or("missing port identity")?;
                let (incoming, converter) = ports
                    .and_then(|p| p.get(&id))
                    .ok_or("missing typed port converter")?;
                let factory = if *incoming {
                    "incomingPort"
                } else {
                    "outgoingPort"
                };
                add(
                    id,
                    format!(
                        "_Platform_{factory}({},{})",
                        serde_json::to_string(name).unwrap(),
                        converter.javascript
                    ),
                    converter.dependencies.clone(),
                    false,
                )?;
            }
            Declaration::Destruct { .. } => return Err("unexpected top-level destructuring".into()),
        }
    }
    if let crate::module::Effects::Manager {
        command,
        subscription,
    } = &ast.header.effects
    {
        let (_, home) = module
            .split_once(':')
            .ok_or("invalid manager module identity")?;
        let home = serde_json::to_string(home).unwrap();
        let mut dependencies = BTreeSet::new();
        let mut args = Vec::new();
        for name in [
            Some("init"),
            Some("onEffects"),
            Some("onSelfMsg"),
            command.as_ref().map(|_| "cmdMap"),
            subscription.as_ref().map(|_| "subMap"),
        ] {
            if let Some(name) = name {
                let id = symbols
                    .lookup(module, name, Space::Value)
                    .ok_or("missing effect lifecycle function")?;
                dependencies.insert(id);
                args.push(references.symbol(id)?.name);
            } else {
                args.push("0".into());
            }
        }
        let registration = Some(Registration {
            javascript: format!(
                "_Platform_effectManagers[{home}]=_Platform_createManager({});\n",
                args.join(",")
            ),
            dependencies,
        });
        for (name, enabled) in [
            ("command", command.is_some()),
            ("subscription", subscription.is_some()),
        ] {
            if enabled {
                let id = symbols
                    .lookup(module, name, Space::Value)
                    .ok_or("missing implicit effect function")?;
                output.push(Definition {
                    symbol: id,
                    javascript: format!(
                        "var {}=_Platform_leaf({home});\n",
                        references.symbol(id)?.name
                    ),
                    dependencies: BTreeSet::new(),
                    function: true,
                    registration: registration.clone(),
                    cycle_initialization: None,
                });
            }
        }
    }
    Ok(output)
}
struct Expressions<'a, 's> {
    ast: &'a Syntax<'s>,
    module: &'a str,
    resolved: &'a Resolved,
    symbols: &'a Symbols,
    layouts: &'a Layouts,
    mode: Mode,
    cyclic_values: &'a BTreeSet<SymbolId>,
    final_patterns: &'a BTreeSet<PatternId>,
}
impl Expressions<'_, '_> {
    fn pattern(
        &self,
        id: PatternId,
        value: &str,
        naming: &mut Naming<'_>,
    ) -> Result<pattern_codegen::Plan, String> {
        let mut plan = pattern_codegen::plan(self.ast, id, value, self.mode, naming)?;
        if self.mode == Mode::Production && self.final_patterns.contains(&id) {
            plan.condition = "true".into();
            plan.tests.clear();
        }
        Ok(plan)
    }
    fn naming(&self) -> Naming<'_> {
        Naming {
            symbols: self.symbols,
            resolved: self.resolved,
            layouts: self.layouts,
            mode: self.mode,
        }
    }
    fn reference(&self, id: crate::ast::ExprId) -> Result<codegen::Reference, String> {
        let mut reference = self.naming().reference(id)?;
        if let Some(Binding::Global(symbol)) = self.resolved.expressions[id.0 as usize] {
            let identity = self.symbols.get(symbol);
            if identity.module.as_ref() == "elm/core:Debug" && identity.name.as_ref() == "todo" {
                if matches!(self.mode, Mode::Production) {
                    return Err("Debug.todo is not allowed in optimized output".into());
                }
                let span = self.ast.expressions[id.0 as usize].span;
                let home = self
                    .module
                    .split_once(':')
                    .ok_or("invalid module identity")?
                    .1;
                let position = |offset: u32| {
                    let prefix = &self.ast.source[..offset as usize];
                    serde_json::json!({
                        "line": prefix.bytes().filter(|byte| *byte == b'\n').count() + 1,
                        "column": prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
                    })
                };
                let region =
                    serde_json::json!({"start": position(span.start), "end": position(span.end)});
                reference.name = format!(
                    "_Debug_todo({},{region})",
                    serde_json::to_string(home).unwrap()
                );
            } else if self.cyclic_values.contains(&symbol) {
                reference.name.push_str("$cycle()");
            }
        }
        Ok(reference)
    }
    fn expression(&self, root: crate::ast::ExprId) -> Result<String, String> {
        let mut naming = self.naming();
        codegen::expression_with_definitions(
            self.ast,
            root,
            self.mode,
            |id| self.reference(id),
            |name| self.layouts.fields.name(name),
            |id, value| self.pattern(id, value, &mut naming),
            |id| self.definitions(id),
        )
    }
    fn definitions(
        &self,
        root: crate::ast::ExprId,
    ) -> Result<Vec<codegen::LocalDefinition>, String> {
        let mut definitions =
            codegen::local_definitions(self.ast, self.resolved, root, js_names::local)?;
        for definition in &mut definitions {
            if !definition.arguments.is_empty() {
                let local = *self
                    .resolved
                    .definitions
                    .get(&definition.body)
                    .ok_or("missing local function identity")?;
                definition.javascript = Some(self.function(
                    Binding::Local(local),
                    &definition.arguments,
                    definition.body,
                )?);
            }
        }
        Ok(definitions)
    }
    fn function(
        &self,
        binding: Binding,
        arguments: &[PatternId],
        body: crate::ast::ExprId,
    ) -> Result<String, String> {
        let mut naming = self.naming();
        if let Some(tail) = crate::tail_codegen::emit(
            self.ast,
            self.resolved,
            crate::tail_codegen::Function {
                binding,
                arguments,
                body,
            },
            self.mode,
            |root| self.expression(root),
            |id, value| self.pattern(id, value, &mut naming),
            |id| self.definitions(id),
        )? {
            Ok(tail)
        } else {
            wrap_function(
                self.ast,
                arguments,
                self.expression(body)?,
                self.mode,
                &mut naming,
            )
        }
    }
}

fn wrap_function(
    ast: &Syntax<'_>,
    arguments: &[PatternId],
    body: String,
    mode: Mode,
    naming: &mut Naming<'_>,
) -> Result<String, String> {
    let mut result = String::new();
    if mode == Mode::Production && arguments.len() > 1 {
        let names = (0..arguments.len())
            .map(|i| format!("$toparg{i}"))
            .collect::<Vec<_>>();
        result.push_str(&codegen::flat_function_start(&names));
        for (arg, name) in arguments.iter().zip(&names) {
            result.push_str(&pattern_codegen::plan(ast, *arg, name, mode, naming)?.declarations());
        }
        result.push_str("return ");
        result.push_str(&body);
        result.push_str(";})");
        return Ok(result);
    }
    for (i, arg) in arguments.iter().enumerate() {
        let name = format!("$toparg{i}");
        let plan = pattern_codegen::plan(ast, *arg, &name, mode, naming)?;
        result.push_str(&format!(
            "(function({name}){{{}return ",
            plan.declarations()
        ));
    }
    result.push_str(&body);
    for _ in arguments {
        result.push_str(";})");
    }
    Ok(result)
}
/// Order a set of emitted definitions across module boundaries. Missing
/// dependencies remain external (kernel and entry-point linking handles them).
pub fn order(definitions: Vec<Definition>) -> Result<Vec<Definition>, String> {
    order_using(definitions, |definition| definition)
}

/// Checkpoints share immutable definitions; only cycle initialization needs
/// copy-on-write, otherwise linking must leave those snapshots untouched.
pub(crate) fn order_shared(definitions: Vec<Rc<Definition>>) -> Result<Vec<Rc<Definition>>, String> {
    order_using(definitions, Rc::make_mut)
}

fn order_using<D: Borrow<Definition>>(
    definitions: Vec<D>,
    edit: fn(&mut D) -> &mut Definition,
) -> Result<Vec<D>, String> {
    // Only lookup by symbol; graph traversal still uses sorted edges.
    let indices: HashMap<_, _> = definitions
        .iter()
        .enumerate()
        .map(|(i, d)| (Borrow::<Definition>::borrow(d).symbol, i))
        .collect();
    if indices.len() != definitions.len() {
        return Err("duplicate generated definition".into());
    }
    let edges: Vec<BTreeSet<usize>> = definitions
        .iter()
        .map(|d| {
            Borrow::<Definition>::borrow(d).dependencies
                .iter()
                .filter_map(|id| indices.get(id).copied())
                .collect()
        })
        .collect();
    let groups = components(&edges);
    let mut definitions: Vec<_> = definitions.into_iter().map(Some).collect();
    let mut result = Vec::new();
    for mut group in groups {
        group.sort_by_key(|i| !Borrow::<Definition>::borrow(definitions[*i].as_ref().unwrap()).function);
        let mut initialization = String::new();
        for index in group {
            let mut definition = definitions[index].take().unwrap();
            if Borrow::<Definition>::borrow(&definition).cycle_initialization.is_some() {
                initialization.push_str(&edit(&mut definition).cycle_initialization.take().unwrap());
            }
            result.push(definition);
        }
        if !initialization.is_empty() && let Some(last) = result.last_mut() {
            edit(last).javascript.push_str(&initialization);
        }
    }
    Ok(result)
}

fn cyclic_values(
    ast: &Syntax<'_>,
    module: &str,
    resolved: &Resolved,
    symbols: &Symbols,
) -> Result<BTreeSet<SymbolId>, String> {
    let mut values = Vec::new();
    for declaration in &ast.declarations {
        if let Declaration::Value {
            name,
            arguments,
            body,
        } = declaration
        {
            values.push((
                symbols
                    .lookup(module, name, Space::Value)
                    .ok_or("missing global identity")?,
                arguments.is_empty(),
                *body,
            ));
        }
    }
    let indices: BTreeMap<_, _> = values
        .iter()
        .enumerate()
        .map(|(i, (id, _, _))| (*id, i))
        .collect();
    let mut edges = vec![BTreeSet::new(); values.len()];
    for (i, (_, _, body)) in values.iter().enumerate() {
        let mut pending = vec![*body];
        while let Some(id) = pending.pop() {
            if let Some(Binding::Global(symbol)) = resolved.expressions[id.0 as usize]
                && let Some(target) = indices.get(&symbol)
            {
                edges[i].insert(*target);
            }
            pending.extend(expr_children(&ast.expressions[id.0 as usize].kind));
        }
    }
    let mut cyclic = BTreeSet::new();
    for group in components(&edges) {
        if group.len() > 1 || edges[group[0]].contains(&group[0]) {
            for i in group {
                if values[i].1 {
                    cyclic.insert(values[i].0);
                }
            }
        }
    }
    Ok(cyclic)
}

#[cfg(test)]
mod shared_definition_tests {
    use super::*;
    use std::rc::Rc;
    #[test]
    fn shared_order_preserves_snapshots_and_does_not_append_cycle_initializers_twice() {
        let definition = Rc::new(Definition {
            symbol: SymbolId(0), javascript: "var value;".into(),
            dependencies: BTreeSet::from([SymbolId(0)]), function: false,
            registration: None, cycle_initialization: Some("value = 1;".into()),
        });
        for _ in 0..2 {
            let ordered = order_shared(vec![definition.clone()]).unwrap();
            assert_eq!(ordered[0].javascript, "var value;value = 1;");
            assert!(ordered[0].cycle_initialization.is_none());
            assert_eq!(definition.javascript, "var value;");
            assert_eq!(definition.cycle_initialization.as_deref(), Some("value = 1;"));
        }
    }
    #[test]
    fn shared_order_retains_immutable_function_definitions() {
        let definition = Rc::new(Definition {
            symbol: SymbolId(0), javascript: "function value(){return 1;}".into(),
            dependencies: BTreeSet::new(), function: true,
            registration: None, cycle_initialization: None,
        });
        let ordered = order_shared(vec![definition.clone()]).unwrap();
        assert!(Rc::ptr_eq(&ordered[0], &definition));
    }
}
