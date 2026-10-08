use figures_parser::number::scan::tokens;
use figures_parser::utf16::{Utf16Cursor, units};

#[test]
fn converts_byte_offsets_to_utf16_and_slices_the_input() {
    let s = "👍 total 1,250 😀 then ١٢ end";
    let u: Vec<u16> = s.encode_utf16().collect();
    let mut cursor = Utf16Cursor::new(s);
    let found: Vec<(&str, u32, u32)> = tokens(s)
        .map(|t| {
            let start = cursor.at(t.start);
            (t.text, start, start + units(t.text))
        })
        .collect();
    assert_eq!(found, [("1,250", 9, 14), ("١٢", 23, 25)]);
    for (text, start, end) in found {
        assert_eq!(
            String::from_utf16(&u[usize::try_from(start).unwrap()..usize::try_from(end).unwrap()])
                .unwrap(),
            text
        );
    }
}

#[test]
fn going_backwards_restarts_and_still_answers() {
    let s = "😀a😀b";
    let mut cursor = Utf16Cursor::new(s);
    assert_eq!(cursor.at(9), 5);
    assert_eq!(cursor.at(4), 2);
    assert_eq!(cursor.at(s.len()), 6);
}
