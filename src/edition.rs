//! Behaviour that depends on the Elm release an application is pinned to.
//!
//! The 0.19 releases share one language, but their official binaries differ in
//! observable details. An application's `elm-version` selects which binary this
//! compiler follows; packages and the REPL keep the 0.19.1 behaviour.
use crate::package_solver::Version;
use std::{cell::Cell, cmp::Ordering};

thread_local! { static ACTIVE: Cell<Version> = const { Cell::new(Version::ELM) }; }

/// Selects the release followed by the rest of this compilation.
pub(crate) fn select(version: Version) {
    ACTIVE.with(|active| active.set(version));
}

fn active() -> Version {
    ACTIVE.with(Cell::get)
}

/// Official messages link to the guide of the release that printed them.
pub fn localize_links(text: &str) -> std::borrow::Cow<'_, str> {
    const REFERENCE: &str = "https://elm-lang.org/0.19.1/";
    let Version([major, minor, patch]) = active();
    if active() == Version::ELM || !text.contains(REFERENCE) {
        return text.into();
    }
    text.replace(REFERENCE, &format!("https://elm-lang.org/{major}.{minor}.{patch}/")).into()
}

/// The 0.19.1 exhaustiveness report misspells "possibilities"; later releases do not.
pub(crate) fn fixed_possibilities_typo() -> bool {
    active() >= Version([0, 19, 2])
}

/// Elm 0.19.3 compares names by byte length before content (`Bytes.compareFast`),
/// which reorders everything the official compiler reads out of a name-keyed map.
pub(crate) fn length_first_names() -> bool {
    active() >= Version([0, 19, 3])
}

/// Orders `Home.name` strings like a map of homes holding sets of names.
pub(crate) fn sort_qualified(names: &mut [String]) {
    let split = |name: &'_ str| name.rsplit_once('.').map_or((String::new(), name.to_owned()), |(home, name)| (home.to_owned(), name.to_owned()));
    names.sort_by(|left, right| {
        let (left, right) = (split(left), split(right));
        compare_names(&left.0, &right.0).then_with(|| compare_names(&left.1, &right.1))
    });
}

/// Orders per-module reports like the official map of module results.
pub fn sort_module_reports(errors: &mut [serde_json::Value]) {
    errors.sort_by(|left, right| {
        compare_names(left["name"].as_str().unwrap_or(""), right["name"].as_str().unwrap_or(""))
    });
}

/// Order of two names in the official compiler's maps for the selected release.
pub fn compare_names(left: &str, right: &str) -> Ordering {
    if length_first_names() {
        left.len().cmp(&right.len()).then_with(|| left.as_bytes().cmp(right.as_bytes()))
    } else {
        left.cmp(right)
    }
}
