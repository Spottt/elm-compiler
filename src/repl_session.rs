//! Transactional REPL declarations, matching terminal/Repl.hs. Parsing user
//! input and terminal interaction are separate from this state machine.
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum Input {
    Import { name: String, source: String },
    Type { name: String, source: String },
    Declaration { name: String, source: String },
    Expression { source: String },
}

#[derive(Debug, Clone, Default)]
pub struct Session {
    imports: BTreeMap<String, String>,
    types: BTreeMap<String, String>,
    declarations: BTreeMap<String, String>,
}

impl Session {
    /// Match terminal/Repl.hs: declarations, types, imports, then commands.
    /// Only imported module names receive a trailing space on completion.
    pub fn completions(&self, prefix: &str) -> Vec<(String, bool)> {
        self.declarations
            .keys()
            .map(|name| (name.as_str(), false))
            .chain(self.types.keys().map(|name| (name.as_str(), false)))
            .chain(self.imports.keys().map(|name| (name.as_str(), true)))
            .chain(
                [":exit", ":help", ":quit", ":reset"]
                    .into_iter()
                    .map(|name| (name, false)),
            )
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, finished)| (name.to_owned(), finished))
            .collect()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The callback must compile and, when a binding is provided, execute it.
    /// A compilation or runtime error leaves the previous definitions intact.
    pub fn attempt<T, E>(
        &mut self,
        input: Input,
        evaluate: impl FnOnce(&str, Option<&str>) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut candidate = self.clone();
        let mut expression = None;
        let binding = match input {
            Input::Import { name, source } => {
                candidate.imports.insert(name, source);
                None
            }
            Input::Type { name, source } => {
                candidate.types.insert(name, source);
                None
            }
            Input::Declaration { name, source } => {
                candidate.declarations.insert(name.clone(), source);
                Some(name)
            }
            Input::Expression { source } => {
                expression = Some(source);
                Some("repl_input_value_".into())
            }
        };
        let mut source = "module Elm_Repl exposing (..)\n".to_string();
        for fragment in candidate
            .imports
            .values()
            .chain(candidate.types.values())
            .chain(candidate.declarations.values())
        {
            source.push_str(fragment);
            if !fragment.ends_with('\n') {
                source.push('\n');
            }
        }
        source.push_str("repl_input_value_ =");
        match expression {
            Some(expression) => {
                for line in expression.split_terminator('\n') {
                    source.push_str("\n  ");
                    source.push_str(line);
                }
                source.push('\n');
            }
            None => source.push_str(" ()\n"),
        }
        let result = evaluate(&source, binding.as_deref())?;
        *self = candidate;
        Ok(result)
    }
}
