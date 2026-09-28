use planexpo_elm::{
    generated_cache,
    module_codegen::{Definition, Registration},
    names::{Space, SymbolKind, Symbols},
};
use std::collections::BTreeSet;
#[test]
fn generated_dependencies_are_remapped_after_symbol_ids_move() {
    let mut old = Symbols::default();
    let first = old.intern("app:Main", "first", Space::Value, SymbolKind::Value);
    let dependency = old.intern("app:Other", "value", Space::Value, SymbolKind::Value);
    let definitions = vec![Definition {
        symbol: first,
        javascript: "var first = other;".into(),
        dependencies: BTreeSet::from([dependency]),
        function: false,
        registration: Some(Registration {
            javascript: "register(first);".into(),
            dependencies: BTreeSet::from([first]),
        }),
        cycle_initialization: Some("initialize(first);".into()),
    }];
    let artifact = generated_cache::encode(&definitions, &old);
    let mut new = Symbols::default();
    new.intern("app:Extra", "unrelated", Space::Value, SymbolKind::Value);
    let next_dependency = new.intern("app:Other", "value", Space::Value, SymbolKind::Value);
    let next_first = new.intern("app:Main", "first", Space::Value, SymbolKind::Value);
    let restored = generated_cache::decode(&artifact, &new).unwrap();
    assert_eq!(restored[0].symbol, next_first);
    assert_eq!(restored[0].dependencies, BTreeSet::from([next_dependency]));
    assert_eq!(
        restored[0].registration.as_ref().unwrap().dependencies,
        BTreeSet::from([next_first])
    );
    assert_eq!(restored[0].javascript, definitions[0].javascript);
    assert_eq!(
        restored[0].cycle_initialization,
        definitions[0].cycle_initialization
    );
    assert!(generated_cache::decode(&artifact, &Symbols::default()).is_none());
    let mut malformed = artifact;
    malformed[0]["function"] = serde_json::json!("not boolean");
    assert!(generated_cache::decode(&malformed, &new).is_none());
}

#[test]
fn cache_preserves_optional_data_and_rejects_damaged_definitions() {
    let mut symbols = Symbols::default();
    let id = symbols.intern("app:Main", "main", Space::Value, SymbolKind::Value);
    let value = serde_json::json!([{
        "symbol": ["app:Main", "main"], "javascript": "var main = 1;",
        "dependencies": [], "function": false,
        "cycle_initialization": null, "registration": null
    }]);
    let restored = generated_cache::decode(&value, &symbols).unwrap();
    assert_eq!(restored[0].symbol, id);
    assert_eq!(restored[0].javascript, "var main = 1;");
    assert!(restored[0].registration.is_none());
    assert!(restored[0].cycle_initialization.is_none());
    for field in ["symbol", "javascript", "dependencies", "function", "cycle_initialization", "registration"] {
        let mut broken = value.clone();
        broken[0].as_object_mut().unwrap().remove(field);
        assert!(generated_cache::decode(&broken, &symbols).is_none(), "{field}");
    }
    for (field, invalid) in [
        ("javascript", serde_json::json!(12)),
        ("cycle_initialization", serde_json::json!(false)),
        ("registration", serde_json::json!({"javascript": "register();", "dependencies": [["missing", "value"]]})),
    ] {
        let mut broken = value.clone(); broken[0][field] = invalid;
        assert!(generated_cache::decode(&broken, &symbols).is_none(), "{field}");
    }
}
