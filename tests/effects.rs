use planexpo_elm::{
    effects,
    module::Effects,
    names::{Space, SymbolKind, Symbols},
    types,
    unify::{Constraint, Engine, Term, Ty},
};
use std::collections::BTreeMap;
fn named(e: &mut Engine, s: &mut Symbols, home: &str, name: &str, args: Vec<Ty>) -> Ty {
    let id = s.intern(
        home,
        name,
        Space::Type,
        SymbolKind::Type {
            arity: args.len(),
            alias: false,
        },
    );
    e.term(Term::Named(id, args))
}
fn arrows(e: &mut Engine, args: &[Ty], result: Ty) -> Ty {
    args.iter()
        .rev()
        .fold(result, |tail, a| e.term(Term::Function(*a, tail)))
}
#[test]
fn lifecycle_state_self_messages_and_map_results_are_checked() {
    for mode in ["cmd", "sub", "both"] {
        for fault in ["none", "init", "state", "self", "map", "error"] {
            let mut s = Symbols::default();
            let mut e = Engine::new(types::builtins(&mut s));
            let unit = e.term(Term::Unit);
            let int = named(&mut e, &mut s, "elm/core:Basics", "Int", vec![]);
            let never = named(&mut e, &mut s, "elm/core:Basics", "Never", vec![]);
            let task = named(
                &mut e,
                &mut s,
                "elm/core:Platform",
                "Task",
                vec![never, unit],
            );
            let msg = e.variable(1, Constraint::Any);
            let router = named(
                &mut e,
                &mut s,
                "elm/core:Platform",
                "Router",
                vec![msg, int],
            );
            let command = (mode != "sub").then(|| "Cmd".to_string());
            let subscription = (mode != "cmd").then(|| "Sub".to_string());
            let mut args = vec![router];
            let mut globals = BTreeMap::new();
            let mut values = vec![];
            for (effect, map) in [(&command, "cmdMap"), (&subscription, "subMap")] {
                if let Some(effect) = effect {
                    let item = named(&mut e, &mut s, "elm/test:Manager", effect, vec![msg]);
                    args.push(named(&mut e, &mut s, "elm/core:List", "List", vec![item]));
                    let a = e.variable(1, Constraint::Any);
                    let b = e.variable(1, Constraint::Any);
                    let mapper = e.term(Term::Function(a, b));
                    let input = named(&mut e, &mut s, "elm/test:Manager", effect, vec![a]);
                    let output = named(&mut e, &mut s, "elm/test:Manager", effect, vec![b]);
                    values.push((
                        map,
                        arrows(
                            &mut e,
                            &[mapper, input],
                            if fault == "map" { unit } else { output },
                        ),
                    ));
                }
            }
            args.push(if fault == "state" { int } else { unit });
            values.push(("onEffects", arrows(&mut e, &args, task)));
            values.push((
                "onSelfMsg",
                arrows(
                    &mut e,
                    &[router, if fault == "self" { unit } else { int }, unit],
                    task,
                ),
            ));
            let init = match fault {
                "init" => unit,
                "error" => named(&mut e, &mut s, "elm/core:Platform", "Task", vec![int, unit]),
                _ => task,
            };
            values.push(("init", init));
            for (name, root) in values {
                let id = s.intern("elm/test:Manager", name, Space::Value, SymbolKind::Value);
                globals.insert(id, e.generalize(root, 0));
            }
            let result = effects::check(
                &Effects::Manager {
                    command,
                    subscription,
                },
                "elm/test:Manager",
                &s,
                &mut e,
                &globals,
            );
            assert_eq!(
                result.is_ok(),
                fault == "none",
                "{mode}/{fault}: {result:?}"
            );
        }
    }
}
