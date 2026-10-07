use planexpo_elm::{
    package_network::{PACKAGE_SERVER, PackageNetwork},
    package_publish::{self as publication, VersionPlan},
    package_solver::Version,
    publish_diagnostic as diagnostic,
    publish_git::Git,
    publish_network::PublicationNetwork,
};
use std::{
    env, fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};
fn styled(code: u8, text: &str, terminal: bool) -> String {
    if terminal {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.into()
    }
}
fn check<T>(
    waiting: &str,
    success: impl FnOnce(&T) -> String,
    failure: &str,
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let terminal = io::stdout().is_terminal();
    let (wait, good, bad) = if cfg!(windows) {
        ("-", "+", "X")
    } else {
        ("→", "●", "✗")
    };
    print!("  {} {waiting}", styled(33, wait, terminal));
    io::stdout().flush().map_err(|e| e.to_string())?;
    let result = work();
    let (mark, message, ending) = match &result {
        Ok(value) => (styled(92, good, terminal), success(value), "\n"),
        Err(_) => (styled(91, bad, terminal), failure.into(), "\n\n"),
    };
    print!(
        "\r  {mark} {message}{}{ending}",
        " ".repeat(
            waiting
                .chars()
                .count()
                .saturating_sub(message.chars().count())
        )
    );
    io::stdout().flush().map_err(|e| e.to_string())?;
    result
}
fn simple<T>(
    waiting: &str,
    success: &str,
    failure: &str,
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    check(waiting, |_| success.into(), failure, work)
}
fn validation(error: publication::Problem) -> String {
    diagnostic::problem(&error).unwrap_or_else(|| format!("{error:?}"))
}
fn git_error(error: planexpo_elm::publish_git::Error) -> String {
    diagnostic::git(&error).unwrap_or_else(|| format!("{error:?}"))
}
pub fn run(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|a| a == "--help") {
        let command = styled(96, "elm publish", io::stderr().is_terminal());
        eprint!(
            "The `publish` command publishes your package on <https://package.elm-lang.org>\nso that anyone in the Elm community can use it.\n\n    {command}\n\nThink hard if you are ready to publish NEW packages though!\n\nPart of what makes Elm great is the packages ecosystem. The fact that there is\nusually one option (usually very well done) makes it way easier to pick packages\nand become productive. So having a million packages would be a failure in Elm.\nWe do not need twenty of everything, all coded in a single weekend.\n\nSo as community members gain wisdom through experience, we want them to share\nthat through thoughtful API design and excellent documentation. It is more about\nsharing ideas and insights than just sharing code! The first step may be asking\nfor advice from people you respect, or in community forums. The second step may\nbe using it at work to see if it is as nice as you think. Maybe it ends up as an\nexperiment on GitHub only. Point is, try to be respectful of the community and\npackage ecosystem!\n\nCheck out <https://package.elm-lang.org/help/design-guidelines> for guidance on\nhow to create great packages!\n\n"
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
        let (what, them) = if args.len() == 1 {
            ("this argument", "it")
        } else {
            ("these arguments", "them")
        };
        return Err(format!(
            "ELM_CLI_RAW:I was not expecting {what}:\n\n    {}\n\nTry removing {them}?\n\n",
            styled(91, &args.join("\n    "), io::stderr().is_terminal())
        ));
    }
    let cwd = env::current_dir().map_err(|e| e.to_string())?;
    let manifest = cwd
        .ancestors()
        .map(|p| p.join("elm.json"))
        .find(|p| p.is_file())
        .ok_or_else(diagnostic::no_outline)?;
    let home = env::var_os("ELM_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".elm")))
        .ok_or("ELM_HOME or HOME is required")?;
    execute(
        &manifest,
        &home,
        &PackageNetwork::new(PACKAGE_SERVER)?,
        &PublicationNetwork::official()?,
    )
}
fn execute(
    manifest: &Path,
    home: &Path,
    packages: &PackageNetwork,
    network: &PublicationNetwork,
) -> Result<(), String> {
    let root = manifest.parent().ok_or("missing package root")?;
    let registry = packages.latest_registry_with_context(
        home,
        "I need the latest list of published packages to make sure this is safe to publish",
    )?;
    let outline =
        planexpo_elm::outline::decode_project(&fs::read_to_string(manifest).map_err(|e| e.to_string())?)?;
    planexpo_elm::outline::validate(&outline, root).map_err(|e| e.encode())?;
    if outline["type"] == "application" {
        return Err(validation(publication::Problem::Application));
    }
    let name = outline["name"].as_str().ok_or("missing package name")?;
    let version = Version::parse(outline["version"].as_str().ok_or("missing version")?)?;
    let published = registry.versions(name);
    if published.is_some() {
        println!("Verifying {name} {version} ...\n");
    } else {
        println!(
            "{}\nI will now verify that everything is in order...\n",
            planexpo_elm::bump_diagnostic::NEW_PACKAGE
        );
    }
    publication::check_description(&outline).map_err(validation)?;
    simple(
        "Looking for README.md",
        "Found README.md",
        "Problem with your README.md",
        || publication::check_readme(root).map_err(validation),
    )?;
    simple(
        "Looking for LICENSE",
        "Found LICENSE",
        "Problem with your LICENSE",
        || publication::check_license(root).map_err(validation),
    )?;
    let docs = simple(
        "Verifying documentation...",
        "Verified documentation",
        "Problem with documentation",
        || publication::build_documentation(manifest, home),
    )?;
    check(
        &format!("Checking semantic versioning rules. Is {version} correct?"),
        |plan| match plan {
            VersionPlan::Initial => "All packages start at version 1.0.0".into(),
            VersionPlan::Compare(candidate) => format!(
                "Version number {version} verified ({} change, {} => {version})",
                match candidate.magnitude {
                    planexpo_elm::api_diff::Magnitude::Patch => "PATCH",
                    planexpo_elm::api_diff::Magnitude::Minor => "MINOR",
                    planexpo_elm::api_diff::Magnitude::Major => "MAJOR",
                },
                candidate.old
            ),
        },
        &format!("Version {version} is not correct!"),
        || {
            publication::verify_version(
                packages,
                home,
                name,
                version,
                published.unwrap_or(&[]),
                &docs,
            )
        },
    )?;
    let git = Git::find(env::var_os("PATH").as_deref()).map_err(git_error)?;
    let commit = simple(
        &format!("Is version {version} tagged on GitHub?"),
        &format!("Version {version} is tagged on GitHub"),
        &format!("Version {version} is not tagged on GitHub!"),
        || {
            git.verify_tag(root, version).map_err(git_error)?;
            network
                .tag(name, version)
                .map_err(|e| diagnostic::network(e, version, "tag"))
        },
    )?;
    simple(
        "Checking for uncommitted changes...",
        "No uncommitted changes in local code",
        "Your local code is different than the code tagged on GitHub",
        || {
            git.verify_changes(root, &commit, version)
                .map_err(git_error)
        },
    )?;
    // Each attempt owns its directory, preventing stale files from contributing
    // to verification and ensuring cleanup on every failure path.
    let stuff = root.join("elm-stuff/0.19.1");
    fs::create_dir_all(&stuff).map_err(|e| e.to_string())?;
    let staging = tempfile::Builder::new()
        .prefix("prepublish-")
        .tempdir_in(&stuff)
        .map_err(|e| e.to_string())?;
    let archive = simple(
        "Downloading code from GitHub...",
        "Code downloaded successfully from GitHub",
        "Could not download code from GitHub!",
        || {
            let archive = network
                .archive(name, version)
                .map_err(|e| diagnostic::network(e, version, "archive"))?;
            archive
                .extract(staging.path())
                .map_err(|e| diagnostic::network(e, version, "archive"))?;
            Ok(archive)
        },
    )?;
    simple(
        "Verifying downloaded code...",
        "Downloaded code compiles successfully",
        "Cannot compile downloaded code!",
        || {
            let manifest = staging.path().join("elm.json");
            publication::build_documentation(&manifest, home)
                .map(|_| ())
                .map_err(|_| diagnostic::bad_archive_build())
        },
    )?;
    drop(staging);
    println!();
    network
        .register(root, name, version, &commit, &docs, &archive)
        .map_err(|e| diagnostic::network(e, version, "register"))?;
    println!("Success!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Cursor, Write},
        process::Command,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };
    fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    struct Server {
        url: String,
        stop: Arc<AtomicBool>,
        requests: Arc<Mutex<Vec<String>>>,
        thread: Option<thread::JoinHandle<()>>,
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
        }
    }
    fn server(commit: String, archive: Vec<u8>) -> Server {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let finished = stop.clone();
        let observed = requests.clone();
        let thread = thread::spawn(move || {
            while !finished.load(Ordering::Relaxed) {
                let Some(mut request) = server.recv_timeout(Duration::from_millis(100)).unwrap()
                else {
                    continue;
                };
                let url = request.url().to_owned();
                observed.lock().unwrap().push(url.clone());
                let mut bytes = Vec::new();
                request.as_reader().read_to_end(&mut bytes).unwrap();
                let body = if url == "/all-packages" {
                    br#"{"elm/core":["1.0.0"]}"#.to_vec()
                } else if url.starts_with("/all-packages/since/") {
                    b"[]".to_vec()
                } else if url == "/repos/elm/core/git/refs/tags/1.0.1" {
                    serde_json::to_vec(&serde_json::json!({"object":{"sha":commit}})).unwrap()
                } else if url == "/elm/core/zipball/1.0.1/" {
                    archive.clone()
                } else if url.starts_with("/register?") {
                    assert_eq!(request.method(), &tiny_http::Method::Post);
                    let body = String::from_utf8(bytes).unwrap();
                    assert!(body.contains("identity"));
                    assert!(body.contains("github-hash"));
                    b"success".to_vec()
                } else {
                    panic!("unexpected request {url}")
                };
                request
                    .respond(tiny_http::Response::from_data(body))
                    .unwrap();
            }
        });
        Server {
            url,
            stop,
            requests,
            thread: Some(thread),
        }
    }
    fn scenario(broken_archive: bool) {
        let project = tempfile::tempdir().unwrap();
        let root = project.path();
        fs::create_dir(root.join("src")).unwrap();
        let outline = serde_json::json!({"type":"package","name":"elm/core","summary":"Local publication protocol fixture","license":"BSD-3-Clause","version":"1.0.1","exposed-modules":["Example"],"elm-version":"0.19.0 <= v < 0.20.0","dependencies":{},"test-dependencies":{}});
        let manifest = serde_json::to_vec(&outline).unwrap();
        fs::write(root.join("elm.json"), &manifest).unwrap();
        let source = "module Example exposing (identity)\n{-| Example\n@docs identity\n-}\n{-| Identity -}\nidentity : a -> a\nidentity x = x\n";
        fs::write(root.join("src/Example.elm"), source).unwrap();
        fs::write(root.join("README.md"), "x".repeat(300)).unwrap();
        fs::write(root.join("LICENSE"), "").unwrap();
        git(root, &["init", "--quiet"]);
        git(root, &["add", "elm.json", "README.md", "LICENSE", "src"]);
        git(
            root,
            &[
                "-c",
                "user.name=Publish Test",
                "-c",
                "user.email=publish@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ],
        );
        git(root, &["tag", "1.0.1"]);
        let commit = git(root, &["rev-parse", "HEAD"]);
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.add_directory("archive/", options).unwrap();
        zip.start_file("archive/elm.json", options).unwrap();
        zip.write_all(&manifest).unwrap();
        zip.start_file("archive/src/Example.elm", options).unwrap();
        zip.write_all(if broken_archive {
            b"module Example exposing (identity)\nidentity =\n"
        } else {
            source.as_bytes()
        })
        .unwrap();
        let server = server(commit, zip.finish().unwrap().into_inner());
        let home = tempfile::tempdir().unwrap();
        let cache = home.path().join("0.19.1/packages/elm/core/1.0.0");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("docs.json"),serde_json::to_vec(&serde_json::json!([{"name":"Example","comment":"","unions":[],"aliases":[],"binops":[],"values":[{"name":"identity","comment":"","type":"a -> a"}]}])).unwrap()).unwrap();
        let result = execute(
            &root.join("elm.json"),
            home.path(),
            &PackageNetwork::new(&server.url).unwrap(),
            &PublicationNetwork::with_endpoints(&server.url, &server.url, &server.url).unwrap(),
        );
        let registrations = server
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|url| url.starts_with("/register?"))
            .count();
        if broken_archive {
            assert!(result.unwrap_err().contains("PROBLEM VERIFYING PACKAGE"));
            assert_eq!(registrations, 0);
        } else {
            result.unwrap();
            assert_eq!(registrations, 1);
        }
        assert!(
            !fs::read_dir(root.join("elm-stuff/0.19.1"))
                .unwrap()
                .any(|entry| entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("prepublish-"))
        );
    }
    #[test]
    fn complete_publication_uses_only_local_test_endpoints() {
        scenario(false);
    }
    #[test]
    fn downloaded_build_failure_prevents_registration_and_cleans_staging() {
        scenario(true);
    }
}
