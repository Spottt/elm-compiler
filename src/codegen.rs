//! Streaming expression emission. The caller supplies resolved names and the
//! shared field mapping; raw source identifiers are never used as JS bindings.
use crate::{
    ast::{Expr, ExprId, PatternId, Syntax},
    kernel::Mode,
    literal,
};
#[derive(Clone)]
pub struct Reference {
    pub name: String,
    /// Whether this identity belongs to the kernel (source calls remain curried).
    pub kernel: bool,
}
impl Reference {
    fn direct_binary(&self, mode: Mode) -> Option<&'static str> {
        if let Some(operator) = self.short_circuit() {
            return Some(operator);
        }
        if mode != Mode::Production {
            return None;
        }
        // These are exactly the JavaScript operations used by Elm's numeric
        // kernel. Integer division, comparison and equality need other rules.
        match self.name.as_str() {
            "$elm$core$Basics$add" => Some(" + "),
            "$elm$core$Basics$sub" => Some(" - "),
            "$elm$core$Basics$mul" => Some(" * "),
            "$elm$core$Basics$fdiv" => Some(" / "),
            _ => None,
        }
    }
    fn short_circuit(&self) -> Option<&'static str> {
        // These are canonical globals, not the source spelling of an operator.
        // In particular, Bitwise.and and local aliases remain ordinary calls.
        match self.name.as_str() {
            "$elm$core$Basics$and" => Some("&&"),
            "$elm$core$Basics$or" => Some("||"),
            _ => None,
        }
    }
}
enum Task {
    Expr(ExprId),
    Apply(ExprId, Vec<ExprId>),
    Text(String),
}
fn text(value: impl Into<String>) -> Task {
    Task::Text(value.into())
}
/// A shallow multi-argument body with Elm-compatible partial application.
pub fn flat_function_start(arguments: &[String]) -> String {
    let arity = arguments.len();
    let wrapper = if (2..=9).contains(&arity) {
        format!("F{arity}(")
    } else {
        format!("_Rust_curry({arity},")
    };
    format!("{wrapper}function({}){{", arguments.join(","))
}
// Preserve left-to-right evaluation; kernel equality/comparison remain structural.
fn binary_tasks(
    ast: &Syntax<'_>,
    function: &Reference,
    left: ExprId,
    right: ExprId,
    mode: Mode,
    reference: &mut impl FnMut(ExprId) -> Result<Reference, String>,
) -> Option<Vec<Task>> {
    if let Some(operator) = function.direct_binary(mode) {
        return Some(vec![
            text("("),
            Task::Expr(left),
            text(operator),
            Task::Expr(right),
            text(")"),
        ]);
    }
    if mode != Mode::Production {
        return None;
    }
    if function.name == "$elm$core$Basics$apL" {
        return Some(vec![Task::Apply(left, vec![right])]);
    }
    if function.name == "$elm$core$Basics$apR" {
        return Some(vec![Task::Apply(right, vec![left])]);
    }
    if function.name == "$elm$core$Basics$append" {
        let mut pending = vec![right, left];
        let mut leaves = Vec::new();
        while let Some(id) = pending.pop() {
            let pair = match &ast.expressions[id.0 as usize].kind {
                Expr::Binary(_, a, b)
                    if reference(id)
                        .ok()
                        .is_some_and(|r| r.name == "$elm$core$Basics$append") =>
                {
                    Some((*a, *b))
                }
                Expr::Call(f, args)
                    if args.len() == 2
                        && matches!(
                            ast.expressions[f.0 as usize].kind,
                            Expr::Var(_) | Expr::Operator(_)
                        )
                        && reference(*f)
                            .ok()
                            .is_some_and(|r| r.name == "$elm$core$Basics$append") =>
                {
                    Some((args[0], args[1]))
                }
                _ => None,
            };
            if let Some((a, b)) = pair {
                pending.extend([b, a]);
            } else {
                leaves.push(id);
            }
        }
        let is_string = leaves.iter().any(|id| {
            matches!(
                ast.expressions[id.0 as usize].kind,
                Expr::Literal(crate::lexer::Kind::String, _)
            )
        });
        let mut tasks = Vec::new();
        if is_string {
            tasks.push(text("("));
            for (i, id) in leaves.into_iter().enumerate() {
                if i > 0 {
                    tasks.push(text("+"));
                }
                tasks.push(Task::Expr(id));
            }
            tasks.push(text(")"));
        } else {
            // Append is associative for immutable strings/lists. Emit the whole
            // chain once, preserving operand order and avoiding quadratic rescans.
            let count = leaves.len();
            for (i, id) in leaves.into_iter().enumerate() {
                if i + 1 < count {
                    tasks.push(text("_Utils_ap("));
                }
                tasks.push(Task::Expr(id));
                if i + 1 < count {
                    tasks.push(text(","));
                }
            }
            tasks.push(text(")".repeat(count - 1)));
        }
        return Some(tasks);
    }

    let (prefix, middle, suffix) = match function.name.as_str() {
        "$elm$core$Basics$idiv" => ("((", ")/(", ")|0)"),
        "$elm$core$Basics$eq" => ("_Utils_eq(", ",", ")"),
        "$elm$core$Basics$neq" => ("(!_Utils_eq(", ",", "))"),
        "$elm$core$Basics$lt" => ("(_Utils_cmp(", ",", ")<0)"),
        "$elm$core$Basics$gt" => ("(_Utils_cmp(", ",", ")>0)"),
        "$elm$core$Basics$le" => ("(_Utils_cmp(", ",", ")<1)"),
        "$elm$core$Basics$ge" => ("(_Utils_cmp(", ",", ")>-1)"),
        "$elm$core$Basics$append" => ("_Utils_ap(", ",", ")"),
        "$elm$core$Basics$xor" => ("(", "!==", ")"),
        _ => return None,
    };
    Some(vec![
        text(prefix),
        Task::Expr(left),
        text(middle),
        Task::Expr(right),
        text(suffix),
    ])
}
fn separated(items: &[ExprId], tasks: &mut Vec<Task>) {
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            tasks.push(text(","));
        }
        tasks.push(Task::Expr(*item));
    }
}
pub fn expression(
    ast: &Syntax<'_>,
    root: ExprId,
    mode: Mode,
    reference: impl FnMut(ExprId) -> Result<Reference, String>,
    field: impl FnMut(&str) -> Result<String, String>,
) -> Result<String, String> {
    expression_with_patterns(ast, root, mode, reference, field, |_, _| {
        Err("pattern naming is required".into())
    })
}
pub fn expression_with_patterns(
    ast: &Syntax<'_>,
    root: ExprId,
    mode: Mode,
    reference: impl FnMut(ExprId) -> Result<Reference, String>,
    field: impl FnMut(&str) -> Result<String, String>,
    pattern: impl FnMut(PatternId, &str) -> Result<crate::pattern_codegen::Plan, String>,
) -> Result<String, String> {
    expression_with_definitions(ast, root, mode, reference, field, pattern, |_| {
        Err("definition naming is required".into())
    })
}
pub struct LocalDefinition {
    pub javascript: Option<String>,
    pub name: Option<String>,
    pub arguments: Vec<PatternId>,
    pub destructure: Option<PatternId>,
    pub body: ExprId,
}
pub fn expression_with_definitions(
    ast: &Syntax<'_>,
    root: ExprId,
    mode: Mode,
    mut reference: impl FnMut(ExprId) -> Result<Reference, String>,
    mut field: impl FnMut(&str) -> Result<String, String>,
    mut pattern: impl FnMut(PatternId, &str) -> Result<crate::pattern_codegen::Plan, String>,
    mut definitions: impl FnMut(ExprId) -> Result<Vec<LocalDefinition>, String>,
) -> Result<String, String> {
    let mut output = String::new();
    let mut pending = vec![Task::Expr(root)];
    while let Some(task) = pending.pop() {
        if let Task::Apply(mut function, mut args) = task {
            // Flatten nested full applications, as Elm's optimizer does for pipes.
            while let Expr::Call(inner, earlier) = &ast.expressions[function.0 as usize].kind {
                let mut joined = earlier.clone();
                joined.extend(args);
                args = joined;
                function = *inner;
            }
            let function = &function;
            let args = &args;
            let mut next = Vec::new();

            if args.len() == 2
                && matches!(
                    ast.expressions[function.0 as usize].kind,
                    Expr::Var(_) | Expr::Operator(_)
                )
                && let Some(tasks) = binary_tasks(
                    ast,
                    &reference(*function)?,
                    args[0],
                    args[1],
                    mode,
                    &mut reference,
                )
            {
                pending.extend(tasks.into_iter().rev());
                continue;
            }
            if mode == Mode::Production
                && args.len() == 1
                && matches!(
                    ast.expressions[function.0 as usize].kind,
                    Expr::Var(_) | Expr::Operator(_)
                )
            {
                let reference = reference(*function)?;
                let operation = match reference.name.as_str() {
                    "$elm$core$Basics$not" => Some(("(!", ")")),
                    "$elm$core$Basics$negate" => Some(("(-", ")")),
                    "$elm$core$Basics$toFloat" => Some(("(", ")")),
                    "$elm$core$Basics$truncate" => Some(("(", "|0)")),
                    _ => None,
                };
                if let Some((prefix, suffix)) = operation {
                    pending.extend([text(suffix), Task::Expr(args[0]), text(prefix)]);
                    continue;
                }
            }
            // Source-level kernel calls obey Elm's curried ABI too. Direct
            // JS calls are reserved for compiler-generated runtime helpers.
            if (2..=9).contains(&args.len()) {
                next.extend([
                    text(format!("A{}(", args.len())),
                    Task::Expr(*function),
                    text(","),
                ]);
                separated(args, &mut next);
                next.push(text(")"));
            } else {
                next.extend([text("("), Task::Expr(*function), text(")")]);
                for arg in args {
                    next.extend([text("("), Task::Expr(*arg), text(")")]);
                }
            }
            pending.extend(next.into_iter().rev());
            continue;
        }
        let Task::Expr(id) = task else {
            if let Task::Text(value) = task {
                output.push_str(&value);
            }
            continue;
        };
        let mut next = Vec::new();
        match &ast.expressions[id.0 as usize].kind {
            Expr::Literal(crate::lexer::Kind::Shader, raw) => {
                next.push(text(crate::shader::Shader::parse(raw)?.emit(&mut field)?))
            }
            Expr::Literal(kind, raw) => next.push(text(literal::emit(*kind, raw, mode)?)),
            Expr::Var(_) | Expr::Operator(_) => next.push(text(reference(id)?.name)),
            Expr::Unit => next.push(text(if !matches!(mode, Mode::Production) {
                "_Utils_Tuple0"
            } else {
                "0"
            })),
            Expr::Negate(value) => next.extend([text("(-"), Task::Expr(*value), text(")")]),
            Expr::List(items) if items.is_empty() => next.push(text("_List_Nil")),
            Expr::List(items) => {
                next.push(text("_List_fromArray(["));
                separated(items, &mut next);
                next.push(text("])"));
            }
            Expr::Tuple(items) => {
                if !(2..=3).contains(&items.len()) {
                    return Err("invalid tuple arity".into());
                }
                next.push(text(format!("_Utils_Tuple{}(", items.len())));
                separated(items, &mut next);
                next.push(text(")"));
            }
            Expr::If(condition, yes, no) => next.extend([
                text("("),
                Task::Expr(*condition),
                text("?"),
                Task::Expr(*yes),
                text(":"),
                Task::Expr(*no),
                text(")"),
            ]),
            Expr::Access(record, name) => next.extend([
                text("("),
                Task::Expr(*record),
                text(format!(
                    ")[{}]",
                    serde_json::to_string(&field(name)?).unwrap()
                )),
            ]),
            Expr::Accessor(name) => next.push(text(format!(
                "(function($record){{return $record[{}];}})",
                serde_json::to_string(&field(name)?).unwrap()
            ))),
            Expr::Record { base, fields } => {
                // Elm canonicalizes records into a Map before JS generation.
                // Sort source field names, before production field renaming,
                // so evaluation order and Debug.toString match the official compiler.
                let mut fields: Vec<_> = fields.iter().collect();
                fields.sort_unstable_by_key(|(name, _)| *name);
                if base.is_some() {
                    next.push(text(format!("_Utils_update({},", reference(id)?.name)));
                }
                next.push(text("({"));
                for (index, (name, value)) in fields.into_iter().enumerate() {
                    if index > 0 {
                        next.push(text(","));
                    }
                    next.push(text(format!(
                        "{}:",
                        serde_json::to_string(&field(name)?).unwrap()
                    )));
                    next.push(Task::Expr(*value));
                }
                next.push(text("})"));
                if base.is_some() {
                    next.push(text(")"));
                }
            }
            Expr::Call(function, args) => next.push(Task::Apply(*function, args.clone())),
            Expr::Binary(_, left, right) => {
                // Operator resolution supplies the actual global function.
                let function = reference(id)?;
                if let Some(tasks) =
                    binary_tasks(ast, &function, *left, *right, mode, &mut reference)
                {
                    pending.extend(tasks.into_iter().rev());
                    continue;
                }
                next.extend([
                    text(format!("A2({},", function.name)),
                    Task::Expr(*left),
                    text(","),
                    Task::Expr(*right),
                    text(")"),
                ]);
            }
            Expr::Binops(..) => return Err("operators must be resolved before generation".into()),
            Expr::Lambda(arguments, body) => {
                if mode == Mode::Production && arguments.len() > 1 {
                    let names = (0..arguments.len())
                        .map(|i| format!("$arg{}_{i}", id.0))
                        .collect::<Vec<_>>();
                    next.push(text(flat_function_start(&names)));
                    for (arg, name) in arguments.iter().zip(&names) {
                        next.push(text(pattern(*arg, name)?.declarations()));
                    }
                    next.push(text("return "));
                    next.push(Task::Expr(*body));
                    next.push(text(";})"));
                    pending.extend(next.into_iter().rev());
                    continue;
                }
                for (index, arg) in arguments.iter().enumerate() {
                    let name = format!("$arg{}_{}", id.0, index);
                    let plan = pattern(*arg, &name)?;
                    next.push(text(format!(
                        "(function({name}){{{}return ",
                        plan.declarations()
                    )));
                }
                next.push(Task::Expr(*body));
                for _ in arguments {
                    next.push(text(";})"));
                }
            }
            Expr::Case(subject, branches) => {
                // A stable variable needs no evaluation wrapper. Keep larger or
                // nested decisions in the shared-test tree, and retain wrappers
                // whenever a branch introduces bindings or the subject can run code.
                if mode == Mode::Production
                    && branches.len() <= 3
                    && matches!(ast.expressions[subject.0 as usize].kind, Expr::Var(_))
                {
                    let subject_name = reference(*subject)?.name;
                    if !subject_name.contains('(') {
                        let plans = branches
                            .iter()
                            .map(|(arg, _)| pattern(*arg, &subject_name))
                            .collect::<Result<Vec<_>, _>>()?;
                        if plans.last().is_some_and(|p| p.condition == "true")
                            && plans
                                .iter()
                                .all(|p| p.bindings.is_empty() && p.tests.len() <= 1)
                        {
                            for (i, (_, body)) in branches.iter().enumerate() {
                                if i + 1 < branches.len() {
                                    next.push(text(format!("({}?", plans[i].condition)));
                                }
                                next.push(Task::Expr(*body));
                                if i + 1 < branches.len() {
                                    next.push(text(":"));
                                }
                            }
                            next.push(text(")".repeat(branches.len() - 1)));
                            pending.extend(next.into_iter().rev());
                            continue;
                        }
                    }
                }
                let name = format!("$case{}", id.0);
                next.push(text(format!("(function({name}){{")));
                let plans = branches
                    .iter()
                    .map(|(arg, _)| pattern(*arg, &name))
                    .collect::<Result<Vec<_>, _>>()?;
                let switch = mode == Mode::Production
                    && plans.last().is_some_and(|plan| plan.condition == "true");
                let labels = if switch {
                    crate::pattern_codegen::switch_labels(&plans)
                } else {
                    None
                };
                if mode == Mode::Production && labels.is_none() {
                    for step in crate::pattern_codegen::decision_steps(&plans) {
                        use crate::pattern_codegen::DecisionStep;
                        match step {
                            DecisionStep::Open(condition) => {
                                next.push(text(format!("if({condition}){{")))
                            }
                            DecisionStep::Close => next.push(text("}")),
                            DecisionStep::Body(i) => {
                                next.push(text(format!("{{{}return ", plans[i].declarations())));
                                next.push(Task::Expr(branches[i].1));
                                next.push(text(";}"));
                            }
                        }
                    }
                    if !plans.iter().any(|p| p.condition == "true") {
                        next.push(text("throw new Error('unreachable Elm pattern');"));
                    }
                } else {
                    if let Some((subject, _)) = &labels {
                        next.push(text(format!("switch({subject}){{")));
                    }
                    let mut exhaustive = false;
                    for (index, ((_, body), plan)) in branches.iter().zip(plans).enumerate() {
                        exhaustive |= plan.condition == "true";
                        if let Some((_, labels)) = &labels {
                            next.push(text(if index < labels.len() {
                                format!("case {}:{{{}return ", labels[index], plan.declarations())
                            } else {
                                format!("default:{{{}return ", plan.declarations())
                            }));
                        } else {
                            next.push(text(format!(
                                "if({}){{{}return ",
                                plan.condition,
                                plan.declarations()
                            )));
                        }
                        next.push(Task::Expr(*body));
                        next.push(text(";}"));
                    }
                    if labels.is_some() {
                        next.push(text("}"));
                    }
                    if !exhaustive {
                        next.push(text("throw new Error('unreachable Elm pattern');"));
                    }
                }
                next.push(text("})("));
                next.push(Task::Expr(*subject));
                next.push(text(")"));
            }
            Expr::Let(_, body) => {
                next.push(text("(function(){"));
                for (index, definition) in definitions(id)?.into_iter().enumerate() {
                    let name = definition
                        .name
                        .unwrap_or_else(|| format!("$destruct{}_{}", id.0, index));
                    next.push(text(format!("var {name}=")));
                    if let Some(javascript) = definition.javascript {
                        next.push(text(javascript));
                        next.push(text(";"));
                        continue;
                    }
                    for (arg_index, arg) in definition.arguments.iter().enumerate() {
                        let argument = format!("$defarg{}_{}_{}", id.0, index, arg_index);
                        let plan = pattern(*arg, &argument)?;
                        next.push(text(format!(
                            "(function({argument}){{{}return ",
                            plan.declarations()
                        )));
                    }
                    next.push(Task::Expr(definition.body));
                    for _ in definition.arguments {
                        next.push(text(";})"));
                    }
                    next.push(text(";"));
                    if let Some(arg) = definition.destructure {
                        next.push(text(pattern(arg, &name)?.declarations()));
                    }
                }
                next.push(text("return "));
                next.push(Task::Expr(*body));
                next.push(text(";})()"));
            }
        }
        pending.extend(next.into_iter().rev());
    }
    Ok(output)
}

/// Order local definitions by resolved dependencies, independently of source
/// order. Functions within a recursive group are closures before any call runs.
pub fn local_definitions(
    ast: &Syntax<'_>,
    resolved: &crate::names::Resolved,
    root: ExprId,
    mut local: impl FnMut(crate::names::LocalId) -> String,
) -> Result<Vec<LocalDefinition>, String> {
    use crate::{
        ast::Declaration,
        infer::{components, expr_children, pattern_children},
        names::Binding,
    };
    use std::collections::{BTreeMap, BTreeSet};
    let Expr::Let(declarations, _) = &ast.expressions[root.0 as usize].kind else {
        return Err("expected let".into());
    };
    let mut definitions = Vec::new();
    let mut owners = BTreeMap::new();
    for declaration in declarations {
        let index = definitions.len();
        match declaration {
            Declaration::Value {
                arguments, body, ..
            } => {
                let id = *resolved
                    .definitions
                    .get(body)
                    .ok_or("missing local definition identity")?;
                owners.insert(id, index);
                definitions.push(LocalDefinition {
                    javascript: None,
                    name: Some(local(id)),
                    arguments: arguments.clone(),
                    destructure: None,
                    body: *body,
                });
            }
            Declaration::Destruct { pattern, body } => {
                let mut pending = vec![*pattern];
                while let Some(id) = pending.pop() {
                    if let Some(bindings) = resolved.pattern_bindings.get(&id) {
                        for (_, binding) in bindings {
                            owners.insert(*binding, index);
                        }
                    }
                    pending.extend(pattern_children(&ast.patterns[id.0 as usize].kind));
                }
                definitions.push(LocalDefinition {
                    javascript: None,
                    name: None,
                    arguments: vec![],
                    destructure: Some(*pattern),
                    body: *body,
                });
            }
            Declaration::Annotation { .. } => {}
            _ => return Err("unexpected declaration in let".into()),
        }
    }
    let mut edges = vec![BTreeSet::new(); definitions.len()];
    for (index, definition) in definitions.iter().enumerate() {
        let mut pending = vec![definition.body];
        while let Some(id) = pending.pop() {
            if let Some(Binding::Local(binding)) = resolved.expressions[id.0 as usize]
                && let Some(owner) = owners.get(&binding)
            {
                edges[index].insert(*owner);
            }
            pending.extend(expr_children(&ast.expressions[id.0 as usize].kind));
        }
    }
    let groups = components(&edges);
    for group in &groups {
        let cyclic = group.len() > 1 || edges[group[0]].contains(&group[0]);
        if cyclic && group.iter().any(|i| definitions[*i].arguments.is_empty()) {
            return Err(
                "cyclic local value: recursive let groups must contain only functions".into(),
            );
        }
    }
    let mut definitions: Vec<_> = definitions.into_iter().map(Some).collect();
    Ok(groups
        .into_iter()
        .flatten()
        .map(|i| definitions[i].take().unwrap())
        .collect())
}
