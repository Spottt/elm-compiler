//! Terminal adapter for the session's Elm-compatible completion candidates.
use rustyline::{
    Context, Helper,
    completion::{Completer, Pair},
    highlight::Highlighter,
    hint::Hinter,
    validate::Validator,
};

#[derive(Default)]
pub struct Completion {
    pub candidates: Vec<(String, bool)>,
}
impl Helper for Completion {}
impl Highlighter for Completion {}
impl Validator for Completion {}
impl Hinter for Completion {
    type Hint = String;
}
impl Completer for Completion {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Haskeline's completeWord uses only space and newline as separators.
        let start = line[..pos].rfind([' ', '\n']).map_or(0, |index| index + 1);
        let prefix = &line[start..pos];
        let candidates = self
            .candidates
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, finished)| Pair {
                display: name.clone(),
                replacement: if *finished {
                    format!("{name} ")
                } else {
                    name.clone()
                },
            })
            .collect();
        Ok((start, candidates))
    }
}
