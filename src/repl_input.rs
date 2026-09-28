//! REPL line accumulation and input classification. Complete inputs use the
//! compiler parser; malformed declaration/expression selection uses parser
//! positions and still needs exhaustive diagnostic-position parity testing.
use crate::{
    ast::Declaration,
    parser::{parse, parse_repl_fragment},
    repl_session::Input,
};

#[derive(Debug)]
pub enum Command {
    Evaluate(Input),
    Skip,
    Reset,
    Exit,
    Help(Option<String>),
    Port,
}
#[derive(Debug)]
pub enum Action {
    More { prefill: String },
    Ready(Command),
}
#[derive(Default)]
pub struct Reader {
    lines: Vec<String>,
}
impl Reader {
    pub fn cancel(&mut self) {
        self.lines.clear();
    }
    pub fn push(&mut self, line: &str) -> Action {
        self.lines
            .push(line.strip_suffix('\\').unwrap_or(line).into());
        let action = classify(&self.lines);
        if matches!(action, Action::Ready(_)) {
            self.lines.clear();
        }
        action
    }
}

fn keyword(line: &str, word: &str) -> bool {
    line.strip_prefix(word)
        .is_some_and(|rest| rest.chars().next().is_none_or(|c| !c.is_alphanumeric()))
}
fn more() -> Action {
    Action::More {
        prefill: "  ".into(),
    }
}
fn evaluate(input: Input) -> Action {
    Action::Ready(Command::Evaluate(input))
}
fn position(error: &str, row_offset: usize, column_offset: usize) -> (usize, usize) {
    let mut parts = error.split(':');
    (
        parts
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
            .saturating_sub(row_offset),
        parts
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
            .saturating_sub(column_offset),
    )
}
fn classify(lines: &[String]) -> Action {
    let first = &lines[0];
    let last_blank = lines.last().unwrap().chars().all(|c| c == ' ');
    if lines.len() == 1 && last_blank {
        return Action::Ready(Command::Skip);
    }
    if let Some(command) = first.trim_start_matches(' ').strip_prefix(':') {
        return Action::Ready(match command {
            "reset" => Command::Reset,
            "exit" | "quit" => Command::Exit,
            "help" => Command::Help(None),
            other => Command::Help(Some(other.split(' ').next().unwrap_or("").into())),
        });
    }
    let source = lines.join("\n") + "\n";
    let module = format!("module Elm_Repl exposing (..)\n{source}");
    let declaration = parse_repl_fragment(&module);
    if keyword(first, "import") {
        let name = declaration.as_ref().ok().and_then(|ast| {
            if ast.header.imports.len() == 1 && ast.declarations.is_empty() {
                Some(ast.header.imports[0].name.clone())
            } else {
                None
            }
        });
        return match name {
            Some(name) => evaluate(Input::Import { name, source }),
            None if last_blank => evaluate(Input::Import {
                name: "ERR".into(),
                source,
            }),
            None => more(),
        };
    }
    if keyword(first, "port") {
        return Action::Ready(Command::Port);
    }
    if let Ok(ast) = &declaration {
        let classified = match ast.declarations.as_slice() {
            [Declaration::Value { name, .. }] => Some((false, *name)),
            [
                Declaration::Annotation {
                    name: annotated, ..
                },
                Declaration::Value { name, .. },
            ] if annotated == name => Some((false, *name)),
            [Declaration::Alias { name, .. } | Declaration::Union { name, .. }] => {
                Some((true, *name))
            }
            [Declaration::Annotation { name, .. }] => {
                return Action::More {
                    prefill: format!("{name} "),
                };
            }
            _ => None,
        };
        if let Some((is_type, name)) = classified {
            if lines.len() != 1 && !last_blank {
                return more();
            }
            let name = name.to_string();
            return evaluate(if is_type {
                Input::Type { name, source }
            } else {
                Input::Declaration { name, source }
            });
        }
    }
    if keyword(first, "type") {
        return if last_blank {
            evaluate(Input::Type {
                name: "ERR".into(),
                source,
            })
        } else {
            more()
        };
    }
    let body = source
        .split_terminator('\n')
        .map(|line| format!("  {line}\n"))
        .collect::<String>();
    let expression = format!("module Elm_Repl exposing (..)\nrepl_input_value_ =\n{body}");
    match parse(&expression) {
        Ok(_) if lines.len() == 1 || last_blank => evaluate(Input::Expression { source }),
        Ok(_) => more(),
        Err(error) if last_blank => {
            let declaration_position = declaration
                .as_ref()
                .err()
                .map(|error| position(error, 1, 0))
                .unwrap_or_default();
            evaluate(if position(&error, 2, 2) >= declaration_position {
                Input::Expression { source }
            } else {
                Input::Declaration {
                    name: "ERR".into(),
                    source,
                }
            })
        }
        Err(_) => more(),
    }
}
