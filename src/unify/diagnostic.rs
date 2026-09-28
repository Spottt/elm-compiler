//! Render types only after failure. Bounds keep recursive or very large inferred
//! types from overflowing the stack or flooding a terminal with model records.
use super::{Constraint, Descriptor, Engine, Term, Ty};
use crate::names::Symbols;
use std::collections::{BTreeMap, BTreeSet};

struct Renderer<'a> {
    engine: &'a mut Engine,
    symbols: &'a Symbols,
    variables: BTreeMap<Ty, String>,
    active: BTreeSet<Ty>,
    remaining: usize,
}
impl Renderer<'_> {
    fn render(&mut self, ty: Ty, depth: usize) -> String {
        if depth >= 24 || self.remaining == 0 {
            return "…".into();
        }
        self.remaining -= 1;
        let root = self.engine.find(ty);
        if !self.active.insert(root) {
            return "<recursive>".into();
        }
        let result = match self.engine.nodes[root.0 as usize].descriptor.clone() {
            Descriptor::Variable { constraint, .. } | Descriptor::Rigid { constraint, .. } => {
                let index = self.variables.len();
                self.variables
                    .entry(root)
                    .or_insert_with(|| {
                        let prefix = match constraint {
                            Constraint::Any => "a",
                            Constraint::Number => "number",
                            Constraint::Comparable => "comparable",
                            Constraint::Appendable => "appendable",
                            Constraint::CompAppend => "compappend",
                        };
                        format!("{prefix}{index}")
                    })
                    .clone()
            }
            Descriptor::Structure(term) => match &*term {
                Term::Alias(_, _, real) => self.render(*real, depth + 1),
                Term::Unit => "()".into(),
                Term::Named(symbol, args) => {
                    let symbol = self.symbols.get(*symbol);
                    let mut text = format!("{}.{}", symbol.module, symbol.name);
                    for arg in args.iter().take(12) {
                        text.push_str(&format!(" ({})", self.render(*arg, depth + 1)));
                    }
                    if args.len() > 12 {
                        text.push_str(" …");
                    }
                    text
                }
                Term::Function(arg, result) => format!(
                    "({} -> {})",
                    self.render(*arg, depth + 1),
                    self.render(*result, depth + 1)
                ),
                Term::Tuple(items) => {
                    let mut parts: Vec<_> = items
                        .iter()
                        .take(12)
                        .map(|t| self.render(*t, depth + 1))
                        .collect();
                    if items.len() > 12 {
                        parts.push("…".into());
                    }
                    format!("({})", parts.join(", "))
                }
                Term::Record { fields, extension } => {
                    let mut parts: Vec<_> = fields
                        .iter()
                        .take(12)
                        .map(|(name, ty)| format!("{name} : {}", self.render(*ty, depth + 1)))
                        .collect();
                    if fields.len() > 12 {
                        parts.push("…".into());
                    }
                    let prefix = extension
                        .map(|ty| format!("{} | ", self.render(ty, depth + 1)))
                        .unwrap_or_default();
                    format!("{{ {prefix}{} }}", parts.join(", "))
                }
            },
        };
        self.active.remove(&root);
        result
    }
}
impl Engine {
    pub(crate) fn unify_diagnostic(
        &mut self,
        left: Ty,
        right: Ty,
        symbols: &Symbols,
    ) -> Result<(), String> {
        self.unify_detail(left, right)
            .map_err(|(reason, left, right)| {
                let mut renderer = Renderer {
                    engine: self,
                    symbols,
                    variables: BTreeMap::new(),
                    active: BTreeSet::new(),
                    remaining: 160,
                };
                let left = renderer.render(left, 0);
                renderer.remaining = 160;
                let right = renderer.render(right, 0);
                format!("{reason}\nCannot unify these types:\n    {left}\nwith:\n    {right}")
            })
    }
}
