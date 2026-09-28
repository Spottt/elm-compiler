//! Pattern tests and bindings over the runtime layout selected by the linker.
use crate::{
    ast::{Pattern, PatternId, Syntax},
    kernel::Mode,
    literal,
};
pub trait Names {
    fn switch_case(
        &mut self,
        _pattern: PatternId,
        _value: &str,
    ) -> Result<Option<(String, String)>, String> {
        Ok(None)
    }
    fn local(&mut self, pattern: PatternId, name: &str) -> Result<String, String>;
    fn field(&mut self, name: &str) -> Result<String, String>;
    /// Return the tag predicate and extraction expressions for constructor args.
    /// This handles booleans, ordinary unions and optimized unboxed constructors.
    fn constructor(
        &mut self,
        pattern: PatternId,
        value: &str,
    ) -> Result<(String, Vec<String>), String>;
}
pub struct Plan {
    pub switch_case: Option<(String, String)>,
    pub condition: String,
    pub tests: Vec<String>,
    pub bindings: Vec<(String, String)>,
}
impl Plan {
    pub fn declarations(&self) -> String {
        self.bindings
            .iter()
            .map(|(name, value)| format!("var {name}={value};"))
            .collect()
    }
}
pub fn plan(
    ast: &Syntax<'_>,
    root: PatternId,
    value: &str,
    mode: Mode,
    names: &mut impl Names,
) -> Result<Plan, String> {
    let switch_case = if mode == Mode::Production {
        match &ast.patterns[root.0 as usize].kind {
            Pattern::Literal(kind, raw) => Some((value.into(), literal::emit(*kind, raw, mode)?)),
            Pattern::Constructor(_, args) if args.is_empty() => names.switch_case(root, value)?,
            _ => None,
        }
    } else {
        None
    };
    let mut pending = vec![(root, value.to_string())];
    let mut conditions = Vec::new();
    let mut bindings = Vec::new();
    let cons = if !matches!(mode, Mode::Production) {
        "'::'"
    } else {
        "1"
    };
    let nil = if !matches!(mode, Mode::Production) {
        "'[]'"
    } else {
        "0"
    };
    while let Some((id, value)) = pending.pop() {
        match &ast.patterns[id.0 as usize].kind {
            Pattern::Wildcard | Pattern::Unit => {}
            Pattern::Var(name) => bindings.push((names.local(id, name)?, value)),
            Pattern::Alias(child, name) => {
                bindings.push((names.local(id, name)?, value.clone()));
                pending.push((*child, value));
            }
            Pattern::Literal(kind, raw) => {
                // Char values are boxed in development; comparison uses payloads.
                let lhs = if *kind == crate::lexer::Kind::Char {
                    format!("({value}).valueOf()")
                } else {
                    value
                };
                conditions.push(format!(
                    "({lhs}==={})",
                    literal::emit(*kind, raw, Mode::Production)?
                ));
            }
            Pattern::Tuple(items) => {
                for (index, child) in items.iter().enumerate().rev() {
                    pending.push((*child, format!("{value}.{}", (b'a' + index as u8) as char)));
                }
            }
            Pattern::Record(fields) => {
                for name in fields {
                    let key = serde_json::to_string(&names.field(name)?).unwrap();
                    bindings.push((names.local(id, name)?, format!("{value}[{key}]")));
                }
            }
            Pattern::Constructor(_, items) => {
                let (condition, fields) = names.constructor(id, &value)?;
                if fields.len() != items.len() {
                    return Err("constructor layout arity mismatch".into());
                }
                conditions.push(condition);
                pending.extend(items.iter().copied().zip(fields).rev());
            }
            Pattern::Cons(head, tail) => {
                conditions.push(format!("({value}.$==={cons})"));
                pending.push((*tail, format!("{value}.b")));
                pending.push((*head, format!("{value}.a")));
            }
            Pattern::List(items) => {
                let mut path = value;
                let mut children = Vec::new();
                for child in items {
                    conditions.push(format!("({path}.$==={cons})"));
                    children.push((*child, format!("{path}.a")));
                    path.push_str(".b");
                }
                conditions.push(format!("({path}.$==={nil})"));
                pending.extend(children.into_iter().rev());
            }
        }
    }
    Ok(Plan {
        switch_case,
        condition: if conditions.is_empty() {
            "true".into()
        } else {
            conditions.join("&&")
        },
        tests: conditions,
        bindings,
    })
}

/// Use only proven, disjoint scalar alternatives and an exhaustive default.
pub fn switch_labels(plans: &[Plan]) -> Option<(String, Vec<String>)> {
    if plans.len() < 3 || plans.last()?.condition != "true" {
        return None;
    }
    let mut labels = Vec::new();
    let mut subject = None;
    for plan in &plans[..plans.len() - 1] {
        let (key, label) = plan.switch_case.as_ref()?;
        if subject.as_ref().is_some_and(|s| s != key) || labels.contains(label) {
            return None;
        }
        subject = Some(key.clone());
        labels.push(label.clone());
    }
    Some((subject?, labels))
}

/// Linearized decision tree. Share only contiguous prefixes, preserving source
/// branch priority and the order of tag tests before accessing nested fields.
pub enum DecisionStep {
    Open(String),
    Body(usize),
    Close,
}
pub fn decision_steps(plans: &[Plan]) -> Vec<DecisionStep> {
    enum Work {
        Range(usize, usize, usize),
        Close,
    }
    let mut work = vec![Work::Range(0, plans.len(), 0)];
    let mut out = Vec::new();
    while let Some(item) = work.pop() {
        let Work::Range(start, end, depth) = item else {
            out.push(DecisionStep::Close);
            continue;
        };
        if start == end {
            continue;
        }
        let tests = &plans[start].tests;
        if tests.len() <= depth {
            out.push(DecisionStep::Body(start));
            continue;
        }
        let key = &tests[depth];
        let mut stop = start + 1;
        while stop < end && plans[stop].tests.get(depth) == Some(key) {
            stop += 1;
        }
        work.push(Work::Range(stop, end, depth));
        if stop == start + 1 {
            out.push(DecisionStep::Open(tests[depth..].join("&&")));
            out.push(DecisionStep::Body(start));
            out.push(DecisionStep::Close);
        } else {
            out.push(DecisionStep::Open(key.clone()));
            work.push(Work::Close);
            work.push(Work::Range(start, stop, depth + 1));
        }
    }
    out
}
