//! Stable symbol names keep module JavaScript reusable when graph IDs change.
use crate::{
    module_codegen::{Definition, Registration},
    names::{Space, SymbolId, Symbols},
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
fn name(id: SymbolId, symbols: &Symbols) -> Value {
    let symbol = symbols.get(id);
    json!([symbol.module.as_ref(), symbol.name.as_ref()])
}
fn names(ids: &BTreeSet<SymbolId>, symbols: &Symbols) -> Vec<Value> {
    ids.iter().map(|id| name(*id, symbols)).collect()
}
pub fn encode(definitions: &[Definition], symbols: &Symbols) -> Value {
    Value::Array(definitions.iter().map(|d| json!({
        "symbol":name(d.symbol,symbols), "javascript":d.javascript,
        "dependencies":names(&d.dependencies,symbols), "function":d.function,
        "cycle_initialization":d.cycle_initialization,
        "registration":d.registration.as_ref().map(|r| json!({"javascript":r.javascript,"dependencies":names(&r.dependencies,symbols)}))
    })).collect())
}
fn symbol(value: &Value, symbols: &Symbols) -> Option<SymbolId> {
    let module = value.get(0)?.as_str()?;
    let name = value.get(1)?.as_str()?;
    symbols
        .lookup(module, name, Space::Value)
        .or_else(|| symbols.lookup(module, name, Space::Constructor))
}
fn dependencies(value: &Value, symbols: &Symbols) -> Option<BTreeSet<SymbolId>> {
    value
        .as_array()?
        .iter()
        .map(|v| symbol(v, symbols))
        .collect()
}
fn optional_string(value: &Value) -> Option<Option<String>> {
    if value.is_null() {
        Some(None)
    } else {
        Some(Some(value.as_str()?.to_owned()))
    }
}
pub fn decode(value: &Value, symbols: &Symbols) -> Option<Vec<Definition>> {
    value
        .as_array()?
        .iter()
        .map(|v| {
            Some(Definition {
                symbol: symbol(v.get("symbol")?, symbols)?,
                javascript: v.get("javascript")?.as_str()?.to_owned(),
                dependencies: dependencies(v.get("dependencies")?, symbols)?,
                function: v.get("function")?.as_bool()?,
                cycle_initialization: optional_string(v.get("cycle_initialization")?)?,
                registration: match v.get("registration")? {
                    Value::Null => None,
                    r => Some(Registration {
                        javascript: r.get("javascript")?.as_str()?.to_owned(),
                        dependencies: dependencies(r.get("dependencies")?, symbols)?,
                    }),
                },
            })
        })
        .collect()
}
