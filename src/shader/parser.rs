//! Recognition and interface extraction for language-glsl 0.3.0.
//! Port of Language/GLSL/Parser.hs by Vo Minh Thu; see LICENSE-GLSL.
//! Keep ordered alternatives and consumed-input failures: Elm exposes the
//! historical parser's behavior, not the validity rules of a modern GPU.
use super::{Shader, Type, is_legacy_reserved};
type R<T = ()> = Result<T, ()>;
#[derive(Clone, Copy)]
enum Qualifier {
    Attribute,
    Uniform,
    Varying,
    Other,
}
type Declaration = Option<(Qualifier, Option<Type>, Vec<String>)>;
const TYPES: &str = "void float int uint bool vec2 vec3 vec4 bvec2 bvec3 bvec4 ivec2 ivec3 ivec4 uvec2 uvec3 uvec4 mat2 mat3 mat4 mat2x2 mat2x3 mat2x4 mat3x2 mat3x3 mat3x4 mat4x2 mat4x3 mat4x4 sampler1D sampler2D sampler3D samplerCube sampler1DShadow sampler2DShadow samplerCubeShadow sampler1DArray sampler2DArray sampler1DArrayShadow sampler2DArrayShadow isampler1D isampler2D isampler3D isamplerCube isampler1DArray isampler2DArray usampler1D usampler2D usampler3D usamplerCube usampler1DArray usampler2DArray sampler2DRect sampler2DRectShadow isampler2DRect usampler2DRect samplerBuffer isamplerBuffer usamplerBuffer sampler2DMS isampler2DMS usampler2DMS sampler2DMSArray isampler2DMSArray usampler2DMSArray";
const KEYWORDS: &str = "attribute const uniform varying layout centroid flat smooth noperspective break continue do for while switch case default if else in out inout true false invariant discard return lowp mediump highp precision struct";
fn keyword(s: &str) -> bool {
    TYPES
        .split_whitespace()
        .chain(KEYWORDS.split_whitespace())
        .any(|w| w == s)
}
fn head(c: char) -> bool {
    crate::unicode::is_alpha(c) || c == '_'
}
fn tail(c: char) -> bool {
    crate::unicode::is_alphanumeric(c) || c == '_'
}
struct Parser<'a> {
    source: &'a str,
    pos: usize,
    furthest: usize,
    depth: usize,
}
pub(super) fn parse(source: &str) -> Result<Shader, String> {
    let mut p = Parser {
        source,
        pos: 0,
        furthest: 0,
        depth: 0,
    };
    let mut shader = Shader {
        source: source.replace('\r', ""),
        ..Shader::default()
    };
    let result = (|| {
        p.blank()?;
        let mut count = 0;
        while p.pos < source.len() {
            if let Some((q, Some(ty), names)) = p.external()?
                && names.len() == 1
            {
                let fields = match q {
                    Qualifier::Attribute => &mut shader.attributes,
                    Qualifier::Uniform => &mut shader.uniforms,
                    Qualifier::Varying => &mut shader.varyings,
                    Qualifier::Other => {
                        count += 1;
                        continue;
                    }
                };
                // Elm folds right; first declaration wins.
                fields.entry(names[0].clone()).or_insert(ty);
            }
            count += 1;
        }
        if count == 0 {
            return p.fail();
        }
        Ok(())
    })();
    result.map_err(|()| {
        let before = &source[..p.furthest];
        let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        format!("invalid GLSL: unexpected input at shader {line}:{column}")
    })?;
    Ok(shader)
}
impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.source[self.pos..]
    }
    fn fail<T>(&mut self) -> R<T> {
        self.furthest = self.furthest.max(self.pos);
        Err(())
    }
    fn attempt<T>(&mut self, f: impl FnOnce(&mut Self) -> R<T>) -> R<T> {
        let start = self.pos;
        let result = f(self);
        if result.is_err() {
            self.pos = start;
        }
        result
    }
    fn optional<T>(&mut self, f: impl FnOnce(&mut Self) -> R<T>) -> R<Option<T>> {
        let start = self.pos;
        match f(self) {
            Ok(value) => Ok(Some(value)),
            Err(()) if self.pos == start => Ok(None),
            Err(()) => Err(()),
        }
    }
    fn choice<T>(&mut self, alternatives: &[fn(&mut Self) -> R<T>]) -> R<T> {
        for f in alternatives {
            let start = self.pos;
            match f(self) {
                Ok(v) => return Ok(v),
                Err(()) if self.pos != start => return Err(()),
                _ => {}
            }
        }
        self.fail()
    }
    fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> R<T>) -> R<T> {
        if self.depth >= 256 {
            return self.fail();
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }
    fn blank(&mut self) -> R {
        loop {
            if self.rest().starts_with("//") {
                self.pos += self.rest().find('\n').unwrap_or(self.rest().len());
            } else if self.rest().starts_with("/*") {
                self.pos += 2;
                let Some(end) = self.rest().find("*/") else {
                    self.pos = self.source.len();
                    return self.fail();
                };
                self.pos += end + 2;
            } else if let Some(c) = self
                .rest()
                .chars()
                .next()
                .filter(|c| crate::unicode::is_space(*c))
            {
                self.pos += c.len_utf8();
            } else {
                return Ok(());
            }
        }
    }
    fn raw(&mut self, s: &str) -> R {
        if !self.rest().starts_with(s) {
            return self.fail();
        }
        self.pos += s.len();
        Ok(())
    }
    fn symbol(&mut self, s: &str) -> R {
        self.raw(s)?;
        self.blank()
    }
    fn word(&mut self, s: &str) -> R {
        if !self.rest().starts_with(s) || self.rest()[s.len()..].chars().next().is_some_and(tail) {
            return self.fail();
        }
        self.pos += s.len();
        self.blank()
    }
    fn one_word(&mut self, words: &str) -> R<&'a str> {
        for word in words.split_whitespace() {
            let start = self.pos;
            if self.word(word).is_ok() {
                return Ok(&self.source[start..start + word.len()]);
            }
            if self.pos != start {
                return Err(());
            }
        }
        self.fail()
    }
    fn identifier(&mut self) -> R<String> {
        if !self.rest().chars().next().is_some_and(head) {
            return self.fail();
        }
        let n = self
            .rest()
            .char_indices()
            .find(|(_, c)| !tail(*c))
            .map_or(self.rest().len(), |(n, _)| n);
        let word = &self.rest()[..n];
        self.pos += n;
        if keyword(word) || is_legacy_reserved(word) || word.contains("__") {
            return self.fail();
        }
        self.blank()?;
        Ok(word.to_owned())
    }
    fn list<T>(&mut self, f: fn(&mut Self) -> R<T>) -> R<Vec<T>> {
        let mut values = Vec::new();
        if let Some(first) = self.optional(f)? {
            values.push(first);
            while self.optional(|p| p.symbol(","))?.is_some() {
                values.push(f(self)?);
            }
        }
        Ok(values)
    }
    fn digits(&mut self, radix: u32) -> R {
        let start = self.pos;
        while self
            .rest()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii() && c.is_digit(radix))
        {
            self.pos += 1;
        }
        if self.pos == start {
            self.fail()
        } else {
            Ok(())
        }
    }
    fn suffix(&mut self, letters: &str) {
        if self
            .rest()
            .chars()
            .next()
            .is_some_and(|c| letters.contains(c))
        {
            self.pos += 1;
        }
    }
    fn exponent(&mut self) -> R {
        self.attempt(|p| {
            if !p.rest().starts_with(['e', 'E']) {
                return p.fail();
            }
            p.pos += 1;
            p.suffix("+-");
            p.digits(10)?;
            p.blank()
        })
    }
    fn integer(&mut self) -> R {
        self.choice(&[
            |p| {
                p.attempt(|p| {
                    p.raw("0")?;
                    if !p.rest().starts_with(['x', 'X']) {
                        return p.fail();
                    }
                    p.pos += 1;
                    p.digits(16)?;
                    p.suffix("uU");
                    p.blank()
                })
            },
            |p| {
                p.attempt(|p| {
                    p.raw("0")?;
                    p.digits(8)?;
                    p.suffix("uU");
                    p.blank()
                })
            },
            |p| {
                p.attempt(|p| {
                    p.raw("0")?;
                    p.digits(16)?;
                    p.blank()
                })?;
                p.fail()
            },
            |p| {
                p.attempt(|p| {
                    p.digits(10)?;
                    if p.rest().starts_with('.') {
                        return p.fail();
                    }
                    let start = p.pos;
                    if p.exponent().is_ok() {
                        p.pos = start;
                        return p.fail();
                    }
                    p.suffix("uU");
                    p.blank()
                })
            },
        ])
    }
    fn floating(&mut self) -> R {
        self.choice(&[
            |p| {
                p.attempt(|p| {
                    p.digits(10)?;
                    p.exponent()?;
                    p.suffix("fF");
                    p.blank()
                })
            },
            |p| {
                p.attempt(|p| {
                    p.digits(10)?;
                    p.raw(".")?;
                    let _ = p.optional(|p| p.digits(10))?;
                    p.optional(Self::exponent)?;
                    p.suffix("fF");
                    p.blank()
                })
            },
            |p| {
                p.attempt(|p| {
                    p.raw(".")?;
                    p.digits(10)?;
                    p.optional(Self::exponent)?;
                    p.suffix("fF");
                    p.blank()
                })
            },
        ])
    }
    fn primary(&mut self) -> R {
        self.choice(&[
            |p| p.attempt(Self::identifier).map(|_| ()),
            Self::integer,
            Self::floating,
            |p| p.word("true"),
            |p| p.word("false"),
            |p| {
                p.symbol("(")?;
                p.expression()?;
                p.symbol(")")
            },
        ])
    }
    fn function_call(&mut self) -> R {
        self.choice(&[
            |p| p.attempt(Self::identifier).map(|_| ()),
            |p| p.type_specifier().map(|_| ()),
        ])?;
        self.symbol("(")?;
        self.choice(&[|p| p.word("void"), |p| p.list(Self::assignment).map(|_| ())])?;
        self.symbol(")")
    }
    fn postfix(&mut self) -> R {
        self.choice(&[|p| p.attempt(Self::function_call), Self::primary])?;
        while self
            .optional(|p| {
                p.choice(&[
                    |p| {
                        p.symbol("[")?;
                        p.expression()?;
                        p.symbol("]")
                    },
                    |p| {
                        p.attempt(|p| {
                            p.raw(".")?;
                            p.function_call()
                        })
                    },
                    |p| {
                        p.attempt(|p| {
                            p.raw(".")?;
                            p.identifier().map(|_| ())
                        })
                    },
                    |p| p.symbol("++"),
                    |p| p.symbol("--"),
                ])
            })?
            .is_some()
        {}
        Ok(())
    }
    fn unary(&mut self) -> R {
        while self
            .optional(|p| {
                p.choice(&[
                    |p| p.symbol("++"),
                    |p| p.symbol("--"),
                    |p| p.symbol("+"),
                    |p| p.symbol("-"),
                    |p| p.symbol("!"),
                    |p| p.symbol("~"),
                ])
            })?
            .is_some()
        {}
        self.postfix()
    }
    fn binary_operator(&mut self, op: &str) -> R {
        if !self.rest().starts_with(op) {
            return self.fail();
        }
        let next = self.rest()[op.len()..].chars().next();
        let disallow_equal = matches!(
            op,
            "*" | "/" | "%" | "+" | "-" | "<<" | ">>" | "<" | ">" | "&" | "^" | "|"
        );
        if disallow_equal && next == Some('=')
            || matches!(op, "&" | "|") && next == op.chars().next()
        {
            return self.fail();
        }
        self.symbol(op)
    }
    fn binary(&mut self, level: usize) -> R {
        const LEVELS: &[&[&str]] = &[
            &["||"],
            &["&&"],
            &["|"],
            &["^"],
            &["&"],
            &["==", "!="],
            &["<", ">", "<=", ">="],
            &["<<", ">>"],
            &["+", "-"],
            &["*", "/", "%"],
        ];
        if level == LEVELS.len() {
            return self.unary();
        }
        self.binary(level + 1)?;
        loop {
            let mut found = false;
            for op in LEVELS[level] {
                if self.optional(|p| p.binary_operator(op))?.is_some() {
                    found = true;
                    break;
                }
            }
            if !found {
                break;
            }
            self.binary(level + 1)?;
        }
        Ok(())
    }
    fn conditional(&mut self) -> R {
        self.nested(|p| {
            p.binary(0)?;
            if p.optional(|p| p.symbol("?"))?.is_some() {
                p.expression()?;
                p.symbol(":")?;
                p.assignment()?;
            }
            Ok(())
        })
    }
    fn assignment_level(&mut self, level: usize) -> R {
        const OPS: &[&str] = &[
            "|=", "^=", "&=", ">>=", "<<=", "%=", "/=", "*=", "-=", "+=", "=",
        ];
        if level == OPS.len() {
            return self.conditional();
        }
        self.assignment_level(level + 1)?;
        if self.optional(|p| p.symbol(OPS[level]))?.is_some() {
            self.nested(|p| p.assignment_level(level))?;
        }
        Ok(())
    }
    fn assignment(&mut self) -> R {
        self.assignment_level(0)
    }
    fn expression(&mut self) -> R {
        self.assignment()?;
        while self.optional(|p| p.symbol(","))?.is_some() {
            self.assignment()?;
        }
        Ok(())
    }
    fn precision(&mut self) -> R {
        self.one_word("highp mediump lowp").map(|_| ())
    }
    fn storage(&mut self) -> R<Qualifier> {
        let word = self.one_word("const attribute varying in out centroid uniform")?;
        Ok(match word {
            "attribute" => Qualifier::Attribute,
            "varying" => Qualifier::Varying,
            "uniform" => Qualifier::Uniform,
            "centroid" => {
                self.one_word("varying in out")?;
                Qualifier::Other
            }
            _ => Qualifier::Other,
        })
    }
    fn interpolation(&mut self) -> R {
        self.one_word("smooth flat noperspective").map(|_| ())
    }
    fn layout(&mut self) -> R {
        self.word("layout")?;
        self.symbol("(")?;
        self.list(|p| {
            p.identifier()?;
            if p.optional(|p| p.symbol("="))?.is_some() {
                p.integer()?;
            }
            Ok(())
        })?;
        self.symbol(")")
    }
    fn qualifier(&mut self) -> R<Qualifier> {
        self.choice(&[
            Self::storage,
            |p| {
                p.layout()?;
                p.optional(Self::storage)?;
                Ok(Qualifier::Other)
            },
            |p| {
                p.interpolation()?;
                p.optional(Self::storage)?;
                Ok(Qualifier::Other)
            },
            |p| {
                p.word("invariant")?;
                if p.optional(Self::interpolation)?.is_some() {
                    p.storage()?;
                } else {
                    p.optional(Self::storage)?;
                }
                Ok(Qualifier::Other)
            },
        ])
    }
    fn array(&mut self, empty: bool) -> R {
        self.symbol("[")?;
        if empty {
            self.optional(Self::conditional)?;
        } else {
            self.conditional()?;
        }
        self.symbol("]")
    }
    fn type_non_array(&mut self) -> R<Option<Type>> {
        self.choice(&[
            |p| {
                let word = p.one_word(TYPES)?;
                Ok(match word {
                    "int" => Some(Type::Int),
                    "float" => Some(Type::Float),
                    "vec2" => Some(Type::Vec2),
                    "vec3" => Some(Type::Vec3),
                    "vec4" => Some(Type::Vec4),
                    "mat4" => Some(Type::Mat4),
                    "sampler2D" => Some(Type::Texture),
                    _ => None,
                })
            },
            |p| {
                p.word("struct")?;
                p.optional(Self::identifier)?;
                p.symbol("{")?;
                p.fields()?;
                p.symbol("}")?;
                Ok(None)
            },
            |p| {
                p.identifier()?;
                Ok(None)
            },
        ])
    }
    fn type_no_precision(&mut self) -> R<Option<Type>> {
        self.nested(|p| {
            let ty = p.type_non_array()?;
            p.optional(|p| p.array(true))?;
            Ok(ty)
        })
    }
    fn type_specifier(&mut self) -> R<Option<Type>> {
        self.optional(Self::precision)?;
        self.type_no_precision()
    }
    fn full_type(&mut self) -> R<(Qualifier, Option<Type>)> {
        self.choice(&[
            |p| {
                p.attempt(Self::type_specifier)
                    .map(|ty| (Qualifier::Other, ty))
            },
            |p| {
                let q = p.qualifier()?;
                Ok((q, p.type_specifier()?))
            },
        ])
    }
    fn field(&mut self) -> R {
        self.optional(Self::qualifier)?;
        self.type_specifier()?;
        self.list(|p| {
            p.identifier()?;
            p.optional(|p| p.array(true))?;
            Ok(())
        })?;
        self.symbol(";")
    }
    fn fields(&mut self) -> R {
        self.field()?;
        while self.optional(Self::field)?.is_some() {}
        Ok(())
    }
    fn declarator(&mut self) -> R<String> {
        let name = self.identifier()?;
        self.optional(|p| p.array(true))?;
        if self.optional(|p| p.symbol("="))?.is_some() {
            self.assignment()?;
        }
        Ok(name)
    }
    fn declaration(&mut self) -> R<Declaration> {
        self.choice(&[
            |p| {
                p.attempt(|p| {
                    let (q, ty) = p.full_type()?;
                    let names = p.list(Self::declarator)?;
                    p.symbol(";")?;
                    Ok(Some((q, ty, names)))
                })
            },
            |p| {
                p.word("invariant")?;
                p.list(Self::declarator)?;
                p.symbol(";")?;
                Ok(None)
            },
            |p| {
                p.word("precision")?;
                p.precision()?;
                p.type_no_precision()?;
                p.symbol(";")?;
                Ok(None)
            },
            |p| {
                p.qualifier()?;
                if p.optional(|p| p.symbol(";"))?.is_some() {
                    return Ok(None);
                }
                p.identifier()?;
                p.symbol("{")?;
                p.fields()?;
                p.symbol("}")?;
                p.optional(|p| {
                    p.identifier()?;
                    p.optional(|p| p.array(true))?;
                    Ok(())
                })?;
                p.symbol(";")?;
                Ok(None)
            },
        ])
    }
    fn parameter(&mut self) -> R {
        self.optional(|p| p.word("const"))?;
        // Historical parser uses strings without a keyword boundary here.
        self.optional(|p| {
            p.choice(&[
                |p| p.symbol("inout"),
                |p| p.symbol("in"),
                |p| p.symbol("out"),
            ])
        })?;
        self.type_specifier()?;
        self.optional(|p| {
            p.identifier()?;
            p.optional(|p| p.array(false))?;
            Ok(())
        })?;
        Ok(())
    }
    fn prototype(&mut self) -> R {
        self.full_type()?;
        self.identifier()?;
        self.symbol("(")?;
        self.list(Self::parameter)?;
        self.symbol(")")
    }
    fn external(&mut self) -> R<Declaration> {
        self.choice(&[
            |p| {
                p.attempt(Self::prototype)?;
                p.choice(&[|p| p.symbol(";"), Self::compound])?;
                Ok(None)
            },
            Self::declaration,
        ])
    }
    fn compound(&mut self) -> R {
        self.symbol("{")?;
        self.nested(|p| {
            while p.optional(|p| p.symbol("}"))?.is_none() {
                p.statement()?;
            }
            Ok(())
        })
    }
    fn expression_statement(&mut self) -> R {
        if self.optional(|p| p.symbol(";"))?.is_some() {
            return Ok(());
        }
        self.expression()?;
        self.symbol(";")
    }
    fn condition(&mut self) -> R {
        self.choice(&[Self::expression, |p| {
            p.full_type()?;
            p.identifier()?;
            p.symbol("=")?;
            p.assignment()
        }])
    }
    fn statement(&mut self) -> R {
        self.nested(|p| {
            p.choice(&[
                Self::compound,
                |p| p.declaration().map(|_| ()),
                Self::expression_statement,
                |p| {
                    p.word("if")?;
                    p.symbol("(")?;
                    p.expression()?;
                    p.symbol(")")?;
                    p.statement()?;
                    p.optional(|p| {
                        p.word("else")?;
                        p.statement()
                    })?;
                    Ok(())
                },
                |p| {
                    p.word("switch")?;
                    p.symbol("(")?;
                    p.expression()?;
                    p.symbol(")")?;
                    p.compound()
                },
                |p| {
                    p.word("case")?;
                    p.expression()?;
                    p.symbol(":")
                },
                |p| {
                    p.word("default")?;
                    p.symbol(":")
                },
                |p| {
                    p.word("while")?;
                    p.symbol("(")?;
                    p.condition()?;
                    p.symbol(")")?;
                    p.statement()
                },
                |p| {
                    p.word("do")?;
                    p.statement()?;
                    p.word("while")?;
                    p.symbol("(")?;
                    p.expression()?;
                    p.symbol(")")?;
                    p.symbol(";")
                },
                |p| {
                    p.word("for")?;
                    p.symbol("(")?;
                    p.choice(&[Self::expression_statement, |p| p.declaration().map(|_| ())])?;
                    p.optional(Self::condition)?;
                    p.symbol(";")?;
                    p.optional(Self::expression)?;
                    p.symbol(")")?;
                    p.statement()
                },
                |p| {
                    p.one_word("continue break discard")?;
                    p.symbol(";")
                },
                |p| {
                    p.word("return")?;
                    p.optional(Self::expression)?;
                    p.symbol(";")
                },
            ])
        })
    }
}
