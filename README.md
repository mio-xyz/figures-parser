# figures-parser

Check a piece of generated text against the sources it was written from.

`figures-parser` finds the numbers, links, dated weekdays, email addresses,
identifiers and durations a text states, and tells you which of them none of
your sources carries. It is built for language-model agents: run it on the
model's reply against the tool results, messages and prompt the model read,
then send the model a correction or record what it found. Everything is
deterministic, local and fast enough to run on every reply.

The workspace holds two crates:

- `figures-parser`, the library, in pure Rust with no `unsafe`.
- `figures-parser-napi`, a Node binding over it, built with napi-rs.

## Example

```rust
use figures_parser::{Date, misdated_weekdays, ungrounded_figures, ungrounded_urls};

// What the model read: a tool result.
let corpus = r#"{"total_amount": 48120.0, "count": 214,
  "report": "https://crm.acme.io/reports/q3"}"#;

// What it wrote.
let reply = "Q3 closed at $48,200 across 214 deals \
             (https://crm.acme.io/reports/q3-final).";

// 48,200 appears in no source; 214 does.
assert_eq!(ungrounded_figures(reply, corpus), ["48,200"]);

// The link's path is not the one the result gave.
assert_eq!(
    ungrounded_urls(reply, corpus, ""),
    ["https://crm.acme.io/reports/q3-final"]
);

// 26 September 2026 is a Saturday.
let today: Date = "2026-10-09".parse().unwrap();
assert_eq!(
    misdated_weekdays("The call is Fri 26 Sep.", today),
    [r#""Fri 26 Sep" (2026-09-26 is a Saturday)"#]
);
```

From Node, once the addon is built (see [Building](#building)):

```js
const fp = require("./figures-parser.linux-x64-gnu.node");

fp.ungroundedFigures(reply, corpus);      // ["48,200"]
fp.ungroundedUrls(reply, corpus, prompt); // links and items no source gave
fp.misdatedWeekdays(reply, "2026-10-09"); // pairs whose weekday is wrong

// Every figure with its place in the input, as UTF-16 offsets.
fp.extract("Fee $1,250 at https://acme.io/x", { kinds: ["number", "url"] });
```

## What it checks

Each kind has its own reader and its own rule for what grounds it. The
comparisons take the reply, the corpus (everything the model read) and, where
it matters, the prompt, and return the reply's ungrounded items once each, as
written.

| Kind | Reads | A reply's item is grounded when | Rust | Node |
|---|---|---|---|---|
| Numbers | Digits in every script Unicode encodes, any locale's separators, a currency sign or ISO 4217 code on either side | The corpus carries the same value | `ungrounded_figures` | `ungroundedFigures` |
| Links | http(s) links, Slack's `<url\|label>`, markdown links and the items named beside them | The corpus holds the link, or its path under no other host | `ungrounded_urls`, `fabricated_urls` | `ungroundedUrls`, `fabricatedUrls` |
| Weekdays | A weekday written beside a date, in 14 languages | The weekday is the date's own | `misdated_weekdays` | `misdatedWeekdays` |
| Email addresses | Addresses, mailto links included | The corpus or the prompt carries it, case aside | `ungrounded_emails` | `ungroundedEmails` |
| Identifiers | UUIDs, hashes, tracker keys (`CORE-1710`), prefixed ids (`cus_9aZ3kLm2`) | The corpus or the prompt carries it | `ungrounded_identifiers` | `ungroundedIdentifiers` |
| Durations | An amount and a unit in 5 languages, "1h30", "an hour and a half" | The corpus carries the same length, in any form | `ungrounded_durations` | `ungroundedDurations` |

### Numbers

A number is read the way a person reads it. "1,250,000", "1.250.000",
"1 250 000", "12,50,000" and "١٬٢٥٠٬٠٠٠" are the same integer. A notation that
fits two readings yields both: "1.250" is 1,250 in Berlin and 1.25 in Boston,
and either one in the corpus grounds it. A row of figures printed side by side
("15977,10140,26117") reads as three values. The digit table covers all 77
runs of ten decimal digits Unicode encodes, astral scripts such as Adlam
included, and a test fails when the Unicode data knows a digit the table lacks.

Not every number in a reply is a claim. A bare figure under 10,000 is usually
a count or the model's own arithmetic, so it passes. A thousands separator
lowers that floor to 1,000, and a currency mark to 100. A decimal with four or
more fractional digits is always checked, and is grounded by any corpus value
that starts with the same digits, so "1.1785" is carried by "1.17853".

`numbers(text)` iterates every reading of every number lazily, and
`extract_numbers(text)` collects them, each with its byte span, its integer
and fraction in ASCII digits, and whether it was grouped or marked as money.

### Links

`urls(text)` finds http(s) links with the `linkify` crate, unwraps Slack's
`<url|label>` form, and drops the markup and punctuation around a link.

`fabricated_urls(reply, corpus)` returns the links no source gave. A link is
grounded when the corpus holds it verbatim, or holds its path without a host,
as an API's `htmlPath` does, and puts that path under no other host.

`ungrounded_urls(reply, corpus, prompt)` is the stricter check for a reply
about to be sent. It also returns:

- a link on a reserved example domain that only the prompt carried, since a
  prompt's examples are not facts;
- a link target that is not a URL, such as `[PR 512](url)`;
- an item's label on a link that is not the item's own, such as `[#87]` on
  the repository's root;
- an item named without the link a result gave for it.

### Weekdays

`misdated_weekdays(reply, today)` finds each weekday written beside a date and
names the pairs whose weekday is wrong, as `"Fri 26 Sep" (2026-09-26 is a
Saturday)`. It reads English, French, German, Spanish, Italian, Portuguese,
Dutch, Swedish, Finnish, Russian, Ukrainian, Japanese, Chinese and Vietnamese.

The text side is a port of chrono-node 2.10.2's parsers, run per language in
chrono's order with its retry and overlap rules. A date without a year takes
the year chrono would give it: the nearest one, the current year in Chinese,
or the era a Japanese date names. Each pair is checked where it stands, so
both ends of "Mon 3 Aug to Thu 7 Aug" are read. A language's patterns compile
the first time a text names one of its weekdays.

### Email addresses

`emails(text)` finds addresses with `linkify`. `ungrounded_emails(reply,
corpus, prompt)` returns those neither source carries, ignoring case. An
address on a reserved example domain (`example.com`, `.org` or `.net`, bare
or under `www.`, the same rule links follow) counts only when the corpus
carries it, since a prompt uses those to show a format.

### Identifiers

`identifiers(text)` reads five shapes, leaving an id inside a link or an
address to those kinds:

- a UUID;
- a hash: 7 to 64 hexadecimal digits mixing letters and digits;
- a tracker key: capitals, a dash, digits (`CORE-1710`);
- a prefixed id: a lowercase prefix, an underscore, then letters and digits
  (`cus_9aZ3kLm2`);
- a long mixed token: ten or more letters, digits, `_` and `-`, mixing
  letters and digits.

`ungrounded_identifiers(reply, corpus, prompt)` always checks a UUID or a
hash, and grounds a short hash by the full one it starts. A tracker key or a
prefixed id is checked only when the corpus holds another of the same prefix.
That catches an invented `CORE-1711` next to a real `CORE-1710` and lets
names of the same shape, such as `GPT-4` or `UTF-8`, pass. Long mixed tokens
share their shape with product and model names, so they are never checked:
`unseen_tokens` returns them apart, for measurement.

### Durations

`durations(text)` reads an amount and a unit in English, French, Spanish,
Italian and Portuguese: digits, decimals with a point or a comma, "1½",
number words ("forty-five", "vingt et un", "trentacinque"), glued runs of
units ("1h30", "1h5m30s", "5m30s"), and fixed phrases ("half an hour", "un
quart d'heure", "mezz'ora", "a fortnight"). Parts in falling units join into
one length, as in "1 hour and 30 minutes" or "2 jours, 4 heures". The words
come from rustling-ontology's grammar.

A length keeps its clock part in milliseconds and its calendar part in
hundredths of a month, since a month and a year have no fixed length.
`ungrounded_durations(reply, corpus, prompt)` grounds a length the corpus
carries in any of these forms:

- written out, or as one part of a longer duration;
- as ISO 8601 (`PT1H30M`);
- as a clock reading (`1:30:00`; `12:30` reads both as hours and minutes and
  as minutes and seconds);
- as a number under a field whose name ends in a unit's word or symbol
  (`"duration_ms": 5400000`, `"elapsedHours": 1.5`; a bare `duration` reads
  as seconds or minutes).

A clock reading right after a date (`2026-10-09 14:30`) is a time of day,
not a length. Rates ("once a day"), dimensions ("3D") and a bare `m`, which
is as often a metre or a million, are not durations. A number glued to a
bare `s` ("30s") may be a plural ("the 90s", "100s of users"), so it grounds
a reply's duration but is never reported itself; `Duration::stated` says
which readings a reply is held to.

## Language support

| Kind | Languages and scripts |
|---|---|
| Numbers | Any language. Digits of every script Unicode encodes, and the group and decimal symbols every CLDR locale prints: comma, dot, the apostrophes Swiss German uses, the spaces French and Russian group with, the Arabic separators and the fullwidth comma. Indian grouping ("12,50,000") reads too. A test reads back how 23 locales print a number. |
| Links | Any language, internationalized domain names included. |
| Weekdays | English, French, German, Spanish, Italian, Portuguese, Dutch, Swedish, Finnish, Russian, Ukrainian, Japanese, Chinese (simplified and traditional) and Vietnamese. |
| Email addresses | Any language, a local part outside ASCII included. |
| Identifiers | Shapes of ASCII letters, digits, `-` and `_`, whatever the language around them. |
| Durations | English, French, Spanish, Italian and Portuguese for words and phrases; the unit symbols (`ms`, `s`, `h`, `d`, `w`, `y`, and `m` inside a glued run) in any language. |

## Known gaps

Each of these was checked against the library as it stands.

**Numbers**

- Number words ("twelve thousand") and Han numerals ("一万二千") are not read.
- A magnitude suffix is not expanded: "1.25M" in the corpus does not ground
  "$1,250,000" in the reply, which is reported.
- A sign is not part of the value, so "-12,500" and "12,500" compare equal.

**Links**

- A link without a scheme ("acme.io/x") is not read; a tool returns full
  links.
- A one-letter top-level domain is not a host (`https://a.b/c`).

**Weekdays**

- Languages outside the fourteen, such as Korean, Polish, Arabic, Hindi or
  Turkish, are not read.
- Two-letter abbreviations ("Mo", "Fr") are not read, since they are words in
  other languages ("to", "so", "di").
- Only a weekday written next to its date is paired. A weekday and a date
  apart in a sentence ("Friday we meet; the date is 26 Sep"), or a relative
  weekday ("next Friday"), are not checked.
- In English, a lowercase "may" and a slash date whose two numbers could each
  be the month ("3/4") are skipped as ambiguous.
- A date without a year takes the year chrono-node would give it, which can
  be the wrong one.

**Email addresses**

- An address written out to dodge scrapers ("ana at acme dot io") is not
  read.

**Identifiers**

- A lowercase tracker key ("core-1710") is not read as a key.
- A key or a prefixed id is checked only when the corpus holds another of
  the same prefix, so an invented one under a prefix the corpus never uses
  passes.
- Long mixed tokens are never checked, and ISO durations ("PT1H30M45S") and
  Slack ids ("U01ABCDEF2") land among them.

**Durations**

- German, Dutch, Swedish, Finnish, Russian, Ukrainian, Japanese, Chinese and
  Vietnamese words are not read, though the weekday check reads those
  languages.
- Number words past ninety-nine ("a hundred hours") are not read, and a
  decimal in words is misread: "one point five hours" reads as five hours.
- "An hour" alone is not read, only "an hour and a half", since "once a day"
  uses the same article.
- A range reads its upper end only: "2-3 hours" is 3 hours.
- Minutes after a spaced hour are dropped ("2h 30" is 2 hours), and "1h30s"
  is not read.
- Calendar lengths do not convert to days: "4 weeks" is not "1 month", and
  "365 days" is not "1 year".
- A time of day written "15h30" reads as a length.

## Offsets

The Rust API speaks byte offsets into the text it was given: `&text[span]` is
the item as written. JavaScript strings index UTF-16 units, so the binding
converts every offset, and `input.slice(start, end)` gives an entity back.
`utf16::Utf16Cursor` does that conversion in one forward pass for callers of
their own.

## Guarantees

**No panics.** The library forbids `unsafe` and builds under clippy's
pedantic and nursery groups plus restriction lints that deny `unwrap`,
`expect`, indexing, `as` conversions and `panic!`, so offset arithmetic goes
through checked or saturating operations. Two tests run in a release build
that also panics on integer overflow and runs debug assertions: every Unicode
code point placed next to a figure, a link and a dated weekday, and 200,000
generated inputs through every entry point.

**No crash in Node.** Every binding export catches a panic and throws it as a
JavaScript `Error`, since a panic unwinding into Node would abort the
process.

**Allocations stay flat.** A comparison allocates what it keeps from the
reply and nothing per token or per match of the corpus. The figure and
identifier comparisons stop reading the corpus once every item has been
found; the duration comparison reads the corpus's written durations whole,
then stops the same way.

**Parity where an original exists.** The number, link and weekday readers
replace TypeScript implementations, and were fuzzed against them with
generated inputs: 150,000 for numbers, 120,000 for links and 120,000 for
weekdays. Every difference that remained is a deliberate one:

- digit groups are counted in code points, so astral digits group;
- a currency code or a sign after a figure marks it too;
- a link right after a backtick or an unspaced script is read;
- a Slack link is unwrapped whatever the case of its scheme;
- both ends of a weekday range are checked;
- a weekday pair is reported once, as written.

## Performance

Worst case: a reply whose every item is ungrounded, so the whole corpus is
read. Each corpus is 300 KB. Times are the median of three criterion runs on
a shared desktop machine, release build; the same code moves by up to 15%
between builds, with code layout alone.

| Check | Prose | Dumped table | Every script | Log of ids | Allocations per call |
|---|---|---|---|---|---|
| `ungrounded_figures` | 371 µs | 1.07 ms | 1.80 ms | 429 µs | 12 |
| `ungrounded_urls` | 83 µs | 51 µs | 44 µs | 57 µs | 14 |
| `ungrounded_emails` | 34 µs | 79 µs | 39 µs | 20 µs | 3 |
| `ungrounded_identifiers` | 9 µs | 73 µs | 24 µs | 64 µs | 5 |
| `ungrounded_durations` | 1.22 ms | 1.65 ms | 1.21 ms | 1.86 ms | 13 |

A grounded reply usually returns sooner, as soon as the corpus has carried
every item. The weekday check reads only the reply and allocates nothing:
about 50 µs for a 4 KB reply that names no weekday and 100 µs for one that
does, after a one-time compile of 5 to 13 ms for each language a reply
names.

## Building

The workspace pins Rust stable in `rust-toolchain.toml`.

```sh
cargo build --release
cargo test
```

The Node addon is the `figures-parser-napi` cdylib. Build it and give it the
name Node loads:

```sh
cargo build --release -p figures-parser-napi
cp target/release/libfigures_parser_napi.so figures-parser.linux-x64-gnu.node
node smoke.mjs
```

On macOS the library is `libfigures_parser_napi.dylib`, and on Windows
`figures_parser_napi.dll`. The binding needs Node-API 9, which every
supported Node release provides.

## Development

```sh
cargo clippy --all-targets   # every warning is an error
cargo test                   # unit, integration and doc tests

# The panic-freedom tests, in a release build that panics on overflow.
# PANIC_CHECK_CASES sets the number of generated inputs (default 200,000).
cargo test --profile panic-check -p figures-parser --test it -- --ignored

# Exact allocations per call, then timings, over four 300 KB corpora.
cargo bench -p figures-parser --bench allocations
cargo bench -p figures-parser --bench figures
```

## Acknowledgements

- The weekday readers port the parsers and dictionaries of
  [chrono-node](https://github.com/wanasit/chrono) 2.10.2 (MIT).
- The duration words and phrases are curated from
  [rustling-ontology](https://github.com/snipsco/rustling-ontology)'s grammar
  (MIT or Apache-2.0).
- Links and addresses are found with [linkify](https://crates.io/crates/linkify),
  dates computed with [jiff](https://crates.io/crates/jiff), and currency
  codes checked against [iso_currency](https://crates.io/crates/iso_currency).
