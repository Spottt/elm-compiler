//! Bounded, content-addressed memoization for a single development worker.
//! Only immutable decoding results survive requests; type engines never do.
use crate::lexer::Token;
use serde_json::Value;
use crate::cache::content_digest;
use std::{cell::RefCell, collections::HashMap, marker::PhantomData, rc::Rc};

const LIMIT: usize = 64 * 1024 * 1024;
const KINDS: usize = 7;
#[derive(Clone)]
enum Decoded {
    Tokens(Vec<Token>, Option<String>),
    Json(Rc<Value>),
    Syntax(Result<(), String>),
    Header(Rc<HeaderResult>),
    Prepared(Rc<crate::typed_artifact::Prepared>),
    GeneratedTokens(Option<[u8; 32]>),
}
struct Entry { value: Decoded, weight: usize, used: u64, request: u64 }
#[derive(Default)]
struct Cache { identity: u64, request: u64, prepared_types: bool, changed_paths: Vec<std::path::PathBuf>, entries: HashMap<(u8, [u8; 32]), Entry>, weight: usize, clock: u64, hits: [u64; KINDS], misses: [u64; KINDS] }
thread_local! { static CACHE: RefCell<Option<Cache>> = const { RefCell::new(None) }; }

pub struct Scope { previous: Option<Cache>, _thread: PhantomData<Rc<()>> }
/// Scoped so library/CLI calls remain uncached, including explicit --no-cache.
pub fn scope(enabled: bool) -> Scope {
    static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    Scope { previous: CACHE.with(|cache| cache.replace(enabled.then(|| Cache {
        identity: SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        prepared_types: std::env::var("PLANEXPO_ELM_PREPARED_TYPES").as_deref() == Ok("1"), ..Cache::default()
    }))), _thread: PhantomData }
}
impl Drop for Scope {
    fn drop(&mut self) { CACHE.with(|cache| cache.replace(self.previous.take())); }
}
fn key(kind: u8, bytes: &[u8]) -> Option<(u8, [u8; 32])> {
    CACHE.with(|cache| cache.borrow().as_ref().map(|_| (kind, content_digest(bytes))))
}
fn get(key: &(u8, [u8; 32])) -> Option<Decoded> {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let cache = cache.as_mut()?;
        cache.clock += 1;
        let Some(entry) = cache.entries.get_mut(key) else { cache.misses[key.0 as usize] += 1; return None; };
        cache.hits[key.0 as usize] += 1;
        entry.used = cache.clock;
        entry.request = cache.request;
        Some(entry.value.clone())
    })
}
fn insert(key: (u8, [u8; 32]), value: Decoded, weight: usize) {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let Some(cache) = cache.as_mut() else { return; };
        if weight > LIMIT { return; }
        if let Some(old) = cache.entries.remove(&key) { cache.weight -= old.weight; }
        while cache.weight + weight > LIMIT {
            // Prefix checkpoints change the working set. Prefer evicting data
            // unused in both this request and the preceding one; otherwise old
            // small prefix tokens can permanently crowd out active artifacts.
            // Syntax proofs are tiny and remain useful when an error forces
            // collection over the otherwise skipped prefix. Protect up to 1 MiB
            // of them, still inside the shared limit. Retire large stale payloads
            // first so a working-set change does not require thousands of evictions.
            let syntax_bytes: usize = cache.entries.iter().filter(|(key, _)| key.0 == 2).map(|(_, entry)| entry.weight).sum();
            if let Some(stale) = cache.entries.iter()
                .filter(|(key, entry)| entry.request.saturating_add(1) < cache.request && (key.0 != 2 || syntax_bytes > LIMIT / 64))
                .max_by_key(|(key, entry)| (matches!(key.0, 0 | 1 | 5), entry.weight, std::cmp::Reverse(entry.used))).map(|(key, _)| *key) {
                cache.weight -= cache.entries.remove(&stale).unwrap().weight;
                continue;
            }
            // A sequential scan larger than the budget makes ordinary LRU
            // evict every useful entry before its next use. Prefer smaller
            // decoded values, retaining a stable working subset instead.
            let Some((largest, previous)) = cache.entries.iter()
                .max_by_key(|(_, entry)| (entry.weight, std::cmp::Reverse(entry.used)))
                .map(|(key, entry)| (*key, entry.weight)) else { break; };
            if weight >= previous { return; }
            cache.weight -= cache.entries.remove(&largest).unwrap().weight;
        }
        cache.clock += 1;
        cache.weight += weight;
        cache.entries.insert(key, Entry { value, weight, used: cache.clock, request: cache.request });
    });
}
pub(crate) fn tokens(source: &str, decode: impl FnOnce() -> (Vec<Token>, Option<String>)) -> (Vec<Token>, Option<String>) {
    let Some(key) = key(0, source.as_bytes()) else { return decode(); };
    if let Some(Decoded::Tokens(tokens, error)) = get(&key) { return (tokens, error); }
    let (tokens, error) = decode();
    let weight = 128 + tokens.capacity() * std::mem::size_of::<Token>() + error.as_ref().map_or(0, String::capacity);
    insert(key, Decoded::Tokens(tokens.clone(), error.clone()), weight);
    (tokens, error)
}
/// Ordinary module syntax only; documentation and REPL grammars are distinct.
/// Store no borrowed AST and no path-dependent diagnostic rendering.
pub(crate) fn syntax(source: &str) -> Option<Result<(), String>> {
    let key = key(2, source.as_bytes())?;
    match get(&key) { Some(Decoded::Syntax(result)) => Some(result), _ => None }
}
pub(crate) fn remember_syntax(source: &str, result: Result<(), String>) {
    let Some(key) = key(2, source.as_bytes()) else { return; };
    let weight = 128 + result.as_ref().err().map_or(0, |error| error.capacity());
    insert(key, Decoded::Syntax(result), weight);
}
pub(crate) fn generated_tokens_with_digest(digest: &[u8; 32], decode: impl FnOnce() -> Option<[u8; 32]>) -> Option<[u8; 32]> {
    let Some(key) = CACHE.with(|cache| cache.borrow().as_ref().map(|_| (6, *digest))) else { return decode(); };
    if let Some(Decoded::GeneratedTokens(value)) = get(&key) { return value; }
    let value = decode();
    insert(key, Decoded::GeneratedTokens(value), 160);
    value
}
type HeaderResult = Result<(Rc<crate::module::Header>, usize, bool), String>;
/// Discovery lexes the full source, so the full bytes (not just the imports)
/// and lexical recovery policy are part of the key. Paths are resolved afresh.
pub(crate) fn discovery_header(source: &str, recover: bool, digest: Option<[u8; 32]>, decode: impl FnOnce() -> HeaderResult) -> HeaderResult {
    let Some(key) = CACHE.with(|cache| cache.borrow().as_ref().map(|_| (if recover { 3 } else { 4 }, digest.unwrap_or_else(|| content_digest(source.as_bytes()))))) else { return decode(); };
    if let Some(Decoded::Header(value)) = get(&key) { return (*value).clone(); }
    let value = decode();
    let weight = match &value {
        Err(error) => 128 + error.capacity(),
        Ok((header, _, _)) => header_weight(header) + 32,
    };
    insert(key, Decoded::Header(Rc::new(value.clone())), weight);
    value
}
fn header_weight(header: &crate::module::Header) -> usize {
    use crate::module::{Effects, Exposed, Exposing, Import};
    fn exposing_weight(exposing: &Exposing) -> usize {
        match exposing {
            Exposing::All => 0,
            Exposing::Explicit(items) => items.capacity() * std::mem::size_of::<Exposed>() + items.iter().map(|item| {
                let name = match item { Exposed::Value(name) | Exposed::Operator(name) | Exposed::Type { name, .. } => name };
                name.capacity() + 16
            }).sum::<usize>(),
        }
    }
    let effects = match &header.effects {
        Effects::Manager { command, subscription } => command.as_ref().map_or(0, String::capacity) + subscription.as_ref().map_or(0, String::capacity) + 32,
        _ => 0,
    };
    256 + std::mem::size_of::<crate::module::Header>() + header.name.capacity() + effects
        + exposing_weight(&header.exposing) + header.imports.capacity() * std::mem::size_of::<Import>()
        + header.imports.iter().map(|import| import.name.capacity() + import.alias.as_ref().map_or(0, String::capacity) + exposing_weight(&import.exposing) + 32).sum::<usize>()
}
/// The disk cache has already verified key and payload before this is called.
/// Re-read/verify bytes every time so external corruption/removal still misses.
pub(crate) fn json(bytes: &[u8]) -> Option<Rc<Value>> {
    let key = key(1, bytes);
    if let Some(key) = &key && let Some(Decoded::Json(value)) = get(key) { return Some(value); }
    let value = Rc::new(serde_json::from_slice(bytes).ok()?);
    if let Some(key) = key {
        // Conservative accounting for Value, map nodes, strings and allocator
        // overhead. Oversized artifacts are decoded normally but not retained.
        let weight = json_weight(&value).saturating_add(128);
        insert(key, Decoded::Json(value.clone()), weight);
    }
    Some(value)
}

/// Bytes are freshly read and verified by OutputCache before lookup. Prepared
/// relative graphs replace JSON retention for worker type artifacts.
pub(crate) fn prepared_types(bytes: &[u8]) -> Option<Rc<crate::typed_artifact::Prepared>> {
    let key = key(5, bytes);
    if let Some(key) = &key && let Some(Decoded::Prepared(value)) = get(key) { return Some(value); }
    let value = Rc::new(crate::typed_artifact::Prepared::new(serde_json::from_slice(bytes).ok()?).ok()?);
    if let Some(key) = key {
        let weight = value.graph_bytes().saturating_add(json_weight(value.metadata())).saturating_add(256);
        insert(key, Decoded::Prepared(value.clone()), weight);
    }
    Some(value)
}

fn json_weight(value: &Value) -> usize {
    let mut weight = std::mem::size_of::<Value>();
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::String(s) => weight += s.capacity() + 16,
            Value::Array(items) => { weight += items.capacity() * std::mem::size_of::<Value>(); pending.extend(items); }
            Value::Object(items) => {
                // Account conservatively for BTree nodes/unused slots as well
                // as inline key/value storage and separate string buffers.
                weight += items.len() * (std::mem::size_of::<Value>() + std::mem::size_of::<String>() + 96);
                for (key, value) in items { weight += key.capacity() + 16; pending.push(value); }
            }
            _ => {}
        }
    }
    weight
}
pub fn statistics() -> Value {
    CACHE.with(|cache| match cache.borrow().as_ref() {
        Some(cache) => {
            let mut bytes_by_kind = [0usize; KINDS];
            for (key, entry) in &cache.entries { bytes_by_kind[key.0 as usize] += entry.weight; }
            serde_json::json!({"entries":cache.entries.len(),"estimated_bytes":cache.weight,"bytes_by_kind":bytes_by_kind,"limit_bytes":LIMIT,"hits":cache.hits,"misses":cache.misses,"prepared_types":cache.prepared_types})
        },
        None => Value::Null,
    })
}

pub(crate) fn use_prepared_types() -> bool { CACHE.with(|cache| cache.borrow().as_ref().is_some_and(|cache| cache.prepared_types)) }

pub(crate) fn identity() -> Option<u64> { CACHE.with(|cache| cache.borrow().as_ref().map(|cache| cache.identity)) }

/// Advisory watch metadata only; source fingerprints remain authoritative.
pub fn set_changed_paths(paths: Vec<std::path::PathBuf>) {
    CACHE.with(|cache| { if let Some(cache) = cache.borrow_mut().as_mut() { cache.changed_paths = paths; cache.request = cache.request.saturating_add(1); } });
}
pub(crate) fn changed_paths() -> Vec<std::path::PathBuf> {
    CACHE.with(|cache| cache.borrow().as_ref().map_or_else(Vec::new, |cache| cache.changed_paths.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_token_fingerprints_are_bounded_and_reported() {
        let _session = scope(true);
        assert_eq!(generated_tokens_with_digest(&content_digest(b"one"), || Some([1; 32])), Some([1; 32]));
        assert_eq!(generated_tokens_with_digest(&content_digest(b"one"), || panic!("must reuse")), Some([1; 32]));
        assert_eq!(generated_tokens_with_digest(&content_digest(b"invalid"), || None), None);
        assert_eq!(generated_tokens_with_digest(&content_digest(b"invalid"), || panic!("must reuse failure")), None);
        let stats = statistics();
        assert_eq!(stats["hits"][6], 2);
        assert_eq!(stats["bytes_by_kind"][6], 320);
        assert!(stats["estimated_bytes"].as_u64().unwrap() <= LIMIT as u64);
    }
    #[test]
    fn content_keys_and_scopes_do_not_replay_changed_or_invalid_json() {
        let _session = scope(true);
        let first = json(br#"{"value":1}"#).unwrap();
        assert!(Rc::ptr_eq(&first, &json(br#"{"value":1}"#).unwrap()));
        assert_eq!(json(br#"{"value":2}"#).unwrap()["value"], 2);
        assert!(json(b"truncated").is_none());
        { let _disabled = scope(false); assert!(!Rc::ptr_eq(&first, &json(br#"{"value":1}"#).unwrap())); }
        assert!(Rc::ptr_eq(&first, &json(br#"{"value":1}"#).unwrap()));
    }
    #[test]
    fn syntax_checks_reuse_only_identical_module_content_and_respect_disabled_scope() {
        let _session = scope(true);
        let valid = "module Main exposing (value)\nvalue = 42\n";
        assert!(crate::parser::parse(valid).is_ok());
        let before = statistics()["hits"][2].as_u64().unwrap();
        assert!(crate::parser::check_syntax(valid).is_ok());
        assert_eq!(statistics()["hits"][2].as_u64().unwrap(), before + 1);
        for invalid in ["module Main exposing (value)\nvalue = )\n", "module Main exposing (value)\nvalue = \"unterminated"] {
            assert!(syntax(invalid).is_none());
            let error = crate::parser::check_syntax(invalid).unwrap_err();
            assert_eq!(crate::parser::check_syntax(invalid).unwrap_err(), error);
            let _disabled = scope(false);
            assert!(syntax(valid).is_none());
            assert_eq!(crate::parser::check_syntax(invalid).unwrap_err(), error);
        }
        assert!(crate::parser::check_syntax(valid).is_ok());
        // A REPL fragment is legal there but not a compiled module. It must
        // never seed the ordinary module syntax cache.
        let fragment = "module Elm_Repl exposing (..)\nimport List\n";
        assert!(crate::parser::parse_repl_fragment(fragment).is_ok());
        assert!(syntax(fragment).is_none());
        assert!(crate::parser::check_syntax(fragment).is_err());
    }
    #[test]
    fn prepared_graphs_are_content_addressed_and_scope_local() {
        let _session = scope(true);
        let bytes = br#"{"version":1,"module":"test:A","entries":[],"graph":{"version":1,"nodes":[["unit"]],"roots":[0]}}"#;
        let first = prepared_types(bytes).unwrap();
        assert!(Rc::ptr_eq(&first, &prepared_types(bytes).unwrap()));
        assert!(prepared_types(b"corrupted").is_none());
        assert!(prepared_types(br#"{"graph":{"version":1,"nodes":[["function",0,0]],"roots":[0]}}"#).is_none());
        { let _disabled = scope(false); assert!(!Rc::ptr_eq(&first, &prepared_types(bytes).unwrap())); }
        assert!(Rc::ptr_eq(&first, &prepared_types(bytes).unwrap()));
    }
    #[test]
    fn unused_prefix_entries_yield_space_after_the_working_set_changes() {
        let _session = scope(true);
        let a = (1, [1; 32]); let b = (1, [2; 32]); let c = (1, [3; 32]);
        for key in [a, b] { insert(key, Decoded::Json(Rc::new(Value::Null)), LIMIT / 2); }
        set_changed_paths(vec![]);
        assert!(get(&a).is_some());
        insert(c, Decoded::Json(Rc::new(Value::Null)), LIMIT / 2);
        assert!(get(&c).is_none(), "keep data from the preceding request");
        set_changed_paths(vec![]);
        assert!(get(&a).is_some());
        insert(c, Decoded::Json(Rc::new(Value::Null)), LIMIT / 2);
        assert!(get(&a).is_some()); assert!(get(&b).is_none()); assert!(get(&c).is_some());
        CACHE.with(|cache| assert_eq!(cache.borrow().as_ref().unwrap().weight, LIMIT));
    }
    #[test]
    fn reclaiming_stale_tokens_preserves_small_syntax_proofs_but_keeps_a_bound() {
        for syntax_weight in [128, LIMIT / 2] {
            let _session = scope(true);
            let syntax_key = (2, [1; 32]); let active = (1, [2; 32]); let incoming = (1, [3; 32]);
            insert(syntax_key, Decoded::Syntax(Ok(())), syntax_weight);
            insert(active, Decoded::Json(Rc::new(Value::Null)), LIMIT / 2);
            if syntax_weight == 128 { insert((0, [4;32]), Decoded::Tokens(vec![], None), LIMIT / 2 - syntax_weight); }
            set_changed_paths(vec![]); assert!(get(&active).is_some());
            set_changed_paths(vec![]); assert!(get(&active).is_some());
            insert(incoming, Decoded::Json(Rc::new(Value::Null)), LIMIT / 2 - if syntax_weight == 128 {128} else {0});
            assert!(get(&incoming).is_some());
            assert_eq!(get(&syntax_key).is_some(), syntax_weight == 128);
            CACHE.with(|cache| assert!(cache.borrow().as_ref().unwrap().weight <= LIMIT));
        }
    }
    #[test]
    fn admission_is_bounded_and_resists_sequential_scan_thrashing() {
        let _session = scope(true);
        let a = (1, [1; 32]); let b = (1, [2; 32]); let c = (1, [3; 32]);
        for key in [a, b] { insert(key, Decoded::Json(Rc::new(Value::Null)), LIMIT / 2); }
        assert!(get(&a).is_some());
        insert(c, Decoded::Json(Rc::new(Value::Null)), LIMIT / 4);
        assert!(get(&a).is_some()); assert!(get(&b).is_none()); assert!(get(&c).is_some());
        for index in 4..20 {
            insert((1, [index; 32]), Decoded::Json(Rc::new(Value::Null)), LIMIT * 3 / 4);
        }
        assert!(get(&a).is_some()); assert!(get(&c).is_some());
        CACHE.with(|cache| assert!(cache.borrow().as_ref().unwrap().weight <= LIMIT));
    }
    #[test]
    fn prepared_graphs_share_the_budget_and_reclaim_stale_tokens() {
        let _session = scope(true);
        let graph = Rc::new(crate::typed_artifact::Prepared::new(serde_json::json!({
            "version":1,"module":"test:A","entries":[],
            "graph":{"version":1,"nodes":[["unit"]],"roots":[0]}
        })).unwrap());
        let token = (0, [1; 32]); let a = (5, [2; 32]); let b = (5, [3; 32]);
        insert(token, Decoded::Tokens(vec![], None), LIMIT / 4);
        insert(a, Decoded::Prepared(graph.clone()), LIMIT * 3 / 4);
        insert(b, Decoded::Prepared(graph.clone()), LIMIT * 3 / 16);
        assert!(get(&a).is_none()); assert!(get(&b).is_some());
        assert!(get(&token).is_some());
        set_changed_paths(vec![]); assert!(get(&b).is_some());
        set_changed_paths(vec![]); assert!(get(&b).is_some());
        insert(a, Decoded::Prepared(graph), LIMIT * 3 / 4);
        assert!(get(&a).is_some()); assert!(get(&token).is_none());
        assert!(get(&b).is_some());
        CACHE.with(|cache| assert!(cache.borrow().as_ref().unwrap().weight <= LIMIT));
    }

}
