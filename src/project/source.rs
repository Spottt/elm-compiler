//! Immutable source bytes shared by discovery graphs. Explicit mutations keep
//! String's behavior through copy-on-write, without altering another snapshot.
use std::{ops::{Deref, DerefMut}, sync::Arc};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source(Arc<String>);
impl From<String> for Source {
    fn from(text: String) -> Self { Self(Arc::new(text)) }
}
impl From<&str> for Source {
    fn from(text: &str) -> Self { text.to_owned().into() }
}
impl Deref for Source {
    type Target = String;
    fn deref(&self) -> &String { &self.0 }
}
impl DerefMut for Source {
    fn deref_mut(&mut self) -> &mut String { Arc::make_mut(&mut self.0) }
}
impl PartialEq<&str> for Source {
    fn eq(&self, other: &&str) -> bool { self.as_str() == *other }
}
impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.as_str().fmt(f) }
}

#[cfg(test)]
mod tests {
    use super::Source;
    #[test]
    fn clones_share_bytes_until_mutation_and_preserve_unicode() {
        let original: Source = "module A exposing (..)\nvalue = \"été 🦀\"\n".into();
        let mut changed = original.clone();
        assert_eq!(original.as_ptr(), changed.as_ptr());
        changed.push_str("-- changed\n");
        assert_ne!(original.as_ptr(), changed.as_ptr());
        assert!(!original.ends_with("-- changed\n"));
        assert!(changed.contains("été 🦀"));
        changed.clear();
        assert!(!original.is_empty());
    }
}
