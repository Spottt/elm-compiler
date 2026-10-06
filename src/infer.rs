//! Hindley-Milner inference over resolved identities. Definitions are processed
//! by dependency components; recursive unannotated references are monomorphic.
use crate::{
    ast::{Declaration, Expr, ExprId, Pattern, PatternId, Type, TypeId},
    lexer::Kind,
    names::{Binding, Space, SymbolId, SymbolKind},
    types::{self, Catalog, SourceTypes},
    unify::{Constraint, Engine, Scheme, Term, Ty},
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub struct Inferred {
    pub expressions: Vec<Option<Ty>>,
}
// Opt-in developer trace; owning names keeps the guard independent of the
// mutable inference engine. Drop also reports groups that return an error.
struct InferenceGroupProfile { module: String, names: Vec<String>, started: std::time::Instant }
impl Drop for InferenceGroupProfile {
    fn drop(&mut self) {
        eprintln!("ELM_INFERENCE_PROFILE {}", serde_json::json!({"module":self.module,"definitions":self.names,"ms":self.started.elapsed().as_secs_f64()*1000.0}));
    }
}
struct InfiniteDiagnostic {
    name: String,
    span: crate::ast::Span,
    root: Ty,
    cycles: Vec<(Ty, Ty)>,
    fallback: String,
}
struct Infer<'a, 's> {
    source: SourceTypes<'a, 's>,
    module: &'a str,
    profile_groups: bool,
    catalog: &'a Catalog,
    engine: &'a mut Engine,
    globals: &'a mut BTreeMap<SymbolId, Scheme>,
    locals: Vec<Option<Scheme>>,
    patterns: Vec<Option<Ty>>,
    expressions: Vec<Option<Ty>>,
    scoped_types: HashMap<&'s str, Ty>,
    annotation_contexts: BTreeMap<ExprId, (crate::annotation_diagnostic::Context, Ty)>,
    annotation_errors: Vec<String>,
    infinite_diagnostics: Vec<InfiniteDiagnostic>,
    call_results: BTreeMap<ExprId, Ty>,
    case_patterns: BTreeMap<ExprId, (PatternId, Ty, ExprId, usize)>,
    depth: usize,
    // Intermediate generic nodes in an unannotated recursive group, paired
    // with the enclosing scope that must remain monomorphic.
    recursive_generics: Vec<(Ty, u32)>,
}
pub fn check<'a, 's>(
    source: SourceTypes<'a, 's>,
    module: &'a str,
    catalog: &'a Catalog,
    engine: &'a mut Engine,
    globals: &'a mut BTreeMap<SymbolId, Scheme>,
) -> Result<Inferred, String> {
    let mut infer = Infer {
        source,
        module,
        profile_groups: std::env::var("PLANEXPO_ELM_PROFILE_INFERENCE").is_ok_and(|value| value == "1" || value == module),
        catalog,
        engine,
        globals,
        locals: vec![None; source.resolved.locals.len()],
        patterns: vec![None; source.ast.patterns.len()],
        expressions: vec![None; source.ast.expressions.len()],
        scoped_types: HashMap::new(),
        annotation_contexts: BTreeMap::new(),
        annotation_errors: Vec::new(),
        infinite_diagnostics: Vec::new(),
        call_results: BTreeMap::new(),
        case_patterns: BTreeMap::new(),
        depth: 0,
        recursive_generics: Vec::new(),
    };
    if let crate::module::Effects::Manager {
        command,
        subscription,
    } = &source.ast.header.effects
    {
        for (name, local_type, target_module, target_name) in [
            ("command", command, "elm/core:Platform.Cmd", "Cmd"),
            ("subscription", subscription, "elm/core:Platform.Sub", "Sub"),
        ] {
            if let Some(local_type) = local_type {
                let id = source
                    .symbols
                    .lookup(module, name, Space::Value)
                    .ok_or("missing generated effect function")?;
                let msg = infer.fresh(1);
                let argument = infer.builtin(module, local_type, vec![msg])?;
                let result = infer.builtin(target_module, target_name, vec![msg])?;
                let root = infer.engine.term(Term::Function(argument, result));
                infer.globals.insert(id, infer.engine.generalize(root, 0));
            }
        }
    }
    let mut port_errors = Vec::new();
    for declaration in &source.ast.declarations {
        if let Declaration::Port { name, ty } = declaration {
            let id = source
                .symbols
                .lookup(module, name, Space::Value)
                .ok_or("missing port symbol")?;
            let scheme = catalog.annotation(
                source.ast,
                source.resolved,
                source.symbols,
                infer.engine,
                *ty,
            )?;
            if let Err(error) = crate::entry::check_port(infer.engine, source.symbols, scheme.root) {
                infer.engine.track_display_names();
                let root = catalog.annotation(
                    source.ast, source.resolved, source.symbols, infer.engine, *ty,
                ).map(|scheme| scheme.root).unwrap_or(scheme.root);
                port_errors.push(crate::port_diagnostic::report(
                    source.ast, module.split(':').next().unwrap_or(module), name,
                    infer.engine, source.symbols, root,
                ).unwrap_or_else(|| format!("port {name}: {error}")));
                continue;
            }
            infer.globals.insert(id, scheme);
        }
    }
    if !port_errors.is_empty() {
        return Err(crate::annotation_diagnostic::combine(source.ast, &port_errors));
    }
    let outcome = infer.group(&source.ast.declarations, true, 0);
    if let Some(report) = infer.infinite_report() {
        infer.annotation_errors.push(report);
    }
    outcome?;
    let mut seen_infinite = BTreeSet::new();
    infer.annotation_errors.retain_mut(|error| {
        let Some(index) = error
            .strip_prefix("INFINITE_PENDING:")
            .and_then(|index| index.parse::<usize>().ok())
        else {
            return true;
        };
        if !seen_infinite.insert(index) {
            return false;
        }
        let pending = &infer.infinite_diagnostics[index];
        *error = crate::infinite_type_diagnostic::capture(
            source.ast,
            source.symbols,
            infer.engine,
            &pending.cycles,
            pending.root,
            &pending.name,
            pending.span,
        )
        .unwrap_or_else(|| pending.fallback.clone());
        true
    });
    if !infer.annotation_errors.is_empty() {
        return Err(crate::annotation_diagnostic::combine(
            source.ast,
            &infer.annotation_errors,
        ));
    }

    crate::effects::check(
        &source.ast.header.effects,
        module,
        source.symbols,
        infer.engine,
        infer.globals,
    )?;
    Ok(Inferred {
        expressions: infer.expressions,
    })
}
pub(crate) fn expr_children(expr: &Expr<'_>) -> Vec<ExprId> {
    match expr {
        Expr::List(xs) | Expr::Tuple(xs) => xs.clone(),
        Expr::Negate(x) | Expr::Access(x, _) | Expr::Lambda(_, x) => vec![*x],
        Expr::Call(f, args) => std::iter::once(*f).chain(args.iter().copied()).collect(),
        Expr::Binary(_, a, b) => vec![*a, *b],
        Expr::Binops(a, rest) => std::iter::once(*a)
            .chain(rest.iter().map(|(_, b)| *b))
            .collect(),
        Expr::If(a, b, c) => vec![*a, *b, *c],
        Expr::Let(decls, body) => decls
            .iter()
            .filter_map(|d| match d {
                Declaration::Value { body, .. } | Declaration::Destruct { body, .. } => Some(*body),
                _ => None,
            })
            .chain(std::iter::once(*body))
            .collect(),
        Expr::Case(subject, branches) => std::iter::once(*subject)
            .chain(branches.iter().map(|(_, b)| *b))
            .collect(),
        Expr::Record { fields, .. } => fields.iter().map(|(_, e)| *e).collect(),
        _ => vec![],
    }
}
pub(crate) fn pattern_children(pattern: &Pattern<'_>) -> Vec<PatternId> {
    match pattern {
        Pattern::Constructor(_, xs) | Pattern::Tuple(xs) | Pattern::List(xs) => xs.clone(),
        Pattern::Cons(a, b) => vec![*a, *b],
        Pattern::Alias(p, _) => vec![*p],
        _ => vec![],
    }
}
// Iterative Kosaraju. Edges point from each definition to the definitions it
// uses; reversing the resulting components puts dependencies first.
pub(crate) fn components(edges: &[BTreeSet<usize>]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; edges.len()];
    let mut order = Vec::new();
    for start in 0..edges.len() {
        let mut pending = vec![(start, false)];
        while let Some((id, finish)) = pending.pop() {
            if finish {
                order.push(id);
                continue;
            }
            if seen[id] {
                continue;
            }
            seen[id] = true;
            pending.push((id, true));
            pending.extend(edges[id].iter().map(|x| (*x, false)));
        }
    }
    let mut reverse = vec![Vec::new(); edges.len()];
    for (id, neighbors) in edges.iter().enumerate() {
        for &n in neighbors {
            reverse[n].push(id);
        }
    }
    seen.fill(false);
    let mut groups = Vec::new();
    for start in order.into_iter().rev() {
        if seen[start] {
            continue;
        }
        let mut group = Vec::new();
        let mut pending = vec![start];
        while let Some(id) = pending.pop() {
            if seen[id] {
                continue;
            }
            seen[id] = true;
            group.push(id);
            pending.extend(&reverse[id]);
        }
        groups.push(group);
    }
    groups.reverse();
    groups
}
impl<'a, 's> Infer<'a, 's> {
    fn fresh(&mut self, level: u32) -> Ty {
        self.engine.variable(level, Constraint::Any)
    }
    fn mono(root: Ty) -> Scheme {
        Scheme {
            root,
            quantified: BTreeSet::new(),
        }
    }
    fn put(&mut self, binding: Binding, scheme: Scheme) {
        match binding {
            Binding::Global(id) => {
                self.globals.insert(id, scheme);
            }
            Binding::Local(id) => self.locals[id.0 as usize] = Some(scheme),
        }
    }
    fn binding(&mut self, binding: Binding, level: u32) -> Result<Ty, String> {
        let scheme = match binding {
            Binding::Local(id) => self.locals[id.0 as usize]
                .as_ref()
                .ok_or("local type not ready")?,
            Binding::Global(id) => {
                if matches!(self.source.symbols.get(id).kind, SymbolKind::Kernel) {
                    return Ok(self.fresh(level));
                }
                self.catalog
                    .constructors
                    .get(&id)
                    .or_else(|| self.globals.get(&id))
                    .ok_or_else(|| {
                        let s = self.source.symbols.get(id);
                        format!("type not ready: {}.{}", s.module, s.name)
                    })?
            }
        };
        if self.engine.tracks_display_names()
            && matches!(binding, Binding::Global(id) if self.catalog.constructors.contains_key(&id))
        {
            Ok(self
                .engine
                .instantiate_diagnostic_constructor(scheme, level))
        } else if scheme.quantified.is_empty() && !self.recursive_generics.is_empty() {
            Ok(self
                .engine
                .instantiate_partial(scheme.root, &self.recursive_generics, level))
        } else {
            Ok(self.engine.instantiate(scheme, level))
        }
    }
    fn reference(&mut self, id: ExprId, level: u32) -> Result<Ty, String> {
        self.binding(
            self.source.resolved.expressions[id.0 as usize].ok_or("unresolved expression")?,
            level,
        )
    }
    fn builtin(&mut self, module: &str, name: &str, args: Vec<Ty>) -> Result<Ty, String> {
        let id = self
            .source
            .symbols
            .lookup(module, name, Space::Type)
            .ok_or_else(|| format!("missing builtin type {module}.{name}"))?;
        Ok(self.engine.term(Term::Named(id, args)))
    }
    fn literal(&mut self, kind: Kind, text: &str, pattern: bool, level: u32) -> Result<Ty, String> {
        match kind {
            Kind::Number if pattern => self.builtin("elm/core:Basics", "Int", vec![]),
            Kind::Number if !text.starts_with("0x") && text.contains(['.', 'e', 'E']) => {
                self.builtin("elm/core:Basics", "Float", vec![])
            }
            Kind::Number => Ok(self.engine.variable(level, Constraint::Number)),
            Kind::String => self.builtin("elm/core:String", "String", vec![]),
            Kind::Char => self.builtin("elm/core:Char", "Char", vec![]),
            Kind::Shader => {
                let shader = crate::shader::Shader::parse(text)?;
                let mut records = Vec::new();
                for (fields, open) in [
                    (&shader.attributes, true),
                    (&shader.uniforms, true),
                    (&shader.varyings, false),
                ] {
                    let mut lowered = BTreeMap::new();
                    for (name, ty) in fields {
                        use crate::shader::Type as S;
                        let (module, ty) = match ty {
                            S::Int => ("elm/core:Basics", "Int"),
                            S::Float => ("elm/core:Basics", "Float"),
                            S::Vec2 => ("elm-explorations/linear-algebra:Math.Vector2", "Vec2"),
                            S::Vec3 => ("elm-explorations/linear-algebra:Math.Vector3", "Vec3"),
                            S::Vec4 => ("elm-explorations/linear-algebra:Math.Vector4", "Vec4"),
                            S::Mat4 => ("elm-explorations/linear-algebra:Math.Matrix4", "Mat4"),
                            S::Texture => ("elm-explorations/webgl:WebGL.Texture", "Texture"),
                        };
                        lowered.insert(name.as_str().into(), self.builtin(module, ty, vec![])?);
                    }
                    let extension = open.then(|| self.fresh(level));
                    records.push(if lowered.is_empty() && open {
                        extension.unwrap()
                    } else {
                        self.engine.term(Term::Record {
                            fields: lowered,
                            extension,
                        })
                    });
                }
                self.builtin("elm-explorations/webgl:WebGL", "Shader", records)
            }
            _ => Err("invalid typed literal".into()),
        }
    }
    fn apply(
        &mut self,
        mut fun: Ty,
        args: impl IntoIterator<Item = Ty>,
        level: u32,
    ) -> Result<Ty, String> {
        for arg in args {
            let result = self.fresh(level);
            let expected = self.engine.term(Term::Function(arg, result));
            self.engine
                .unify_diagnostic(fun, expected, self.source.symbols)?;
            fun = result;
        }
        Ok(fun)
    }
    fn pattern(&mut self, root: PatternId, level: u32) -> Result<Ty, String> {
        let mut pending = vec![(root, false)];
        while let Some((id, finish)) = pending.pop() {
            if self.patterns[id.0 as usize].is_some() {
                continue;
            }
            let pattern = &self.source.ast.patterns[id.0 as usize].kind;
            if !finish {
                pending.push((id, true));
                let mut children = pattern_children(pattern);
                if self.engine.tracks_display_names() {
                    children.reverse();
                }
                pending.extend(children.into_iter().map(|p| (p, false)));
                continue;
            }
            let ty = (|| -> Result<Ty, String> {
                let get = |p: PatternId| self.patterns[p.0 as usize].unwrap();
                let ty = match pattern {
                    Pattern::Wildcard | Pattern::Var(_) => self.fresh(level),
                    Pattern::Literal(k, t) => self.literal(*k, t, true, level)?,
                    Pattern::Unit => self.engine.term(Term::Unit),
                    Pattern::Alias(p, _) => get(*p),
                    Pattern::Tuple(xs) => self
                        .engine
                        .term(Term::Tuple(xs.iter().map(|p| get(*p)).collect())),
                    Pattern::List(xs) => {
                        let parts: Vec<_> = xs.iter().map(|p| get(*p)).collect();
                        let item = self.fresh(level);
                        for (index, (child, actual)) in xs.iter().zip(parts).enumerate() {
                            if self.engine.tracks_display_names() {
                                let region = self.source.ast.patterns[id.0 as usize].span;
                                if let Err(error) = self.engine.unify_annotation_diagnostic(
                                    item,
                                    actual,
                                    self.source.symbols,
                                ) {
                                    let Some(report) =
                                        crate::annotation_diagnostic::pattern_mismatch(
                                            self.source.ast,
                                            self.source.symbols,
                                            self.engine,
                                            (
                                                *child,
                                                region,
                                                index + 1,
                                                crate::annotation_diagnostic::PatternContext::List,
                                            ),
                                            actual,
                                            item,
                                        )
                                    else {
                                        return Err(error);
                                    };
                                    self.annotation_errors.push(report);
                                    self.engine.mark_diagnostic_error(item);
                                }
                            } else {
                                self.engine
                                    .unify_diagnostic(item, actual, self.source.symbols)?;
                            }
                        }
                        self.builtin("elm/core:List", "List", vec![item])?
                    }
                    Pattern::Cons(a, b) => {
                        let (head, tail) = (get(*a), get(*b));
                        let list = self.builtin("elm/core:List", "List", vec![head])?;
                        if self.engine.tracks_display_names() {
                            if let Err(error) = self.engine.unify_annotation_diagnostic(
                                list,
                                tail,
                                self.source.symbols,
                            ) {
                                let region = self.source.ast.patterns[id.0 as usize].span;
                                let Some(report) = crate::annotation_diagnostic::pattern_mismatch(
                                    self.source.ast,
                                    self.source.symbols,
                                    self.engine,
                                    (
                                        *b,
                                        region,
                                        0,
                                        crate::annotation_diagnostic::PatternContext::Tail,
                                    ),
                                    tail,
                                    list,
                                ) else {
                                    return Err(error);
                                };
                                self.annotation_errors.push(report);
                                self.engine.mark_diagnostic_error(list);
                            }
                        } else {
                            self.engine
                                .unify_diagnostic(list, tail, self.source.symbols)?;
                        }
                        list
                    }
                    Pattern::Constructor(name, xs) => {
                        let args: Vec<_> = xs.iter().map(|p| get(*p)).collect();
                        let constructor = self.source.resolved.constructors[id.0 as usize]
                            .ok_or("unresolved constructor")?;
                        let mut fun = self.binding(Binding::Global(constructor), level)?;
                        if self.engine.tracks_display_names() {
                            for (index, (child, actual)) in xs.iter().zip(args).enumerate() {
                                let expected = self.fresh(level);
                                let result = self.fresh(level);
                                let shape = self.engine.term(Term::Function(expected, result));
                                self.engine
                                    .unify_diagnostic(fun, shape, self.source.symbols)?;
                                if let Err(error) = self.engine.unify_annotation_diagnostic(
                                    expected,
                                    actual,
                                    self.source.symbols,
                                ) {
                                    let region = self.source.ast.patterns[id.0 as usize].span;
                                    let Some(report) =
                                        crate::annotation_diagnostic::pattern_mismatch(
                                            self.source.ast,
                                            self.source.symbols,
                                            self.engine,
                                            (*child, region, index + 1, crate::annotation_diagnostic::PatternContext::Constructor(name.rsplit('.').next().unwrap_or(name))),
                                            actual,
                                            expected,
                                        )
                                    else {
                                        return Err(error);
                                    };
                                    self.annotation_errors.push(report);
                                    self.engine.mark_diagnostic_error(expected);
                                    self.engine.mark_diagnostic_error(actual);
                                }
                                fun = result;
                            }
                            fun
                        } else {
                            self.apply(fun, args, level)?
                        }
                    }
                    Pattern::Record(fields) => {
                        let fields = fields
                            .iter()
                            .map(|name| ((*name).into(), self.fresh(level)))
                            .collect();
                        let extension = Some(self.fresh(level));
                        self.engine.term(Term::Record { fields, extension })
                    }
                };
                Ok(ty)
            })()
            .map_err(|error| {
                crate::source_error::locate(
                    self.source.ast.source,
                    self.source.ast.patterns[id.0 as usize].span,
                    error,
                )
            })?;
            if let Some(bindings) = self.source.resolved.pattern_bindings.get(&id) {
                for (name, local) in bindings {
                    let bound = if matches!(pattern, Pattern::Record(_)) {
                        let term = self.engine.structure(ty).unwrap();
                        let Term::Record { fields, .. } = &*term else {
                            unreachable!()
                        };
                        fields[&crate::edition::FieldName::from(name.as_str())]
                    } else {
                        ty
                    };
                    self.locals[local.0 as usize] = Some(Self::mono(bound));
                }
            }
            self.patterns[id.0 as usize] = Some(ty);
        }
        Ok(self.patterns[root.0 as usize].unwrap())
    }
    fn expr(&mut self, root: ExprId, level: u32) -> Result<Ty, String> {
        self.depth += 1;
        if self.depth > 512 {
            return Err("type inference nesting limit exceeded".into());
        }
        let mut pending = vec![(root, false)];
        while let Some((id, finish)) = pending.pop() {
            if self.expressions[id.0 as usize].is_some() {
                continue;
            }
            let expr = &self.source.ast.expressions[id.0 as usize].kind;
            if !finish {
                if let Some((pattern, subject, case, index)) = self.case_patterns.remove(&id) {
                    let actual = self.pattern(pattern, level)?;
                    if let Err(error) = self.engine.unify_annotation_diagnostic(
                        subject,
                        actual,
                        self.source.symbols,
                    ) {
                        let Some(report) = crate::annotation_diagnostic::case_pattern(
                            self.source.ast,
                            self.source.symbols,
                            self.engine,
                            (pattern, case, index),
                            actual,
                            subject,
                        ) else {
                            return Err(error);
                        };
                        self.annotation_errors.push(report);
                        self.engine.mark_diagnostic_error(subject);
                        self.engine.mark_diagnostic_error(actual);
                    }
                }
                if let Some((context, expected)) = self.annotation_contexts.get(&id).cloned() {
                    match expr {
                        Expr::If(..) if context.is_annotation() => {
                            let (chain, final_branch) = if_chain(self.source.ast, id);
                            for (index, (_, _, yes)) in chain.iter().enumerate() {
                                let mut branch = context.clone();
                                branch.branch = Some(("if", index + 1));
                                self.annotation_contexts.insert(*yes, (branch, expected));
                            }
                            let mut branch = context;
                            branch.branch = Some(("if", chain.len() + 1));
                            self.annotation_contexts
                                .insert(final_branch, (branch, expected));
                        }
                        Expr::Case(_, branches) if context.is_annotation() => {
                            for (index, (_, body)) in branches.iter().enumerate() {
                                let mut branch = context.clone();
                                branch.branch = Some(("case", index + 1));
                                self.annotation_contexts.insert(*body, (branch, expected));
                            }
                        }
                        Expr::Let(_, body) => {
                            self.annotation_contexts.insert(*body, (context, expected));
                        }
                        _ => {}
                    }
                }
                if self.engine.tracks_display_names()
                    && let Expr::Record {
                        base: Some(_),
                        fields,
                    } = expr
                {
                    let original = self.reference(id, level)?;
                    let mut ordered: Vec<_> = fields.iter().collect();
                    ordered.sort_by(|(left, _), (right, _)| crate::edition::compare_names(left, right));
                    let expected_fields: BTreeMap<_, _> = ordered
                        .iter()
                        .map(|(name, _)| ((*name).into(), self.fresh(level)))
                        .collect();
                    let extension = Some(self.fresh(level));
                    let record = self.engine.term(Term::Record {
                        fields: expected_fields.clone(),
                        extension,
                    });
                    if self
                        .engine
                        .unify_annotation_diagnostic(record, original, self.source.symbols)
                        .is_ok()
                    {
                        let region = self.source.ast.expressions[id.0 as usize].span;
                        for (name, child) in &ordered {
                            let mut context = crate::annotation_diagnostic::Context::body("");
                            context.update_value = Some((region, name.to_string()));
                            let expected = expected_fields[&crate::edition::FieldName::from(*name)];
                            self.annotation_contexts.insert(*child, (context, expected));
                        }
                        self.call_results.insert(id, record);
                        pending.push((id, true));
                        pending.extend(ordered.iter().rev().map(|(_, child)| (*child, false)));
                        continue;
                    } else if let Some(report) = crate::record_access_diagnostic::capture(
                        self.source,
                        self.engine,
                        id,
                        id,
                        original,
                        record,
                    ) {
                        self.annotation_errors.push(report);
                        self.engine.mark_diagnostic_error(original);
                        self.engine.mark_diagnostic_error(record);
                        self.call_results.insert(id, record);
                        pending.push((id, true));
                        pending.extend(ordered.iter().rev().map(|(_, child)| (*child, false)));
                        continue;
                    }
                }
                if self.engine.tracks_display_names() {
                    let region = self.source.ast.expressions[id.0 as usize].span;
                    match expr {
                        Expr::Negate(child) => {
                            let expected = self.engine.variable(level, Constraint::Number);
                            let mut context = crate::annotation_diagnostic::Context::body("");
                            context.operator = Some((region, "negate", ""));
                            self.annotation_contexts.insert(*child, (context, expected));
                        }
                        Expr::Binary(op, left, right)
                            if matches!(
                                *op,
                                "&&" | "||"
                                    | "-"
                                    | "^"
                                    | "+"
                                    | "*"
                                    | "/"
                                    | "//"
                                    | "::"
                                    | "++"
                                    | "=="
                                    | "/="
                                    | "<"
                                    | ">"
                                    | "<="
                                    | ">="
                                    | "|>"
                                    | "<|"
                                    | ">>"
                                    | "<<"
                            ) =>
                        {
                            let operator = match *op {
                                "&&" => "&&",
                                "||" => "||",
                                "-" => "-",
                                "^" => "^",
                                "+" => "+",
                                "*" => "*",
                                "/" => "/",
                                "//" => "//",
                                "::" => "::",
                                "++" => "++",
                                "==" => "==",
                                "/=" => "/=",
                                "<" => "<",
                                ">" => ">",
                                "<=" => "<=",
                                ">=" => ">=",
                                "|>" => "|>",
                                "<|" => "<|",
                                ">>" => ">>",
                                "<<" => "<<",
                                _ => unreachable!(),
                            };
                            let left_type = self.fresh(level);
                            let right_type = self.fresh(level);
                            let result = self.fresh(level);
                            let right_function =
                                self.engine.term(Term::Function(right_type, result));
                            let signature =
                                self.engine.term(Term::Function(left_type, right_function));
                            let function = self.reference(id, level)?;
                            self.engine.unify_diagnostic(
                                function,
                                signature,
                                self.source.symbols,
                            )?;
                            for (child, expected, side) in
                                [(*left, left_type, "left"), (*right, right_type, "right")]
                            {
                                let mut context = crate::annotation_diagnostic::Context::body("");
                                context.operator = Some((region, operator, side));
                                let expected = self.engine.diagnostic_type_occurrence(expected);
                                self.annotation_contexts.insert(child, (context, expected));
                            }
                            self.call_results.insert(id, result);
                        }
                        _ => {}
                    }
                }
                if self.engine.tracks_display_names()
                    && let Expr::List(items) = expr
                {
                    let expected = self.fresh(level);
                    let region = self.source.ast.expressions[id.0 as usize].span;
                    for (index, item) in items.iter().enumerate() {
                        let mut context = crate::annotation_diagnostic::Context::body("");
                        context.branch = Some(("list", index + 1));
                        context.inferred = Some(region);
                        self.annotation_contexts.insert(*item, (context, expected));
                    }
                }
                if self.engine.tracks_display_names()
                    && let Expr::Case(subject, branches) = expr
                {
                    let Some(subject_type) = self.expressions[subject.0 as usize] else {
                        pending.push((id, false));
                        pending.push((*subject, false));
                        continue;
                    };
                    let annotated = self
                        .annotation_contexts
                        .get(&id)
                        .is_some_and(|(context, _)| context.is_annotation());
                    let expected = self.fresh(level);
                    let region = self.source.ast.expressions[id.0 as usize].span;
                    for (index, (pattern, body)) in branches.iter().enumerate() {
                        self.case_patterns
                            .insert(*body, (*pattern, subject_type, id, index + 1));
                        if !annotated {
                            let mut context = crate::annotation_diagnostic::Context::body("");
                            context.branch = Some(("case", index + 1));
                            context.inferred = Some(region);
                            self.annotation_contexts.insert(*body, (context, expected));
                        }
                    }
                    pending.push((id, true));
                    pending.extend(branches.iter().rev().map(|(_, body)| (*body, false)));
                    continue;
                }
                if self.engine.tracks_display_names() && matches!(expr, Expr::If(..)) {
                    let (chain, final_branch) = if_chain(self.source.ast, id);
                    let boolean = self.builtin("elm/core:Basics", "Bool", vec![])?;
                    let region = self.source.ast.expressions[id.0 as usize].span;
                    let annotated = self
                        .annotation_contexts
                        .get(&id)
                        .is_some_and(|(context, _)| context.is_annotation());
                    if !annotated {
                        let expected = self.fresh(level);
                        for (index, branch) in chain
                            .iter()
                            .map(|(_, _, yes)| *yes)
                            .chain(std::iter::once(final_branch))
                            .enumerate()
                        {
                            let mut context = crate::annotation_diagnostic::Context::body("");
                            context.branch = Some(("if", index + 1));
                            context.inferred = Some(region);
                            self.annotation_contexts.insert(branch, (context, expected));
                        }
                    }
                    for (_, condition, _) in &chain {
                        self.annotation_contexts.insert(
                            *condition,
                            (
                                crate::annotation_diagnostic::Context::condition(region),
                                boolean,
                            ),
                        );
                    }
                    // Canonical If constraints visit every condition first,
                    // then the branches; complete the nested AST nodes last.
                    pending.extend(chain.iter().map(|(id, _, _)| (*id, true)));
                    let children: Vec<_> = chain
                        .iter()
                        .map(|(_, condition, _)| *condition)
                        .chain(chain.iter().map(|(_, _, yes)| *yes))
                        .chain(std::iter::once(final_branch))
                        .collect();
                    pending.extend(children.into_iter().rev().map(|id| (id, false)));
                    continue;
                }
                if self.engine.tracks_display_names()
                    && let Expr::Call(function, args) = expr
                {
                    let Some(function_type) = self.expressions[function.0 as usize] else {
                        pending.push((id, false));
                        pending.push((*function, false));
                        continue;
                    };
                    let argument_types: Vec<_> = args.iter().map(|_| self.fresh(level)).collect();
                    let result = self.fresh(level);
                    let mut arity = result;
                    for arg in argument_types.iter().rev() {
                        arity = self.engine.term(Term::Function(*arg, arity));
                    }
                    if self
                        .engine
                        .unify_annotation_diagnostic(function_type, arity, self.source.symbols)
                        .is_err()
                    {
                        let mut current = function_type;
                        let mut count = 0;
                        let mut seen = BTreeSet::new();
                        while seen.insert(self.engine.find(current)) {
                            let Some(term) = self.engine.structure(current) else {
                                break;
                            };
                            let Term::Function(_, result) = &*term else {
                                break;
                            };
                            count += 1;
                            current = *result;
                        }
                        self.annotation_errors
                            .push(crate::annotation_diagnostic::call_arity(
                                self.source.ast,
                                *function,
                                id,
                                count,
                                args.len(),
                            ));
                        self.engine.mark_diagnostic_error(function_type);
                        self.engine.mark_diagnostic_error(arity);
                    }
                    let name = match &self.source.ast.expressions[function.0 as usize].kind {
                        Expr::Var(name) => format!("`{}`", name.rsplit('.').next().unwrap_or(name)),
                        Expr::Operator(name) => format!("({name})"),
                        _ => "this function".into(),
                    };
                    let region = self.source.ast.expressions[id.0 as usize].span;
                    for (index, (arg, expected)) in args.iter().zip(argument_types).enumerate() {
                        let mut context = crate::annotation_diagnostic::Context::body(&name);
                        context.argument = Some((region, index + 1));
                        self.annotation_contexts.insert(*arg, (context, expected));
                    }
                    self.call_results.insert(id, result);
                    pending.push((id, true));
                    pending.extend(args.iter().rev().map(|id| (*id, false)));
                    continue;
                }
                match expr {
                    Expr::Lambda(args, _) => {
                        for p in args {
                            self.pattern(*p, level)?;
                        }
                    }
                    Expr::Case(_, branches) => {
                        for (p, _) in branches {
                            self.pattern(*p, level)?;
                        }
                    }
                    Expr::Let(declarations, body) => {
                        self.group(declarations, false, level)?;
                        pending.push((id, true));
                        pending.push((*body, false));
                        continue;
                    }
                    _ => {}
                }
                pending.push((id, true));
                pending.extend(expr_children(expr).into_iter().rev().map(|e| (e, false)));
                continue;
            }
            let mut ty = (|| -> Result<Ty, String> {
                let get = |id: ExprId| self.expressions[id.0 as usize].unwrap();
                let ty = match expr {
                    Expr::Literal(k, t) => self.literal(*k, t, false, level)?,
                    Expr::Unit => self.engine.term(Term::Unit),
                    Expr::Var(_) | Expr::Operator(_) => self.reference(id, level)?,
                    Expr::Tuple(xs) => self
                        .engine
                        .term(Term::Tuple(xs.iter().map(|x| get(*x)).collect())),
                    Expr::List(xs) => {
                        let parts: Vec<_> = xs.iter().map(|x| get(*x)).collect();
                        let item = self.fresh(level);
                        for p in parts {
                            self.engine.unify_diagnostic(item, p, self.source.symbols)?;
                        }
                        self.builtin("elm/core:List", "List", vec![item])?
                    }
                    Expr::Negate(x) => {
                        let x = get(*x);
                        let num = self.engine.variable(level, Constraint::Number);
                        self.engine.unify_diagnostic(num, x, self.source.symbols)?;
                        x
                    }
                    Expr::Call(f, args) => {
                        if let Some(result) = self.call_results.remove(&id) {
                            result
                        } else {
                            let f = get(*f);
                            let args: Vec<_> = args.iter().map(|a| get(*a)).collect();
                            self.apply(f, args, level)?
                        }
                    }
                    Expr::Binary(_, a, b) => {
                        if let Some(result) = self.call_results.remove(&id) {
                            result
                        } else {
                            let args = [get(*a), get(*b)];
                            let f = self.reference(id, level)?;
                            self.apply(f, args, level)?
                        }
                    }
                    Expr::Binops(_, _) => return Err("unresolved binary operators".into()),
                    Expr::Lambda(args, body) => {
                        let mut t = get(*body);
                        for p in args.iter().rev() {
                            t = self
                                .engine
                                .term(Term::Function(self.patterns[p.0 as usize].unwrap(), t));
                        }
                        t
                    }
                    Expr::If(c, a, b) => {
                        let (c, a, b) = (get(*c), get(*a), get(*b));
                        let boolean = self.builtin("elm/core:Basics", "Bool", vec![])?;
                        self.engine
                            .unify_diagnostic(c, boolean, self.source.symbols)?;
                        self.engine.unify_diagnostic(a, b, self.source.symbols)?;
                        a
                    }
                    Expr::Let(_, body) => get(*body),
                    Expr::Case(subject, branches) => {
                        let subject = get(*subject);
                        let branches: Vec<_> = branches
                            .iter()
                            .map(|(p, b)| (self.patterns[p.0 as usize].unwrap(), get(*b)))
                            .collect();
                        let result = self.fresh(level);
                        for (p, b) in branches {
                            self.engine
                                .unify_diagnostic(subject, p, self.source.symbols)?;
                            self.engine
                                .unify_diagnostic(result, b, self.source.symbols)?;
                        }
                        result
                    }
                    Expr::Accessor(field) => {
                        let value = self.fresh(level);
                        let extension = Some(self.fresh(level));
                        let record = self.engine.term(Term::Record {
                            fields: [((*field).into(), value)].into(),
                            extension,
                        });
                        self.engine.term(Term::Function(record, value))
                    }
                    Expr::Access(base, field) => {
                        let base_expr = *base;
                        let base = get(*base);
                        let value = self.fresh(level);
                        let extension = Some(self.fresh(level));
                        let record = self.engine.term(Term::Record {
                            fields: [((*field).into(), value)].into(),
                            extension,
                        });
                        if self.engine.tracks_display_names() {
                            if let Err(error) = self.engine.unify_annotation_diagnostic(
                                base,
                                record,
                                self.source.symbols,
                            ) {
                                let Some(report) = crate::record_access_diagnostic::capture(
                                    self.source,
                                    self.engine,
                                    id,
                                    base_expr,
                                    base,
                                    record,
                                ) else {
                                    return Err(error);
                                };
                                self.annotation_errors.push(report);
                                self.engine.mark_diagnostic_error(base);
                                self.engine.mark_diagnostic_error(value);
                            }
                        } else {
                            self.engine
                                .unify_diagnostic(base, record, self.source.symbols)?;
                        }
                        value
                    }
                    Expr::Record { base, fields } => {
                        if let Some(result) = self.call_results.remove(&id) {
                            return Ok(result);
                        }
                        let fields = fields
                            .iter()
                            .map(|(name, id)| ((*name).into(), get(*id)))
                            .collect();
                        let extension = base.map(|_| self.fresh(level));
                        let record = self.engine.term(Term::Record { fields, extension });
                        if base.is_some() {
                            let original = self.reference(id, level)?;
                            self.engine
                                .unify_diagnostic(record, original, self.source.symbols)?;
                        }
                        record
                    }
                };
                Ok(ty)
            })()
            .map_err(|error| {
                crate::source_error::locate(
                    self.source.ast.source,
                    self.source.ast.expressions[id.0 as usize].span,
                    error,
                )
            })?;
            if let Some((context, expected)) = self.annotation_contexts.remove(&id)
                && let Err(error) =
                    self.engine
                        .unify_annotation_diagnostic(expected, ty, self.source.symbols)
            {
                let Some(error) = self.infinite_report().or_else(|| {
                    crate::annotation_diagnostic::capture(
                        self.source,
                        self.engine,
                        &context,
                        id,
                        ty,
                        expected,
                    )
                }) else {
                    return Err(error);
                };
                self.annotation_errors.push(error);
                if context.update_value.is_some() {
                    self.engine.mark_diagnostic_error(ty);
                }
                if context.inferred.is_some()
                    || context.argument.is_some()
                    || context.operator.is_some()
                    || context.update_value.is_some()
                {
                    self.engine.mark_diagnostic_error(expected);
                }
                // This expression has already been reported. Continue sibling
                // constraints using the expected type to avoid cascaded errors.
                ty = expected;
            }
            self.expressions[id.0 as usize] = Some(ty);
        }
        self.depth -= 1;
        Ok(self.expressions[root.0 as usize].unwrap())
    }
    fn infinite_report(&mut self) -> Option<String> {
        let (variable, value) = self.engine.take_infinite_type()?;
        let variable = self.engine.find(variable);
        let value = self.engine.find(value);
        let mut selected = None;
        let mut root = value;
        for exact in [true, false] {
            for (pattern, bindings) in &self.source.resolved.pattern_bindings {
                for (name, local) in bindings {
                    if self
                        .locals
                        .get(local.0 as usize)?
                        .as_ref()
                        .is_some_and(|scheme| {
                            if if exact {
                                self.engine.find(scheme.root) == variable
                            } else {
                                self.engine.contains_type(scheme.root, variable)
                            } {
                                root = scheme.root;
                                true
                            } else {
                                false
                            }
                        })
                    {
                        let node = &self.source.ast.patterns[pattern.0 as usize];
                        let found = match &node.kind {
                            Pattern::Var(found) | Pattern::Alias(_, found) if *found == name => {
                                Some(*found)
                            }
                            Pattern::Record(fields) => {
                                fields.iter().find(|field| **field == name).copied()
                            }
                            _ => None,
                        };
                        if let Some(found) = found {
                            let start = (found.as_ptr() as usize)
                                .checked_sub(self.source.ast.source.as_ptr() as usize)?
                                as u32;
                            selected = Some((
                                found,
                                if matches!(node.kind, Pattern::Record(_)) {
                                    node.span
                                } else {
                                    crate::ast::Span {
                                        start,
                                        end: start + found.len() as u32,
                                    }
                                },
                            ));
                            break;
                        }
                    }
                }
                if selected.is_some() {
                    break;
                }
            }
            if selected.is_some() {
                break;
            }
        }
        if selected.is_none() {
            for expression in &self.source.ast.expressions {
                if let Expr::Let(declarations, _) = &expression.kind {
                    for declaration in declarations {
                        if let Declaration::Value { name, body, .. } = declaration {
                            let local = self.source.resolved.definitions.get(body)?;
                            if self.locals[local.0 as usize]
                                .as_ref()
                                .is_some_and(|scheme| {
                                    if self.engine.contains_type(scheme.root, variable) {
                                        root = scheme.root;
                                        true
                                    } else {
                                        false
                                    }
                                })
                            {
                                let start = (name.as_ptr() as usize)
                                    .checked_sub(self.source.ast.source.as_ptr() as usize)?
                                    as u32;
                                selected = Some((
                                    *name,
                                    crate::ast::Span {
                                        start,
                                        end: start + name.len() as u32,
                                    },
                                ));
                                break;
                            }
                        }
                    }
                }
                if selected.is_some() {
                    break;
                }
            }
        }
        if selected.is_none() {
            for declaration in &self.source.ast.declarations {
                if let Declaration::Value { name, .. } = declaration {
                    let id = self
                        .source
                        .symbols
                        .lookup(self.module, name, Space::Value)?;
                    if self.globals.get(&id).is_some_and(|scheme| {
                        if self.engine.contains_type(scheme.root, variable) {
                            root = scheme.root;
                            true
                        } else {
                            false
                        }
                    }) {
                        let start = (name.as_ptr() as usize)
                            .checked_sub(self.source.ast.source.as_ptr() as usize)?
                            as u32;
                        selected = Some((
                            *name,
                            crate::ast::Span {
                                start,
                                end: start + name.len() as u32,
                            },
                        ));
                        break;
                    }
                }
            }
        }
        let (name, span) = selected?;
        if let Some(index) = self
            .infinite_diagnostics
            .iter()
            .position(|pending| pending.span == span && pending.name == name)
        {
            self.infinite_diagnostics[index]
                .cycles
                .push((variable, value));
            return Some(format!("INFINITE_PENDING:{index}"));
        }
        let cycles = vec![(variable, value)];
        let fallback = crate::infinite_type_diagnostic::capture(
            self.source.ast,
            self.source.symbols,
            self.engine,
            &cycles,
            root,
            name,
            span,
        )?;
        let index = self.infinite_diagnostics.len();
        self.infinite_diagnostics.push(InfiniteDiagnostic {
            name: name.into(),
            span,
            root,
            cycles,
            fallback,
        });
        Some(format!("INFINITE_PENDING:{index}"))
    }
    fn targets(&self, decl: &Declaration<'s>, top: bool) -> Result<Vec<Binding>, String> {
        match decl {
            Declaration::Value { name, body, .. } => Ok(vec![if top {
                Binding::Global(
                    self.source
                        .symbols
                        .lookup(self.module, name, Space::Value)
                        .ok_or("missing definition symbol")?,
                )
            } else {
                Binding::Local(
                    *self
                        .source
                        .resolved
                        .definitions
                        .get(body)
                        .ok_or("missing let definition")?,
                )
            }]),
            Declaration::Destruct { pattern, .. } => {
                let mut pending = vec![*pattern];
                let mut out = Vec::new();
                while let Some(p) = pending.pop() {
                    out.extend(
                        self.source
                            .resolved
                            .pattern_bindings
                            .get(&p)
                            .into_iter()
                            .flatten()
                            .map(|(_, local)| Binding::Local(*local)),
                    );
                    pending.extend(pattern_children(
                        &self.source.ast.patterns[p.0 as usize].kind,
                    ));
                }
                Ok(out)
            }
            _ => Ok(vec![]),
        }
    }
    fn scoped_signature(&mut self, ty: TypeId, level: u32, rigid: bool) -> Result<Ty, String> {
        let mut pending = vec![ty];
        while let Some(id) = pending.pop() {
            match &self.source.ast.types[id.0 as usize].kind {
                Type::Var(name) => {
                    self.scoped_types.entry(name).or_insert_with(|| {
                        if rigid {
                            self.engine
                                .named_rigid(level, types::constraint(name), name)
                        } else {
                            self.engine
                                .named_variable(level, types::constraint(name), name)
                        }
                    });
                }
                Type::Constructor(_, args) | Type::Tuple(args) => pending.extend(args),
                Type::Function(a, b) => pending.extend([*a, *b]),
                Type::Record { extension, fields } => {
                    if let Some(name) = extension {
                        self.scoped_types.entry(name).or_insert_with(|| {
                            if rigid {
                                self.engine
                                    .named_rigid(level, types::constraint(name), name)
                            } else {
                                self.engine
                                    .named_variable(level, types::constraint(name), name)
                            }
                        });
                    }
                    pending.extend(fields.iter().map(|(_, ty)| *ty));
                }
                Type::Unit => {}
            }
        }
        self.catalog
            .lower(self.source, self.engine, ty, &mut self.scoped_types, level)
    }
    fn function(
        &mut self,
        args: &[PatternId],
        body: ExprId,
        level: u32,
        expected: Option<Ty>,
        annotation: Option<&str>,
    ) -> Result<Ty, String> {
        // Destructuring introduces fresh pattern variables. Preserve sharing of
        // variables already constrained by earlier bodies, while propagating
        // intermediate generic status to newly introduced pattern variables.
        let generics: BTreeSet<_> = self
            .recursive_generics
            .iter()
            .map(|(ty, _)| self.engine.find(*ty))
            .collect();
        let mut expected_args = Vec::new();
        if !generics.is_empty() {
            let mut tail = expected;
            for _ in args {
                if let Some(ty) = tail
                    && let Some(term) = self.engine.structure(ty)
                    && let Term::Function(arg, result) = &*term
                {
                    let arg = self.engine.find(*arg);
                    let blocked = self
                        .engine
                        .generalize(arg, 0)
                        .quantified
                        .into_iter()
                        .filter(|id| !generics.contains(id))
                        .collect::<Vec<_>>();
                    expected_args.push(generics.contains(&arg).then_some(blocked));
                    tail = Some(*result);
                    continue;
                }
                expected_args.push(None);
                tail = None;
            }
        }
        let mut argument_types = Vec::new();
        for p in args {
            argument_types.push(self.pattern(*p, level)?);
        }
        let typed_arguments = if self.engine.tracks_display_names() && annotation.is_some() {
            Some(args.iter().map(|_| self.fresh(level)).collect::<Vec<_>>())
        } else {
            None
        };
        let result = self.fresh(level);
        let mut fun = result;
        for arg in typed_arguments
            .as_ref()
            .unwrap_or(&argument_types)
            .iter()
            .rev()
        {
            fun = self.engine.term(Term::Function(*arg, fun));
        }
        if let Some(expected) = expected {
            self.engine
                .unify_diagnostic(fun, expected, self.source.symbols)?;
        }
        if let Some(expected) = typed_arguments {
            let name = annotation.unwrap();
            for (index, ((pattern, actual), expected)) in
                args.iter().zip(&argument_types).zip(expected).enumerate()
            {
                if let Err(error) =
                    self.engine
                        .unify_annotation_diagnostic(expected, *actual, self.source.symbols)
                {
                    let region = self.source.ast.patterns[pattern.0 as usize].span;
                    let Some(report) = crate::annotation_diagnostic::pattern_mismatch(
                        self.source.ast,
                        self.source.symbols,
                        self.engine,
                        (
                            *pattern,
                            region,
                            index + 1,
                            crate::annotation_diagnostic::PatternContext::TypedArgument(name),
                        ),
                        *actual,
                        expected,
                    ) else {
                        return Err(error);
                    };
                    self.annotation_errors.push(report);
                    self.engine.mark_diagnostic_error(expected);
                    self.engine.mark_diagnostic_error(*actual);
                }
            }
        }
        for (arg, blocked) in argument_types.into_iter().zip(expected_args) {
            if let Some(blocked) = blocked {
                let blocked: BTreeSet<_> =
                    blocked.into_iter().map(|id| self.engine.find(id)).collect();
                for ty in self.engine.generalize(arg, level - 1).quantified {
                    if !blocked.contains(&ty) {
                        self.recursive_generics.push((ty, level - 1));
                    }
                }
            }
        }
        if self.engine.tracks_display_names()
            && let Some(name) = annotation
        {
            self.annotation_contexts.insert(
                body,
                (crate::annotation_diagnostic::Context::body(name), result),
            );
        }
        let body_type = self.expr(body, level)?;
        let unified = if annotation.is_some() {
            self.engine
                .unify_annotation_diagnostic(result, body_type, self.source.symbols)
        } else {
            self.engine
                .unify_diagnostic(result, body_type, self.source.symbols)
        };
        unified.map_err(|error| {
            annotation
                .and_then(|name| {
                    crate::annotation_diagnostic::capture(
                        self.source,
                        self.engine,
                        &crate::annotation_diagnostic::Context::body(name),
                        body,
                        body_type,
                        result,
                    )
                })
                .unwrap_or(error)
        })?;
        Ok(fun)
    }
    // Elm canonicalizes local bindings with Data.Graph.stronglyConnComp.
    // The SCC root depends on the entire let graph, including acyclic bindings
    // and synthetic destructuring nodes, not just the recursive component.
    fn local_recursive_order(
        &self,
        definitions: &[&Declaration<'s>],
        owners: &BTreeMap<Binding, usize>,
        component: &[usize],
    ) -> Result<Vec<usize>, String> {
        fn pattern_names(ast: &crate::ast::Syntax<'_>, id: PatternId) -> Vec<String> {
            match &ast.patterns[id.0 as usize].kind {
                Pattern::Var(name) => vec![name.to_string()],
                Pattern::Record(fields) => fields.iter().map(|s| s.to_string()).collect(),
                Pattern::Alias(p, name) => {
                    let mut names = pattern_names(ast, *p);
                    names.push(name.to_string());
                    names
                }
                Pattern::Constructor(_, args) | Pattern::Tuple(args) | Pattern::List(args) => args
                    .iter()
                    .rev()
                    .flat_map(|p| pattern_names(ast, *p))
                    .collect(),
                Pattern::Cons(a, b) => {
                    let mut names = pattern_names(ast, *b);
                    names.extend(pattern_names(ast, *a));
                    names
                }
                _ => vec![],
            }
        }
        let mut nodes = BTreeMap::<String, (Option<usize>, BTreeSet<String>)>::new();
        for (index, definition) in definitions.iter().enumerate() {
            let (key, body, names) = match definition {
                Declaration::Value { name, body, .. } => (name.to_string(), *body, Vec::new()),
                Declaration::Destruct { pattern, body } => {
                    let names = pattern_names(self.source.ast, *pattern);
                    let key = names
                        .first()
                        .map_or_else(|| "_".to_string(), |name| format!("_M${name}"));
                    (key, *body, names)
                }
                _ => unreachable!(),
            };
            let mut dependencies = BTreeSet::new();
            let mut pending = vec![body];
            while let Some(id) = pending.pop() {
                if let Some(binding) = self.source.resolved.expressions[id.0 as usize]
                    && owners.contains_key(&binding)
                    && let Binding::Local(id) = binding
                {
                    dependencies.insert(self.source.resolved.locals[id.0 as usize].clone());
                }
                pending.extend(expr_children(
                    &self.source.ast.expressions[id.0 as usize].kind,
                ));
            }
            for name in names {
                nodes.insert(name, (None, BTreeSet::from([key.clone()])));
            }
            nodes.insert(key, (Some(index), dependencies));
        }
        let keys: BTreeMap<_, _> = nodes
            .keys()
            .enumerate()
            .map(|(i, key)| (key.clone(), i))
            .collect();
        let node_owners: Vec<_> = nodes.values().map(|(owner, _)| *owner).collect();
        let graph: Vec<Vec<_>> = nodes
            .values()
            .map(|(_, deps)| {
                deps.iter()
                    .filter_map(|key| keys.get(key).copied())
                    .collect()
            })
            .collect();
        let mut transpose = vec![Vec::new(); graph.len()];
        for (id, neighbors) in graph.iter().enumerate() {
            for &next in neighbors {
                transpose[next].push(id);
            }
        }
        let mut seen = BTreeSet::new();
        let mut postorder = Vec::new();
        for start in 0..graph.len() {
            let mut pending = vec![(start, false)];
            while let Some((id, finish)) = pending.pop() {
                if finish {
                    postorder.push(id);
                    continue;
                }
                if !seen.insert(id) {
                    continue;
                }
                pending.push((id, true));
                // Data.Graph.buildG prepends incoming edges; visit them in reverse key order.
                pending.extend(transpose[id].iter().map(|&n| (n, false)));
            }
        }
        let members: BTreeSet<_> = component.iter().copied().collect();
        let start = postorder
            .into_iter()
            .rev()
            .find(|&id| node_owners[id].is_some_and(|i| members.contains(&i)))
            .ok_or("missing recursive group root")?;
        let mut pending = vec![start];
        let mut visited = BTreeSet::new();
        let mut result = Vec::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let Some(owner) = node_owners[id].filter(|i| members.contains(i)) else {
                continue;
            };
            result.push(owner);
            pending.extend(graph[id].iter().rev().copied());
        }
        if result.len() != component.len() {
            return Err("incomplete recursive group traversal".into());
        }
        Ok(result)
    }
    // Elm rejects cycles in immediate value dependencies. Lambda bodies and
    // named function bodies are delayed, including functions inside a let.
    // Keep this graph separate from the full graph needed by type inference.
    fn check_global_value_cycle(
        &self,
        definitions: &[&Declaration<'s>],
        owners: &BTreeMap<Binding, usize>,
        component: &[usize],
    ) -> Result<(), String> {
        let members: BTreeMap<_, _> = component
            .iter()
            .enumerate()
            .map(|(local, &original)| (original, local))
            .collect();
        let mut direct = vec![BTreeSet::new(); component.len()];
        for (local, &index) in component.iter().enumerate() {
            let body = match definitions[index] {
                Declaration::Value {
                    arguments, body, ..
                } if arguments.is_empty() => *body,
                Declaration::Destruct { body, .. } => *body,
                _ => continue,
            };
            let mut pending = vec![body];
            while let Some(id) = pending.pop() {
                if let Some(binding) = self.source.resolved.expressions[id.0 as usize]
                    && let Some(owner) = owners.get(&binding)
                    && let Some(target) = members.get(owner)
                {
                    direct[local].insert(*target);
                }
                match &self.source.ast.expressions[id.0 as usize].kind {
                    Expr::Lambda(..) => {}
                    Expr::Let(declarations, body) => {
                        pending.push(*body);
                        pending.extend(declarations.iter().filter_map(|d| match d {
                            Declaration::Value {
                                arguments, body, ..
                            } if arguments.is_empty() => Some(*body),
                            Declaration::Destruct { body, .. } => Some(*body),
                            _ => None,
                        }));
                    }
                    expression => pending.extend(expr_children(expression)),
                }
            }
        }
        // Canonicalize.Module applies Data.Graph to the direct-dependency
        // subgraph using names as keys. Preserve that order for the diagnostic.
        let mut keys: Vec<_> = (0..component.len()).collect();
        keys.sort_by_key(|&local| match definitions[component[local]] {
            Declaration::Value { name, .. } => *name,
            _ => "",
        });
        let ranks: BTreeMap<_, _> = keys
            .iter()
            .enumerate()
            .map(|(rank, &local)| (local, rank))
            .collect();
        let mut transpose = vec![BTreeSet::new(); keys.len()];
        for (from, targets) in direct.iter().enumerate() {
            for target in targets {
                transpose[ranks[target]].insert(ranks[&from]);
            }
        }
        for group in components(&transpose).into_iter().rev() {
            let members: Vec<_> = group.iter().map(|&rank| keys[rank]).collect();
            if members.len() > 1 || direct[members[0]].contains(&members[0]) {
                let names: Option<Vec<_>> = members
                    .iter()
                    .map(|&local| match definitions[component[local]] {
                        Declaration::Value { name, .. } => Some(*name),
                        _ => None,
                    })
                    .collect();
                if let Some(names) = names {
                    return Err(crate::source_error::locate_slice(
                        self.source.ast.source,
                        names[0],
                        crate::name_diagnostic::cycle(names[0], &names[1..]),
                    ));
                }
                return Err(
                    "cyclic global value: immediate dependencies cannot be recursive".into(),
                );
            }
        }
        Ok(())
    }
    fn group(
        &mut self,
        declarations: &'a [Declaration<'s>],
        top: bool,
        level: u32,
    ) -> Result<(), String> {
        // Diagnostic constraints follow Elm's canonical dependency order. The
        // successful compact pass keeps its existing inference schedule.
        let definitions: Vec<_> = if self.engine.tracks_display_names() {
            crate::coverage::ordered_declarations(
                self.source.ast,
                self.source.resolved,
                self.source.symbols,
                declarations,
                top,
            )
        } else {
            declarations
                .iter()
                .filter(|d| matches!(d, Declaration::Value { .. } | Declaration::Destruct { .. }))
                .collect()
        };
        let mut owners = BTreeMap::new();
        let mut targets = Vec::new();
        for (index, decl) in definitions.iter().enumerate() {
            let bindings = self.targets(decl, top)?;
            for binding in &bindings {
                owners.insert(*binding, index);
            }
            targets.push(bindings);
        }
        let annotations: HashMap<_, _> = declarations
            .iter()
            .filter_map(|d| {
                if let Declaration::Annotation { name, ty } = d {
                    Some((*name, *ty))
                } else {
                    None
                }
            })
            .collect();
        let mut edges = vec![BTreeSet::new(); definitions.len()];
        for (index, decl) in definitions.iter().enumerate() {
            let body = match decl {
                Declaration::Value { body, .. } | Declaration::Destruct { body, .. } => *body,
                _ => unreachable!(),
            };
            let mut pending = vec![body];
            while let Some(id) = pending.pop() {
                if let Some(binding) = self.source.resolved.expressions[id.0 as usize]
                    && let Some(target) = owners.get(&binding)
                {
                    edges[index].insert(*target);
                }
                pending.extend(expr_children(
                    &self.source.ast.expressions[id.0 as usize].kind,
                ));
            }
        }
        for mut component in components(&edges) {
            let _profile = (top && self.profile_groups).then(|| InferenceGroupProfile {
                module: self.module.to_owned(),
                names: component.iter().map(|&index| match definitions[index] {
                    Declaration::Value { name, .. } => (*name).to_owned(),
                    _ => "<destructuring>".to_owned(),
                }).collect(),
                started: std::time::Instant::now(),
            });
            // Elm permits mutually recursive local functions, but forbids any
            // local recursive group containing a value or destructuring. Check
            // this even for unused definitions and before type unification.
            let cyclic = component.len() > 1 || edges[component[0]].contains(&component[0]);
            if top && cyclic {
                self.check_global_value_cycle(&definitions, &owners, &component)?;
            }
            if !top
                && cyclic
                && component.iter().any(|&index| match definitions[index] {
                    Declaration::Value { arguments, .. } => arguments.is_empty(),
                    Declaration::Destruct { .. } => true,
                    _ => false,
                })
            {
                return Err(crate::local_cycle_diagnostic::error(
                    self.source.ast,
                    self.source.resolved,
                    &definitions,
                )
                .unwrap_or_else(|| {
                    "cyclic local value: recursive let groups must contain only functions".into()
                }));
            }
            let mut placeholders = BTreeMap::new();
            let mut annotated = BTreeMap::new();
            let generic_start = self.recursive_generics.len();
            let partial = cyclic
                && component.len() > 1
                && component.iter().all(|&i| {
                    matches!(definitions[i], Declaration::Value { name, arguments, .. }
                        if !arguments.is_empty() && !annotations.contains_key(name))
                });
            if partial {
                // Top-level canonicalization performs a second SCC traversal;
                // constraints then visit functions in lexical order. Local lets
                // retain the DFS order of their single canonicalization pass.
                component.sort_by_key(|&i| match definitions[i] {
                    Declaration::Value { name, .. } => *name,
                    _ => unreachable!(),
                });
                if !top {
                    component = self.local_recursive_order(&definitions, &owners, &component)?;
                }
            }
            if self.engine.tracks_display_names() && cyclic && !partial {
                // recDefsHelp prepends each rigid/flexible constraint, so each
                // partition runs in reverse canonical declaration order.
                component.sort_unstable_by(|a, b| b.cmp(a));
            }
            let mut parameters = BTreeMap::new();
            for &index in &component {
                if let Declaration::Value { name, .. } = definitions[index]
                    && let Some(ty) = annotations.get(name)
                {
                    let saved = self.scoped_types.clone();
                    let root = self.scoped_signature(*ty, level + 1, false)?;
                    self.scoped_types = saved;
                    let scheme = self.engine.generalize(root, level);
                    self.put(targets[index][0], scheme.clone());
                    annotated.insert(index, scheme);
                    continue;
                }
                for &binding in &targets[index] {
                    let root = if partial {
                        let Declaration::Value { arguments, .. } = definitions[index] else {
                            unreachable!()
                        };
                        let mut vars = Vec::new();
                        for _ in arguments {
                            vars.push(self.fresh(level + 1));
                        }
                        let result = self.fresh(level + 1);
                        let mut root = result;
                        for arg in vars.iter().rev() {
                            root = self.engine.term(Term::Function(*arg, root));
                        }
                        vars.push(result);
                        parameters.insert(index, vars);
                        root
                    } else {
                        self.fresh(level + 1)
                    };
                    self.put(binding, Self::mono(root));
                    placeholders.insert(binding, root);
                }
            }
            // Elm checks flexible recursive definitions under the declared
            // schemes first, then generalizes them before checking rigid bodies.
            // Otherwise an annotation's skolems can leak into an inferred peer.
            if partial {
                // recDefsHelp leaves the later prototypes at noRank until
                // their parameter groups are introduced. makeCopy can thus
                // instantiate these nodes before the whole SCC is generalized.
                for index in component.iter().skip(1) {
                    self.recursive_generics
                        .extend(parameters[index].iter().map(|ty| (*ty, level)));
                }
            }
            for checking_annotated in [false, true] {
                for &index in component
                    .iter()
                    .filter(|index| annotated.contains_key(index) == checking_annotated)
                {
                    let outcome = (|| -> Result<(), String> {
                        match definitions[index] {
                            Declaration::Value {
                                name,
                                arguments,
                                body,
                            } => {
                                let saved = self.scoped_types.clone();
                                let expected = if let Some(ty) = annotations.get(name) {
                                    Some(self.scoped_signature(*ty, level + 1, true)?)
                                } else {
                                    Some(placeholders[&targets[index][0]])
                                };
                                if annotated.contains_key(&index) {
                                    // The function's own recursive calls must obey its
                                    // rigid annotation while its body is checked.
                                    self.put(targets[index][0], Self::mono(expected.unwrap()));
                                }
                                let result = self.function(
                                    arguments,
                                    *body,
                                    level + 1,
                                    expected,
                                    annotations.contains_key(name).then_some(*name),
                                );
                                if let Some(scheme) = annotated.get(&index) {
                                    self.put(targets[index][0], scheme.clone());
                                }
                                self.scoped_types = saved;
                                result.map(|_| ())
                            }
                            Declaration::Destruct { pattern, body } => {
                                let p = self.pattern(*pattern, level + 1)?;
                                let expression = *body;
                                let body = self.expr(expression, level + 1)?;
                                if let Err(error) = self.engine.unify_annotation_diagnostic(
                                    p,
                                    body,
                                    self.source.symbols,
                                ) {
                                    if !self.engine.tracks_display_names() {
                                        return Err(error);
                                    }
                                    let mut context =
                                        crate::annotation_diagnostic::Context::body("");
                                    context.destructure = self.source.ast.expressions.iter().find_map(|node| {
                                        match &node.kind {
                                            Expr::Let(definitions, _) if definitions.iter().any(|definition|
                                                matches!(definition, Declaration::Destruct { pattern: found, .. } if found == pattern)) => Some(node.span),
                                            _ => None,
                                        }
                                    });
                                    let Some(report) = crate::annotation_diagnostic::capture(
                                        self.source,
                                        self.engine,
                                        &context,
                                        expression,
                                        body,
                                        p,
                                    ) else {
                                        return Err(error);
                                    };
                                    self.annotation_errors.push(report);
                                    self.engine.mark_diagnostic_error(p);
                                    self.engine.mark_diagnostic_error(body);
                                }
                                for &binding in &targets[index] {
                                    let actual = self.binding(binding, level + 1)?;
                                    self.engine.unify_diagnostic(
                                        placeholders[&binding],
                                        actual,
                                        self.source.symbols,
                                    )?;
                                }
                                Ok(())
                            }
                            _ => unreachable!(),
                        }
                    })();
                    let outcome = if let Some(report) = self.infinite_report() {
                        self.annotation_errors.push(report);
                        for binding in &targets[index] {
                            if let Some(root) = placeholders.get(binding) {
                                self.engine.mark_diagnostic_error(*root);
                            }
                        }
                        Ok(())
                    } else {
                        outcome
                    };
                    outcome.map_err(|error| {
                        let (name, body) = match definitions[index] {
                            Declaration::Value { name, body, .. } => (*name, *body),
                            Declaration::Destruct { body, .. } => ("destructuring", *body),
                            _ => unreachable!(),
                        };
                        let error = if crate::name_diagnostic::has_details(&error) {
                            error
                        } else {
                            format!("{error} (in `{name}`)")
                        };
                        crate::source_error::locate(
                            self.source.ast.source,
                            self.source.ast.expressions[body.0 as usize].span,
                            error,
                        )
                    })?;
                }
                if !checking_annotated {
                    for &index in component
                        .iter()
                        .filter(|index| !annotated.contains_key(index))
                    {
                        for &binding in &targets[index] {
                            let scheme = self.engine.generalize(placeholders[&binding], level);
                            self.put(binding, scheme);
                        }
                    }
                }
            }
            self.recursive_generics.truncate(generic_start);
        }
        Ok(())
    }
}

fn if_chain(ast: &crate::ast::Syntax<'_>, root: ExprId) -> (Vec<(ExprId, ExprId, ExprId)>, ExprId) {
    let mut chain = Vec::new();
    let mut cursor = root;
    while let Expr::If(condition, yes, no) = &ast.expressions[cursor.0 as usize].kind {
        chain.push((cursor, *condition, *yes));
        let gap = ast
            .source
            .get(
                ast.expressions[yes.0 as usize].span.end as usize
                    ..ast.expressions[no.0 as usize].span.start as usize,
            )
            .unwrap_or("");
        let parenthesized = crate::lexer::lex(gap)
            .ok()
            .is_some_and(|tokens| tokens.iter().any(|token| token.text(gap) == "("));
        cursor = *no;
        if parenthesized {
            break;
        }
    }
    (chain, cursor)
}
