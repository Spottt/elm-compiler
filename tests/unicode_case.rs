#[allow(dead_code)]
#[path = "../src/unicode.rs"]
mod unicode;
#[allow(dead_code)]
#[path = "../src/unicode_15_1.rs"]
mod unicode_15_1;
/// The tables are included directly, so this test selects the release itself.
mod edition {
    thread_local! { pub static UNICODE_15_1: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    pub fn unicode_15_1() -> bool {
        UNICODE_15_1.with(std::cell::Cell::get)
    }
}
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

#[test]
fn predicates_follow_unicode_15_1_for_elm_0_19_2_and_later() {
    // U+1C90 GEORGIAN MTAVRULI CAPITAL LETTER AN was added in Unicode 11.
    assert!(!unicode::is_upper('\u{1C90}'));
    edition::UNICODE_15_1.with(|selected| selected.set(true));
    assert!(unicode::is_upper('\u{1C90}') && unicode::is_alpha('\u{1C90}'));
    assert_eq!(to_lower('\u{1C90}'), '\u{10D0}');
    // Combining marks stopped counting as alphanumeric.
    assert!(!unicode::is_alphanumeric('\u{0345}'));
    edition::UNICODE_15_1.with(|selected| selected.set(false));
    assert!(unicode::is_alphanumeric('\u{0345}'));
}
