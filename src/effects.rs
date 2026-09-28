//! Lifecycle constraints from Type.Constrain.Module in Elm 0.19.1.
use crate::{
    module::Effects,
    names::{Space, SymbolId, Symbols},
    unify::{Constraint, Engine, Scheme, Term, Ty},
};
use std::collections::BTreeMap;

pub fn check(
    effects: &Effects,
    module: &str,
    symbols: &Symbols,
    engine: &mut Engine,
    globals: &BTreeMap<SymbolId, Scheme>,
) -> Result<(), String> {
    let Effects::Manager {
        command,
        subscription,
    } = effects
    else {
        return Ok(());
    };
    let named = |engine: &mut Engine, home: &str, name: &str, args| {
        let id = symbols
            .lookup(home, name, Space::Type)
            .ok_or_else(|| format!("missing effect type {home}.{name}"))?;
        Ok::<Ty, String>(engine.term(Term::Named(id, args)))
    };
    let require = |engine: &mut Engine, name: &str, expected| {
        let id = symbols
            .lookup(module, name, Space::Value)
            .ok_or_else(|| format!("missing effect function {name}"))?;
        let scheme = globals
            .get(&id)
            .ok_or_else(|| format!("missing inferred effect function {name}"))?;
        let actual = engine.instantiate(scheme, 1);
        engine
            .unify(actual, expected)
            .map_err(|e| format!("effect manager {name}: {e}"))
    };
    // The state and self-message variables are shared across all three callbacks.
    // Application-message variables are independent, as in the reference solver.
    let state = engine.variable(1, Constraint::Any);
    let self_msg = engine.variable(1, Constraint::Any);
    let msg1 = engine.variable(1, Constraint::Any);
    let msg2 = engine.variable(1, Constraint::Any);
    let never = named(engine, "elm/core:Basics", "Never", vec![])?;
    let task = named(engine, "elm/core:Platform", "Task", vec![never, state])?;
    require(engine, "init", task)?;
    let router = named(engine, "elm/core:Platform", "Router", vec![msg1, self_msg])?;
    let mut arguments = vec![router];
    for effect in [command, subscription].into_iter().flatten() {
        let effect = named(engine, module, effect, vec![msg1])?;
        arguments.push(named(engine, "elm/core:List", "List", vec![effect])?);
    }
    arguments.push(state);
    let signature = arrows(engine, &arguments, task);
    require(engine, "onEffects", signature)?;
    let router = named(engine, "elm/core:Platform", "Router", vec![msg2, self_msg])?;
    let signature = arrows(engine, &[router, self_msg, state], task);
    require(engine, "onSelfMsg", signature)?;
    for (effect, function) in [(command, "cmdMap"), (subscription, "subMap")] {
        if let Some(effect) = effect {
            let a = engine.variable(1, Constraint::Any);
            let b = engine.variable(1, Constraint::Any);
            let mapper = engine.term(Term::Function(a, b));
            let input = named(engine, module, effect, vec![a])?;
            let output = named(engine, module, effect, vec![b])?;
            let signature = arrows(engine, &[mapper, input], output);
            require(engine, function, signature)?;
        }
    }
    Ok(())
}
fn arrows(engine: &mut Engine, arguments: &[Ty], result: Ty) -> Ty {
    arguments
        .iter()
        .rev()
        .fold(result, |tail, arg| engine.term(Term::Function(*arg, tail)))
}
