//! REPL command. Terminal editing and piped input share the same classifier.
//! Unix evaluation processes can be interrupted without losing session state.
use planexpo_elm::{
    repl_input::{Action, Command as InputCommand, Reader},
    repl_session::Session,
};
use std::{
    env, fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

#[cfg(not(unix))]
use planexpo_elm::repl_compile;
#[cfg(not(unix))]
use std::process::{Command, Stdio};

const HELP: &str = "Valid commands include:\n\n  :exit    Exit the REPL\n  :help    Show this information\n  :reset   Clear all previous imports and definitions\n\nMore info at <https://elm-lang.org/0.19.1/repl>\n";

pub fn run(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help") {
        eprintln!(
            "The `repl` command opens up an interactive programming session:\n\n    elm repl\n\nStart working through <https://guide.elm-lang.org> to learn how to use this! It\nhas a whole chapter that uses the REPL for everything, so that is probably the\nquickest way to get started.\n\nYou can customize this command with the following flags:\n\n    --interpreter=<interpreter>\n        Path to a alternate JS interpreter, like node or nodejs.\n    \n    --no-colors\n        Turn off the colors in the REPL. This can help if you are having trouble\n        reading the values. Some terminals use a custom color scheme that\n        diverges significantly from the standard ANSI colors, so another path\n        may be to pick a more standard color scheme.\n"
        );
        return Ok(());
    }
    let (ansi, interpreter) = parse_flags(args)?;
    println!(
        "\x1b[90m----\x1b[0m \x1b[36mElm 0.19.1\x1b[0m \x1b[90m----------------------------------------------------------------\x1b[0m\n\x1b[90mSay :help for help and :exit to exit! More at <https://elm-lang.org/0.19.1/repl>\x1b[0m\n\x1b[90m--------------------------------------------------------------------------------\x1b[0m"
    );
    let node = match interpreter {
        Some(name) => executable(&name).ok_or_else(|| missing_interpreter(&name))?,
        None => executable("node")
            .or_else(|| executable("nodejs"))
            .ok_or_else(|| missing_interpreter("node` or `nodejs"))?,
    };
    let temporary = tempfile::tempdir().map_err(|e| e.to_string())?;
    let entry = temporary.path().join("Elm_Repl.elm");
    let cwd = env::current_dir().map_err(|e| e.to_string())?;
    let manifest = match cwd.ancestors().find(|p| p.join("elm.json").is_file()) {
        Some(root) => root.join("elm.json"),
        None => {
            fs::create_dir(temporary.path().join("src")).map_err(|e| e.to_string())?;
            let manifest = temporary.path().join("elm.json");
            let outline = serde_json::json!({"type":"package","name":"author/project","summary":"helpful summary of your project, less than 80 characters","license":"BSD-3-Clause","version":"1.0.0","exposed-modules":[],"elm-version":"0.19.1 <= v < 0.20.0","dependencies":{"elm/core":"1.0.0 <= v <= 65535.0.0","elm/json":"1.0.0 <= v <= 65535.0.0","elm/html":"1.0.0 <= v <= 65535.0.0"},"test-dependencies":{}});
            fs::write(
                &manifest,
                serde_json::to_vec(&outline).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            manifest
        }
    };
    let home = env::var_os("ELM_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".elm")))
        .ok_or("ELM_HOME or HOME is required")?;
    let mut history = crate::repl_history::History::load(&home)?;
    #[cfg(unix)]
    let evaluation =
        crate::repl_evaluation::Evaluation::new(&manifest, &home, &entry, &node, ansi)?;
    let mut session = Session::default();
    let mut reader = Reader::default();
    let mut continuation = false;
    let mut editor = if io::stdin().is_terminal() && io::stdout().is_terminal() {
        let config = rustyline::Config::builder()
            .history_ignore_dups(false)
            .map_err(|e| e.to_string())?
            .completion_type(rustyline::CompletionType::List)
            .build();
        let mut editor = rustyline::Editor::<
            crate::repl_completion::Completion,
            rustyline::history::DefaultHistory,
        >::with_config(config)
        .map_err(|e| e.to_string())?;
        editor.set_helper(Some(crate::repl_completion::Completion::default()));
        for line in history.oldest_first() {
            editor.add_history_entry(line).map_err(|e| e.to_string())?;
        }
        Some(editor)
    } else {
        None
    };
    let mut prefill = String::new();
    loop {
        let prompt = if continuation { "| " } else { "> " };
        let line = if let Some(editor) = &mut editor {
            editor
                .helper_mut()
                .expect("REPL completion helper")
                .candidates = session.completions("");
            match editor.readline_with_initial(prompt, (&prefill, "")) {
                Ok(line) => {
                    if !line.chars().all(char::is_whitespace) {
                        editor
                            .add_history_entry(line.as_str())
                            .map_err(|e| e.to_string())?;
                        history.add(&line);
                    }
                    Some(line)
                }
                Err(rustyline::error::ReadlineError::Interrupted) => {
                    reader.cancel();
                    continuation = false;
                    prefill.clear();
                    continue;
                }
                Err(rustyline::error::ReadlineError::Eof) => None,
                Err(error) => return Err(error.to_string()),
            }
        } else {
            print!("{prompt}");
            io::stdout().flush().map_err(|e| e.to_string())?;
            let mut line = String::new();
            if io::stdin()
                .read_line(&mut line)
                .map_err(|e| e.to_string())?
                == 0
            {
                None
            } else {
                if line.ends_with('\n') {
                    line.pop();
                    if line.ends_with('\r') {
                        line.pop();
                    }
                }
                Some(line)
            }
        };
        let Some(line) = line else {
            if continuation {
                reader.cancel();
                continuation = false;
                prefill.clear();
                continue;
            }
            break;
        };
        let action = reader.push(&line);
        continuation = matches!(action, Action::More { .. });
        prefill = match &action {
            Action::More { prefill } => prefill.clone(),
            Action::Ready(_) => String::new(),
        };
        let Action::Ready(command) = action else {
            continue;
        };
        match command {
            InputCommand::Exit => break,
            InputCommand::Skip => {}
            InputCommand::Reset => {
                session.reset();
                println!("<reset>");
            }
            InputCommand::Help(unknown) => {
                if let Some(name) = unknown {
                    print!("I do not recognize the :{name} command. ");
                }
                println!("{HELP}");
            }
            InputCommand::Port => println!("I cannot handle port declarations."),
            InputCommand::Evaluate(input) => {
                let result = session.attempt(input, |source, binding| {
                    #[cfg(unix)]
                    {
                        evaluation.run(source, binding)
                    }
                    #[cfg(not(unix))]
                    {
                        let script =
                            repl_compile::compile(&manifest, &home, &entry, source, binding, ansi)?;
                        if let Some(script) = script {
                            interpret(&node, &script)?;
                        }
                        Ok::<_, String>(())
                    }
                });
                if let Err(error) = result
                    && !error.is_empty()
                {
                    eprintln!(
                        "{}",
                        crate::diagnostic::terminal(&crate::diagnostic::repl_error(&entry, error))
                    );
                }
            }
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn interpret(node: &Path, script: &str) -> Result<(), String> {
    let mut child = Command::new(node)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| e.to_string())?;
    let write = child
        .stdin
        .take()
        .ok_or("missing interpreter stdin")?
        .write_all(script.as_bytes());
    let status = child.wait().map_err(|e| e.to_string())?;
    write.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(String::new())
    }
}
fn executable(name: &str) -> Option<PathBuf> {
    let candidates: Vec<_> = if Path::new(name).components().count() > 1 {
        vec![PathBuf::from(name)]
    } else {
        env::split_paths(&env::var_os("PATH").unwrap_or_default())
            .map(|p| p.join(name))
            .collect()
    };
    candidates.into_iter().find(|p| {
        let Ok(metadata) = p.metadata() else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    })
}
fn missing_interpreter(name: &str) -> String {
    format!(
        "The REPL relies on node.js to execute JavaScript code outside the browser.\nI could not find executable `{name}` on your PATH though!\n\nYou can install node.js from <http://nodejs.org/>. If it is already installed\nbut has a different name, use the --interpreter flag."
    )
}

// Elm consumes each declared flag once, in declaration order, then reports
// remaining flags before positional arguments. Repeated flags are unknown.
fn parse_flags(mut args: Vec<String>) -> Result<(bool, Option<String>), String> {
    let mut interpreter = None;
    if let Some(index) = args
        .iter()
        .position(|arg| arg == "--interpreter" || arg.starts_with("--interpreter="))
    {
        let arg = args.remove(index);
        interpreter = Some(if let Some(value) = arg.strip_prefix("--interpreter=") {
            value.to_owned()
        } else if args.get(index).is_some_and(|value| !value.starts_with('-')) {
            args.remove(index)
        } else {
            return Err("This flag needs more information:\n\n    --interpreter\n\nIt needs a <interpreter> like this:\n\n    --interpreter=node\n    --interpreter=nodejs\n".into());
        });
    }
    let mut ansi = true;
    if let Some(index) = args
        .iter()
        .position(|arg| arg == "--no-colors" || arg.starts_with("--no-colors="))
    {
        let arg = args.remove(index);
        if arg != "--no-colors" {
            return Err(format!(
                "This on/off flag was given a value:\n\n    {arg}\n\nAn on/off flag either exists or not. It cannot have an equals sign and value.\nMaybe you want this instead?\n\n    --no-colors\n"
            ));
        }
        ansi = false;
    }
    if let Some(arg) = args.iter().find(|arg| arg.starts_with('-')) {
        let name = arg.trim_start_matches('-').split('=').next().unwrap_or("");
        let mut suggestions = [
            (distance(name, "interpreter"), "--interpreter=<interpreter>"),
            (distance(name, "no-colors"), "--no-colors"),
        ]
        .to_vec();
        if suggestions.iter().any(|(distance, _)| *distance < 3) {
            suggestions.retain(|(distance, _)| *distance < 3);
        }
        suggestions.sort_by_key(|(distance, _)| *distance);
        let hint = if suggestions.len() == 1 {
            format!("Maybe you want {} instead?", suggestions[0].1)
        } else {
            format!(
                "Maybe you want one of these instead?\n\n{}",
                suggestions
                    .iter()
                    .map(|(_, flag)| format!("    {flag}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };
        return Err(format!(
            "I do not recognize this flag:\n\n    {arg}\n\n{hint}\n"
        ));
    }
    if !args.is_empty() {
        let (noun, pronoun) = if args.len() == 1 {
            ("this argument", "it")
        } else {
            ("these arguments", "them")
        };
        let list = args
            .iter()
            .map(|arg| format!("    {arg}"))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!(
            "I was not expecting {noun}:\n\n{list}\n\nTry removing {pronoun}?\n"
        ));
    }
    Ok((ansi, interpreter))
}

// Restricted Damerau-Levenshtein distance, as used by Reporting.Suggest.
fn distance(left: &str, right: &str) -> usize {
    let a: Vec<_> = left.chars().collect();
    let b: Vec<_> = right.chars().collect();
    let mut rows = vec![vec![0; b.len() + 1]; a.len() + 1];
    for (i, row) in rows.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, value) in rows[0].iter_mut().enumerate() {
        *value = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            rows[i][j] = (rows[i - 1][j] + 1)
                .min(rows[i][j - 1] + 1)
                .min(rows[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]));
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                rows[i][j] = rows[i][j].min(rows[i - 2][j - 2] + 1);
            }
        }
    }
    rows[a.len()][b.len()]
}
