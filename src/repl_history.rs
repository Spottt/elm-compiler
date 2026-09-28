//! Haskeline's default history format: UTF-8, newest line first, 100 entries.
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

pub struct History {
    path: PathBuf,
    lines: VecDeque<String>,
}
impl History {
    pub fn load(home: &Path) -> Result<Self, String> {
        let directory = home.join("0.19.1/repl");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let path = directory.join("history");
        // Like Haskeline, missing or unreadable history does not block the REPL.
        let contents = fs::read_to_string(&path).unwrap_or_default();
        let lines = contents
            .split_terminator('\n')
            .take(100)
            .map(str::to_owned)
            .collect();
        Ok(Self { path, lines })
    }
    pub fn oldest_first(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().rev().map(String::as_str)
    }
    pub fn add(&mut self, line: &str) {
        if line.chars().all(char::is_whitespace) {
            return;
        }
        self.lines.push_front(line.into());
        self.lines.truncate(100);
    }
}
impl Drop for History {
    fn drop(&mut self) {
        let mut text = String::new();
        for line in &self.lines {
            text.push_str(line);
            text.push('\n');
        }
        // Haskeline ignores write errors, too. No Rust-specific header or escapes.
        let _ = fs::write(&self.path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_duplicates_unicode_spacing_and_limits_newest_first() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("0.19.1/repl/history");
        {
            let mut history = History::load(home.path()).unwrap();
            for n in 0..105 {
                history.add(&format!("value{n}"));
            }
            history.add("  :héllo  ");
            history.add("  :héllo  ");
            history.add("\t  ");
        }
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with("  :héllo  \n  :héllo  \nvalue104\n"));
        assert!(contents.ends_with("value7\n"));
        assert_eq!(contents.lines().count(), 100);
        let history = History::load(home.path()).unwrap();
        assert_eq!(history.oldest_first().next(), Some("value7"));
    }
}
