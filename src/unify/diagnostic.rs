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

impl Engine {
    /// During diagnostic replay, keep composite types distinct until every
    /// child constraint succeeds. Linking their roots before an inner failure
    /// otherwise erases the actual type (e.g. List String becomes List Int).
    pub(crate) fn unify_annotation_diagnostic(
        &mut self,
        left: Ty,
        right: Ty,
        symbols: &Symbols,
    ) -> Result<(), String> {
        if self.display_names.is_none() {
            return self.unify_diagnostic(left, right, symbols);
        }
        enum Work {
            Unify(Ty, Ty, Option<usize>),
            RecordFields(Vec<(Ty, Ty)>, usize),
        }
        let mut pending = vec![Work::Unify(left, right, None)];
        let mut failures = 0usize;
        let mut links = Vec::new();
        let mut seen = BTreeSet::new();
        let mut error = None;
        while let Some(work) = pending.pop() {
            let (left, right, guard) = match work {
                Work::Unify(left, right, guard) => (left, right, guard),
                Work::RecordFields(fields, checkpoint) => {
                    if checkpoint == failures {
                        pending.extend(
                            fields
                                .into_iter()
                                .rev()
                                .map(|(a, b)| Work::Unify(a, b, None)),
                        );
                    }
                    continue;
                }
            };
            if guard.is_some_and(|checkpoint| checkpoint != failures) {
                continue;
            }
            let a = self.find(left);
            let b = self.find(right);
            if a == b || !seen.insert((a, b)) {
                continue;
            }
            let composite = matches!(
                self.nodes[a.0 as usize].descriptor,
                Descriptor::Structure(_)
            ) && matches!(
                self.nodes[b.0 as usize].descriptor,
                Descriptor::Structure(_)
            );
            let function = matches!(
                (&self.nodes[a.0 as usize].descriptor, &self.nodes[b.0 as usize].descriptor),
                (Descriptor::Structure(left), Descriptor::Structure(right))
                    if matches!((&**left, &**right), (Term::Function(..), Term::Function(..)))
            );
            let record_fields = match (
                self.nodes[a.0 as usize].descriptor.clone(),
                self.nodes[b.0 as usize].descriptor.clone(),
            ) {
                (Descriptor::Structure(left), Descriptor::Structure(right)) => {
                    match (&*left, &*right) {
                        (
                            Term::Record {
                                fields: fa,
                                extension: ea,
                            },
                            Term::Record {
                                fields: fb,
                                extension: eb,
                            },
                        ) => {
                            let left = self.gather_fields(fa, *ea)?;
                            let right = self.gather_fields(fb, *eb)?;
                            Some(
                                left.fields
                                    .keys()
                                    .filter(|name| right.fields.contains_key(*name))
                                    .count(),
                            )
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            let previous = self.nodes[b.0 as usize].parent;
            let mut children = Vec::new();
            if let Err(reason) = self.unify_pair(a, b, &mut children) {
                failures += 1;
                children.clear();
                error.get_or_insert(reason);
            }
            if function && children.len() == 2 {
                // Elm checks the argument first and checks the result only
                // when the complete argument constraint succeeds.
                let (result_left, result_right) = children.pop().unwrap();
                let (arg_left, arg_right) = children.pop().unwrap();
                pending.push(Work::Unify(result_left, result_right, Some(failures)));
                pending.push(Work::Unify(arg_left, arg_right, None));
            } else if let Some(shared) = record_fields {
                // Elm validates the row extensions before any shared fields.
                // A failed extension must not specialize those field types.
                let tail = children.split_off(shared.min(children.len()));
                pending.push(Work::RecordFields(children, failures));
                pending.extend(
                    tail.into_iter()
                        .rev()
                        .map(|(a, b)| Work::Unify(a, b, Some(failures))),
                );
            } else {
                pending.extend(children.into_iter().map(|(a, b)| Work::Unify(a, b, None)));
            }
            if composite && self.nodes[b.0 as usize].parent != previous {
                links.push((b, self.nodes[b.0 as usize].parent));
                self.nodes[b.0 as usize].parent = previous;
            }
        }
        if let Some(error) = error {
            return Err(error);
        }
        for (child, parent) in links {
            let child = self.find(child);
            let parent = self.find(parent);
            if child != parent {
                self.nodes[child.0 as usize].parent = parent;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{names::SymbolId, unify::Builtins};
    #[test]
    fn constructor_diagnostic_recovery_does_not_poison_the_template_or_other_uses() {
        let mut engine = Engine::new(Builtins {
            int: SymbolId(0),
            float: SymbolId(1),
            string: SymbolId(2),
            char: SymbolId(3),
            list: SymbolId(4),
        });
        let int = engine.term(Term::Named(SymbolId(0), vec![]));
        let result = engine.term(Term::Unit);
        let root = engine.term(Term::Function(int, result));
        let scheme = super::super::Scheme {
            root,
            quantified: BTreeSet::new(),
        };
        let first = engine.instantiate_diagnostic_constructor(&scheme, 1);
        let second = engine.instantiate_diagnostic_constructor(&scheme, 1);
        let Term::Function(first_arg, _) = &*engine.structure(first).unwrap() else {
            panic!()
        };
        let first_arg = *first_arg;
        let Term::Function(second_arg, _) = &*engine.structure(second).unwrap() else {
            panic!()
        };
        let second_arg = *second_arg;
        engine.mark_diagnostic_error(first_arg);
        let string = engine.term(Term::Named(SymbolId(2), vec![]));
        assert!(engine.unify(second_arg, string).is_err());
        assert!(engine.unify(int, string).is_err());
        assert!(engine.unify(first_arg, string).is_ok());
    }
    #[test]
    fn reported_inferred_error_propagates_without_hiding_independent_errors() {
        let mut engine = Engine::new(Builtins {
            int: SymbolId(0),
            float: SymbolId(1),
            string: SymbolId(2),
            char: SymbolId(3),
            list: SymbolId(4),
        });
        engine.track_display_names();
        let number = engine.variable(1, Constraint::Number);
        engine.mark_diagnostic_error(number);
        let string = engine.term(Term::Named(SymbolId(2), vec![]));
        engine.unify(number, string).unwrap();
        let unit = engine.term(Term::Unit);
        engine.unify(string, unit).unwrap();
        let independent_number = engine.variable(1, Constraint::Number);
        let independent_string = engine.term(Term::Named(SymbolId(2), vec![]));
        assert!(
            engine
                .unify(independent_number, independent_string)
                .is_err()
        );
        let mut roots = [number, string, unit, independent_number, independent_string];
        engine.compact(&mut roots);
        engine.unify(roots[0], roots[2]).unwrap();
        assert!(engine.unify(roots[3], roots[4]).is_err());
    }
    #[test]
    fn annotation_failure_keeps_both_structures_and_solves_compatible_fields() {
        let mut engine = Engine::new(Builtins {
            int: SymbolId(0),
            float: SymbolId(1),
            string: SymbolId(2),
            char: SymbolId(3),
            list: SymbolId(4),
        });
        engine.track_display_names();
        let int = engine.term(Term::Named(SymbolId(0), vec![]));
        let string = engine.term(Term::Named(SymbolId(2), vec![]));
        let number = engine.variable(1, Constraint::Number);
        let actual = engine.term(Term::Tuple(vec![number, string]));
        let expected = engine.term(Term::Tuple(vec![int, int]));
        assert!(
            engine
                .unify_annotation_diagnostic(expected, actual, &Symbols::default())
                .is_err()
        );
        assert_eq!(engine.find(number), engine.find(int));
        assert_ne!(engine.find(actual), engine.find(expected));
        let Term::Tuple(items) = &*engine.structure(actual).unwrap() else {
            panic!("tuple lost");
        };
        assert_eq!(items[1], string);
        let valid = engine.term(Term::Tuple(vec![number, int]));
        engine
            .unify_annotation_diagnostic(expected, valid, &Symbols::default())
            .unwrap();
        assert_eq!(engine.find(valid), engine.find(expected));
    }
    #[test]
    fn function_argument_failure_does_not_constrain_the_result() {
        let mut engine = Engine::new(Builtins {
            int: SymbolId(0),
            float: SymbolId(1),
            string: SymbolId(2),
            char: SymbolId(3),
            list: SymbolId(4),
        });
        engine.track_display_names();
        let int = engine.term(Term::Named(SymbolId(0), vec![]));
        let string = engine.term(Term::Named(SymbolId(2), vec![]));
        let result = engine.variable(1, Constraint::Any);
        let expected = engine.term(Term::Function(int, result));
        let actual = engine.term(Term::Function(string, int));
        assert!(
            engine
                .unify_annotation_diagnostic(expected, actual, &Symbols::default())
                .is_err()
        );
        assert!(engine.structure(result).is_none());
        assert_ne!(engine.find(actual), engine.find(expected));
        let valid = engine.term(Term::Function(int, string));
        engine
            .unify_annotation_diagnostic(expected, valid, &Symbols::default())
            .unwrap();
        assert_eq!(engine.find(result), engine.find(string));
    }
    #[test]
    fn missing_record_fields_do_not_constrain_common_fields() {
        let mut engine = Engine::new(Builtins {
            int: SymbolId(0),
            float: SymbolId(1),
            string: SymbolId(2),
            char: SymbolId(3),
            list: SymbolId(4),
        });
        engine.track_display_names();
        let int = engine.term(Term::Named(SymbolId(0), vec![]));
        let number = engine.variable(1, Constraint::Number);
        let actual = engine.term(Term::Record {
            fields: [("a".into(), number)].into(),
            extension: None,
        });
        let expected = engine.term(Term::Record {
            fields: [("a".into(), int), ("b".into(), int)].into(),
            extension: None,
        });
        assert!(
            engine
                .unify_annotation_diagnostic(expected, actual, &Symbols::default())
                .is_err()
        );
        assert!(engine.structure(number).is_none());
        assert_ne!(engine.find(actual), engine.find(expected));
    }
    #[test]
    fn rigid_record_extension_failure_preserves_common_fields() {
        let mut engine = Engine::new(Builtins {
            int: SymbolId(0),
            float: SymbolId(1),
            string: SymbolId(2),
            char: SymbolId(3),
            list: SymbolId(4),
        });
        engine.track_display_names();
        let int = engine.term(Term::Named(SymbolId(0), vec![]));
        let number = engine.variable(1, Constraint::Number);
        let row = engine.rigid(1, Constraint::Any);
        let actual = engine.term(Term::Record {
            fields: [("x".into(), number)].into(),
            extension: None,
        });
        let expected = engine.term(Term::Record {
            fields: [("x".into(), int)].into(),
            extension: Some(row),
        });
        assert!(
            engine
                .unify_annotation_diagnostic(expected, actual, &Symbols::default())
                .is_err()
        );
        assert!(engine.structure(number).is_none());
        assert!(engine.structure(row).is_none());
    }
    #[test]
    fn rejected_infinite_type_is_retained_without_creating_a_cycle() {
        let mut engine = Engine::new(Builtins {
            int: SymbolId(0),
            float: SymbolId(1),
            string: SymbolId(2),
            char: SymbolId(3),
            list: SymbolId(4),
        });
        engine.track_display_names();
        let argument = engine.variable(1, Constraint::Any);
        let result = engine.variable(1, Constraint::Any);
        let function = engine.term(Term::Function(argument, result));
        assert!(engine.unify(argument, function).is_err());
        assert_eq!(engine.take_infinite_type(), Some((argument, function)));
        assert_eq!(engine.take_infinite_type(), None);
        assert!(engine.structure(argument).is_none());
        assert_eq!(engine.generalize(function, 0).quantified.len(), 2);
    }
}
