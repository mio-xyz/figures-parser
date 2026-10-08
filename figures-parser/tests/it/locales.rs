use figures_parser::extract_numbers;

/// How CLDR locales print 1234567.89, as Node's ICU formats it.
const PRINTED: [(&str, &str); 23] = [
    ("en", "1,234,567.89"),
    ("en-IN", "12,34,567.89"),
    ("fr", "1\u{202f}234\u{202f}567,89"),
    ("de", "1.234.567,89"),
    ("de-CH", "1'234'567.89"),
    ("it", "1.234.567,89"),
    ("es", "1.234.567,89"),
    ("pt-BR", "1.234.567,89"),
    ("nl", "1.234.567,89"),
    ("ru", "1\u{a0}234\u{a0}567,89"),
    ("pl", "1\u{a0}234\u{a0}567,89"),
    ("sv", "1\u{a0}234\u{a0}567,89"),
    ("tr", "1.234.567,89"),
    ("ar-EG", "١٬٢٣٤٬٥٦٧٫٨٩"),
    ("fa", "۱٬۲۳۴٬۵۶۷٫۸۹"),
    ("he", "1,234,567.89"),
    ("hi", "12,34,567.89"),
    ("ja", "1,234,567.89"),
    ("ko", "1,234,567.89"),
    ("zh", "1,234,567.89"),
    ("th", "1,234,567.89"),
    ("vi", "1.234.567,89"),
    ("id", "1.234.567,89"),
];

#[test]
fn reads_the_number_every_locale_prints() {
    for (locale, text) in PRINTED {
        let read = extract_numbers(text)
            .iter()
            .any(|e| e.integer == "1234567" && e.fraction.as_deref() == Some("89"));
        assert!(read, "{locale} prints {text:?}");
    }
}
