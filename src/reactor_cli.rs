//! Terminal flag parsing and documents from Elm 0.19.1 (LICENSE-ELM).
fn color(code: u8, value: &str, ansi: bool) -> String {
    if ansi {
        format!("\x1b[{code}m{value}\x1b[0m")
    } else {
        value.into()
    }
}

pub fn help(ansi: bool) -> String {
    format!(
        "The `reactor` command starts a local server on your computer:\n\n    {}\n\nAfter running that command, you would have a server at <http://localhost:8000>\nthat helps with development. It shows your files like a file viewer. If you\nclick on an Elm file, it will compile it for you! And you can just press the\nrefresh button in the browser to recompile things.\n\nYou can customize this command with the following flags:\n\n    {}\n        The port of the server (default: 8000)\n\n",
        color(96, "elm reactor", ansi),
        color(36, "--port=<port>", ansi)
    )
}

pub fn parse(mut args: Vec<String>, ansi: bool) -> Result<Option<isize>, String> {
    if args.iter().any(|arg| arg == "--help") {
        return Ok(None);
    }
    let port = if let Some(index) = args
        .iter()
        .position(|arg| arg == "--port" || arg.starts_with("--port="))
    {
        let flag = args.remove(index);
        let value = if let Some(value) = flag.strip_prefix("--port=") {
            value.to_owned()
        } else if args.get(index).is_some_and(|arg| !arg.starts_with('-')) {
            args.remove(index)
        } else {
            return Err(port_error(None, ansi));
        };
        read_int(&value).ok_or_else(|| port_error(Some(&value), ansi))?
    } else {
        8000
    };
    if let Some(flag) = args.iter().find(|arg| arg.starts_with('-')) {
        return Err(format!(
            "ELM_CLI_RAW:I do not recognize this flag:\n\n    {}\n\nMaybe you want {} instead?\n\n",
            color(91, flag, ansi),
            color(92, "--port=<port>", ansi)
        ));
    }
    if !args.is_empty() {
        let (what, pronoun) = if args.len() == 1 {
            ("this argument", "it")
        } else {
            ("these arguments", "them")
        };
        return Err(format!(
            "ELM_CLI_RAW:I was not expecting {what}:\n\n    {}\n\nTry removing {pronoun}?\n\n",
            color(91, &args.join("\n    "), ansi)
        ));
    }
    Ok(Some(port))
}

fn port_error(value: Option<&str>, ansi: bool) -> String {
    let (intro, flag, advice) = if let Some(value) = value {
        (
            "This flag was given a bad value:",
            format!("--port={value}"),
            format!(
                "I need a valid {} value. For example:",
                color(93, "<port>", ansi)
            ),
        )
    } else {
        (
            "This flag needs more information:",
            "--port".into(),
            format!("It needs a {} like this:", color(93, "<port>", ansi)),
        )
    };
    format!(
        "ELM_CLI_RAW:{intro}\n\n    {}\n\n{advice}\n\n    {}\n    {}\n\n",
        color(91, &flag, ansi),
        color(92, "--port=3000", ansi),
        color(92, "--port=8000", ansi)
    )
}

// Haskell's Read Int accepts surrounding parentheses/whitespace, radix prefixes
// and a minus sign; conversion from Integer wraps to the host's signed word.
fn read_int(source: &str) -> Option<isize> {
    let mut text = source.trim();
    while text.starts_with('(') && text.ends_with(')') {
        text = text[1..text.len() - 1].trim();
    }
    let negative = text.starts_with('-');
    if negative {
        text = text[1..].trim_start();
    }
    let (base, digits) = if text.starts_with("0x") || text.starts_with("0X") {
        (16, &text[2..])
    } else if text.starts_with("0o") || text.starts_with("0O") {
        (8, &text[2..])
    } else {
        (10, text)
    };
    if digits.is_empty() {
        return None;
    }
    let mut number = 0u64;
    for digit in digits.chars() {
        number = number
            .wrapping_mul(base as u64)
            .wrapping_add(digit.to_digit(base)? as u64);
    }
    Some(if negative {
        0u64.wrapping_sub(number)
    } else {
        number
    } as isize)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_syntax_matches_read_int_instead_of_unsigned_port_parsing() {
        for value in [
            "8000",
            "0x1f40",
            "0o17500",
            "(( 8000 ))",
            "18446744073709559616",
        ] {
            assert_eq!(read_int(value), Some(8000));
        }
        assert_eq!(read_int("(- 1)"), Some(-1));
        for value in ["", "+8000", "1.0", "1_000", "(1)2", "0x", "--1"] {
            assert_eq!(read_int(value), None);
        }
    }
    #[test]
    fn help_precedes_validation_and_port_precedes_unknown_flags() {
        let args = |items: &[&str]| items.iter().map(|s| s.to_string()).collect();
        assert_eq!(parse(args(&["--bad", "--help"]), false).unwrap(), None);
        assert!(
            parse(args(&["--bad", "--port=nope"]), false)
                .unwrap_err()
                .contains("bad value")
        );
        assert!(
            parse(args(&["--port=8000", "--port=nope"]), false)
                .unwrap_err()
                .contains("recognize")
        );
        assert_eq!(parse(vec![], false).unwrap(), Some(8000));
    }
}
