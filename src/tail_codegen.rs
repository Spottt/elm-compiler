//! Return-position statements avoid expression IIFEs in production. Direct self
//! tail calls become loops without changing curried closure capture.
use crate::{
    ast::{Expr, ExprId, PatternId, Syntax},
    codegen,
    names::{Binding, Resolved},
    pattern_codegen::Plan,
};

pub struct Function<'a> {
    pub binding: Binding,
    pub arguments: &'a [PatternId],
    pub body: ExprId,
}

fn self_call(
    ast: &Syntax<'_>,
    resolved: &Resolved,
    root: ExprId,
    function: &Function<'_>,
) -> Option<Vec<ExprId>> {
    let mut callee = root;
    let mut groups = Vec::new();
    while let Expr::Call(next, args) = &ast.expressions[callee.0 as usize].kind {
        groups.push(args);
        callee = *next;
    }
    if groups.is_empty() || resolved.expressions[callee.0 as usize] != Some(function.binding) {
        return None;
    }
    let args: Vec<_> = groups
        .into_iter()
        .rev()
        .flat_map(|args| args.iter().copied())
        .collect();
    (args.len() == function.arguments.len()).then_some(args)
}

fn has_tail_call(ast: &Syntax<'_>, resolved: &Resolved, function: &Function<'_>) -> bool {
    let mut pending = vec![function.body];
    while let Some(id) = pending.pop() {
        if self_call(ast, resolved, id, function).is_some() {
            return true;
        }
        match &ast.expressions[id.0 as usize].kind {
            Expr::If(_, yes, no) => pending.extend([*yes, *no]),
            Expr::Case(_, branches) => pending.extend(branches.iter().map(|(_, body)| *body)),
            Expr::Let(_, body) => pending.push(*body),
            _ => {}
        }
    }
    false
}

fn bindings(plan: &Plan) -> String {
    // Elm 0.19.1 uses function-scoped bindings in tail loops. This also
    // preserves its observable capture behavior for closures made in a loop.
    plan.declarations()
}

enum Task {
    Tail(ExprId),
    Text(String),
}

pub fn emit(
    ast: &Syntax<'_>,
    resolved: &Resolved,
    function: Function<'_>,
    mode: crate::kernel::Mode,
    mut expression: impl FnMut(ExprId) -> Result<String, String>,
    mut pattern: impl FnMut(PatternId, &str) -> Result<Plan, String>,
    mut definitions: impl FnMut(ExprId) -> Result<Vec<codegen::LocalDefinition>, String>,
) -> Result<Option<String>, String> {
    if function.arguments.is_empty() {
        return Ok(None);
    }
    let recursive = has_tail_call(ast, resolved, &function);
    if !recursive && mode != crate::kernel::Mode::Production {
        return Ok(None);
    }
    let mut parameters = Vec::new();
    for (i, arg) in function.arguments.iter().enumerate() {
        let name = format!("$tailInput{i}");
        let name = if !recursive
            && matches!(
                ast.patterns[arg.0 as usize].kind,
                crate::ast::Pattern::Var(_)
            ) {
            pattern(*arg, &name)?
                .bindings
                .into_iter()
                .next()
                .ok_or("missing argument binding")?
                .0
        } else {
            name
        };
        parameters.push(name);
    }
    let mut output = String::new();
    let flat = mode == crate::kernel::Mode::Production && function.arguments.len() > 1;
    if flat {
        output.push_str(&codegen::flat_function_start(&parameters));
    } else {
        for (i, parameter) in parameters.iter().enumerate() {
            output.push_str(&format!("(function({parameter}){{"));
            if i + 1 < function.arguments.len() {
                output.push_str("return ");
            }
        }
    }
    // Copy even outer curried arguments inside the final closure, so invoking
    // the same partially applied function twice starts with fresh loop state.
    if recursive {
        for i in 0..function.arguments.len() {
            output.push_str(&format!("let $tailState{i}=$tailInput{i};"));
        }
        output.push_str("$tailLoop:while(true){");
    }
    for (i, arg) in function.arguments.iter().enumerate() {
        if !recursive
            && matches!(
                ast.patterns[arg.0 as usize].kind,
                crate::ast::Pattern::Var(_)
            )
        {
            continue;
        }
        let name = if recursive {
            format!("$tailState{i}")
        } else {
            format!("$tailInput{i}")
        };
        output.push_str(&bindings(&pattern(*arg, &name)?));
    }
    let mut pending = vec![Task::Tail(function.body)];
    while let Some(task) = pending.pop() {
        let id = match task {
            Task::Text(text) => {
                output.push_str(&text);
                continue;
            }
            Task::Tail(id) => id,
        };
        if let Some(args) = self_call(ast, resolved, id, &function) {
            for (i, arg) in args.iter().enumerate() {
                output.push_str(&format!("let $tailNext{i}={};", expression(*arg)?));
            }
            for i in 0..args.len() {
                output.push_str(&format!("$tailState{i}=$tailNext{i};"));
            }
            output.push_str("continue $tailLoop;");
            continue;
        }
        let mut next = Vec::new();
        match &ast.expressions[id.0 as usize].kind {
            Expr::If(condition, yes, no) => {
                output.push_str(&format!("if({}){{", expression(*condition)?));
                next.extend([
                    Task::Tail(*yes),
                    Task::Text("}else{".into()),
                    Task::Tail(*no),
                    Task::Text("}".into()),
                ]);
            }
            Expr::Case(subject, branches) => {
                let name = format!("$tailCase{}", id.0);
                output.push_str(&format!("let {name}={};", expression(*subject)?));
                let plans = branches
                    .iter()
                    .map(|(arg, _)| pattern(*arg, &name))
                    .collect::<Result<Vec<_>, _>>()?;
                let switch = mode == crate::kernel::Mode::Production
                    && plans.last().is_some_and(|plan| plan.condition == "true");
                let labels = if switch {
                    crate::pattern_codegen::switch_labels(&plans)
                } else {
                    None
                };
                if mode == crate::kernel::Mode::Production && labels.is_none() {
                    for step in crate::pattern_codegen::decision_steps(&plans) {
                        use crate::pattern_codegen::DecisionStep;
                        match step {
                            DecisionStep::Open(condition) => {
                                next.push(Task::Text(format!("if({condition}){{")))
                            }
                            DecisionStep::Close => next.push(Task::Text("}".into())),
                            DecisionStep::Body(i) => {
                                next.push(Task::Text(format!("{{{}", bindings(&plans[i]))));
                                next.push(Task::Tail(branches[i].1));
                                next.push(Task::Text("}".into()));
                            }
                        }
                    }
                    if !plans.iter().any(|p| p.condition == "true") {
                        next.push(Task::Text(
                            "throw new Error('unreachable Elm pattern');".into(),
                        ));
                    }
                } else {
                    if let Some((subject, _)) = &labels {
                        next.push(Task::Text(format!("switch({subject}){{")));
                    }
                    let mut exhaustive = false;
                    for (index, ((_, body), plan)) in branches.iter().zip(plans).enumerate() {
                        exhaustive |= plan.condition == "true";
                        let start = if let Some((_, labels)) = &labels {
                            if index < labels.len() {
                                format!("case {}:{{{}", labels[index], bindings(&plan))
                            } else {
                                format!("default:{{{}", bindings(&plan))
                            }
                        } else {
                            format!("if({}){{{}", plan.condition, bindings(&plan))
                        };
                        next.push(Task::Text(start));
                        next.push(Task::Tail(*body));
                        next.push(Task::Text("}".into()));
                    }
                    if labels.is_some() {
                        next.push(Task::Text("}".into()));
                    }
                    if !exhaustive {
                        next.push(Task::Text(
                            "throw new Error('unreachable Elm pattern');".into(),
                        ));
                    }
                }
            }
            Expr::Let(_, body) => {
                output.push('{');
                for (index, definition) in definitions(id)?.into_iter().enumerate() {
                    let name = definition
                        .name
                        .unwrap_or_else(|| format!("$tailDestruct{}_{}", id.0, index));
                    output.push_str(&format!("var {name}="));
                    if let Some(javascript) = definition.javascript {
                        output.push_str(&javascript);
                        output.push(';');
                        continue;
                    }
                    for (i, arg) in definition.arguments.iter().enumerate() {
                        let parameter = format!("$tailDef{}_{}_{}", id.0, index, i);
                        output.push_str(&format!(
                            "(function({parameter}){{{}return ",
                            pattern(*arg, &parameter)?.declarations()
                        ));
                    }
                    output.push_str(&expression(definition.body)?);
                    for _ in definition.arguments {
                        output.push_str(";})");
                    }
                    output.push(';');
                    if let Some(arg) = definition.destructure {
                        output.push_str(&bindings(&pattern(arg, &name)?));
                    }
                }
                next.extend([Task::Tail(*body), Task::Text("}".into())]);
            }
            _ => output.push_str(&format!("return {};", expression(id)?)),
        }
        pending.extend(next.into_iter().rev());
    }
    if recursive {
        output.push('}');
    }
    for _ in 0..if flat { 1 } else { function.arguments.len() } {
        output.push_str(";})");
    }
    Ok(Some(output))
}
