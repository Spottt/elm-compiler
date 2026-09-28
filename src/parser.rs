//! Recursive-descent source parser with indentation boundaries. Binary chains
//! intentionally remain unresolved until imported operator fixities are known.
use crate::{
    ast::*,
    lexer::{DocComment, Kind, Token, lex, lex_with_docs},
    module::header,
};
#[derive(Clone, Copy)]
enum ExpressionContext<'a> { Definition(&'a str), Phrase(&'static str) }
struct Parser<'s> {
    source: &'s str,
    tokens: Vec<Token>,
    pos: usize,
    depth: usize,
    pattern_binding: bool,
    expression_context: Option<(usize, ExpressionContext<'s>)>,
    ast: Syntax<'s>,
}
pub(crate) fn reserved(s: &str) -> bool {
    matches!(
        s,
        "if" | "then"
            | "else"
            | "case"
            | "of"
            | "let"
            | "in"
            | "type"
            | "module"
            | "where"
            | "import"
            | "exposing"
            | "as"
            | "port"
    )
}
pub(crate) fn binop(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| "+-/*=.<>:&|^?%!".contains(c))
        && !matches!(s, "." | ".." | "|" | "->" | "=" | ":")
}
impl<'s> Parser<'s> {
    fn peek(&self) -> &'s str {
        self.tokens
            .get(self.pos)
            .map_or("", |t| t.text(self.source))
    }
    fn at(&self, offset: usize) -> &'s str {
        self.tokens
            .get(self.pos + offset)
            .map_or("", |t| t.text(self.source))
    }
    fn kind(&self) -> Option<Kind> {
        self.tokens.get(self.pos).map(|t| t.kind)
    }
    fn take(&mut self, s: &str) -> bool {
        if self.peek() == s {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn error(&self, message: &str) -> String {
        let (row, col) = self.tokens.get(self.pos).map_or_else(
            || crate::lexer::end_position(self.source, &self.tokens),
            |t| (t.row, t.column),
        );
        format!("{row}:{col}: {message}; found {:?}", self.peek())
    }
    fn expect(&mut self, s: &str) -> Result<(), String> {
        if self.take(s) {
            Ok(())
        } else {
            Err(self.error(&format!("expected {s:?}")))
        }
    }
    fn keep(&self, floor: u32) -> bool {
        self.tokens.get(self.pos).is_some_and(|t| t.column > floor)
    }
    fn require_indent(&self, floor: u32) -> Result<(), String> {
        let column = self.tokens.get(self.pos).map_or_else(
            || crate::lexer::end_position(self.source, &self.tokens).1,
            |token| token.column,
        );
        if column > floor {
            Ok(())
        } else {
            Err(self.error("expected an indented continuation"))
        }
    }
    fn control_indent(&self, floor: u32, start: usize, phase: &str) -> Result<(), String> {
        self.require_indent(floor).map_err(|_| {
            let previous = &self.tokens[self.pos - 1];
            let (row, column) = previous.text(self.source).chars().fold(
                (previous.row, previous.column),
                |(row, column), c| if c == '\n' { (row + 1, 1) } else { (row, column + 1) },
            );
            format!("{row}:{column}: {} indent-{phase} {}", self.tokens[start].text(self.source), self.tokens[start].row)
        })
    }
    fn definition_indent(&self, floor: u32, start: usize, phase: &str, local: bool) -> Result<(), String> {
        if !local { return self.require_indent(floor); }
        self.control_indent(floor, start, phase).map_err(|error| {
            let position = error.split_once(": ").map_or(error.as_str(), |(position, _)| position);
            format!("{position}: local-definition {phase} {} {}", self.tokens[start].row, self.tokens[start].text(self.source))
        })
    }
    fn destruct_indent(&self, floor: u32, start: usize, phase: &str) -> Result<(), String> {
        self.control_indent(floor, start, phase).map_err(|error| {
            let position = error.split_once(": ").map_or(error.as_str(), |(position, _)| position);
            format!("{position}: local-destruct indent-{phase} {}", self.tokens[start].row)
        })
    }
    fn span(&self, start: usize) -> Span {
        Span {
            start: self.tokens[start].start,
            end: self.tokens[self.pos - 1].end,
        }
    }
    fn e(&mut self, start: usize, kind: Expr<'s>) -> ExprId {
        let id = ExprId(self.ast.expressions.len() as u32);
        self.ast.expressions.push(Node {
            span: self.span(start),
            kind,
        });
        id
    }
    fn p(&mut self, start: usize, kind: Pattern<'s>) -> PatternId {
        let id = PatternId(self.ast.patterns.len() as u32);
        self.ast.patterns.push(Node {
            span: self.span(start),
            kind,
        });
        id
    }
    fn t(&mut self, start: usize, kind: Type<'s>) -> TypeId {
        let id = TypeId(self.ast.types.len() as u32);
        self.ast.types.push(Node {
            span: self.span(start),
            kind,
        });
        id
    }
    fn lower(&mut self) -> Result<&'s str, String> {
        if self.kind() != Some(Kind::Lower) || reserved(self.peek()) {
            return Err(self.error("expected a lowercase name"));
        }
        let name = self.peek();
        self.pos += 1;
        Ok(name)
    }
    fn upper(&mut self) -> Result<&'s str, String> {
        if self.kind() != Some(Kind::Upper) {
            return Err(self.error("expected an uppercase name"));
        }
        let name = self.peek();
        self.pos += 1;
        Ok(name)
    }
    fn adjacent(&self, left: usize, right: usize) -> bool {
        self.tokens
            .get(left)
            .zip(self.tokens.get(right))
            .is_some_and(|(a, b)| a.end == b.start)
    }
    fn qualified(&mut self, allow_lower: bool) -> Result<&'s str, String> {
        let start = self.pos;
        self.upper()?;
        while self.peek() == "."
            && self.adjacent(self.pos - 1, self.pos)
            && self.adjacent(self.pos, self.pos + 1)
        {
            let kind = self.tokens.get(self.pos + 1).map(|t| t.kind);
            if kind == Some(Kind::Upper) {
                self.pos += 2;
            } else if allow_lower && kind == Some(Kind::Lower) {
                self.pos += 2;
                break;
            } else {
                break;
            }
        }
        let span = self.span(start);
        Ok(&self.source[span.start as usize..span.end as usize])
    }
    fn type_start(&self) -> bool {
        matches!(self.kind(), Some(Kind::Upper))
            || (self.kind() == Some(Kind::Lower) && !reserved(self.peek()))
            || matches!(self.peek(), "(" | "{")
    }
    fn ty(&mut self, floor: u32) -> Result<TypeId, String> {
        if self.depth >= 512 {
            return Err(self.error("syntax nesting exceeds 512 levels"));
        }
        self.depth += 1;
        let result = self.ty_inner(floor);
        self.depth -= 1;
        result
    }
    fn ty_inner(&mut self, floor: u32) -> Result<TypeId, String> {
        self.require_indent(floor)?;
        let start = self.pos;
        let left = self.type_app(floor)?;
        if self.keep(floor) && self.take("->") {
            let right = self.ty(floor)?;
            Ok(self.t(start, Type::Function(left, right)))
        } else {
            Ok(left)
        }
    }
    fn type_app(&mut self, floor: u32) -> Result<TypeId, String> {
        if self.kind() == Some(Kind::Upper) {
            let start = self.pos;
            let name = self.qualified(false)?;
            let mut args = Vec::new();
            while self.keep(floor) && self.type_start() {
                args.push(self.type_atom(floor)?);
            }
            Ok(self.t(start, Type::Constructor(name, args)))
        } else {
            self.type_atom(floor)
        }
    }
    fn type_atom(&mut self, floor: u32) -> Result<TypeId, String> {
        let start = self.pos;
        let kind = if self.kind() == Some(Kind::Upper) {
            Type::Constructor(self.qualified(false)?, Vec::new())
        } else if self.kind() == Some(Kind::Lower) {
            Type::Var(self.lower()?)
        } else if self.take("(") {
            // Unlike unit patterns/expressions, the unit type is a literal ().
            if self.peek() == ")" && self.adjacent(self.pos - 1, self.pos) {
                self.pos += 1;
                Type::Unit
            } else {
                let first = self.ty(floor)?;
                self.require_indent(floor)?;
                if self.take(",") {
                    let mut parts = vec![first, self.ty(floor)?];
                    self.require_indent(floor)?;
                    while self.take(",") {
                        parts.push(self.ty(floor)?);
                        self.require_indent(floor)?;
                    }
                    self.expect(")")?;
                    Type::Tuple(parts)
                } else {
                    self.expect(")")?;
                    return Ok(first);
                }
            }
        } else if self.take("{") {
            self.require_indent(floor)?;
            let mut fields = Vec::new();
            let mut extension = None;
            if !self.take("}") {
                let mut name = self.lower()?;
                self.require_indent(floor)?;
                if self.take("|") {
                    extension = Some(name);
                    self.require_indent(floor)?;
                    name = self.lower()?;
                    self.require_indent(floor)?;
                }
                loop {
                    self.expect(":")?;
                    fields.push((name, self.ty(floor)?));
                    self.require_indent(floor)?;
                    if !self.take(",") {
                        break;
                    }
                    self.require_indent(floor)?;
                    name = self.lower()?;
                    self.require_indent(floor)?;
                }
                self.expect("}")?;
            }
            Type::Record { extension, fields }
        } else {
            return Err(self.error("expected type"));
        };
        Ok(self.t(start, kind))
    }
    fn pattern_start(&self) -> bool {
        matches!(
            self.kind(),
            Some(Kind::Upper | Kind::Number | Kind::String | Kind::Char)
        ) || (self.kind() == Some(Kind::Lower) && !reserved(self.peek()))
            || self.peek().starts_with('_')
            || matches!(self.peek(), "(" | "[" | "{")
    }
    fn pattern(&mut self, floor: u32) -> Result<PatternId, String> {
        if self.depth >= 512 {
            return Err(self.error("syntax nesting exceeds 512 levels"));
        }
        self.depth += 1;
        let result = self.pattern_inner(floor);
        self.depth -= 1;
        result
    }
    fn binding_pattern(&mut self, floor: u32) -> Result<PatternId, String> {
        let previous = self.pattern_binding;
        self.pattern_binding = true;
        let result = self.pattern(floor);
        self.pattern_binding = previous;
        result
    }
    fn binding_term(&mut self, floor: u32) -> Result<PatternId, String> {
        if !self.pattern_start() {
            return Err(self.error("pattern start binding"));
        }
        let previous = self.pattern_binding;
        self.pattern_binding = true;
        let result = self.pattern_atom(floor);
        self.pattern_binding = previous;
        result
    }
    fn pattern_inner(&mut self, floor: u32) -> Result<PatternId, String> {
        let start = self.pos;
        let mut parts = Vec::new();
        loop {
            if !self.pattern_start() {
                return Err(self.error(if self.pattern_binding {
                    "pattern start binding"
                } else {
                    "pattern start argument"
                }));
            }
            let item_start = self.pos;
            let part = if self.kind() == Some(Kind::Upper) {
                let name = self.qualified(false)?;
                let mut args = Vec::new();
                while self.keep(floor) && self.pattern_start() {
                    args.push(self.pattern_atom(floor)?);
                }
                self.p(item_start, Pattern::Constructor(name, args))
            } else {
                self.pattern_atom(floor)?
            };
            parts.push(part);
            if !(self.keep(floor) && self.take("::")) {
                break;
            }
            self.pattern_indent(floor, start, "start", "indent")?;
        }
        let mut result = parts.pop().unwrap();
        while let Some(head) = parts.pop() {
            let id = PatternId(self.ast.patterns.len() as u32);
            let span = Span {
                start: self.ast.patterns[head.0 as usize].span.start,
                end: self.ast.patterns[result.0 as usize].span.end,
            };
            self.ast.patterns.push(Node {
                span,
                kind: Pattern::Cons(head, result),
            });
            result = id;
        }
        if self.keep(floor) && self.take("as") {
            self.pattern_indent(floor, start, "alias", "indent")?;
            let name = self.lower().map_err(|_| self.error("pattern alias name"))?;
            result = self.p(start, Pattern::Alias(result, name));
        }
        Ok(result)
    }
    fn pattern_indent(
        &self,
        floor: u32,
        start: usize,
        family: &str,
        phase: &str,
    ) -> Result<(), String> {
        let column = self.tokens.get(self.pos).map_or_else(
            || crate::lexer::end_position(self.source, &self.tokens).1,
            |t| t.column,
        );
        if column > floor.max(1) {
            return Ok(());
        }
        let previous = &self.tokens[self.pos - 1];
        let (row, col) = previous.text(self.source).chars().fold(
            (previous.row, previous.column),
            |(row, col), c| {
                if c == '\n' {
                    (row + 1, 1)
                } else {
                    (row, col + 1)
                }
            },
        );
        Err(format!(
            "{row}:{col}: pattern {family} {phase} {}",
            self.tokens[start].row
        ))
    }
    fn pattern_atom(&mut self, floor: u32) -> Result<PatternId, String> {
        let start = self.pos;
        let kind = match self.kind() {
            Some(Kind::Lower) => Pattern::Var(self.lower()?),
            Some(Kind::Upper) => Pattern::Constructor(self.qualified(false)?, Vec::new()),
            Some(k @ (Kind::Number | Kind::String | Kind::Char)) => {
                let text = self.peek();
                if k == Kind::Number && !text.starts_with("0x") && text.contains(['.', 'e', 'E']) {
                    return Err(self.error(&format!("pattern float {}", text.len())));
                }
                self.pos += 1;
                Pattern::Literal(k, text)
            }
            _ if self.peek().starts_with('_') && self.peek() != "_" => {
                return Err(self.error("pattern wildcard name"));
            }
            _ if self.take("_") => Pattern::Wildcard,
            _ if self.take("(") => {
                self.pattern_indent(floor, start, "parentheses", "indent-open")?;
                if self.take(")") {
                    Pattern::Unit
                } else {
                    if !self.pattern_start() {
                        return Err(self.error(&format!(
                            "pattern parentheses open {}",
                            self.tokens[start].row
                        )));
                    }
                    let first = self.pattern(floor)?;
                    self.pattern_indent(floor, start, "parentheses", "indent-end")?;
                    if self.take(",") {
                        self.pattern_indent(floor, start, "parentheses", "indent-next")?;
                        let mut parts = vec![first, self.pattern(floor)?];
                        self.pattern_indent(floor, start, "parentheses", "indent-end")?;
                        while self.take(",") {
                            self.pattern_indent(floor, start, "parentheses", "indent-next")?;
                            parts.push(self.pattern(floor)?);
                            self.pattern_indent(floor, start, "parentheses", "indent-end")?;
                        }
                        if !self.take(")") {
                            return Err(self.error(&format!(
                                "pattern parentheses end {}",
                                self.tokens[start].row
                            )));
                        }
                        Pattern::Tuple(parts)
                    } else {
                        if !self.take(")") {
                            return Err(self.error(&format!(
                                "pattern parentheses end {}",
                                self.tokens[start].row
                            )));
                        }
                        return Ok(first);
                    }
                }
            }
            _ if self.take("[") => {
                self.pattern_indent(floor, start, "list", "indent-open")?;
                let mut items = Vec::new();
                if !self.take("]") {
                    if !self.pattern_start() {
                        return Err(
                            self.error(&format!("pattern list open {}", self.tokens[start].row))
                        );
                    }
                    loop {
                        items.push(self.pattern(floor)?);
                        self.pattern_indent(floor, start, "list", "indent-end")?;
                        if !self.take(",") {
                            break;
                        }
                        self.pattern_indent(floor, start, "list", "indent-next")?;
                    }
                    if !self.take("]") {
                        return Err(
                            self.error(&format!("pattern list end {}", self.tokens[start].row))
                        );
                    }
                }
                Pattern::List(items)
            }
            _ if self.take("{") => {
                self.pattern_indent(floor, start, "record", "indent-open")?;
                let mut names = Vec::new();
                if !self.take("}") {
                    let mut phase = "open";
                    loop {
                        names.push(self.lower().map_err(|_| {
                            self.error(&format!(
                                "pattern record {phase} {}",
                                self.tokens[start].row
                            ))
                        })?);
                        self.pattern_indent(floor, start, "record", "indent-end")?;
                        if !self.take(",") {
                            break;
                        }
                        phase = "field";
                        self.pattern_indent(floor, start, "record", "indent-field")?;
                    }
                    if !self.take("}") {
                        return Err(
                            self.error(&format!("pattern record end {}", self.tokens[start].row))
                        );
                    }
                }
                Pattern::Record(names)
            }
            _ => return Err(self.error("expected pattern")),
        };
        Ok(self.p(start, kind))
    }
    fn negative_argument(&self) -> bool {
        self.peek() == "-"
            && self.pos > 0
            && !self.adjacent(self.pos - 1, self.pos)
            && self.adjacent(self.pos, self.pos + 1)
    }
    fn atom_start(&self) -> bool {
        matches!(
            self.kind(),
            Some(Kind::Upper | Kind::Number | Kind::String | Kind::Char | Kind::Shader)
        ) || (self.kind() == Some(Kind::Lower) && !reserved(self.peek()))
            || matches!(self.peek(), "(" | "[" | "{" | ".")
            || self.negative_argument()
    }
    fn expr(&mut self, floor: u32) -> Result<ExprId, String> {
        if self.depth >= 512 {
            return Err(self.error("syntax nesting exceeds 512 levels"));
        }
        self.depth += 1;
        let result = self.expr_inner(floor);
        self.depth -= 1;
        result
    }
    fn expr_context(&mut self, floor: u32, context_start: usize, context: ExpressionContext<'s>) -> Result<ExprId, String> {
        let before = self.pos;
        let previous = self.expression_context.replace((context_start, context));
        let result = self.expr(floor);
        self.expression_context = previous;
        result.map_err(|error| {
            let missing = error.contains(": expected expression;") || error.contains(": expected a lowercase name;");
            let negative_gap = error.split_once(": negative expression missing").map(|(position, _)| position);
            if (missing && self.pos == before) || negative_gap.is_some() {
                let thing = match context {
                    ExpressionContext::Definition(name) => format!("the `{name}` definition"),
                    ExpressionContext::Phrase(phrase) => phrase.to_string(),
                };
                let kind = format!("expression-start {} {thing}", self.tokens[context_start].row);
                if let Some(position) = negative_gap {
                    format!("{position}: {kind}")
                } else if self.minus_before_invalid_symbol() {
                    // Elm first consumes unary minus, then fails at the next symbol.
                    let token = &self.tokens[self.pos];
                    format!("{}:{}: {kind}", token.row, token.column + 1)
                } else { self.error(&kind) }
            } else { error }
        })
    }
    fn minus_before_invalid_symbol(&self) -> bool {
        let token = self.peek();
        token.len() > 1 && token.starts_with('-') && !token.starts_with("-.")
    }
    fn control_start(&self) -> bool {
        matches!(self.peek(), "if" | "let" | "case" | "\\")
    }
    fn expr_inner(&mut self, floor: u32) -> Result<ExprId, String> {
        self.require_indent(floor)?;
        let start = self.pos;
        let final_term = self.control_start();
        let first = self.application(floor)?;
        if final_term {
            return Ok(first);
        }
        let mut rest = Vec::new();
        while self.keep(floor) && binop(self.peek()) {
            let op = self.peek();
            self.pos += 1;
            self.require_indent(floor)?;
            let final_term = self.control_start();
            rest.push((op, self.application(floor)?));
            // Parse.Expression returns immediately after a control expression
            // on the right of an operator. Parentheses make it a regular term.
            if final_term {
                break;
            }
        }
        if rest.is_empty() {
            Ok(first)
        } else {
            Ok(self.e(start, Expr::Binops(first, rest)))
        }
    }
    fn if_expression(&mut self, floor: u32, diagnostic_start: usize) -> Result<ExprId, String> {
        let start = self.pos;
        self.expect("if")?;
            self.control_indent(floor, diagnostic_start, "condition")?;
            let condition = self.expr_context(floor, diagnostic_start, ExpressionContext::Phrase("an `if` expression"))?;
            self.control_indent(floor, diagnostic_start, "then")?;
            self.expect("then").map_err(|_| self.error(&format!("if then {}", self.tokens[diagnostic_start].row)))?;
            self.control_indent(floor, diagnostic_start, "then-branch")?;
            let yes = self.expr_context(floor, diagnostic_start, ExpressionContext::Phrase("an `if` expression"))?;
            self.control_indent(floor, diagnostic_start, "else")?;
            self.expect("else").map_err(|_| self.error(&format!("if else {}", self.tokens[diagnostic_start].row)))?;
            self.control_indent(floor, diagnostic_start, "else-branch")?;
            let else_start = self.pos;
            let no = if self.peek() == "if" {
                if self.depth >= 512 {
                    return Err(self.error("syntax nesting exceeds 512 levels"));
                }
                self.depth += 1;
                let result = self.if_expression(floor, diagnostic_start);
                self.depth -= 1;
                result
            } else {
                self.expr_context(floor, diagnostic_start, ExpressionContext::Phrase("an `if` expression"))
            }.map_err(|error| {
                if self.pos == else_start && !self.minus_before_invalid_symbol() {
                    self.error(&format!("if else-start {}", self.tokens[diagnostic_start].row))
                } else {
                    error
                }
            })?;
            Ok(self.e(start, Expr::If(condition, yes, no)))
    }
    fn application(&mut self, floor: u32) -> Result<ExprId, String> {
        let start = self.pos;
        if self.peek() == "if" {
            return self.if_expression(floor, start);
        }
        if self.take("\\") {
            self.control_indent(floor, start, "argument")?;
            let mut args = Vec::new();
            while self.keep(floor) && self.pattern_start() {
                args.push(self.pattern_atom(floor)?);
                self.control_indent(floor, start, "arrow")?;
            }
            if args.is_empty() {
                return Err(self.error("pattern start argument"));
            }
            self.require_indent(floor)?;
            if !self.take("->") {
                return Err(self.error(&format!("\\ arrow {}", self.tokens[start].row)));
            }
            self.control_indent(floor, start, "body")?;
            let body = self.expr_context(floor, start, ExpressionContext::Phrase("an anonymous function"))?;
            return Ok(self.e(start, Expr::Lambda(args, body)));
        }
        if self.take("let") {
            self.control_indent(self.tokens[start].column, start, "definition")?;
            let indent = self.tokens.get(self.pos).map_or_else(
                || crate::lexer::end_position(self.source, &self.tokens).1,
                |token| token.column,
            );
            let mut declarations = Vec::new();
            if !self.pattern_start() {
                return Err(self.error(&format!("let name {}", self.tokens[start].row)));
            }
            while self.peek() != "in" {
                if self.tokens.get(self.pos).is_none_or(|t| t.column != indent) {
                    if !declarations.is_empty() {
                        break;
                    }
                    return Err(self.error("let declarations must align"));
                }
                declarations.push(self.declaration(true, false)?);
            }
            if declarations.is_empty() {
                return Err(self.error("empty let"));
            }
            validate_annotations(&declarations)?;
            self.control_indent(floor, start, "in")?;
            if !self.take("in") {
                return Err(self.error(&format!("let in {}", self.tokens[start].row)));
            }
            self.control_indent(floor, start, "body")?;
            let body = match self.expression_context {
                Some((context_start, context)) => self.expr_context(floor, context_start, context)?,
                None => self.expr(floor)?,
            };
            return Ok(self.e(start, Expr::Let(declarations, body)));
        }
        if self.take("case") {
            self.control_indent(floor, start, "expr")?;
            let subject = self.expr_context(floor, start, ExpressionContext::Phrase("a `case` expression"))?;
            self.control_indent(floor, start, "of")?;
            self.expect("of").map_err(|_| self.error(&format!("case of {}", self.tokens[start].row)))?;
            self.control_indent(floor, start, "pattern")?;
            let indent = self.tokens.get(self.pos).map_or_else(
                || crate::lexer::end_position(self.source, &self.tokens).1,
                |token| token.column,
            );
            let mut branches = Vec::new();
            loop {
                let pattern = self.binding_pattern(indent)?;
                self.control_indent(indent, start, "arrow")?;
                self.expect("->").map_err(|_| {
                    self.error(&format!("case arrow {}", self.tokens[start].row))
                })?;
                self.control_indent(indent, start, "branch")?;
                let body = self.expr_context(indent, start, ExpressionContext::Phrase("a `case` expression"))?;
                branches.push((pattern, body));
                if self.tokens.get(self.pos).is_none_or(|t| t.column != indent)
                    || !self.pattern_start()
                {
                    break;
                }
            }
            return Ok(self.e(start, Expr::Case(subject, branches)));
        }
        let first = self.atom(floor)?;
        let mut args = Vec::new();
        while self.keep(floor) && self.atom_start() {
            args.push(self.atom(floor)?);
        }
        if args.is_empty() {
            Ok(first)
        } else {
            // Elm merges the function and last argument's expression regions;
            // grouping parentheses are consumed but do not extend that region.
            let span = Span {
                start: self.ast.expressions[first.0 as usize].span.start,
                end: self.ast.expressions[args.last().unwrap().0 as usize].span.end,
            };
            let call = self.e(start, Expr::Call(first, args));
            self.ast.expressions[call.0 as usize].span = span;
            Ok(call)
        }
    }
    fn atom(&mut self, floor: u32) -> Result<ExprId, String> {
        let start = self.pos;
        let kind = match self.kind() {
            Some(Kind::Lower) => Expr::Var(self.lower()?),
            Some(Kind::Upper) => Expr::Var(self.qualified(true)?),
            Some(k @ (Kind::Number | Kind::String | Kind::Char | Kind::Shader)) => {
                let text = self.peek();
                self.pos += 1;
                Expr::Literal(k, text)
            }
            _ if self.peek() == "-." => {
                // In term position Elm reads unary `-` followed by accessor `.`.
                // Keep the combined token for operator syntax elsewhere and restore
                // its span after parsing so enclosing nodes and recovery retain it.
                let original = self.tokens[self.pos];
                self.tokens[self.pos].start += 1;
                self.tokens[self.pos].column += 1;
                let inner = self.atom(floor);
                self.tokens[start] = original;
                Expr::Negate(inner?)
            }
            _ if self.take("-") => {
                if !self.adjacent(self.pos - 1, self.pos) || self.peek() == "-" {
                    let minus = &self.tokens[self.pos - 1];
                    return Err(format!("{}:{}: negative expression missing", minus.row, minus.column + 1));
                }
                let inner = self.atom(floor)?;
                Expr::Negate(inner)
            }
            _ if self.take(".") => {
                let name = self.access_field()?;
                Expr::Accessor(name)
            }
            _ if self.take("(") => {
                self.control_indent(floor, start, "open")?;
                // Parse.Expression compares positions before/after whitespace:
                // a bare CR does not advance its column, unlike spaces/comments.
                let position = self.tokens.get(self.pos).map_or_else(
                    || crate::lexer::end_position(self.source, &self.tokens),
                    |token| (token.row, token.column),
                );
                let literal = position == (self.tokens[start].row, self.tokens[start].column + 1);
                if literal && self.pos == self.tokens.len() {
                    return Err(self.error(&format!("( indent-open {}", self.tokens[start].row)));
                }
                if literal && matches!(self.peek(), "|" | "->" | "=" | ":") {
                    let phase = match self.peek() { "|" => "pipe", "->" => "arrow", "=" => "equals", _ => "colon" };
                    return Err(self.error(&format!("( reserved-{phase} {}", self.tokens[start].row)));
                }
                if literal && self.take(")") {
                    Expr::Unit
                } else if literal
                    && (binop(self.peek()) || self.peek() == "..")
                    && self.at(1) == ")"
                    && self.adjacent(self.pos, self.pos + 1)
                {
                    let op = self.peek();
                    self.pos += 2;
                    Expr::Operator(op)
                } else if literal && self.peek() != "-" && (binop(self.peek()) || self.peek() == "..") {
                    let op = &self.tokens[self.pos];
                    return Err(format!("{}:{}: ( operator-close {}", op.row, op.column + op.text(self.source).chars().count() as u32, self.tokens[start].row));
                } else {
                    let expression_start = self.pos;
                    let first = self.expr_context(floor, start, ExpressionContext::Phrase("some parentheses")).map_err(|error| {
                        // Elm tries an operator close before a negated term.
                        // Only a term that consumed nothing falls back to that error.
                        if literal && self.tokens[expression_start].text(self.source) == "-"
                            && self.pos == expression_start + 1 {
                            let minus = &self.tokens[expression_start];
                            format!("{}:{}: ( operator-close {}", minus.row, minus.column + 1, self.tokens[start].row)
                        } else {
                            error
                        }
                    })?;
                    self.control_indent(floor, start, "end")?;
                    if self.take(",") {
                        self.control_indent(floor, start, "next")?;
                        let mut parts = vec![first, self.expr_context(floor, start, ExpressionContext::Phrase("some parentheses"))?];
                        self.control_indent(floor, start, "end")?;
                        while self.take(",") {
                            self.control_indent(floor, start, "next")?;
                            parts.push(self.expr_context(floor, start, ExpressionContext::Phrase("some parentheses"))?);
                            self.control_indent(floor, start, "end")?;
                        }
                        self.expect(")").map_err(|_| self.error(&format!("( close {}", self.tokens[start].row)))?;
                        Expr::Tuple(parts)
                    } else {
                        self.expect(")").map_err(|_| self.error(&format!("( close {}", self.tokens[start].row)))?;
                        return self.access(start, first);
                    }
                }
            }
            _ if self.take("[") => {
                self.control_indent(floor, start, "open")?;
                let mut items = Vec::new();
                if !self.take("]") {
                    loop {
                        let entry_start = self.pos;
                        let entry = self.expr_context(floor, start, ExpressionContext::Phrase("a list")).map_err(|error| {
                            if self.pos == entry_start + 1 && self.tokens[entry_start].text(self.source) == "-" && error.contains(": expression-start ") {
                                let position = error.split_once(": ").map_or(error.as_str(), |(position, _)| position);
                                format!("{position}: [ entry {}", self.tokens[start].row)
                            } else if self.pos == entry_start && self.minus_before_invalid_symbol() {
                                let token = &self.tokens[self.pos];
                                format!("{}:{}: [ entry {}", token.row, token.column + 1, self.tokens[start].row)
                            } else if self.pos == entry_start {
                                self.error(&format!("[ {} {}", if items.is_empty() { "open" } else { "entry" }, self.tokens[start].row))
                            } else { error }
                        })?;
                        items.push(entry);
                        self.control_indent(floor, start, "end")?;
                        if !self.take(",") {
                            break;
                        }
                        self.control_indent(floor, start, "next")?;
                    }
                    self.expect("]").map_err(|_| self.error(&format!("[ close {}", self.tokens[start].row)))?;
                }
                Expr::List(items)
            }
            _ if self.take("{") => {
                self.control_indent(floor, start, "open")?;
                let mut fields = Vec::new();
                let mut base = None;
                if !self.take("}") {
                    let mut name = self.lower().map_err(|_| self.error(&format!("{{ open-token {}", self.tokens[start].row)))?;
                    self.control_indent(floor, start, "equals")?;
                    if self.take("|") {
                        base = Some(name);
                        self.control_indent(floor, start, "field")?;
                        name = self.lower().map_err(|_| self.error(&format!("{{ field-token {}", self.tokens[start].row)))?;
                        self.control_indent(floor, start, "equals")?;
                    }
                    loop {
                        self.expect("=").map_err(|_| self.error(&format!("{{ equals-token {}", self.tokens[start].row)))?;
                        self.control_indent(floor, start, "expr")?;
                        fields.push((name, self.expr_context(floor, start, ExpressionContext::Phrase("a record"))?));
                        self.control_indent(floor, start, "end")?;
                        if !self.take(",") {
                            break;
                        }
                        self.control_indent(floor, start, "field")?;
                        name = self.lower().map_err(|_| self.error(&format!("{{ field-token {}", self.tokens[start].row)))?;
                        self.control_indent(floor, start, "equals")?;
                    }
                    self.expect("}").map_err(|_| self.error(&format!("{{ close-token {}", self.tokens[start].row)))?;
                }
                Expr::Record { base, fields }
            }
            _ => return Err(self.error("expected expression")),
        };
        let accessible = matches!(kind, Expr::Var(_) | Expr::Record { .. } | Expr::Tuple(_) | Expr::Unit);
        let expr = self.e(start, kind);
        if accessible { self.access(start, expr) } else { Ok(expr) }
    }
    fn access_field(&mut self) -> Result<&'s str, String> {
        let dot = &self.tokens[self.pos - 1];
        let error = format!("{}:{}: record accessor", dot.row, dot.column + 1);
        if !self.adjacent(self.pos - 1, self.pos) {
            return Err(error);
        }
        self.lower().map_err(|_| error)
    }
    fn access(&mut self, start: usize, mut expr: ExprId) -> Result<ExprId, String> {
        while self.peek() == "."
            && self.adjacent(self.pos - 1, self.pos)
        {
            self.pos += 1;
            let field = self.access_field()?;
            expr = self.e(start, Expr::Access(expr, field));
        }
        Ok(expr)
    }
    fn infix_error(&self) -> String {
        let (row, col) = self.tokens.get(self.pos).map_or_else(
            || crate::lexer::end_position(self.source, &self.tokens),
            |t| (t.row, t.column),
        );
        format!("{row}:{col}: bad infix")
    }
    fn infix_previous_end(&self) -> (u32, u32) {
        let previous = &self.tokens[self.pos - 1];
        (
            previous.row,
            previous.column + previous.text(self.source).chars().count() as u32,
        )
    }
    fn infix_indent(&self) -> Result<(), String> {
        let column = self.tokens.get(self.pos).map_or_else(
            || crate::lexer::end_position(self.source, &self.tokens).1,
            |t| t.column,
        );
        if column > 1 {
            Ok(())
        } else {
            let (row, col) = self.infix_previous_end();
            Err(format!("{row}:{col}: bad infix"))
        }
    }
    fn infix_adjacent(&self) -> Result<(), String> {
        if self
            .tokens
            .get(self.pos)
            .is_some_and(|next| next.start == self.tokens[self.pos - 1].end)
        {
            Ok(())
        } else {
            let (row, col) = self.infix_previous_end();
            Err(format!("{row}:{col}: bad infix"))
        }
    }
    fn infix_expect(&mut self, expected: &str) -> Result<(), String> {
        if self.take(expected) {
            Ok(())
        } else {
            Err(self.infix_error())
        }
    }
    fn infix_declaration(&mut self) -> Result<Declaration<'s>, String> {
        self.infix_indent()?;
        let associativity = self.peek();
        if !matches!(associativity, "left" | "right" | "non") {
            return Err(self.infix_error());
        }
        self.pos += 1;
        self.infix_indent()?;
        let number = self.peek();
        let Some(digit) = number.as_bytes().first().filter(|c| c.is_ascii_digit()) else {
            return Err(self.infix_error());
        };
        // Parse.Number.precedence consumes exactly one decimal digit.
        if number.len() != 1 {
            let token = &self.tokens[self.pos];
            return Err(format!("{}:{}: bad infix", token.row, token.column + 1));
        }
        let precedence = digit - b'0';
        self.pos += 1;
        self.infix_indent()?;
        self.infix_expect("(")?;
        self.infix_adjacent()?;
        let operator = self.peek();
        if operator != ".." && !binop(operator) {
            return Err(self.infix_error());
        }
        self.pos += 1;
        self.infix_adjacent()?;
        self.infix_expect(")")?;
        self.infix_indent()?;
        self.infix_expect("=")?;
        self.infix_indent()?;
        let function = self.lower().map_err(|_| self.infix_error())?;
        let column = self.tokens.get(self.pos).map_or_else(
            || crate::lexer::end_position(self.source, &self.tokens).1,
            |t| t.column,
        );
        if column != 1 {
            return Err(self.infix_error());
        }
        Ok(Declaration::Infix {
            associativity,
            precedence,
            operator,
            function,
        })
    }
    fn declaration(&mut self, local: bool, allow_infix: bool) -> Result<Declaration<'s>, String> {
        let floor = self
            .tokens
            .get(self.pos)
            .ok_or_else(|| self.error("expected declaration"))?
            .column;
        if !local && self.take("type") {
            self.require_indent(floor)?;
            let alias = self.take("alias");
            self.require_indent(floor)?;
            let name = self.upper()?;
            let mut parameters = Vec::new();
            while self.keep(floor) && self.kind() == Some(Kind::Lower) {
                parameters.push(self.lower()?);
            }
            self.require_indent(floor)?;
            self.expect("=")?;
            if alias {
                return Ok(Declaration::Alias {
                    name,
                    parameters,
                    ty: self.ty(floor)?,
                });
            }
            let mut variants = Vec::new();
            loop {
                self.require_indent(floor)?;
                let name = self.upper()?;
                let mut args = Vec::new();
                while self.keep(floor) && self.type_start() {
                    args.push(self.type_atom(floor)?);
                }
                variants.push((name, args));
                if !(self.keep(floor) && self.take("|")) {
                    break;
                }
            }
            return Ok(Declaration::Union {
                name,
                parameters,
                variants,
            });
        }
        if !local && self.take("port") {
            self.require_indent(floor)?;
            let name = self.lower()?;
            self.require_indent(floor)?;
            self.expect(":")?;
            return Ok(Declaration::Port {
                name,
                ty: self.ty(floor)?,
            });
        }
        if allow_infix && self.take("infix") {
            return self.infix_declaration();
        }
        if self.kind() == Some(Kind::Lower) {
            if !local && reserved(self.peek()) {
                return Err(self.error("declaration start"));
            }
            let start = self.pos;
            let name = self.lower()?;
            self.definition_indent(floor, start, "equals", local)?;
            if self.take(":") {
                if local { self.definition_indent(floor, start, "type", true)?; }
                let ty = self.ty(floor)?;
                if local {
                    let column = self.tokens.get(self.pos).map_or_else(|| crate::lexer::end_position(self.source, &self.tokens).1, |token| token.column);
                    let phase = if column != floor { Some(("alignment", floor.to_string())) }
                        else if self.kind() != Some(Kind::Lower) || reserved(self.peek()) { Some(("missing", String::new())) }
                        else if self.peek() != name { Some(("mismatch", self.peek().to_string())) }
                        else { None };
                    if let Some((phase, detail)) = phase {
                        return Err(self.error(&format!("local-annotation {phase} {} {name} {detail}", self.tokens[start].row)));
                    }
                }
                return Ok(Declaration::Annotation { name, ty });
            }
            let mut arguments = Vec::new();
            while self.keep(floor) && self.pattern_start() {
                arguments.push(self.pattern_atom(floor)?);
                if local { self.definition_indent(floor, start, "equals", true)?; }
            }
            self.definition_indent(floor, start, "equals", local)?;
            self.expect("=").map_err(|error| if local {
                self.error(&format!("local-equals {} {name}", self.tokens[start].row))
            } else { error })?;
            if local { self.definition_indent(floor, start, "body", true)?; }
            return Ok(Declaration::Value {
                name,
                arguments,
                body: self.expr_context(floor, start, ExpressionContext::Definition(name))?,
            });
        }
        if local {
            let start = self.pos;
            let pattern = self.binding_term(floor)?;
            self.destruct_indent(floor, start, "equals")?;
            self.expect("=").map_err(|_| self.error(&format!("local-destruct equals {}", self.tokens[start].row)))?;
            self.destruct_indent(floor, start, "body")?;
            return Ok(Declaration::Destruct {
                pattern,
                body: self.expr_context(floor, start, ExpressionContext::Phrase("a definition"))?,
            });
        }
        Err(self.error("declaration start"))
    }
}
pub fn parse(source: &str) -> Result<Syntax<'_>, String> {
    let result = lex(source).map_err(|error| lexical_error(source, error))
        .and_then(|tokens| parse_tokens(source, tokens, None, None, false));
    crate::session_cache::remember_syntax(source, result.as_ref().map(|_| ()).map_err(Clone::clone));
    result
}

/// Syntax-only error collection can reuse an earlier full parse of these bytes.
pub(crate) fn check_syntax(source: &str) -> Result<(), String> {
    crate::session_cache::syntax(source).unwrap_or_else(|| parse(source).map(|_| ()))
}

/// Parse a REPL candidate, allowing imports without a body and an annotation
/// whose definition will arrive on the next line. Compiled modules still use
/// `parse`, which enforces both invariants.
pub(crate) fn parse_repl_fragment(source: &str) -> Result<Syntax<'_>, String> {
    let tokens = lex(source).map_err(|error| lexical_error(source, error))?;
    parse_tokens(source, tokens, None, None, true)
}

/// Documentation is collected only on request; ordinary compilation keeps its
/// existing token-only path. Spans borrow the original source.
#[derive(Debug, Default)]
pub struct Documentation<'s> {
    pub overview: Option<DocComment>,
    pub declarations: std::collections::BTreeMap<&'s str, DocComment>,
}

pub fn parse_with_docs(source: &str) -> Result<(Syntax<'_>, Documentation<'_>), String> {
    let (tokens, comments) = lex_with_docs(source).map_err(|error| lexical_error(source, error))?;
    let mut spans = Vec::new();
    let ast = parse_tokens(source, tokens, Some(&mut spans), None, false)?;
    let docs = associate_docs(&ast, &comments, &spans)?;
    Ok((ast, docs))
}

fn lexical_error(source: &str, error: String) -> String {
    let (mut tokens, _) = crate::lexer::lex_prefix(source);
    let header_error = crate::module::prefer_header_error(source, &tokens, error.clone());
    if header_error != error {
        return header_error;
    }
    let boundary = tokens.len();
    let synthetic = crate::lexer::failed_token_start(source, &tokens);
    if let Some(token) = synthetic {
        tokens.push(token);
    }
    let mut failed_at = tokens.len();
    if let Err(syntax_error) = parse_tokens(source, tokens, None, Some(&mut failed_at), false)
        && (failed_at < boundary || (synthetic.is_some() && failed_at == boundary))
    {
        return syntax_error;
    }
    // Consumption of the placeholder means the grammar attempted to read the
    // malformed literal. Its real lexical error takes precedence in that case.
    error
}

fn parse_tokens<'s>(
    source: &'s str,
    tokens: Vec<Token>,
    mut spans: Option<&mut Vec<Span>>,
    error_cursor: Option<&mut usize>,
    repl_fragment: bool,
) -> Result<Syntax<'s>, String> {
    let h = header(source, &tokens)?;
    // The first span covers only the explicit module header, before imports.
    if let Some(spans) = spans.as_deref_mut() {
        let end = tokens[..h.body_start]
            .iter()
            .position(|token| token.text(source) == "import")
            .unwrap_or(h.body_start);
        spans.push(Span {
            start: 0,
            end: if h.explicit { tokens[end - 1].end } else { 0 },
        });
    }
    let mut p = Parser {
        source,
        tokens,
        pos: h.body_start,
        depth: 0,
        pattern_binding: false,
        expression_context: None,
        ast: Syntax {
            source,
            header: h,
            declarations: Vec::new(),
            expressions: Vec::new(),
            patterns: Vec::new(),
            types: Vec::new(),
        },
    };
    // The recovery caller tracks whether failure occurs before consuming the
    // incomplete lexical token. Never treat a recovered prefix as valid.
    let result = (|| {
    let mut infix_prefix = true;
    while p.pos < p.tokens.len() {
        if p.tokens[p.pos].column != 1 {
            return Err(p.error("top-level declaration must begin in column 1"));
        }
        let start = p.pos;
        let d = p.declaration(false, infix_prefix)?;
        infix_prefix &= matches!(d, Declaration::Infix { .. });
        if let Some(spans) = spans.as_deref_mut() {
            spans.push(p.span(start));
        }
        p.ast.declarations.push(d);
    }
    if infix_prefix && !repl_fragment {
        return Err(p.error("declaration start"));
    }
    if repl_fragment { Ok(()) } else { validate_annotations(&p.ast.declarations) }
    })();
    if result.is_err() && let Some(cursor) = error_cursor {
        *cursor = p.pos;
    }
    result?;
    Ok(p.ast)
}
fn validate_annotations(declarations: &[Declaration<'_>]) -> Result<(), String> {
    for (i, d) in declarations.iter().enumerate() {
        if let Declaration::Annotation { name, .. } = d
            && !matches!(declarations.get(i+1), Some(Declaration::Value {name: next, ..}) if next == name)
        {
            return Err(format!(
                "annotation for {name} must be followed by its definition"
            ));
        }
    }
    Ok(())
}

/// Match the positions at which Parse.Module and Parse.Declaration consume
/// doc comments. In particular an annotation and its definition form one
/// documented declaration; a comment between them is not a second document.
fn associate_docs<'s>(
    ast: &Syntax<'s>,
    comments: &[DocComment],
    spans: &[Span],
) -> Result<Documentation<'s>, String> {
    let mut docs = Documentation::default();
    let mut remaining = comments.iter().copied().peekable();
    let tokens = lex(ast.source)?;
    let header_end = spans[0].end;
    let next_token = tokens.iter().find(|token| token.start >= header_end);
    if ast.header.explicit
        && let Some(comment) = remaining.peek()
        && comment.start >= header_end
        && next_token.is_none_or(|token| comment.end <= token.start)
    {
        if comment.column != 1 {
            return Err(format!(
                "{}:{}: module documentation must begin in column 1",
                comment.row, comment.column
            ));
        }
        docs.overview = remaining.next();
    }
    for (i, decl) in ast.declarations.iter().enumerate() {
        let span = spans[i + 1];
        if let Some(comment) = remaining.peek().copied()
            && comment.start < span.start
        {
            let previous_end = if i == 0 {
                tokens
                    .get(ast.header.body_start.wrapping_sub(1))
                    .map_or(0, |token| token.end)
            } else {
                spans[i].end
            };
            let name = match decl {
                Declaration::Annotation { name, .. }
                | Declaration::Alias { name, .. }
                | Declaration::Union { name, .. }
                | Declaration::Port { name, .. } => Some(*name),
                Declaration::Value { name, .. }
                    if !matches!(
                        i.checked_sub(1)
                            .and_then(|index| ast.declarations.get(index)),
                        Some(Declaration::Annotation { .. })
                    ) =>
                {
                    Some(*name)
                }
                _ => None,
            };
            if comment.start < previous_end || comment.column != 1 || name.is_none() {
                return Err(format!(
                    "{}:{}: misplaced documentation comment",
                    comment.row, comment.column
                ));
            }
            docs.declarations.insert(name.unwrap(), comment);
            remaining.next();
            if remaining.peek().is_some_and(|next| next.start < span.end) {
                return Err("unexpected documentation comment in declaration".into());
            }
        }
        if remaining.peek().is_some_and(|next| next.start < span.end) {
            return Err("unexpected documentation comment in declaration".into());
        }
    }
    if let Some(comment) = remaining.next() {
        return Err(format!(
            "{}:{}: documentation comment must precede a declaration",
            comment.row, comment.column
        ));
    }
    Ok(docs)
}
