//! Linear-time operator precedence resolution. Scope construction supplies this
//! table from the module's imported/local infix declarations; no global list of
//! assumed Elm operators is baked into the compiler.
use crate::ast::{Declaration, Expr, ExprId, Node, Span, Syntax};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Associativity {
    Left,
    Right,
    Non,
}
#[derive(Debug, Clone, Copy)]
pub struct Fixity {
    pub precedence: u8,
    pub associativity: Associativity,
}
pub type Table = BTreeMap<String, Fixity>;
pub fn declared(ast: &Syntax<'_>) -> Result<Table, String> {
    let mut table = Table::new();
    for d in &ast.declarations {
        if let Declaration::Infix {
            associativity,
            precedence,
            operator,
            ..
        } = d
        {
            let associativity = match *associativity {
                "left" => Associativity::Left,
                "right" => Associativity::Right,
                "non" => Associativity::Non,
                _ => return Err("invalid associativity".into()),
            };
            if table
                .insert(
                    operator.to_string(),
                    Fixity {
                        precedence: *precedence,
                        associativity,
                    },
                )
                .is_some()
            {
                return Err(format!("duplicate infix declaration {operator}"));
            }
        }
    }
    Ok(table)
}
fn reduce_before(previous: Fixity, next: Fixity) -> Result<bool, String> {
    if previous.precedence != next.precedence {
        return Ok(previous.precedence > next.precedence);
    }
    match (previous.associativity, next.associativity) {
        (Associativity::Left, Associativity::Left) => Ok(true),
        (Associativity::Right, Associativity::Right) => Ok(false),
        _ => Err(
            "operators of equal precedence have incompatible associativity; add parentheses".into(),
        ),
    }
}
fn unknown(ast: &Syntax<'_>, op: &str, table: &Table) -> String {
    crate::source_error::locate_slice(
        ast.source,
        op,
        crate::name_diagnostic::unknown_operator(op, table.keys().cloned().collect()),
    )
}
pub fn resolve<'s>(ast: &mut Syntax<'s>, table: &Table) -> Result<(), String> {
    for node in &ast.expressions {
        if let Expr::Operator(op) = node.kind
            && !table.contains_key(op)
        {
            return Err(crate::source_error::locate(
                ast.source,
                node.span,
                crate::name_diagnostic::unknown_operator(op, table.keys().cloned().collect()),
            ));
        }
    }
    let original_len = ast.expressions.len();
    for index in 0..original_len {
        let (first, rest) = match &ast.expressions[index].kind {
            Expr::Binops(first, rest) => (*first, rest.clone()),
            _ => continue,
        };
        let base = ast.expressions.len();
        let mut pending: Vec<Node<Expr<'s>>> = Vec::with_capacity(rest.len());
        let mut values = vec![first];
        let mut ops = Vec::<(&'s str, Fixity)>::new();
        let reduce = |values: &mut Vec<ExprId>,
                      ops: &mut Vec<(&'s str, Fixity)>,
                      pending: &mut Vec<Node<Expr<'s>>>| {
            // The reduction stack maintains one more operand than operators.
            let (operator, _) = ops.pop().unwrap();
            let right = values.pop().unwrap();
            let left = values.pop().unwrap();
            let span_of = |id: ExprId| {
                if (id.0 as usize) < base {
                    ast.expressions[id.0 as usize].span
                } else {
                    pending[id.0 as usize - base].span
                }
            };
            let span = Span {
                start: span_of(left).start,
                end: span_of(right).end,
            };
            let id = ExprId((base + pending.len()) as u32);
            pending.push(Node {
                span,
                kind: Expr::Binary(operator, left, right),
            });
            values.push(id);
        };
        for (op, right) in rest {
            let fix = *table.get(op).ok_or_else(|| unknown(ast, op, table))?;
            while let Some((previous_op, previous)) = ops.last() {
                if !reduce_before(*previous, fix).map_err(|reason| {
                    crate::source_error::locate(
                        ast.source,
                        ast.expressions[index].span,
                        crate::name_diagnostic::associativity(previous_op, op, reason),
                    )
                })? {
                    break;
                }
                reduce(&mut values, &mut ops, &mut pending);
            }
            ops.push((op, fix));
            values.push(right);
        }
        while !ops.is_empty() {
            reduce(&mut values, &mut ops, &mut pending);
        }
        if let Some(mut root) = pending.pop() {
            // The parsed chain includes delimiters around its final operand.
            // Reducing it must not shorten the diagnostic region.
            root.span = ast.expressions[index].span;
            ast.expressions[index] = root;
            ast.expressions.extend(pending);
        }
    }
    Ok(())
}
