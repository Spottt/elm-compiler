//! Shared type graph for inference. Union-find links are mutable; structural
//! nodes are immutable and reference-counted so inspecting large records never
//! copies their fields. A failed unification invalidates this inference attempt;
//! callers must discard it, rather than continuing with partially solved state.
mod diagnostic;
mod fingerprint;
mod snapshot;
use crate::names::SymbolId;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap},
    rc::Rc,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ty(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    Any,
    Number,
    Comparable,
    Appendable,
    CompAppend,
}
impl Constraint {
    fn intersect(self, other: Self) -> Result<Self, String> {
        use Constraint::*;
        match (self, other) {
            (Any, x) | (x, Any) => Ok(x),
            (a, b) if a == b => Ok(a),
            (Number, Comparable) | (Comparable, Number) => Ok(Number),
            (Comparable, Appendable)
            | (Appendable, Comparable)
            | (Comparable, CompAppend)
            | (CompAppend, Comparable)
            | (Appendable, CompAppend)
            | (CompAppend, Appendable) => Ok(CompAppend),
            _ => Err(format!("incompatible constraints {self:?} and {other:?}")),
        }
    }
}
#[derive(Debug, Clone)]
pub enum Term {
    /// REPL-only nominal wrapper around a structurally checked alias body.
    Alias(SymbolId, Vec<Ty>, Ty),
    Named(SymbolId, Vec<Ty>),
    Function(Ty, Ty),
    Unit,
    Tuple(Vec<Ty>),
    Record {
        fields: BTreeMap<String, Ty>,
        extension: Option<Ty>,
    },
}
#[derive(Debug, Clone)]
enum Descriptor {
    Variable { level: u32, constraint: Constraint },
    Rigid { level: u32, constraint: Constraint },
    Structure(Rc<Term>),
}
#[derive(Debug)]
struct Node {
    parent: Ty,
    rank: u32,
    descriptor: Descriptor,
}
#[derive(Debug, Clone, Copy)]
pub struct Builtins {
    pub int: SymbolId,
    pub float: SymbolId,
    pub string: SymbolId,
    pub char: SymbolId,
    pub list: SymbolId,
}
#[derive(Debug, Clone)]
pub struct Scheme {
    pub root: Ty,
    pub quantified: BTreeSet<Ty>,
}
struct RecordRow<'a> {
    fields: Cow<'a, BTreeMap<String, Ty>>,
    extension: Option<Ty>,
}
#[derive(Debug)]
pub struct Engine {
    nodes: Vec<Node>,
    builtins: Builtins,
    display_names: Option<BTreeMap<Ty, String>>,
}
impl Engine {
    pub fn new(builtins: Builtins) -> Self {
        Self {
            nodes: Vec::new(),
            builtins,
            display_names: None,
        }
    }
    fn node(&mut self, descriptor: Descriptor) -> Ty {
        let id = Ty(self.nodes.len() as u32);
        self.nodes.push(Node {
            parent: id,
            rank: 0,
            descriptor,
        });
        id
    }
    pub fn variable(&mut self, level: u32, constraint: Constraint) -> Ty {
        self.node(Descriptor::Variable { level, constraint })
    }
    /// Enable source-name retention for REPL display only, before inference.
    pub fn track_display_names(&mut self) {
        self.display_names.get_or_insert_with(BTreeMap::new);
    }
    pub fn named_variable(&mut self, level: u32, constraint: Constraint, name: &str) -> Ty {
        let ty = self.variable(level, constraint);
        if let Some(names) = &mut self.display_names {
            names.insert(ty, name.into());
        }
        ty
    }
    fn copy_display_name(&mut self, from: Ty, to: Ty) {
        if let Some(names) = &mut self.display_names
            && let Some(name) = names.get(&from).cloned()
        {
            names.entry(to).or_insert(name);
        }
    }
    pub fn rigid(&mut self, level: u32, constraint: Constraint) -> Ty {
        self.node(Descriptor::Rigid { level, constraint })
    }
    pub fn alias(&mut self, name: SymbolId, args: Vec<Ty>, real: Ty) -> Ty {
        if self.display_names.is_some() {
            self.term(Term::Alias(name, args, real))
        } else {
            real
        }
    }
    pub fn term(&mut self, term: Term) -> Ty {
        self.node(Descriptor::Structure(Rc::new(term)))
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    /// Retain only graphs reachable from these external handles, rewriting all
    /// handles and structural edges. Call only after transient inference state
    /// has been dropped; handles not included in `roots` become invalid.
    pub fn compact(&mut self, roots: &mut [Ty]) {
        let mut relocation = vec![None; self.nodes.len()];
        let mut live = Vec::new();
        let mut pending = roots.to_vec();
        while let Some(ty) = pending.pop() {
            let ty = self.find(ty);
            if relocation[ty.0 as usize].is_some() {
                continue;
            }
            relocation[ty.0 as usize] = Some(Ty(live.len() as u32));
            live.push(ty);
            if let Descriptor::Structure(term) = &self.nodes[ty.0 as usize].descriptor {
                pending.extend(Self::children(term));
            }
        }
        // Release unreachable descriptors first, including union-find aliases
        // that may retain an Rc also owned by a live root. Parent links remain
        // intact until all child handles have been relocated.
        let empty = || Descriptor::Variable {
            level: 0,
            constraint: Constraint::Any,
        };
        for (index, node) in self.nodes.iter_mut().enumerate() {
            if relocation[index].is_none() {
                node.descriptor = empty();
            }
        }
        let mut nodes = Vec::with_capacity(live.len());
        for old in live {
            let descriptor = std::mem::replace(&mut self.nodes[old.0 as usize].descriptor, empty());
            let mut remap = |ty| relocation[self.find(ty).0 as usize].expect("marked type child");
            let descriptor = match descriptor {
                Descriptor::Structure(mut term) => {
                    // Usually uniquely owned now. Preserve an external immutable
                    // view if one exists, otherwise reuse record/string storage.
                    match Rc::make_mut(&mut term) {
                        Term::Alias(_, args, real) => {
                            for arg in args {
                                *arg = remap(*arg);
                            }
                            *real = remap(*real);
                        }
                        Term::Named(_, args) | Term::Tuple(args) => {
                            for ty in args {
                                *ty = remap(*ty);
                            }
                        }
                        Term::Function(a, b) => {
                            *a = remap(*a);
                            *b = remap(*b);
                        }
                        Term::Unit => {}
                        Term::Record { fields, extension } => {
                            for ty in fields.values_mut() {
                                *ty = remap(*ty);
                            }
                            *extension = extension.map(&mut remap);
                        }
                    }
                    Descriptor::Structure(term)
                }
                other => other,
            };
            let parent = Ty(nodes.len() as u32);
            nodes.push(Node {
                parent,
                rank: 0,
                descriptor,
            });
        }
        for root in roots {
            *root = relocation[self.find(*root).0 as usize].expect("marked type root");
        }
        if let Some(names) = self.display_names.take() {
            self.display_names = Some(
                names
                    .into_iter()
                    .filter_map(|(old, name)| relocation[old.0 as usize].map(|new| (new, name)))
                    .collect(),
            );
        }
        self.nodes = nodes;
    }
    pub fn find(&mut self, mut id: Ty) -> Ty {
        let start = id;
        while self.nodes[id.0 as usize].parent != id {
            id = self.nodes[id.0 as usize].parent;
        }
        let mut cursor = start;
        while cursor != id {
            let next = self.nodes[cursor.0 as usize].parent;
            self.nodes[cursor.0 as usize].parent = id;
            cursor = next;
        }
        id
    }
    pub fn structure(&mut self, mut id: Ty) -> Option<Rc<Term>> {
        loop {
            id = self.find(id);
            match &self.nodes[id.0 as usize].descriptor {
                Descriptor::Structure(t) => match &**t {
                    Term::Alias(_, _, real) => id = *real,
                    _ => return Some(t.clone()),
                },
                _ => return None,
            }
        }
    }
    fn children(term: &Term) -> Vec<Ty> {
        match term {
            Term::Alias(_, args, real) => args.iter().copied().chain([*real]).collect(),
            Term::Named(_, args) | Term::Tuple(args) => args.clone(),
            Term::Function(a, b) => vec![*a, *b],
            Term::Record { fields, extension } => fields
                .values()
                .copied()
                .chain(extension.iter().copied())
                .collect(),
            Term::Unit => vec![],
        }
    }
    fn occurs_and_lower(&mut self, var: Ty, root: Ty, level: u32) -> Result<(), String> {
        let mut pending = vec![root];
        let mut seen = BTreeSet::new();
        while let Some(id) = pending.pop() {
            let id = self.find(id);
            if id == var {
                return Err("infinite type (occurs check)".into());
            }
            if !seen.insert(id) {
                continue;
            }
            match self.nodes[id.0 as usize].descriptor.clone() {
                Descriptor::Variable {
                    level: old,
                    constraint,
                } => {
                    self.nodes[id.0 as usize].descriptor = Descriptor::Variable {
                        level: old.min(level),
                        constraint,
                    }
                }
                Descriptor::Structure(t) => pending.extend(Self::children(&t)),
                Descriptor::Rigid { level: scope, .. } => {
                    if scope > level {
                        return Err("annotation variable escapes its scope".into());
                    }
                }
            }
        }
        Ok(())
    }
    fn constrain(&mut self, root: Ty, constraint: Constraint) -> Result<(), String> {
        if constraint == Constraint::Any {
            return Ok(());
        }
        let mut pending = vec![(root, constraint)];
        let mut seen = BTreeSet::new();
        while let Some((id, constraint)) = pending.pop() {
            let id = self.find(id);
            if !seen.insert((id, constraint as u8)) {
                continue;
            }
            match self.nodes[id.0 as usize].descriptor.clone() {
                Descriptor::Variable {
                    level,
                    constraint: old,
                } => {
                    self.nodes[id.0 as usize].descriptor = Descriptor::Variable {
                        level,
                        constraint: old.intersect(constraint)?,
                    }
                }
                Descriptor::Rigid {
                    constraint: declared,
                    ..
                } => {
                    if declared.intersect(constraint)? != declared {
                        return Err(
                            "body requires a stronger constraint than the annotation".into()
                        );
                    }
                }
                Descriptor::Structure(term) => {
                    use Constraint::*;
                    match (&*term, constraint) {
                        (Term::Alias(_, _, real), _) => pending.push((*real, constraint)),
                        (Term::Named(name, args), Number)
                            if args.is_empty()
                                && (*name == self.builtins.int || *name == self.builtins.float) => {
                        }
                        (Term::Named(name, args), Comparable)
                            if args.is_empty()
                                && [
                                    self.builtins.int,
                                    self.builtins.float,
                                    self.builtins.string,
                                    self.builtins.char,
                                ]
                                .contains(name) => {}
                        (Term::Named(name, args), Appendable | CompAppend)
                            if args.is_empty() && *name == self.builtins.string => {}
                        (Term::Named(name, args), Appendable)
                            if *name == self.builtins.list && args.len() == 1 => {}
                        (Term::Named(name, args), Comparable | CompAppend)
                            if *name == self.builtins.list && args.len() == 1 =>
                        {
                            pending.push((args[0], Comparable))
                        }
                        (Term::Tuple(args), Comparable) if (2..=3).contains(&args.len()) => {
                            pending.extend(args.iter().map(|a| (*a, Comparable)))
                        }
                        _ => return Err(format!("type does not satisfy {constraint:?}")),
                    }
                }
            }
        }
        Ok(())
    }
    fn bind(
        &mut self,
        var: Ty,
        value: Ty,
        level: u32,
        constraint: Constraint,
    ) -> Result<(), String> {
        if constraint != Constraint::Any
            && let Descriptor::Structure(term) = &self.nodes[value.0 as usize].descriptor
            && let Term::Alias(_, _, real) = &**term
        {
            return self.bind(var, *real, level, constraint);
        }
        self.occurs_and_lower(var, value, level)?;
        self.constrain(value, constraint)?;
        if matches!(
            self.nodes[value.0 as usize].descriptor,
            Descriptor::Variable { .. } | Descriptor::Rigid { .. }
        ) {
            self.copy_display_name(var, value);
        }
        self.nodes[var.0 as usize].parent = value;
        Ok(())
    }
    pub fn unify(&mut self, left: Ty, right: Ty) -> Result<(), String> {
        self.unify_detail(left, right)
            .map_err(|(reason, _, _)| reason)
    }
    fn unify_detail(&mut self, left: Ty, right: Ty) -> Result<(), (String, Ty, Ty)> {
        let mut pending = vec![(left, right)];
        while let Some((left, right)) = pending.pop() {
            let a = self.find(left);
            let b = self.find(right);
            if a != b {
                self.unify_pair(a, b, &mut pending)
                    .map_err(|reason| (reason, a, b))?;
            }
        }
        Ok(())
    }
    fn unify_pair(&mut self, a: Ty, b: Ty, pending: &mut Vec<(Ty, Ty)>) -> Result<(), String> {
        let da = self.nodes[a.0 as usize].descriptor.clone();
        let db = self.nodes[b.0 as usize].descriptor.clone();
        match (da, db) {
            (
                Descriptor::Variable {
                    level: la,
                    constraint: ca,
                },
                Descriptor::Variable {
                    level: lb,
                    constraint: cb,
                },
            ) => {
                let constraint = ca.intersect(cb)?;
                let (root, child) =
                    if self.nodes[a.0 as usize].rank >= self.nodes[b.0 as usize].rank {
                        (a, b)
                    } else {
                        (b, a)
                    };
                if self.nodes[root.0 as usize].rank == self.nodes[child.0 as usize].rank {
                    self.nodes[root.0 as usize].rank += 1;
                }
                self.nodes[child.0 as usize].parent = root;
                if let Some(names) = &mut self.display_names {
                    let name = if ca == constraint {
                        names.get(&a).cloned()
                    } else {
                        None
                    }
                    .or_else(|| {
                        if cb == constraint {
                            names.get(&b).cloned()
                        } else {
                            None
                        }
                    });
                    names.remove(&a);
                    names.remove(&b);
                    if let Some(name) = name {
                        names.insert(root, name);
                    }
                }
                self.nodes[root.0 as usize].descriptor = Descriptor::Variable {
                    level: la.min(lb),
                    constraint,
                };
            }
            (Descriptor::Variable { level, constraint }, _) => {
                self.bind(a, b, level, constraint)?
            }
            (_, Descriptor::Variable { level, constraint }) => {
                self.bind(b, a, level, constraint)?
            }
            (Descriptor::Structure(term), Descriptor::Rigid { .. })
                if matches!(&*term, Term::Alias(..)) =>
            {
                let Term::Alias(_, _, real) = &*term else {
                    unreachable!()
                };
                pending.push((*real, b));
            }
            (Descriptor::Rigid { .. }, Descriptor::Structure(term))
                if matches!(&*term, Term::Alias(..)) =>
            {
                let Term::Alias(_, _, real) = &*term else {
                    unreachable!()
                };
                pending.push((a, *real));
            }
            (Descriptor::Rigid { .. }, _) | (_, Descriptor::Rigid { .. }) => {
                return Err("rigid annotation variable mismatch".into());
            }
            (Descriptor::Structure(ta), Descriptor::Structure(tb)) => {
                match (&*ta, &*tb) {
                    (Term::Alias(na, aa, _), Term::Alias(nb, ab, _))
                        if na == nb && aa.len() == ab.len() =>
                    {
                        pending.extend(aa.iter().copied().zip(ab.iter().copied()));
                    }
                    (Term::Alias(_, _, real), _) => {
                        pending.push((*real, b));
                        return Ok(());
                    }
                    (_, Term::Alias(_, _, real)) => {
                        pending.push((a, *real));
                        return Ok(());
                    }
                    (Term::Named(na, aa), Term::Named(nb, ab))
                        if na == nb && aa.len() == ab.len() =>
                    {
                        pending.extend(aa.iter().copied().zip(ab.iter().copied()))
                    }
                    (Term::Function(aa, ar), Term::Function(ba, br)) => {
                        pending.push((*aa, *ba));
                        pending.push((*ar, *br));
                    }
                    (Term::Unit, Term::Unit) => {}
                    (Term::Tuple(aa), Term::Tuple(ab)) if aa.len() == ab.len() => {
                        pending.extend(aa.iter().copied().zip(ab.iter().copied()))
                    }
                    (
                        Term::Record {
                            fields: fa,
                            extension: ea,
                        },
                        Term::Record {
                            fields: fb,
                            extension: eb,
                        },
                    ) => {
                        self.records(fa, *ea, fb, *eb, pending)?;
                        // Keep row graphs distinct: their extension equations
                        // carry constraints that must not be hidden by linking.
                        return Ok(());
                    }
                    _ => return Err("type constructor mismatch".into()),
                }
                self.nodes[b.0 as usize].parent = a;
            }
        }
        Ok(())
    }
    // Elm's gatherFields uses a left-biased union: an outer field wins over
    // an identically named field in an expanded extension. Borrow the common
    // flat case so comparing a large record does not clone its field names.
    fn gather_fields<'a>(
        &mut self,
        fields: &'a BTreeMap<String, Ty>,
        mut extension: Option<Ty>,
    ) -> Result<RecordRow<'a>, String> {
        let mut fields = Cow::Borrowed(fields);
        let mut seen = BTreeSet::new();
        while let Some(tail) = extension {
            let tail = self.find(tail);
            if !seen.insert(tail) {
                return Err("infinite record row".into());
            }
            match self.nodes[tail.0 as usize].descriptor.clone() {
                Descriptor::Variable { .. } | Descriptor::Rigid { .. } => {
                    return Ok(RecordRow {
                        fields,
                        extension: Some(tail),
                    });
                }
                Descriptor::Structure(term) => match &*term {
                    Term::Alias(_, _, real) => extension = Some(*real),
                    Term::Record {
                        fields: inner,
                        extension: next,
                    } => {
                        if !inner.is_empty() {
                            let gathered = fields.to_mut();
                            for (name, ty) in inner {
                                gathered.entry(name.clone()).or_insert(*ty);
                            }
                        }
                        extension = *next;
                    }
                    _ => return Err("record extension is not a record".into()),
                },
            }
        }
        Ok(RecordRow {
            fields,
            extension: None,
        })
    }
    fn records(
        &mut self,
        a: &BTreeMap<String, Ty>,
        ea: Option<Ty>,
        b: &BTreeMap<String, Ty>,
        eb: Option<Ty>,
        pending: &mut Vec<(Ty, Ty)>,
    ) -> Result<(), String> {
        let RecordRow {
            fields: a,
            extension: ea,
        } = self.gather_fields(a, ea)?;
        let RecordRow {
            fields: b,
            extension: eb,
        } = self.gather_fields(b, eb)?;
        let mut only_a = BTreeMap::new();
        let mut only_b = BTreeMap::new();
        for (field, ty) in a.iter() {
            if let Some(other) = b.get(field) {
                pending.push((*ty, *other));
            } else {
                only_a.insert(field.clone(), *ty);
            }
        }
        for (field, ty) in b.iter() {
            if !a.contains_key(field) {
                only_b.insert(field.clone(), *ty);
            }
        }
        match (ea, eb) {
            (None, None) => {
                if !only_a.is_empty() || !only_b.is_empty() {
                    return Err("record fields mismatch".into());
                }
            }
            (Some(tail), None) => {
                if !only_a.is_empty() {
                    return Err("closed record is missing required fields".into());
                }
                let row = self.term(Term::Record {
                    fields: only_b,
                    extension: None,
                });
                pending.push((tail, row));
            }
            (None, Some(tail)) => {
                if !only_b.is_empty() {
                    return Err("closed record is missing required fields".into());
                }
                let row = self.term(Term::Record {
                    fields: only_a,
                    extension: None,
                });
                pending.push((tail, row));
            }
            (Some(ta), Some(tb)) if only_a.is_empty() && only_b.is_empty() => {
                pending.push((ta, tb))
            }
            (Some(ta), Some(tb)) if only_a.is_empty() => {
                let row = self.term(Term::Record {
                    fields: only_b,
                    extension: Some(tb),
                });
                pending.push((ta, row));
            }
            (Some(ta), Some(tb)) if only_b.is_empty() => {
                let row = self.term(Term::Record {
                    fields: only_a,
                    extension: Some(ta),
                });
                pending.push((tb, row));
            }
            (Some(ta), Some(tb)) => {
                if self.find(ta) == self.find(tb) {
                    return Err("infinite record row".into());
                }
                // Level lowering during binding prevents this fresh tail from
                // escaping its environment or being incorrectly generalized.
                let tail = self.variable(u32::MAX, Constraint::Any);
                let ra = self.term(Term::Record {
                    fields: only_b,
                    extension: Some(tail),
                });
                let rb = self.term(Term::Record {
                    fields: only_a,
                    extension: Some(tail),
                });
                pending.push((ta, ra));
                pending.push((tb, rb));
            }
        }
        Ok(())
    }
    pub fn generalize(&mut self, root: Ty, environment_level: u32) -> Scheme {
        let mut pending = vec![root];
        let mut seen = BTreeSet::new();
        let mut quantified = BTreeSet::new();
        while let Some(id) = pending.pop() {
            let id = self.find(id);
            if !seen.insert(id) {
                continue;
            }
            match self.nodes[id.0 as usize].descriptor.clone() {
                Descriptor::Variable { level, .. } => {
                    if level > environment_level {
                        quantified.insert(id);
                    }
                }
                Descriptor::Structure(t) => pending.extend(Self::children(&t)),
                Descriptor::Rigid { .. } => {}
            }
        }
        Scheme { root, quantified }
    }
    pub fn instantiate(&mut self, scheme: &Scheme, level: u32) -> Ty {
        self.copy_scheme(scheme, level, false)
    }
    /// Copy intermediate generic nodes, stopping at every monomorphic node.
    /// Unlike a complete Scheme, generic children do not make their parent
    /// generic. The scope check keeps captured outer variables shared even
    /// after a marked variable is unified with a structure containing them.
    pub fn instantiate_partial(&mut self, root: Ty, markers: &[(Ty, u32)], level: u32) -> Ty {
        let mut generic = BTreeSet::new();
        for (ty, scope) in markers {
            let scheme = self.generalize(*ty, *scope);
            if self.is_closed_scheme(&scheme) {
                generic.insert(self.find(*ty));
            }
        }
        let mut mapped = HashMap::new();
        let mut pending = vec![(root, false)];
        while let Some((id, finish)) = pending.pop() {
            let id = self.find(id);
            if mapped.contains_key(&id) {
                continue;
            }
            if !generic.contains(&id) {
                mapped.insert(id, id);
                continue;
            }
            match self.nodes[id.0 as usize].descriptor.clone() {
                Descriptor::Variable { constraint, .. } => {
                    let value = self.variable(level, constraint);
                    self.copy_display_name(id, value);
                    mapped.insert(id, value);
                }
                Descriptor::Rigid { .. } => {
                    mapped.insert(id, id);
                }
                Descriptor::Structure(t) if !finish => {
                    pending.push((id, true));
                    pending.extend(Self::children(&t).into_iter().map(|c| (c, false)));
                }
                Descriptor::Structure(t) => {
                    let mut get = |old| mapped[&self.find(old)];
                    let term = match &*t {
                        Term::Alias(name, args, real) => {
                            Term::Alias(*name, args.iter().map(|a| get(*a)).collect(), get(*real))
                        }
                        Term::Named(n, args) => {
                            Term::Named(*n, args.iter().map(|a| get(*a)).collect())
                        }
                        Term::Function(a, b) => Term::Function(get(*a), get(*b)),
                        Term::Unit => Term::Unit,
                        Term::Tuple(args) => Term::Tuple(args.iter().map(|a| get(*a)).collect()),
                        Term::Record { fields, extension } => Term::Record {
                            fields: fields.iter().map(|(k, v)| (k.clone(), get(*v))).collect(),
                            extension: extension.map(get),
                        },
                    };
                    let value = self.term(term);
                    mapped.insert(id, value);
                }
            }
        }
        mapped[&self.find(root)]
    }
    /// Create independent universally quantified variables for checking an
    /// annotation. These must never be specialized by the implementation.
    pub fn skolemize(&mut self, scheme: &Scheme, level: u32) -> Ty {
        self.copy_scheme(scheme, level, true)
    }
    fn copy_scheme(&mut self, scheme: &Scheme, level: u32, rigid: bool) -> Ty {
        if scheme.quantified.is_empty() {
            return self.find(scheme.root);
        }
        self.copy_graph(
            scheme.root,
            &scheme.quantified,
            level,
            rigid,
            HashMap::new(),
        )
    }
    /// Apply an alias's positional type arguments without mutating its template.
    /// Unaffected structures and the supplied arguments remain shared. Callers
    /// are responsible for validating the alias arity and parameter constraints.
    pub fn substitute(&mut self, root: Ty, replacements: &[(Ty, Ty)]) -> Ty {
        if replacements.is_empty() {
            return self.find(root);
        }
        let mapped = replacements
            .iter()
            .map(|(parameter, value)| (self.find(*parameter), *value))
            .collect();
        self.copy_graph(root, &BTreeSet::new(), 0, false, mapped)
    }
    fn copy_graph(
        &mut self,
        root: Ty,
        quantified: &BTreeSet<Ty>,
        level: u32,
        rigid: bool,
        mut mapped: HashMap<Ty, Ty>,
    ) -> Ty {
        let mut pending = vec![(root, false)];
        while let Some((id, finish)) = pending.pop() {
            let id = self.find(id);
            if mapped.contains_key(&id) {
                continue;
            }
            match self.nodes[id.0 as usize].descriptor.clone() {
                Descriptor::Variable { constraint, .. } => {
                    let value = if quantified.contains(&id) {
                        if rigid {
                            self.node(Descriptor::Rigid { level, constraint })
                        } else {
                            self.variable(level, constraint)
                        }
                    } else {
                        id
                    };
                    self.copy_display_name(id, value);
                    mapped.insert(id, value);
                }
                Descriptor::Rigid { .. } => {
                    mapped.insert(id, id);
                }
                Descriptor::Structure(t) if !finish => {
                    pending.push((id, true));
                    pending.extend(Self::children(&t).into_iter().map(|c| (c, false)));
                }
                Descriptor::Structure(t) => {
                    // Reuse unchanged subgraphs before constructing a new term.
                    // In particular, do not copy a large monomorphic record
                    // just because another branch of the scheme is polymorphic.
                    let changed = Self::children(&t).iter().any(|child| {
                        let old = self.find(*child);
                        mapped[&old] != old
                    });
                    if !changed {
                        mapped.insert(id, id);
                        continue;
                    }
                    let mut get = |old: Ty| {
                        let old = self.find(old);
                        mapped[&old]
                    };
                    let term = match &*t {
                        Term::Alias(name, args, real) => {
                            Term::Alias(*name, args.iter().map(|a| get(*a)).collect(), get(*real))
                        }
                        Term::Named(n, args) => {
                            Term::Named(*n, args.iter().map(|a| get(*a)).collect())
                        }
                        Term::Function(a, b) => Term::Function(get(*a), get(*b)),
                        Term::Unit => Term::Unit,
                        Term::Tuple(args) => Term::Tuple(args.iter().map(|a| get(*a)).collect()),
                        Term::Record { fields, extension } => Term::Record {
                            fields: fields
                                .iter()
                                .map(|(name, ty)| (name.clone(), get(*ty)))
                                .collect(),
                            extension: extension.map(get),
                        },
                    };
                    let value = self.term(term);
                    mapped.insert(id, value);
                }
            }
        }
        let root = self.find(root);
        mapped[&root]
    }
}
