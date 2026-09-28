#[allow(dead_code)]
#[path = "../src/unicode.rs"]
mod unicode;
use unicode::to_lower;

#[test]
fn lowercase_uses_ghc_simple_mapping_and_its_unicode_version() {
    for (input, expected) in [
        ('A', 'a'),
        ('É', 'é'),
        ('İ', 'i'),
        ('ǅ', 'ǆ'),
        ('Σ', 'σ'),
        ('K', 'k'),
        ('ẞ', 'ß'),
        ('𐐀', '𐐨'),
        ('Ა', 'Ა'),
        ('ß', 'ß'),
    ] {
        assert_eq!(to_lower(input), expected, "{input}");
    }
}
