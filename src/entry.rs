//! Entry-point validation after inference. Mirrors Optimize.Module's accepted
//! VirtualDom.Node / Platform.Program shapes and Canonicalize.Effects payloads.
use crate::{
    names::Symbols,
    unify::{Engine, Scheme, Term, Ty},
};
use std::collections::BTreeSet;
pub fn check_main(engine: &mut Engine, symbols: &Symbols, scheme: &Scheme) -> Result<(), String> {
    let term = engine
        .structure(scheme.root)
        .ok_or("bad main type: unresolved type variable")?;
    if let Term::Named(id, args) = &*term {
        let symbol = symbols.get(*id);
        match (
            symbol.module.as_ref(),
            symbol.name.as_ref(),
            args.as_slice(),
        ) {
            ("elm/virtual-dom:VirtualDom", "Node", [_]) => return Ok(()),
            ("elm/core:Platform", "Program", [flags, _, _]) => {
                return payload(engine, symbols, *flags)
                    .map_err(|e| format!("bad main flags: {e}"));
            }
            _ => {}
        }
    }
    Err("bad main type: expected Html/VirtualDom.Node or Platform.Program".into())
}
pub fn payload(engine: &mut Engine, symbols: &Symbols, root: Ty) -> Result<(), String> {
    check_payload(engine, symbols, root, false)
}
fn check_payload(
    engine: &mut Engine,
    symbols: &Symbols,
    root: Ty,
    port: bool,
) -> Result<(), String> {
    let mut pending = vec![(root, false)];
    let mut seen = BTreeSet::new();
    while let Some((ty, row)) = pending.pop() {
        let ty = engine.find(ty);
        if !seen.insert((ty, row)) {
            continue;
        }
        let term = engine
            .structure(ty)
            .ok_or("payload contains a type variable or open record")?;
        if row && !matches!(&*term, Term::Record { .. }) {
            return Err("payload record extension is not a record".into());
        }
        match &*term {
            Term::Alias(_, _, real) => pending.push((*real, row)),
            Term::Unit => {}
            Term::Tuple(parts) => pending.extend(parts.iter().map(|ty| (*ty, false))),
            Term::Record { fields, extension } => {
                // Canonicalize.Effects rejects syntactically extended records,
                // even when an alias supplies a closed record for the row.
                // Main flags are checked after inference and row flattening.
                if port && extension.is_some() {
                    return Err("port payload contains an extended record".into());
                }
                pending.extend(fields.values().map(|ty| (*ty, false)));
                pending.extend(extension.iter().map(|ty| (*ty, true)));
            }
            Term::Named(id, args) => {
                let symbol = symbols.get(*id);
                match (
                    symbol.module.as_ref(),
                    symbol.name.as_ref(),
                    args.as_slice(),
                ) {
                    ("elm/core:Basics", "Int" | "Float" | "Bool", [])
                    | ("elm/core:String", "String", [])
                    | ("elm/json:Json.Encode", "Value", []) => {}
                    ("elm/core:List", "List", [arg])
                    | ("elm/core:Maybe", "Maybe", [arg])
                    | ("elm/core:Array", "Array", [arg]) => pending.push((*arg, false)),
                    _ => {
                        return Err(format!(
                            "unsupported payload type {}.{}",
                            symbol.module, symbol.name
                        ));
                    }
                }
            }
            Term::Function(_, _) => return Err("payload contains a function".into()),
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Incoming,
    Outgoing,
}
#[derive(Debug, Clone, Copy)]
pub struct Port {
    pub direction: Direction,
    pub payload: Ty,
    pub message: Ty,
}
/// Validate the declared (uninstantiated) port signature. Variable identity is
/// significant for an incoming port's callback and Sub message parameter.
pub fn check_port(engine: &mut Engine, symbols: &Symbols, root: Ty) -> Result<Port, String> {
    let mut result = root;
    let mut args = Vec::new();
    let final_type = loop {
        let term = engine
            .structure(result)
            .ok_or("port must return Cmd or Sub")?;
        if let Term::Function(argument, rest) = &*term {
            args.push(*argument);
            result = *rest;
        } else {
            break term;
        }
    };
    let Term::Named(id, parameters) = &*final_type else {
        return Err("port must return Cmd or Sub".into());
    };
    let symbol = symbols.get(*id);
    let [message] = parameters.as_slice() else {
        return Err("port must return Cmd msg or Sub msg".into());
    };
    if engine.structure(*message).is_some() {
        return Err("port message must be a type variable".into());
    }
    let [argument] = args.as_slice() else {
        return Err("port must have exactly one argument".into());
    };
    let (direction, data) = match (symbol.module.as_ref(), symbol.name.as_ref()) {
        ("elm/core:Platform.Cmd", "Cmd") => (Direction::Outgoing, *argument),
        ("elm/core:Platform.Sub", "Sub") => {
            let callback = engine
                .structure(*argument)
                .ok_or("incoming port requires a callback")?;
            let Term::Function(data, callback_message) = &*callback else {
                return Err("incoming port requires a callback".into());
            };
            if engine.find(*callback_message) != engine.find(*message) {
                return Err("incoming port callback and Sub message variables must match".into());
            }
            (Direction::Incoming, *data)
        }
        _ => return Err("port must return Cmd or Sub from elm/core".into()),
    };
    check_payload(engine, symbols, data, true).map_err(|e| format!("invalid port payload: {e}"))?;
    Ok(Port {
        direction,
        payload: data,
        message: *message,
    })
}

pub fn check_module(ast: &crate::ast::Syntax<'_>, owner: &str) -> Result<(), String> {
    use crate::{ast::Declaration, module::Effects};
    let kernel = owner.starts_with("elm/") || owner.starts_with("elm-explorations/");
    if !kernel
        && ast
            .declarations
            .iter()
            .any(|d| matches!(d, Declaration::Infix { .. }))
    {
        return Err("operator declarations require a kernel package".into());
    }
    let ports = ast
        .declarations
        .iter()
        .filter(|d| matches!(d, Declaration::Port { .. }))
        .count();
    if !ast.header.explicit {
        return Ok(());
    }
    match &ast.header.effects {
        Effects::None if ports > 0 && owner != "application" => {
            Err("packages cannot declare ports".into())
        }
        Effects::None if ports > 0 => Err("port declarations require a port module".into()),
        Effects::Ports if owner != "application" => {
            Err("packages cannot contain port modules".into())
        }
        Effects::Ports if ports == 0 => Err("port module must declare at least one port".into()),
        Effects::Manager {
            command,
            subscription,
        } => {
            if !kernel {
                return Err("effect modules require a kernel package".into());
            }
            if ports > 0 {
                return Err("effect managers cannot declare ports".into());
            }
            let values: BTreeSet<_> = ast
                .declarations
                .iter()
                .filter_map(|d| {
                    if let Declaration::Value { name, .. } = d {
                        Some(*name)
                    } else {
                        None
                    }
                })
                .collect();
            for name in ["init", "onEffects", "onSelfMsg"] {
                if !values.contains(name) {
                    return Err(format!("effect manager is missing {name}"));
                }
            }
            for (target, map) in [(command, "cmdMap"), (subscription, "subMap")] {
                if let Some(target) = target {
                    if !ast
                        .declarations
                        .iter()
                        .any(|d| matches!(d,Declaration::Union{name,..} if *name==target))
                    {
                        return Err(format!(
                            "effect manager type {target} must be a local union"
                        ));
                    }
                    if !values.contains(map) {
                        return Err(format!("effect manager is missing {map}"));
                    }
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
