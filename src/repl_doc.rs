//! The plain-text subset of ansi-wl-pprint used by Elm's type renderer.
#[derive(Debug)]
pub(crate) enum Doc {
    Text(String),
    Line(bool),
    Concat(Vec<Doc>),
    Group(Box<Doc>),
    Align(Box<Doc>),
    Nest(usize, Box<Doc>),
}
impl Doc {
    pub(crate) fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }
    pub(crate) fn concat(parts: Vec<Self>) -> Self {
        Self::Concat(parts)
    }
    fn join(parts: Vec<Self>, space: bool) -> Self {
        let mut joined = Vec::new();
        for part in parts {
            if !joined.is_empty() {
                joined.push(Self::Line(space));
            }
            joined.push(part);
        }
        Self::Group(Box::new(Self::Concat(joined)))
    }
    pub(crate) fn sep(parts: Vec<Self>) -> Self {
        Self::join(parts, true)
    }
    pub(crate) fn cat(parts: Vec<Self>) -> Self {
        Self::join(parts, false)
    }
    pub(crate) fn align(self) -> Self {
        Self::Align(Box::new(self))
    }
    pub(crate) fn hang(self, amount: usize) -> Self {
        Self::Nest(amount, Box::new(self)).align()
    }

    pub(crate) fn render(&self, width: usize) -> String {
        let mut output = String::new();
        let mut column = 0;
        let mut pending = vec![(0, false, self)];
        while let Some((indent, flat, doc)) = pending.pop() {
            match doc {
                Self::Text(text) => {
                    output.push_str(text);
                    column += text.chars().count();
                }
                Self::Line(space) if flat => {
                    if *space {
                        output.push(' ');
                        column += 1;
                    }
                }
                Self::Line(_) => {
                    output.push('\n');
                    output.extend(std::iter::repeat_n(' ', indent));
                    column = indent;
                }
                Self::Concat(parts) => {
                    pending.extend(parts.iter().rev().map(|part| (indent, flat, part)))
                }
                Self::Align(inner) => pending.push((column, flat, inner)),
                Self::Nest(amount, inner) => pending.push((indent + amount, flat, inner)),
                Self::Group(inner) => {
                    let mut trial = pending.clone();
                    trial.push((indent, true, inner));
                    pending.push((
                        indent,
                        flat || fits(width as isize - column as isize, trial),
                        inner,
                    ));
                }
            }
        }
        output
    }
}

fn fits(mut remaining: isize, mut pending: Vec<(usize, bool, &Doc)>) -> bool {
    while remaining >= 0 {
        let Some((indent, flat, doc)) = pending.pop() else {
            return true;
        };
        match doc {
            Doc::Text(text) => remaining -= text.chars().count() as isize,
            Doc::Line(space) if flat => remaining -= isize::from(*space),
            Doc::Line(_) => return true,
            Doc::Concat(parts) => {
                pending.extend(parts.iter().rev().map(|part| (indent, flat, part)))
            }
            Doc::Group(inner) => pending.push((indent, true, inner)),
            Doc::Align(inner) | Doc::Nest(_, inner) => pending.push((indent, flat, inner)),
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::Doc;
    #[test]
    fn layout_respects_exact_width_and_counts_unicode_characters() {
        let doc = Doc::sep(vec![Doc::text("é".repeat(76)), Doc::text("abc")]).hang(4);
        assert_eq!(doc.render(80), format!("{} abc", "é".repeat(76)));
        assert_eq!(doc.render(79), format!("{}\n    abc", "é".repeat(76)));
    }
    #[test]
    fn group_fit_includes_text_following_the_group() {
        let doc = Doc::concat(vec![
            Doc::sep(vec![Doc::text("aaaa"), Doc::text("bbbb")]),
            Doc::text("xx"),
        ]);
        assert_eq!(doc.render(10), "aaaa\nbbbbxx");
    }
}
