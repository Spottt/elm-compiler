//! Plain terminal rendering of package API changes, following terminal/Diff.hs.
use crate::{
    api_diff::{Changes, Magnitude, PackageChanges},
    ast::{Declaration, Syntax, Type, TypeId},
    repl_doc::Doc,
};
use serde_json::Value;
fn magnitude(m: Magnitude) -> &'static str {
    match m {
        Magnitude::Patch => "PATCH",
        Magnitude::Minor => "MINOR",
        Magnitude::Major => "MAJOR",
    }
}
fn tipe(ast: &Syntax<'_>, id: TypeId, context: u8) -> Doc {
    let (doc, precedence) = match &ast.types[id.0 as usize].kind {
        Type::Var(name) => (Doc::text(*name), 2),
        Type::Unit => (Doc::text("()"), 2),
        Type::Constructor(name, args) => {
            let mut parts = vec![Doc::text(name.rsplit('.').next().unwrap_or(name))];
            parts.extend(args.iter().map(|id| tipe(ast, *id, 2)));
            (Doc::sep(parts).hang(4), if args.is_empty() { 2 } else { 1 })
        }
        Type::Function(_, _) => {
            let mut parts = Vec::new();
            let mut cursor = id;
            while let Type::Function(a, b) = ast.types[cursor.0 as usize].kind {
                let doc = tipe(ast, a, 1);
                parts.push(if parts.is_empty() {
                    doc
                } else {
                    Doc::concat(vec![Doc::text("-> "), doc])
                });
                cursor = b;
            }
            parts.push(Doc::concat(vec![Doc::text("-> "), tipe(ast, cursor, 1)]));
            (Doc::sep(parts).align(), 0)
        }
        Type::Tuple(args) => {
            let parts = args
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    Doc::concat(vec![
                        Doc::text(if i == 0 { "( " } else { ", " }),
                        tipe(ast, *id, 0),
                    ])
                })
                .collect();
            (Doc::sep(vec![Doc::cat(parts), Doc::text(")")]).align(), 2)
        }
        Type::Record { extension, fields } => {
            let entries: Vec<_> = fields
                .iter()
                .map(|(name, id)| {
                    Doc::sep(vec![Doc::text(format!("{name} :")), tipe(ast, *id, 0)]).hang(4)
                })
                .collect();
            let doc = if let Some(row) = extension {
                let entries = entries
                    .into_iter()
                    .enumerate()
                    .map(|(i, d)| Doc::concat(vec![Doc::text(if i == 0 { "| " } else { ", " }), d]))
                    .collect();
                Doc::sep(vec![
                    Doc::sep(vec![Doc::text(format!("{{ {row}")), Doc::cat(entries)]).hang(4),
                    Doc::text("}"),
                ])
                .align()
            } else if entries.is_empty() {
                Doc::text("{}")
            } else {
                let entries = entries
                    .into_iter()
                    .enumerate()
                    .map(|(i, d)| Doc::concat(vec![Doc::text(if i == 0 { "{ " } else { ", " }), d]))
                    .collect();
                Doc::sep(vec![Doc::cat(entries), Doc::text("}")]).align()
            };
            (doc, 2)
        }
    };
    if precedence < context {
        Doc::cat(vec![Doc::text("("), doc, Doc::text(")")])
    } else {
        doc
    }
}
fn type_doc(text: &str, context: u8) -> Result<Doc, String> {
    let source = format!("module Documentation exposing (..)\ntype alias Documented = {text}\n");
    let ast = crate::parser::parse(&source)?;
    match ast.declarations.as_slice() {
        [Declaration::Alias { ty, .. }] => Ok(tipe(&ast, *ty, context)),
        _ => Err("invalid documented type".into()),
    }
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v[key].as_str().ok_or(format!("invalid docs {key}"))
}
fn entry(kind: &str, name: &str, v: &Value, width: usize, ansi: bool) -> Result<String, String> {
    let args = || -> Result<String, String> {
        Ok(v["args"]
            .as_array()
            .ok_or("missing type parameters")?
            .iter()
            .map(|x| {
                x.as_str()
                    .map(|s| format!(" {s}"))
                    .ok_or("invalid type parameter".into())
            })
            .collect::<Result<Vec<_>, String>>()?
            .concat())
    };
    let doc = match kind {
        "unions" => {
            // Elm joins the header with an unconditional space before its
            // parameter document, including when that document is empty.
            let parameters = args()?;
            let mut parts = vec![Doc::text(format!(
                "type {name} {}",
                parameters.trim_start()
            ))];
            for (i, case) in v["cases"]
                .as_array()
                .ok_or("missing cases")?
                .iter()
                .enumerate()
            {
                let mut ctor = vec![Doc::text(case[0].as_str().ok_or("missing constructor")?)];
                for arg in case[1].as_array().ok_or("missing constructor arguments")? {
                    ctor.push(type_doc(
                        arg.as_str().ok_or("missing constructor type")?,
                        2,
                    )?);
                }
                parts.push(Doc::concat(vec![
                    Doc::text(if i == 0 { "= " } else { "| " }),
                    Doc::sep(ctor).hang(4),
                ]));
            }
            Doc::sep(parts).hang(4)
        }
        "aliases" => Doc::sep(vec![
            Doc::text(format!("type alias {name}{} =", args()?)),
            type_doc(string(v, "type")?, 0)?,
        ])
        .hang(4),
        "values" => Doc::sep(vec![
            Doc::text(format!("{name} :")),
            type_doc(string(v, "type")?, 0)?,
        ])
        .hang(4),
        "binops" => Doc::concat(vec![
            Doc::text(format!("({name}) : ")),
            type_doc(string(v, "type")?, 0)?,
            Doc::text(format!(
                "    ({}/{})",
                string(v, "associativity")?,
                v["precedence"]
            )),
        ]),
        _ => return Err("unknown documented entry".into()),
    };
    let rendered = doc.render(width);
    if kind == "binops" && ansi {
        let suffix = format!("    ({}/{})", string(v, "associativity")?, v["precedence"]);
        let prefix = rendered.strip_suffix(&suffix).ok_or("missing operator fixity")?;
        Ok(format!("{prefix}{}", styled(90, &suffix, true)))
    } else {
        Ok(rendered)
    }
}
fn indent(text: &str, prefix: &str) -> String {
    text.split('\n')
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
fn section(category: &str, groups: [(&str, &Changes); 4], ansi: bool) -> Result<Option<String>, String> {
    let mut entries = Vec::new();
    for (kind, changes) in groups {
        match category {
            "Added" | "Removed" => {
                for (name, value) in if category == "Added" {
                    &changes.added
                } else {
                    &changes.removed
                } {
                    entries.push(indent(&entry(kind, name, value, 72, ansi)?, "    "));
                }
            }
            "Changed" => {
                for (name, (old, new)) in &changes.changed {
                    let render = |v| -> Result<String, String> {
                        let text = entry(kind, name, v, 72, ansi)?;
                        let mut lines = text.split('\n');
                        let first = lines.next().unwrap_or("");
                        Ok(format!(
                            "{first}{}",
                            lines.map(|l| format!("\n    {l}")).collect::<String>()
                        ))
                    };
                    entries.push(format!("  - {}\n  + {}\n", render(old)?, render(new)?));
                }
            }
            _ => unreachable!(),
        }
    }
    Ok((!entries.is_empty()).then(|| format!("{category}:\n{}", entries.join("\n"))))
}
fn styled(code: u8, text: &str, ansi: bool) -> String {
    if ansi { format!("\x1b[{code}m{text}\x1b[0m") } else { text.into() }
}
pub fn render(changes: &PackageChanges) -> Result<String, String> {
    render_terminal(changes, false)
}
pub fn render_terminal(changes: &PackageChanges, ansi: bool) -> Result<String, String> {
    if changes.added.is_empty() && changes.changed.is_empty() && changes.removed.is_empty() {
        return Ok(format!("No API changes detected, so this is a {} change.\n", styled(92, "PATCH", ansi)));
    }
    let mut out = format!("This is a {} change.\n\n", styled(92, magnitude(changes.magnitude()), ansi));
    for (title, mag, names) in [
        ("ADDED MODULES", Magnitude::Minor, &changes.added),
        ("REMOVED MODULES", Magnitude::Major, &changes.removed),
    ] {
        if !names.is_empty() {
            out.push_str(&format!(
                "{}\n\n{}\n\n\n",
                styled(36, &format!("---- {title} - {} ----", magnitude(mag)), ansi),
                indent(&names.join("\n"), "    ")
            ));
        }
    }
    for (name, c) in &changes.changed {
        let groups = [
            ("unions", &c.unions),
            ("aliases", &c.aliases),
            ("binops", &c.binops),
            ("values", &c.values),
        ];
        let sections = ["Added", "Removed", "Changed"]
            .iter()
            .map(|category| section(category, groups, ansi))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        out.push_str(&format!(
            "{}\n\n{}\n\n\n",
            styled(36, &format!("---- {name} - {} ----", magnitude(c.magnitude())), ansi),
            indent(&sections.join("\n\n"), "    ")
        ));
    }
    Ok(out)
}
