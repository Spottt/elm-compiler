//! Missing-import reports following Reporting.Error.Import in Elm 0.19.1.
use crate::docs_diagnostic::{reflow, reflow_chunks, snippet_positions, text};
use serde_json::{Value, json};
use std::{collections::{BTreeMap, BTreeSet}, path::PathBuf};

pub(crate) enum Ambiguity {
    Local(Vec<PathBuf>),
    LocalForeign(PathBuf, String),
    Foreign(Vec<String>),
}

pub(crate) struct ImportProblems {
    pub owner: String,
    pub module: String,
    pub path: PathBuf,
    pub source: String,
    pub missing: BTreeSet<String>,
    pub ambiguous: BTreeMap<String, Ambiguity>,
}
fn style(content: String, color: Option<&str>, underline: bool) -> Value {
    json!({"bold":false,"underline":underline,"color":color,"string":content})
}
fn package_hint(name: &str) -> Option<&'static str> {
    match name {
        "Browser" => Some("elm/browser"),
        "File" | "File.Download" | "File.Select" => Some("elm/file"),
        "Html" | "Html.Attributes" | "Html.Events" => Some("elm/html"),
        "Http" => Some("elm/http"),
        "Json.Decode" | "Json.Encode" => Some("elm/json"),
        "Random" => Some("elm/random"),
        "Time" => Some("elm/time"),
        "Url" | "Url.Parser" => Some("elm/url"),
        _ => None,
    }
}
// Styled documents are atomic in Elm's fillSep, even if they contain spaces.
fn fill_sep(chunks: Vec<Value>) -> Vec<Value> {
    let mut words = Vec::new();
    for chunk in chunks {
        if let Some(value) = chunk.as_str() {
            words.extend(value.split_whitespace().map(|word| json!(word)));
        } else { words.push(chunk); }
    }
    let mut output = Vec::new();
    let mut column = 0;
    for word in words {
        let width = word.as_str().or_else(|| word["string"].as_str()).unwrap_or("").chars().count();
        if column != 0 {
            if column + 1 + width > 80 { text(&mut output, "\n".into()); column = 0; }
            else { text(&mut output, " ".into()); column += 1; }
        }
        if let Some(value) = word.as_str() { text(&mut output, value.into()); }
        else { output.push(word); }
        column += width;
    }
    output
}
fn ambiguity_message(message: &mut Vec<Value>, ambiguity: &Ambiguity) {
    match ambiguity {
        Ambiguity::Local(paths) => {
            text(message, format!("\n{}\n\n", reflow("But I found multiple files in your \"source-directories\" with that name:")));
            message.push(style(paths.iter().map(|p| format!("    {}", p.display())).collect::<Vec<_>>().join("\n"), Some("yellow"), false));
            text(message, "\n\nChange the module names to be distinct!".into());
        }
        Ambiguity::LocalForeign(path, package) => {
            text(message, "\n".into());
            let details = fill_sep(vec![json!("But I found multiple modules with that name. One in the"),
                style(package.clone(), Some("yellow"), false),
                json!("package, and another defined locally in the"),
                style(path.display().to_string(), Some("yellow"), false),
                json!("file. I do not have a way to choose between them.")]);
            for chunk in details {
                if let Some(value) = chunk.as_str() { text(message, value.into()); }
                else { message.push(chunk); }
            }
            text(message, format!("\n\n{}", reflow("Try changing the name of the locally defined module to clear up the ambiguity?")));
        }
        Ambiguity::Foreign(packages) => {
            text(message, format!("\n{}\n\n", reflow("But multiple packages in your \"dependencies\" that expose a module that name:")));
            message.push(style(packages.iter().map(|p| format!("    {p}")).collect::<Vec<_>>().join("\n"), Some("yellow"), false));
            text(message, format!("\n\n{}\n\n", reflow("There is no way to disambiguate in cases like this right now. Of the known name clashes, they are usually for packages with similar purposes, so the current recommendation is to pick just one of them.")));
            message.push(style("Note".into(), None, true));
            text(message, reflow("Note: It seems possible to resolve this with new syntax in imports, but that is more complicated than it sounds. Right now, our module names are tied to GitHub repos, but we may want to get rid of that dependency for a variety of reasons. That would in turn have implications for our package infrastructure, hosting costs, and possibly on how package names are specified. The particular syntax chosen seems like it would interact with all these factors in ways that are difficult to predict, potentially leading to harder problems later on. So more design work and planning is needed on these topics.")[4..].into());
        }
    }
}
impl ImportProblems {
    pub fn report(&self, known: &BTreeSet<String>) -> Result<Value, String> {
        let tokens = crate::lexer::lex(&self.source)?;
        let mut regions = std::collections::BTreeMap::new();
        for (index, token) in tokens.iter().enumerate() {
            if token.text(&self.source) != "import" { continue; }
            let Some(start) = tokens.get(index + 1) else { continue; };
            let mut end = index + 1;
            while tokens.get(end + 1).is_some_and(|t| t.text(&self.source) == ".")
                && tokens.get(end + 2).is_some_and(|t| t.kind == crate::lexer::Kind::Upper)
            { end += 2; }
            let name = &self.source[start.start as usize..tokens[end].end as usize];
            let end_column = tokens[end].column as usize + tokens[end].text(&self.source).chars().count();
            regions.insert(name, ((start.row as usize, start.column as usize), (tokens[end].row as usize, end_column)));
        }
        let mut problems = Vec::new();
        let mut missing: Vec<_> = self.missing.iter().chain(self.ambiguous.keys()).collect();
        missing.sort_by_key(|name| regions.get(name.as_str()).copied());
        for name in missing {
            let &(start, end) = regions.get(name.as_str()).ok_or_else(|| format!("{}: cannot locate import {name}", self.path.display()))?;
            let mut message = snippet_positions(&self.source, start, end,
                &format!("You are trying to import a `{name}` module:"))
                .ok_or("cannot render import region")?;
            if let Some(ambiguity) = self.ambiguous.get(name) {
                ambiguity_message(&mut message, ambiguity);
                problems.push(json!({"title":"AMBIGUOUS IMPORT","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}));
                continue;
            }
            text(&mut message, format!("\n{}\n\n", reflow("I checked the \"dependencies\" and \"source-directories\" listed in your elm.json, but I cannot find it! Maybe it is a typo for one of these names?")));
            let mut suggestions: Vec<_> = known.iter()
                .filter(|candidate| !regions.contains_key(candidate.as_str())).collect();
            suggestions.sort_by_key(|candidate| crate::name_diagnostic::distance(name, candidate));
            message.push(style(suggestions.into_iter().take(4).map(|s| format!("    {s}")).collect::<Vec<_>>().join("\n"), Some("yellow"), false));
            text(&mut message, "\n\n".into());
            let mut hint = vec![style("Hint".into(), None, true)];
            if let Some(package) = package_hint(name) {
                let prefix = reflow(&format!("Hint: Maybe you want the `{name}` module defined in the {package} package? Running"));
                let mut column = prefix.rsplit('\n').next().unwrap_or("").chars().count();
                text(&mut hint, prefix[4..].into());
                let command = format!("elm install {package}");
                // The historical green command is one document, including spaces.
                // Wrapping it word-by-word changes the JSON segments and layout.
                let width = command.chars().count();
                if column + 1 + width > 80 { text(&mut hint, "\n".into()); column = 0; }
                else { text(&mut hint, " ".into()); column += 1; }
                hint.push(style(command, Some("GREEN"), false));
                column += width;
                for word in ["should", "make", "it", "available!"] {
                    if column + 1 + word.len() > 80 { text(&mut hint, "\n".into()); column = 0; }
                    else { text(&mut hint, " ".into()); column += 1; }
                    text(&mut hint, word.into()); column += word.len();
                }
                message.extend(hint);
            } else {
                text(&mut hint, ": If it is not a typo, check the \"dependencies\" and \"source-directories\" of your elm.json to make sure all the packages you need are listed there!".into());
                message.extend(reflow_chunks(hint));
            }
            problems.push(json!({"title":"MODULE NOT FOUND","region":{"start":{"line":start.0,"column":start.1},"end":{"line":end.0,"column":end.1}},"message":message}));
        }
        Ok(json!({"path":self.path,"name":self.module,"problems":problems}))
    }
}
