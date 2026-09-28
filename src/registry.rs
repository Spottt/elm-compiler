//! Elm 0.19.1 registry.dat and the package.elm-lang.org registry payloads.
//! Binary layout follows Deps.Registry, Elm.Package and Elm.Version upstream.
use crate::package_solver::{Version, valid_name};
use serde_json::Value;
use std::{cmp::Ordering, collections::BTreeMap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registry {
    count: usize,
    packages: BTreeMap<String, Vec<Version>>,
}
/// Elm compares author and project separately, not the combined slash string.
pub fn compare_names(a: &str, b: &str) -> Ordering {
    a.split_once('/').cmp(&b.split_once('/'))
}
fn check_name(name: &str) -> Result<(), String> {
    if !valid_name(name) || name.split('/').any(|part| part.len() > 255) {
        Err(format!("invalid registry package name {name}"))
    } else {
        Ok(())
    }
}
impl Registry {
    pub fn read_cached(home: &std::path::Path) -> Result<Option<Self>, String> {
        let path = home.join("0.19.1/packages/registry.dat");
        match std::fs::read(&path) {
            Ok(bytes) => match Self::decode(&bytes) {
                Ok(registry) => Ok(Some(registry)),
                Err(problem) => {
                    // File.readBinary in Elm treats undecodable cache data as
                    // absent, allowing /all-packages to repair it. Preserve the
                    // file until a replacement has been successfully fetched.
                    eprintln!(
                        "Corrupt File: {}\nMessage: {problem}\nTrying to continue anyway.",
                        path.display()
                    );
                    Ok(None)
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn package_count(&self) -> usize {
        self.packages.len()
    }
    pub fn versions(&self, name: &str) -> Option<&[Version]> {
        self.packages.get(name).map(Vec::as_slice)
    }
    /// Terminal.Helpers ranks malformed package arguments by the full name,
    /// unlike Elm.Package.nearbyNames which ranks author and project separately.
    pub fn argument_examples(&self, given: &str) -> Vec<String> {
        let target: Vec<_> = given.chars().flat_map(char::to_lowercase).collect();
        let mut names: Vec<_> = self.packages.keys().collect();
        names.sort_by(|a, b| compare_names(a, b));
        names.sort_by_key(|name| {
            let candidate: Vec<_> = name.chars().flat_map(char::to_lowercase).collect();
            edit_distance(&target, &candidate)
        });
        names.into_iter().take(4).cloned().collect()
    }
    /// Elm.Package.nearbyNames: restricted Damerau-Levenshtein distance,
    /// with no author penalty for the two official package organizations.
    pub fn nearby_names(&self, name: &str) -> Vec<String> {
        let Some((author, project)) = name.split_once('/') else {
            return Vec::new();
        };
        let mut ranked: Vec<_> = self
            .packages
            .keys()
            .map(|candidate| {
                let (other_author, other_project) = candidate.split_once('/').unwrap();
                let author_distance = if matches!(other_author, "elm" | "elm-explorations") {
                    0
                } else {
                    edit_distance(author.as_bytes(), other_author.as_bytes())
                };
                (
                    author_distance + edit_distance(project.as_bytes(), other_project.as_bytes()),
                    candidate,
                )
            })
            .collect();
        ranked.sort_by(|(left_score, left), (right_score, right)| {
            left_score
                .cmp(right_score)
                .then_with(|| compare_names(left, right))
        });
        ranked
            .into_iter()
            .take(4)
            .map(|(_, name)| name.clone())
            .collect()
    }
    pub fn from_json(value: &Value) -> Result<Self, String> {
        let mut packages = BTreeMap::new();
        let mut count = 0usize;
        for (name, versions) in value.as_object().ok_or("expected all-packages object")? {
            check_name(name)?;
            let mut versions = versions
                .as_array()
                .ok_or("expected package version list")?
                .iter()
                .map(|v| Version::parse(v.as_str().ok_or("expected package version string")?))
                .collect::<Result<Vec<_>, String>>()?;
            if versions.is_empty() {
                return Err(format!("empty versions for {name}"));
            }
            versions.sort_unstable_by(|a, b| b.cmp(a));
            count = count
                .checked_add(versions.len())
                .ok_or("registry count overflow")?;
            packages.insert(name.clone(), versions);
        }
        Ok(Self { count, packages })
    }
    /// `/all-packages/since/N` lists newest additions first. The Haskell update
    /// folds right, so process oldest additions first and prepend each version.
    pub fn updated(&self, value: &Value) -> Result<Self, String> {
        let updates = value.as_array().ok_or("expected registry update list")?;
        let mut result = self.clone();
        for update in updates.iter().rev() {
            let (name, version) = update
                .as_str()
                .and_then(|s| s.split_once('@'))
                .ok_or("invalid registry update")?;
            check_name(name)?;
            let version = Version::parse(version)?;
            result
                .packages
                .entry(name.into())
                .or_default()
                .insert(0, version);
            result.count = result
                .count
                .checked_add(1)
                .ok_or("registry count overflow")?;
        }
        Ok(result)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut input = Input { bytes, offset: 0 };
        let count = input.count()?;
        let length = input.count()?;
        let mut packages = BTreeMap::new();
        let mut previous: Option<String> = None;
        let mut observed = 0usize;
        for _ in 0..length {
            let author = input.text()?;
            let project = input.text()?;
            let name = format!("{author}/{project}");
            check_name(&name)?;
            if previous
                .as_ref()
                .is_some_and(|p| compare_names(p, &name) != Ordering::Less)
            {
                return Err("registry package keys are not strictly ascending".into());
            }
            previous = Some(name.clone());
            let newest = input.version()?;
            let old_count = input.count()?;
            let mut versions = Vec::with_capacity(old_count + 1);
            versions.push(newest);
            for _ in 0..old_count {
                versions.push(input.version()?);
            }
            observed = observed
                .checked_add(versions.len())
                .ok_or("registry count overflow")?;
            packages.insert(name, versions);
        }
        if input.offset != bytes.len() {
            return Err("trailing registry data".into());
        }
        if count != observed {
            return Err("registry version count mismatch".into());
        }
        Ok(Self { count, packages })
    }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        put_count(&mut out, self.count)?;
        put_count(&mut out, self.packages.len())?;
        let mut packages = self.packages.iter().collect::<Vec<_>>();
        packages.sort_unstable_by(|(a, _), (b, _)| compare_names(a, b));
        for (name, versions) in packages {
            for part in name.split('/') {
                out.push(u8::try_from(part.len()).map_err(|_| "registry name too long")?);
                out.extend_from_slice(part.as_bytes());
            }
            put_version(&mut out, versions[0]);
            put_count(&mut out, versions.len() - 1)?;
            for version in &versions[1..] {
                put_version(&mut out, *version);
            }
        }
        Ok(out)
    }
}

fn edit_distance<T: PartialEq>(left: &[T], right: &[T]) -> usize {
    let mut previous: Vec<_> = (0..=right.len()).collect();
    let mut before_previous = previous.clone();
    let mut current = previous.clone();
    for i in 1..=left.len() {
        current[0] = i;
        for j in 1..=right.len() {
            current[j] = (previous[j] + 1)
                .min(current[j - 1] + 1)
                .min(previous[j - 1] + usize::from(left[i - 1] != right[j - 1]));
            if i > 1 && j > 1 && left[i - 1] == right[j - 2] && left[i - 2] == right[j - 1] {
                current[j] = current[j].min(before_previous[j - 2] + 1);
            }
        }
        std::mem::swap(&mut before_previous, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}
fn put_count(out: &mut Vec<u8>, count: usize) -> Result<(), String> {
    out.extend_from_slice(
        &i64::try_from(count)
            .map_err(|_| "registry length overflow")?
            .to_be_bytes(),
    );
    Ok(())
}
fn put_version(out: &mut Vec<u8>, Version([a, b, c]): Version) {
    if a < 255 && b < 256 && c < 256 {
        out.extend_from_slice(&[a as u8, b as u8, c as u8]);
    } else {
        out.push(255);
        for part in [a, b, c] {
            out.extend_from_slice(&part.to_be_bytes());
        }
    }
}
struct Input<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Input<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or("registry offset overflow")?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or("truncated registry data")?;
        self.offset = end;
        Ok(slice)
    }
    fn count(&mut self) -> Result<usize, String> {
        let count = i64::from_be_bytes(self.take(8)?.try_into().unwrap());
        let count = usize::try_from(count).map_err(|_| "negative registry length")?;
        if count > self.bytes.len() {
            return Err("invalid registry length".into());
        }
        Ok(count)
    }
    fn text(&mut self) -> Result<&'a str, String> {
        let length = self.take(1)?[0] as usize;
        std::str::from_utf8(self.take(length)?).map_err(|_| "invalid UTF-8 registry name".into())
    }
    fn version(&mut self) -> Result<Version, String> {
        let major = self.take(1)?[0];
        if major == 255 {
            let bytes = self.take(6)?;
            Ok(Version([
                u16::from_be_bytes([bytes[0], bytes[1]]),
                u16::from_be_bytes([bytes[2], bytes[3]]),
                u16::from_be_bytes([bytes[4], bytes[5]]),
            ]))
        } else {
            let rest = self.take(2)?;
            Ok(Version([major as u16, rest[0] as u16, rest[1] as u16]))
        }
    }
}
