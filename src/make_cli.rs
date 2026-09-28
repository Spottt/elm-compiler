//! Make flag suggestions, following Terminal.Error and Reporting.Suggest.
use std::io::IsTerminal;

fn styled(code: u8, text: &str) -> String {
    if std::io::stderr().is_terminal() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

// Restricted Damerau-Levenshtein, case sensitive for CLI flag names.
pub(crate) fn distance(a: &str, b: &str) -> usize {
    let a: Vec<_> = a.chars().collect();
    let b: Vec<_> = b.chars().collect();
    let mut rows = vec![vec![0; b.len() + 1]; a.len() + 1];
    for (i, row) in rows.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, value) in rows[0].iter_mut().enumerate() {
        *value = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            rows[i][j] = (rows[i - 1][j] + 1)
                .min(rows[i][j - 1] + 1)
                .min(rows[i - 1][j - 1] + usize::from(a[i - 1] != b[j - 1]));
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                rows[i][j] = rows[i][j].min(rows[i - 2][j - 2] + 1);
            }
        }
    }
    rows[a.len()][b.len()]
}

pub fn unknown(flag: &str) -> String {
    unknown_with(
        flag,
        &[
            ("debug", "--debug"),
            ("optimize", "--optimize"),
            ("output", "--output=<output-file>"),
            ("report", "--report=<report-type>"),
            ("docs", "--docs=<json-file>"),
        ],
    )
}

pub fn unknown_with(flag: &str, declarations: &[(&str, &str)]) -> String {
    let name = flag.trim_start_matches('-').split('=').next().unwrap_or("");
    let mut flags: Vec<_> = declarations
        .iter()
        .map(|(name_, suggestion)| (distance(name, name_), *suggestion))
        .collect();
    if flags.iter().any(|(distance, _)| *distance < 3) {
        flags.retain(|(distance, _)| *distance < 3);
    }
    flags.sort_by_key(|(distance, _)| *distance);
    let advice = if flags.len() == 1 {
        format!("Maybe you want {} instead?", styled(92, flags[0].1))
    } else {
        let list = flags
            .iter()
            .map(|(_, flag)| format!("    {flag}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Maybe you want one of these instead?\n\n    {}",
            styled(92, &list[4..])
        )
    };
    format!(
        "ELM_CLI_RAW:I do not recognize this flag:\n\n    {}\n\n{advice}\n\n",
        styled(91, flag)
    )
}

pub fn boolean_value(flag: &str) -> String {
    let name = flag.split('=').next().unwrap();
    format!(
        "ELM_CLI_RAW:This on/off flag was given a value:\n\n    {}\n\nAn on/off flag either exists or not. It cannot have an equals sign and value.\nMaybe you want this instead?\n\n    {}\n\n",
        styled(91, flag),
        styled(92, name)
    )
}

pub fn bad_file(path: &str) -> String {
    format!(
        "ELM_CLI_RAW:I am having trouble with this argument:\n\n    {}\n\nIt is supposed to be a {} value, like one of these:\n\n    {}\n\n",
        styled(91, path),
        styled(93, "<elm-file>"),
        styled(92, "Main.elm\n    src/Main.elm")
    )
}

pub fn flag_value_error(flag: &str, value: Option<&str>) -> String {
    let (token, examples): (&str, &[&str]) = match flag {
        "--output" => ("<output-file>", &["elm.js", "index.html", "/dev/null"]),
        "--docs" => ("<json-file>", &["docs.json", "documentation.json"]),
        "--report" => ("<report-type>", &["json"]),
        "--interpreter" => ("<interpreter>", &["node", "nodejs"]),
        _ => unreachable!(),
    };
    let ansi = std::io::stderr().is_terminal();
    let color = |code, text: &str| {
        if ansi {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    };
    let wrong = value.map_or_else(|| flag.to_owned(), |v| format!("{flag}={v}"));
    let intro = if value.is_some() {
        "This flag was given a bad value:"
    } else {
        "This flag needs more information:"
    };
    let explanation = if value.is_some() {
        format!("I need a valid {} value. For example:", color(93, token))
    } else {
        format!("It needs a {} like this:", color(93, token))
    };
    let examples = examples
        .iter()
        .map(|v| format!("    {}", color(92, &format!("{flag}={v}"))))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "ELM_CLI_RAW:{intro}\n\n    {}\n\n{explanation}\n\n{examples}\n\n",
        color(91, &wrong)
    )
}
