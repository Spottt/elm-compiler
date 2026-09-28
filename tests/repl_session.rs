use planexpo_elm::repl_session::{Input, Session};

#[test]
fn session_commits_only_successful_compilation_and_execution() {
    let mut session = Session::default();
    let declaration = |text: &str| Input::Declaration {
        name: "value".into(),
        source: text.into(),
    };
    session
        .attempt(declaration("value = 1"), |source, binding| {
            assert!(source.contains("value = 1\n"));
            assert_eq!(binding, Some("value"));
            Ok::<_, &str>(())
        })
        .unwrap();
    let failed = session.attempt(declaration("value = Debug.todo \"boom\""), |_, _| {
        Err::<(), _>("Node failed")
    });
    assert!(failed.is_err());
    session
        .attempt(
            Input::Expression {
                source: "value\n".into(),
            },
            |source, binding| {
                assert_eq!(binding, Some("repl_input_value_"));
                assert!(source.contains("value = 1\n"));
                assert!(!source.contains("boom"));
                assert!(source.ends_with("repl_input_value_ =\n  value\n"));
                Ok::<_, &str>(())
            },
        )
        .unwrap();
}

#[test]
fn session_orders_and_replaces_definitions_and_reset_clears_them() {
    let mut session = Session::default();
    for input in [
        Input::Declaration {
            name: "z".into(),
            source: "z = ()".into(),
        },
        Input::Import {
            name: "Set".into(),
            source: "import Set".into(),
        },
        Input::Type {
            name: "Box".into(),
            source: "type Box = Box".into(),
        },
        Input::Import {
            name: "Set".into(),
            source: "import Set as S".into(),
        },
    ] {
        session.attempt(input, |_, _| Ok::<_, ()>(())).unwrap();
    }
    session.attempt(Input::Declaration { name: "a".into(), source: "a = z".into() }, |source, _| {
        assert_eq!(source, "module Elm_Repl exposing (..)\nimport Set as S\ntype Box = Box\na = z\nz = ()\nrepl_input_value_ = ()\n");
        Ok::<_, ()>(())
    }).unwrap();
    session.reset();
    session
        .attempt(
            Input::Expression {
                source: "()".into(),
            },
            |source, _| {
                assert_eq!(
                    source,
                    "module Elm_Repl exposing (..)\nrepl_input_value_ =\n  ()\n"
                );
                Ok::<_, ()>(())
            },
        )
        .unwrap();
}

#[test]
fn completions_follow_committed_state_and_elm_namespace_order() {
    let mut session = Session::default();
    for input in [
        Input::Declaration {
            name: "zulu".into(),
            source: "zulu = ()".into(),
        },
        Input::Declaration {
            name: "alpha".into(),
            source: "alpha = ()".into(),
        },
        Input::Type {
            name: "Box".into(),
            source: "type Box = Box".into(),
        },
        Input::Import {
            name: "Maybe".into(),
            source: "import Maybe as M".into(),
        },
    ] {
        session.attempt(input, |_, _| Ok::<_, ()>(())).unwrap();
    }
    assert_eq!(
        session.completions(""),
        vec![
            ("alpha".into(), false),
            ("zulu".into(), false),
            ("Box".into(), false),
            ("Maybe".into(), true),
            (":exit".into(), false),
            (":help".into(), false),
            (":quit".into(), false),
            (":reset".into(), false),
        ]
    );
    assert_eq!(session.completions("Ma"), vec![("Maybe".into(), true)]);
    assert!(session.completions("M.").is_empty());
    let result = session.attempt(
        Input::Declaration {
            name: "broken".into(),
            source: "broken = missing".into(),
        },
        |_, _| Err::<(), _>("compile failed"),
    );
    assert!(result.is_err());
    assert!(session.completions("bro").is_empty());
    session.reset();
    assert_eq!(session.completions(":").len(), 4);
    assert!(session.completions("Ma").is_empty());
    assert!(session.completions("alpha").is_empty());
}
