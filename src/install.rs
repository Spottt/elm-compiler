//! Interactive installation; project writes happen only after dependency verification.
use planexpo_elm::{
    installation::{self, Change, PlanKind},
    package_network::{PACKAGE_SERVER, PackageNetwork},
};
use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
};

fn report(title: &str, message: &str) -> String {
    format!(
        "ELM_DEPENDENCY_JSON:{}",
        serde_json::json!({"type":"error","path":null,"title":title,"message":[message]})
    )
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    if args.as_slice() == ["--help"] {
        eprintln!(
            "The `install` command fetches packages from <https://package.elm-lang.org> for\nuse in your project:\n\n    elm install\n    elm install <package>\n\nFor example, if you want to get packages for HTTP and JSON, you would say:\n\n    elm install elm/http\n    elm install elm/json\n\nNotice that you must say the AUTHOR name and PROJECT name! After running those\ncommands, you could say `import Http` or `import Json.Decode` in your code.\n\nWhat if two projects use different versions of the same package? No problem!\nEach project is independent, so there cannot be conflicts like that!\n"
        );
        return Ok(());
    }
    crate::global_cli::reject_flags(&args)?;
    if let Some(package) = args.first()
        && !planexpo_elm::package_solver::valid_name(package)
    {
        return Err(crate::diff::argument_error(package, true)?);
    }
    if args.len() > 1 {
        // The zero-argument command shape is tried first by Terminal.Chomp.
        // Malformed package arguments outrank it; otherwise all extras remain.
        return Err(crate::global_cli::extra_arguments(&args));
    }
    let cwd = env::current_dir().map_err(|e| e.to_string())?;
    let root = cwd.ancestors().find(|p| p.join("elm.json").is_file()).ok_or_else(|| report("NEW PROJECT?", "Are you trying to start a new project? Try this command instead:\n\n    elm init\n\nIt will help you get started!"))?;
    let package = args.first().ok_or_else(|| report("INSTALL WHAT?", "I am expecting commands like:\n\n    elm install elm/http\n    elm install elm/json\n    elm install elm/random"))?;
    let path = root.join("elm.json");
    let home = env::var_os("ELM_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".elm")))
        .ok_or("ELM_HOME or HOME is required")?;
    let network = PackageNetwork::new(PACKAGE_SERVER)?;
    let state = network.registry(&home)?;
    let connection = state.online.then_some(&network);
    let original = fs::read(&path).map_err(|e| e.to_string())?;
    let outline =
        planexpo_elm::outline::decode(std::str::from_utf8(&original).map_err(|e| e.to_string())?)?;
    let plan = installation::plan(&home, &outline, package, connection)?;
    match plan.kind {
        PlanKind::AlreadyInstalled => {
            println!("It is already installed!");
            return Ok(());
        }
        PlanKind::PromoteIndirect => print!(
            "I found it in your elm.json file, but in the \"indirect\" dependencies.\nShould I move it into \"direct\" dependencies for more general use? [Y/n]: "
        ),
        PlanKind::PromoteTest => print!(
            "I found it in your elm.json file, but in the \"test-dependencies\" field.\nShould I move it into \"dependencies\" for more general use? [Y/n]: "
        ),
        PlanKind::Changes => {
            println!("Here is my plan:");
            let mut changes: Vec<_> = plan.changes.iter().collect();
            changes.sort_by(|(a, _), (b, _)| planexpo_elm::registry::compare_names(a, b));
            let width = changes
                .iter()
                .map(|(name, _)| name.len())
                .max()
                .unwrap_or(0)
                + 3;
            let left = changes
                .iter()
                .map(|(_, change)| match change {
                    Change::Insert(v) | Change::Remove(v) => v.len(),
                    Change::Replace { old, .. } => old.len(),
                })
                .max()
                .unwrap_or(0);
            let right = changes
                .iter()
                .map(|(_, change)| match change {
                    Change::Replace { new, .. } => new.len(),
                    _ => 0,
                })
                .max()
                .unwrap_or(0);
            for section in ["Add:", "Change:", "Remove:"] {
                let mut started = false;
                for (name, change) in &changes {
                    let relevant = matches!(
                        (section, change),
                        ("Add:", Change::Insert(_))
                            | ("Change:", Change::Replace { .. })
                            | ("Remove:", Change::Remove(_))
                    );
                    if !relevant {
                        continue;
                    }
                    if !started {
                        println!("  \n  {section}");
                        started = true;
                    }
                    match change {
                        Change::Insert(v) | Change::Remove(v) => {
                            println!("    {name:<width$} {v:>left$}")
                        }
                        Change::Replace { old, new } => {
                            println!("    {name:<width$} {old:>left$} => {new:>right$}")
                        }
                    }
                }
            }
            print!("\nWould you like me to update your elm.json accordingly? [Y/n]: ");
        }
    }
    loop {
        io::stdout().flush().map_err(|e| e.to_string())?;
        let mut answer = String::new();
        if io::stdin()
            .read_line(&mut answer)
            .map_err(|e| e.to_string())?
            == 0
        {
            return Err(crate::diagnostic::confirmation_eof());
        }
        match answer.strip_suffix('\n').unwrap_or(&answer) {
            "" | "Y" | "y" => break,
            "n" => {
                println!("Okay, I did not change anything!");
                return Ok(());
            }
            _ => print!("Must type 'y' for yes or 'n' for no: "),
        }
    }
    installation::apply(&home, &path, &original, &plan, connection)?;
    println!("Success!");
    Ok(())
}
