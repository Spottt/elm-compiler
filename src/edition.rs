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

/// Elm 0.19.2 corrected several misspelled or ungrammatical 0.19.1 messages.
pub(crate) fn corrected_wording() -> bool {
    active() >= Version([0, 19, 2])
}

/// Elm 0.19.2 and later were built with a GHC whose character predicates follow Unicode 15.1.
pub(crate) fn unicode_15_1() -> bool {
    active() >= Version([0, 19, 2])
}

/// Explanation carried by the located error for a source file that is not valid UTF-8.
pub const NOT_UTF8: &str = "source is not UTF-8";

/// Since 0.19.2 a file that is not valid UTF-8 gets an encoding report at the first bad byte,
/// unless an ordinary syntax error comes first. `Ok` carries the text to keep compiling with.
pub(crate) fn decode_source(bytes: Vec<u8>) -> Result<String, (String, usize, usize)> {
    let error = match String::from_utf8(bytes) {
        Ok(source) => return Ok(source),
        Err(error) => error,
    };
    let valid = error.utf8_error().valid_up_to();
    let bytes = error.into_bytes();
    let lossy = String::from_utf8_lossy(&bytes).into_owned();
    let prefix = &lossy[..valid];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    Err((lossy, line, column))
}

/// Whether sources that are not valid UTF-8 get the dedicated report of 0.19.2 and later.
pub(crate) fn reports_encoding() -> bool {
    active() >= Version([0, 19, 2])
}

/// Since 0.19.2 character literals are decoded by the parser instead of kept as written.
pub(crate) fn decoded_char_literals() -> bool {
    active() >= Version([0, 19, 2])
}

/// Since 0.19.2 a name clash is reported on the other of its two occurrences.
pub(crate) fn swapped_duplicate_regions() -> bool {
    active() >= Version([0, 19, 2])
}

/// Elm 0.19.3 corrected the `Debug` remnant explanation ("JavaScript is smaller").
pub(crate) fn corrected_debug_remnant_wording() -> bool {
    active() >= Version([0, 19, 3])
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

/// A record field name ordered like the official compiler's maps for the selected release.
///
/// Maps keyed by it must be built and read within one compilation, since the
/// order follows the release selected for the current thread.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FieldName(std::rc::Rc<str>);

impl FieldName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl Ord for FieldName {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_names(&self.0, &other.0)
    }
}
impl PartialOrd for FieldName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl std::ops::Deref for FieldName {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}
impl AsRef<str> for FieldName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Display for FieldName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
impl From<&str> for FieldName {
    fn from(name: &str) -> Self {
        Self(name.into())
    }
}
impl From<String> for FieldName {
    fn from(name: String) -> Self {
        Self(name.into())
    }
}
impl From<std::rc::Rc<str>> for FieldName {
    fn from(name: std::rc::Rc<str>) -> Self {
        Self(name)
    }
}
impl From<FieldName> for std::rc::Rc<str> {
    fn from(name: FieldName) -> Self {
        name.0
    }
}
