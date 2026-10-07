//! Lower resolved source types into shared inference graphs. Alias templates are
//! immutable and parameter substitution preserves unchanged subgraphs. This pass
//! builds declared signatures; it does not infer or validate expression bodies.
use crate::{
    ast::{Declaration, Syntax, Type, TypeId},
    names::{Resolved, Space, SymbolId, SymbolKind, Symbols},
    unify::{Builtins, Constraint, Engine, Scheme, Term, Ty},
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub fn builtins(symbols: &mut Symbols) -> Builtins {
    let mut ty = |module, name, arity| {
        symbols.intern(
            module,
            name,
            Space::Type,
            SymbolKind::Type {
                arity,
                alias: false,
            },
        )
    };
    for (module, name, arity) in [
        ("elm-explorations/webgl:WebGL", "Shader", 3),
        ("elm-explorations/webgl:WebGL.Texture", "Texture", 0),
        ("elm-explorations/linear-algebra:Math.Vector2", "Vec2", 0),
        ("elm-explorations/linear-algebra:Math.Vector3", "Vec3", 0),
        ("elm-explorations/linear-algebra:Math.Vector4", "Vec4", 0),
        ("elm-explorations/linear-algebra:Math.Matrix4", "Mat4", 0),
    ] {
        ty(module, name, arity);
    }
    Builtins {
        int: ty("elm/core:Basics", "Int", 0),
        float: ty("elm/core:Basics", "Float", 0),
        string: ty("elm/core:String", "String", 0),
        char: ty("elm/core:Char", "Char", 0),
        list: ty("elm/core:List", "List", 1),
    }
}

pub fn constraint(name: &str) -> Constraint {
    for (prefix, kind) in [
        ("number", Constraint::Number),
        ("comparable", Constraint::Comparable),
        ("appendable", Constraint::Appendable),
        ("compappend", Constraint::CompAppend),
    ] {
        if name.starts_with(prefix) {
            return kind;
        }
    }
    Constraint::Any
}

#[derive(Debug, Clone)]
pub struct Alias {
    pub parameters: Vec<Ty>,
    pub root: Ty,
}
#[derive(Default, Clone)]
pub struct Catalog {
    pub aliases: BTreeMap<SymbolId, Alias>,
    pub constructors: BTreeMap<SymbolId, Scheme>,
}
#[derive(Default)]
pub struct Declarations {
    pub annotations: BTreeMap<SymbolId, Scheme>,
    pub ports: BTreeMap<SymbolId, Scheme>,
}
fn symbol(symbols: &Symbols, module: &str, name: &str, space: Space) -> Result<SymbolId, String> {
    symbols
        .lookup(module, name, space)
        .ok_or_else(|| format!("missing resolved declaration {module}.{name}"))
}
fn children(ty: &Type<'_>) -> Vec<TypeId> {
    match ty {
        Type::Constructor(_, args) | Type::Tuple(args) => args.clone(),
        Type::Function(a, b) => vec![*a, *b],
        Type::Record { fields, .. } => fields.iter().map(|(_, ty)| *ty).collect(),
        Type::Var(_) | Type::Unit => vec![],
    }
}
fn variable<'s>(
    engine: &mut Engine,
    variables: &mut HashMap<&'s str, Ty>,
    name: &'s str,
    level: u32,
) -> Ty {
    *variables
        .entry(name)
        .or_insert_with(|| engine.named_variable(level, constraint(name), name))
}
#[derive(Clone, Copy)]
pub struct SourceTypes<'a, 's> {
    pub ast: &'a Syntax<'s>,
    pub resolved: &'a Resolved,
    pub symbols: &'a Symbols,
}
impl Catalog {
    /// The catalog and global schemes are the only type handles that survive a
    /// completed module. Keep phantom alias parameters and quantified variables
    /// too, even when they are not reachable from a scheme's result type.
    pub fn compact(&mut self, engine: &mut Engine, globals: &mut BTreeMap<SymbolId, Scheme>) {
        self.compact_with_policy(engine, globals, false);
    }
    pub(crate) fn compact_for_analysis(&mut self, engine: &mut Engine, globals: &mut BTreeMap<SymbolId, Scheme>) {
        self.compact_with_policy(engine, globals, true);
    }
    fn compact_with_policy(&mut self, engine: &mut Engine, globals: &mut BTreeMap<SymbolId, Scheme>, defer_sparse: bool) {
        let mut roots = Vec::new();
        for alias in self.aliases.values() {
            roots.push(alias.root);
            roots.extend(&alias.parameters);
        }
        for scheme in self.constructors.values().chain(globals.values()) {
            roots.push(scheme.root);
            roots.extend(&scheme.quantified);
        }
        if defer_sparse {
            if !engine.compact_if_useful(&mut roots) { return; }
        } else {
            engine.compact(&mut roots);
        }
        let mut roots = roots.into_iter();
        for alias in self.aliases.values_mut() {
            alias.root = roots.next().unwrap();
            for parameter in &mut alias.parameters {
                *parameter = roots.next().unwrap();
            }
        }
        for scheme in self.constructors.values_mut().chain(globals.values_mut()) {
            scheme.root = roots.next().unwrap();
            scheme.quantified = roots.by_ref().take(scheme.quantified.len()).collect();
        }
        debug_assert!(roots.next().is_none());
    }
    pub fn lower<'s>(
        &self,
        source: SourceTypes<'_, 's>,
        engine: &mut Engine,
        root: TypeId,
        variables: &mut HashMap<&'s str, Ty>,
        level: u32,
    ) -> Result<Ty, String> {
        let SourceTypes {
            ast,
            resolved,
            symbols,
        } = source;
        let mut done = HashMap::<TypeId, Ty>::new();
        let mut pending = vec![(root, false)];
        while let Some((id, finish)) = pending.pop() {
            if done.contains_key(&id) {
                continue;
            }
            let ty = &ast.types[id.0 as usize].kind;
            if !finish {
                pending.push((id, true));
                pending.extend(children(ty).into_iter().map(|child| (child, false)));
                continue;
            }
            let result = match ty {
                Type::Var(name) => variable(engine, variables, name, level),
                Type::Unit => engine.term(Term::Unit),
                Type::Function(a, b) => engine.term(Term::Function(done[a], done[b])),
                Type::Tuple(args) => {
                    engine.term(Term::Tuple(args.iter().map(|id| done[id]).collect()))
                }
                Type::Record { extension, fields } => {
                    let extension = extension.map(|name| variable(engine, variables, name, level));
                    let result = engine.term(Term::Record {
                        extension,
                        fields: fields
                            .iter()
                            .map(|(name, id)| ((*name).into(), done[id]))
                            .collect(),
                    });
                    engine.record_field_order(
                        result,
                        fields.iter().map(|(name, _)| name.to_string()),
                    );
                    result
                }
                Type::Constructor(_, args) => {
                    let symbol =
                        resolved.types[id.0 as usize].ok_or("unresolved type reference")?;
                    let SymbolKind::Type { arity, alias } = symbols.get(symbol).kind else {
                        return Err("type reference points to a non-type symbol".into());
                    };
                    if arity != args.len() {
                        return Err("type argument count mismatch".into());
                    }
                    let args: Vec<_> = args.iter().map(|id| done[id]).collect();
                    if alias {
                        let template = self.aliases.get(&symbol).ok_or_else(|| {
                            format!("alias template not ready: {}", symbols.get(symbol).name)
                        })?;
                        let replacements: Vec<_> = template
                            .parameters
                            .iter()
                            .copied()
                            .zip(args.iter().copied())
                            .collect();
                        {
                            let real = engine.substitute(template.root, &replacements);
                            engine.alias(symbol, args, real)
                        }
                    } else {
                        engine.term(Term::Named(symbol, args))
                    }
                }
            };
            done.insert(id, result);
        }
        Ok(done[&root])
    }
    pub fn annotation(
        &self,
        ast: &Syntax<'_>,
        resolved: &Resolved,
        symbols: &Symbols,
        engine: &mut Engine,
        ty: TypeId,
    ) -> Result<Scheme, String> {
        let root = self.lower(
            SourceTypes {
                ast,
                resolved,
                symbols,
            },
            engine,
            ty,
            &mut HashMap::new(),
            1,
        )?;
        Ok(engine.generalize(root, 0))
    }
    pub fn register(
        &mut self,
        ast: &Syntax<'_>,
        resolved: &Resolved,
        symbols: &Symbols,
        module: &str,
        engine: &mut Engine,
    ) -> Result<Declarations, String> {
        self.register_declarations(ast, resolved, symbols, module, engine, true)
    }

    /// Inference lowers value annotations and ports itself, with the appropriate
    /// scoped variables. Prepare only aliases and constructors for that path.
    pub fn register_type_definitions(
        &mut self,
        ast: &Syntax<'_>,
        resolved: &Resolved,
        symbols: &Symbols,
        module: &str,
        engine: &mut Engine,
    ) -> Result<(), String> {
        self.register_declarations(ast, resolved, symbols, module, engine, false)?;
        Ok(())
    }

    fn register_declarations(
        &mut self,
        ast: &Syntax<'_>,
        resolved: &Resolved,
        symbols: &Symbols,
        module: &str,
        engine: &mut Engine,
        signatures: bool,
    ) -> Result<Declarations, String> {
        // Alias references may point forward in source order. Sort only aliases;
        // recursive unions remain nominal nodes and are never expanded here.
        let mut local = BTreeMap::new();
        for (index, decl) in ast.declarations.iter().enumerate() {
            if let Declaration::Alias { name, .. } = decl {
                local.insert(symbol(symbols, module, name, Space::Type)?, index);
            }
        }
        let mut degrees = BTreeMap::new();
        let mut users = BTreeMap::<SymbolId, Vec<SymbolId>>::new();
        for (&id, &index) in &local {
            let Declaration::Alias { ty, .. } = &ast.declarations[index] else {
                unreachable!()
            };
            let mut dependencies = BTreeSet::new();
            let mut pending = vec![*ty];
            while let Some(ty) = pending.pop() {
                if let Some(target) = resolved.types[ty.0 as usize]
                    && local.contains_key(&target)
                {
                    dependencies.insert(target);
                }
                pending.extend(children(&ast.types[ty.0 as usize].kind));
            }
            degrees.insert(id, dependencies.len());
            for dependency in dependencies {
                users.entry(dependency).or_default().push(id);
            }
        }
        let mut ready: Vec<_> = degrees
            .iter()
            .filter_map(|(id, count)| (*count == 0).then_some(*id))
            .collect();
        let mut visited = 0;
        while let Some(id) = ready.pop() {
            let Declaration::Alias { parameters, ty, .. } = &ast.declarations[local[&id]] else {
                unreachable!()
            };
            // Alias parameters are holes, not constrained annotation variables:
            // e.g. `type alias N number = number` applied to String is String.
            let mut variables: HashMap<_, _> = parameters
                .iter()
                .map(|name| (*name, engine.named_variable(1, Constraint::Any, name)))
                .collect();
            let root = self.lower(
                SourceTypes {
                    ast,
                    resolved,
                    symbols,
                },
                engine,
                *ty,
                &mut variables,
                1,
            )?;
            let parameters = parameters.iter().map(|name| variables[name]).collect();
            self.aliases.insert(id, Alias { parameters, root });
            visited += 1;
            for user in users.get(&id).into_iter().flatten() {
                let count = degrees.get_mut(user).unwrap();
                *count -= 1;
                if *count == 0 {
                    ready.push(*user);
                }
            }
        }
        if visited != local.len() {
            return Err("recursive alias definitions".into());
        }
        let mut output = Declarations::default();
        for decl in &ast.declarations {
            match decl {
                Declaration::Union {
                    name,
                    parameters,
                    variants,
                } => {
                    let result_symbol = symbol(symbols, module, name, Space::Type)?;
                    for (variant, args) in variants {
                        let mut variables: HashMap<_, _> = parameters
                            .iter()
                            .map(|name| (*name, engine.named_variable(1, constraint(name), name)))
                            .collect();
                        let args: Vec<_> = args
                            .iter()
                            .map(|ty| {
                                self.lower(
                                    SourceTypes {
                                        ast,
                                        resolved,
                                        symbols,
                                    },
                                    engine,
                                    *ty,
                                    &mut variables,
                                    1,
                                )
                            })
                            .collect::<Result<_, _>>()?;
                        let mut root = engine.term(Term::Named(
                            result_symbol,
                            parameters.iter().map(|name| variables[name]).collect(),
                        ));
                        for argument in args.into_iter().rev() {
                            root = engine.term(Term::Function(argument, root));
                        }
                        let scheme = engine.generalize(root, 0);
                        self.constructors.insert(
                            symbol(symbols, module, variant, Space::Constructor)?,
                            scheme,
                        );
                    }
                }
                Declaration::Alias {
                    name,
                    parameters,
                    ty,
                } => {
                    if let Type::Record {
                        extension: None,
                        fields,
                    } = &ast.types[ty.0 as usize].kind
                    {
                        let mut variables: HashMap<_, _> = parameters
                            .iter()
                            .map(|name| (*name, engine.named_variable(1, constraint(name), name)))
                            .collect();
                        let mut root = self.lower(
                            SourceTypes {
                                ast,
                                resolved,
                                symbols,
                            },
                            engine,
                            *ty,
                            &mut variables,
                            1,
                        )?;
                        let Some(term) = engine.structure(root) else {
                            unreachable!()
                        };
                        let Term::Record { fields: types, .. } = &*term else {
                            unreachable!()
                        };
                        root = engine.alias(
                            symbol(symbols, module, name, Space::Type)?,
                            parameters.iter().map(|name| variables[name]).collect(),
                            root,
                        );
                        // Record constructor arguments follow declaration order,
                        // not the sorted field map used by record unification.
                        for (field, _) in fields.iter().rev() {
                            root = engine.term(Term::Function(types[&crate::edition::FieldName::from(*field)], root));
                        }
                        self.constructors.insert(
                            symbol(symbols, module, name, Space::Constructor)?,
                            engine.generalize(root, 0),
                        );
                    }
                }
                Declaration::Annotation { name, ty } if signatures => {
                    output.annotations.insert(
                        symbol(symbols, module, name, Space::Value)?,
                        self.annotation(ast, resolved, symbols, engine, *ty)?,
                    );
                }
                Declaration::Port { name, ty } if signatures => {
                    output.ports.insert(
                        symbol(symbols, module, name, Space::Value)?,
                        self.annotation(ast, resolved, symbols, engine, *ty)?,
                    );
                }
                _ => {}
            }
        }
        Ok(output)
    }
}
