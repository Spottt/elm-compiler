//! Annotation-body type mismatch context, retained at the failed constraint.
use crate::{
    ast::{Expr, ExprId, Pattern, PatternId, Span, Syntax},
    docs_diagnostic::{reflow, snippet_positions, text},
    lexer::Kind,
    names::Symbols,
    type_localizer::Localizer,
    unify::{Engine, Ty},
};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Clone)]
pub(crate) struct Context {
    pub name: String,
    pub destructure: Option<Span>,
    pub branch: Option<(&'static str, usize)>,
    pub condition: Option<Span>,
    pub inferred: Option<Span>,
    pub argument: Option<(Span, usize)>,
    pub access: Option<(Span, String)>,
    pub update_base: Option<Span>,
    pub update_value: Option<(Span, String)>,
    pub operator: Option<(Span, &'static str, &'static str)>,
}
impl Context {
    pub fn is_annotation(&self) -> bool {
        self.destructure.is_none()
            && self.condition.is_none()
            && self.inferred.is_none()
            && self.argument.is_none()
            && self.operator.is_none()
            && self.access.is_none()
            && self.update_value.is_none()
            && self.update_base.is_none()
    }
    pub fn condition(region: Span) -> Self {
        Self {
            name: String::new(),
            branch: None,
            destructure: None,
            condition: Some(region),
            inferred: None,
            argument: None,
            operator: None,
            access: None,
            update_value: None,
            update_base: None,
        }
    }
    pub fn body(name: &str) -> Self {
        Self {
            name: name.into(),
            branch: None,
            destructure: None,
            condition: None,
            inferred: None,
            argument: None,
            operator: None,
            access: None,
            update_value: None,
            update_base: None,
        }
    }
}
fn ordinal(index: usize) -> String {
    let suffix = if (11..=13).contains(&(index % 100)) {
        "th"
    } else {
        match index % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    format!("{index}{suffix}")
}
pub(crate) fn call_arity(
    ast: &Syntax<'_>,
    function: ExprId,
    call: ExprId,
    count: usize,
    given: usize,
) -> String {
    let label = match &ast.expressions[function.0 as usize].kind {
        Expr::Var(name) => {
            let name = name.rsplit('.').next().unwrap_or(name);
            let noun = if count == 0 {
                "value"
            } else if name.starts_with(char::is_uppercase) {
                "constructor"
            } else {
                "function"
            };
            format!("The `{name}` {noun}")
        }
        Expr::Operator(name) => format!("The ({name}) operator"),
        _ => if count == 0 {
            "This value"
        } else {
            "This function"
        }
        .into(),
    };
    let region = ast.expressions[call.0 as usize].span;
    crate::source_error::locate(
        ast.source,
        ast.expressions[function.0 as usize].span,
        crate::name_diagnostic::annotation(
            json!({"kind":"call_arity", "label":label, "count":count, "given":given, "region":[region.start,region.end]}),
        ),
    )
}
pub(crate) fn arity_report(
    source: &str,
    module: &str,
    path: &Path,
    start: (usize, usize),
    end: (usize, usize),
    data: &Value,
) -> Option<Value> {
    let count = data["count"].as_u64()?;
    let given = data["given"].as_u64()?;
    let label = data["label"].as_str()?;
    let args = |n| format!("{n} argument{}", if n == 1 { "" } else { "s" });
    let heading = if count == 0 {
        format!(
            "{label} is not a function, but it was given {}.",
            args(given)
        )
    } else {
        format!(
            "{label} expects {}, but it got {given} instead.",
            args(count)
        )
    };
    let region = Span {
        start: data["region"][0].as_u64()? as u32,
        end: data["region"][1].as_u64()? as u32,
    };
    let mut message =
        crate::docs_diagnostic::snippet_highlight(source, region, start, end, &reflow(&heading))?;
    text(
        &mut message,
        "\nAre there any missing commas? Or missing parentheses?".into(),
    );
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{
        "title":"TOO MANY ARGS","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    )
}
fn type_data(
    ast: &Syntax<'_>,
    symbols: &Symbols,
    engine: &mut Engine,
    actual: Ty,
    expected: Ty,
) -> Option<Value> {
    let mut graph = engine
        .export_types(&[actual, expected], |id| {
            let s = symbols.get(id);
            Ok(json!([s.module.as_ref(), s.name.as_ref()]).to_string())
        })
        .ok()?;
    // Error.Type prints canonical record fields in lexical order.
    graph["record_field_order"] = Value::Null;
    let names = Localizer::from_header(&ast.header, "application");
    let actual = crate::repl_type::render_width(&graph, 0, &names, 76).ok()?;
    let expected = crate::repl_type::render_width(&graph, 1, &names, 76).ok()?;
    let actual_root = graph["roots"][0].as_u64()? as usize;
    let actual_node = &graph["nodes"][actual_root];
    let actual_alias = actual_node[0] == "alias";
    let actual_list = actual_node[0] == "named"
        && actual_node[1]
            .as_str()
            .and_then(|name| serde_json::from_str::<(String, String)>(name).ok())
            .is_some_and(|(module, name)| module == "elm/core:List" && name == "List");
    let root = graph["roots"][1].as_u64()? as usize;
    let mut data = json!({"actual":actual,"actual_function":actual_node[0] == "function","actual_list":actual_list,"actual_alias":actual_alias,"expected":expected,"rigid":graph["nodes"][root][0] == "rigid"});
    if let Some(comparison) = crate::type_comparison::compare(&graph, &names) {
        data.as_object_mut()?
            .extend(comparison.as_object()?.clone());
    }
    Some(data)
}
pub(crate) enum PatternContext<'a> {
    Case,
    Constructor(&'a str),
    TypedArgument(&'a str),
    List,
    Tail,
}
pub(crate) fn case_pattern(
    ast: &Syntax<'_>,
    symbols: &Symbols,
    engine: &mut Engine,
    location: (PatternId, ExprId, usize),
    actual: Ty,
    expected: Ty,
) -> Option<String> {
    let (pattern, case, index) = location;
    pattern_mismatch(
        ast,
        symbols,
        engine,
        (
            pattern,
            ast.expressions[case.0 as usize].span,
            index,
            PatternContext::Case,
        ),
        actual,
        expected,
    )
}
pub(crate) fn pattern_mismatch(
    ast: &Syntax<'_>,
    symbols: &Symbols,
    engine: &mut Engine,
    location: (PatternId, Span, usize, PatternContext<'_>),
    actual: Ty,
    expected: Ty,
) -> Option<String> {
    let (mut pattern, region, index, context) = location;
    let (kind, name) = match context {
        PatternContext::Case => ("case", ""),
        PatternContext::Constructor(name) => ("constructor", name),
        PatternContext::TypedArgument(name) => ("typed_argument", name),
        PatternContext::List => ("list", ""),
        PatternContext::Tail => ("tail", ""),
    };
    while let Pattern::Alias(inner, _) = ast.patterns[pattern.0 as usize].kind {
        pattern = inner;
    }
    let category = match &ast.patterns[pattern.0 as usize].kind {
        Pattern::Unit => "unit values".into(),
        Pattern::Tuple(_) => "tuples of type".into(),
        Pattern::List(_) | Pattern::Cons(..) => "lists of type".into(),
        Pattern::Record(_) => "record values of type".into(),
        Pattern::Literal(Kind::Number, _) => "integers".into(),
        Pattern::Literal(Kind::String, _) => "strings".into(),
        Pattern::Literal(Kind::Char, _) => "characters".into(),
        Pattern::Constructor(name, _) => {
            let name = name.rsplit('.').next()?;
            if name == "True" || name == "False" {
                "booleans".into()
            } else {
                format!("`{name}` values of type")
            }
        }
        _ => return None,
    };
    let mut data = type_data(ast, symbols, engine, actual, expected)?;
    data.as_object_mut()?.extend(json!({"kind":"annotation_mismatch","name":name,"pattern_kind":kind,"category":category,"pattern":[region.start as usize,region.end as usize,index]}).as_object()?.clone());
    Some(crate::source_error::locate(
        ast.source,
        ast.patterns[pattern.0 as usize].span,
        crate::name_diagnostic::annotation(data),
    ))
}
pub(crate) fn capture(
    source: crate::types::SourceTypes<'_, '_>,
    engine: &mut Engine,
    context: &Context,
    body: ExprId,
    actual: Ty,
    expected: Ty,
) -> Option<String> {
    let crate::types::SourceTypes { ast, symbols, .. } = source;
    let mut data = type_data(ast, symbols, engine, actual, expected)?;
    if context
        .operator
        .is_some_and(|(_, op, side)| op == "::" && side == "right")
        && data["actual_list"] == true
    {
        let actual_term = engine.structure(actual)?;
        let expected_term = engine.structure(expected)?;
        if let (
            crate::unify::Term::Named(_, actual_args),
            crate::unify::Term::Named(_, expected_args),
        ) = (&*actual_term, &*expected_term)
            && actual_args.len() == 1
            && expected_args.len() == 1
        {
            data = type_data(ast, symbols, engine, expected_args[0], actual_args[0])?;
            data["cons_comparison"] = json!(true);
        }
    }
    if let Some((_, "++", side)) = context.operator {
        let reverse = type_data(ast, symbols, engine, expected, actual)?;
        let numeric = data["actual_rigid"] != true
            && matches!(data["actual"].as_str(), Some("Int" | "Float" | "number"));
        let left_string = reverse["actual"] == "String";
        let left_list = reverse["actual_list"] == true;
        let mode = if side == "left" && numeric {
            "number"
        } else if side == "left" {
            "other"
        } else if left_string && numeric {
            "string_number"
        } else if left_list && numeric {
            "list_number"
        } else if (left_string && data["actual_list"] == true)
            || (left_list && data["actual"] == "String")
        {
            "mixed"
        } else {
            "compare"
        };
        if mode == "compare" {
            data = reverse;
        }
        data["append_mode"] = json!(mode);
    }
    if context.operator.is_some_and(|(_, op, side)| {
        matches!(op, "==" | "/=" | "<" | ">" | "<=" | ">=") && side == "right"
    }) {
        data = type_data(ast, symbols, engine, expected, actual)?;
        data["operator_comparison"] = json!(true);
    }
    if let Some((_, op, "right")) = context.operator {
        if op == "<|" {
            data["pipe_argument"] = json!(true);
        } else if op == "|>" && data["actual_function"] == true {
            let actual_term = engine.structure(actual)?;
            let expected_term = engine.structure(expected)?;
            if let (
                crate::unify::Term::Function(actual_arg, _),
                crate::unify::Term::Function(expected_arg, _),
            ) = (&*actual_term, &*expected_term)
            {
                data = type_data(ast, symbols, engine, *expected_arg, *actual_arg)?;
                data["pipe_argument"] = json!(true);
            }
        }
    }
    let category = match &ast.expressions[body.0 as usize].kind {
        Expr::Literal(Kind::Number, raw) => {
            if !raw.starts_with("0x") && raw.contains(['.', 'e', 'E']) {
                "a float of type"
            } else {
                "a number of type"
            }
            .to_string()
        }
        Expr::Literal(Kind::String, _) => "a string of type".into(),
        Expr::Literal(Kind::Char, _) => "a character of type".into(),
        Expr::If(..) => "This `if` expression produces:".into(),
        Expr::Case(..) => "This `case` expression produces:".into(),
        Expr::Negate(_) => "a number of type".into(),
        Expr::Unit => "a unit value".into(),
        Expr::Tuple(_) => "a tuple of type".into(),
        Expr::List(_) => "a list of type".into(),
        Expr::Record {
            base: Some(name), ..
        } if context.update_base.is_some() => format!("This `{name}` value is a:"),
        Expr::Record { .. } => "a record of type".into(),
        Expr::Lambda(..) => "an anonymous function of type".into(),
        Expr::Var(name) => format!("This `{}` value is a:", name.rsplit('.').next()?),
        Expr::Accessor(name) => format!("This .{name} field access function has type:"),
        Expr::Access(_, name) => format!("The value at .{name} is a:"),
        Expr::Call(fun, _) => match &ast.expressions[fun.0 as usize].kind {
            Expr::Var(_) if matches!(source.resolved.expressions.get(fun.0 as usize),
                Some(Some(crate::names::Binding::Global(symbol)))
                    if symbols.get(*symbol).module.as_ref() == "elm/core:Debug") =>
                "The body is:".into(),
            Expr::Var(name) => format!("This `{}` call produces:", name.rsplit('.').next()?),
            _ => "The body is:".into(),
        },
        _ => "The body is:".into(),
    };
    data.as_object_mut()?.extend(json!({"destructure":context.destructure.map(|span| [span.start,span.end]),"update_base":context.update_base.map(|span| [span.start,span.end]),"kind":"annotation_mismatch","name":context.name,"category":category,"branch":context.branch,"condition":context.condition.map(|span| [span.start,span.end]),"inferred":context.inferred.map(|span| [span.start,span.end]),"argument":context.argument.map(|(span,index)| [span.start as usize,span.end as usize,index]),"update_value":context.update_value.as_ref().map(|(span,field)| json!([span.start,span.end,field])),"access":context.access.as_ref().map(|(span,field)| json!([span.start,span.end,field])),"operator":context.operator.map(|(span,op,side)| json!([span.start,span.end,op,side]))}).as_object()?.clone());
    Some(crate::source_error::locate(
        ast.source,
        context
            .update_base
            .unwrap_or(ast.expressions[body.0 as usize].span),
        crate::name_diagnostic::annotation(data),
    ))
}
fn append_type(message: &mut Vec<Value>, spans: &Value, fallback: &str) {
    if let Some(spans) = spans.as_array() {
        for span in spans {
            let value = span[0].as_str().unwrap_or("").replace('\n', "\n    ");
            if span[1] == true {
                message.push(styled(&value, "yellow"));
            } else {
                text(message, value);
            }
        }
    } else {
        message.push(styled(&fallback.replace('\n', "\n    "), "yellow"));
    }
}
fn styled(value: &str, color: &str) -> Value {
    json!({"bold":false,"underline":false,"color":color,"string":value})
}
fn hint(message: &mut Vec<Value>, label: &str) {
    text(message, "\n\n".into());
    message.push(json!({"bold":false,"underline":true,"color":null,"string":label}));
}
pub fn report(
    source: &str,
    module: &str,
    path: &Path,
    start: (usize, usize),
    end: (usize, usize),
    data: &Value,
) -> Option<Value> {
    let name = data["name"].as_str()?;
    let actual = data["actual"].as_str()?;
    let expected = data["expected"].as_str()?;
    let category = data["category"].as_str()?;
    let branch = data["branch"]
        .as_array()
        .and_then(|b| Some((b[0].as_str()?, b[1].as_u64()? as usize)));
    let destructure = data["destructure"].as_array().and_then(|r| {
        Some(Span {
            start: r[0].as_u64()? as u32,
            end: r[1].as_u64()? as u32,
        })
    });
    let update_base = data["update_base"].as_array().is_some();
    let update_value = data["update_value"].as_array().and_then(|r| {
        Some((
            Span {
                start: r[0].as_u64()? as u32,
                end: r[1].as_u64()? as u32,
            },
            r[2].as_str()?,
        ))
    });
    let access = data["access"].as_array().and_then(|r| {
        Some((
            Span {
                start: r[0].as_u64()? as u32,
                end: r[1].as_u64()? as u32,
            },
            r[2].as_str()?,
        ))
    });
    let condition = data["condition"].as_array().and_then(|r| {
        Some(Span {
            start: r[0].as_u64()? as u32,
            end: r[1].as_u64()? as u32,
        })
    });
    let inferred = data["inferred"].as_array().and_then(|r| {
        Some(Span {
            start: r[0].as_u64()? as u32,
            end: r[1].as_u64()? as u32,
        })
    });
    let argument = data["argument"].as_array().and_then(|r| {
        Some((
            Span {
                start: r[0].as_u64()? as u32,
                end: r[1].as_u64()? as u32,
            },
            r[2].as_u64()? as usize,
        ))
    });
    let pattern = data["pattern"].as_array().and_then(|r| {
        Some((
            Span {
                start: r[0].as_u64()? as u32,
                end: r[1].as_u64()? as u32,
            },
            r[2].as_u64()? as usize,
        ))
    });
    let operator = data["operator"].as_array().and_then(|r| {
        Some((
            Span {
                start: r[0].as_u64()? as u32,
                end: r[1].as_u64()? as u32,
            },
            r[2].as_str()?,
            r[3].as_str()?,
        ))
    });
    if let Some((region, "++", _)) = operator
        && matches!(
            data["append_mode"].as_str(),
            Some("number" | "string_number" | "list_number" | "mixed")
        )
    {
        let mode = data["append_mode"].as_str()?;
        let mut message = Vec::new();
        if mode == "mixed" {
            let from = if expected == "String" {
                "String"
            } else {
                "List"
            };
            let to = if from == "String" { "List" } else { "String" };
            message = snippet_positions(
                source,
                crate::docs_diagnostic::position(source, region.start)?,
                crate::docs_diagnostic::position(source, region.end)?,
                "The (++) operator needs the same type of value on both sides:",
            )?;
            text(&mut message, "\nI see a ".into());
            message.push(styled(from, "yellow"));
            text(&mut message, " on the left and a ".into());
            message.push(styled(to, "yellow"));
            text(&mut message, " on the right. Which should it be? Does the\nstring need [] around it to become a list?".into());
        } else {
            if mode == "number" {
                text(
                    &mut message,
                    "The (++) operator can append List and String values, but not ".into(),
                );
                message.push(styled(actual, "yellow"));
                text(&mut message, " values like\nthis:".into());
            } else {
                text(&mut message, "I thought I was appending ".into());
                message.push(styled(
                    if mode == "string_number" {
                        "String"
                    } else {
                        "List"
                    },
                    "yellow",
                ));
                text(&mut message, " values here, not ".into());
                message.push(styled(actual, "yellow"));
                text(&mut message, " values like this:".into());
            }
            for part in crate::docs_diagnostic::snippet_highlight(source, region, start, end, "")? {
                if let Some(value) = part.as_str() {
                    text(&mut message, value.into());
                } else {
                    message.push(part);
                }
            }
            if mode == "list_number" {
                text(
                    &mut message,
                    "\nTry putting it in [] to make it a list?".into(),
                );
            } else {
                let function = if actual == "Float" {
                    "String.fromFloat"
                } else {
                    "String.fromInt"
                };
                text(&mut message, "\nTry using ".into());
                message.push(styled(function, "GREEN"));
                let prefix = format!("Try using {function}");
                let suffix = if mode == "number" {
                    " to turn it into a string? Or put it in [] to make it a list? Or switch to the (::) operator?"
                } else {
                    " to turn it into a string?"
                };
                text(
                    &mut message,
                    reflow(&format!("{prefix}{suffix}"))[prefix.len()..].into(),
                );
            }
        }
        return Some(
            json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{
            "title":"TYPE MISMATCH","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
        );
    }
    if let Some((region, op, "right")) = operator
        && matches!(op, "-" | "^" | "+" | "*")
        && matches!((actual, expected), ("Int", "Float") | ("Float", "Int"))
    {
        let highlight_start = crate::docs_diagnostic::position(source, region.start)?;
        let highlight_end = crate::docs_diagnostic::position(source, region.end)?;
        let heading = format!(
            "I need both sides of ({op}) to be the exact same type. Both Int or both Float."
        );
        let mut message = snippet_positions(source, highlight_start, highlight_end, &heading)?;
        text(
            &mut message,
            format!(
                "\nBut I see {} ",
                if expected == "Int" { "an" } else { "a" }
            ),
        );
        message.push(styled(expected, "yellow"));
        text(
            &mut message,
            format!(
                " on the left and {} ",
                if actual == "Int" { "an" } else { "a" }
            ),
        );
        message.push(styled(actual, "yellow"));
        text(&mut message, " on the right.\n\nUse ".into());
        message.push(styled(
            if expected == "Int" {
                "toFloat"
            } else {
                "round"
            },
            "GREEN",
        ));
        text(&mut message, " on the left (or ".into());
        message.push(styled(
            if actual == "Int" { "toFloat" } else { "round" },
            "GREEN",
        ));
        text(
            &mut message,
            " on the right) to make both sides match!".into(),
        );
        hint(&mut message, "Note");
        text(&mut message, reflow("Note: Read <https://elm-lang.org/0.19.1/implicit-casts> to learn why Elm does not implicitly convert Ints to Floats.")[4..].into());
        return Some(
            json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{
            "title":"TYPE MISMATCH","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
        );
    }
    if let Some((region, "+", _)) = operator
        && actual == "String"
    {
        let mut message = Vec::new();
        text(&mut message, "I cannot do addition with ".into());
        message.push(styled("String", "yellow"));
        text(&mut message, " values like this one:".into());
        for part in crate::docs_diagnostic::snippet_highlight(source, region, start, end, "")? {
            if let Some(value) = part.as_str() {
                text(&mut message, value.into());
            } else {
                message.push(part);
            }
        }
        text(&mut message, "\nThe (+) operator only works with ".into());
        message.push(styled("Int", "yellow"));
        text(&mut message, " and ".into());
        message.push(styled("Float", "yellow"));
        text(&mut message, " values.".into());
        hint(&mut message, "Hint");
        text(&mut message, ": Switch to the ".into());
        message.push(styled("(++)", "GREEN"));
        text(&mut message, " operator to append strings!".into());
        return Some(
            json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{
            "title":"TYPE MISMATCH","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
        );
    }
    let (heading, prefix) = if destructure.is_some() {
        (
            "This definition is causing issues:".into(),
            "You are defining".into(),
        )
    } else if let Some((_, field)) = update_value {
        (
            format!("I cannot update the `{field}` field like this:"),
            format!("You are trying to update `{field}` to be"),
        )
    } else if update_base {
        (
            "This is not a record, so it has no fields to update!".into(),
            "It is".into(),
        )
    } else if access.is_some() {
        (
            "This is not a record, so it has no fields to access!".into(),
            "It is".into(),
        )
    } else if let Some((_, op, _)) = operator.filter(|_| data["operator_comparison"] == true) {
        (
            format!("I need both sides of ({op}) to be the same type:"),
            format!("The left side of ({op}) is"),
        )
    } else if let Some((_, op, side)) = operator.filter(|(_, op, _)| matches!(*op, ">>" | "<<")) {
        (
            format!(
                "The {side} argument of ({op}) is causing problems{}",
                if side == "left" { ":" } else { "." }
            ),
            format!("The {side} argument is"),
        )
    } else if let Some((_, op, side)) = operator {
        (
            if op == "|>" && data["pipe_argument"] == true {
                "This function cannot handle the argument sent through the (|>) pipe:"
            } else if op == "|>" {
                "The right side of (|>) needs to be a function so I can pipe arguments to it!"
            } else if op == "<|" && side == "right" {
                "I cannot send this through the (<|) pipe:"
            } else if op == "<|" {
                "The left side of (<|) needs to be a function so I can pipe arguments to it!"
            } else if matches!(op, "<" | ">" | "<=" | ">=") {
                "I cannot do a comparison with this value:"
            } else if op == "++" && data["append_mode"] == "compare" {
                "The (++) operator cannot append these two values:"
            } else if op == "++" {
                "The (++) operator cannot append this type of value:"
            } else if op == "::" && data["cons_comparison"] == true {
                "I am having trouble with this (::) operator:"
            } else if op == "::" {
                "The (::) operator can only add elements onto lists."
            } else if op == "negate" {
                "I do not know how to negate this type of value:"
            } else if op == "+" && data["actual_list"] == true {
                "I cannot do addition with lists:"
            } else if op == "+" {
                "Addition does not work with this value:"
            } else if op == "*" {
                "Multiplication does not work with this value:"
            } else if op == "-" {
                "Subtraction does not work with this value:"
            } else if op == "/" {
                "The (/) operator is specifically for floating-point division:"
            } else if op == "//" {
                "The (//) operator is specifically for integer division:"
            } else if op == "^" {
                "Exponentiation does not work with this value:"
            } else {
                "I am struggling with this boolean operation:"
            }
            .into(),
            if op == "|>" {
                "But instead of a function, I am seeing".into()
            } else if matches!(op, "++" | "<|") {
                "I am seeing".into()
            } else if op == "::" {
                "The right side is".into()
            } else if matches!(op, "-" | "^" | "+" | "*" | "<" | ">" | "<=" | ">=") {
                format!("The {side} side of ({op}) is")
            } else {
                "It is".into()
            },
        )
    } else if let Some((_, index)) = pattern {
        if data["pattern_kind"] == "typed_argument" {
            (
                format!("The {} argument to `{name}` is weird.", ordinal(index)),
                "The argument is a pattern that matches".into(),
            )
        } else if data["pattern_kind"] == "constructor" {
            (
                format!("The {} argument to `{name}` is weird.", ordinal(index)),
                "It is trying to match".into(),
            )
        } else if data["pattern_kind"] == "list" {
            (
                format!(
                    "The {} pattern in this list does not match all the previous ones:",
                    ordinal(index)
                ),
                format!("The {} pattern is trying to match", ordinal(index)),
            )
        } else if data["pattern_kind"] == "tail" {
            (
                "The pattern after (::) is causing issues.".into(),
                "The pattern after (::) is trying to match".into(),
            )
        } else if index == 1 {
            (
                "The 1st pattern in this `case` causing a mismatch:".into(),
                "The first pattern is trying to match".into(),
            )
        } else {
            (
                format!(
                    "The {} pattern in this `case` does not match the previous ones.",
                    ordinal(index)
                ),
                format!("The {} pattern is trying to match", ordinal(index)),
            )
        }
    } else if let Some((_, index)) = argument {
        (
            format!(
                "The {} argument to {name} is not what I expect:",
                ordinal(index)
            ),
            "This argument is".into(),
        )
    } else if inferred.is_some() && branch.is_some_and(|(kind, _)| kind == "list") {
        let (_, index) = branch?;
        (
            format!(
                "The {} element of this list does not match all the previous elements:",
                ordinal(index)
            ),
            format!("The {} element is", ordinal(index)),
        )
    } else if inferred.is_some() {
        let (kind, index) = branch?;
        (
            format!(
                "The {} branch of this `{kind}` does not match all the previous branches:",
                ordinal(index)
            ),
            format!("The {} branch is", ordinal(index)),
        )
    } else if condition.is_some() {
        (
            "This `if` condition does not evaluate to a boolean value, True or False.".into(),
            "It is".into(),
        )
    } else {
        match branch {
            Some((kind, index)) => (
                format!(
                    "Something is off with the {} branch of this `{kind}` expression:",
                    ordinal(index)
                ),
                format!("The {} branch is", ordinal(index)),
            ),
            None => (
                format!("Something is off with the body of the `{name}` definition:"),
                "The body is".into(),
            ),
        }
    };
    let mut message = if data["cons_comparison"] == true
        || data["append_mode"] == "compare"
        || data["operator_comparison"] == true
    {
        let region = operator?.0;
        snippet_positions(
            source,
            crate::docs_diagnostic::position(source, region.start)?,
            crate::docs_diagnostic::position(source, region.end)?,
            &heading,
        )?
    } else if let Some(region) = destructure {
        snippet_positions(
            source,
            crate::docs_diagnostic::position(source, region.start)?,
            crate::docs_diagnostic::position(source, region.end)?,
            &heading,
        )?
    } else if let Some(region) = access
        .map(|(span, _)| span)
        .or(update_value.map(|(span, _)| span))
        .or(condition)
        .or(inferred)
        .or(argument.map(|(span, _)| span))
        .or(pattern.map(|(span, _)| span))
        .or(operator.map(|(span, _, _)| span))
    {
        crate::docs_diagnostic::snippet_highlight(source, region, start, end, &heading)?
    } else {
        snippet_positions(source, start, end, &heading)?
    };
    if let Some((_, op, side)) = operator
        && matches!((op, actual), ("/", "Int") | ("//", "Float"))
    {
        let float_division = op == "/";
        let required = if float_division { "Float" } else { "Int" };
        text(
            &mut message,
            format!(
                "\nThe {side} side of ({op}) must be {} ",
                if float_division { "a" } else { "an" }
            ),
        );
        message.push(styled(required, "yellow"));
        text(
            &mut message,
            format!(
                ", but I am seeing {} ",
                if float_division { "an" } else { "a" }
            ),
        );
        message.push(styled(actual, "yellow"));
        if float_division {
            text(&mut message, ". I recommend:\n\n".into());
            message.push(styled("toFloat", "GREEN"));
            text(&mut message, " for explicit conversions     ".into());
            message.push(styled("(toFloat 5 / 2) == 2.5", "BLACK"));
            text(&mut message, "\n".into());
            message.push(styled("(//)   ", "GREEN"));
            text(&mut message, " for integer division         ".into());
            message.push(styled("(5 // 2)        == 2", "BLACK"));
        } else {
            let prefix = format!("The {side} side of (//) must be an Int, but I am seeing a Float");
            let paragraph = reflow(&format!(
                "{prefix}. I recommend doing the conversion explicitly with one of these functions:"
            ));
            text(&mut message, format!("{}\n\n", &paragraph[prefix.len()..]));
            for (index, (function, example)) in [
                ("round", " 3.5     == 4"),
                ("floor", " 3.5     == 3"),
                ("ceiling", " 3.5   == 4"),
                ("truncate", " 3.5  == 3"),
            ]
            .iter()
            .enumerate()
            {
                if index != 0 {
                    text(&mut message, "\n".into());
                }
                message.push(styled(function, "GREEN"));
                text(&mut message, (*example).into());
            }
        }
        hint(&mut message, "Note");
        text(&mut message, reflow("Note: Read <https://elm-lang.org/0.19.1/implicit-casts> to learn why Elm does not implicitly convert Ints to Floats.")[4..].into());
        return Some(
            json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{
            "title":"TYPE MISMATCH","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
        );
    }
    let introduction = if data["pipe_argument"] == true {
        "The argument is:".into()
    } else if data["operator_comparison"] == true {
        format!("{prefix}:")
    } else if data["append_mode"] == "compare" {
        "I already figured out that the left side of (++) is:".into()
    } else if data["cons_comparison"] == true {
        "The left side of (::) is:".into()
    } else if category == "The body is:" {
        format!("{prefix}:")
    } else if category.ends_with(':') {
        category.to_string()
    } else {
        format!("{prefix} {category}:")
    };
    if let Some((_, op, side)) = operator.filter(|(_, op, _)| matches!(*op, "&&" | "||")) {
        text(&mut message, format!("\nBoth sides of ({op}) must be "));
        message.push(styled("Bool", "yellow"));
        text(
            &mut message,
            format!(" values, but the {side} side is:\n\n    "),
        );
    } else if let Some((_, op, side)) = operator.filter(|(_, op, _)| matches!(*op, "/" | "//")) {
        text(
            &mut message,
            format!(
                "\nThe {side} side of ({op}) must be {} ",
                if op == "/" { "a" } else { "an" }
            ),
        );
        message.push(styled(if op == "/" { "Float" } else { "Int" }, "yellow"));
        text(&mut message, ", but instead I am seeing:\n\n    ".into());
    } else {
        text(&mut message, format!("\n{}\n\n    ", reflow(&introduction)));
    }
    append_type(&mut message, &data["actual_spans"], actual);
    if update_base {
        text(&mut message, "\n\nBut I need a record!".into());
    } else if let Some((_, field)) = access {
        let paragraph = vec![
            json!("But I need a record with a "),
            styled(field, "yellow"),
            json!(" field!"),
        ];
        text(&mut message, "\n\n".into());
        for chunk in crate::docs_diagnostic::reflow_chunks(paragraph) {
            if let Some(value) = chunk.as_str() {
                text(&mut message, value.into());
            } else {
                message.push(chunk);
            }
        }
    } else if let Some((_, op, side)) = operator {
        if matches!(op, ">>" | "<<") {
            text(
                &mut message,
                format!("\n\nBut ({op}) needs the {side} argument to be:\n\n    "),
            );
            append_type(&mut message, &data["expected_spans"], expected);
            if side == "right" {
                hint(&mut message, "Hint");
                text(&mut message, reflow(&format!("Hint: With operators like ({op}) I always check the left side first. If it seems fine, I assume it is correct and check the right side. So the problem may be in how the left and right arguments interact!"))[4..].into());
            }
        }
        if data["pipe_argument"] == true {
            text(
                &mut message,
                format!("\n\nBut ({op}) is piping it to a function that expects:\n\n    "),
            );
            append_type(&mut message, &data["expected_spans"], expected);
        } else if op == "<|" && side == "left" {
            text(
                &mut message,
                "\n\nThis needs to be some kind of function though!".into(),
            );
        }
        if data["operator_comparison"] == true {
            text(&mut message, "\n\nBut the right side is:\n\n    ".into());
            append_type(&mut message, &data["expected_spans"], expected);
            if matches!(op, "<" | ">" | "<=" | ">=") {
                text(
                    &mut message,
                    format!(
                        "\n\n{}",
                        reflow(&format!(
                            "I cannot compare different types though! Which side of ({op}) is the problem?"
                        ))
                    ),
                );
            } else if actual == "Float" || expected == "Float" {
                hint(&mut message, "Note");
                text(&mut message, reflow("Note: Equality on floats is not 100% reliable due to the design of IEEE 754. I recommend a check like (abs (x - y) < 0.0001) instead.")[4..].into());
            } else {
                text(
                    &mut message,
                    "\n\nDifferent types can never be equal though! Which side is messed up?"
                        .into(),
                );
            }
        }
        if matches!(op, "<" | ">" | "<=" | ">=") && data["operator_comparison"] != true {
            text(&mut message, format!("\n\nBut ({op}) only works on "));
            for (index, name) in ["Int", "Float", "Char", "String"].iter().enumerate() {
                if index > 0 {
                    text(
                        &mut message,
                        if index == 3 { ", and " } else { ", " }.into(),
                    );
                }
                message.push(styled(name, "yellow"));
            }
            let prefix = format!("But ({op}) only works on Int, Float, Char, and String");
            let paragraph = reflow(&format!(
                "{prefix} values. It can work on lists and tuples of comparable values as well, but it is usually better to find a different path."
            ));
            text(&mut message, paragraph[prefix.len()..].into());
        }
        if op == "++" {
            if data["append_mode"] == "compare" {
                let prefix = "But this clashes with the right side, which is";
                let intro = if category == "The body is:" {
                    format!("{prefix}:")
                } else if category.ends_with(':') {
                    category.into()
                } else {
                    format!("{prefix} {category}:")
                };
                text(&mut message, format!("\n\n{}\n\n    ", reflow(&intro)));
                append_type(&mut message, &data["expected_spans"], expected);
            } else {
                text(
                    &mut message,
                    "\n\nBut the (++) operator is only for appending ".into(),
                );
                message.push(styled("List", "yellow"));
                text(&mut message, " and ".into());
                message.push(styled("String", "yellow"));
                text(
                    &mut message,
                    " values. Maybe put\nthis value in [] to make it a list?".into(),
                );
            }
        }
        if op == "::" {
            if data["cons_comparison"] == true {
                text(
                    &mut message,
                    "\n\nBut you are trying to put that into a list filled with:\n\n    ".into(),
                );
                append_type(&mut message, &data["expected_spans"], expected);
                if data["actual_list"] == true {
                    hint(&mut message, "Hint");
                    text(&mut message, reflow("Hint: Are you trying to append two lists? The (++) operator appends lists, whereas the (::) operator is only for adding ONE element to a list.")[4..].into());
                } else {
                    text(
                        &mut message,
                        "\n\nLists need ALL elements to be the same type though.".into(),
                    );
                }
            } else {
                text(&mut message, "\n\nBut (::) needs a ".into());
                message.push(styled("List", "yellow"));
                text(&mut message, " on the right.".into());
            }
        }
        if matches!(op, "negate" | "-" | "^" | "+" | "*") {
            text(
                &mut message,
                if op == "negate" {
                    "\n\nBut I only now how to negate ".into()
                } else {
                    format!("\n\nBut ({op}) only works with ")
                },
            );
            message.push(styled("Int", "yellow"));
            text(&mut message, " and ".into());
            message.push(styled("Float", "yellow"));
            text(&mut message, " values.".into());
        }
    } else if condition.is_some() {
        text(
            &mut message,
            "\n\nBut I need this `if` condition to be a ".into(),
        );
        message.push(styled("Bool", "yellow"));
        text(&mut message, " value.".into());
    } else {
        text(
            &mut message,
            format!(
                "\n\n{}\n\n    ",
                reflow(&if destructure.is_some() {
                    "But then trying to destructure it as:".into()
                } else if update_value.is_some() {
                    "But it should be:".into()
                } else if let Some((_, index)) = pattern {
                    if data["pattern_kind"] == "typed_argument" {
                        format!(
                            "But the type annotation on `{name}` says the {} argument should be:",
                            ordinal(index)
                        )
                    } else if data["pattern_kind"] == "constructor" {
                        format!("But `{name}` needs its {} argument to be:", ordinal(index))
                    } else if data["pattern_kind"] == "list" {
                        "But all the previous patterns in the list are:".into()
                    } else if data["pattern_kind"] == "tail" {
                        "But it needs to match lists like this:".into()
                    } else if index == 1 {
                        "But the expression between `case` and `of` is:".into()
                    } else {
                        "But all the previous patterns match:".into()
                    }
                } else if let Some((_, index)) = argument {
                    format!("But {name} needs the {} argument to be:", ordinal(index))
                } else if inferred.is_some() && branch.is_some_and(|(kind, _)| kind == "list") {
                    "But all the previous elements in the list are:".into()
                } else if inferred.is_some() {
                    "But all the previous branches result in:".into()
                } else {
                    format!("But the type annotation on `{name}` says it should be:")
                })
            ),
        );
        append_type(&mut message, &data["expected_spans"], expected);
    }
    if update_value.is_some() {
        hint(&mut message, "Note");
        text(&mut message, reflow("Note: The record update syntax does not allow you to change the type of fields. You can achieve that with record constructors or the record literal syntax.")[4..].into());
    }
    if inferred.is_some() {
        let (kind, _) = branch?;
        hint(&mut message, "Hint");
        if kind == "list" {
            text(&mut message, reflow("Hint: Everything in a list must be the same type of value. This way, we never run into unexpected values partway through a List.map, List.foldl, etc. Read <https://elm-lang.org/0.19.1/custom-types> to learn how to “mix” types.")[4..].into());
        } else {
            text(&mut message, reflow(&format!(
            "Hint: All branches in {} `{kind}` must produce the same type of values. This way, no matter which branch we take, the result is always a consistent shape. Read <https://elm-lang.org/0.19.1/custom-types> to learn how to “mix” types.",
            if kind == "if" { "an" } else { "a" }
        ))[4..].into());
        }
    }
    if argument.is_some_and(|(_, index)| index > 1) {
        hint(&mut message, "Hint");
        text(&mut message, reflow("Hint: I always figure out the argument types from left to right. If an argument is acceptable, I assume it is “correct” and move on. So the problem may actually be in one of the previous arguments!")[4..].into());
    }
    if let Some((_, op, _)) = operator
        && data["actual_list"] == true
        && matches!(op, "+" | "*")
    {
        hint(&mut message, "Hint");
        if op == "+" {
            text(&mut message, ": Switch to the ".into());
            message.push(styled("(++)", "GREEN"));
            text(&mut message, " operator to append lists!".into());
        } else {
            text(&mut message, ": Maybe you want ".into());
            message.push(styled("List.repeat", "GREEN"));
            text(&mut message, " to build a list of repeated values?".into());
        }
    }
    let actual = data["actual_problem"].as_str().unwrap_or(actual);
    let expected = data["expected_problem"].as_str().unwrap_or(expected);
    match (actual, expected) {
        _ if data["record_hint"].is_object() => {
            let extra = data["record_hint"]["extra"].as_array()?;
            let missing = data["record_hint"]["missing"].as_array()?;
            if let Some(typo) = extra.first().and_then(Value::as_str) {
                let mut candidates: Vec<_> = data["record_hint"]["possibilities"]
                    .as_array()?
                    .iter()
                    .filter_map(Value::as_str)
                    .collect();
                candidates.sort_by_key(|name| crate::name_diagnostic::distance(typo, name));
                if let Some(nearest) = candidates.first() {
                    hint(&mut message, "Hint");
                    let paragraph_start = message.len() - 1;
                    text(
                        &mut message,
                        ": Seems like a record field typo. Maybe ".into(),
                    );
                    message.push(styled(typo, "yellow"));
                    text(&mut message, " should be ".into());
                    message.push(styled(nearest, "GREEN"));
                    text(&mut message, "?".into());
                    let paragraph = message.split_off(paragraph_start);
                    message.extend(crate::docs_diagnostic::reflow_chunks(paragraph));
                    hint(&mut message, "Hint");
                    text(&mut message, reflow("Hint: Can more type annotations be added? Type annotations always help me give more specific messages, and I think they could help a lot in this case!")[4..].into());
                }
            } else if !missing.is_empty() {
                hint(&mut message, "Hint");
                let paragraph_start = message.len() - 1;
                text(
                    &mut message,
                    if missing.len() == 1 {
                        ": Looks like the "
                    } else {
                        ": Looks like fields "
                    }
                    .into(),
                );
                for (index, name) in missing.iter().enumerate() {
                    if index > 0 {
                        text(
                            &mut message,
                            if index + 1 == missing.len() {
                                if missing.len() > 2 { ", and " } else { " and " }
                            } else {
                                ", "
                            }
                            .into(),
                        );
                    }
                    message.push(styled(name.as_str()?, "GREEN"));
                }
                text(
                    &mut message,
                    if missing.len() == 1 {
                        " field is missing."
                    } else {
                        " are missing."
                    }
                    .into(),
                );
                let paragraph = message.split_off(paragraph_start);
                message.extend(crate::docs_diagnostic::reflow_chunks(paragraph));
            }
        }
        _ if data["to_list"] == true => {}
        _ if data["from_maybe"] == true => {
            hint(&mut message, "Hint");
            text(&mut message, ": Use ".into());
            message.push(styled("Maybe.withDefault", "GREEN"));
            text(&mut message, " to handle possible errors. Longer term, it is\nusually better to write out the full `case` though!".into());
        }
        ("String", "Int" | "Float") | ("Int" | "Float", "String") => {
            hint(&mut message, "Hint");
            let article = if expected == "Int" { "an" } else { "a" };
            text(
                &mut message,
                format!(
                    ": Want to convert {} {actual} into {article} {expected}? Use the ",
                    if actual == "Int" { "an" } else { "a" }
                ),
            );
            let function = match (actual, expected) {
                ("String", "Int") => "String.toInt",
                ("String", "Float") => "String.toFloat",
                ("Int", _) => "String.fromInt",
                _ => "String.fromFloat",
            };
            message.push(styled(function, "GREEN"));
            text(&mut message, " function!".into());
        }
        ("Int", "Float") | ("Float", "Int") => {
            hint(&mut message, "Note");
            text(&mut message, ": Read <https://elm-lang.org/0.19.1/implicit-casts> to learn why Elm does\nnot implicitly convert Ints to Floats. Use ".into());
            message.push(styled("toFloat", "GREEN"));
            text(&mut message, " and ".into());
            message.push(styled("round", "GREEN"));
            text(&mut message, " to do explicit\nconversions.".into());
        }
        ("number", "String") | ("String", "number") => {
            hint(&mut message, "Hint");
            text(&mut message, ": Try using ".into());
            message.push(styled(
                if actual == "number" {
                    "String.fromInt"
                } else {
                    "String.toInt"
                },
                "GREEN",
            ));
            text(
                &mut message,
                if actual == "number" {
                    " to convert it to a string?"
                } else {
                    " to convert it to an integer?"
                }
                .into(),
            );
        }
        ("comparable", _) | (_, "comparable")
            if data["rigid"] != true
                && data["actual_rigid"] != true
                && data["actual_alias"] != true =>
        {
            let side = if actual == "comparable" {
                "expected"
            } else {
                "actual"
            };
            let shape = data[format!("{side}_problem_shape")].as_str().unwrap_or("");
            hint(&mut message, "Hint");
            if shape == "record" {
                text(&mut message, reflow("Hint: I do not know how to compare records. I can only compare ints, floats, chars, strings, lists of comparable values, and tuples of comparable values. Check out <https://elm-lang.org/0.19.1/comparing-records> for ideas on how to proceed.")[4..].into());
            } else if let Some(name) = data[format!("{side}_problem_name")].as_str() {
                text(&mut message, reflow(&format!("Hint: I do not know how to compare `{name}` values. I can only compare ints, floats, chars, strings, lists of comparable values, and tuples of comparable values."))[4..].into());
                text(
                    &mut message,
                    format!(
                        "\n\n{}",
                        reflow(
                            "Check out <https://elm-lang.org/0.19.1/comparing-custom-types> for ideas on how to proceed."
                        )
                    ),
                );
            } else {
                text(&mut message, reflow("Hint: I only know how to compare ints, floats, chars, strings, lists of comparable values, and tuples of comparable values.")[4..].into());
            }
        }
        ("appendable", _) | (_, "appendable")
            if data["rigid"] != true
                && data["actual_rigid"] != true
                && data["actual_alias"] != true =>
        {
            hint(&mut message, "Hint");
            text(
                &mut message,
                ": I only know how to append strings and lists.".into(),
            );
        }
        ("number", _) | (_, "number")
            if data["rigid"] != true
                && data["actual_rigid"] != true
                && data["actual_alias"] != true =>
        {
            hint(&mut message, "Hint");
            text(&mut message, ": Only ".into());
            message.push(styled("Int", "GREEN"));
            text(&mut message, " and ".into());
            message.push(styled("Float", "GREEN"));
            text(&mut message, " values work as numbers.".into());
        }
        (_, "Bool") if data["actual_named_atom"] == true => {
            hint(&mut message, "Hint");
            text(&mut message,reflow("Hint: Elm does not have “truthiness” such that ints and strings and lists are automatically converted to booleans. Do that conversion explicitly!")[4..].into());
        }
        _ if data["rigid"] == true && data["actual_rigid"] == true => {
            hint(&mut message, "Hint");
            text(&mut message, reflow(&format!("Hint: Your type annotation uses `{actual}` and `{expected}` as separate type variables. Your code seems to be saying they are the same though. Maybe they should be the same in your type annotation? Maybe your code uses them in a weird way?"))[4..].into());
            text(
                &mut message,
                "\n\nRead <https://elm-lang.org/0.19.1/type-annotations> for more advice!".into(),
            );
        }
        _ if data["rigid"] == true || data["actual_rigid"] == true => {
            let (actual, expected) = if data["actual_rigid"] == true {
                (expected, actual)
            } else {
                (actual, expected)
            };
            hint(&mut message, "Hint");
            let actual_shape = if data["actual_rigid"] == true {
                &data["expected_problem_shape"]
            } else {
                &data["actual_problem_shape"]
            };
            let thing = if actual_shape == "record" {
                "a record".to_string()
            } else if actual == "()" {
                "a unit value".to_string()
            } else {
                format!(
                    "{} `{actual}` value",
                    if actual == "appendable" { "an" } else { "a" }
                )
            };
            let constraint = if data["actual_rigid"] == true {
                &data["actual_constraint"]
            } else {
                &data["expected_constraint"]
            };
            let many = match constraint.as_str() {
                Some("number") => Some(("number", "ints AND floats")),
                Some("comparable") => Some((
                    "comparable",
                    "ints, floats, chars, strings, lists, and tuples",
                )),
                Some("appendable") => Some(("appendable", "strings AND lists")),
                Some("compappend") => Some(("compappend", "strings AND lists")),
                _ => None,
            };
            let intro = if let Some((constraint, many)) = many {
                format!(
                    "The `{constraint}` in your type annotation is saying that {many} can flow through"
                )
            } else {
                format!(
                    "Your type annotation uses type variable `{expected}` which means ANY type of value can flow through"
                )
            };
            text(&mut message, reflow(&format!("Hint: {intro}, but your code is saying it specifically wants {thing}. Maybe change your type annotation to be more specific? Maybe change the code to be more general?"))[4..].into());
            text(
                &mut message,
                "\n\nRead <https://elm-lang.org/0.19.1/type-annotations> for more advice!".into(),
            );
        }
        _ => text(&mut message, "".into()),
    }
    if let Some((_, index)) = pattern.filter(|_| data["pattern_kind"] == "case") {
        if index == 1 {
            text(
                &mut message,
                "\n\nThese can never match! Is the pattern the problem? Or is it the expression?"
                    .into(),
            );
        } else {
            hint(&mut message, "Note");
            text(&mut message, reflow("Note: A `case` expression can only handle one type of value, so you may want to use <https://elm-lang.org/0.19.1/custom-types> to handle “mixing” types.")[4..].into());
        }
    }
    if pattern.is_some() && data["pattern_kind"] == "list" {
        hint(&mut message, "Hint");
        text(&mut message, reflow("Hint: Everything in a list must be the same type of value. This way, we never run into unexpected values partway through a List.map, List.foldl, etc. Read <https://elm-lang.org/0.19.1/custom-types> to learn how to “mix” types.")[4..].into());
    }
    Some(
        json!({"type":"compile-errors","errors":[{"path":path,"name":module,"problems":[{
        "title":"TYPE MISMATCH","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}]}]}),
    )
}

pub(crate) fn combine(ast: &Syntax<'_>, errors: &[String]) -> String {
    let mut problems = Vec::new();
    for error in errors.iter().rev() {
        if let Some(report) = crate::docs_diagnostic::report_encoded(error) {
            if let Some(items) = report["errors"][0]["problems"].as_array() {
                problems.extend(items.iter().cloned());
                continue;
            }
            return error.clone();
        }
        let mut parts = error.splitn(5, ':');
        let positions: Option<Vec<usize>> = (0..4).map(|_| parts.next()?.parse().ok()).collect();
        let Some(positions) = positions else {
            return error.clone();
        };
        let Some(report) = crate::name_diagnostic::report(
            ast.source,
            &ast.header.name,
            Path::new(""),
            (positions[0], positions[1]),
            (positions[2], positions[3]),
            parts.next().unwrap_or("").trim_start(),
        ) else {
            return error.clone();
        };
        problems.extend(
            report["errors"][0]["problems"]
                .as_array()
                .unwrap()
                .iter()
                .cloned(),
        );
    }
    crate::docs_diagnostic::encode(&json!({"type":"compile-errors","errors":[{
        "path":"","name":ast.header.name,"problems":problems
    }]}))
}
