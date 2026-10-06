//! Missing record fields, with the compiler's nearby-field suggestions.
use crate::{
    ast::{Expr, ExprId, Span},
    docs_diagnostic::{position, reflow, text},
    repl_doc::Doc,
    type_localizer::Localizer,
    unify::{Engine, Term, Ty},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(crate) fn capture(
    source: crate::types::SourceTypes<'_, '_>,
    engine: &mut Engine,
    access: ExprId,
    base: ExprId,
    actual: Ty,
    expected: Ty,
) -> Option<String> {
    let crate::types::SourceTypes { ast, symbols, .. } = source;
    let expression = &ast.expressions[access.0 as usize];
    let update = matches!(expression.kind, Expr::Record { base: Some(_), .. });
    if !engine
        .structure(actual)
        .is_some_and(|term| matches!(&*term, Term::Record { .. }))
    {
        let mut context = crate::annotation_diagnostic::Context::body("");
        match &expression.kind {
            Expr::Access(_, field) => context.access = Some((expression.span, field.to_string())),
            Expr::Record {
                base: Some(name), ..
            } => context.update_base = Some(source_span(ast.source, name)?),
            _ => return None,
        }
        return crate::annotation_diagnostic::capture(
            source, engine, &context, base, actual, expected,
        );
    }

    let mut fields = BTreeMap::new();
    let mut cursor = actual;
    let mut seen = BTreeSet::new();
    let extension = loop {
        if !seen.insert(cursor) {
            return None;
        }
        let Some(term) = engine.structure(cursor) else {
            break Some(cursor);
        };
        let Term::Record {
            fields: row,
            extension,
        } = &*term
        else {
            return None;
        };
        fields.extend(row.iter().map(|(name, ty)| (name.clone(), *ty)));
        if let Some(next) = extension {
            cursor = *next;
        } else {
            break None;
        }
    };
    let field = match &expression.kind {
        Expr::Access(_, field) => *field,
        Expr::Record {
            fields: updates, ..
        } => updates
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| !fields.contains_key(&crate::edition::FieldName::from(*name)))
            .min()?,
        _ => return None,
    };
    let mut ordered: Vec<_> = fields.into_iter().collect();
    ordered.sort_by_key(|(name, _)| crate::name_diagnostic::distance(field, name));
    let mut roots: Vec<_> = ordered.iter().map(|(_, ty)| *ty).collect();
    roots.extend(extension);
    let context_root = roots.len();
    roots.push(actual);
    let graph = engine
        .export_types(&roots, |id| {
            let s = symbols.get(id);
            Ok(json!([s.module.as_ref(), s.name.as_ref()]).to_string())
        })
        .ok()?;
    let localizer = Localizer::from_header(&ast.header, "application");
    let mut documents: Vec<_> =
        crate::repl_type::record_documents(&graph, context_root, &localizer)
            .ok()?
            .into_iter()
            .map(Some)
            .collect();
    let mut lines = Vec::new();
    let abbreviated = ordered.len() > 4;
    if extension.is_some() && !abbreviated {
        lines.push(Doc::concat(vec![
            Doc::text("{ "),
            documents[ordered.len()].take()?,
        ]));
    }
    for (index, (name, _)) in ordered.iter().take(4).enumerate() {
        let entry = Doc::sep(vec![
            Doc::text(format!("{name} :")),
            documents[index].take()?,
        ])
        .hang(4);
        let prefix = if extension.is_some() && !abbreviated {
            if index == 0 { "    | " } else { "    , " }
        } else if index == 0 {
            "{ "
        } else {
            ", "
        };
        lines.push(Doc::concat(vec![Doc::text(prefix), entry]));
    }
    if abbreviated {
        lines.push(Doc::text(", ..."));
    }
    lines.push(Doc::text("}"));
    let mut parts = Vec::new();
    for line in lines {
        if !parts.is_empty() {
            parts.push(Doc::Line(false));
        }
        parts.push(line);
    }
    let nearby = Doc::concat(parts).render(76);
    let region = ast.expressions[access.0 as usize].span;
    let name = match &ast.expressions[base.0 as usize].kind {
        Expr::Record {
            base: Some(name), ..
        } => Some(*name),
        Expr::Var(name) => name.rsplit('.').next(),
        _ => None,
    };
    let highlight = if update {
        source_span(ast.source, field)?
    } else {
        Span {
            start: region.end - field.len() as u32,
            end: region.end,
        }
    };
    let data = json!({"update":update,"highlight":[highlight.start,highlight.end],"kind":"record_access","name":name,"field":field,"region":[region.start,region.end],"nearby":nearby,"nearest":ordered.first().map(|(name, _)| name.as_ref())});
    Some(crate::source_error::locate(
        ast.source,
        if update {
            source_span(ast.source, name?)?
        } else {
            ast.expressions[base.0 as usize].span
        },
        crate::name_diagnostic::annotation(data),
    ))
}

pub(crate) fn report(
    source: &str,
    module: &str,
    path: &Path,
    start: (usize, usize),
    end: (usize, usize),
    data: &Value,
) -> Option<Value> {
    let field = data["field"].as_str()?;
    let name = data["name"]
        .as_str()
        .map(|name| format!("`{name}` "))
        .unwrap_or_default();
    let region = Span {
        start: data["region"][0].as_u64()? as u32,
        end: data["region"][1].as_u64()? as u32,
    };
    let determiner = if data["update"] == true {
        "The"
    } else {
        "This"
    };
    let heading = format!("{determiner} {name}record does not have a `{field}` field:");
    let mut message = crate::docs_diagnostic::snippet_highlight(
        source,
        region,
        position(source, data["highlight"][0].as_u64()? as u32)?,
        position(source, data["highlight"][1].as_u64()? as u32)?,
        &heading,
    )?;
    if let Some(nearest) = data["nearest"].as_str() {
        text(
            &mut message,
            format!(
                "\n{}\n\n    {}\n\n",
                reflow(&format!(
                    "This is usually a typo. Here are the {name}fields that are most similar:"
                )),
                data["nearby"].as_str()?.replace('\n', "\n    ")
            ),
        );
        let suggestion = vec![
            json!("So maybe "),
            json!({"bold":false,"underline":false,"color":"yellow","string":field}),
            json!(" should be "),
            json!({"bold":false,"underline":false,"color":"GREEN","string":nearest}),
            json!("?"),
        ];
        for chunk in crate::docs_diagnostic::reflow_chunks(suggestion) {
            if let Some(value) = chunk.as_str() {
                text(&mut message, value.into());
            } else {
                message.push(chunk);
            }
        }
    } else {
        text(
            &mut message,
            if data["update"] == true {
                format!("\nIn fact, {name}is a record with NO fields!")
            } else {
                "\nIn fact, it is a record with NO fields!".into()
            },
        );
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{"title":"TYPE MISMATCH","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    )
}

fn source_span(source: &str, fragment: &str) -> Option<Span> {
    let start = (fragment.as_ptr() as usize).checked_sub(source.as_ptr() as usize)?;
    let end = start.checked_add(fragment.len())?;
    (source.get(start..end)? == fragment).then_some(Span {
        start: start as u32,
        end: end as u32,
    })
}
