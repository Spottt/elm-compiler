//! Carry real AST regions through the existing string-based compiler boundary.
use crate::ast::Span;

pub fn is_located(message: &str) -> bool {
    let mut parts = message.splitn(5, ':');
    (0..4).all(|_| parts.next().is_some_and(|p| p.parse::<usize>().is_ok()))
        && parts.next().is_some()
}

pub fn locate(source: &str, span: Span, message: String) -> String {
    if is_located(&message) {
        return message;
    }
    fn position(source: &str, offset: u32) -> Option<(usize, usize)> {
        let prefix = source.get(..offset as usize)?;
        Some((
            prefix.bytes().filter(|b| *b == b'\n').count() + 1,
            prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
        ))
    }
    match (position(source, span.start), position(source, span.end)) {
        (Some((line, column)), Some((end_line, end_column))) => {
            format!("{line}:{column}:{end_line}:{end_column}: {message}")
        }
        _ => message,
    }
}

/// Recover the exact range of an AST string borrowed from this source. Do not
/// search by text: repeated identifiers must point at the actual occurrence.
pub fn locate_slice(source: &str, fragment: &str, message: String) -> String {
    let Some(start) = (fragment.as_ptr() as usize).checked_sub(source.as_ptr() as usize) else {
        return message;
    };
    let Some(end) = start.checked_add(fragment.len()) else {
        return message;
    };
    if source.get(start..end) != Some(fragment) {
        return message;
    }
    let (Ok(start), Ok(end)) = (u32::try_from(start), u32::try_from(end)) else {
        return message;
    };
    locate(source, Span { start, end }, message)
}

/// Transport separate source failures without flattening their file regions.
const BATCH_PREFIX: &str = "ELM_SOURCE_ERRORS:";
pub fn batch(messages: Vec<String>) -> String {
    if messages.len() == 1 {
        return messages.into_iter().next().unwrap();
    }
    format!(
        "{BATCH_PREFIX}{}",
        serde_json::to_string(&messages).unwrap()
    )
}
pub fn batch_messages(message: &str) -> Option<Vec<String>> {
    serde_json::from_str(message.strip_prefix(BATCH_PREFIX)?).ok()
}

const MODULE_PREFIX: &str = "ELM_SOURCE_MODULE:";
/// Keep a module identity obtained from discovery when its header cannot parse.
pub fn in_module(name: &str, message: String) -> String {
    format!(
        "{MODULE_PREFIX}{}",
        serde_json::to_string(&(name, message)).unwrap()
    )
}
pub fn module_message(message: &str) -> Option<(String, String)> {
    serde_json::from_str(message.strip_prefix(MODULE_PREFIX)?).ok()
}
