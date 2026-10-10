# Guarantees

What the library promises a caller, and how each promise is checked.

## No `unsafe`

The library crate is `#![forbid(unsafe_code)]`, and the workspace denies
`unsafe_code` everywhere. The Node binding's only `unsafe` is what the napi-rs
macros generate.

## No panics

The library builds under clippy's pedantic and nursery groups, plus
restriction lints that deny `unwrap`, `expect`, indexing and slicing, `as`
conversions, string slicing and `panic!`. Offset arithmetic therefore goes
through checked or saturating operations, and every lookup through `get`.

Two tests run that claim in a release build that also panics on integer
overflow and runs debug assertions (the `panic-check` profile):

- every Unicode code point, placed next to a figure, a link and a dated
  weekday, through every entry point;
- 200,000 generated inputs, built from the pieces each reader looks for, as
  reply, corpus and prompt of every comparison.

CI runs both on every push. `PANIC_CHECK_CASES` raises the number of
generated inputs for a longer run.

## No crash in Node

Every export of the binding runs under `#[napi(catch_unwind)]`: a panic
reaches JavaScript as a thrown `Error`. A panic unwinding into Node would
otherwise abort the process.

## Deterministic

The same inputs give the same output on every run and every machine. Nothing
reads the clock, the locale or the environment: a weekday check takes
today's date as an argument.

## Allocations stay flat

A comparison allocates what it keeps from the reply, and nothing per token or
per match of the corpus. The figure and identifier comparisons stop reading
the corpus once every item has been found; the duration comparison reads the
corpus's written durations whole, then stops the same way. The
[allocation bench](benchmarks.md#allocations) counts every allocation exactly,
so a change that adds one per token shows.

## Parity where an original exists

The number, link and weekday readers replace TypeScript implementations, and
were fuzzed against them with generated inputs: 150,000 for numbers, 120,000
for links and 120,000 for weekdays. Every difference that remained is a
deliberate one:

- digit groups are counted in code points, so astral digits group;
- a sign before a figure is part of its value;
- a figure marked approximate is grounded by a value that rounds to it;
- a scale word after the digits multiplies the figure;
- a reply's figure that fits two readings is grounded by either;
- a number written in words is read;
- a currency code or a sign after a figure marks it too;
- a link right after a backtick or an unspaced script is read;
- a Slack link is unwrapped whatever the case of its scheme;
- both ends of a weekday range are checked;
- a weekday pair is reported once, as written.
