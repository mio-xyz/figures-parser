use std::collections::BTreeMap;

use figures_parser::extract_numbers;

#[test]
fn reads_the_number_every_locale_prints() {
    let fixture = include_str!("../fixtures/locale-numbers.json");
    let printed: BTreeMap<String, String> =
        serde_json::from_str(fixture).expect("locale-numbers.json is an object of strings");
    assert!(!printed.is_empty());
    for (locale, text) in printed {
        let read = extract_numbers(&text)
            .iter()
            .any(|e| e.integer == "1234567" && e.fraction.as_deref() == Some("89"));
        assert!(read, "{locale} prints {text:?}");
    }
}
