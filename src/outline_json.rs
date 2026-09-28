//! Elm 0.19.1 Json.Decode lexical rules, with raw string contents and Word-sized
//! integers. Adapted from the BSD-licensed reference; see LICENSE-ELM.
use serde_json::{Value, json};

#[derive(Clone, Copy)]
enum Kind {
    Start,
    ObjectField,
    ObjectColon,
    ObjectEnd,
    ArrayEnd,
    StringEnd,
    Control,
    Escape,
    Hex,
    LeadingZero,
    Float,
    End,
}
impl Kind {
    fn title(self) -> &'static str {
        match self {
            Self::Start => "EXPECTING A VALUE",
            Self::ObjectField => "EXTRA COMMA",
            Self::ObjectColon => "EXPECTING COLON",
            Self::ObjectEnd => "UNFINISHED OBJECT",
            Self::ArrayEnd => "UNFINISHED ARRAY",
            Self::StringEnd => "ENDLESS STRING",
            Self::Control => "UNEXPECTED CONTROL CHARACTER",
            Self::Escape => "UNKNOWN ESCAPE",
            Self::Hex => "BAD HEX ESCAPE",
            Self::LeadingZero => "BAD NUMBER",
            Self::Float => "UNEXPECTED NUMBER",
            Self::End => "JSON PROBLEM",
        }
    }
    fn introduction(self) -> &'static str {
        match self {
            Self::Start => "I was expecting to see a JSON value next:",
            Self::ObjectField | Self::ObjectColon | Self::ObjectEnd => {
                "I was partway through parsing a JSON object when I got stuck here:"
            }
            Self::ArrayEnd => "I was partway through parsing a JSON array when I got stuck here:",
            Self::StringEnd => {
                "I got to the end of the line without seeing the closing double quote:"
            }
            Self::Control => "I ran into a control character unexpectedly:",
            Self::Escape => {
                "Backslashes always start escaped characters, but I do not recognize this one:"
            }
            Self::Hex => "This is not a valid hex escape:",
            Self::LeadingZero => "Numbers cannot start with zeros like this:",
            Self::Float => "I got stuck while trying to parse this number:",
            Self::End => "I was partway through parsing some JSON when I got stuck here:",
        }
    }
}
struct Error {
    kind: Kind,
    row: usize,
    col: usize,
}
struct Parser<'a> {
    source: &'a str,
    offset: usize,
    row: usize,
    col: usize,
    output: String,
}
type Result<T> = std::result::Result<T, Error>;
impl Parser<'_> {
    fn error(&self, kind: Kind) -> Error {
        Error {
            kind,
            row: self.row,
            col: self.col,
        }
    }
    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }
    fn advance(&mut self) {
        if let Some(ch) = self.peek() {
            self.offset += ch.len_utf8();
            match ch {
                '\n' => {
                    self.row += 1;
                    self.col = 1;
                }
                '\r' => {}
                _ => self.col += 1,
            }
        }
    }
    fn eat(&mut self, ch: char) -> bool {
        if self.peek() == Some(ch) {
            self.advance();
            self.output.push(ch);
            true
        } else {
            false
        }
    }
    fn spaces(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.advance();
        }
        self.output.push(' ');
    }
    fn string(&mut self, missing: Kind) -> Result<()> {
        if self.peek() != Some('"') {
            return Err(self.error(missing));
        }
        self.advance();
        let start = self.offset;
        loop {
            match self.peek() {
                None | Some('\n') => return Err(self.error(Kind::StringEnd)),
                Some('"') => {
                    let raw = &self.source[start..self.offset];
                    self.output
                        .push_str(&serde_json::to_string(raw).expect("string serialization"));
                    self.advance();
                    return Ok(());
                }
                Some('\\') => {
                    let at = self.error(Kind::Escape);
                    let suffix = &self.source[self.offset + 1..];
                    match suffix.chars().next() {
                        None => {
                            return Err(Error {
                                kind: Kind::StringEnd,
                                row: self.row + 1,
                                col: self.col,
                            });
                        }
                        Some('"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't') => {
                            self.advance();
                            self.advance();
                        }
                        Some('u') => {
                            if !suffix
                                .as_bytes()
                                .get(1..5)
                                .is_some_and(|bytes| bytes.iter().all(u8::is_ascii_hexdigit))
                            {
                                return Err(Error {
                                    kind: Kind::Hex,
                                    ..at
                                });
                            }
                            for _ in 0..6 {
                                self.advance();
                            }
                        }
                        _ => return Err(at),
                    }
                }
                Some(ch) if ch < '\u{20}' => return Err(self.error(Kind::Control)),
                Some(_) => self.advance(),
            }
        }
    }
    fn number(&mut self) -> Result<()> {
        let zero = self.peek() == Some('0');
        let mut value = 0i64;
        while let Some(ch) = self.peek().filter(char::is_ascii_digit) {
            value = value
                .wrapping_mul(10)
                .wrapping_add(i64::from(ch as u8 - b'0'));
            self.advance();
            if zero {
                break;
            }
        }
        if zero && self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            return Err(self.error(Kind::LeadingZero));
        }
        if self.peek() == Some('.') || (!zero && matches!(self.peek(), Some('e' | 'E'))) {
            return Err(self.error(Kind::Float));
        }
        self.output.push_str(&value.to_string());
        Ok(())
    }
    fn keyword(&mut self) -> Result<()> {
        for word in ["true", "false", "null"] {
            if let Some(suffix) = self.source[self.offset..].strip_prefix(word)
                && !suffix.chars().next().is_some_and(|ch| {
                    crate::unicode::is_alpha(ch) || ch.is_ascii_digit() || ch == '_'
                })
            {
                self.output.push_str(word);
                for _ in 0..word.len() {
                    self.advance();
                }
                return Ok(());
            }
        }
        Err(self.error(Kind::Start))
    }
    fn value(&mut self) -> Result<()> {
        enum Frame {
            Value,
            ObjectKey,
            ObjectEnd,
            ArrayEnd,
        }
        let mut frames = vec![Frame::Value];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Value => match self.peek() {
                    Some('"') => self.string(Kind::Start)?,
                    Some('{') => {
                        self.eat('{');
                        self.spaces();
                        if !self.eat('}') {
                            frames.push(Frame::ObjectKey);
                        }
                    }
                    Some('[') => {
                        self.eat('[');
                        self.spaces();
                        if !self.eat(']') {
                            frames.extend([Frame::ArrayEnd, Frame::Value]);
                        }
                    }
                    Some(ch) if ch.is_ascii_digit() => self.number()?,
                    _ => self.keyword()?,
                },
                Frame::ObjectKey => {
                    self.string(Kind::ObjectField)?;
                    self.spaces();
                    if !self.eat(':') {
                        return Err(self.error(Kind::ObjectColon));
                    }
                    self.spaces();
                    frames.extend([Frame::ObjectEnd, Frame::Value]);
                }
                Frame::ObjectEnd => {
                    self.spaces();
                    if !self.eat('}') {
                        if !self.eat(',') {
                            return Err(self.error(Kind::ObjectEnd));
                        }
                        self.spaces();
                        frames.push(Frame::ObjectKey);
                    }
                }
                Frame::ArrayEnd => {
                    self.spaces();
                    if !self.eat(']') {
                        if !self.eat(',') {
                            return Err(self.error(Kind::ArrayEnd));
                        }
                        self.spaces();
                        frames.extend([Frame::ArrayEnd, Frame::Value]);
                    }
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn normalize(source: &str) -> std::result::Result<String, String> {
    let mut parser = Parser {
        source,
        offset: 0,
        row: 1,
        col: 1,
        output: String::new(),
    };
    let result = (|| {
        parser.spaces();
        parser.value()?;
        parser.spaces();
        if parser.peek().is_some() {
            return Err(parser.error(Kind::End));
        }
        Ok(())
    })();
    result.map_err(|error| format!("ELM_OUTLINE_JSON:{}", report(source, error)))?;
    Ok(parser.output)
}

fn report(source: &str, error: Error) -> Value {
    let title = error.kind.title();
    let mut message = Vec::new();
    crate::docs_diagnostic::text(
        &mut message,
        format!(
            "{}\n\n",
            crate::docs_diagnostic::reflow(&format!(
                "I ran into a problem with your elm.json file. {}",
                error.kind.introduction()
            ))
        ),
    );
    let first = error.row.saturating_sub(2).max(1);
    let width = error.row.to_string().len();
    for (index, line) in source
        .split_terminator('\n')
        .chain(std::iter::once(""))
        .enumerate()
        .skip(first - 1)
        .take(error.row - first + 1)
    {
        crate::docs_diagnostic::text(&mut message, format!("{:>width$}| {line}\n", index + 1));
    }
    crate::docs_diagnostic::text(&mut message, " ".repeat(width + error.col + 1));
    message.push(json!({"bold":false,"underline":false,"color":"RED","string":"^"}));
    let templates: Value = serde_json::from_str(include_str!("outline_json_messages.json"))
        .expect("JSON diagnostic templates");
    if let Some(body) = templates[title].as_array() {
        for chunk in body {
            if let Some(plain) = chunk.as_str() {
                crate::docs_diagnostic::text(&mut message, plain.into());
            } else {
                message.push(chunk.clone());
            }
        }
    }
    json!({"type":"error","path":"elm.json","title":title,"message":message})
}
