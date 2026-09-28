//! Elm 0.19.1 terminal help documents and dispatch rules (LICENSE-ELM).
use std::io::IsTerminal;
const COMMANDS: &[&str] = &[
    "repl", "init", "reactor", "make", "install", "bump", "diff", "publish",
];

pub fn help_or_version(args: &[String]) -> bool {
    if args == ["--version"] {
        println!("0.19.1");
        return true;
    }
    if args == ["--compiler-version"] {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return true;
    }
    let topic = if args.is_empty() || args == ["--help"] {
        Some("overview")
    } else if COMMANDS.contains(&args[0].as_str()) && args[1..].iter().any(|arg| arg == "--help") {
        Some(args[0].as_str())
    } else {
        None
    };
    let Some(topic) = topic else {
        return false;
    };
    let docs: serde_json::Value =
        serde_json::from_str(include_str!("cli_help.json")).expect("bundled Elm CLI help");
    let mode = if std::io::stderr().is_terminal() {
        "ansi"
    } else {
        "plain"
    };
    eprint!("{}", docs[topic][mode].as_str().expect("known help topic"));
    true
}

pub fn unknown(command: &str) -> String {
    let lower = |text: &str| {
        text.chars()
            .map(|c| c.to_lowercase().next().unwrap())
            .collect::<String>()
    };
    let mut ranked: Vec<_> = COMMANDS
        .iter()
        .map(|name| {
            (
                crate::make_cli::distance(&lower(command), &lower(name)),
                *name,
            )
        })
        .filter(|(distance, _)| *distance <= 3)
        .collect();
    ranked.sort_by_key(|(distance, _)| *distance);
    let ansi = std::io::stderr().is_terminal();
    let color = |code, text: &str| {
        if ansi {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    };
    let word = |text: &str| (text.chars().count(), text.to_owned());
    let mut words = vec![
        word("There"),
        word("is"),
        word("no"),
        (command.chars().count(), color(91, command)),
        word("command."),
    ];
    if !ranked.is_empty() {
        words.push(word("Try"));
        for (index, (_, name)) in ranked.iter().enumerate() {
            if index > 0 && index == ranked.len() - 1 {
                words.push(word("or"));
            }
            let comma = if ranked.len() > 2 && index < ranked.len() - 1 {
                ","
            } else {
                ""
            };
            words.push((
                name.len() + comma.len(),
                format!("{}{comma}", color(92, name)),
            ));
        }
        words.push(word("instead?"));
    }
    let mut text = String::new();
    let mut column = 0;
    for (width, word) in words {
        if column > 0 {
            if column + 1 + width > 80 {
                text.push('\n');
                column = 0;
            } else {
                text.push(' ');
                column += 1;
            }
        }
        text.push_str(&word);
        column += width;
    }
    format!("ELM_CLI_RAW:{text}\n\nRun `elm` with no arguments to get more hints.\n\n")
}

fn styled(code: u8, text: &str) -> String {
    if std::io::stderr().is_terminal() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

pub fn reject_flags(args: &[String]) -> Result<(), String> {
    if let Some(flag) = args.iter().find(|arg| arg.starts_with('-')) {
        Err(format!(
            "ELM_CLI_RAW:I do not recognize this flag:\n\n    {}\n\n",
            styled(91, flag)
        ))
    } else {
        Ok(())
    }
}

pub fn extra_arguments(args: &[String]) -> String {
    let (these, them) = if args.len() == 1 {
        ("this argument", "it")
    } else {
        ("these arguments", "them")
    };
    format!(
        "ELM_CLI_RAW:I was not expecting {these}:\n\n    {}\n\nTry removing {them}?\n\n",
        styled(91, &args.join("\n    "))
    )
}
