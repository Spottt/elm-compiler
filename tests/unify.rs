use planexpo_elm::{
    names::SymbolId,
    unify::{Builtins, Constraint as C, Engine, Term, Ty},
};
use std::collections::BTreeMap;
fn engine() -> Engine {
    Engine::new(Builtins {
        int: SymbolId(0),
        float: SymbolId(1),
        string: SymbolId(2),
        char: SymbolId(3),
        list: SymbolId(4),
    })
}
fn int(e: &mut Engine) -> Ty {
    e.term(Term::Named(SymbolId(0), vec![]))
}
fn string(e: &mut Engine) -> Ty {
    e.term(Term::Named(SymbolId(2), vec![]))
}
#[test]
fn optional_display_names_survive_instantiation_unification_and_compaction() {
    let mut e = engine();
    e.track_display_names();
    let named = e.named_variable(1, C::Any, "value");
    let scheme = e.generalize(named, 0);
    let copied = e.instantiate(&scheme, 1);
    let unnamed = e.variable(1, C::Any);
    e.unify(unnamed, copied).unwrap();
    let mut roots = [unnamed];
    e.compact(&mut roots);
    let artifact = e.export_types(&roots, |_| unreachable!()).unwrap();
    assert_eq!(artifact["variable_names"]["0"], "value");
    let mut restored = engine();
    restored.track_display_names();
    restored.variable(0, C::Any);
    let roots = restored
        .import_types(&artifact, |_| unreachable!())
        .unwrap();
    let roundtrip = restored.export_types(&roots, |_| unreachable!()).unwrap();
    assert_eq!(roundtrip["variable_names"]["0"], "value");
    let mut ordinary = engine();
    let named = ordinary.named_variable(1, C::Any, "value");
    assert!(
        ordinary
            .export_types(&[named], |_| unreachable!())
            .unwrap()
            .get("variable_names")
            .is_none()
    );
}

#[test]
fn repl_alias_identity_is_distinct_from_its_shared_structural_body() {
    let mut e = engine();
    e.track_display_names();
    let parameter = e.named_variable(1, C::Any, "item");
    let alias = e.alias(SymbolId(10), vec![parameter], parameter);
    let scheme = e.generalize(alias, 0);
    let instance = e.instantiate(&scheme, 1);
    let unit = e.term(Term::Unit);
    e.unify(instance, unit).unwrap();
    assert!(matches!(e.structure(instance).as_deref(), Some(Term::Unit)));
    let mut roots = [instance, unit];
    e.compact(&mut roots);
    let artifact = e.export_types(&roots, |_| Ok("Alias".into())).unwrap();
    let ids = artifact["roots"].as_array().unwrap();
    assert_eq!(
        artifact["nodes"][ids[0].as_u64().unwrap() as usize][0],
        "alias"
    );
    assert_eq!(
        artifact["nodes"][ids[1].as_u64().unwrap() as usize][0],
        "unit"
    );
    let mut restored = engine();
    restored.track_display_names();
    restored.variable(0, C::Any);
    let restored_roots = restored
        .import_types(&artifact, |_| Ok(SymbolId(10)))
        .unwrap();
    let roundtrip = restored
        .export_types(&restored_roots, |_| Ok("Alias".into()))
        .unwrap();
    assert_eq!(roundtrip, artifact);
    let mut ordinary = engine();
    let unit = ordinary.term(Term::Unit);
    assert_eq!(ordinary.alias(SymbolId(10), vec![], unit), unit);
}
#[test]
fn compaction_preserves_shared_types_constraints_and_polymorphism() {
    let mut e = engine();
    for _ in 0..10_000 {
        let discarded = e.variable(1, C::Any);
        e.term(Term::Function(discarded, discarded));
    }
    let a = e.variable(1, C::Any);
    let alias = e.variable(1, C::Any);
    e.unify(alias, a).unwrap();
    let function = e.term(Term::Function(alias, a));
    let number = e.variable(1, C::Number);
    let mut roots = [function, a, alias, number];
    e.compact(&mut roots);
    assert_eq!(e.node_count(), 3);
    assert_eq!(roots[1], roots[2]);
    let scheme = e.generalize(roots[0], 0);
    assert_eq!(scheme.quantified.len(), 1);
    let first = e.instantiate(&scheme, 1);
    let second = e.instantiate(&scheme, 1);
    let integer = int(&mut e);
    let text = string(&mut e);
    let int_fn = e.term(Term::Function(integer, integer));
    let str_fn = e.term(Term::Function(text, text));
    e.unify(first, int_fn).unwrap();
    e.unify(second, str_fn).unwrap();
    assert!(e.unify(roots[3], text).is_err());
}

#[test]
fn compaction_preserves_immutable_views_and_relocates_unified_records() {
    let mut e = engine();
    for _ in 0..100 {
        e.variable(0, C::Any);
    }
    let integer = int(&mut e);
    let first = record(&mut e, &[("value", integer)], None);
    let second = e.variable(0, C::Any);
    let view = e.structure(first).unwrap();
    e.unify(first, second).unwrap();
    let mut roots = [first, second, integer];
    e.compact(&mut roots);
    assert_eq!(roots[0], roots[1]);
    assert_eq!(e.node_count(), 2);
    let Term::Record { fields, .. } = &*view else {
        panic!("expected record")
    };
    assert_eq!(
        fields["value"], integer,
        "immutable pre-compaction view was modified"
    );
    let relocated = e.structure(roots[0]).unwrap();
    let Term::Record { fields, .. } = &*relocated else {
        panic!("expected record")
    };
    assert_eq!(fields["value"], roots[2]);
    assert_ne!(integer, roots[2]);
    let text = string(&mut e);
    assert!(e.unify(roots[2], text).is_err());
}
fn record(e: &mut Engine, fields: &[(&str, Ty)], extension: Option<Ty>) -> Ty {
    e.term(Term::Record {
        fields: fields.iter().map(|(n, t)| (n.to_string(), *t)).collect(),
        extension,
    })
}
#[test]
fn polymorphic_identity_instantiates_independently() {
    let mut e = engine();
    let a = e.variable(1, C::Any);
    let f = e.term(Term::Function(a, a));
    let scheme = e.generalize(f, 0);
    let first = e.instantiate(&scheme, 1);
    let second = e.instantiate(&scheme, 1);
    let i = int(&mut e);
    let s = string(&mut e);
    let fi = e.term(Term::Function(i, i));
    let fs = e.term(Term::Function(s, s));
    e.unify(first, fi).unwrap();
    e.unify(second, fs).unwrap();
    assert!(e.unify(first, second).is_err());
}
#[test]
fn environment_variables_are_not_generalized_and_occurs_check_rejects_cycles() {
    let mut e = engine();
    let outer = e.variable(0, C::Any);
    let inner = e.variable(1, C::Any);
    let list = e.term(Term::Named(SymbolId(4), vec![inner]));
    e.unify(outer, list).unwrap();
    assert!(e.generalize(inner, 0).quantified.is_empty());
    let f = e.term(Term::Function(inner, outer));
    assert!(e.unify(inner, f).unwrap_err().contains("infinite"));
}
#[test]
fn open_records_unify_symmetrically_and_reject_missing_closed_fields() {
    let mut e = engine();
    let i = int(&mut e);
    let s = string(&mut e);
    let ra = e.variable(1, C::Any);
    let rb = e.variable(1, C::Any);
    let a = record(&mut e, &[("a", i)], Some(ra));
    let b = record(&mut e, &[("b", s)], Some(rb));
    e.unify(a, b).unwrap();
    let both = record(&mut e, &[("a", i), ("b", s)], None);
    e.unify(a, both).unwrap();
    e.unify(b, both).unwrap();
    let missing = record(&mut e, &[("b", s)], None);
    assert!(e.unify(a, missing).is_err());
}
#[test]
fn constraints_propagate_into_lists_and_intersect() {
    let mut e = engine();
    let comparable = e.variable(1, C::Comparable);
    let appendable = e.variable(1, C::Appendable);
    e.unify(comparable, appendable).unwrap();
    let element = e.variable(1, C::Any);
    let list = e.term(Term::Named(SymbolId(4), vec![element]));
    e.unify(comparable, list).unwrap();
    let empty = record(&mut e, &[], None);
    assert!(e.unify(element, empty).is_err());
    let mut e = engine();
    let number = e.variable(1, C::Number);
    let s = string(&mut e);
    assert!(e.unify(number, s).is_err());
}
#[test]
fn monomorphic_shared_records_are_reused_across_instantiations() {
    let mut e = engine();
    let i = int(&mut e);
    let fields = (0..10000)
        .map(|n| (format!("field{n}"), i))
        .collect::<BTreeMap<_, _>>();
    let large = e.term(Term::Record {
        fields,
        extension: None,
    });
    let scheme = e.generalize(large, 0);
    let before = e.node_count();
    for _ in 0..1000 {
        assert_eq!(e.instantiate(&scheme, 1), large);
    }
    assert_eq!(e.node_count(), before);
}

#[test]
fn expanded_record_rows_preserve_outer_fields_like_elm() {
    let mut e = engine();
    let i = int(&mut e);
    let s = string(&mut e);
    let extension = record(&mut e, &[("a", s), ("b", s)], None);
    let expanded = record(&mut e, &[("a", i)], Some(extension));
    let flat = record(&mut e, &[("a", i), ("b", s)], None);
    e.unify(expanded, flat).unwrap();
}

#[test]
fn resolved_record_extensions_cannot_be_non_records() {
    let mut e = engine();
    let i = int(&mut e);
    let a = record(&mut e, &[("a", i)], Some(i));
    let b = record(&mut e, &[("a", i)], Some(i));
    assert!(e.unify(a, b).is_err());
}

#[test]
fn annotations_are_checked_for_every_type_not_just_one_instance() {
    let mut e = engine();
    let a = e.variable(1, C::Any);
    let signature = e.term(Term::Function(a, a));
    let scheme = e.generalize(signature, 0);
    let annotation = e.skolemize(&scheme, 1);
    let implementation_var = e.variable(1, C::Any);
    let identity = e.term(Term::Function(implementation_var, implementation_var));
    e.unify(identity, annotation).unwrap();

    let annotation = e.skolemize(&scheme, 1);
    let i = int(&mut e);
    let only_int = e.term(Term::Function(i, i));
    assert!(e.unify(only_int, annotation).is_err());
}

#[test]
fn distinct_annotation_variables_stay_distinct_and_cannot_escape() {
    let mut e = engine();
    let a = e.variable(1, C::Any);
    let b = e.variable(1, C::Any);
    let signature = e.term(Term::Function(a, b));
    let scheme = e.generalize(signature, 0);
    let annotation = e.skolemize(&scheme, 1);
    let x = e.variable(1, C::Any);
    let identity = e.term(Term::Function(x, x));
    assert!(e.unify(identity, annotation).is_err());

    let mut e = engine();
    let a = e.variable(1, C::Any);
    let scheme = e.generalize(a, 0);
    let rigid = e.skolemize(&scheme, 1);
    let captured = e.variable(0, C::Any);
    assert!(e.unify(captured, rigid).is_err());
}

#[test]
fn annotation_constraints_cannot_be_strengthened_by_the_body() {
    for (declared, body, accepted) in [
        (C::Any, C::Number, false),
        (C::Comparable, C::Number, false),
        (C::Number, C::Comparable, true),
        (C::CompAppend, C::Appendable, true),
        (C::Appendable, C::Comparable, false),
        (C::Any, C::Any, true),
    ] {
        let mut e = engine();
        let a = e.variable(1, declared);
        let scheme = e.generalize(a, 0);
        let annotation = e.skolemize(&scheme, 1);
        let implementation = e.variable(1, body);
        assert_eq!(
            e.unify(implementation, annotation).is_ok(),
            accepted,
            "declared {declared:?}, body {body:?}"
        );
    }
}

#[test]
fn alias_substitution_preserves_sharing_and_does_not_specialize_the_template() {
    let mut e = engine();
    let parameter = e.variable(1, C::Any);
    let i = int(&mut e);
    let fields = (0..10000).map(|n| (format!("field{n}"), i)).collect();
    let large = e.term(Term::Record {
        fields,
        extension: None,
    });
    let template = e.term(Term::Tuple(vec![parameter, parameter, large]));
    let before = e.node_count();
    let first = e.substitute(template, &[(parameter, i)]);
    assert_eq!(e.node_count(), before + 1);
    let Term::Tuple(parts) = &*e.structure(first).unwrap() else {
        panic!()
    };
    assert_eq!(parts, &[i, i, large]);
    let s = string(&mut e);
    let second = e.substitute(template, &[(parameter, s)]);
    let Term::Tuple(parts) = &*e.structure(second).unwrap() else {
        panic!()
    };
    assert_eq!(parts, &[s, s, large]);
    let Term::Tuple(parts) = &*e.structure(template).unwrap() else {
        panic!()
    };
    assert_eq!(parts, &[parameter, parameter, large]);
    assert!(e.structure(parameter).is_none());
}

#[test]
fn polymorphic_scheme_reuses_large_monomorphic_subgraphs() {
    let mut e = engine();
    let a = e.variable(1, C::Any);
    let i = int(&mut e);
    let fields = (0..10000).map(|n| (format!("field{n}"), i)).collect();
    let large = e.term(Term::Record {
        fields,
        extension: None,
    });
    let f = e.term(Term::Function(a, large));
    let scheme = e.generalize(f, 0);
    let before = e.node_count();
    let instance = e.instantiate(&scheme, 1);
    assert_eq!(e.node_count(), before + 2);
    let Term::Function(arg, result) = &*e.structure(instance).unwrap() else {
        panic!()
    };
    assert_ne!(*arg, a);
    assert_eq!(*result, large);
}

#[test]
fn portable_type_graph_preserves_sharing_constraints_and_symbol_remapping() {
    let mut source = engine();
    let a = source.variable(1, C::Any);
    let alias = source.variable(1, C::Any);
    source.unify(a, alias).unwrap();
    let identity = source.term(Term::Function(alias, a));
    let number = source.variable(1, C::Number);
    let rigid = source.rigid(1, C::Any);
    let integer = int(&mut source);
    let row = source.variable(1, C::Any);
    let open = record(&mut source, &[("value", a)], Some(row));
    let snapshot = source
        .export_types(&[identity, a, alias, number, rigid, integer, open], |id| {
            Ok(format!("type{}", id.0))
        })
        .unwrap();
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    let snapshot = serde_json::from_slice(&bytes).unwrap();
    let mut dest = Engine::new(Builtins {
        int: SymbolId(10),
        float: SymbolId(11),
        string: SymbolId(12),
        char: SymbolId(13),
        list: SymbolId(14),
    });
    let existing = dest.variable(0, C::Any);
    let roots = dest
        .import_types(&snapshot, |name| {
            Ok(SymbolId(
                name.strip_prefix("type").unwrap().parse::<u32>().unwrap() + 10,
            ))
        })
        .unwrap();
    assert_ne!(roots[1], existing);
    assert_eq!(roots[1], roots[2]);
    let scheme = dest.generalize(roots[0], 0);
    assert_eq!(scheme.quantified.len(), 1);
    let first = dest.instantiate(&scheme, 1);
    let second = dest.instantiate(&scheme, 1);
    let text = dest.term(Term::Named(SymbolId(12), vec![]));
    let int_fn = dest.term(Term::Function(roots[5], roots[5]));
    let str_fn = dest.term(Term::Function(text, text));
    dest.unify(first, int_fn).unwrap();
    dest.unify(second, str_fn).unwrap();
    let row_scheme = dest.generalize(roots[6], 0);
    let instance = dest.instantiate(&row_scheme, 1);
    let closed = record(&mut dest, &[("value", roots[5]), ("extra", text)], None);
    dest.unify(instance, closed).unwrap();
    assert!(dest.unify(roots[4], roots[5]).is_err());
    // A failed unification invalidates an inference attempt; use a fresh engine
    // for the independent numeric-constraint rejection.
    let mut constrained = engine();
    let roots = constrained
        .import_types(&snapshot, |name| {
            Ok(SymbolId(
                name.strip_prefix("type").unwrap().parse().unwrap(),
            ))
        })
        .unwrap();
    let text = string(&mut constrained);
    assert!(constrained.unify(roots[3], text).is_err());
}

#[test]
fn invalid_type_artifacts_do_not_mutate_the_destination_engine() {
    use serde_json::json;
    let bad = [
        json!({"version":2,"nodes":[],"roots":[]}),
        json!({"version":1,"nodes":[["unit"]],"roots":[8]}),
        json!({"version":1,"nodes":[["function",0,0]],"roots":[0]}),
        json!({"version":1,"nodes":[["unit"],["function",1,1]],"roots":[0]}),
        json!({"version":1,"nodes":[["unit"],["function",0,3]],"roots":[0]}),
        json!({"version":1,"nodes":[["variable",1,"unknown"]],"roots":[0]}),
        json!({"version":1,"nodes":[["tuple",[0]]],"roots":[0]}),
        json!({"version":1,"nodes":[["named","missing",[]]],"roots":[0]}),
        json!({"version":1,"nodes":[["unit"]],"roots":[0],"variable_names":{"0":"value"}}),
        json!({"version":1,"nodes":[["variable",1,"any"]],"roots":[0],"variable_names":{"1":"value"}}),
        json!({"version":1,"nodes":[["variable",1,"any"]],"roots":[0],"variable_names":{"0":""}}),
        json!({"version":1,"nodes":[["variable",1,"any"]],"roots":[0],"variable_names":[]}),
    ];
    for artifact in bad {
        let mut dest = engine();
        let existing = int(&mut dest);
        assert!(
            dest.import_types(&artifact, |_| Err("missing symbol".into()))
                .is_err()
        );
        assert_eq!(dest.node_count(), 1);
        assert!(matches!(
            &*dest.structure(existing).unwrap(),
            Term::Named(SymbolId(0), _)
        ));
    }
}

#[test]
fn portable_type_graph_handles_deep_shared_structures_without_recursion() {
    let mut source = engine();
    let unit = source.term(Term::Unit);
    let mut root = unit;
    for _ in 0..20_000 {
        root = source.term(Term::Function(unit, root));
    }
    let snapshot = source
        .export_types(&[root], |_| Err("no named types".into()))
        .unwrap();
    let mut dest = engine();
    let roots = dest
        .import_types(&snapshot, |_| Err("no named types".into()))
        .unwrap();
    assert_eq!(dest.node_count(), 20_001);
    let mut cursor = roots[0];
    let mut shared = None;
    for _ in 0..20_000 {
        let term = dest.structure(cursor).unwrap();
        let Term::Function(a, b) = &*term else {
            panic!("lost function")
        };
        assert_eq!(*a, *shared.get_or_insert(*a));
        cursor = *b;
    }
    assert!(matches!(&*dest.structure(cursor).unwrap(), Term::Unit));
}
