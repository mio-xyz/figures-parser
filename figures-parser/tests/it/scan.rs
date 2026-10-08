use figures_parser::number::scan::tokens;

fn texts(s: &str) -> Vec<&str> {
    tokens(s).map(|t| t.text).collect()
}

#[test]
fn reads_digit_groups_joined_by_single_symbols() {
    assert_eq!(texts("ARR 1,250,000 in 2026"), ["1,250,000", "2026"]);
    assert_eq!(texts("Umsatz: 1.250.000 €."), ["1.250.000"]);
    assert_eq!(texts("Total CHF 1'250'000."), ["1'250'000"]);
    assert_eq!(
        texts("Total : 1\u{202f}250\u{202f}000 €"),
        ["1\u{202f}250\u{202f}000"]
    );
    assert_eq!(texts("الإجمالي ١٬٢٥٠٬٠٠٠ ريال."), ["١٬٢٥٠٬٠٠٠"]);
    assert_eq!(texts("[15977,10140,26117]"), ["15977,10140,26117"]);
}

#[test]
fn a_token_knows_where_it_starts() {
    let t = tokens("ARR 1,250.5 now").next().unwrap();
    assert_eq!((t.text, t.start, t.currency), ("1,250.5", 4, false));
}

#[test]
fn a_space_joins_groups_but_a_double_space_or_a_dash_does_not() {
    assert_eq!(
        texts("Rows 12 5 and 1 2345 are separate."),
        ["12 5", "1 2345"]
    );
    assert_eq!(texts("2026-11-05"), ["2026", "11", "05"]);
    assert_eq!(texts("a  12  b"), ["12"]);
}

// A run gives back its last group when it ends against a letter: "12.5K"
// yields "12", "v3.1" yields "1", and an id's digits yield nothing.
#[test]
fn stands_apart_from_the_letters_of_a_word_backtracking_by_group() {
    assert_eq!(
        texts("See p1789368220119 and v3.1, about 12.5K."),
        ["1", "12"]
    );
    assert_eq!(texts("FY2026 revenue: $1,250,000."), ["1,250,000"]);
    assert_eq!(texts("2026x and 1,250K"), ["1"]);
    assert_eq!(texts("12.5K9 then 7"), ["12", "7"]);
}

#[test]
fn sits_against_letters_of_scripts_written_without_spaces() {
    assert_eq!(texts("合計は1,250,000円です。"), ["1,250,000"]);
    assert_eq!(texts("合计１，２５０，０００元"), ["１，２５０，０００"]);
    assert_eq!(texts("कुल ₹12,50,000 है।"), ["12,50,000"]);
    assert_eq!(texts("ユーザー100人"), ["100"]);
    assert_eq!(texts("ℝ1250 and 𝐀12500"), Vec::<&str>::new());
    assert_eq!(texts("ｻｰﾊﾞｰ5台"), ["5"]);
    assert_eq!(texts("注文は3件"), ["3"]);
}

#[test]
fn marks_a_currency_sign_before_the_token() {
    assert_eq!(
        tokens("The fee was $729 and € 1,458 and USD 99.")
            .map(|t| (t.text, t.currency))
            .collect::<Vec<_>>(),
        [("729", true), ("1,458", true), ("99", false)]
    );
}

#[test]
fn a_trailing_symbol_is_not_part_of_the_token() {
    assert_eq!(texts("Rows: 2,000."), ["2,000"]);
    assert_eq!(texts("Août : 1 128 616 € installés."), ["1 128 616"]);
}
