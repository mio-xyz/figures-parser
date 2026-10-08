//! The inputs both benches run over: four 300 KB corpora shaped like the
//! turns the gate sees, and the replies checked against them.

use std::hint::black_box;

use figures_parser::number::scan::tokens;
use figures_parser::{extract_numbers, ungrounded_figures};

/// The size every corpus is built to, in bytes.
const SIZE: usize = 300 * 1024;

/// `unit` repeated to `SIZE` bytes.
fn repeat_to_size(unit: &str) -> String {
    let mut out = String::with_capacity(SIZE + unit.len());
    while out.len() < SIZE {
        out.push_str(unit);
    }
    out
}

/// A turn that is mostly prose and carries a handful of figures: the common
/// shape of a tool result with a message around it.
#[must_use]
pub fn prose() -> String {
    repeat_to_size(
        "The quarterly review covered pipeline health, hiring and the renewal calendar. \
         Most accounts renewed on time; two asked for a revised quote and one churned. \
         Revenue for the period came to $1,250,000 against a plan of 1,200,000, with 38% \
         of new logos from referrals. Next steps: confirm the Q3 forecast by 2026-11-05. ",
    )
}

/// A dumped table: nearly every token is a figure.
#[must_use]
pub fn table() -> String {
    repeat_to_size(
        "0  2026-11   15977   10140   26117   1.0842   8,220.5042\n\
         1  2026-12    9133       0    9133   1.0815   1,913.00\n",
    )
}

/// Figures in every script and notation the reader knows, around ids and units.
#[must_use]
pub fn multiscript() -> String {
    repeat_to_size(
        "Umsatz: 1.250.000 € | الإجمالي ١٬٢٥٠٬٠٠٠ ريال | 合計は1,250,000円です | कुल ₹12,50,000 | \
         Total CHF 1'250'000 | Итого: 1 250 000 ₽ | 合计１，２５０，０００元 | id p1789368220119 v3.1 12.5K ",
    )
}

/// A log with timestamps and ids: digit runs that are not figures.
#[must_use]
pub fn log() -> String {
    repeat_to_size(
        "2026-10-08T14:23:11.482Z req_01J9ZK3Q7X8 user=U07ABCD1234 status=200 latency_ms=183 \
         bytes=48213 trace=4f2a9c1e7b3d trace_id=1789368220119 \n",
    )
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
pub fn paths(case: &Case) -> [Path<'_>; 4] {
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
    ]
}

/// Every corpus with its grounded reply.
#[must_use]
pub fn all() -> Vec<Case> {
    vec![
        Case {
            name: "prose",
            text: prose(),
            grounded: "Revenue came to $1,250,000 against a plan of 1,200,000.",
        },
        Case {
            name: "table",
            text: table(),
            grounded: "Direct 26,117 at a rate of 1.0842, fee 8,220.5042, December 9,133.",
        },
        Case {
            name: "multiscript",
            text: multiscript(),
            grounded: "Umsatz 1.250.000 €, in lakh 12,50,000.",
        },
        Case {
            name: "log",
            text: log(),
            grounded: "48213 bytes on trace 1789368220119.",
        },
    ]
}

/// A reply with figures no corpus carries, so the comparison reads the whole corpus.
pub const UNGROUNDED_REPLY: &str =
    "Revenue came to $1,010,000, direct 26,171 and the rate 1.1785; 99,999 bytes.";
