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
fn hash(e: &mut Engine, roots: &[Ty]) -> [u8; 32] {
    e.fingerprint_types(roots, |id| Ok(format!("type-{}", id.0)))
        .unwrap()
}

#[test]
fn alpha_renaming_allocation_levels_and_structural_sharing_do_not_change_hash() {
    let mut a = engine();
    let x = a.variable(1, C::Any);
    let y = a.variable(1, C::Number);
    let pair = a.term(Term::Tuple(vec![x, y]));
    let root = a.term(Term::Function(pair, pair));
    let expected = hash(&mut a, &[root]);
    let mut b = engine();
    for _ in 0..100 {
        b.variable(12, C::Comparable);
    }
    let y = b.variable(99, C::Number);
    let x = b.variable(3, C::Any);
    let alias = b.variable(3, C::Any);
    b.unify(alias, x).unwrap();
    let first = b.term(Term::Tuple(vec![alias, y]));
    let second = b.term(Term::Tuple(vec![x, y]));
    let root = b.term(Term::Function(first, second));
    assert_eq!(expected, hash(&mut b, &[root]));
    let mut roots = [root];
    b.compact(&mut roots);
    assert_eq!(expected, hash(&mut b, &roots));
}

#[test]
fn variable_relationships_constraints_and_alias_parameter_order_matter() {
    let mut e = engine();
    let a = e.variable(1, C::Any);
    let b = e.variable(1, C::Any);
    let n = e.variable(1, C::Number);
    let aa = e.term(Term::Function(a, a));
    let ab = e.term(Term::Function(a, b));
    let ba = e.term(Term::Function(b, a));
    assert_ne!(hash(&mut e, &[aa]), hash(&mut e, &[ab]));
    assert_eq!(hash(&mut e, &[ab]), hash(&mut e, &[ba]));
    assert_ne!(hash(&mut e, &[a]), hash(&mut e, &[n]));
    assert_ne!(hash(&mut e, &[a, b, ab]), hash(&mut e, &[b, a, ab]));
    let variants: Vec<_> = [
        C::Any,
        C::Number,
        C::Comparable,
        C::Appendable,
        C::CompAppend,
    ]
    .into_iter()
    .map(|c| {
        let ty = e.variable(1, c);
        hash(&mut e, &[ty])
    })
    .collect();
    assert_eq!(
        variants
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        5
    );
}

#[test]
fn names_records_and_argument_order_are_semantic() {
    let mut e = engine();
    let int = e.term(Term::Named(SymbolId(0), vec![]));
    let str_ = e.term(Term::Named(SymbolId(2), vec![]));
    let a = e.term(Term::Function(int, str_));
    let b = e.term(Term::Function(str_, int));
    assert_ne!(hash(&mut e, &[a]), hash(&mut e, &[b]));
    let x = e.variable(1, C::Any);
    let closed = e.term(Term::Record {
        fields: BTreeMap::from([("x".into(), int)]),
        extension: None,
    });
    let renamed = e.term(Term::Record {
        fields: BTreeMap::from([("y".into(), int)]),
        extension: None,
    });
    let open = e.term(Term::Record {
        fields: BTreeMap::from([("x".into(), int)]),
        extension: Some(x),
    });
    assert_ne!(hash(&mut e, &[closed]), hash(&mut e, &[renamed]));
    assert_ne!(hash(&mut e, &[closed]), hash(&mut e, &[open]));
    let relocated = e.term(Term::Named(SymbolId(98), vec![]));
    assert_eq!(
        hash(&mut e, &[int]),
        e.fingerprint_types(&[relocated], |_| Ok("type-0".into()))
            .unwrap()
    );
    assert!(
        e.fingerprint_types(&[int], |_| Err("unknown symbol".into()))
            .is_err()
    );
    let rigid = e.rigid(1, C::Any);
    assert!(e.fingerprint_types(&[rigid], |_| unreachable!()).is_err());
}

#[test]
fn deep_and_exponentially_shared_graphs_are_linear_and_stack_safe() {
    let mut e = engine();
    let mut root = e.variable(1, C::Any);
    for _ in 0..20_000 {
        root = e.term(Term::Function(root, root));
    }
    let before = hash(&mut e, &[root]);
    let artifact = e.export_types(&[root], |_| unreachable!()).unwrap();
    let mut restored = engine();
    restored.variable(0, C::Any);
    let roots = restored
        .import_types(&artifact, |_| unreachable!())
        .unwrap();
    assert_eq!(before, hash(&mut restored, &roots));
}

#[test]
fn scheme_fingerprints_reject_free_variables_and_survive_instantiation() {
    let mut e = engine();
    let a = e.variable(1, C::Any);
    let root = e.term(Term::Function(a, a));
    let open = planexpo_elm::unify::Scheme {
        root,
        quantified: Default::default(),
    };
    assert!(e.fingerprint_scheme(&open, |_| unreachable!()).is_err());
    let closed = e.generalize(root, 0);
    let before = e.fingerprint_scheme(&closed, |_| unreachable!()).unwrap();
    let instance = e.instantiate(&closed, 12);
    let generalized = e.generalize(instance, 0);
    assert_eq!(
        before,
        e.fingerprint_scheme(&generalized, |_| unreachable!())
            .unwrap()
    );
}

#[test]
fn record_extensions_are_normalized_to_their_semantic_fields() {
    let mut e = engine();
    let unit = e.term(Term::Unit);
    let empty = e.term(Term::Record {
        fields: BTreeMap::new(),
        extension: None,
    });
    let inner = e.term(Term::Record {
        fields: BTreeMap::from([("a".into(), unit)]),
        extension: Some(empty),
    });
    let nested = e.term(Term::Record {
        fields: BTreeMap::from([("z".into(), unit)]),
        extension: Some(inner),
    });
    let flat = e.term(Term::Record {
        fields: BTreeMap::from([("a".into(), unit), ("z".into(), unit)]),
        extension: None,
    });
    assert_eq!(hash(&mut e, &[nested]), hash(&mut e, &[flat]));
}
