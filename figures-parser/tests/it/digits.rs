use std::borrow::Cow;
use std::collections::BTreeMap;

use figures_parser::number::digits::{ascii, digit_value};

#[test]
fn table_matches_the_platform_fixture() {
    let json = include_str!("../fixtures/digits.json");
    let table: BTreeMap<String, u8> = serde_json::from_str(json).expect("fixture parses");
    assert_eq!(table.len(), 770);
    for (glyph, value) in &table {
        let c = glyph.chars().next().unwrap();
        assert_eq!(
            digit_value(c),
            Some(*value),
            "glyph {glyph:?} (U+{:04X})",
            u32::from(c)
        );
    }
    let digits = ('\0'..=char::MAX)
        .filter(|&c| digit_value(c).is_some())
        .count();
    assert_eq!(
        digits,
        table.len(),
        "every digit the reader knows is in the fixture"
    );
}

#[test]
fn letters_and_symbols_are_not_digits() {
    for c in ['a', '٬', '.', ' ', 'Ⅳ', '①', '²', '\u{10FFFF}'] {
        assert_eq!(digit_value(c), None, "{c:?}");
    }
}

#[test]
fn ascii_maps_every_script_and_keeps_the_rest() {
    assert_eq!(ascii("١٬٢٥٠"), "1٬250");
    assert_eq!(ascii("۱۲۵۰"), "1250");
    assert_eq!(ascii("１，２５０"), "1，250");
    assert_eq!(ascii("1250"), "1250");
    assert!(matches!(ascii("1250"), Cow::Borrowed(_)));
    assert!(matches!(ascii("١٢٥٠"), Cow::Owned(_)));
}
