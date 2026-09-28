//! Module headers, export lists and imports. Body parsing lives in parser.rs.
use crate::lexer::{Kind, Token};
#[derive(Debug, PartialEq, Eq)]
pub enum Exposed {
    Value(String),
    Type { name: String, constructors: bool },
    Operator(String),
}
#[derive(Debug, PartialEq, Eq)]
pub enum Exposing {
    All,
    Explicit(Vec<Exposed>),
}
#[derive(Debug, PartialEq, Eq)]
pub enum Effects {
    None,
    Ports,
    Manager {
        command: Option<String>,
        subscription: Option<String>,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub struct Import {
    pub name: String,
    pub alias: Option<String>,
    pub exposing: Exposing,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Header {
    pub explicit: bool,
    pub name: String,
    pub effects: Effects,
    pub exposing: Exposing,
    pub imports: Vec<Import>,
    pub body_start: usize,
}
struct Parser<'a> {
    source: &'a str,
    tokens: &'a [Token],
    pos: usize,
    literal_failure: bool,
}
impl Parser<'_> {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(|t| t.text(self.source))
    }
    fn take(&mut self, text: &str) -> bool {
        if self.peek() == Some(text) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn position(&self) -> (u32, u32) {
        self.tokens.get(self.pos).map_or_else(
            || crate::lexer::end_position(self.source, self.tokens),
            |t| (t.row, t.column),
        )
    }
    fn error(&self, message: &str) -> String {
        let (row, col) = self.position();
        format!("{row}:{col}: {message}; got {:?}", self.peek())
    }
    fn header_problem(&self, port: bool) -> String {
        let (row, column) = self.position();
        Self::header_problem_at(row, column, port)
    }
    fn header_problem_at(row: u32, column: u32, port: bool) -> String {
        format!(
            "{row}:{column}: {}",
            if port {
                "unfinished port module declaration"
            } else {
                "unfinished module declaration"
            }
        )
    }
    fn header_indent(&self, port: bool) -> Result<(), String> {
        if self.position().1 > 1 {
            return Ok(());
        }
        // Parse.Space reports the position before consuming the whitespace
        // which revealed the indentation failure, not the following token.
        let (row, column) = self
            .pos
            .checked_sub(1)
            .and_then(|i| self.tokens.get(i))
            .map(|token| {
                token.text(self.source).chars().fold(
                    (token.row, token.column),
                    |(row, column), c| {
                        if c == '\n' {
                            (row + 1, 1)
                        } else {
                            (row, column + 1)
                        }
                    },
                )
            })
            .unwrap_or_else(|| self.position());
        Err(Self::header_problem_at(row, column, port))
    }
    fn expect(&mut self, text: &str) -> Result<(), String> {
        if self.take(text) {
            Ok(())
        } else {
            Err(self.error(&format!("expected {text:?}")))
        }
    }
    fn upper(&mut self) -> Result<String, String> {
        let t = self
            .tokens
            .get(self.pos)
            .ok_or_else(|| self.error("expected uppercase name"))?;
        if t.kind != Kind::Upper {
            return Err(self.error("expected uppercase name"));
        }
        self.pos += 1;
        Ok(t.text(self.source).into())
    }
    fn name(&mut self) -> Result<String, String> {
        let mut name = self.upper()?;
        while self.peek() == Some(".") {
            let before = self.tokens[self.pos - 1].end;
            let dot = self.tokens[self.pos];
            if dot.start != before
                || self
                    .tokens
                    .get(self.pos + 1)
                    .is_none_or(|t| t.start != dot.end)
            {
                return Err(self.error("module name dots must be adjacent"));
            }
            self.pos += 1;
            name.push('.');
            name.push_str(&self.upper()?);
        }
        Ok(name)
    }
    fn header_name(&mut self, port: bool) -> Result<String, String> {
        self.name().map_err(|error| {
            let point = if self.peek() == Some(".")
                && self.pos > 0
                && self.tokens[self.pos - 1].end == self.tokens[self.pos].start
            {
                let dot = self.tokens[self.pos];
                Some((dot.row, dot.column + 1))
            } else if error.contains("expected uppercase name") {
                let mut parts = error.split(':');
                parts
                    .next()
                    .and_then(|r| r.parse::<u32>().ok())
                    .zip(parts.next().and_then(|c| c.parse::<u32>().ok()))
            } else {
                None
            };
            match point {
                Some((row, column)) if column > 1 => format!(
                    "{row}:{column}: {}",
                    if port {
                        "expected port module name"
                    } else {
                        "expected module name"
                    }
                ),
                _ if self.peek() == Some(".") => self.header_problem(port),
                _ => error,
            }
        })
    }
    fn fresh(&self) -> Result<(), String> {
        if self.tokens.get(self.pos).is_none_or(|t| t.column == 1) {
            Ok(())
        } else {
            Err(self.error("expected a new declaration in column 1"))
        }
    }
    fn import_indent(&self, kind: &str) -> Result<(), String> {
        if self.position().1 > 1 {
            Ok(())
        } else {
            let previous = &self.tokens[self.pos - 1];
            let row = previous.row;
            let column = previous.column + previous.text(self.source).chars().count() as u32;
            Err(format!("{row}:{column}: import {kind}"))
        }
    }
    fn exposing(&mut self, start_line: u32) -> Result<Exposing, String> {
        self.exposing_expect("(", start_line, "start")?;
        self.exposing_indent(start_line, "indent-value")?;
        // The wildcard is a literal two-byte prefix, not an operator token.
        // A longer operator leaves its remaining symbols where ')' is expected.
        if self
            .peek()
            .is_some_and(|text| text.starts_with("..") && text != "..")
        {
            let (row, column) = self.position();
            return Err(format!("{row}:{}: exposing {start_line} end", column + 2));
        }
        if self.take("..") {
            self.exposing_indent(start_line, "indent-end")?;
            self.exposing_expect(")", start_line, "end")?;
            return Ok(Exposing::All);
        }
        let mut items = Vec::new();
        loop {
            let t = self
                .tokens
                .get(self.pos)
                .ok_or_else(|| self.exposing_error(start_line, "value"))?;
            let item = match t.kind {
                Kind::Lower if !crate::parser::reserved(t.text(self.source)) => {
                    self.pos += 1;
                    Exposed::Value(t.text(self.source).into())
                }
                Kind::Upper => {
                    let name = self.upper()?;
                    self.exposing_indent(start_line, "indent-end")?;
                    let constructors = if self.take("(") {
                        self.exposing_indent(start_line, "privacy")?;
                        // Elm consumes the two literal dots before expecting
                        // ')', even if the lexer grouped more operator bytes.
                        if self
                            .peek()
                            .is_some_and(|text| text.starts_with("..") && text != "..")
                        {
                            let (row, column) = self.position();
                            return Err(format!(
                                "{row}:{}: exposing {start_line} privacy",
                                column + 2
                            ));
                        }
                        self.exposing_expect("..", start_line, "privacy")?;
                        self.exposing_indent(start_line, "privacy")?;
                        self.exposing_expect(")", start_line, "privacy")?;
                        true
                    } else {
                        false
                    };
                    Exposed::Type { name, constructors }
                }
                _ if self.take("(") => {
                    self.exposing_adjacent(start_line, "operator")?;
                    let op = self.peek().unwrap().to_string();
                    if matches!(op.as_str(), "." | "|" | "->" | "=" | ":") {
                        let (row, column) = self.position();
                        return Err(format!(
                            "{row}:{column}: exposing {start_line} reserved-{op}"
                        ));
                    }
                    if op != ".." && !crate::parser::binop(&op) {
                        let (row, column) = self.position();
                        return Err(format!("{row}:{column}: exposing {start_line} operator"));
                    }
                    self.pos += 1;
                    self.exposing_adjacent(start_line, "operator-close")?;
                    self.exposing_expect(")", start_line, "operator-close")?;
                    Exposed::Operator(op)
                }
                _ => return Err(self.exposing_error(start_line, "value")),
            };
            items.push(item);
            self.exposing_indent(start_line, "indent-end")?;
            if !self.take(",") {
                break;
            }
            self.exposing_indent(start_line, "indent-value")?;
        }
        self.exposing_expect(")", start_line, "end")?;
        Ok(Exposing::Explicit(items))
    }
    fn exposing_error(&self, start_line: u32, kind: &str) -> String {
        let (row, column) = self.position();
        format!("{row}:{column}: exposing {start_line} {kind}")
    }
    fn exposing_expect(
        &mut self,
        expected: &str,
        start_line: u32,
        kind: &str,
    ) -> Result<(), String> {
        if self.take(expected) {
            Ok(())
        } else {
            let (row, column) = self.position();
            Err(format!("{row}:{column}: exposing {start_line} {kind}"))
        }
    }
    fn exposing_indent(&self, start_line: u32, kind: &str) -> Result<(), String> {
        if self.position().1 > 1 {
            Ok(())
        } else {
            let token = &self.tokens[self.pos - 1];
            let (row, column) = token.text(self.source).chars().fold(
                (token.row, token.column),
                |(row, column), c| {
                    if c == '\n' {
                        (row + 1, 1)
                    } else {
                        (row, column + 1)
                    }
                },
            );
            Err(format!("{row}:{column}: exposing {start_line} {kind}"))
        }
    }
    fn exposing_adjacent(&mut self, start_line: u32, kind: &str) -> Result<(), String> {
        if self
            .tokens
            .get(self.pos)
            .is_some_and(|next| self.pos > 0 && self.tokens[self.pos - 1].end == next.start)
        {
            Ok(())
        } else {
            self.literal_failure = true;
            let previous = &self.tokens[self.pos - 1];
            let row = previous.row;
            let column = previous.column + previous.text(self.source).chars().count() as u32;
            Err(format!("{row}:{column}: exposing {start_line} {kind}"))
        }
    }
    fn effect_problem(error: String) -> String {
        let mut parts = error.splitn(3, ':');
        format!("{}:{}: bad effect header", parts.next().unwrap(), parts.next().unwrap())
    }
    fn effect_indent(&self) -> Result<(), String> {
        self.header_indent(false).map_err(Self::effect_problem)
    }
    fn effect_expect(&mut self, token: &str) -> Result<(), String> {
        self.expect(token).map_err(Self::effect_problem)
    }
    fn manager(&mut self) -> Result<Effects, String> {
        self.effect_indent()?;
        self.effect_expect("where")?;
        self.effect_indent()?;
        self.effect_expect("{")?;
        self.effect_indent()?;
        let mut command = None;
        let mut subscription = None;
        loop {
            let slot = match self.peek() {
                Some("command") => &mut command,
                Some("subscription") => &mut subscription,
                _ => return Err(Self::effect_problem(self.error("expected manager field"))),
            };
            if slot.is_some() {
                return Err(Self::effect_problem(self.error("duplicate manager field")));
            }
            self.pos += 1;
            self.effect_indent()?;
            self.effect_expect("=")?;
            self.effect_indent()?;
            *slot = Some(self.upper().map_err(Self::effect_problem)?);
            self.effect_indent()?;
            if !self.take(",") {
                break;
            }
            self.effect_indent()?;
        }
        self.effect_expect("}")?;
        self.effect_indent()?;
        Ok(Effects::Manager {
            command,
            subscription,
        })
    }

}
pub fn header(source: &str, tokens: &[Token]) -> Result<Header, String> {
    parse_header(&mut Parser {
        source,
        tokens,
        pos: 0,
        literal_failure: false,
    })
}

/// A lexical error later in the input must not mask an already established
/// header error. Header grammar can also reject a malformed literal's first
/// character without reading its contents. This recovery-only token never
/// participates in successful lexing or a successfully returned syntax tree.
pub(crate) fn prefer_header_error(source: &str, tokens: &[Token], lexical_error: String) -> String {
    let mut header_tokens;
    let tokens = if let Some(token) = crate::lexer::failed_token_start(source, tokens) {
        header_tokens = tokens.to_vec();
        header_tokens.push(token);
        header_tokens.as_slice()
    } else {
        tokens
    };
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        literal_failure: false,
    };
    match parse_header(&mut parser) {
        Err(error) if parser.pos < tokens.len() || parser.literal_failure => error,
        _ => lexical_error,
    }
}

fn parse_header(p: &mut Parser<'_>) -> Result<Header, String> {
    p.fresh()?;
    let effect = p.take("effect");
    let port = !effect && p.take("port");
    if port {
        p.header_indent(true)?;
    } else if effect {
        p.effect_indent()?;
    }
    let explicit = p.peek() == Some("module");
    let (name, effects, exposing) = if p.take("module") {
        if effect {
            p.effect_indent()?;
        } else {
            p.header_indent(port)?;
        }
        let name = p.header_name(port)?;
        let effects = if effect {
            p.manager()?
        } else if port {
            Effects::Ports
        } else {
            Effects::None
        };
        if effect {
            p.effect_expect("exposing")?;
            p.effect_indent()?;
        } else {
            p.header_indent(port)?;
            if !p.take("exposing") {
                return Err(p.header_problem(port));
            }
            p.header_indent(port)?;
        }
        let exposing_start = p.position();
        let exposing = p.exposing(p.tokens[0].row).map_err(|error| {
            if effect {
                // specialize (const E.Effect) reports the start of exposing.
                format!("{}:{}: bad effect header", exposing_start.0, exposing_start.1)
            } else {
                error
            }
        })?;
        p.fresh()?;
        (name, effects, exposing)
    } else if port {
        return Err(p.header_problem(true));
    } else if effect {
        return Err(Parser::effect_problem(p.error("expected module header")));
    } else {
        ("Main".into(), Effects::None, Exposing::All)
    };
    let mut imports = Vec::new();
    while p.take("import") {
        let import_line = p.tokens[p.pos - 1].row;
        p.import_indent("end")?;
        let name = p.header_name(false).map_err(|error| {
            if let Some(prefix) = error.strip_suffix("expected module name") {
                format!("{prefix}import name")
            } else if let Some(prefix) = error.strip_suffix("unfinished module declaration") {
                format!("{prefix}import end")
            } else {
                error
            }
        })?;
        let alias = if p.position().1 > 1 && p.take("as") {
            p.import_indent("end")?;
            Some(p.upper().map_err(|_| {
                let (row, column) = p.position();
                format!("{row}:{column}: import alias")
            })?)
        } else {
            None
        };
        let exposing = if p.position().1 > 1 && p.take("exposing") {
            p.import_indent("exposed-list")?;
            p.exposing(import_line)?
        } else {
            Exposing::Explicit(Vec::new())
        };
        // Unlike a completed expression, an import must end at column one,
        // even at EOF. Trailing spaces/comments do not imply a fresh line.
        if p.position().1 != 1 {
            let (row, column) = p.position();
            return Err(format!("{row}:{column}: import end"));
        }
        imports.push(Import {
            name,
            alias,
            exposing,
        });
    }
    Ok(Header {
        explicit,
        name,
        effects,
        exposing,
        imports,
        body_start: p.pos,
    })
}
