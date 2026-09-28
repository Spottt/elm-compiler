//! Module effect restrictions from Reporting.Error.Syntax.checkEffects.
use crate::{ast::{Declaration, Syntax}, docs_diagnostic::{reflow, snippet_positions, text}};
use serde_json::{Value, json};
use std::path::Path;

fn styled(value: &str, color: Option<&str>, underline: bool) -> Value {
    json!({"bold":false,"underline":underline,"color":color,"string":value})
}
fn note(message: &mut Vec<Value>, body: &str) {
    message.push(styled("Note", None, true));
    text(message, reflow(&format!("Note: {body}"))[4..].to_string());
}
fn package_note(message: &mut Vec<Value>) {
    note(message, "One of the major goals of the package ecosystem is to be completely written in Elm. This means when you install an Elm package, you can be sure you are safe from security issues on install and that you are not going to get any runtime exceptions coming from your new dependency. This design also sets the ecosystem up to target other platforms more easily (like mobile phones, WebAssembly, etc.) since no community code explicitly depends on JavaScript even existing.");
    text(message, format!("\n\n{}", reflow("Given that overall goal, allowing ports in packages would lead to some pretty surprising behavior. If ports were allowed in packages, you could install a package but not realize that it brings in an indirect dependency that defines a port. Now you have a program that does not work and the fix is to realize that some JavaScript needs to be added for a dependency you did not even know about. That would be extremely frustrating! \"So why not allow the package author to include the necessary JS code as well?\" Now we are back in conflict with our overall goal to keep all community packages free from runtime exceptions.")));
}
pub fn report(ast: &Syntax<'_>, path: &Path, error: &str) -> Option<Value> {
    let (title, preface) = match error {
        "port declarations require a port module" | "effect managers cannot declare ports" => ("UNEXPECTED PORTS", "You are declaring ports in a normal module."),
        "port module must declare at least one port" => ("NO PORTS", "This module does not declare any ports, but it says it will:"),
        "packages cannot contain port modules" | "packages cannot declare ports" => ("PACKAGES CANNOT HAVE PORTS", "Packages cannot declare any ports, so I am getting stuck here:"),
        "effect modules require a kernel package" => ("INVALID EFFECT MODULE", "It is not possible to declare an `effect module` outside the @elm organization, so I am getting stuck here:"),
        _ => return None,
    };
    let tokens = crate::lexer::lex(ast.source).ok()?;
    let (first, last) = if error == "packages cannot declare ports" {
        let name = ast.declarations.iter().find_map(|d| match d { Declaration::Port {name, ..} => Some(*name), _ => None })?;
        let offset = name.as_ptr() as usize - ast.source.as_ptr() as usize;
        let token = tokens.iter().find(|t| t.start as usize == offset)?;
        (token, token)
    } else {
        (tokens.first()?, tokens.iter().find(|t| t.text(ast.source) == "module")?)
    };
    let start = (first.row as usize, first.column as usize);
    let end = (last.row as usize, last.column as usize + last.text(ast.source).chars().count());
    let mut message = snippet_positions(ast.source, start, end, preface)?;
    text(&mut message, "\n".into());
    match error {
        "port declarations require a port module" | "effect managers cannot declare ports" => {
            text(&mut message, "Switch this to say ".into());
            message.push(styled("port module", Some("CYAN"), false));
            text(&mut message, " instead, marking that this module contains port\ndeclarations.\n\n".into());
            note(&mut message, "Ports are not a traditional FFI for calling JS functions directly. They need a different mindset! Read <https://elm-lang.org/0.19.1/ports> to learn the syntax and how to use it effectively.");
        }
        "port module must declare at least one port" => {
            text(&mut message, "Switch this to ".into());
            message.push(styled("module", Some("CYAN"), false));
            text(&mut message, " and you should be all set!".into());
        }
        "packages cannot contain port modules" => {
            text(&mut message, "Remove the ".into());
            message.push(styled("port", Some("CYAN"), false));
            text(&mut message, " keyword and I should be able to continue.\n\n".into());
            package_note(&mut message);
        }
        "packages cannot declare ports" => {
            text(&mut message, "Remove this port declaration.\n\n".into());
            package_note(&mut message);
        }
        _ => {
            text(&mut message, "Switch to a normal module declaration.\n\n".into());
            note(&mut message, "Effect modules are designed to allow certain core functionality to be defined separately from the compiler. So the @elm organization has access to this so that certain changes, extensions, and fixes can be introduced without needing to release new Elm binaries. For example, we want to make it possible to test effects, but this may require changes to the design of effect modules. By only having them defined in the @elm organization, that kind of design work can proceed much more smoothly.");
        }
    }
    Some(json!({"type":"compile-errors","errors":[{"path":path,"name":ast.header.name,"problems":[{
        "title":title,"region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message
    }]}]}))
}
