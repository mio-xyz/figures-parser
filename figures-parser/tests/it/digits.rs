use figures_parser::number::digits::{ascii_in, digit_value};
use unicode_general_category::{GeneralCategory, get_general_category};

// The table must hold every decimal digit the Unicode tables know; it may
// hold digits newer than them, which they report as unassigned.
#[test]
fn the_table_holds_every_decimal_digit_unicode_encodes() {
    let mut digits = 0;
    for c in '\0'..=char::MAX {
        let category = get_general_category(c);
        let known = digit_value(c).is_some();
        if category == GeneralCategory::DecimalNumber {
            assert!(
                known,
                "U+{:04X} is a decimal digit missing from the table",
                u32::from(c)
            );
        }
        if known {
            assert!(
                matches!(
                    category,
                    GeneralCategory::DecimalNumber | GeneralCategory::Unassigned
                ),
                "U+{:04X} is in the table but not a decimal digit",
                u32::from(c)
            );
            digits += 1;
        }
    }
    assert_eq!(digits, 770);
}

#[test]
fn reads_each_scripts_digits_by_their_place_in_the_run() {
    for (c, value) in [
        ('0', 0),
        ('9', 9),
        ('٣', 3),
        ('۷', 7),
        ('७', 7),
        ('๙', 9),
        ('５', 5),
        ('𞥕', 5),
        ('𝟗', 9),
        ('\u{1D7EE}', 2),
    ] {
        assert_eq!(digit_value(c), Some(value), "{c:?}");
    }
}

#[test]
fn letters_and_symbols_are_not_digits() {
    for c in ['a', '٬', '.', ' ', 'Ⅳ', '①', '²', '\u{10FFFF}'] {
        assert_eq!(digit_value(c), None, "{c:?}");
    }
}

#[test]
fn ascii_in_maps_every_script_and_keeps_the_rest() {
    let mut scratch = String::new();
    for (written, ascii) in [
        ("١٬٢٥٠", "1٬250"),
        ("۱۲۵۰", "1250"),
        ("１，２５０", "1，250"),
    ] {
        assert_eq!(ascii_in(written, &mut scratch), ascii);
    }
    let plain = "1250";
    assert!(std::ptr::eq(ascii_in(plain, &mut scratch), plain));
}
