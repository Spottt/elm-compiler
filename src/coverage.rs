//! Pattern usefulness and exhaustiveness (Maranget's matrix algorithm, also used
//! by Elm's Nitpick.PatternMatches). Flat pattern storage and explicit worklists
//! avoid recursion through long list/cons patterns.
use crate::{
    ast::{Declaration, Expr, Pattern, PatternId, Syntax},
    lexer::Kind,
    names::{Resolved, SymbolId, SymbolKind, Symbols},
};
use std::collections::{BTreeMap, BTreeSet};
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
struct Checker {
    patterns: Vec<Pat>,
    mapped: Vec<Option<usize>>,
    families: BTreeMap<Constructor, Vec<(Constructor, usize)>>,
}
use crate::literal::canonical_text;

impl Checker {
    fn new(ast: &Syntax<'_>, symbols: &Symbols) -> Self {
        let mut unions = BTreeMap::<SymbolId, Vec<(Constructor, usize)>>::new();
        for (index, symbol) in symbols.entries.iter().enumerate() {
            if let SymbolKind::Constructor {
                result,
                arity,
                record: false,
            } = symbol.kind
            {
                unions
                    .entry(result)
                    .or_default()
                    .push((Constructor::User(SymbolId(index as u32)), arity));
            }
        }
        let mut families = BTreeMap::new();
        for family in unions.values() {
            for (ctor, _) in family {
                families.insert(*ctor, family.clone());
            }
        }
        for family in [
            vec![(Constructor::Unit, 0)],
            vec![(Constructor::Tuple(2), 2)],
            vec![(Constructor::Tuple(3), 3)],
            vec![(Constructor::Nil, 0), (Constructor::Cons, 2)],
        ] {
            for (ctor, _) in &family {
                families.insert(*ctor, family.clone());
            }
        }
        Self {
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
                        for (ctor, arity) in family {
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
    fn check(
        &mut self,
        ast: &Syntax<'_>,
        resolved: &Resolved,
        patterns: &[PatternId],
    ) -> Result<(), String> {
        let mut matrix = Vec::new();
        for (id, pattern) in patterns.iter().enumerate() {
            let row = vec![self.simplify(ast, resolved, *pattern)?];
            if !self.useful(matrix.clone(), row.clone()) {
                return Err(format!(
                    "redundant pattern in branch {} at byte {}",
                    id + 1,
                    ast.patterns[pattern.0 as usize].span.start
                ));
            }
            matrix.push(row);
        }
        if self.useful(matrix, vec![0]) {
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
                        self.check(ast, resolved, &[*p])?;
                    }
                }
                Declaration::Destruct { pattern, .. } => self.check(ast, resolved, &[*pattern])?,
                _ => {}
            }
        }
        Ok(())
    }
}
pub fn check(ast: &Syntax<'_>, resolved: &Resolved, symbols: &Symbols) -> Result<(), String> {
    let mut checker = Checker::new(ast, symbols);
    checker.declarations(ast, resolved, &ast.declarations)?;
    for node in &ast.expressions {
        match &node.kind {
            Expr::Case(_, branches) => checker.check(
                ast,
                resolved,
                &branches.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
            )?,
            Expr::Lambda(args, _) => {
                for p in args {
                    checker.check(ast, resolved, &[*p])?;
                }
            }
            Expr::Let(declarations, _) => checker.declarations(ast, resolved, declarations)?,
            _ => {}
        }
    }
    Ok(())
}
