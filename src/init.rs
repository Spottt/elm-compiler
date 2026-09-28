//! Interactive project creation, following Elm 0.19.1 terminal/src/Init.hs.
use planexpo_elm::{
    package_network::{PACKAGE_SERVER, PackageNetwork},
    package_solver,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, IsTerminal, Write},
    path::PathBuf,
};

const QUESTION: &str = "Hello! Elm projects always start with an elm.json file. I can create them!\n\nNow you may be wondering, what will be in this file? How do I add Elm files to\nmy project? How do I see it in the browser? How will my code grow? Do I need\nmore directories? What about tests? Etc.\n\nCheck out <https://elm-lang.org/0.19.1/init> for all the answers!\n\nKnowing all that, would you like me to create an elm.json file now? [Y/n]: ";

fn report(title: &str, message: Vec<Value>) -> String {
    format!(
        "ELM_DEPENDENCY_JSON:{}",
        json!({"type":"error","path":null,"title":title,"message":message})
    )
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    if args.as_slice() == ["--help"] {
        eprintln!(
            "The `init` command helps start Elm projects:\n\n    elm init\n\nIt will ask permission to create an elm.json file, the one thing common to all\nElm projects. It also provides a link explaining what to do from there.\n"
        );
        return Ok(());
    }
    crate::global_cli::reject_flags(&args)?;
    if !args.is_empty() {
        return Err(crate::global_cli::extra_arguments(&args));
    }
    if std::path::Path::new("elm.json").is_file() {
        return Err(report("EXISTING PROJECT", vec![
            json!("You already have an elm.json file, so there is nothing for me to initialize!\n\nMaybe "),
            json!({"bold":false,"underline":false,"color":"GREEN","string":"<https://elm-lang.org/0.19.1/init>"}),
            json!(" can help you figure out what to do\nnext?"),
        ]));
    }
    if io::stdout().is_terminal() {
        print!("{}", QUESTION
            .replacen("elm.json", "\x1b[92melm.json\x1b[0m", 1)
            .replace("<https://elm-lang.org/0.19.1/init>", "\x1b[96m<https://elm-lang.org/0.19.1/init>\x1b[0m"));
    } else {
        print!("{QUESTION}");
    }
    loop {
        io::stdout().flush().map_err(|e| e.to_string())?;
        let mut input = String::new();
        if io::stdin()
            .read_line(&mut input)
            .map_err(|e| e.to_string())?
            == 0
        {
            return Err(crate::diagnostic::confirmation_eof());
        }
        let answer = input.strip_suffix('\n').unwrap_or(&input);
        match answer {
            "" | "y" | "Y" => break,
            "n" => {
                println!("Okay, I did not make any changes!");
                return Ok(());
            }
            _ => print!("Must type 'y' for yes or 'n' for no: "),
        }
    }
    let home = env::var_os("ELM_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".elm")))
        .ok_or("ELM_HOME or HOME is required")?;
    let network = PackageNetwork::new(PACKAGE_SERVER)?;
    let state = network.registry(&home)?;
    let defaults = json!({"elm/core":"1.0.0 <= v <= 65535.0.0","elm/browser":"1.0.0 <= v <= 65535.0.0","elm/html":"1.0.0 <= v <= 65535.0.0"});
    let empty = json!({});
    let selected = if state.online {
        package_solver::solve_online(&home, &defaults, &empty, &network)
    } else {
        package_solver::solve(&home, &defaults, &empty)
    }.map_err(|error| {
        let diagnostic = planexpo_elm::dependency_error::report_encoded(&error);
        let title = diagnostic.as_ref().and_then(|r| r["title"].as_str());
        if matches!(title, Some("INCOMPATIBLE DEPENDENCIES" | "TROUBLE VERIFYING DEPENDENCIES")) {
            let (title, detail) = if state.online {
                ("NO SOLUTION", "I could not find compatible versions though! This should not happen, so please\nask around one of the community forums at https://elm-lang.org/community to learn\nwhat is going on!")
            } else {
                ("NO OFFLINE SOLUTION", "I could not find compatible versions though, but that may be because I could not\nconnect to https://package.elm-lang.org to get the latest list of packages. Are\nyou able to connect to the internet? Please ask around one of the community\nforums at https://elm-lang.org/community for help!")
            };
            report(title, vec![
                json!("I tried to create an elm.json with the following direct dependencies:\n\n    "),
                json!({"bold":false,"underline":false,"color":"yellow","string":"elm/browser"}),
                json!("\n    "),
                json!({"bold":false,"underline":false,"color":"yellow","string":"elm/core"}),
                json!("\n    "),
                json!({"bold":false,"underline":false,"color":"yellow","string":"elm/html"}),
                json!(format!("\n\n{detail}")),
            ])
        } else { error }
    })?;
    let (direct, indirect): (BTreeMap<_, _>, BTreeMap<_, _>) = selected
        .into_iter()
        .partition(|(name, _)| defaults.get(name).is_some());
    let config = json!({"type":"application","source-directories":["src"],"elm-version":"0.19.1",
        "dependencies":{"direct":direct,"indirect":indirect},"test-dependencies":{"direct":{},"indirect":{}}});
    // Prepare the complete file before publishing it; a concurrent init cannot
    // overwrite an elm.json created after the initial existence check.
    let mut temp = tempfile::NamedTempFile::new_in(".").map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
    serde::Serialize::serialize(&config, &mut serializer).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    fs::create_dir_all("src").map_err(|e| e.to_string())?;
    temp.persist_noclobber("elm.json")
        .map_err(|e| e.to_string())?;
    println!("Okay, I created it. Now read that link!");
    Ok(())
}
