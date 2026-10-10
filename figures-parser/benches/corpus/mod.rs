//! The inputs both benches run over: four 300 KB corpora shaped like the
//! turns the gate sees, and the replies checked against them.

use std::hint::black_box;

use figures_parser::number::scan::tokens;
use figures_parser::{
    Date, extract_numbers, misdated_weekdays, ungrounded_durations, ungrounded_emails,
    ungrounded_figures, ungrounded_identifiers, ungrounded_urls, urls,
};

/// The size every corpus is built to, in bytes.
const SIZE: usize = 300 * 1024;

/// A turn that is mostly prose and carries a handful of figures: the common
/// shape of a tool result with a message around it.
const PROSE: &str = "The quarterly review covered pipeline health, hiring and the renewal calendar. \
     Most accounts renewed on time; two asked for a revised quote and one churned. \
     Revenue for the period came to $1,250,000 against a plan of 1,200,000, with 38% \
     of new logos from referrals. Next steps: confirm the Q3 forecast by 2026-11-05. ";

/// A dumped table: nearly every token is a figure.
const TABLE: &str = "0  2026-11   15977   10140   26117   1.0842   8,220.5042\n\
     1  2026-12    9133       0    9133   1.0815   1,913.00\n";

/// Figures in every script and notation the reader knows, around ids and units.
const MULTISCRIPT: &str = "Umsatz: 1.250.000 € | الإجمالي ١٬٢٥٠٬٠٠٠ ريال | 合計は1,250,000円です | कुल ₹12,50,000 | \
     Total CHF 1'250'000 | Итого: 1 250 000 ₽ | 合计１，２５０，０００元 | id p1789368220119 v3.1 12.5K ";

/// A log with timestamps and ids: digit runs that are not figures.
const LOG: &str = "2026-10-08T14:23:11.482Z req_01J9ZK3Q7X8 user=U07ABCD1234 status=200 latency_ms=183 \
     bytes=48213 trace=4f2a9c1e7b3d trace_id=1789368220119 \n";

/// `unit` repeated to at least `SIZE` bytes.
fn repeat_to_size(unit: &str) -> String {
    unit.repeat(SIZE.div_ceil(unit.len()))
}

/// One corpus and the replies checked against it.
pub struct Case {
    /// The corpus's name.
    pub name: &'static str,
    /// The corpus.
    pub text: String,
    /// A reply whose figures the corpus carries, so the comparison exits early.
    pub grounded: &'static str,
}

/// One measured path over a case: its name and the call, which returns a
/// count so the work cannot be optimised away.
pub type Path<'c> = (&'static str, Box<dyn Fn() -> usize + 'c>);

/// The hot paths both benches measure over `case`.
#[must_use]
pub fn paths(case: &Case) -> [Path<'_>; 11] {
    let text = case.text.as_str();
    [
        ("tokens", Box::new(move || tokens(black_box(text)).count())),
        (
            "extract_numbers",
            Box::new(move || extract_numbers(black_box(text)).len()),
        ),
        (
            "ungrounded/grounded",
            Box::new(move || ungrounded_figures(case.grounded, black_box(text)).len()),
        ),
        (
            "ungrounded/ungrounded",
            Box::new(move || ungrounded_figures(UNGROUNDED_REPLY, black_box(text)).len()),
        ),
        (
            "ungrounded/signed",
            Box::new(move || ungrounded_figures(SIGNED_REPLY, black_box(text)).len()),
        ),
        ("urls", Box::new(move || urls(black_box(text)).len())),
        (
            "ungrounded_urls",
            Box::new(move || ungrounded_urls(LINK_REPLY, black_box(text), "").len()),
        ),
        (
            "ungrounded_emails",
            Box::new(move || ungrounded_emails(OTHER_REPLY, black_box(text), "").len()),
        ),
        (
            "ungrounded_identifiers",
            Box::new(move || ungrounded_identifiers(OTHER_REPLY, black_box(text), "").len()),
        ),
        (
            "ungrounded_durations",
            Box::new(move || ungrounded_durations(OTHER_REPLY, black_box(text), "").len()),
        ),
        (
            "misdated_weekdays",
            Box::new(move || misdated_weekdays(black_box(text), Date::constant(2026, 10, 2)).len()),
        ),
    ]
}

/// Every corpus with its grounded reply.
#[must_use]
pub fn all() -> Vec<Case> {
    vec![
        Case {
            name: "prose",
            text: repeat_to_size(PROSE),
            grounded: "Revenue came to $1,250,000 against a plan of 1,200,000.",
        },
        Case {
            name: "table",
            text: repeat_to_size(TABLE),
            grounded: "Direct 26,117 at a rate of 1.0842, fee 8,220.5042, December 9,133.",
        },
        Case {
            name: "multiscript",
            text: repeat_to_size(MULTISCRIPT),
            grounded: "Umsatz 1.250.000 €, in lakh 12,50,000.",
        },
        Case {
            name: "log",
            text: repeat_to_size(LOG),
            grounded: "48213 bytes on trace 1789368220119.",
        },
    ]
}

/// A reply with links, items and a placeholder no corpus carries, so the
/// link comparison reads the whole corpus for each.
const LINK_REPLY: &str = "See [#87](https://github.com/acme/app/issues/87), \
     https://linear.app/acme/issue/CORE-1710/spike and CORE-1711; the [board](url).";

/// A reply with addresses, identifiers and durations no corpus carries, so
/// each comparison reads the whole corpus.
const OTHER_REPLY: &str = "Mail zed.q@nowhere.example.org about 9f2e4c1a7b and CORE-99999; \
     the call ran 3 hours and 17 minutes, then deux heures et demie.";

/// [`UNGROUNDED_REPLY`] with signs, so the corpus is read with its signs too.
const SIGNED_REPLY: &str =
    "Revenue moved by -1,010,000, direct −26,171 and the rate -1.1785; +99,999 bytes.";

/// A reply with figures no corpus carries, so the comparison reads the whole corpus.
const UNGROUNDED_REPLY: &str =
    "Revenue came to $1,010,000, direct 26,171 and the rate 1.1785; 99,999 bytes.";
