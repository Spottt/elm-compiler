//! The plain-text subset of ansi-wl-pprint used by Elm's type renderer.
#[derive(Debug)]
pub(crate) enum Doc {
    Text(String),
    Highlight(Box<Doc>),
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
                Self::Highlight(inner) => pending.push((indent, flat, inner)),
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
    pub(crate) fn render_spans(&self, width: usize) -> Vec<(String, bool)> {
        fn emit(out: &mut Vec<(String, bool)>, value: &str, highlight: bool) {
            if let Some((text, previous)) = out.last_mut()
                && *previous == highlight
            {
                text.push_str(value);
            } else {
                out.push((value.into(), highlight));
            }
        }
        let mut out = Vec::new();
        let mut column = 0;
        let mut pending = vec![(0, false, false, self)];
        while let Some((indent, flat, highlight, doc)) = pending.pop() {
            match doc {
                Self::Text(text) => {
                    emit(&mut out, text, highlight);
                    column += text.chars().count();
                }
                Self::Line(space) if flat => {
                    if *space {
                        emit(&mut out, " ", highlight);
                        column += 1;
                    }
                }
                Self::Line(_) => {
                    emit(&mut out, &format!("\n{}", " ".repeat(indent)), highlight);
                    column = indent;
                }
                Self::Concat(parts) => {
                    pending.extend(parts.iter().rev().map(|p| (indent, flat, highlight, p)))
                }
                Self::Highlight(inner) => pending.push((indent, flat, true, inner)),
                Self::Align(inner) => pending.push((column, flat, highlight, inner)),
                Self::Nest(amount, inner) => {
                    pending.push((indent + amount, flat, highlight, inner))
                }
                Self::Group(inner) => {
                    let mut trial: Vec<_> =
                        pending.iter().map(|(i, f, _, d)| (*i, *f, *d)).collect();
                    trial.push((indent, true, inner));
                    pending.push((
                        indent,
                        flat || fits(width as isize - column as isize, trial),
                        highlight,
                        inner,
                    ));
                }
            }
        }
        out
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
            Doc::Highlight(inner) | Doc::Align(inner) | Doc::Nest(_, inner) => {
                pending.push((indent, flat, inner))
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::Doc;
    #[test]
    fn highlights_do_not_change_width_or_alignment() {
        let doc = Doc::sep(vec![
            Doc::text("List"),
            Doc::Highlight(Box::new(Doc::text("String"))),
        ])
        .hang(4);
        assert_eq!(
            doc.render_spans(80),
            vec![("List ".into(), false), ("String".into(), true)]
        );
        for width in [9, 10, 80] {
            let spans = doc.render_spans(width);
            assert_eq!(
                spans.into_iter().map(|(text, _)| text).collect::<String>(),
                doc.render(width)
            );
        }
    }
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
