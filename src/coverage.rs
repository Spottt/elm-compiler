//! Pattern usefulness and exhaustiveness (Maranget's matrix algorithm, also used
//! by Elm's Nitpick.PatternMatches). Flat pattern storage and explicit worklists
//! avoid recursion through long list/cons patterns.
use crate::{
    ast::{Declaration, Expr, Pattern, PatternId, Span, Syntax},
    lexer::Kind,
    names::{Resolved, SymbolId, SymbolKind, Symbols},
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Constructor {
    Unit,
    Tuple(usize),
    Nil,
    Cons,
    User(SymbolId),
}
#[derive(Clone, PartialEq, Eq)]
enum Literal {
    Int(u64),
    Text(KindKey, String),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum KindKey {
    Char,
    String,
}
#[derive(Clone)]
enum Pat {
    Any,
    Literal(Literal),
    Constructor(Constructor, Vec<usize>),
}
struct Checker<'a> {
    symbols: &'a Symbols,
    path: Option<&'a std::path::Path>,
    patterns: Vec<Pat>,
    mapped: Vec<Option<usize>>,
    families: BTreeMap<Constructor, Rc<[(Constructor, usize)]>>,
}
use crate::literal::canonical_text;

impl<'a> Checker<'a> {
    fn new(
        ast: &Syntax<'_>,
        resolved: &Resolved,
        symbols: &'a Symbols,
        path: Option<&'a std::path::Path>,
    ) -> Self {
        // Only constructor families mentioned by source patterns can be used
        // by specialization. Keep every variant of those families, including
        // variants not mentioned by a branch, to detect missing alternatives.
        let required: BTreeSet<_> = resolved
            .constructors
            .iter()
            .flatten()
            .filter_map(|id| match symbols.get(*id).kind {
                SymbolKind::Constructor {
                    result,
                    record: false,
                    ..
                } => Some(result),
                _ => None,
            })
            .collect();
        let mut unions = BTreeMap::<SymbolId, Vec<(Constructor, usize)>>::new();
        for (index, symbol) in symbols.entries.iter().enumerate() {
            if let SymbolKind::Constructor {
                result,
                arity,
                record: false,
            } = symbol.kind
                && required.contains(&result)
            {
                unions
                    .entry(result)
                    .or_default()
                    .push((Constructor::User(SymbolId(index as u32)), arity));
            }
        }
        let mut families = BTreeMap::new();
        for family in unions.into_values() {
            let family: Rc<[_]> = family.into();
            for (ctor, _) in family.iter() {
                families.insert(*ctor, Rc::clone(&family));
            }
        }
        for family in [
            vec![(Constructor::Unit, 0)],
            vec![(Constructor::Tuple(2), 2)],
            vec![(Constructor::Tuple(3), 3)],
            vec![(Constructor::Nil, 0), (Constructor::Cons, 2)],
        ] {
            let family: Rc<[_]> = family.into();
            for (ctor, _) in family.iter() {
                families.insert(*ctor, Rc::clone(&family));
            }
        }
        Self {
            symbols,
            path,
            patterns: vec![Pat::Any],
            mapped: vec![None; ast.patterns.len()],
            families,
        }
    }
    fn add(&mut self, pat: Pat) -> usize {
        let id = self.patterns.len();
        self.patterns.push(pat);
        id
    }
    fn simplify(
        &mut self,
        ast: &Syntax<'_>,
        resolved: &Resolved,
        root: PatternId,
    ) -> Result<usize, String> {
        let mut pending = vec![(root, false)];
        while let Some((id, finish)) = pending.pop() {
            if self.mapped[id.0 as usize].is_some() {
                continue;
            }
            let p = &ast.patterns[id.0 as usize].kind;
            if !finish {
                pending.push((id, true));
                match p {
                    Pattern::Tuple(args) | Pattern::List(args) | Pattern::Constructor(_, args) => {
                        pending.extend(args.iter().map(|p| (*p, false)))
                    }
                    Pattern::Cons(a, b) => pending.extend([(*a, false), (*b, false)]),
                    Pattern::Alias(p, _) => pending.push((*p, false)),
                    _ => {}
                }
                continue;
            }
            let get = |p: PatternId| self.mapped[p.0 as usize].unwrap();
            let node = match p {
                Pattern::Wildcard | Pattern::Var(_) | Pattern::Record(_) => 0,
                Pattern::Alias(p, _) => get(*p),
                Pattern::Unit => self.add(Pat::Constructor(Constructor::Unit, vec![])),
                Pattern::Tuple(args) => self.add(Pat::Constructor(
                    Constructor::Tuple(args.len()),
                    args.iter().map(|p| get(*p)).collect(),
                )),
                Pattern::Constructor(_, args) => self.add(Pat::Constructor(
                    Constructor::User(
                        resolved.constructors[id.0 as usize]
                            .ok_or("unresolved pattern constructor")?,
                    ),
                    args.iter().map(|p| get(*p)).collect(),
                )),
                Pattern::Cons(a, b) => {
                    self.add(Pat::Constructor(Constructor::Cons, vec![get(*a), get(*b)]))
                }
                Pattern::List(args) => {
                    let parts: Vec<_> = args.iter().map(|p| get(*p)).collect();
                    let mut tail = self.add(Pat::Constructor(Constructor::Nil, vec![]));
                    for head in parts.into_iter().rev() {
                        tail = self.add(Pat::Constructor(Constructor::Cons, vec![head, tail]));
                    }
                    tail
                }
                Pattern::Literal(kind, raw) => {
                    let literal = match kind {
                        Kind::Number => Literal::Int(crate::literal::integer_value(raw)? as u64),
                        Kind::Char => Literal::Text(KindKey::Char, canonical_text(raw)?),
                        Kind::String => Literal::Text(KindKey::String, canonical_text(raw)?),
                        _ => return Err("invalid pattern literal".into()),
                    };
                    self.add(Pat::Literal(literal))
                }
            };
            self.mapped[id.0 as usize] = Some(node);
        }
        Ok(self.mapped[root.0 as usize].unwrap())
    }
    fn specialize(
        &self,
        matrix: &[Vec<usize>],
        ctor: Constructor,
        arity: usize,
    ) -> Vec<Vec<usize>> {
        matrix
            .iter()
            .filter_map(|row| {
                let mut result = match &self.patterns[row[0]] {
                    Pat::Any => vec![0; arity],
                    Pat::Constructor(c, args) if *c == ctor => args.clone(),
                    _ => return None,
                };
                result.extend_from_slice(&row[1..]);
                Some(result)
            })
            .collect()
    }
    fn useful(&self, matrix: Vec<Vec<usize>>, vector: Vec<usize>) -> bool {
        let mut pending = vec![(matrix, vector)];
        while let Some((matrix, vector)) = pending.pop() {
            if matrix.is_empty() {
                return true;
            }
            if vector.is_empty() {
                continue;
            }
            match &self.patterns[vector[0]] {
                Pat::Constructor(ctor, args) => {
                    let specialized = self.specialize(&matrix, *ctor, args.len());
                    let mut next = args.clone();
                    next.extend_from_slice(&vector[1..]);
                    pending.push((specialized, next));
                }
                Pat::Literal(literal) => {
                    let specialized = matrix
                        .iter()
                        .filter_map(|row| match &self.patterns[row[0]] {
                            Pat::Any => Some(row[1..].to_vec()),
                            Pat::Literal(l) if l == literal => Some(row[1..].to_vec()),
                            _ => None,
                        })
                        .collect();
                    pending.push((specialized, vector[1..].to_vec()));
                }
                Pat::Any => {
                    let constructors: BTreeSet<_> = matrix
                        .iter()
                        .filter_map(|row| {
                            if let Pat::Constructor(c, _) = &self.patterns[row[0]] {
                                Some(*c)
                            } else {
                                None
                            }
                        })
                        .collect();
                    let family = constructors
                        .first()
                        .and_then(|ctor| self.families.get(ctor));
                    if let Some(family) = family
                        .filter(|family| family.iter().all(|(ctor, _)| constructors.contains(ctor)))
                    {
                        for (ctor, arity) in family.iter() {
                            let mut next = vec![0; *arity];
                            next.extend_from_slice(&vector[1..]);
                            pending.push((self.specialize(&matrix, *ctor, *arity), next));
                        }
                    } else {
                        let defaults = matrix
                            .iter()
                            .filter(|row| matches!(self.patterns[row[0]], Pat::Any))
                            .map(|row| row[1..].to_vec())
                            .collect();
                        pending.push((defaults, vector[1..].to_vec()));
                    }
                }
            }
        }
        false
    }
    /// Recover missing rows using Elm's exhaustiveness algorithm. An explicit
    /// work stack keeps deeply nested source patterns off the Rust call stack.
    fn missing(&mut self, matrix: Vec<Vec<usize>>, columns: usize) -> Vec<Vec<usize>> {
        enum Work {
            Eval(Vec<Vec<usize>>, usize),
            Prefix(Vec<usize>),
            Recover(Constructor, usize),
            Combine(usize),
        }
        let mut pending = vec![Work::Eval(matrix, columns)];
        let mut results: Vec<Vec<Vec<usize>>> = Vec::new();
        while let Some(work) = pending.pop() {
            match work {
                Work::Eval(matrix, n) => {
                    if matrix.is_empty() {
                        results.push(vec![vec![0; n]]);
                    } else if n == 0 {
                        results.push(vec![]);
                    } else {
                        let seen: BTreeSet<_> = matrix
                            .iter()
                            .filter_map(|row| match self.patterns[row[0]] {
                                Pat::Constructor(c, _) => Some(c),
                                _ => None,
                            })
                            .collect();
                        let defaults = || {
                            matrix
                                .iter()
                                .filter(|row| matches!(self.patterns[row[0]], Pat::Any))
                                .map(|row| row[1..].to_vec())
                                .collect()
                        };
                        let family = seen.first().and_then(|c| self.families.get(c)).cloned();
                        match family {
                            None => {
                                pending.push(Work::Prefix(vec![0]));
                                pending.push(Work::Eval(defaults(), n - 1));
                            }
                            Some(family) if seen.len() < family.len() => {
                                let heads = family
                                    .iter()
                                    .copied()
                                    .filter(|(c, _)| !seen.contains(c))
                                    .map(|(c, arity)| self.add(Pat::Constructor(c, vec![0; arity])))
                                    .collect();
                                pending.push(Work::Prefix(heads));
                                pending.push(Work::Eval(
                                    matrix
                                        .iter()
                                        .filter(|row| matches!(self.patterns[row[0]], Pat::Any))
                                        .map(|row| row[1..].to_vec())
                                        .collect(),
                                    n - 1,
                                ));
                            }
                            Some(family) => {
                                pending.push(Work::Combine(family.len()));
                                for (ctor, arity) in family.iter().copied().rev() {
                                    pending.push(Work::Recover(ctor, arity));
                                    pending.push(Work::Eval(
                                        self.specialize(&matrix, ctor, arity),
                                        arity + n - 1,
                                    ));
                                }
                            }
                        }
                    }
                }
                Work::Prefix(heads) => {
                    let rows = results.pop().unwrap();
                    results.push(
                        heads
                            .into_iter()
                            .flat_map(|head| {
                                rows.iter().map(move |row| {
                                    let mut result = vec![head];
                                    result.extend(row);
                                    result
                                })
                            })
                            .collect(),
                    );
                }
                Work::Recover(ctor, arity) => {
                    let rows = results.pop().unwrap();
                    results.push(
                        rows.into_iter()
                            .map(|row| {
                                let head = self.add(Pat::Constructor(ctor, row[..arity].to_vec()));
                                let mut result = vec![head];
                                result.extend_from_slice(&row[arity..]);
                                result
                            })
                            .collect(),
                    );
                }
                Work::Combine(n) => {
                    let rows = results
                        .split_off(results.len() - n)
                        .into_iter()
                        .flatten()
                        .collect();
                    results.push(rows);
                }
            }
        }
        results.pop().unwrap()
    }
    fn pattern_text(&self, root: usize) -> String {
        enum Work {
            Pattern(usize, u8),
            Text(String),
        }
        let mut pending = vec![Work::Pattern(root, 0)];
        let mut out = String::new();
        while let Some(work) = pending.pop() {
            let Work::Pattern(id, context) = work else {
                if let Work::Text(text) = work {
                    out.push_str(&text);
                }
                continue;
            };
            let mut tail = id;
            let mut heads = Vec::new();
            while let Pat::Constructor(Constructor::Cons, args) = &self.patterns[tail] {
                heads.push(args[0]);
                tail = args[1];
            }
            let mut sequence = Vec::new();
            if matches!(self.patterns[tail], Pat::Constructor(Constructor::Nil, _)) {
                sequence.push(Work::Text("[".into()));
                // Elm 0.19.1's delist returns finite entries in reverse order.
                for (index, head) in heads.into_iter().rev().enumerate() {
                    if index > 0 {
                        sequence.push(Work::Text(",".into()));
                    }
                    sequence.push(Work::Pattern(head, 0));
                }
                sequence.push(Work::Text("]".into()));
            } else if !heads.is_empty() {
                if context != 0 {
                    sequence.push(Work::Text("(".into()));
                }
                for head in heads {
                    sequence.push(Work::Pattern(head, 2));
                    sequence.push(Work::Text(" :: ".into()));
                }
                sequence.push(Work::Pattern(tail, 0));
                if context != 0 {
                    sequence.push(Work::Text(")".into()));
                }
            } else {
                match &self.patterns[id] {
                    Pat::Any => sequence.push(Work::Text("_".into())),
                    Pat::Constructor(Constructor::Unit, _) => {
                        sequence.push(Work::Text("()".into()))
                    }
                    Pat::Constructor(Constructor::Tuple(_), args) => {
                        sequence.push(Work::Text("( ".into()));
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                sequence.push(Work::Text(", ".into()));
                            }
                            sequence.push(Work::Pattern(*arg, 0));
                        }
                        sequence.push(Work::Text(" )".into()));
                    }
                    Pat::Constructor(Constructor::User(symbol), args) => {
                        let parens = context == 1 && !args.is_empty();
                        if parens {
                            sequence.push(Work::Text("(".into()));
                        }
                        sequence.push(Work::Text(self.symbols.get(*symbol).name.to_string()));
                        for arg in args {
                            sequence.push(Work::Text(" ".into()));
                            sequence.push(Work::Pattern(*arg, 1));
                        }
                        if parens {
                            sequence.push(Work::Text(")".into()));
                        }
                    }
                    _ => unreachable!("missing witnesses only contain constructors and wildcards"),
                }
            }
            pending.extend(sequence.into_iter().rev());
        }
        out
    }
    fn check(
        &mut self,
        ast: &Syntax<'_>,
        resolved: &Resolved,
        patterns: &[PatternId],
        span: Span,
        context: &str,
    ) -> Result<(), String> {
        let mut matrix = Vec::new();
        for (id, pattern) in patterns.iter().enumerate() {
            let row = vec![self.simplify(ast, resolved, *pattern)?];
            if !self.useful(matrix.clone(), row.clone()) {
                if let Some(path) = self.path {
                    return Err(crate::coverage_diagnostic::redundant(
                        ast,
                        path,
                        span,
                        ast.patterns[pattern.0 as usize].span,
                        id + 1,
                    ));
                }
                return Err(format!(
                    "redundant pattern in branch {} at byte {}",
                    id + 1,
                    ast.patterns[pattern.0 as usize].span.start
                ));
            }
            matrix.push(row);
        }
        if self.useful(matrix, vec![0]) {
            if let Some(path) = self.path {
                let matrix = patterns
                    .iter()
                    .map(|p| vec![self.mapped[p.0 as usize].unwrap()])
                    .collect();
                let witnesses = self
                    .missing(matrix, 1)
                    .iter()
                    .map(|row| self.pattern_text(row[0]))
                    .collect::<Vec<_>>();
                return Err(crate::coverage_diagnostic::missing(
                    ast, path, span, context, &witnesses,
                ));
            }
            return Err("missing patterns (non-exhaustive match)".into());
        }
        Ok(())
    }
    fn declarations(
        &mut self,
        ast: &Syntax<'_>,
        resolved: &Resolved,
        declarations: &[Declaration<'_>],
    ) -> Result<(), String> {
        for declaration in declarations {
            match declaration {
                Declaration::Value { arguments, .. } => {
                    for p in arguments {
                        self.check(
                            ast,
                            resolved,
                            &[*p],
                            ast.patterns[p.0 as usize].span,
                            "argument",
                        )?;
                    }
                }
                Declaration::Destruct { pattern, .. } => self.check(
                    ast,
                    resolved,
                    &[*pattern],
                    ast.patterns[pattern.0 as usize].span,
                    "destruct",
                )?,
                _ => {}
            }
        }
        Ok(())
    }
}
pub fn check(ast: &Syntax<'_>, resolved: &Resolved, symbols: &Symbols) -> Result<(), String> {
    check_inner(ast, resolved, symbols, None)
}
pub fn check_report(
    ast: &Syntax<'_>,
    resolved: &Resolved,
    symbols: &Symbols,
    path: &std::path::Path,
) -> Result<(), String> {
    match check(ast, resolved, symbols) {
        Ok(()) => Ok(()),
        Err(original) => match aggregate_report(ast, resolved, symbols, path) {
            Ok(()) => Err(original),
            result => result,
        },
    }
}
fn check_inner(
    ast: &Syntax<'_>,
    resolved: &Resolved,
    symbols: &Symbols,
    path: Option<&std::path::Path>,
) -> Result<(), String> {
    let mut checker = Checker::new(ast, resolved, symbols, path);
    checker.declarations(ast, resolved, &ast.declarations)?;
    for node in &ast.expressions {
        match &node.kind {
            Expr::Case(_, branches) => checker.check(
                ast,
                resolved,
                &branches.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
                node.span,
                "case",
            )?,
            Expr::Lambda(args, _) => {
                for p in args {
                    checker.check(
                        ast,
                        resolved,
                        &[*p],
                        ast.patterns[p.0 as usize].span,
                        "argument",
                    )?;
                }
            }
            Expr::Let(declarations, _) => checker.declarations(ast, resolved, declarations)?,
            _ => {}
        }
    }
    Ok(())
}

fn aggregate_report(
    ast: &Syntax<'_>,
    resolved: &Resolved,
    symbols: &Symbols,
    path: &std::path::Path,
) -> Result<(), String> {
    use crate::ast::ExprId;
    enum Work<'a, 's> {
        Decl(&'a Declaration<'s>),
        Expr(ExprId),
        Check(Vec<PatternId>, Span, &'static str),
    }
    let mut checker = Checker::new(ast, resolved, symbols, Some(path));
    let mut pending: Vec<_> = ordered_declarations(ast, resolved, symbols, &ast.declarations, true)
        .into_iter()
        .rev()
        .map(Work::Decl)
        .collect();
    let mut problems = Vec::new();
    while let Some(work) = pending.pop() {
        match work {
            Work::Check(patterns, span, context) => {
                if let Err(error) = checker.check(ast, resolved, &patterns, span, context) {
                    let Some(report) = crate::docs_diagnostic::report_encoded(&error) else {
                        return Err(error);
                    };
                    problems.extend(
                        report["errors"][0]["problems"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .cloned(),
                    );
                }
            }
            Work::Decl(declaration) => match declaration {
                Declaration::Value {
                    arguments, body, ..
                } => {
                    pending.push(Work::Expr(*body));
                    for p in arguments.iter().rev() {
                        pending.push(Work::Check(
                            vec![*p],
                            ast.patterns[p.0 as usize].span,
                            "argument",
                        ));
                    }
                }
                Declaration::Destruct { pattern, body } => {
                    pending.push(Work::Expr(*body));
                    pending.push(Work::Check(
                        vec![*pattern],
                        ast.patterns[pattern.0 as usize].span,
                        "destruct",
                    ));
                }
                _ => {}
            },
            Work::Expr(id) => {
                let node = &ast.expressions[id.0 as usize];
                match &node.kind {
                    Expr::Case(subject, branches) => {
                        pending.extend(branches.iter().rev().map(|(_, body)| Work::Expr(*body)));
                        pending.push(Work::Check(
                            branches.iter().map(|(p, _)| *p).collect(),
                            node.span,
                            "case",
                        ));
                        pending.push(Work::Expr(*subject));
                    }
                    Expr::Lambda(args, body) => {
                        pending.push(Work::Expr(*body));
                        for p in args.iter().rev() {
                            pending.push(Work::Check(
                                vec![*p],
                                ast.patterns[p.0 as usize].span,
                                "argument",
                            ));
                        }
                    }
                    Expr::Let(declarations, body) => {
                        pending.push(Work::Expr(*body));
                        pending.extend(
                            ordered_declarations(ast, resolved, symbols, declarations, false)
                                .into_iter()
                                .rev()
                                .map(Work::Decl),
                        );
                    }
                    Expr::Record { fields, .. } => {
                        let mut fields: Vec<_> = fields.iter().collect();
                        fields.sort_by_key(|(name, _)| *name);
                        pending.extend(
                            fields
                                .into_iter()
                                .rev()
                                .map(|(_, value)| Work::Expr(*value)),
                        );
                    }
                    expression => pending.extend(
                        crate::infer::expr_children(expression)
                            .into_iter()
                            .rev()
                            .map(Work::Expr),
                    ),
                }
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(crate::docs_diagnostic::encode(
            &serde_json::json!({"type":"compile-errors","errors":[{
                "path":path,"name":ast.header.name,"problems":problems
            }]}),
        ))
    }
}

/// Data.Graph visits declaration keys in lexical order on the transpose.
/// This ordering is only needed when presenting failures, never on a good build.
pub(crate) fn ordered_declarations<'a, 's>(
    ast: &Syntax<'_>,
    resolved: &Resolved,
    symbols: &Symbols,
    declarations: &'a [Declaration<'s>],
    top: bool,
) -> Vec<&'a Declaration<'s>> {
    if !top
        && declarations
            .iter()
            .any(|d| matches!(d, Declaration::Destruct { .. }))
    {
        return ordered_destructuring_let(ast, resolved, declarations);
    }
    use crate::names::Binding;
    let mut definitions: Vec<_> = declarations
        .iter()
        .filter(|d| matches!(d, Declaration::Value { .. } | Declaration::Destruct { .. }))
        .collect();
    definitions.sort_by_key(|d| match d {
        Declaration::Value { name, .. } => *name,
        _ => "",
    });
    let mut owners = BTreeMap::new();
    let mut names = BTreeMap::new();
    for (index, declaration) in definitions.iter().enumerate() {
        match declaration {
            Declaration::Value { name, body, .. } => {
                names.insert(*name, index);
                if !top && let Some(local) = resolved.definitions.get(body) {
                    owners.insert(Binding::Local(*local), index);
                }
            }
            Declaration::Destruct { pattern, .. } => {
                if let Some(bindings) = resolved.pattern_bindings.get(pattern) {
                    for (_, local) in bindings {
                        owners.insert(Binding::Local(*local), index);
                    }
                }
            }
            _ => {}
        }
    }
    let mut transpose = vec![BTreeSet::new(); definitions.len()];
    let mut direct = transpose.clone();
    for (index, declaration) in definitions.iter().enumerate() {
        let body = match declaration {
            Declaration::Value { body, .. } | Declaration::Destruct { body, .. } => *body,
            _ => unreachable!(),
        };
        let delayed =
            matches!(declaration, Declaration::Value { arguments, .. } if !arguments.is_empty());
        let mut pending = vec![(body, delayed)];
        while let Some((id, delayed)) = pending.pop() {
            if let Some(binding) = resolved.expressions[id.0 as usize] {
                let target = match binding {
                    Binding::Global(id) if top => {
                        let symbol = symbols.get(id);
                        if symbol.module.split(':').next_back() == Some(ast.header.name.as_str()) {
                            names.get(symbol.name.as_ref()).copied()
                        } else {
                            None
                        }
                    }
                    _ => owners.get(&binding).copied(),
                };
                if let Some(target) = target {
                    transpose[target].insert(index);
                    if !delayed {
                        direct[target].insert(index);
                    }
                }
            }
            let expression = &ast.expressions[id.0 as usize].kind;
            if let Expr::Let(declarations, body) = expression {
                pending.push((*body, delayed));
                for declaration in declarations {
                    match declaration {
                        Declaration::Value {
                            body, arguments, ..
                        } => pending.push((*body, delayed || !arguments.is_empty())),
                        Declaration::Destruct { body, .. } => pending.push((*body, delayed)),
                        _ => {}
                    }
                }
            } else {
                let delayed = delayed || matches!(expression, Expr::Lambda(..));
                pending.extend(
                    crate::infer::expr_children(expression)
                        .into_iter()
                        .map(|id| (id, delayed)),
                );
            }
        }
    }
    let mut ordered = Vec::new();
    for mut group in crate::infer::components(&transpose).into_iter().rev() {
        if group.len() > 1 && top {
            group.sort_unstable();
            let ranks: BTreeMap<_, _> = group
                .iter()
                .enumerate()
                .map(|(rank, index)| (*index, rank))
                .collect();
            let edges: Vec<BTreeSet<usize>> = group
                .iter()
                .map(|index| {
                    direct[*index]
                        .iter()
                        .filter_map(|target| ranks.get(target).copied())
                        .collect()
                })
                .collect();
            for inner in crate::infer::components(&edges).into_iter().rev().flatten() {
                ordered.push(definitions[group[inner]]);
            }
        } else {
            if !top && group.len() > 1 {
                group.reverse();
            }
            ordered.extend(group.into_iter().map(|index| definitions[index]));
        }
    }
    ordered
}

// A destructuring let introduces one graph node for the expression and an
// additional edge node for each bound name (Canonicalize.Expression.addDefNodes).
fn ordered_destructuring_let<'a, 's>(
    ast: &Syntax<'_>,
    resolved: &Resolved,
    declarations: &'a [Declaration<'s>],
) -> Vec<&'a Declaration<'s>> {
    use crate::names::Binding;
    struct Node<'a, 's> {
        key: String,
        declaration: Option<&'a Declaration<'s>>,
        binding: Option<Binding>,
        edge: Option<String>,
    }
    let mut nodes = Vec::new();
    for declaration in declarations {
        match declaration {
            Declaration::Value { name, body, .. } => nodes.push(Node {
                key: name.to_string(),
                declaration: Some(declaration),
                binding: resolved.definitions.get(body).copied().map(Binding::Local),
                edge: None,
            }),
            Declaration::Destruct { pattern, .. } => {
                let mut names = std::collections::VecDeque::new();
                let mut bindings = Vec::new();
                let mut pending = vec![*pattern];
                while let Some(id) = pending.pop() {
                    if let Some(bound) = resolved.pattern_bindings.get(&id) {
                        bindings.extend(bound.iter().cloned());
                    }
                    match &ast.patterns[id.0 as usize].kind {
                        Pattern::Var(name) | Pattern::Alias(_, name) => names.push_front(*name),
                        Pattern::Record(fields) => {
                            for name in fields.iter().rev() {
                                names.push_front(*name);
                            }
                        }
                        _ => {}
                    }
                    pending.extend(
                        crate::infer::pattern_children(&ast.patterns[id.0 as usize].kind)
                            .into_iter()
                            .rev(),
                    );
                }
                let key = format!("_M${}", names.front().copied().unwrap_or(""));
                nodes.push(Node {
                    key: key.clone(),
                    declaration: Some(declaration),
                    binding: None,
                    edge: None,
                });
                for (name, local) in bindings {
                    nodes.push(Node {
                        key: name,
                        declaration: None,
                        binding: Some(Binding::Local(local)),
                        edge: Some(key.clone()),
                    });
                }
            }
            _ => {}
        }
    }
    nodes.sort_by(|a, b| a.key.cmp(&b.key));
    let keys: BTreeMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.key.as_str(), i))
        .collect();
    let owners: BTreeMap<_, _> = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, node)| node.binding.map(|b| (b, i)))
        .collect();
    let mut transpose = vec![BTreeSet::new(); nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        if let Some(edge) = &node.edge {
            transpose[keys[edge.as_str()]].insert(index);
        }
        let body = match node.declaration {
            Some(Declaration::Value { body, .. } | Declaration::Destruct { body, .. }) => *body,
            _ => continue,
        };
        let mut pending = vec![body];
        while let Some(id) = pending.pop() {
            if let Some(binding) = resolved.expressions[id.0 as usize]
                && let Some(target) = owners.get(&binding)
            {
                transpose[*target].insert(index);
            }
            pending.extend(crate::infer::expr_children(
                &ast.expressions[id.0 as usize].kind,
            ));
        }
    }
    let mut ordered = Vec::new();
    for mut group in crate::infer::components(&transpose).into_iter().rev() {
        if group.len() > 1 {
            group.reverse();
        }
        ordered.extend(group.into_iter().filter_map(|i| nodes[i].declaration));
    }
    ordered
}
