//! Elm literal emission using the same escaped representation as pattern checks.
use crate::{kernel::Mode, lexer::Kind};

/// Emit an already lexed Elm literal as a JavaScript expression. Character
/// boxing in development mode is required by the Elm kernel's equality/debugger.
pub fn emit(kind: Kind, raw: &str, mode: Mode) -> Result<String, String> {
    match kind {
        Kind::String | Kind::Char => {
            let value = format!("'{}'", canonical_text(raw)?);
            if kind == Kind::Char && matches!(mode, Mode::Development) {
                Ok(format!("_Utils_chr({value})"))
            } else {
                Ok(value)
            }
        }
        Kind::Number => {
            if !raw.starts_with("0x") && raw.contains(['.', 'e', 'E']) {
                return Ok(raw.into());
            }
            // Preserve ordinary literal spelling, but reproduce the reference
            // parser's signed machine-integer accumulation on overflow.
            let ordinary = if let Some(hex) = raw.strip_prefix("0x") {
                i64::from_str_radix(hex, 16).is_ok()
            } else {
                raw.parse::<i64>().is_ok()
            };
            if ordinary {
                return Ok(raw.into());
            }
            let value = integer_value(raw)?;
            Ok(if value < 0 { format!("({value})") } else { value.to_string() })
        }
        _ => Err("expected a number, string or character literal".into()),
    }
}

// Elm compares its JS-escaped string representation here, not decoded runtime
// text. Preserve this distinction: 'a' and '\u{0061}' are different patterns
// for upstream's redundancy check, while '\u{0061}' and '\u{000061}' are equal.
pub(crate) fn canonical_text(raw: &str) -> Result<String, String> {
    let multiline = raw.starts_with("\"\"\"");
    let is_char = raw.starts_with('\'');
    let quote = if multiline { 3 } else { 1 };
    let mut chars = raw[quote..raw.len() - quote].chars();
    let mut output = String::new();
    while let Some(c) = chars.next() {
        if c != '\\' {
            match c {
                '\'' if !is_char => output.push_str("\\'"),
                '"' if is_char => output.push_str("\\\""),
                '\n' if multiline => output.push_str("\\n"),
                '\r' if multiline => output.push_str("\\r"),
                _ => output.push(c),
            }
            continue;
        }
        let escaped = chars.next().ok_or("unfinished escape")?;
        if escaped != 'u' {
            output.push('\\');
            output.push(escaped);
            continue;
        }
        if chars.next() != Some('{') {
            return Err("invalid unicode escape".into());
        }
        let mut hex = String::new();
        for c in chars.by_ref() {
            if c == '}' {
                break;
            }
            hex.push(c);
        }
        let code = i64::from_str_radix(&hex, 16).map_err(|_| "invalid unicode escape")?;
        if code < 0xFFFF {
            output.push_str(&format!("\\u{code:04X}"));
        } else {
            let offset = code - 0x10000;
            let high = offset.div_euclid(0x400) + 0xD800;
            let low = offset.rem_euclid(0x400) + 0xDC00;
            output.push_str(&format!("\\u{high:04X}\\u{low:04X}"));
        }
    }
    Ok(output)
}

/// Parse.Number accumulates decimal and hexadecimal integers in a Haskell Int.
pub(crate) fn integer_value(raw: &str) -> Result<i64, String> {
    let (digits, radix) = raw.strip_prefix("0x").map_or((raw, 10), |s| (s, 16));
    if digits.is_empty() { return Err("empty integer literal".into()); }
    digits.chars().try_fold(0i64, |value, c| {
        let digit = c.to_digit(radix).ok_or("invalid integer literal")?;
        Ok(value.wrapping_mul(radix as i64).wrapping_add(digit as i64))
    })
}
