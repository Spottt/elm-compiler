//! Source-backed tokens: no allocation per identifier or literal. Coordinates
//! use Unicode scalar columns and UTF-8 byte offsets. Layout stays available to
//! the parser even though comments and whitespace are discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Lower,
    Upper,
    Number,
    String,
    Char,
    Symbol,
    Shader,
}
#[derive(Debug, Clone, Copy)]
pub struct Token {
    pub kind: Kind,
    pub start: u32,
    pub end: u32,
    pub row: u32,
    pub column: u32,
}
impl Token {
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start as usize..self.end as usize]
    }
}
/// An outer documentation comment, including its delimiters in the byte span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocComment {
    pub start: u32,
    pub end: u32,
    pub row: u32,
    pub column: u32,
}
impl DocComment {
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start as usize + 3..self.end as usize - 2]
    }
    /// Json.String.fromComment removes carriage returns but keeps all other
    /// whitespace, nested-comment delimiters, quotes and backslashes.
    pub fn normalized_text(&self, source: &str) -> String {
        self.text(source).replace('\r', "")
    }
}

struct Cursor<'a> {
    source: &'a str,
    offset: usize,
    row: u32,
    column: u32,
}
impl Cursor<'_> {
    fn rest(&self) -> &str {
        &self.source[self.offset..]
    }
    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }
    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.offset += c.len_utf8();
        if c == '\n' {
            self.row += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }
    fn take(&mut self, text: &str) -> bool {
        if !self.rest().starts_with(text) {
            return false;
        }
        for _ in text.chars() {
            self.advance();
        }
        true
    }
    fn error(&self, message: &str) -> String {
        format!("{}:{}: {message}", self.row, self.column)
    }
    fn skip(&mut self, docs: &mut Option<&mut Vec<DocComment>>) -> Result<(), String> {
        loop {
            match self.peek() {
                // Parse.Space ignores CR in layout, but comments and literals
                // still advance their columns normally.
                Some('\r') => self.offset += 1,
                Some(' ' | '\n') => {
                    self.advance();
                }
                Some('\t') => return Err(self.error("tabs are not allowed in Elm layout")),
                _ if self.take("--") => {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.advance();
                    }
                }
                // Parse.Space needs a byte after the opener before treating
                // this as a comment. A bare '{-' remains syntax to parse.
                _ if self.rest().len() > 2 && self.take("{-") => {
                    let start = (self.offset as u32 - 2, self.row, self.column - 2);
                    let documentation = self.peek() == Some('|');
                    let mut depth = 1usize;
                    while depth > 0 {
                        if self.take("{-") {
                            depth += 1;
                        } else if self.take("-}") {
                            depth -= 1;
                        } else if self.peek() == Some('\t') {
                            return Err(self.error("tabs are not allowed in block comments"));
                        } else if self.advance().is_none() {
                            return Err(format!(
                                "{}:{}: unterminated block comment",
                                start.1, start.2
                            ));
                        }
                    }
                    if documentation && let Some(docs) = docs.as_mut() {
                        docs.push(DocComment {
                            start: start.0,
                            end: self.offset as u32,
                            row: start.1,
                            column: start.2,
                        });
                    }
                }
                _ => return Ok(()),
            }
        }
    }
    fn escape(&mut self) -> Result<(), String> {
        let (escape_row, escape_column) = (self.row, self.column - 1);
        match self.advance() {
            Some('n' | 'r' | 't' | '"' | '\'' | '\\') => Ok(()),
            Some('u') => {
                let error = |details: String| format!("{escape_row}:{escape_column}: literal unicode {details}");
                if !self.take("{") {
                    return Err(error("format 2".into()));
                }
                let start = self.offset;
                let mut value = -1i64;
                let mut accumulator = 0i64;
                while let Some(digit) = self.peek().and_then(|c| c.to_digit(16)) {
                    let next = accumulator.wrapping_mul(16).wrapping_add(digit as i64);
                    if next < 0 {
                        break;
                    }
                    self.advance();
                    value = next;
                    accumulator = next;
                }
                let digits = self.offset - start;
                if !self.take("}") {
                    return Err(error(format!("format {}", digits + 3)));
                }
                let width = digits + 4;
                if !(0..=0x10ffff).contains(&value) {
                    return Err(error(format!("code {width}")));
                }
                if !(4..=6).contains(&digits) {
                    return Err(error(format!("length {width} {digits} {value}")));
                }
                Ok(())
            }
            _ => Err(format!("{escape_row}:{escape_column}: literal unknown escape")),
        }
    }
    fn quoted(&mut self, quote: char, multiline: bool) -> Result<(), String> {
        let (start_row, start_column) = (self.row, self.column - if multiline { 3 } else { 1 });
        let mut length = 0;
        loop {
            if multiline && self.take("\"\"\"") {
                return Ok(());
            }
            let (row, column) = (self.row, self.column);
            let endless = |row, column| {
                if multiline {
                    format!("{start_row}:{start_column}: literal endless multi")
                } else {
                    format!("{row}:{column}: literal endless {}", if quote == '\'' { "char" } else { "single" })
                }
            };
            match self.advance() {
                None => return Err(endless(row, column)),
                Some(c) if !multiline && c == quote => {
                    if quote == '\'' && length != 1 {
                        return Err(format!("{start_row}:{start_column}: literal char width {}", self.column - start_column));
                    }
                    return Ok(());
                }
                Some('\n') if !multiline => return Err(endless(row, column)),
                Some('\\') => {
                    if self.peek().is_none() {
                        return Err(endless(row, column + u32::from(quote != '\'')));
                    }
                    self.escape()?;
                    length += 1;
                }
                Some('\r') if multiline => self.column -= 1,
                Some(_) => length += 1,
            }
        }
    }
    fn number(&mut self) -> Result<(), String> {
        let number_start = self.offset;
        let zero = self.peek() == Some('0');
        self.advance();
        if zero && self.take("x") {
            let start = self.offset;
            let mut value = 0i64;
            while let Some(digit) = self.peek().and_then(|c| c.to_digit(16)) {
                let next = value.wrapping_mul(16).wrapping_add(digit as i64);
                if next == -1 {
                    // Haskell's chompHex sentinel also catches an overflow to
                    // -1: retain the previous value and leave this digit unread.
                    return Ok(());
                }
                if next < 0 {
                    return Err(self.error("invalid hexadecimal literal"));
                }
                value = next;
                self.advance();
            }
            if self.offset == start || self.peek().is_some_and(inner) {
                return Err(self.error("invalid hexadecimal literal"));
            }
            return Ok(());
        }
        if zero && self.peek().is_some_and(|c| c.is_ascii_digit()) {
            return Err(self.error("leading zero"));
        }
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }
        let fractional = self.take(".");
        if fractional {
            let start = self.offset;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
            if start == self.offset {
                let integer = self.source[number_start..self.offset - 1]
                    .bytes()
                    .fold(0i64, |n, digit| n.wrapping_mul(10).wrapping_add((digit - b'0') as i64));
                return Err(format!("{}:{}: number dot {integer}", self.row, self.column - 1));
            }
        }
        if (!zero || fractional) && (self.take("e") || self.take("E")) {
            let exponent_column = self.column;
            if !self.take("+") {
                self.take("-");
            }
            let start = self.offset;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
            if start == self.offset {
                return Err(format!("{}:{exponent_column}: missing exponent digits", self.row));
            }
            // Parse.Number.chompExponentHelp stops at any non-digit without
            // the dirty-suffix check used for integers and fractions.
            return Ok(());
        }
        if self.peek().is_some_and(inner) {
            return Err(self.error("invalid number suffix"));
        }
        Ok(())
    }
}
fn inner(c: char) -> bool {
    crate::unicode::is_alpha(c) || c.is_ascii_digit() || c == '_'
}
fn operator(c: char) -> bool {
    "+-/*=.<>:&|^?%!".contains(c)
}

pub fn lex(source: &str) -> Result<Vec<Token>, String> {
    let (tokens, error) = lex_prefix(source);
    match error { Some(error) => Err(error), None => Ok(tokens) }
}

/// Position after the trailing layout of an already lexed source. Starting at
/// the last token preserves the distinction between CR in comments and layout
/// without scanning the entire file again when reporting an EOF error.
pub(crate) fn end_position(source: &str, tokens: &[Token]) -> (u32, u32) {
    let mut cursor = Cursor {
        source,
        offset: 0,
        row: 1,
        column: 1,
    };
    if let Some(token) = tokens.last() {
        cursor.offset = token.start as usize;
        cursor.row = token.row;
        cursor.column = token.column;
        while cursor.offset < token.end as usize {
            cursor.advance();
        }
    }
    // A lexical error is reported separately by the caller. On a prefix this
    // still yields the position where trailing layout stopped being valid.
    let _ = cursor.skip(&mut None);
    (cursor.row, cursor.column)
}

/// Expose the first character of a failed token for error selection only.
/// A recovery parser may reject its position in the grammar, but must retain
/// the lexical error if it consumes this synthetic token. No recovered AST
/// may be returned as a successfully parsed source.
pub(crate) fn failed_token_start(source: &str, tokens: &[Token]) -> Option<Token> {
    let mut cursor = Cursor { source, offset: 0, row: 1, column: 1 };
    if let Some(token) = tokens.last() {
        cursor.offset = token.start as usize;
        cursor.row = token.row;
        cursor.column = token.column;
        while cursor.offset < token.end as usize {
            cursor.advance();
        }
    }
    // Layout failures must retain the existing whitespace-priority rules.
    cursor.skip(&mut None).ok()?;
    let ch = cursor.peek()?;
    let kind = if ch.is_ascii_digit() { Kind::Number }
        else if ch == '"' { Kind::String }
        else if ch == '\'' { Kind::Char }
        else { Kind::Symbol };
    Some(Token {kind, start: cursor.offset as u32,
        end: (cursor.offset + ch.len_utf8()) as u32,
        row: cursor.row, column: cursor.column})
}

/// Opt-in collection keeps the ordinary compiler path free of comment storage.
pub fn lex_with_docs(source: &str) -> Result<(Vec<Token>, Vec<DocComment>), String> {
    let mut docs = Vec::new();
    let tokens = lex_inner(source, Some(&mut docs))?;
    Ok((tokens, docs))
}

fn lex_inner(source: &str, docs: Option<&mut Vec<DocComment>>) -> Result<Vec<Token>, String> {
    if source.len() > u32::MAX as usize {
        return Err("source exceeds 4 GiB".into());
    }
    let mut tokens = Vec::with_capacity(source.len() / 8);
    lex_into(source, docs, &mut tokens)?;
    Ok(tokens)
}

/// Discovery may use a completed header before a broken body. The failure is
/// retained; callers must never treat this prefix as a successfully lexed file.
pub fn lex_prefix(source: &str) -> (Vec<Token>, Option<String>) {
    crate::session_cache::tokens(source, || lex_prefix_uncached(source))
}

fn lex_prefix_uncached(source: &str) -> (Vec<Token>, Option<String>) {
    if source.len() > u32::MAX as usize {
        return (Vec::new(), Some("source exceeds 4 GiB".into()));
    }
    let mut tokens = Vec::with_capacity(source.len() / 8);
    let error = lex_into(source, None, &mut tokens).err();
    (tokens, error)
}

fn lex_into(
    source: &str,
    mut docs: Option<&mut Vec<DocComment>>,
    tokens: &mut Vec<Token>,
) -> Result<(), String> {
    if source.len() > u32::MAX as usize {
        return Err("source exceeds 4 GiB".into());
    }
    let mut c = Cursor {
        source,
        offset: 0,
        row: 1,
        column: 1,
    };
    loop {
        c.skip(&mut docs)?;
        let Some(ch) = c.peek() else {
            return Ok(());
        };
        let (start, row, column) = (c.offset as u32, c.row, c.column);
        let kind = if crate::unicode::is_lower(ch) || crate::unicode::is_upper(ch) {
            c.advance();
            while c.peek().is_some_and(inner) {
                c.advance();
            }
            if crate::unicode::is_upper(ch) {
                Kind::Upper
            } else {
                Kind::Lower
            }
        } else if ch.is_ascii_digit() {
            c.number()?;
            Kind::Number
        } else if c.take("\"\"\"") {
            c.quoted('"', true)?;
            Kind::String
        } else if c.take("\"") {
            c.quoted('"', false)?;
            Kind::String
        } else if c.take("'") {
            c.quoted('\'', false)?;
            Kind::Char
        } else if c.take("[glsl|") {
            while !c.take("|]") {
                if c.advance().is_none() {
                    return Err(c.error("unterminated shader"));
                }
            }
            crate::shader::Shader::parse(&source[start as usize..c.offset])
                .map_err(|error| format!("{row}:{column}: {error}"))?;
            Kind::Shader
        } else if operator(ch) {
            c.advance();
            while c.peek().is_some_and(operator) {
                c.advance();
            }
            Kind::Symbol
        } else if ch == '_' {
            c.advance();
            // Keep the whole invalid name so the pattern parser can report its
            // context, extent and a useful replacement. It is never a name token.
            while c.peek().is_some_and(inner) {
                c.advance();
            }
            Kind::Symbol
        } else if "()[]{},\\".contains(ch) {
            c.advance();
            Kind::Symbol
        } else {
            return Err(c.error(&format!("unexpected character {ch:?}")));
        };
        tokens.push(Token {
            kind,
            start,
            end: c.offset as u32,
            row,
            column,
        });
    }
}
