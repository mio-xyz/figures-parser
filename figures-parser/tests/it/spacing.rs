//! Digit groups joined by any space, runs glued to a word or a currency
//! written in letters, and the words that make a scale letter no scale.

use figures_parser::{
    Amounts, mispaired_per_unit, prose_arithmetic, stated_figures, states_figure,
    ungrounded_figures,
};

const fn none() -> Vec<String> {
    Vec::new()
}

// The thin, figure, en, punctuation, hair and ideographic spaces fold into
// the plain space, and code writes an underscore: each joins thousands.
#[test]
fn joins_thousands_across_every_space_the_plain_one_folds_from() {
    for space in [
        '\u{2009}', '\u{2007}', '\u{2002}', '\u{2008}', '\u{200a}', '\u{3000}', '_',
    ] {
        let reply = format!("Revenue: 12{space}480 €");
        assert_eq!(ungrounded_figures(&reply, "x 12480"), none(), "{space:?}");
        assert_eq!(ungrounded_figures(&reply, "x"), [format!("12{space}480")]);
        let total = format!("Le total est de 4{space}570{space}163 €.");
        assert_eq!(ungrounded_figures(&total, "total: 4570163"), none());
        let stated = format!("Spent $1{space}250{space}000.00 on it.");
        let values: Vec<f64> = stated_figures(&stated).iter().map(|f| f.value).collect();
        assert_eq!(values, [1_250_000.0], "{space:?}");
    }
    let mispaired = mispaired_per_unit(
        "Spent $1\u{2009}250.00 on 25 leads ($60.00 each).",
        "spend $1,500.00 leads 25 cpl $60.00",
    );
    assert_eq!(mispaired.len(), 1);
    assert_eq!(mispaired[0].stated, "$1\u{2009}250.00 / 25 = $50.00");
    assert_eq!(
        prose_arithmetic(
            "Total 22\u{2009}172 €, average 5\u{2009}543.",
            "12\u{2009}480 + 3\u{2009}442 + 1\u{2009}250 + 5\u{2009}000 ?",
            "...print: 22172 5543",
        ),
        none()
    );
}

// An underscore that already joins the pieces of a name before the digits
// joins no groups.
#[test]
fn an_underscore_in_a_name_joins_no_groups() {
    assert_eq!(
        ungrounded_figures("job run_123_456 done", "run_123_456"),
        none()
    );
    let values: Vec<f64> = stated_figures("job run_123_456 done")
        .iter()
        .map(|f| f.value)
        .collect();
    assert!(!values.contains(&123_456.0), "{values:?}");
}

// "7,3 tn" is how Swedish compact notation prints 7,307: English never
// writes a decimal comma, so the scale word is read as the continental
// locales read it. "7.3tn" stays English.
#[test]
fn a_decimal_comma_reads_a_scale_word_as_the_locales_that_write_one() {
    assert_eq!(
        ungrounded_figures("Besökare: 7,3 tn", "visits 7307"),
        none()
    );
    assert_eq!(ungrounded_figures("Total: 12,5 tn", "count 12480"), none());
    assert_eq!(ungrounded_figures("Total: 7.3tn", "count 7307"), ["7.3tn"]);
}

// A run glued after a word is an amount when it is grouped into thousands,
// as no word's digits are, or when the letters are a currency; it is read
// whole, so the corpus's print of the whole run grounds it.
#[test]
fn reads_a_grouped_run_or_a_currency_glued_after_letters_whole() {
    assert_eq!(
        ungrounded_figures("Ingresos de3,442,000", "x"),
        ["3,442,000"]
    );
    assert_eq!(
        ungrounded_figures("Ingresos de3,442,000", "x 3442000"),
        none()
    );
    assert_eq!(ungrounded_figures("Итого12,500,000", "x"), ["12,500,000"]);
    assert_eq!(
        ungrounded_figures("Total Kč915,478,593", "x"),
        ["915,478,593"]
    );
    assert_eq!(ungrounded_figures("Total Rp1.250.000", "x"), ["1.250.000"]);
    assert_eq!(
        ungrounded_figures("Total Rp1.250.000", "jumlah 1250000"),
        none()
    );
    assert_eq!(
        Amounts::of("| m0 | Kč915,478,593 |").shared_with("value: Kč915,478,593"),
        ["915,478,593"]
    );
    assert!(states_figure("Total Kč٩٩,٩٩ milhões", "99.99"));
    assert_eq!(
        ungrounded_figures("see p1789368220119 and v2.5 and Q1,12", "x"),
        none()
    );
}

// Two-digit groups ending in one of three, joined by plain spaces, are one
// Indian-grouped amount beside a currency or a scale word; without one they
// stay the small values a list sets side by side.
#[test]
fn joins_a_spaced_lakh_run_beside_a_currency_or_a_scale() {
    assert_eq!(
        ungrounded_figures("Total ₹1 00 00 000", "x"),
        ["1 00 00 000"]
    );
    assert_eq!(
        ungrounded_figures("Total ₹1 00 00 000", "x 10000000"),
        none()
    );
    assert_eq!(ungrounded_figures("Razem 10 00 001 TL", "x"), ["10 00 001"]);
    assert_eq!(
        Amounts::of("合計 1 00 00 00 000 thousand").shared_with("value: 1 00 00 00 000 thousand"),
        ["1 00 00 00 000 thousand"]
    );
    assert_eq!(
        ungrounded_figures("scores: 10 20 30 400 overall", "x 10 20 30 400"),
        none()
    );
}

// "đ" glued after an amount is the Vietnamese đồng, not a word the run
// ends against; "4K" before a display word is a resolution.
#[test]
fn a_letter_currency_after_a_run_and_a_resolution_make_no_piece() {
    assert_eq!(
        ungrounded_figures("Doanh thu: 12.480.000đ", "dt 12480000"),
        none()
    );
    assert_eq!(
        ungrounded_figures("Doanh thu: 12.480.000đ", "dt 1"),
        ["12.480.000"]
    );
    assert_eq!(
        ungrounded_figures(
            "p95 latency is 120 ms, 1080p video, 4K screens.",
            "p95=120ms"
        ),
        none()
    );
    assert_eq!(ungrounded_figures("We have 4K users now", "x"), ["4K"]);
}

// A mark that only groups joins groups of three it joined before, so a
// no-break space before a scaled figure sets two figures apart.
#[test]
fn a_no_break_space_after_a_grouped_figure_opens_the_next() {
    for space in ['\u{a0}', '\u{202f}'] {
        let reply = format!("Revenue $1,240,000{space}5m users");
        assert_eq!(ungrounded_figures(&reply, "4200"), ["1,240,000", "5m"]);
        let fee = format!("Fee 1,458{space}5k");
        assert_eq!(Amounts::of(&fee).shared_with("1,458"), ["1,458"]);
    }
    assert_eq!(
        ungrounded_figures("Prix 1\u{a0}250\u{a0}000 €", "1250000"),
        none()
    );
}

// A times sign measures only between two measures; glued to a figure, after
// a word or ending a link's path it leaves the scale a scale.
#[test]
fn a_times_sign_makes_a_unit_only_between_measures() {
    for reply in [
        "Grew 2x 5m users",
        "Option x 5m users",
        "https://acme.io/x 5m users",
        "https://[::1]/x\n5m users",
    ] {
        assert_eq!(ungrounded_figures(reply, "1"), ["5m"], "{reply}");
    }
    assert_eq!(ungrounded_figures("the room is 3m x 5m wide", "1"), none());
}

// The word before a range's low bound measures its high one.
#[test]
fn a_time_word_before_a_range_makes_its_scale_a_unit() {
    assert_eq!(ungrounded_figures("the build takes 40-50m", "x"), none());
    assert_eq!(ungrounded_figures("takes 20-30m", "25 minutes"), none());
    assert_eq!(ungrounded_figures("длилось ~47 – 52m", "x"), none());
    assert_eq!(ungrounded_figures("we have 5-10m users", "x"), ["10m"]);
}

// A dash after a line break opens a list item, not a range a printed bound
// grounds.
#[test]
fn a_line_break_ends_a_round_range() {
    for page in [
        "- 1.2k\n- 9,100,000",
        "1.2k\n\n- 9,100,000",
        "1.2k\r\n- 9,100,000",
    ] {
        assert_eq!(ungrounded_figures(page, "1.2k"), ["9,100,000"], "{page:?}");
    }
    assert_eq!(ungrounded_figures("$3\n- 9,100,000", "$3"), ["9,100,000"]);
}

// A scaled figure is named from the currency mark glued before it.
#[test]
fn names_a_scaled_figure_with_its_currency_mark() {
    assert_eq!(
        ungrounded_figures("Created it for €34,0M.", "x"),
        ["€34,0M"]
    );
    assert_eq!(
        ungrounded_figures("Created it for $34.0M.", "x"),
        ["$34.0M"]
    );
    assert_eq!(ungrounded_figures("Created it for €3,4 M", "x"), ["€3,4 M"]);
    assert_eq!(ungrounded_figures("about ~$1.2M", "x"), ["~$1.2M"]);
    assert_eq!(ungrounded_figures("USD 34M budget", "x"), ["34M"]);
}
