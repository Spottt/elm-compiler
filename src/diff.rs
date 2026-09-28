use planexpo_elm::{
    api_diff, api_diff_render, diff_diagnostic,
    package_network::{PACKAGE_SERVER, PackageNetwork},
    package_solver::{Version, valid_name},
};
use std::{env, fs, io::IsTerminal, path::PathBuf};
fn styled(code: u8, text: &str) -> String {
    if std::io::stderr().is_terminal() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.into()
    }
}
// Terminal.Chomp tries all four command shapes. If none match, the first
// malformed argument outranks missing or extra arguments, even in a later shape.
type ParsedArgs<'a> = (Option<&'a str>, Vec<Version>);
fn parse_args(args: &[String]) -> Result<ParsedArgs<'_>, (&str, bool)> {
    if args.len() <= 2 {
        let versions: Result<Vec<_>, _> = args.iter().map(|s| Version::parse(s)).collect();
        if let Ok(versions) = versions {
            return Ok((None, versions));
        }
    }
    if let [name, old, new] = args
        && valid_name(name)
        && let (Ok(old), Ok(new)) = (Version::parse(old), Version::parse(new))
    {
        return Ok((Some(name), vec![old, new]));
    }
    for arg in args.iter().take(2) {
        if Version::parse(arg).is_err() {
            return Err((arg, false));
        }
    }
    Err((&args[0], true))
}
fn version_examples(arg: &str) -> Vec<String> {
    let mut parts: Vec<_> = arg.split('.').collect();
    if parts
        .iter()
        .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
    {
        parts.resize(3, "0");
        vec![parts.join(".")]
    } else {
        vec!["1.0.0".into(), "2.0.3".into()]
    }
}
fn home() -> Result<PathBuf, String> {
    env::var_os("ELM_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".elm")))
        .ok_or("ELM_HOME or HOME is required".into())
}
pub(crate) fn argument_error(arg: &str, package: bool) -> Result<String, String> {
    let examples = if package {
        let home = home()?;
        fs::create_dir_all(home.join("0.19.1/packages")).map_err(|e| e.to_string())?;
        match planexpo_elm::registry::Registry::read_cached(&home)? {
            Some(registry) => registry.argument_examples(arg),
            None => vec!["elm/json".into(), "elm/http".into(), "elm/random".into()],
        }
    } else {
        version_examples(arg)
    };
    let kind = if package { "package" } else { "version" };
    let description = if examples.len() == 1 {
        "this"
    } else {
        "one of these"
    };
    let examples = styled(92, &examples.join("\n    "));
    let arg = styled(91, arg);
    let token = styled(93, &format!("<{kind}>"));
    Ok(format!(
        "ELM_CLI_RAW:I am having trouble with this argument:\n\n    {arg}\n\nIt is supposed to be a {token} value, like {description}:\n\n    {examples}\n\n"
    ))
}
pub fn run(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|s| s == "--help") {
        let usage = styled(
            96,
            "elm diff\n    elm diff <version>\n    elm diff <version> <version>\n    elm diff <package> <version> <version>",
        );
        let example = styled(92, "elm diff elm/html 1.0.0 2.0.0");
        eprint!(
            "The `diff` command detects API changes:\n\n    {usage}\n\nFor example, to see what changed in the HTML package between versions 1.0.0 and\n2.0.0, you can say:\n\n    {example}\n\nSometimes a MAJOR change is not actually very big, so this can help you plan\nyour upgrade timelines.\n\n"
        );
        return Ok(());
    }
    if let Some(flag) = args.iter().find(|arg| arg.starts_with('-')) {
        let flag = styled(91, flag);
        return Err(format!(
            "ELM_CLI_RAW:I do not recognize this flag:\n\n    {flag}\n\n"
        ));
    }
    let (explicit, parsed) = match parse_args(&args) {
        Ok(parsed) => parsed,
        Err((arg, package)) => return Err(argument_error(arg, package)?),
    };
    let cwd = env::current_dir().map_err(|e| e.to_string())?;
    let manifest = cwd
        .ancestors()
        .map(|p| p.join("elm.json"))
        .find(|p| p.is_file());
    let home = home()?;
    let network = PackageNetwork::new(PACKAGE_SERVER)?;
    let registry = network.latest_registry(&home)?;
    let config = if explicit.is_none() {
        let path = manifest.as_ref().ok_or_else(diff_diagnostic::no_outline)?;
        let value =
            planexpo_elm::outline::decode(&fs::read_to_string(path).map_err(|e| e.to_string())?)?;
        planexpo_elm::outline::validate(&value, path.parent().unwrap()).map_err(|e| e.encode())?;
        if value["type"] != "package" {
            return Err(diff_diagnostic::application());
        }
        Some(value)
    } else {
        None
    };
    let name = explicit
        .or_else(|| config.as_ref()?.get("name")?.as_str())
        .ok_or("missing package name")?;
    let known = registry.versions(name).ok_or_else(|| {
        if explicit.is_some() {
            diff_diagnostic::unknown_package(name, &registry.nearby_names(name))
        } else {
            diff_diagnostic::unpublished()
        }
    })?;
    let mut requested = parsed.clone();
    requested.sort();
    for version in &requested {
        if !known.contains(version) {
            return Err(diff_diagnostic::unknown_version(*version, known));
        }
    }
    let (old, new) = if parsed.len() == 2 {
        let a = parsed[0].min(parsed[1]);
        let b = parsed[0].max(parsed[1]);
        (
            network
                .documentation(&home, name, a)
                .map_err(|e| diff_diagnostic::documentation(a, e))?,
            network
                .documentation(&home, name, b)
                .map_err(|e| diff_diagnostic::documentation(b, e))?,
        )
    } else {
        let version = parsed
            .first()
            .copied()
            .unwrap_or(*known.first().ok_or("no published versions")?);
        let old = network
            .documentation(&home, name, version)
            .map_err(|e| diff_diagnostic::documentation(version, e))?;
        let manifest = manifest.as_ref().ok_or("missing local package")?;
        let selected = planexpo_elm::package_resolution::resolve(manifest, &home)?;
        planexpo_elm::dependency_build::verify(manifest, &home, &selected)?;
        let exposed = &config.as_ref().ok_or("missing local package outline")?["exposed-modules"];
        if exposed.as_array().is_some_and(Vec::is_empty)
            || exposed.as_object().is_some_and(|groups| {
                groups
                    .values()
                    .all(|v| v.as_array().is_some_and(Vec::is_empty))
            })
        {
            return Err(diff_diagnostic::no_exposed());
        }
        let entries = planexpo_elm::project::exposed_entries(manifest)?;
        let graph = planexpo_elm::project::discover_many_selected(
            manifest,
            &entries,
            &home,
            false,
            Some(&selected),
        )?;
        let docs = planexpo_elm::analyze::documentation(&graph)?.documentation;
        (old, serde_json::Value::Array(docs))
    };
    print!(
        "{}",
        api_diff_render::render_terminal(
            &api_diff::diff(&old, &new)?,
            std::io::stdout().is_terminal()
        )?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn argument_shapes_and_error_priority_follow_terminal_chomp() {
        for (args, expected) in [
            (vec!["elm/core", "1.0.0", "bad"], ("elm/core", false)),
            (vec!["1.0.0", "bad", "3.0.0"], ("bad", false)),
            (vec!["1.0.0", "2.0.0", "3.0.0"], ("1.0.0", true)),
        ] {
            let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
            assert_eq!(parse_args(&args).unwrap_err(), expected);
        }
        for args in [
            vec![],
            vec!["1.0.0"],
            vec!["1.0.0", "2.0.0"],
            vec!["elm/core", "1.0.0", "2.0.0"],
        ] {
            assert!(parse_args(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_ok());
        }
        assert_eq!(version_examples("1.2.3.4"), vec!["1.2.3"]);
        assert_eq!(version_examples("2"), vec!["2.0.0"]);
    }
}
