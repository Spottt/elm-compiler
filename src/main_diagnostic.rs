//! Main type errors from Elm 0.19.1 Reporting.Error.Main (LICENSE-ELM).
use crate::{
    ast::{Declaration, Syntax},
    docs_diagnostic::{snippet_positions, text},
    names::Symbols,
    type_localizer::Localizer,
    unify::{Engine, Ty},
};
use serde_json::json;
use std::path::Path;
pub fn bad_type(
    ast: &Syntax<'_>,
    owner: &str,
    path: &Path,
    engine: &mut Engine,
    symbols: &Symbols,
    ty: Ty,
) -> Option<String> {
    let name = ast
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            Declaration::Value { name, .. } if *name == "main" => Some(*name),
            _ => None,
        })?;
    let offset = (name.as_ptr() as usize).checked_sub(ast.source.as_ptr() as usize)?;
    let prefix = ast.source.get(..offset)?;
    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next()?.chars().count() + 1;
    let graph = engine
        .export_types(&[ty], |id| {
            let symbol = symbols.get(id);
            Ok(json!([symbol.module.as_ref(), symbol.name.as_ref()]).to_string())
        })
        .ok()?;
    let tipe =
        crate::repl_type::render_width(&graph, 0, &Localizer::from_header(&ast.header, owner), 76)
            .ok()?;
    let mut message = snippet_positions(
        ast.source,
        (line, column),
        (line, column + 4),
        "I cannot handle this type of `main` value:",
    )?;
    text(
        &mut message,
        "\nThe type of `main` value I am seeing is:\n\n    ".into(),
    );
    message.push(json!({"bold":false,"underline":false,"color":"yellow","string":tipe.replace('\n',"\n    ")}));
    text(&mut message,"\n\nI only know how to handle Html, Svg, and Programs though. Modify `main` to be\none of those types of values!".into());
    Some(crate::docs_diagnostic::encode(
        &json!({"type":"compile-errors","errors":[{"path":path,"name":ast.header.name,"problems":[{"title":"BAD MAIN TYPE","region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+4}},"message":message}]}]}),
    ))
}

/// Render the first unsupported flags payload, in canonical field/tuple order.
pub fn bad_flags(
    ast: &Syntax<'_>,
    owner: &str,
    path: &Path,
    engine: &mut Engine,
    symbols: &Symbols,
    main: Ty,
) -> Option<String> {
    use crate::unify::Term;
    let term = engine.structure(main)?;
    let Term::Named(_, args) = &*term else {
        return None;
    };
    let (kind, advice) = payload_details(ast, owner, engine, symbols, *args.first()?, false)?;
    let name = ast.declarations.iter().find_map(|d| match d {
        Declaration::Value {
            name: name @ "main",
            ..
        } => Some(*name),
        _ => None,
    })?;
    let offset = (name.as_ptr() as usize).checked_sub(ast.source.as_ptr() as usize)?;
    let prefix = ast.source.get(..offset)?;
    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next()?.chars().count() + 1;
    let mut message = snippet_positions(
        ast.source,
        (line, column),
        (line, column + 4),
        &format!("Your `main` program wants {kind} from JavaScript."),
    )?;
    let advice = advice
        .split("\n\n")
        .map(|paragraph| {
            if let Some(body) = paragraph.strip_prefix("    ") {
                format!(
                    "    {}",
                    crate::docs_diagnostic::reflow_width(body, 76).replace('\n', "\n    ")
                )
            } else {
                crate::docs_diagnostic::reflow(paragraph)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    text(&mut message, format!("\n{advice}"));
    Some(crate::docs_diagnostic::encode(
        &json!({"type":"compile-errors","errors":[{
        "path":path,"name":ast.header.name,"problems":[{"title":"BAD FLAGS",
        "region":{"start":{"line":line,"column":column},"end":{"line":line,"column":column+4}},"message":message}]}]}),
    ))
}

pub(crate) fn payload_details(
    ast: &Syntax<'_>,
    owner: &str,
    engine: &mut Engine,
    symbols: &Symbols,
    root: Ty,
    port: bool,
) -> Option<(String, String)> {
    use crate::unify::Term;
    let mut pending = vec![root];
    let mut seen = std::collections::BTreeSet::new();
    let (kind, advice) = loop {
        let ty = pending.pop()?;
        if !seen.insert(engine.find(ty)) {
            continue;
        }
        let term = engine.structure(ty);
        match term.as_deref() {
            None => {
                let graph = engine.export_types(&[ty], |_| Err("unexpected named type".into())).ok()?;
                let name = crate::repl_type::render(&graph, 0, &Localizer::from_header(&ast.header, owner)).ok()?;
                break ("an unspecified type".to_owned(), format!("But type variables like `{name}` cannot be given as flags. I need to know exactly what type of data I am getting, so I can guarantee that unexpected data cannot sneak in and crash the Elm program."));
            }
            Some(Term::Alias(_, _, real)) => pending.push(*real),
            Some(Term::Unit) => {},
            Some(Term::Tuple(parts)) => pending.extend(parts.iter().rev().copied()),
            Some(Term::Record { fields, extension }) => {
                if let Some(row) = extension {
                    if port || engine.structure(*row).is_none() {
                        break ("an extended record".into(), "But the exact shape of the record must be known at compile time. No type variables!".into());
                    }
                    pending.push(*row);
                }
                pending.extend(fields.values().rev().copied());
            }
            Some(Term::Function(_, _)) => break ("a function".into(), "But if I allowed functions from JS, it would be possible to sneak side-effects and runtime exceptions into Elm!".into()),
            Some(Term::Named(id, args)) => {
                let symbol = symbols.get(*id);
                match (symbol.module.as_ref(), symbol.name.as_ref(), args.as_slice()) {
                    ("elm/core:Basics", "Int" | "Float" | "Bool", [])
                    | ("elm/core:String", "String", [])
                    | ("elm/json:Json.Encode", "Value", []) => {},
                    ("elm/core:List", "List", [arg])
                    | ("elm/core:Maybe", "Maybe", [arg])
                    | ("elm/core:Array", "Array", [arg]) => pending.push(*arg),
                    _ => break (format!("a `{}` value", symbol.name), "I cannot handle that. The types that CAN be in flags include:\n\n    Ints, Floats, Bools, Strings, Maybes, Lists, Arrays, tuples, records, and JSON values.\n\nSince JSON values can flow through, you can use JSON encoders and decoders to allow other types through as well. More advanced users often just do everything with encoders and decoders for more control and better errors.".into()),
                }
            }
        }
    };
    Some((kind, advice))
}
