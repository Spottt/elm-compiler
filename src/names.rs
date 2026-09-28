//! Module-qualified symbols and lexical bindings. No type inference is performed
//! here: every reference is connected to an identity used by subsequent passes.
use crate::{
    ast::*,
    module::{Effects, Exposed, Exposing},
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    rc::Rc,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Binding {
    Global(SymbolId),
    Local(LocalId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Space {
    Value,
    Type,
    Constructor,
}
#[derive(Debug, Clone)]
pub enum SymbolKind {
    Value,
    Type {
        arity: usize,
        alias: bool,
    },
    Constructor {
        result: SymbolId,
        arity: usize,
        record: bool,
    },
    Kernel,
}
#[derive(Debug, Clone)]
pub struct Symbol {
    pub module: Rc<str>,
    pub name: Rc<str>,
    pub kind: SymbolKind,
}
#[derive(Default, Clone)]
pub struct Symbols {
    pub entries: Vec<Symbol>,
    strings: HashMap<String, Rc<str>>,
    index: HashMap<(Rc<str>, Rc<str>, Space), SymbolId>,
    by_module: HashMap<Rc<str>, Vec<SymbolId>>,
}
impl Symbols {
    fn string(&mut self, s: &str) -> Rc<str> {
        if let Some(interned) = self.strings.get(s) {
            return interned.clone();
        }
        self.strings
            .entry(s.into())
            .or_insert_with(|| Rc::from(s))
            .clone()
    }
    pub fn intern(&mut self, module: &str, name: &str, space: Space, kind: SymbolKind) -> SymbolId {
        let module = self.string(module);
        let name = self.string(name);
        let key = (module.clone(), name.clone(), space);
        if let Some(id) = self.index.get(&key) {
            return *id;
        }
        let id = SymbolId(self.entries.len() as u32);
        self.by_module.entry(module.clone()).or_default().push(id);
        self.entries.push(Symbol { module, name, kind });
        self.index.insert(key, id);
        id
    }
    /// IDs in insertion order, including all namespaces and non-contiguous IDs.
    pub(crate) fn module_symbols(&self, module: &str) -> &[SymbolId] {
        self.by_module.get(module).map(Vec::as_slice).unwrap_or_default()
    }
    pub fn get(&self, id: SymbolId) -> &Symbol {
        &self.entries[id.0 as usize]
    }
    pub fn lookup(&self, module: &str, name: &str, space: Space) -> Option<SymbolId> {
        // Both components were interned when the symbol was inserted. Reuse
        // their storage instead of allocating two temporary strings per lookup.
        let module = self.strings.get(module)?;
        let name = self.strings.get(name)?;
        self.index.get(&(module.clone(), name.clone(), space)).copied()
    }
}
#[derive(Debug, Clone, Default)]
pub struct Interface {
    pub values: BTreeMap<String, SymbolId>,
    pub types: BTreeMap<String, SymbolId>,
    pub constructors: BTreeMap<String, SymbolId>,
    pub operators: BTreeMap<String, SymbolId>,
}
impl Interface {
    fn namespace(&self, space: Space) -> &BTreeMap<String, SymbolId> {
        match space {
            Space::Value => &self.values,
            Space::Type => &self.types,
            Space::Constructor => &self.constructors,
        }
    }
}
// Almost every imported name has one binding. Allocate a set only when
// different imports actually make a name ambiguous.
#[derive(Clone)]
enum Bindings {
    One(SymbolId),
    Many(BTreeSet<SymbolId>),
}
type Candidates = BTreeMap<String, Bindings>;
#[derive(Clone, Default)]
pub struct Environment {
    values: Candidates,
    types: Candidates,
    constructors: Candidates,
    operators: Candidates,
    // Keep each imported interface once; expand qualified names only for
    // diagnostics. Cloning an environment shares these immutable interfaces.
    qualified: BTreeMap<String, Vec<Rc<Interface>>>,
    // An imported module exists even when one namespace has no exposed names.
    imported_prefixes: BTreeSet<String>,
    kernel: BTreeMap<String, String>,
    allow_kernel: bool,
}
impl Bindings {
    fn add(&mut self, id: SymbolId) {
        match self {
            Self::One(previous) if *previous != id => {
                *self = Self::Many(BTreeSet::from([*previous, id]));
            }
            Self::One(_) => {}
            Self::Many(ids) => { ids.insert(id); }
        }
    }
}
fn merge(target: &mut Candidates, name: &str, id: SymbolId) {
    target.entry(name.into()).and_modify(|bindings| bindings.add(id)).or_insert(Bindings::One(id));
}
fn replace(target: &mut Candidates, name: &str, id: SymbolId) {
    target.insert(name.into(), Bindings::One(id));
}
fn unique(map: &Candidates, name: &str, symbols: &Symbols, thing: &str) -> Result<SymbolId, String> {
    unique_bindings(map.get(name).ok_or_else(|| format!("unknown name {name}"))?, name, symbols, thing)
}
fn unique_bindings(
    candidates: &Bindings,
    name: &str,
    symbols: &Symbols,
    thing: &str,
) -> Result<SymbolId, String> {
    match candidates {
        Bindings::One(id) => Ok(*id),
        Bindings::Many(candidates) => {
            let mut choices = candidates
                .iter()
                .map(|id| {
                    let symbol = symbols.get(*id);
                    let module = symbol.module.rsplit(':').next().unwrap_or(&symbol.module);
                    format!("{module}.{}", symbol.name)
                })
                .collect::<Vec<_>>();
            choices.sort();
            choices.dedup();
            let homes = choices
                .iter()
                .filter_map(|choice| choice.rsplit_once('.').map(|(home, _)| home.to_owned()))
                .collect();
            Err(crate::name_diagnostic::ambiguous(
                name, thing, homes, &choices,
            ))
        }
    }
}
fn insert(map: &mut BTreeMap<String, SymbolId>, name: &str, id: SymbolId) -> Result<(), String> {
    if map.insert(name.into(), id).is_some() {
        return Err(format!("duplicate declaration {name}"));
    }
    Ok(())
}
impl Environment {
    pub fn import(
        &mut self,
        prefix: &str,
        foreign: &Interface,
        exposing: &Exposing,
        symbols: &Symbols,
    ) -> Result<(), String> {
        self.import_shared(prefix, Rc::new(foreign.clone()), exposing, symbols)
    }
    pub fn import_shared(
        &mut self,
        prefix: &str,
        foreign: Rc<Interface>,
        exposing: &Exposing,
        symbols: &Symbols,
    ) -> Result<(), String> {
        self.imported_prefixes.insert(prefix.into());
        self.qualified.entry(prefix.into()).or_default().push(foreign.clone());
        match exposing {
            Exposing::All => {
                for (src, dst) in [
                    (&foreign.values, &mut self.values),
                    (&foreign.types, &mut self.types),
                    (&foreign.constructors, &mut self.constructors),
                    (&foreign.operators, &mut self.operators),
                ] {
                    for (name, id) in src {
                        merge(dst, name, *id);
                    }
                }
            }
            Exposing::Explicit(items) => {
                for item in items {
                    match item {
                        Exposed::Value(name) => merge(
                            &mut self.values,
                            name,
                            *foreign
                                .values
                                .get(name)
                                .ok_or_else(|| format!("{prefix} does not expose value {name}"))?,
                        ),
                        Exposed::Operator(name) => replace(
                            &mut self.operators,
                            name,
                            *foreign.operators.get(name).ok_or_else(|| {
                                format!("{prefix} does not expose operator {name}")
                            })?,
                        ),
                        Exposed::Type { name, constructors } => {
                            let id = *foreign
                                .types
                                .get(name)
                                .ok_or_else(|| format!("{prefix} does not expose type {name}"))?;
                            let alias = matches!(
                                symbols.get(id).kind,
                                SymbolKind::Type { alias: true, .. }
                            );
                            if alias && *constructors {
                                return Err(format!(
                                    "cannot expose constructors with (..) for alias {name}"
                                ));
                            }
                            replace(&mut self.types, name, id);
                            if alias || *constructors {
                                for (name, ctor) in &foreign.constructors {
                                    if matches!(symbols.get(*ctor).kind,SymbolKind::Constructor {result,..} if result==id)
                                    {
                                        merge(&mut self.constructors, name, *ctor);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn locals(&self, space: Space) -> &Candidates {
        match space {
            Space::Value => &self.values,
            Space::Type => &self.types,
            Space::Constructor => &self.constructors,
        }
    }
    fn candidates(&self, name: &str, space: Space) -> Option<Bindings> {
        let Some((prefix, field)) = name.rsplit_once('.') else {
            return self.locals(space).get(name).cloned();
        };
        let mut candidates: Option<Bindings> = None;
        for interface in self.qualified.get(prefix)? {
            if let Some(id) = interface.namespace(space).get(field) {
                match &mut candidates {
                    Some(bindings) => bindings.add(*id),
                    None => candidates = Some(Bindings::One(*id)),
                }
            }
        }
        candidates
    }
    fn qualified_names(&self, space: Space) -> BTreeSet<String> {
        self.qualified.iter().flat_map(|(prefix, interfaces)| {
            interfaces.iter().flat_map(move |interface| {
                interface.namespace(space).keys().map(move |name| format!("{prefix}.{name}"))
            })
        }).collect()
    }
    pub fn kernel(&mut self, name: &str, owner: &str) {
        self.kernel.insert(name.into(), owner.into());
    }
    pub fn builtin_list(&mut self, symbols: &mut Symbols) {
        let id = symbols.intern(
            "elm/core:List",
            "List",
            Space::Type,
            SymbolKind::Type {
                arity: 1,
                alias: false,
            },
        );
        replace(&mut self.types, "List", id);
    }
    fn add_local(&mut self, interface: &Interface) {
        for (src, dst) in [
            (&interface.values, &mut self.values),
            (&interface.types, &mut self.types),
            (&interface.constructors, &mut self.constructors),
        ] {
            for (name, id) in src {
                replace(dst, name, *id);
            }
        }
    }
}
#[derive(Debug, Default)]
pub struct Resolved {
    pub expressions: Vec<Option<Binding>>,
    pub types: Vec<Option<SymbolId>>,
    pub constructors: Vec<Option<SymbolId>>,
    pub pattern_bindings: BTreeMap<PatternId, Vec<(String, LocalId)>>,
    pub definitions: BTreeMap<ExprId, LocalId>,
    pub locals: Vec<String>,
}
fn declare(ast: &Syntax<'_>, module: &str, symbols: &mut Symbols) -> Result<Interface, String> {
    let mut local = Interface::default();
    for d in &ast.declarations {
        match d {
            Declaration::Value { name, .. } | Declaration::Port { name, .. } => {
                let id = symbols.intern(module, name, Space::Value, SymbolKind::Value);
                insert(&mut local.values, name, id)
                    .map_err(|e| crate::source_error::locate_slice(ast.source, name, e))?;
            }
            Declaration::Alias {
                name, parameters, ..
            }
            | Declaration::Union {
                name, parameters, ..
            } => {
                let mut seen = BTreeSet::new();
                for parameter in parameters {
                    if !seen.insert(*parameter) {
                        return Err(crate::source_error::locate_slice(
                            ast.source,
                            parameter,
                            format!("duplicate type parameter {parameter} in {name}"),
                        ));
                    }
                }
                let id = symbols.intern(
                    module,
                    name,
                    Space::Type,
                    SymbolKind::Type {
                        arity: parameters.len(),
                        alias: matches!(d, Declaration::Alias { .. }),
                    },
                );
                insert(&mut local.types, name, id)
                    .map_err(|e| crate::source_error::locate_slice(ast.source, name, e))?;
            }
            _ => {}
        }
    }
    if let Effects::Manager {
        command,
        subscription,
    } = &ast.header.effects
    {
        for (name, enabled) in [
            ("command", command.is_some()),
            ("subscription", subscription.is_some()),
        ] {
            if enabled {
                let id = symbols.intern(module, name, Space::Value, SymbolKind::Value);
                insert(&mut local.values, name, id)
                    .map_err(|e| crate::source_error::locate_slice(ast.source, name, e))?;
            }
        }
    }
    // Parse.Module stores unions and aliases in reverse source order. Elm
    // combines their constructor duplicate maps in that order, with unions
    // before record aliases. Keep symbol allocation below unchanged.
    let mut constructor_names = BTreeMap::<&str, Vec<&str>>::new();
    for d in ast.declarations.iter().rev() {
        if let Declaration::Union { variants, .. } = d {
            for (name, _) in variants {
                constructor_names.entry(name).or_default().push(name);
            }
        }
    }
    for d in ast.declarations.iter().rev() {
        if let Declaration::Alias { name, ty, .. } = d
            && matches!(
                ast.types[ty.0 as usize].kind,
                Type::Record {
                    extension: None,
                    ..
                }
            )
        {
            constructor_names.entry(name).or_default().push(name);
        }
    }
    for (name, locations) in constructor_names {
        if locations.len() > 1 {
            return Err(crate::source_error::locate_slice(
                ast.source,
                locations[1],
                format!("duplicate declaration {name}"),
            ));
        }
    }
    for d in &ast.declarations {
        match d {
            Declaration::Union { name, variants, .. } => {
                for (variant, args) in variants {
                    let id = symbols.intern(
                        module,
                        variant,
                        Space::Constructor,
                        SymbolKind::Constructor {
                            result: local.types[*name],
                            arity: args.len(),
                            record: false,
                        },
                    );
                    insert(&mut local.constructors, variant, id)
                        .map_err(|e| crate::source_error::locate_slice(ast.source, variant, e))?;
                }
            }
            Declaration::Alias { name, ty, .. } => {
                if let Type::Record {
                    extension: None,
                    fields,
                } = &ast.types[ty.0 as usize].kind
                {
                    let id = symbols.intern(
                        module,
                        name,
                        Space::Constructor,
                        SymbolKind::Constructor {
                            result: local.types[*name],
                            arity: fields.len(),
                            record: true,
                        },
                    );
                    insert(&mut local.constructors, name, id)
                        .map_err(|e| crate::source_error::locate_slice(ast.source, name, e))?;
                }
            }
            Declaration::Infix {
                operator, function, ..
            } => {
                let id = *local.values.get(*function).ok_or_else(|| {
                    format!("operator {operator} references undefined function {function}")
                })?;
                insert(&mut local.operators, operator, id)
                    .map_err(|e| crate::source_error::locate_slice(ast.source, operator, e))?;
            }
            _ => {}
        }
    }
    Ok(local)
}
fn exported(ast: &Syntax<'_>, local: &Interface, symbols: &Symbols) -> Result<Interface, String> {
    let Exposing::Explicit(items) = &ast.header.exposing else {
        return Ok(local.clone());
    };
    let mut out = Interface::default();
    let mut seen = BTreeSet::new();
    for item in items {
        let name = match item {
            Exposed::Value(n) | Exposed::Operator(n) | Exposed::Type { name: n, .. } => n,
        };
        if !seen.insert(name) {
            return Err(format!("duplicate export {name}"));
        }
        match item {
            Exposed::Value(name) => {
                insert(
                    &mut out.values,
                    name,
                    *local
                        .values
                        .get(name)
                        .ok_or_else(|| format!("exported value {name} is not defined"))?,
                )?;
            }
            Exposed::Operator(name) => {
                insert(
                    &mut out.operators,
                    name,
                    *local
                        .operators
                        .get(name)
                        .ok_or_else(|| format!("exported operator {name} is not defined"))?,
                )?;
            }
            Exposed::Type { name, constructors } => {
                let id = *local
                    .types
                    .get(name)
                    .ok_or_else(|| format!("exported type {name} is not defined"))?;
                let alias = matches!(symbols.get(id).kind, SymbolKind::Type { alias: true, .. });
                if alias && *constructors {
                    return Err(format!("cannot export alias {name}(..)"));
                }
                out.types.insert(name.clone(), id);
                if alias || *constructors {
                    for (name, ctor) in &local.constructors {
                        if matches!(symbols.get(*ctor).kind,SymbolKind::Constructor {result,..} if result==id)
                        {
                            out.constructors.insert(name.clone(), *ctor);
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}
struct Resolver<'a, 's> {
    ast: &'a Syntax<'s>,
    env: Environment,
    symbols: &'a mut Symbols,
    local: Interface,
    scope: HashMap<&'s str, LocalId>,
    binding_context: &'static str,
    binding_function: Option<&'s str>,
    out: Resolved,
}
impl<'a, 's> Resolver<'a, 's> {
    fn bind_name(&mut self, name: &'s str, frame: &mut Vec<&'s str>) -> Result<LocalId, String> {
        if self.scope.contains_key(name) || self.local.values.contains_key(name) {
            let previous = frame.iter().find(|old| **old == name);
            // Pattern duplicate detection in Elm reports the earlier binding;
            // shadowing reports the new binding in the inner scope.
            let region_name = previous.map_or(name, |old| {
                if (old.as_ptr() as usize) < (name.as_ptr() as usize) {
                    *old
                } else {
                    name
                }
            });
            return Err(crate::source_error::locate_slice(
                self.ast.source,
                region_name,
                if let Some(previous) = previous {
                    let later = if (previous.as_ptr() as usize) > (name.as_ptr() as usize) {
                        *previous
                    } else {
                        name
                    };
                    crate::name_diagnostic::duplicate_pattern(
                        self.ast.source,
                        name,
                        later,
                        self.binding_context,
                        self.binding_function,
                    )
                } else {
                    let first =
                        self.scope
                            .get_key_value(name)
                            .map(|(bound, _)| *bound)
                            .or_else(|| {
                                self.ast.declarations.iter().find_map(|declaration| {
                                    match declaration {
                                        Declaration::Value { name: bound, .. }
                                        | Declaration::Port { name: bound, .. }
                                            if *bound == name =>
                                        {
                                            Some(*bound)
                                        }
                                        _ => None,
                                    }
                                })
                            });
                    first.map_or_else(
                        || format!("shadowing local name {name}"),
                        |first| crate::name_diagnostic::shadowing(self.ast.source, name, first),
                    )
                },
            ));
        }
        let id = LocalId(self.out.locals.len() as u32);
        self.out.locals.push(name.into());
        self.scope.insert(name, id);
        frame.push(name);
        Ok(id)
    }
    fn pop(&mut self, frame: Vec<&'s str>) {
        for name in frame {
            self.scope.remove(name);
        }
    }
    fn lookup(&mut self, name: &str) -> Result<Binding, String> {
        if let Some(id) = self.scope.get(name) {
            return Ok(Binding::Local(*id));
        }
        if let Some((prefix, field)) = name.rsplit_once('.')
            && prefix.starts_with("Elm.Kernel.")
            && self.env.allow_kernel
        {
            let id = self.symbols.intern(
                &format!("kernel:{prefix}"),
                field,
                Space::Value,
                SymbolKind::Kernel,
            );
            return Ok(Binding::Global(id));
        }
        let upper = name
            .rsplit('.')
            .next()
            .is_some_and(|n| n.chars().next().is_some_and(crate::unicode::is_upper));
        self.global(name, if upper { Space::Constructor } else { Space::Value }).map(Binding::Global)
    }
    fn global(&self, name: &str, space: Space) -> Result<SymbolId, String> {
        let candidates = self.env.candidates(name, space).ok_or_else(|| self.unknown(name, space))?;
        let thing = match space { Space::Value => "variable", Space::Type => "type", Space::Constructor => "variant" };
        unique_bindings(&candidates, name, self.symbols, thing)
            .map_err(|error| crate::source_error::locate_slice(self.ast.source, name, error))
    }
    fn unknown(&self, name: &str, space: Space) -> String {
        let thing = match space { Space::Value => "variable", Space::Type => "type", Space::Constructor => "variant" };
        let mut local_names: BTreeSet<String> = self.env.locals(space).keys().cloned().collect();
        if space == Space::Value {
            local_names.extend(self.scope.keys().map(|name| (*name).to_owned()));
        }
        let candidates = self.env.qualified_names(space).into_iter().chain(local_names).collect();
        let known_prefix = name
            .rsplit_once('.')
            .is_some_and(|(prefix, _)| self.env.imported_prefixes.contains(prefix));
        crate::source_error::locate_slice(
            self.ast.source,
            name,
            crate::name_diagnostic::unknown(name, thing, known_prefix, candidates),
        )
    }
    fn bind_pattern(&mut self, root: PatternId, frame: &mut Vec<&'s str>) -> Result<(), String> {
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            let result = (|| {
                let mut names = Vec::new();
                match &self.ast.patterns[id.0 as usize].kind {
                    Pattern::Var(name) => names.push(*name),
                    Pattern::Record(fields) => names.extend(fields),
                    Pattern::Alias(inner, name) => {
                        names.push(*name);
                        pending.push(*inner);
                    }
                    Pattern::Tuple(parts) | Pattern::List(parts) => {
                        if matches!(self.ast.patterns[id.0 as usize].kind, Pattern::Tuple(_))
                            && parts.len() > 3
                        {
                            return Err("tuples can have at most 3 entries".into());
                        }
                        pending.extend(parts.iter().rev());
                    }
                    Pattern::Cons(head, tail) => {
                        pending.push(*tail);
                        pending.push(*head);
                    }
                    Pattern::Constructor(name, args) => {
                        let symbol = self.global(name, Space::Constructor)?;
                        match self.symbols.get(symbol).kind {
                            SymbolKind::Constructor {
                                record: false,
                                arity,
                                ..
                            } if arity == args.len() => {}
                            SymbolKind::Constructor { record: true, .. } => {
                                return Err(crate::name_diagnostic::record_pattern(name));
                            }
                            SymbolKind::Constructor { arity, .. } => {
                                return Err(crate::name_diagnostic::arity(
                                    name,
                                    "variant",
                                    arity,
                                    args.len(),
                                    format!(
                                        "constructor pattern {name} expects {arity} arguments, got {}",
                                        args.len()
                                    ),
                                ));
                            }
                            _ => return Err(format!("{name} is not a constructor")),
                        }
                        self.out.constructors[id.0 as usize] = Some(symbol);
                        pending.extend(args.iter().rev());
                    }
                    _ => {}
                }
                for name in names {
                    let local = self.bind_name(name, frame)?;
                    self.out
                        .pattern_bindings
                        .entry(id)
                        .or_default()
                        .push((name.into(), local));
                }
                Ok(())
            })();
            result.map_err(|e| {
                crate::source_error::locate(
                    self.ast.source,
                    self.ast.patterns[id.0 as usize].span,
                    e,
                )
            })?;
        }
        Ok(())
    }
    fn ty(&mut self, root: TypeId, allowed: Option<&BTreeSet<&str>>) -> Result<(), String> {
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            let result = (|| {
                match &self.ast.types[id.0 as usize].kind {
                    Type::Var(name) => {
                        if allowed.is_some_and(|a| !a.contains(name)) {
                            return Err(crate::type_variable_diagnostic::unbound(
                                self.ast,
                                name,
                                format!("unbound type variable {name}"),
                            ));
                        }
                    }
                    Type::Constructor(name, args) => {
                        let symbol = self.global(name, Space::Type)?;
                        if let SymbolKind::Type { arity, .. } = self.symbols.get(symbol).kind
                            && arity != args.len()
                        {
                            return Err(crate::name_diagnostic::arity(
                                name,
                                "type",
                                arity,
                                args.len(),
                                format!(
                                    "type {name} expects {arity} arguments, got {}",
                                    args.len()
                                ),
                            ));
                        }
                        self.out.types[id.0 as usize] = Some(symbol);
                        pending.extend(args);
                    }
                    Type::Function(a, b) => {
                        pending.push(*a);
                        pending.push(*b);
                    }
                    Type::Tuple(parts) => {
                        if parts.len() > 3 {
                            return Err("tuples can have at most 3 entries".into());
                        }
                        pending.extend(parts);
                    }
                    Type::Record { extension, fields } => {
                        if let Some(name) = extension
                            && allowed.is_some_and(|a| !a.contains(name))
                        {
                            return Err(crate::type_variable_diagnostic::unbound(
                                self.ast,
                                name,
                                format!("unbound record variable {name}"),
                            ));
                        }
                        let mut names = BTreeSet::new();
                        for (name, ty) in fields {
                            if !names.insert(name) {
                                return Err(crate::source_error::locate_slice(
                                    self.ast.source,
                                    name,
                                    format!("duplicate record field {name}"),
                                ));
                            }
                            pending.push(*ty);
                        }
                    }
                    Type::Unit => {}
                }
                Ok(())
            })();
            result.map_err(|e| {
                crate::source_error::locate(self.ast.source, self.ast.types[id.0 as usize].span, e)
            })?;
        }
        Ok(())
    }
    fn expression(&mut self, root: ExprId) -> Result<(), String> {
        enum Work<'a, 's> {
            Expr(ExprId),
            Pop(Vec<&'s str>),
            Function(Option<&'s str>, &'a [PatternId], ExprId),
            Branch(PatternId, ExprId),
        }
        let mut pending = vec![Work::Expr(root)];
        while let Some(work) = pending.pop() {
            match work {
                Work::Pop(frame) => self.pop(frame),
                Work::Function(name, patterns, body) => {
                    self.binding_context = if name.is_some() { "function" } else { "lambda" };
                    self.binding_function = name;
                    let mut frame = Vec::new();
                    for pattern in patterns {
                        self.bind_pattern(*pattern, &mut frame)?;
                    }
                    pending.push(Work::Pop(frame));
                    pending.push(Work::Expr(body));
                }
                Work::Branch(pattern, body) => {
                    self.binding_context = "case";
                    self.binding_function = None;
                    let mut frame = Vec::new();
                    self.bind_pattern(pattern, &mut frame)?;
                    pending.push(Work::Pop(frame));
                    pending.push(Work::Expr(body));
                }
                Work::Expr(id) => {
                    let result = (|| {
                        match &self.ast.expressions[id.0 as usize].kind {
                            Expr::Var(name) => {
                                self.out.expressions[id.0 as usize] = Some(self.lookup(name)?)
                            }
                            Expr::Operator(op) => {
                                self.out.expressions[id.0 as usize] = Some(Binding::Global(unique(
                                    &self.env.operators,
                                    op,
                                    self.symbols,
                                    "operator",
                                )?))
                            }
                            Expr::Binary(op, a, b) => {
                                self.out.expressions[id.0 as usize] = Some(Binding::Global(
                                    unique(&self.env.operators, op, self.symbols, "operator")?,
                                ));
                                pending.push(Work::Expr(*b));
                                pending.push(Work::Expr(*a));
                            }
                            Expr::Binops(_, _) => {
                                return Err(
                                    "resolve operator fixities before name resolution".into()
                                );
                            }
                            Expr::List(parts) | Expr::Tuple(parts) => {
                                if matches!(
                                    self.ast.expressions[id.0 as usize].kind,
                                    Expr::Tuple(_)
                                ) && parts.len() > 3
                                {
                                    return Err("tuples can have at most 3 entries".into());
                                }
                                pending.extend(parts.iter().rev().map(|id| Work::Expr(*id)));
                            }
                            Expr::Negate(inner) | Expr::Access(inner, _) => {
                                pending.push(Work::Expr(*inner))
                            }
                            Expr::Call(fun, args) => {
                                pending.extend(args.iter().rev().map(|id| Work::Expr(*id)));
                                pending.push(Work::Expr(*fun));
                            }
                            Expr::If(condition, yes, no) => {
                                pending.push(Work::Expr(*no));
                                pending.push(Work::Expr(*yes));
                                pending.push(Work::Expr(*condition));
                            }
                            Expr::Record { base, fields } => {
                                if let Some(name) = base {
                                    self.out.expressions[id.0 as usize] = Some(self.lookup(name)?);
                                }
                                let mut seen = BTreeSet::new();
                                for (name, expr) in fields {
                                    if !seen.insert(name) {
                                        return Err(crate::source_error::locate_slice(
                                            self.ast.source,
                                            name,
                                            format!("duplicate record field {name}"),
                                        ));
                                    }
                                    pending.push(Work::Expr(*expr));
                                }
                            }
                            Expr::Lambda(args, body) => {
                                pending.push(Work::Function(None, args, *body))
                            }
                            Expr::Case(subject, branches) => {
                                for (pattern, body) in branches.iter().rev() {
                                    pending.push(Work::Branch(*pattern, *body));
                                }
                                pending.push(Work::Expr(*subject));
                            }
                            Expr::Let(declarations, body) => {
                                let mut frame = Vec::new();
                                for declaration in declarations {
                                    match declaration {
                                        Declaration::Value { name, body, .. } => {
                                            self.binding_context = "let";
                                            self.binding_function = None;
                                            let local = self.bind_name(name, &mut frame)?;
                                            self.out.definitions.insert(*body, local);
                                        }
                                        Declaration::Destruct { pattern, .. } => {
                                            self.binding_context = "let";
                                            self.binding_function = None;
                                            self.bind_pattern(*pattern, &mut frame)?
                                        }
                                        Declaration::Annotation { ty, .. } => self.ty(*ty, None)?,
                                        _ => return Err("invalid local declaration".into()),
                                    }
                                }
                                pending.push(Work::Pop(frame));
                                pending.push(Work::Expr(*body));
                                for declaration in declarations.iter().rev() {
                                    match declaration {
                                        Declaration::Value {
                                            name,
                                            arguments,
                                            body,
                                            ..
                                        } => pending.push(Work::Function(
                                            Some(name),
                                            arguments,
                                            *body,
                                        )),
                                        Declaration::Destruct { body, .. } => {
                                            pending.push(Work::Expr(*body))
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            Expr::Literal(_, _) | Expr::Unit | Expr::Accessor(_) => {}
                        }
                        Ok(())
                    })();
                    result.map_err(|e| {
                        crate::source_error::locate(
                            self.ast.source,
                            self.ast.expressions[id.0 as usize].span,
                            e,
                        )
                    })?;
                }
            }
        }
        Ok(())
    }
}
pub fn resolve(
    ast: &Syntax<'_>,
    module: &str,
    mut env: Environment,
    symbols: &mut Symbols,
) -> Result<(Interface, Resolved), String> {
    check_aliases(ast)?;
    let owner = module.split(':').next().unwrap_or("");
    env.allow_kernel = owner.starts_with("elm/") || owner.starts_with("elm-explorations/");
    let local = declare(ast, module, symbols)?;
    env.add_local(&local);
    let out = Resolved {
        expressions: vec![None; ast.expressions.len()],
        types: vec![None; ast.types.len()],
        constructors: vec![None; ast.patterns.len()],
        ..Resolved::default()
    };
    let mut r = Resolver {
        ast,
        env,
        symbols,
        local,
        scope: HashMap::new(),
        binding_context: "pattern",
        binding_function: None,
        out,
    };
    let mut errors = Vec::new();
    for declaration in ast.declarations.iter().rev() {
        let result = (|| -> Result<(), String> {
            match declaration {
                Declaration::Annotation { ty, .. } | Declaration::Port { ty, .. } => {
                    r.ty(*ty, None)?
                }
                Declaration::Alias { parameters, ty, .. } => {
                    r.ty(*ty, Some(&parameters.iter().copied().collect()))?
                }
                Declaration::Union {
                    parameters,
                    variants,
                    ..
                } => {
                    let allowed = parameters.iter().copied().collect();
                    for (_, args) in variants {
                        for ty in args {
                            r.ty(*ty, Some(&allowed))?;
                        }
                    }
                }
                Declaration::Value {
                    name,
                    arguments,
                    body,
                    ..
                } => {
                    r.binding_context = "function";
                    r.binding_function = Some(name);
                    let mut frame = Vec::new();
                    for pattern in arguments {
                        r.bind_pattern(*pattern, &mut frame)?;
                    }
                    r.expression(*body)?;
                    r.pop(frame);
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            errors.push(error);
            // Failed expression traversal can leave nested bindings on the stack.
            // Top-level declarations always begin with an empty local scope.
            r.scope.clear();
        }
    }
    if !errors.is_empty() {
        return Err(crate::source_error::batch(errors));
    }
    let interface = exported(ast, &r.local, r.symbols)?;
    Ok((interface, r.out))
}

fn check_aliases(ast: &Syntax<'_>) -> Result<(), String> {
    let aliases: BTreeMap<&str, TypeId> = ast
        .declarations
        .iter()
        .filter_map(|d| match d {
            Declaration::Alias { name, ty, .. } => Some((*name, *ty)),
            _ => None,
        })
        .collect();
    let mut edges = BTreeMap::<&str, BTreeSet<&str>>::new();
    for d in &ast.declarations {
        if let Declaration::Alias {
            name,
            parameters,
            ty,
        } = d
        {
            let mut pending = vec![*ty];
            let mut variables = BTreeSet::new();
            let mut deps = BTreeSet::new();
            while let Some(id) = pending.pop() {
                match &ast.types[id.0 as usize].kind {
                    Type::Var(name) => {
                        variables.insert(*name);
                    }
                    Type::Constructor(name, args) => {
                        if aliases.contains_key(name) {
                            deps.insert(*name);
                        }
                        pending.extend(args);
                    }
                    Type::Function(a, b) => {
                        pending.push(*a);
                        pending.push(*b);
                    }
                    Type::Tuple(parts) => pending.extend(parts),
                    Type::Record { extension, fields } => {
                        if let Some(name) = extension {
                            variables.insert(*name);
                        }
                        pending.extend(fields.iter().map(|(_, ty)| *ty));
                    }
                    Type::Unit => {}
                }
            }
            for parameter in parameters {
                if !variables.contains(parameter) {
                    return Err(crate::type_variable_diagnostic::unbound(
                        ast,
                        parameter,
                        format!("unused type parameter {parameter} in alias {name}"),
                    ));
                }
            }
            edges.insert(name, deps);
        }
    }
    let mut reverse = BTreeMap::<&str, Vec<&str>>::new();
    let mut counts = BTreeMap::new();
    let mut ready = Vec::new();
    for (name, deps) in &edges {
        counts.insert(*name, deps.len());
        if deps.is_empty() {
            ready.push(*name);
        }
        for dep in deps {
            reverse.entry(dep).or_default().push(name);
        }
    }
    let mut visited = 0;
    while let Some(name) = ready.pop() {
        visited += 1;
        if let Some(parents) = reverse.get(name) {
            for parent in parents {
                let n = counts.get_mut(parent).unwrap();
                *n -= 1;
                if *n == 0 {
                    ready.push(*parent);
                }
            }
        }
    }
    if visited != edges.len() {
        return Err(format!(
            "recursive type aliases: {:?}",
            counts
                .iter()
                .filter(|(_, n)| **n > 0)
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod module_symbol_tests {
    use super::*;
    #[test]
    fn module_index_tracks_interleaved_spaces_and_cloned_symbol_tables() {
        let mut symbols = Symbols::default();
        let a = symbols.intern("A", "value", Space::Value, SymbolKind::Value);
        let b = symbols.intern("B", "value", Space::Value, SymbolKind::Value);
        let t = symbols.intern("A", "value", Space::Type, SymbolKind::Type { arity: 0, alias: false });
        assert_eq!(symbols.intern("A", "value", Space::Value, SymbolKind::Value), a);
        assert_eq!(symbols.module_symbols("A"), &[a, t]);
        assert_eq!(symbols.module_symbols("B"), &[b]);
        assert!(symbols.module_symbols("missing").is_empty());
        let mut next = symbols.clone();
        let extra = next.intern("A", "extra", Space::Value, SymbolKind::Value);
        assert_eq!(next.module_symbols("A"), &[a, t, extra]);
        assert_eq!(symbols.module_symbols("A"), &[a, t]);
    }
}
