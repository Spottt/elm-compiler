//! Port reports from Elm 0.19.1 Reporting.Error.Canonicalize (LICENSE-ELM).
use crate::{
    ast::Syntax,
    docs_diagnostic::{reflow, reflow_width, snippet_positions, text},
    names::Symbols,
    unify::{Engine, Term, Ty},
};
use serde_json::{Value, json};

pub fn report(
    ast: &Syntax<'_>,
    owner: &str,
    name: &str,
    engine: &mut Engine,
    symbols: &Symbols,
    root: Ty,
) -> Option<String> {
    let mut result = root;
    let mut args = Vec::new();
    let final_type = loop {
        match engine.structure(result).as_deref() {
            Some(Term::Alias(_, _, real)) => result = *real,
            Some(Term::Function(arg, rest)) => {
                args.push(*arg);
                result = *rest;
            }
            other => break other.cloned(),
        }
    };
    let mut payload = None;
    let mut sample = false;
    let (before, advice) = match final_type {
        Some(Term::Named(id, ref parameters)) if symbols.get(id).module.as_ref() == "elm/core:Platform.Cmd" && symbols.get(id).name.as_ref() == "Cmd" && parameters.len() == 1 => {
            match args.as_slice() {
                [] => (format!("The `{name}` port cannot be just a command."), "It can be (() -> Cmd msg) if you just need to trigger a JavaScript function, but there is often a better way to set things up.".into()),
                [arg] if engine.structure(parameters[0]).is_none() => { payload = Some(*arg); (String::new(), String::new()) },
                [_] => (format!("The `{name}` port cannot send any messages to the `update` function."), "It must produce a (Cmd msg) type. Notice the lower case `msg` type variable. The command will trigger some JS code, but it will not send anything particular back to Elm.".into()),
                _ => {
                    let items = match args.len() { 2 => "both of these items into a tuple or record".into(), 3 => "these 3 items into a tuple or record".into(), n => format!("these {n} items into a record") };
                    (format!("The `{name}` port can only send ONE value out to JavaScript."), format!("You can put {items} to send them out though."))
                }
            }
        }
        Some(Term::Named(id, ref parameters)) if symbols.get(id).module.as_ref() == "elm/core:Platform.Sub" && symbols.get(id).name.as_ref() == "Sub" && parameters.len() == 1 => {
            if let [arg] = args.as_slice()
                && let Some(Term::Function(data, message)) = engine.structure(*arg).as_deref()
                && engine.structure(*message).is_none()
                && engine.find(*message) == engine.find(parameters[0]) { payload = Some(*data); }
            sample = payload.is_none();
            (format!("There is something off about this `{name}` port declaration."), "To receive messages from JavaScript, you need to define a port like this:".into())
        }
        _ => (format!("I am confused about the `{name}` port declaration."), "Ports need to produce a command (Cmd) or a subscription (Sub) but this is neither. I do not know how to handle this.".into()),
    };
    let (title, before, advice, hint) = if let Some(payload) = payload {
        let (kind, mut advice) =
            crate::main_diagnostic::payload_details(ast, owner, engine, symbols, payload, true)?;
        advice = advice
            .replace("cannot be given as flags", "cannot flow through ports")
            .replace("CAN be in flags", "CAN flow in and out of Elm");
        if kind == "a function" {
            advice = "But functions cannot be sent in and out ports. If we allowed functions in from JS they may perform some side-effects. If we let functions out, they could produce incorrect results because Elm optimizations assume there are no side-effects.".into();
        }
        (
            "PORT ERROR",
            format!("The `{name}` port is trying to transmit {kind}:"),
            advice,
            "Hint: Ports are not a traditional FFI, so if you have tons of annoying ports, definitely read <https://elm-lang.org/0.19.1/ports> to learn how they are meant to work. They require a different mindset!",
        )
    } else {
        (
            "BAD PORT",
            before,
            advice,
            "Hint: Read <https://elm-lang.org/0.19.1/ports> for more advice. For example, do not end up with one port per JS function!",
        )
    };
    let offset = (name.as_ptr() as usize).checked_sub(ast.source.as_ptr() as usize)?;
    let prefix = ast.source.get(..offset)?;
    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next()?.chars().count() + 1;
    let end = column + name.chars().count();
    let mut message = snippet_positions(ast.source, (line, column), (line, end), &before)?;
    let advice = advice
        .split("\n\n")
        .map(|p| {
            if let Some(body) = p.strip_prefix("    ") {
                format!("    {}", reflow_width(body, 76).replace('\n', "\n    "))
            } else {
                reflow(p)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    text(&mut message, format!("\n{advice}"));
    if sample {
        text(&mut message, "\n\n    ".into());
        message.push(json!({"bold":false,"underline":false,"color":"yellow","string":format!("port {name} : (Int -> msg) -> Sub msg")}));
        text(
            &mut message,
            format!(
                "\n\n{}",
                reflow(
                    "Now every time JS sends an `Int` to this port, it is converted to a `msg`. And if you subscribe, those `msg` values will be piped into your `update` function. The only thing you can customize here is the `Int` type."
                )
            ),
        );
    }
    text(&mut message, "\n\n".into());
    message.push(json!({"bold":false,"underline":true,"color":Value::Null,"string":"Hint"}));
    text(&mut message, reflow(hint).strip_prefix("Hint")?.into());
    Some(crate::docs_diagnostic::encode(
        &json!({"type":"compile-errors","errors":[{"path":"","name":ast.header.name,"problems":[{"title":title,"region":{"start":{"line":line,"column":column},"end":{"line":line,"column":end}},"message":message}]}]}),
    ))
}
