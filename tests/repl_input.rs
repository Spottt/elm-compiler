use planexpo_elm::repl_input::{Action, Command, Reader};
use planexpo_elm::repl_session::Input;

#[test]
fn commands_blank_lines_and_legacy_backslash_match_repl_rules() {
    let mut reader = Reader::default();
    assert!(matches!(reader.push("   "), Action::Ready(Command::Skip)));
    assert!(matches!(
        reader.push("  :quit"),
        Action::Ready(Command::Exit)
    ));
    assert!(matches!(
        reader.push(":reset"),
        Action::Ready(Command::Reset)
    ));
    assert!(
        matches!(reader.push(":help "), Action::Ready(Command::Help(Some(name))) if name == "help")
    );
    assert!(
        matches!(reader.push("value = 1\\"), Action::Ready(Command::Evaluate(Input::Declaration { name, source })) if name == "value" && source == "value = 1\n")
    );
}

#[test]
fn multiline_values_wait_for_blank_line_and_annotations_prefill_definition() {
    let mut reader = Reader::default();
    assert!(matches!(reader.push("value : Int"), Action::More { prefill } if prefill == "value "));
    assert!(matches!(reader.push("value = 3"), Action::More { .. }));
    assert!(
        matches!(reader.push(""), Action::Ready(Command::Evaluate(Input::Declaration { name, .. })) if name == "value")
    );
    assert!(matches!(reader.push("[1,"), Action::More { .. }));
    assert!(matches!(reader.push(" 2]"), Action::More { .. }));
    assert!(matches!(
        reader.push(""),
        Action::Ready(Command::Evaluate(Input::Expression { .. }))
    ));
    assert!(matches!(
        reader.push("value"),
        Action::Ready(Command::Evaluate(Input::Expression { .. }))
    ));
}

#[test]
fn imports_types_and_ports_are_distinguished_and_incomplete_input_can_be_cancelled() {
    let mut reader = Reader::default();
    assert!(
        matches!(reader.push("import Maybe as M"), Action::Ready(Command::Evaluate(Input::Import { name, .. })) if name == "Maybe")
    );
    assert!(
        matches!(reader.push("type Box a = Box a"), Action::Ready(Command::Evaluate(Input::Type { name, .. })) if name == "Box")
    );
    assert!(matches!(
        reader.push("port broken"),
        Action::Ready(Command::Port)
    ));
    assert!(matches!(reader.push("value ="), Action::More { .. }));
    reader.cancel();
    assert!(matches!(
        reader.push("()"),
        Action::Ready(Command::Evaluate(Input::Expression { .. }))
    ));
}
