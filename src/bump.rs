use planexpo_elm::{
    api_diff, bump_diagnostic, diff_diagnostic, package_bump,
    package_network::{PACKAGE_SERVER, PackageNetwork},
    package_solver::Version,
};
use std::{
    env, fs,
    io::{self, IsTerminal, Write},
    path::PathBuf,
};
fn styled(code: u8, text: &str, terminal: bool) -> String {
    if terminal {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.into()
    }
}
pub fn run(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|s| s == "--help") {
        let example = styled(96, "elm bump", io::stderr().is_terminal());
        eprint!(
            "The `bump` command figures out the next version number based on API changes:\n\n    {example}\n\nSay you just published version 1.0.0, but then decided to remove a function. I\nwill compare the published API to what you have locally, figure out that it is a\nMAJOR change, and bump your version number to 2.0.0. I do this with all\npackages, so there cannot be MAJOR changes hiding in PATCH releases in Elm!\n\n"
        );
        return Ok(());
    }
    if let Some(flag) = args.iter().find(|s| s.starts_with('-')) {
        return Err(format!(
            "ELM_CLI_RAW:I do not recognize this flag:\n\n    {}\n\n",
            styled(91, flag, io::stderr().is_terminal())
        ));
    }
    if !args.is_empty() {
        let (these, them) = if args.len() == 1 {
            ("this argument", "it")
        } else {
            ("these arguments", "them")
        };
        return Err(format!(
            "ELM_CLI_RAW:I was not expecting {these}:\n\n    {}\n\nTry removing {them}?\n\n",
            styled(91, &args.join("\n    "), io::stderr().is_terminal())
        ));
    }
    let cwd = env::current_dir().map_err(|e| e.to_string())?;
    let manifest = cwd
        .ancestors()
        .map(|p| p.join("elm.json"))
        .find(|p| p.is_file())
        .ok_or_else(bump_diagnostic::no_outline)?;
    let home = env::var_os("ELM_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".elm")))
        .ok_or("ELM_HOME or HOME is required")?;
    let network = PackageNetwork::new(PACKAGE_SERVER)?;
    let registry = network.latest_registry_with_context(
        &home,
        "I need the latest list of published packages before I can bump any versions",
    )?;
    let original = fs::read(&manifest).map_err(|e| e.to_string())?;
    let outline =
        planexpo_elm::outline::decode_project(std::str::from_utf8(&original).map_err(|e| e.to_string())?)?;
    planexpo_elm::outline::validate(&outline, manifest.parent().unwrap())
        .map_err(|e| e.encode())?;
    if outline["type"] != "package" {
        return Err(bump_diagnostic::application());
    }
    let name = outline["name"].as_str().ok_or("missing package name")?;
    let current = Version::parse(outline["version"].as_str().ok_or("missing version")?)?;
    let target = if let Some(published) = registry.versions(name) {
        let allowed = package_bump::bumpable_versions(published);
        if !allowed.contains(&current) {
            return Err(bump_diagnostic::unexpected_version(current, &allowed));
        }
        let old = network
            .documentation(&home, name, current)
            .map_err(|error| {
                diff_diagnostic::documentation_context(
                    &format!("I need the docs for {current} to compute the next version number"),
                    error,
                )
            })?;
        let selected = planexpo_elm::package_resolution::resolve(&manifest, &home)?;
        planexpo_elm::dependency_build::verify(&manifest, &home, &selected)?;
        let exposed = &outline["exposed-modules"];
        if exposed.as_array().is_some_and(Vec::is_empty)
            || exposed.as_object().is_some_and(|groups| {
                groups
                    .values()
                    .all(|v| v.as_array().is_some_and(Vec::is_empty))
            })
        {
            return Err(bump_diagnostic::no_exposed());
        }
        let entries = planexpo_elm::project::exposed_entries(&manifest)?;
        let graph = planexpo_elm::project::discover_many_selected(
            &manifest,
            &entries,
            &home,
            false,
            Some(&selected),
        )?;
        let new =
            serde_json::Value::Array(planexpo_elm::analyze::documentation(&graph)?.documentation);
        let changes = api_diff::diff(&old, &new)?;
        let magnitude = changes.magnitude();
        let target = magnitude.bump(current);
        let kind = match magnitude {
            api_diff::Magnitude::Patch => "PATCH",
            api_diff::Magnitude::Minor => "MINOR",
            api_diff::Magnitude::Major => "MAJOR",
        };
        print!(
            "Based on your new API, this should be a {} change ({current} => {target})\nBail out of this command and run 'elm diff' for a full explanation.\n\nShould I perform the update ({current} => {target}) in elm.json? [Y/n] ",
            styled(92, kind, io::stdout().is_terminal())
        );
        target
    } else {
        println!("{}", bump_diagnostic::NEW_PACKAGE);
        let initial = Version([1, 0, 0]);
        if current == initial {
            println!("The version number in elm.json is correct so you are all set!");
            return Ok(());
        }
        print!(
            "It looks like the version in elm.json has been changed though!\nWould you like me to change it back to 1.0.0? [Y/n] "
        );
        initial
    };
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
            "" | "y" | "Y" => break,
            "n" => {
                println!("Okay, I did not change anything!");
                return Ok(());
            }
            _ => print!("Must type 'y' for yes or 'n' for no: "),
        }
    }
    package_bump::change_version(&manifest, &original, target)?;
    println!(
        "Version changed to {}!",
        styled(92, &target.to_string(), io::stdout().is_terminal())
    );
    Ok(())
}
