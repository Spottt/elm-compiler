//! A conservative identity for changes to expression string contents only.
use crate::{ast::{Expr, Syntax}, lexer::Kind};
use sha2::{Digest, Sha256};

pub(super) fn identity(ast: &Syntax<'_>) -> Option<[u8; 32]> {
    let mut ranges = Vec::new();
    for node in &ast.expressions {
        if let Expr::Literal(Kind::String, text) = node.kind {
            let start = (text.as_ptr() as usize).checked_sub(ast.source.as_ptr() as usize)?;
            let end = start.checked_add(text.len())?;
            if ast.source.get(start..end)? != text { return None; }
            ranges.push((start, end));
        }
    }
    if ranges.is_empty() { return None; }
    ranges.sort_unstable(); ranges.dedup();
    let mut hash = Sha256::new();
    super::add(&mut hash, b"expression-string-contents-and-ast-v1");
    let mut previous = 0;
    for (start, end) in ranges {
        if start < previous { return None; }
        super::add(&mut hash, ast.source.get(previous..start)?.as_bytes());
        previous = end;
    }
    super::add(&mut hash, ast.source.get(previous..)?.as_bytes());
    // Changed literal width/newlines can affect Elm's layout grammar. Require
    // the parsed structure as well, excluding positions and these values only.
    for node in &ast.expressions {
        match &node.kind {
            Expr::Literal(Kind::String, _) => super::add(&mut hash, b"expression-string"),
            other => super::add(&mut hash, format!("{other:?}").as_bytes()),
        }
    }
    for node in &ast.patterns { super::add(&mut hash, format!("{:?}", node.kind).as_bytes()); }
    for node in &ast.types { super::add(&mut hash, format!("{:?}", node.kind).as_bytes()); }
    super::add(&mut hash, format!("{:?}", ast.declarations).as_bytes());
    Some(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(source: &str) -> Option<[u8; 32]> { identity(&crate::parser::parse(source).unwrap()) }
    #[test]
    fn expression_text_changes_preserve_types_but_every_other_byte_matters() {
        let original = "module Main exposing (..)\nvalue = \"short\"\n";
        for text in ["\"longer é 😀\"", "\"\"\"first\nsecond\"\"\"", "\"\\n\\u{0041}\""] {
            assert_eq!(key(original), key(&original.replace("\"short\"", text)));
        }
        for changed in [original.replace("value", "renamed"), original.replace("=", "= identity"), original.replace("\"short\"", "42"), format!("{original}-- comment\n")] {
            assert_ne!(key(original), key(&changed));
        }
    }
    #[test]
    fn patterns_shaders_numbers_and_binding_structure_are_not_erased() {
        let source = "module Main exposing (..)\nvalue x =\n    case x of\n        \"a\" -> \"yes\"\n        _ -> \"no\"\nnumber = 1\nshader = [glsl|void main() {}|]\n";
        for changed in [source.replace("\"a\"", "\"b\""), source.replace("number = 1", "number = 2"), source.replace("main() {}", "main() { float x; }"), source.replace("value x", "value y")] {
            assert_ne!(key(source), key(&changed));
        }
        assert_eq!(key("module Main exposing (..)\nvalue = 1\n"), None);
        let mut ast = crate::parser::parse(source).unwrap();
        let before = identity(&ast);
        ast.expressions.reverse();
        assert_ne!(before, identity(&ast), "identical source slices cannot bypass structural validation");
    }
    #[test]
    fn alternate_artifacts_keep_dependency_compiler_manifest_path_and_mode_boundaries() {
        use crate::{project::{Graph, Module}, cache::SourceDigests, kernel::Mode};
        use super::super::TypeCache;
        use std::collections::BTreeMap;
        let source = "module Main exposing (..)\nvalue = \"first\"\n";
        let ast = crate::parser::parse(source).unwrap();
        let mut graph = Graph { entry:"app:Main".into(), entries:vec!["app:Main".into()], manifests:vec![("elm.json".into(), "manifest".into())], import_errors:vec![], modules:vec![] };
        for (name, kernel, imports) in [("Elm.Kernel.Test", true, vec![]), ("Dep", false, vec![]), ("Main", false, vec!["app:Dep".into()])] {
            graph.modules.push(Module { owner:"app".into(), name:name.into(), source:source.into(), path:format!("src/{name}.elm").into(), imports, kernel, missing_header:None, bytes:0, tokens:0 });
        }
        let directory = tempfile::tempdir().unwrap();
        let cache = |graph: &Graph, compiler: &[u8], mode| TypeCache::with_text_edits(directory.path(), &SourceDigests::new(graph), compiler, mode, true).unwrap();
        let interfaces = BTreeMap::from([("app:Dep".into(), [1;32])]);
        let original = cache(&graph, b"compiler", Mode::Development);
        original.for_text_edits("app:Main", &ast, &interfaces).unwrap().store(b"types").unwrap();
        let edited = crate::parser::parse("module Main exposing (..)\nvalue = \"longer text\"\n").unwrap();
        assert_eq!(original.for_text_edits("app:Main", &edited, &interfaces).unwrap().load(), Some(b"types".to_vec()));
        assert!(original.for_text_edits("app:Main", &edited, &BTreeMap::from([("app:Dep".into(), [2;32])])).unwrap().load().is_none());
        assert!(cache(&graph, b"other compiler", Mode::Development).for_text_edits("app:Main", &ast, &interfaces).unwrap().load().is_none());
        for mode in [Mode::Production, Mode::Debug] { assert!(cache(&graph, b"compiler", mode).for_text_edits("app:Main", &ast, &interfaces).is_none()); }
        assert!(original.for_diagnostics().for_text_edits("app:Main", &ast, &interfaces).is_none());
        for variant in 0..3 {
            let mut changed = graph.clone();
            match variant { 0 => changed.manifests[0].1.push('!'), 1 => changed.modules[0].source.push('!'), _ => changed.modules[2].path = "other/Main.elm".into() }
            assert!(cache(&changed, b"compiler", Mode::Development).for_text_edits("app:Main", &ast, &interfaces).unwrap().load().is_none());
        }
    }
}
