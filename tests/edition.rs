//! Behaviour that follows the Elm release an application is pinned to.
use planexpo_elm::{analyze, project::discover};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Application {
    root: PathBuf,
}

impl Application {
    fn pinned_to(version: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "elm-rs-edition-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        let core = root.join("cache/0.19.1/packages/elm/core/1.0.0");
        let defaults = [
            "Basics", "Debug", "List", "Maybe", "Result", "String", "Char", "Tuple", "Platform", "Platform.Cmd",
            "Platform.Sub",
        ];
        for name in defaults {
            let path = core.join("src").join(name.replace('.', "/") + ".elm");
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            // Only what the default imports name; these programs use nothing else.
            let body = match name {
                "List" => "infix right 5 (::) = cons\ncons a b = b",
                "Maybe" => "type Maybe a = Just a | Nothing",
                "Result" => "type Result error value = Ok value | Err error",
                "String" => "type String = String",
                "Char" => "type Char = Char",
                "Platform" => "type Program flags model msg = Program",
                "Platform.Cmd" => "type Cmd msg = Cmd",
                "Platform.Sub" => "type Sub msg = Sub",
                _ => "stub = ()",
            };
            fs::write(path, format!("module {name} exposing (..)\n{body}\n")).unwrap();
        }
        fs::write(
            core.join("elm.json"),
            json!({"type":"package","name":"elm/core","summary":"Test package","license":"BSD-3-Clause","version":"1.0.0","test-dependencies":{},"elm-version":"0.19.0 <= v < 0.20.0","exposed-modules":defaults,"dependencies":{}}).to_string(),
        )
        .unwrap();
        fs::write(
            root.join("elm.json"),
            json!({"type":"application","elm-version":version,"source-directories":["src"],"dependencies":{"direct":{"elm/core":"1.0.0"},"indirect":{}}}).to_string(),
        )
        .unwrap();
        Self { root }
    }

    /// JavaScript of every definition of `Main`, or the located error.
    fn compile(&self, body: &[u8]) -> Result<String, String> {
        let mut source = b"module Main exposing (..)\n".to_vec();
        source.extend_from_slice(body);
        fs::write(self.root.join("src/Main.elm"), source).unwrap();
        let graph = discover(&self.root.join("elm.json"), std::path::Path::new("src/Main.elm"), &self.root.join("cache"))?;
        let report = analyze::generate_modules(&graph)?;
        Ok(report.generated.iter().map(|definition| definition.javascript.as_str()).collect::<Vec<_>>().join("\n"))
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn position(javascript: &str, needle: &str) -> usize {
    javascript.find(needle).unwrap_or_else(|| panic!("{needle} is missing from {javascript}"))
}

#[test]
fn record_fields_follow_the_name_order_of_the_pinned_release() {
    let body = b"value = { alpha = (), b = () }\n";
    let lexical = Application::pinned_to("0.19.1").compile(body).unwrap();
    assert!(position(&lexical, "alpha") < position(&lexical, "\"b\""), "{lexical}");
    // Elm 0.19.3 compares names by length before content.
    let length_first = Application::pinned_to("0.19.3").compile(body).unwrap();
    assert!(position(&length_first, "\"b\"") < position(&length_first, "alpha"), "{length_first}");
}

#[test]
fn identifiers_follow_the_unicode_tables_of_the_pinned_release() {
    // U+1C90 GEORGIAN MTAVRULI CAPITAL LETTER AN is a letter since Unicode 11.
    let body = "value\u{1C90}suffix = ()\n".as_bytes();
    assert!(Application::pinned_to("0.19.1").compile(body).is_err());
    assert!(Application::pinned_to("0.19.2").compile(body).is_ok());
    assert!(Application::pinned_to("0.19.3").compile(body).is_ok());
}

#[test]
fn character_literals_are_decoded_since_0_19_2() {
    let body = b"value = '\\u{0041}'\n";
    let kept = Application::pinned_to("0.19.1").compile(body).unwrap();
    assert!(kept.contains("\\u0041"), "{kept}");
    let decoded = Application::pinned_to("0.19.3").compile(body).unwrap();
    assert!(decoded.contains("'A'") && !decoded.contains("\\u0041"), "{decoded}");
    // A lone surrogate was written as invalid UTF-8, read back as replacement characters.
    let surrogate = Application::pinned_to("0.19.3").compile(b"value = '\\u{D800}'\n").unwrap();
    assert!(surrogate.contains("\\uFFFD\\uFFFD\\uFFFD"), "{surrogate}");
}

#[test]
fn duplicate_arguments_are_reported_on_the_occurrence_of_the_pinned_release() {
    let body = b"f x x = ()\n";
    let later = Application::pinned_to("0.19.1").compile(body).unwrap_err();
    let earlier = Application::pinned_to("0.19.3").compile(body).unwrap_err();
    assert_ne!(later, earlier);
    assert!(later.contains("duplicate local name x") && earlier.contains("duplicate local name x"));
}

#[test]
fn sources_that_are_not_utf8_get_an_encoding_report_since_0_19_2() {
    let body = b"value = () -- caf\xff\n";
    let generic = Application::pinned_to("0.19.1").compile(body).unwrap_err();
    assert!(!generic.contains("source is not UTF-8"), "{generic}");
    let located = Application::pinned_to("0.19.3").compile(body).unwrap_err();
    assert!(located.ends_with(":2:18:2:18: source is not UTF-8"), "{located}");
    // A syntax error before the invalid byte still comes first.
    let syntax = Application::pinned_to("0.19.3").compile(b"value = = () -- caf\xff\n").unwrap_err();
    assert!(!syntax.contains("source is not UTF-8"), "{syntax}");
}
