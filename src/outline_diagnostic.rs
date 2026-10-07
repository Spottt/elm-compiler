//! Source-aware outline diagnostics. Borrowed JSON values retain exact offsets.
use serde_json::{Value, json, value::RawValue};

struct Fields<'a>(Vec<(String, &'a RawValue)>);
impl<'de> serde::Deserialize<'de> for Fields<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Fields<'de>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut fields = Vec::new();
                while let Some(entry) = map.next_entry::<String, &'de RawValue>()? {
                    fields.push(entry);
                }
                Ok(Fields(fields))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

fn value_at<'a>(source: &'a str, path: &[String]) -> Option<&'a RawValue> {
    let mut raw: &RawValue = serde_json::from_str(source).ok()?;
    for key in path {
        raw = if raw.get().starts_with('[') {
            let entries: Vec<&RawValue> = serde_json::from_str(raw.get()).ok()?;
            *entries.get(key.parse::<usize>().ok()?)?
        } else {
            let Fields(fields) = serde_json::from_str(raw.get()).ok()?;
            // Named outline fields use the first occurrence, including escaped keys.
            fields.into_iter().find(|(name, _)| name == key)?.1
        };
    }
    Some(raw)
}

// Named fields use their first occurrence, while dictionaries validate every
// occurrence before collapsing duplicate keys. Keep those diagnostic candidates.
fn diagnostic_values_at<'a>(source: &'a str, path: &[String]) -> Option<Vec<&'a RawValue>> {
    let application =
        value_at(source, &["type".into()]).is_some_and(|raw| raw.get() == "\"application\"");
    let mut values = vec![value_at(source, &[])?];
    for (depth, key) in path.iter().enumerate() {
        let dictionary = (depth == 1 && path[0] == "exposed-modules")
            || (matches!(path[0].as_str(), "dependencies" | "test-dependencies")
                && depth == if application { 2 } else { 1 });
        let mut next = Vec::new();
        for raw in values {
            if raw.get().starts_with('[') {
                let entries: Vec<&RawValue> = serde_json::from_str(raw.get()).ok()?;
                if let Ok(index) = key.parse::<usize>()
                    && let Some(entry) = entries.get(index)
                {
                    next.push(*entry);
                }
            } else if raw.get().starts_with('{') {
                let Fields(fields) = serde_json::from_str(raw.get()).ok()?;
                for (name, value) in fields {
                    if &name == key {
                        next.push(value);
                        if !dictionary {
                            break;
                        }
                    }
                }
            }
        }
        values = next;
    }
    Some(values)
}

pub(crate) fn missing_field(source: &str, path: &[String], name: &str) -> Option<Value> {
    let raw = value_at(source, path)?;
    let offset = raw.get().as_ptr() as usize - source.as_ptr() as usize;
    let start = crate::docs_diagnostic::position(source, offset.try_into().ok()?)?;
    let end = (start.0, start.1 + 1);
    let introduction = path.last().map_or_else(
        || "I ran into some trouble here:".to_string(),
        |field| format!("I ran into trouble with the value of the \"{field}\" field:"),
    );
    let mut message = crate::docs_diagnostic::snippet_positions(
        source,
        start,
        end,
        &format!("I ran into a problem with your elm.json file. {introduction}"),
    )?;
    crate::docs_diagnostic::text(&mut message, "\nI was expecting to run into an ".into());
    let green =
        |string: String| json!({"bold":false,"underline":false,"color":"GREEN","string":string});
    message.push(green("OBJECT".into()));
    crate::docs_diagnostic::text(&mut message, " with a ".into());
    message.push(green(format!("\"{name}\"")));
    crate::docs_diagnostic::text(&mut message, " field.".into());
    Some(json!({"type":"error","path":"elm.json","title":"MISSING FIELD","message":message}))
}

fn styled(string: &str, color: &str) -> Value {
    json!({"bold":false,"underline":false,"color":color,"string":string})
}
fn report(title: &str, message: Vec<Value>) -> Value {
    json!({"type":"error","path":"elm.json","title":title,"message":message})
}
pub(crate) fn empty_sources(source: &str) -> Option<Value> {
    let raw = value_at(source, &["source-directories".into()])?;
    let offset = raw.get().as_ptr() as usize - source.as_ptr() as usize;
    let start = crate::docs_diagnostic::position(source, offset.try_into().ok()?)?;
    let end =
        crate::docs_diagnostic::position(source, (offset + raw.get().len()).try_into().ok()?)?;
    let mut message = crate::docs_diagnostic::snippet_positions(
        source,
        start,
        end,
        "I got stuck while reading your elm.json file. You do not have any \"source-directories\" listed here:",
    )?;
    crate::docs_diagnostic::text(&mut message, "\nI need something like ".into());
    message.push(styled("[\"src\"]", "GREEN"));
    crate::docs_diagnostic::text(
        &mut message,
        " so I know where to look for your modules!".into(),
    );
    Some(report("NO SOURCE DIRECTORIES", message))
}
pub(crate) fn missing_sources(dirs: &[&str]) -> Value {
    let plural = dirs.len() > 1;
    let noun = if plural { "directories" } else { "directory" };
    let mut message = vec![];
    crate::docs_diagnostic::text(
        &mut message,
        format!(
            "{}\n\n    ",
            crate::docs_diagnostic::reflow(&format!(
                "I need a valid elm.json file, but the \"source-directories\" field lists the following {noun}:"
            ))
        ),
    );
    for (index, dir) in dirs.iter().enumerate() {
        if index > 0 {
            crate::docs_diagnostic::text(&mut message, "\n    ".into());
        }
        message.push(styled(dir, "RED"));
    }
    crate::docs_diagnostic::text(
        &mut message,
        format!(
            "\n\n{}",
            if plural {
                "I cannot find them though. Are they missing? Are there typos?"
            } else {
                "I cannot find it though. Is it missing? Is there a typo?"
            }
        ),
    );
    report(
        if plural {
            "MISSING SOURCE DIRECTORIES"
        } else {
            "MISSING SOURCE DIRECTORY"
        },
        message,
    )
}
pub(crate) fn duplicate_sources(canonical: &std::path::Path, first: &str, second: &str) -> Value {
    let identical = first == second;
    let heading = if identical {
        "I need a valid elm.json file, but the \"source-directories\" field lists the same directory twice:"
    } else {
        "I need a valid elm.json file, but the \"source-directories\" field has some redundant directories:"
    };
    let mut message = vec![];
    crate::docs_diagnostic::text(
        &mut message,
        format!("{}\n\n    ", crate::docs_diagnostic::reflow(heading)),
    );
    message.push(styled(first, "RED"));
    crate::docs_diagnostic::text(&mut message, "\n    ".into());
    message.push(styled(second, "RED"));
    if identical {
        crate::docs_diagnostic::text(&mut message, "\n\nRemove one of the entries!".into());
    } else {
        crate::docs_diagnostic::text(
            &mut message,
            "\n\nThese are two different ways of refering to the same directory:\n\n    ".into(),
        );
        message.push(styled(&canonical.to_string_lossy(), "yellow"));
        crate::docs_diagnostic::text(
            &mut message,
            "\n\nRemove one of the redundant entries from your \"source-directories\" field."
                .into(),
        );
    }
    report("REDUNDANT SOURCE DIRECTORIES", message)
}

/// Static prose is transcribed from Elm 0.19.1 Reporting.Exit (BSD-3-Clause).
/// Source regions and license suggestions are computed for each input.
pub(crate) fn package_metadata(source: &str, title: &str) -> Option<Value> {
    let (field, heading) = match title {
        "SUMMARY TOO LONG" => (
            "summary",
            "I got stuck while reading your elm.json file. Your \"summary\" is too long:",
        ),
        "UNKNOWN LICENSE" => (
            "license",
            "I got stuck while reading your elm.json file. I do not know about this type of license:",
        ),
        "INVALID PACKAGE NAME" => (
            "name",
            "I got stuck while reading your elm.json file. I ran into trouble with the package name:",
        ),
        _ => return None,
    };
    let raw = value_at(source, &[field.into()])?;
    let offset = raw.get().as_ptr() as usize - source.as_ptr() as usize;
    let content = raw.get().strip_prefix('"')?.strip_suffix('"')?;
    let (start, end) = if title == "INVALID PACKAGE NAME" {
        let at = offset + 1 + crate::package_solver::name_error_offset(content)?;
        let pos = crate::docs_diagnostic::position(source, at.try_into().ok()?)?;
        (pos, (pos.0, pos.1 + 1))
    } else {
        (
            crate::docs_diagnostic::position(source, offset.try_into().ok()?)?,
            crate::docs_diagnostic::position(source, (offset + raw.get().len()).try_into().ok()?)?,
        )
    };
    let mut message = crate::docs_diagnostic::snippet_positions(source, start, end, heading)?;
    let templates: Value =
        serde_json::from_str(include_str!("outline_metadata_messages.json")).ok()?;
    let mut body = templates[title].as_array()?.clone();
    if title == "UNKNOWN LICENSE" {
        let licenses = crate::outline::LICENSES;
        let mut candidates: Vec<_> = licenses
            .iter()
            .map(|(code, _)| (*code, *code))
            .chain(licenses.iter().copied())
            .collect();
        candidates.sort_by_key(|(_, label)| crate::name_diagnostic::distance(content, label));
        let suggestions = candidates
            .iter()
            .take(4)
            .map(|(code, _)| *code)
            .collect::<Vec<_>>()
            .join("\n    ");
        for chunk in &mut body {
            if chunk["color"] == "yellow" {
                chunk["string"] = json!(suggestions);
            }
        }
    }
    // Join adjacent plain text at the snippet/body boundary, as Elm's renderer does.
    for chunk in body {
        if let Some(text) = chunk.as_str() {
            crate::docs_diagnostic::text(&mut message, text.into());
        } else {
            message.push(chunk);
        }
    }
    Some(report(title, message))
}

fn expect_byte(bytes: &[u8], cursor: &mut usize, expected: u8) -> Result<(), usize> {
    if bytes.get(*cursor) == Some(&expected) {
        *cursor += 1;
        Ok(())
    } else {
        Err(*cursor)
    }
}
fn version_prefix(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<crate::package_solver::Version, usize> {
    let mut parts = [0u32; 3];
    for (index, part) in parts.iter_mut().enumerate() {
        if index > 0 {
            expect_byte(bytes, cursor, b'.')?;
        }
        if bytes.get(*cursor) == Some(&b'0') {
            *cursor += 1;
        } else {
            if !bytes.get(*cursor).is_some_and(u8::is_ascii_digit) {
                return Err(*cursor);
            }
            let start = *cursor;
            while bytes.get(*cursor).is_some_and(u8::is_ascii_digit) {
                *cursor += 1;
            }
            *part = crate::edition::version_component(bytes[start..*cursor].iter().copied(), index);
        }
    }
    Ok(crate::package_solver::Version(parts))
}
fn constraint_parts(
    content: &str,
) -> Result<
    (
        crate::package_solver::Version,
        crate::package_solver::Version,
        usize,
    ),
    usize,
> {
    let bytes = content.as_bytes();
    let mut cursor = 0;
    let lower = version_prefix(bytes, &mut cursor)?;
    for first in [true, false] {
        expect_byte(bytes, &mut cursor, b' ')?;
        expect_byte(bytes, &mut cursor, b'<')?;
        if bytes.get(cursor) == Some(&b'=') {
            cursor += 1;
        }
        expect_byte(bytes, &mut cursor, b' ')?;
        if first {
            expect_byte(bytes, &mut cursor, b'v')?;
        }
    }
    let upper = version_prefix(bytes, &mut cursor)?;
    Ok((lower, upper, cursor))
}
// fillSep treats an explicitly styled document (such as a quoted constraint)
// as one atom even when it contains spaces. Keep that atom intact when wrapping.
fn prose_atoms(chunks: Vec<Value>) -> Vec<Value> {
    let mut result = vec![];
    let mut column = 0;
    for chunk in chunks {
        let atoms = if let Some(plain) = chunk.as_str() {
            plain
                .split_whitespace()
                .map(|s| json!(s))
                .collect::<Vec<_>>()
        } else {
            vec![chunk]
        };
        for atom in atoms {
            let content = atom
                .as_str()
                .or_else(|| atom["string"].as_str())
                .unwrap_or("");
            let width = content.chars().count();
            if column > 0 {
                let newline = column + 1 + width > 80;
                crate::docs_diagnostic::text(&mut result, if newline { "\n" } else { " " }.into());
                column = if newline { 0 } else { column + 1 };
            }
            if let Some(plain) = atom.as_str() {
                crate::docs_diagnostic::text(&mut result, plain.into());
            } else {
                result.push(atom);
            }
            column += width;
        }
    }
    result
}

pub(crate) fn version_constraint(source: &str, path: &[String], title: &str) -> Option<Value> {
    let raw = diagnostic_values_at(source, path)?
        .into_iter()
        .find(|raw| {
            let Some(content) = raw
                .get()
                .strip_prefix('"')
                .and_then(|text| text.strip_suffix('"'))
            else {
                return false;
            };
            if title == "PROBLEM WITH VERSION" {
                crate::package_solver::Version::parse(content).is_err()
            } else {
                crate::package_solver::Constraint::parse(content).is_err()
            }
        })?;
    let offset = raw.get().as_ptr() as usize - source.as_ptr() as usize;
    let content = raw.get().strip_prefix('"')?.strip_suffix('"')?;
    let mut range = None;
    let highlight = if title == "PROBLEM WITH VERSION" {
        let mut cursor = 0;
        Some(match version_prefix(content.as_bytes(), &mut cursor) {
            Err(at) => at,
            Ok(_) if cursor < content.len() => cursor,
            Ok(_) => return None,
        })
    } else {
        match constraint_parts(content) {
            Err(at) => Some(at),
            Ok((lower, upper, _)) if lower >= upper => {
                range = Some((lower, upper));
                None
            }
            Ok((_, _, end)) if end < content.len() => Some(end),
            _ => return None,
        }
    };
    let heading = if title == "PROBLEM WITH VERSION" {
        "I got stuck while reading your elm.json file. I was expecting a version number here:"
    } else if range.is_some() {
        "I got stuck while reading your elm.json file. I ran into an invalid version constraint:"
    } else {
        "I got stuck while reading your elm.json file. I do not understand this version constraint:"
    };
    let (start, end) = if let Some(at) = highlight {
        let start = crate::docs_diagnostic::position(source, (offset + 1 + at).try_into().ok()?)?;
        (start, (start.0, start.1 + 1))
    } else {
        (
            crate::docs_diagnostic::position(source, offset.try_into().ok()?)?,
            crate::docs_diagnostic::position(source, (offset + raw.get().len()).try_into().ok()?)?,
        )
    };
    let mut message = crate::docs_diagnostic::snippet_positions(source, start, end, heading)?;
    crate::docs_diagnostic::text(&mut message, "\n".into());
    let body = if title == "PROBLEM WITH VERSION" {
        prose_atoms(vec![
            json!("I need something like"),
            styled("\"1.0.0\"", "GREEN"),
            json!("or"),
            styled("\"2.0.4\"", "GREEN"),
            json!("that explicitly states all three numbers!"),
        ])
    } else if let Some((lower, upper)) = range {
        let next = lower.next_major();
        let recommendation = styled(&format!("\"{lower} <= v < {next}\""), "GREEN");
        if lower == upper {
            prose_atoms(vec![
                json!(
                    "Elm checks that all package APIs follow semantic versioning, so it is best to use wide constraints. I recommend"
                ),
                recommendation,
                json!(
                    "since it is guaranteed that breaking API changes cannot happen in any of the versions in that range."
                ),
            ])
        } else {
            prose_atoms(vec![
                json!("Maybe you want something like"),
                recommendation,
                json!(
                    "instead? Elm checks that all package APIs follow semantic versioning, so it is guaranteed that breaking API changes cannot happen in any of the versions in that range."
                ),
            ])
        }
    } else {
        let mut body = prose_atoms(vec![
            json!("I need something like"),
            styled("\"1.0.0 <= v < 2.0.0\"", "GREEN"),
            json!("that explicitly lists the lower and upper bounds."),
        ]);
        crate::docs_diagnostic::text(&mut body, "\n\n".into());
        body.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
        crate::docs_diagnostic::text(&mut body, crate::docs_diagnostic::reflow("Note: The spaces in there are required! Taking them out will confuse me. Adding extra spaces confuses me too. I recommend starting with a valid example and just changing the version numbers.")[4..].into());
        body
    };
    for chunk in body {
        if let Some(plain) = chunk.as_str() {
            crate::docs_diagnostic::text(&mut message, plain.into());
        } else {
            message.push(chunk);
        }
    }
    Some(report(title, message))
}

// Field names need their raw spelling, including JSON escapes, just like the
// outline decoder. Borrow the adjacent values to locate keys without searching
// for repeated strings elsewhere in the document.
fn key_at<'a>(source: &'a str, path: &[String]) -> Option<&'a str> {
    let (name, parents) = path.split_last()?;
    let object = value_at(source, parents)?;
    let Fields(fields) = serde_json::from_str(object.get()).ok()?;
    let mut offset = object.get().as_ptr() as usize - source.as_ptr() as usize + 1;
    for (_, value) in fields {
        let remaining =
            source[offset..].trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
        let raw = serde_json::Deserializer::from_str(remaining)
            .into_iter::<&RawValue>()
            .next()?
            .ok()?;
        let content = raw.get().strip_prefix('"')?.strip_suffix('"')?;
        if content == name {
            return Some(content);
        }
        offset = value.get().as_ptr() as usize - source.as_ptr() as usize + value.get().len();
    }
    None
}

pub(crate) fn names(source: &str, path: &[String], title: &str) -> Option<Value> {
    let dependency = title == "PROBLEM WITH DEPENDENCY NAME";
    let (start, end) = if dependency {
        let name = key_at(source, path)?;
        let offset = name.as_ptr() as usize - source.as_ptr() as usize
            + crate::package_solver::name_error_offset(name)?;
        let start = crate::docs_diagnostic::position(source, offset.try_into().ok()?)?;
        (start, (start.0, start.1 + 1))
    } else {
        let raw = value_at(source, &[])?;
        let offset = raw.get().as_ptr() as usize - source.as_ptr() as usize;
        (
            crate::docs_diagnostic::position(source, offset.try_into().ok()?)?,
            crate::docs_diagnostic::position(source, (offset + raw.get().len()).try_into().ok()?)?,
        )
    };
    let heading = if dependency {
        "I got stuck while reading your elm.json file. There is something wrong with this dependency name:"
    } else {
        "I got stuck while reading your elm.json file. I cannot handle a \"type\" like this:"
    };
    let mut message = crate::docs_diagnostic::snippet_positions(source, start, end, heading)?;
    crate::docs_diagnostic::text(&mut message, "\n".into());
    let paragraphs = if dependency {
        vec![
            prose_atoms(vec![
                json!(
                    "Package names always include the name of the author, so I am expecting to see dependencies like"
                ),
                styled("\"mdgriffith/elm-ui\"", "yellow"),
                json!("and"),
                styled("\"Microsoft/elm-json-tree-view\"", "yellow"),
            ]),
            prose_atoms(vec![
                json!(
                    "I generally recommend finding the package you want on the package website, and installing it with the"
                ),
                styled("elm install", "GREEN"),
                json!("command!"),
            ]),
        ]
    } else {
        vec![prose_atoms(vec![
            json!("Try changing the \"type\" to"),
            styled("\"application\"", "GREEN"),
            json!("or"),
            styled("\"package\"", "GREEN"),
            json!("instead."),
        ])]
    };
    for (index, paragraph) in paragraphs.into_iter().enumerate() {
        if index > 0 {
            crate::docs_diagnostic::text(&mut message, "\n\n".into());
        }
        for chunk in paragraph {
            if let Some(plain) = chunk.as_str() {
                crate::docs_diagnostic::text(&mut message, plain.into());
            } else {
                message.push(chunk);
            }
        }
        if dependency && index == 0 {
            crate::docs_diagnostic::text(&mut message, ".".into());
        }
    }
    Some(report(title, message))
}

pub(crate) fn modules(source: &str) -> Option<Value> {
    fn invalid_module(raw: &RawValue) -> Option<(&str, usize)> {
        let entries: Vec<&RawValue> = serde_json::from_str(raw.get()).ok()?;
        for entry in entries {
            let name = entry.get().strip_prefix('"')?.strip_suffix('"')?;
            if let Some(offset) = crate::outline::module_name_error_offset(name) {
                return Some((name, offset));
            }
        }
        None
    }
    let raw = value_at(source, &["exposed-modules".into()])?;
    let (name, at) = if raw.get().starts_with('{') {
        let Fields(groups) = serde_json::from_str(raw.get()).ok()?;
        groups
            .into_iter()
            .find_map(|(_, value)| invalid_module(value))?
    } else {
        invalid_module(raw)?
    };
    let offset = name.as_ptr() as usize - source.as_ptr() as usize + at;
    let start = crate::docs_diagnostic::position(source, offset.try_into().ok()?)?;
    let mut message = crate::docs_diagnostic::snippet_positions(
        source,
        start,
        (start.0, start.1 + 1),
        "I got stuck while reading your elm.json file. I was expecting a module name here:",
    )?;
    crate::docs_diagnostic::text(&mut message, "\n".into());
    for chunk in prose_atoms(vec![
        json!("I need something like"),
        styled("\"Html.Events\"", "GREEN"),
        json!("or"),
        styled("\"Browser.Navigation\"", "GREEN"),
        json!(
            "where each segment starts with a capital letter and the segments are separated by dots."
        ),
    ]) {
        if let Some(plain) = chunk.as_str() {
            crate::docs_diagnostic::text(&mut message, plain.into());
        } else {
            message.push(chunk);
        }
    }
    Some(report("PROBLEM WITH MODULE NAME", message))
}

pub(crate) fn expectation(source: &str, path: &[String], title: &str) -> Option<Value> {
    let expected = match title {
        "EXPECTING STRING" => '"',
        "EXPECTING ARRAY" => '[',
        "EXPECTING OBJECT" => '{',
        _ => return None,
    };
    let raw = diagnostic_values_at(source, path)?
        .into_iter()
        .find(|raw| !raw.get().starts_with(expected))?;
    let offset = raw.get().as_ptr() as usize - source.as_ptr() as usize;
    let start = crate::docs_diagnostic::position(source, offset.try_into().ok()?)?;
    let mut end =
        crate::docs_diagnostic::position(source, (offset + raw.get().len()).try_into().ok()?)?;
    // Elm reduces multiline values to their opening character. Its same-line
    // branch loops in Reporting.Error.Json; retain a finite, useful span here.
    if end.0 != start.0 {
        end = (start.0, start.1 + 1);
    }
    let introduction = if let Some((last, parents)) = path.split_last() {
        let parent = value_at(source, parents)?;
        if parent.get().starts_with('[') {
            let index = last.parse::<usize>().ok()?;
            let suffix = if (11..=13).contains(&(index % 100)) {
                "th"
            } else {
                match index % 10 {
                    1 => "st",
                    2 => "nd",
                    3 => "rd",
                    _ => "th",
                }
            };
            if let Some((field, grandparents)) = parents.split_last()
                && value_at(source, grandparents)?.get().starts_with('{')
            {
                format!(
                    "When looking at the \"{field}\" field, I ran into trouble with the {index}{suffix} entry:"
                )
            } else {
                format!("I ran into trouble with the {index}{suffix} index of this array:")
            }
        } else {
            format!("I ran into trouble with the value of the \"{last}\" field:")
        }
    } else {
        "I ran into some trouble here:".into()
    };
    let mut message = crate::docs_diagnostic::snippet_positions(
        source,
        start,
        end,
        &format!("I ran into a problem with your elm.json file. {introduction}"),
    )?;
    let (article, kind) = match title {
        "EXPECTING STRING" => ("a", "STRING"),
        "EXPECTING ARRAY" => ("an", "ARRAY"),
        "EXPECTING OBJECT" => ("an", "OBJECT"),
        _ => return None,
    };
    crate::docs_diagnostic::text(
        &mut message,
        format!("\nI was expecting to run into {article} "),
    );
    message.push(styled(kind, "GREEN"));
    crate::docs_diagnostic::text(&mut message, ".".into());
    Some(report(title, message))
}
