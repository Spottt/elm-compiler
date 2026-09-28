//! Public API comparison adapted from Elm 0.19.1 Deps.Diff (LICENSE-ELM).
//! Comments do not affect compatibility; variable renaming must be bijective.
use crate::{
    ast::{Declaration, Syntax, Type, TypeId},
    package_solver::Version,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Magnitude {
    Patch,
    Minor,
    Major,
}
impl Magnitude {
    pub fn bump(self, version: Version) -> Version {
        let [major, minor, patch] = version.0;
        Version(match self {
            Self::Patch => [major, minor, patch.wrapping_add(1)],
            Self::Minor => [major, minor.wrapping_add(1), 0],
            Self::Major => [major.wrapping_add(1), 0, 0],
        })
    }
}
#[derive(Debug, Default)]
pub struct Changes {
    pub added: BTreeMap<String, Value>,
    pub changed: BTreeMap<String, (Value, Value)>,
    pub removed: BTreeMap<String, Value>,
}
impl Changes {
    pub fn magnitude(&self) -> Magnitude {
        if !self.changed.is_empty() || !self.removed.is_empty() {
            Magnitude::Major
        } else if !self.added.is_empty() {
            Magnitude::Minor
        } else {
            Magnitude::Patch
        }
    }
}
#[derive(Debug)]
pub struct ModuleChanges {
    pub unions: Changes,
    pub aliases: Changes,
    pub values: Changes,
    pub binops: Changes,
}
impl ModuleChanges {
    pub fn magnitude(&self) -> Magnitude {
        [&self.unions, &self.aliases, &self.values, &self.binops]
            .into_iter()
            .map(Changes::magnitude)
            .max()
            .unwrap()
    }
}
#[derive(Debug, Default)]
pub struct PackageChanges {
    pub added: Vec<String>,
    pub changed: BTreeMap<String, ModuleChanges>,
    pub removed: Vec<String>,
}
impl PackageChanges {
    pub fn magnitude(&self) -> Magnitude {
        let base = if !self.removed.is_empty() {
            Magnitude::Major
        } else if !self.added.is_empty() {
            Magnitude::Minor
        } else {
            Magnitude::Patch
        };
        self.changed
            .values()
            .map(ModuleChanges::magnitude)
            .fold(base, std::cmp::max)
    }
}
fn source(tipe: &str) -> String {
    format!("module Documentation exposing (..)\ntype alias Documented = {tipe}\n")
}
fn root(ast: &Syntax<'_>) -> Result<TypeId, String> {
    match ast.declarations.as_slice() {
        [Declaration::Alias { ty, .. }] => Ok(*ty),
        _ => Err("invalid documentation type".into()),
    }
}
fn same_name(old: &str, new: &str) -> bool {
    // Elm.Compiler.Type.fromRawType discards qualification when decoding docs
    // JSON. Diffing persisted documentation must preserve that behavior.
    old.rsplit('.').next() == new.rsplit('.').next()
}

fn category(name: &str) -> u8 {
    for (prefix, code) in [
        ("compappend", 1),
        ("comparable", 2),
        ("appendable", 3),
        ("number", 4),
    ] {
        if name.starts_with(prefix) {
            return code;
        }
    }
    0
}
fn compatible(old: &str, new: &str) -> bool {
    let (a, b) = (category(old), category(new));
    b == 0 || a == b || (a == 4 && b == 2)
}
/// This is directional, matching the reference's permitted constraint widening.
pub fn equivalent_type(
    old: &str,
    new: &str,
    old_parameters: &[String],
    new_parameters: &[String],
) -> Result<bool, String> {
    if old_parameters.len() != new_parameters.len() {
        return Ok(false);
    }
    let (old_source, new_source) = (source(old), source(new));
    let old = crate::parser::parse(&old_source)?;
    let new = crate::parser::parse(&new_source)?;
    let mut pending = vec![(root(&old)?, root(&new)?)];
    let mut pairs: Vec<(String, String)> = old_parameters
        .iter()
        .cloned()
        .zip(new_parameters.iter().cloned())
        .collect();
    while let Some((a, b)) = pending.pop() {
        match (&old.types[a.0 as usize].kind, &new.types[b.0 as usize].kind) {
            (Type::Var(a), Type::Var(b)) => pairs.push((a.to_string(), b.to_string())),
            (Type::Function(a, b), Type::Function(x, y)) => {
                pending.push((*a, *x));
                pending.push((*b, *y));
            }
            (Type::Constructor(a, args), Type::Constructor(b, other))
                if same_name(a, b) && args.len() == other.len() =>
            {
                pending.extend(args.iter().copied().zip(other.iter().copied()))
            }
            (Type::Tuple(args), Type::Tuple(other)) if args.len() == other.len() => {
                pending.extend(args.iter().copied().zip(other.iter().copied()))
            }
            (
                Type::Record {
                    extension: a,
                    fields,
                },
                Type::Record {
                    extension: b,
                    fields: other,
                },
            ) => {
                match (a, b) {
                    (Some(a), Some(b)) => pairs.push((a.to_string(), b.to_string())),
                    (None, None) => {}
                    _ => return Ok(false),
                }
                if fields.len() != other.len() {
                    return Ok(false);
                }
                let mut fields: Vec<_> = fields.iter().collect();
                let mut other: Vec<_> = other.iter().collect();
                fields.sort_by_key(|(name, _)| *name);
                other.sort_by_key(|(name, _)| *name);
                for ((a, ta), (b, tb)) in fields.into_iter().zip(other) {
                    if a != b {
                        return Ok(false);
                    }
                    pending.push((*ta, *tb));
                }
            }
            (Type::Unit, Type::Unit) => {}
            _ => return Ok(false),
        }
    }
    let mut forward = BTreeMap::new();
    let mut reverse = BTreeSet::new();
    for (a, b) in pairs {
        if !compatible(&a, &b) {
            return Ok(false);
        }
        if let Some(previous) = forward.get(&a) {
            if previous != &b {
                return Ok(false);
            }
        } else {
            if !reverse.insert(b.clone()) {
                return Ok(false);
            }
            forward.insert(a, b);
        }
    }
    Ok(true)
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v[key]
        .as_str()
        .ok_or_else(|| format!("invalid docs field {key}"))
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    v[key]
        .as_array()
        .ok_or_else(|| format!("invalid docs field {key}"))
}
fn parameters(v: &Value) -> Result<Vec<String>, String> {
    array(v, "args")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or("invalid docs type parameter".into())
        })
        .collect()
}
fn dictionary(values: &[Value]) -> Result<BTreeMap<String, Value>, String> {
    values
        .iter()
        .map(|v| Ok((string(v, "name")?.to_owned(), v.clone())))
        .collect()
}
fn cases(v: &Value) -> Result<Vec<(&str, Vec<&str>)>, String> {
    array(v, "cases")?
        .iter()
        .map(|v| {
            let pair = v
                .as_array()
                .filter(|v| v.len() == 2)
                .ok_or("invalid documented constructor")?;
            Ok((
                pair[0].as_str().ok_or("invalid constructor name")?,
                pair[1]
                    .as_array()
                    .ok_or("invalid constructor arguments")?
                    .iter()
                    .map(|v| v.as_str().ok_or("invalid constructor type".into()))
                    .collect::<Result<_, String>>()?,
            ))
        })
        .collect()
}
fn validate_type(tipe: &str) -> Result<(), String> {
    root(&crate::parser::parse(&source(tipe))?).map(|_| ())
}
fn entries(module: &Value, kind: &str) -> Result<BTreeMap<String, Value>, String> {
    let entries = dictionary(array(module, kind)?)?;
    for value in entries.values() {
        string(value, "comment")?;
        if kind == "unions" {
            parameters(value)?;
            for (_, types) in cases(value)? {
                for tipe in types {
                    validate_type(tipe)?;
                }
            }
        } else {
            validate_type(string(value, "type")?)?;
            if kind == "aliases" {
                parameters(value)?;
            }
        }
        if kind == "binops"
            && (!matches!(string(value, "associativity")?, "left" | "right" | "non")
                || value["precedence"].as_i64().is_none())
        {
            return Err("invalid documented operator fixity".into());
        }
    }
    Ok(entries)
}
fn equivalent(kind: &str, old: &Value, new: &Value) -> Result<bool, String> {
    if kind == "unions" {
        let (a, b) = (cases(old)?, cases(new)?);
        if a.len() != b.len() {
            return Ok(false);
        }
        let (ap, bp) = (parameters(old)?, parameters(new)?);
        for ((name, args), (other, types)) in a.into_iter().zip(b) {
            if name != other || args.len() != types.len() {
                return Ok(false);
            }
            for (a, b) in args.into_iter().zip(types) {
                if !equivalent_type(a, b, &ap, &bp)? {
                    return Ok(false);
                }
            }
        }
        return Ok(true);
    }
    let (a, b) = if kind == "aliases" {
        (parameters(old)?, parameters(new)?)
    } else {
        (vec![], vec![])
    };
    Ok(
        equivalent_type(string(old, "type")?, string(new, "type")?, &a, &b)?
            && (kind != "binops"
                || (old["associativity"] == new["associativity"]
                    && old["precedence"] == new["precedence"])),
    )
}
fn changes(
    kind: &str,
    old: BTreeMap<String, Value>,
    new: BTreeMap<String, Value>,
) -> Result<Changes, String> {
    let mut result = Changes::default();
    for (name, value) in &new {
        if let Some(previous) = old.get(name) {
            if !equivalent(kind, previous, value)? {
                result
                    .changed
                    .insert(name.clone(), (previous.clone(), value.clone()));
            }
        } else {
            result.added.insert(name.clone(), value.clone());
        }
    }
    for (name, value) in old {
        if !new.contains_key(&name) {
            result.removed.insert(name, value);
        }
    }
    Ok(result)
}
pub fn diff(old: &Value, new: &Value) -> Result<PackageChanges, String> {
    let old = dictionary(old.as_array().ok_or("documentation must be an array")?)?;
    let new = dictionary(new.as_array().ok_or("documentation must be an array")?)?;
    // Validate even modules which are wholly added/removed, as Elm.Docs does.
    for module in old.values().chain(new.values()) {
        string(module, "comment")?;
        for kind in ["unions", "aliases", "values", "binops"] {
            entries(module, kind)?;
        }
    }
    let mut result = PackageChanges::default();
    for (name, module) in &new {
        if let Some(previous) = old.get(name) {
            let c = ModuleChanges {
                unions: changes(
                    "unions",
                    entries(previous, "unions")?,
                    entries(module, "unions")?,
                )?,
                aliases: changes(
                    "aliases",
                    entries(previous, "aliases")?,
                    entries(module, "aliases")?,
                )?,
                values: changes(
                    "values",
                    entries(previous, "values")?,
                    entries(module, "values")?,
                )?,
                binops: changes(
                    "binops",
                    entries(previous, "binops")?,
                    entries(module, "binops")?,
                )?,
            };
            if c.magnitude() != Magnitude::Patch {
                result.changed.insert(name.clone(), c);
            }
        } else {
            result.added.push(name.clone());
        }
    }
    result.removed = old
        .keys()
        .filter(|name| !new.contains_key(*name))
        .cloned()
        .collect();
    Ok(result)
}
