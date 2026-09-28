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
struct Infer<'a, 's> {
    source: SourceTypes<'a, 's>,
    module: &'a str,
    catalog: &'a Catalog,
    engine: &'a mut Engine,
    globals: &'a mut BTreeMap<SymbolId, Scheme>,
    locals: Vec<Option<Scheme>>,
    patterns: Vec<Option<Ty>>,
    expressions: Vec<Option<Ty>>,
    scoped_types: HashMap<&'s str, Ty>,
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
        catalog,
        engine,
        globals,
        locals: vec![None; source.resolved.locals.len()],
        patterns: vec![None; source.ast.patterns.len()],
        expressions: vec![None; source.ast.expressions.len()],
        scoped_types: HashMap::new(),
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
            crate::entry::check_port(infer.engine, source.symbols, scheme.root)
                .map_err(|e| format!("port {name}: {e}"))?;
            infer.globals.insert(id, scheme);
        }
    }
    infer.group(&source.ast.declarations, true, 0)?;
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
        if scheme.quantified.is_empty() && !self.recursive_generics.is_empty() {
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
                        lowered.insert(name.clone(), self.builtin(module, ty, vec![])?);
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
                pending.extend(pattern_children(pattern).into_iter().map(|p| (p, false)));
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
                        for ty in parts {
                            self.engine
                                .unify_diagnostic(item, ty, self.source.symbols)?;
                        }
                        self.builtin("elm/core:List", "List", vec![item])?
                    }
                    Pattern::Cons(a, b) => {
                        let (head, tail) = (get(*a), get(*b));
                        let list = self.builtin("elm/core:List", "List", vec![head])?;
                        self.engine
                            .unify_diagnostic(list, tail, self.source.symbols)?;
                        list
                    }
                    Pattern::Constructor(_, xs) => {
                        let args: Vec<_> = xs.iter().map(|p| get(*p)).collect();
                        let constructor = self.source.resolved.constructors[id.0 as usize]
                            .ok_or("unresolved constructor")?;
                        let fun = self.binding(Binding::Global(constructor), level)?;
                        self.apply(fun, args, level)?
                    }
                    Pattern::Record(fields) => {
                        let fields = fields
                            .iter()
                            .map(|name| (name.to_string(), self.fresh(level)))
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
                        fields[name]
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
            let ty = (|| -> Result<Ty, String> {
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
                        let f = get(*f);
                        let args: Vec<_> = args.iter().map(|a| get(*a)).collect();
                        self.apply(f, args, level)?
                    }
                    Expr::Binary(_, a, b) => {
                        let args = [get(*a), get(*b)];
                        let f = self.reference(id, level)?;
                        self.apply(f, args, level)?
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
                            fields: [(field.to_string(), value)].into(),
                            extension,
                        });
                        self.engine.term(Term::Function(record, value))
                    }
                    Expr::Access(base, field) => {
                        let base = get(*base);
                        let value = self.fresh(level);
                        let extension = Some(self.fresh(level));
                        let record = self.engine.term(Term::Record {
                            fields: [(field.to_string(), value)].into(),
                            extension,
                        });
                        self.engine
                            .unify_diagnostic(base, record, self.source.symbols)?;
                        value
                    }
                    Expr::Record { base, fields } => {
                        let fields = fields
                            .iter()
                            .map(|(name, id)| (name.to_string(), get(*id)))
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
            self.expressions[id.0 as usize] = Some(ty);
        }
        self.depth -= 1;
        Ok(self.expressions[root.0 as usize].unwrap())
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
                            self.engine.rigid(level, types::constraint(name))
                        } else {
                            self.engine.named_variable(level, types::constraint(name), name)
                        }
                    });
                }
                Type::Constructor(_, args) | Type::Tuple(args) => pending.extend(args),
                Type::Function(a, b) => pending.extend([*a, *b]),
                Type::Record { extension, fields } => {
                    if let Some(name) = extension {
                        self.scoped_types.entry(name).or_insert_with(|| {
                            if rigid {
                                self.engine.rigid(level, types::constraint(name))
                            } else {
                                self.engine.named_variable(level, types::constraint(name), name)
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
        let result = self.fresh(level);
        let mut fun = result;
        for arg in argument_types.iter().rev() {
            fun = self.engine.term(Term::Function(*arg, fun));
        }
        if let Some(expected) = expected {
            self.engine
                .unify_diagnostic(fun, expected, self.source.symbols)?;
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
        let body_type = self.expr(body, level)?;
        self.engine
            .unify_diagnostic(result, body_type, self.source.symbols)?;
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
        if components(&direct)
            .iter()
            .any(|group| group.len() > 1 || direct[group[0]].contains(&group[0]))
        {
            return Err("cyclic global value: immediate dependencies cannot be recursive".into());
        }
        Ok(())
    }
    fn group(
        &mut self,
        declarations: &'a [Declaration<'s>],
        top: bool,
        level: u32,
    ) -> Result<(), String> {
        let definitions: Vec<_> = declarations
            .iter()
            .filter(|d| matches!(d, Declaration::Value { .. } | Declaration::Destruct { .. }))
            .collect();
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
                return Err(
                    "cyclic local value: recursive let groups must contain only functions".into(),
                );
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
                                let result = self.function(arguments, *body, level + 1, expected);
                                if let Some(scheme) = annotated.get(&index) {
                                    self.put(targets[index][0], scheme.clone());
                                }
                                self.scoped_types = saved;
                                result.map(|_| ())
                            }
                            Declaration::Destruct { pattern, body } => {
                                let p = self.pattern(*pattern, level + 1)?;
                                let body = self.expr(*body, level + 1)?;
                                self.engine.unify_diagnostic(p, body, self.source.symbols)?;
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
                    outcome.map_err(|error| {
                        let (name, body) = match definitions[index] {
                            Declaration::Value { name, body, .. } => (*name, *body),
                            Declaration::Destruct { body, .. } => ("destructuring", *body),
                            _ => unreachable!(),
                        };
                        let error = format!("{error} (in `{name}`)");
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
