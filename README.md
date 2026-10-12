# figures-parser

[![CI](https://github.com/mio-xyz/figures-parser/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/mio-xyz/figures-parser/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](LICENSE)
[![Rust 1.99, 2024 edition](https://img.shields.io/badge/rust-1.99%20%C2%B7%202024%20edition-orange.svg?logo=rust)](rust-toolchain.toml)
[![unsafe: forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](docs/guarantees.md)
[![Clippy: pedantic + nursery](https://img.shields.io/badge/clippy-pedantic%20%2B%20nursery-brightgreen.svg)](CONTRIBUTING.md#lint-policy)
[![Node-API 9](https://img.shields.io/badge/Node--API-9-339933.svg?logo=nodedotjs&logoColor=white)](#from-node)

Check a piece of generated text against the sources it was written from.

`figures-parser` finds the numbers, links, dated weekdays, email addresses,
identifiers and durations a text states, and tells you which of them none of
your sources carries. Run it on a language model's reply against the tool
results, messages and prompt the model read, then send the model a
correction or record what it found. It is deterministic, local, and fast
enough to run on every reply.

## Quick start

The crate is not on crates.io yet; depend on the repository:

```toml
[dependencies]
figures-parser = { git = "https://github.com/mio-xyz/figures-parser" }
```

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

## What it checks

Every check takes the reply and the corpus (everything the model read), and
where it matters the prompt. It returns the reply's ungrounded items once
each, as the reply wrote them. Every kind also has a reader that returns
what a text states, with byte spans.

| Kind | Check | Reader | A reply's item is grounded when |
|---|---|---|---|
| [Numbers](#numbers) | `ungrounded_figures` | `numbers`, `extract_numbers` | the corpus carries the same value, in any notation |
| [Links](#links) | `ungrounded_urls`, `fabricated_urls` | `urls` | the corpus holds the link, or its path under no other host |
| [Weekdays](#weekdays) | `misdated_weekdays` | | the weekday is the date's own |
| [Email addresses](#email-addresses) | `ungrounded_emails` | `emails` | the corpus or the prompt carries it, case aside |
| [Identifiers](#identifiers) | `ungrounded_identifiers` | `identifiers` | the corpus or the prompt carries it |
| [Durations](#durations) | `ungrounded_durations` | `durations` | the corpus carries the same length, in any form |

### Numbers

`ungrounded_figures(reply, corpus)` returns the figures the reply states and
the corpus does not carry. A value matches in any notation: "1,250,000",
"1.250.000", "1 250 000", "12,50,000", "١٬٢٥٠٬٠٠٠", "1.25 million" and "one
million two hundred fifty thousand" are the same number.

```rust
use figures_parser::ungrounded_figures;

let corpus = r#"{"revenue": 1250000, "plan": 1200000}"#;
let reply = "Revenue hit $1.25 million against a plan of 1.2M, up from 1,190,000.";
assert_eq!(ungrounded_figures(reply, corpus), ["1,190,000"]);
```

What to expect:

- **Small figures pass.** A bare figure under 10,000 is usually a count or
  the model's own arithmetic, so it is not checked. A thousands separator
  lowers that floor to 1,000, a currency mark to 100, and a decimal with four
  fractional digits or more is always checked.
- **Signs count.** "-12,500" needs a negative value in the corpus. An
  unsigned figure takes either sign, since prose often says "fell by 12,500".
- **Rounded figures match what they round.** "~12,500", "≈ $12,500", "$1.2
  million" and "twelve thousand five hundred" are grounded by any value that
  rounds to them, such as 12,487.
- **Ambiguous notations match either reading.** "1.250" is 1,250 in Berlin
  and 1.25 in Boston; either value in the corpus grounds it.

`numbers(text)` iterates every reading of every number, and
`extract_numbers(text)` collects them. Each reading gives its span, its
integer and fraction in ASCII digits, its sign, and whether it was grouped,
marked as money or stated as approximate.

```rust
use figures_parser::{Sign, numbers};

let first = numbers("Refund: -€1.234,50").next().unwrap();
assert_eq!((first.text, &*first.integer), ("1.234,50", "1234"));
assert_eq!((first.fraction.as_deref(), first.currency), (Some("50"), true));
assert_eq!(first.sign, Sign::Minus);
```

### Links

`fabricated_urls(reply, corpus)` returns the links no source gave. A link is
grounded when the corpus holds it verbatim, or holds its path under no other
host (as an API's `htmlPath` does).

`ungrounded_urls(reply, corpus, prompt)` is the stricter check for a reply
about to be sent. It also flags a link on an example domain only the prompt
carried, a link target that is not a URL (`[PR 512](url)`), an item's label
on a link that is not the item's own, and an item named without the link a
result gave for it.

```rust
use figures_parser::fabricated_urls;

let corpus = r#"{"html_url": "https://github.com/acme/app/pull/429"}"#;
let reply = "Merged https://github.com/acme/app/pull/429, next is https://github.com/acme/app/pull/430.";
assert_eq!(fabricated_urls(reply, corpus), ["https://github.com/acme/app/pull/430"]);
```

`urls(text)` finds http(s) links, Slack's `<url|label>` and markdown links
included, without the markup around them.

### Weekdays

`misdated_weekdays(reply, today)` finds each weekday written beside a date
and names the pairs whose weekday is wrong. `today` gives a date without a
year its year. It reads 15 languages, Arabic included (see
[Language support](#language-support)).

```rust
use figures_parser::{Date, misdated_weekdays};

let today: Date = "2026-10-09".parse().unwrap();
assert_eq!(
    misdated_weekdays("On se voit le vendredi 26 septembre.", today),
    [r#""vendredi 26 septembre" (2026-09-26 is a Saturday)"#]
);
assert_eq!(
    misdated_weekdays("الاجتماع يوم الجمعة ٢٦ سبتمبر ٢٠٢٦", today),
    [r#""يوم الجمعة ٢٦ سبتمبر ٢٠٢٦" (2026-09-26 is a Saturday)"#]
);
```

### Email addresses

`ungrounded_emails(reply, corpus, prompt)` returns the addresses neither the
corpus nor the prompt carries, ignoring case. An address on an example domain
(`example.com` and the like) counts only when the corpus carries it, since a
prompt uses those to show a format. `emails(text)` finds them, `mailto:`
links included.

```rust
use figures_parser::ungrounded_emails;

let corpus = r#"{"owner": "Ana@Acme.io"}"#;
assert_eq!(
    ungrounded_emails("Write to ana@acme.io or ben@acme.io.", corpus, ""),
    ["ben@acme.io"]
);
```

### Identifiers

`ungrounded_identifiers(reply, corpus, prompt)` checks UUIDs, hashes, tracker
keys (`CORE-1710`) and prefixed ids (`cus_9aZ3kLm2`). A UUID or a hash is
always checked, and a short hash is grounded by the full one it starts. A
tracker key or a prefixed id is checked only when the corpus holds another of
the same prefix, so an invented `CORE-1711` beside a real `CORE-1710` is
caught while names of the same shape (`GPT-4`, `UTF-8`) pass.

```rust
use figures_parser::ungrounded_identifiers;

let corpus = "CORE-1710 is open.";
assert_eq!(
    ungrounded_identifiers("Fixed in CORE-1711, built with GPT-4.", corpus, ""),
    ["CORE-1711"]
);
```

`identifiers(text)` reads them. `unseen_tokens(reply, corpus, prompt)`
returns long mixed tokens no source carries, for measurement only: they share
their shape with product and model names, so they are never reported as
errors.

### Durations

`ungrounded_durations(reply, corpus, prompt)` returns the lengths of time the
corpus does not carry. A reply's "1h30" is grounded by "90 minutes",
`PT1H30M`, `1:30:00` or `"elapsed_ms": 5400000`. Words and phrases are read
in English, French, Spanish, Italian, Portuguese and Arabic ("an hour and a
half", "deux heures et demie", "ساعة ونصف").

```rust
use figures_parser::ungrounded_durations;

let corpus = r#"{"elapsed_ms": 5400000}"#;
assert_eq!(
    ungrounded_durations("The job ran 1h30, then 45 minutes.", corpus, ""),
    ["45 minutes"]
);
```

`durations(text)` reads them, each with its length.

## From Node

`@mio-xyz/figures-parser` is a Node binding (Node-API 9, Node 22 or later)
with the same checks, named in camelCase, and one `extract` reader for every
kind. Offsets are UTF-16 units, so `input.slice(start, end)` gives an item
back.

It is published to GitHub Packages with a prebuilt binary for macOS (x64,
arm64), Windows (x64, arm64), Linux on glibc (x64, arm64, armv7) and Linux on
musl, for Alpine (x64, arm64). GitHub Packages asks for a token even to
install: a personal access token with `read:packages`, or `GITHUB_TOKEN` in
GitHub Actions. In your project's `.npmrc`:

```ini
@mio-xyz:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=${GITHUB_TOKEN}
```

```sh
npm install @mio-xyz/figures-parser
```

```js
const fp = require("@mio-xyz/figures-parser");

fp.ungroundedFigures(reply, corpus);             // ["48,200"]
fp.ungroundedUrls(reply, corpus, prompt);        // also fabricatedUrls
fp.misdatedWeekdays(reply, "2026-10-09");
fp.ungroundedEmails(reply, corpus, prompt);
fp.ungroundedIdentifiers(reply, corpus, prompt); // also unseenTokens
fp.ungroundedDurations(reply, corpus, prompt);

// Every item with its place in the input; kinds: number, url, email, id, duration.
fp.extract("Fee $1,250 at https://acme.io/x", { kinds: ["number", "url"] });
```

A panic inside the library reaches JavaScript as a thrown `Error`, never as a
crashed process.

To build it from a checkout instead, for the machine you are on:

```sh
npm ci
npm run build   # needs a Rust toolchain; writes figures-parser.<platform>.node
node smoke.mjs  # loads it through index.js, as the package does
```

## Offsets

The Rust API speaks byte offsets into the text it was given: `&text[span]`
is the item as written. `utf16::Utf16Cursor` converts them to UTF-16 offsets
in one forward pass, as the binding does.

## Language support

| Kind | Languages and scripts |
|---|---|
| Numbers | Any language: digits of every script Unicode encodes, and the separators every CLDR locale prints, Indian grouping included. Scale words and numbers in words: English, French, German, Spanish, Italian, Portuguese, lakh and crore, and Han numerals. |
| Links | Any language, internationalized domain names included. |
| Weekdays | English, French, German, Spanish, Italian, Portuguese, Dutch, Swedish, Finnish, Russian, Ukrainian, Japanese, Chinese (simplified and traditional), Vietnamese and Arabic (the month names of every Arab country, Arabic-Indic digits). |
| Email addresses | Any language, a local part outside ASCII included. |
| Identifiers | ASCII letters, digits, `-` and `_`, whatever the language around them. |
| Durations | English, French, Spanish, Italian, Portuguese and Arabic for words; unit symbols (`ms`, `s`, `h`, `d`, `w`, `y`) in any language. |

## Documentation

- [Guarantees](docs/guarantees.md): no panics, no `unsafe`, flat allocations.
- [Performance](docs/performance.md): what each check costs on a 300 KB corpus.
- [Benchmarks](docs/benchmarks.md): how those numbers are measured, and how to
  run them.
- [Known gaps](docs/known-gaps.md): what each reader does not read yet.
- [Internals](docs/internals.md): how each kind reads a text and decides what
  grounds an item.
- [Contributing](CONTRIBUTING.md): setup, checks and conventions.

## Acknowledgements

- The weekday readers port the parsers and dictionaries of
  [chrono-node](https://github.com/wanasit/chrono) 2.10.2 (MIT).
- The duration words and phrases are curated from
  [rustling-ontology](https://github.com/sonos/rustling-ontology)'s grammar
  (MIT or Apache-2.0).
- Links and addresses are found with [linkify](https://crates.io/crates/linkify),
  dates computed with [jiff](https://crates.io/crates/jiff), and currency
  codes checked against [iso_currency](https://crates.io/crates/iso_currency).

## License

Copyright (C) 2026 the figures-parser authors.

figures-parser is free software: you can redistribute it and/or modify it
under the terms of the GNU Affero General Public License, version 3 only
(`AGPL-3.0-only`), as published by the Free Software Foundation. It is
distributed in the hope that it will be useful, but without any warranty;
see `LICENSE` for the full text.

In short: you may use, study, change and share it, and whoever distributes
it, or a work built on it, must offer the complete source under the same
license. That holds for a modified version run as a service over a network
too: its users must be offered its source.

The parts derived from chrono-node and rustling-ontology keep their MIT
notices in `THIRD_PARTY_NOTICES.md`.
