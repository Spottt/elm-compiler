//! Structured session probe. Inputs are preclassified JSON, not a terminal UI.
use planexpo_elm::{
    repl_compile,
    repl_session::{Input, Session},
};
use std::{
    env, fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

fn main() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    let [home, manifest, input, options @ ..] = args.as_slice() else {
        return Err("expected ELM_HOME elm.json inputs.json".into());
    };
    if !options.is_empty() && options != ["--classify"] {
        return Err("unsupported probe option".into());
    }
    let temporary = tempfile::tempdir().map_err(|e| e.to_string())?;
    let entry = temporary.path().join("Elm_Repl.elm");
    let inputs: Vec<serde_json::Value> =
        serde_json::from_str(&fs::read_to_string(input).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let mut session = Session::default();
    let mut outcomes = Vec::new();
    for input in inputs {
        if input["kind"] == "reset" {
            session.reset();
            outcomes.push(serde_json::json!({"ok":true,"stdout":"<reset>\n"}));
            continue;
        }
        let source = input["source"]
            .as_str()
            .ok_or("missing input source")?
            .to_string();
        let input = if !options.is_empty() {
            let mut reader = planexpo_elm::repl_input::Reader::default();
            let mut selected = None;
            for line in source.split('\n') {
                use planexpo_elm::repl_input::{Action, Command};
                match reader.push(line) {
                    Action::More { .. } | Action::Ready(Command::Skip) => {}
                    Action::Ready(Command::Evaluate(input)) => {
                        if selected.replace(input).is_some() {
                            return Err("multiple inputs in one probe step".into());
                        }
                    }
                    other => return Err(format!("unexpected probe action: {other:?}")),
                }
            }
            selected.ok_or("probe input still incomplete")?
        } else {
            match input["kind"].as_str() {
                Some("expression") => Input::Expression { source },
                Some(kind @ ("import" | "type" | "declaration")) => {
                    let name = input["name"]
                        .as_str()
                        .ok_or("missing input name")?
                        .to_string();
                    match kind {
                        "import" => Input::Import { name, source },
                        "type" => Input::Type { name, source },
                        _ => Input::Declaration { name, source },
                    }
                }
                _ => return Err("unknown input kind".into()),
            }
        };
        let result = session.attempt(input, |source, binding| -> Result<String, String> {
            let script = repl_compile::compile(
                Path::new(manifest),
                Path::new(home),
                &entry,
                source,
                binding,
                false,
            )?;
            let Some(script) = script else {
                return Ok(String::new());
            };
            let mut node = Command::new("node")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            node.stdin
                .take()
                .ok_or("missing Node stdin")?
                .write_all(script.as_bytes())
                .map_err(|e| e.to_string())?;
            let output = node.wait_with_output().map_err(|e| e.to_string())?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).into_owned());
            }
            String::from_utf8(output.stdout).map_err(|e| e.to_string())
        });
        outcomes.push(match result {
            Ok(stdout) => serde_json::json!({"ok":true,"stdout":stdout}),
            Err(error) => serde_json::json!({"ok":false,"error":error}),
        });
    }
    println!(
        "{}",
        serde_json::to_string(&outcomes).map_err(|e| e.to_string())?
    );
    Ok(())
}
