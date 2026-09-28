//! Verify all exposed dependency modules before building project sources.
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::Path,
};

pub fn verify(
    manifest: &Path,
    home: &Path,
    selected: &BTreeMap<String, String>,
) -> Result<(), String> {
    verify_with_progress(manifest, home, selected, false, false)
}

pub fn verify_with_progress(
    manifest: &Path,
    home: &Path,
    selected: &BTreeMap<String, String>,
    terminal: bool,
    downloaded: bool,
) -> Result<(), String> {
    verify_downloads_with_progress(
        manifest,
        home,
        selected,
        terminal,
        downloaded,
        &BTreeMap::new(),
    )
}

pub fn verify_downloads_with_progress(
    manifest: &Path,
    home: &Path,
    selected: &BTreeMap<String, String>,
    terminal: bool,
    downloaded: bool,
    download_failures: &BTreeMap<String, String>,
) -> Result<(), String> {
    let directory = manifest
        .parent()
        .ok_or("missing project root")?
        .join("elm-stuff/planexpo-rust");
    let _lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("dependencies-built.lock"))
        .and_then(|file| file.lock().map(|_| file))
        .ok();
    let identity = crate::cache::running_compiler_identity();
    let resolution = fs::read(directory.join("dependencies-v1.json")).ok();
    let key = json!({"schema":1,"compiler":identity,"resolution":resolution});
    let stamp = directory.join("dependencies-built-v1.json");
    let cacheable = download_failures.is_empty() && identity.is_some() && resolution.is_some();
    if cacheable
        && fs::read(&stamp)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .as_ref()
            == Some(&key)
    {
        return Ok(());
    }
    let total = selected.len();
    let mut first = true;
    let mut progress = |completed: usize| {
        if terminal {
            let separator = if first && downloaded { '\n' } else { '\r' };
            print!("{separator}Verifying dependencies ({completed}/{total})");
            let _ = std::io::stdout().flush();
        }
        first = false;
    };
    progress(0);
    let mut done: BTreeMap<_, _> = download_failures
        .iter()
        .map(|(name, error)| (name.clone(), Err(error.clone())))
        .collect();
    if !done.is_empty() {
        progress(done.len());
    }
    let mut active = BTreeSet::new();
    // Download failures take precedence over independent compilation failures.
    let mut failure = download_failures.values().next().cloned();
    for name in selected.keys() {
        if let Err(error) = verify_one(name, home, selected, &mut done, &mut active, &mut progress)
        {
            failure.get_or_insert(error);
        }
    }
    if terminal {
        let width = format!("Verifying dependencies ({total}/{total})").len();
        println!(
            "\r{}\r{}",
            " ".repeat(width),
            if failure.is_some() {
                "Dependency problem!"
            } else {
                "Dependencies ready!"
            }
        );
    }
    if let Some(error) = failure {
        return Err(error);
    }
    if cacheable {
        // The verification cache is optional, just like the output cache.
        if let Ok(mut file) = tempfile::NamedTempFile::new_in(&directory)
            && serde_json::to_writer(&mut file, &key).is_ok()
            && file.flush().is_ok()
        {
            let _ = file.persist(stamp);
        }
    }
    Ok(())
}

fn verify_one(
    name: &str,
    home: &Path,
    selected: &BTreeMap<String, String>,
    done: &mut BTreeMap<String, Result<(), String>>,
    active: &mut BTreeSet<String>,
    progress: &mut dyn FnMut(usize),
) -> Result<(), String> {
    if let Some(result) = done.get(name) {
        return result.clone();
    }
    if !active.insert(name.to_owned()) {
        return Err("cyclic package dependencies".into());
    }
    let result = verify_package(name, home, selected, done, active, progress);
    active.remove(name);
    done.insert(name.to_owned(), result.clone());
    progress(done.len());
    result
}

fn verify_package(
    name: &str,
    home: &Path,
    selected: &BTreeMap<String, String>,
    done: &mut BTreeMap<String, Result<(), String>>,
    active: &mut BTreeSet<String>,
    progress: &mut dyn FnMut(usize),
) -> Result<(), String> {
    let version = selected.get(name).ok_or("missing selected dependency")?;
    let manifest = home
        .join("0.19.1/packages")
        .join(name)
        .join(version)
        .join("elm.json");
    let source = fs::read_to_string(&manifest).map_err(|error| error.to_string())?;
    let config = crate::outline::decode(&source)?;
    let dependencies = config["dependencies"]
        .as_object()
        .ok_or("invalid package dependencies")?;
    let direct: BTreeMap<_, _> = dependencies
        .keys()
        .filter_map(|name| {
            selected
                .get(name)
                .map(|version| (name.clone(), version.clone()))
        })
        .collect();
    for dependency in direct.keys() {
        verify_one(dependency, home, selected, done, active, progress)?;
    }
    let mut closure = direct.clone();
    let mut pending: Vec<_> = direct.keys().cloned().collect();
    while let Some(dependency) = pending.pop() {
        let dep_manifest = home
            .join("0.19.1/packages")
            .join(&dependency)
            .join(&selected[&dependency])
            .join("elm.json");
        let config =
            crate::outline::decode(&fs::read_to_string(dep_manifest).map_err(|e| e.to_string())?)?;
        if let Some(deps) = config["dependencies"].as_object() {
            for name in deps.keys() {
                if !closure.contains_key(name) {
                    closure.insert(
                        name.clone(),
                        selected
                            .get(name)
                            .ok_or("missing transitive dependency")?
                            .clone(),
                    );
                    pending.push(name.clone());
                }
            }
        }
    }
    let build = || -> Result<(), String> {
        crate::outline::validate(&config, manifest.parent().unwrap()).map_err(|e| e.encode())?;
        let entries = crate::project::dependency_exposed_entries(&manifest)?;
        if entries.is_empty() {
            return Ok(());
        }
        let graph = crate::project::discover_many_selected(
            &manifest,
            &entries,
            home,
            false,
            Some(&closure),
        )?;
        let report = crate::analyze::generate_modules(&graph)?;
        if !report.generation_errors.is_empty() {
            return Err(report.generation_errors.join("\n"));
        }
        Ok(())
    };
    build().map_err(|_| crate::dependency_error::bad_build(name, version, &direct))?;
    Ok(())
}
