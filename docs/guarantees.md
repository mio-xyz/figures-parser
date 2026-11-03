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

The number, link and date readers replace TypeScript implementations, and
were fuzzed against them with generated inputs: 150,000 for numbers, 120,000
for links, 220,000 for weekdays (60,000 of them with a time of day, a length
from now or a zone after a bare day, in all fourteen of chrono's languages),
65,000 for the time-of-day reading alone and 112,000 for date windows (40,000
of them with times beside the dates). The per-unit check was run beside its
TypeScript original on 28,000 generated replies and prints, and agrees on
all of them once a plain space between two figures sets them apart, as it
does everywhere in the library. Every difference that remained is a
deliberate one:

- digit groups are counted in code points, so astral digits group;
- a sign before a figure is part of its value;
- a figure marked approximate is grounded by a value that rounds to it;
- a scale abbreviation reads at the one scale of the language that writes it
  so ("2tn" is never 2,000), and a lowercase "m" or "t" glued to the digits
  reads as a scale only beside a currency ("took 5m" is minutes);
- a figure is held on its own when a scaled figure beside it shares its
  digits: "Paid $500 of a 500 k€ budget" still holds the $500;
- a scale word after the digits multiplies the figure;
- a reply's figure that fits two readings is grounded by either;
- a number written in words is read;
- a currency code or a sign after a figure marks it too;
- a link right after a backtick or an unspaced script is read;
- a `<url|label>` wrapped link is unwrapped whatever the case of its scheme;
- both ends of a weekday range are checked;
- a weekday pair is reported once, as written.

### Dates read more accurately than the TypeScript check

Where the TypeScript check reads a date or a time the way no reader would,
this library reads it as written. Each case is pinned by a test in
`tests/it/dates.rs`.

- **A dash between two dates is a range, never a zone.** chrono reads the
  "-14" of "29 octobre–14 déc." as the zone UTC-14 after "29 octobre", so
  the TypeScript check opens no window there, and it names the window of
  "1-22 nov - 6 posts" "1-22 nov - 6". No zone is written after a date with
  no time; this library opens the window the two dates write and names it
  without the count.
- **A date merges with its time before ranges are joined, in every
  language.** chrono's Chinese, Vietnamese, Finnish and German refiners join
  a range first, so "10月2号15点30分-10月23日" (October 2 at 15:30 to October
  23) is a window of October 2 alone there, and the row "4. Nov. - 15 Uhr"
  a range of a date and an hour that is never checked. This library reads
  them as chrono reads the same words in English: the window runs to the
  23rd, and the row is November 4 at 15:00.
- **A weekday pair is checked where it stands.** chrono loses a pair to a
  result beside it: "Fri 05" to the zone it reads in "+11mins", "Sunday 21
  Nov" to a range with the time on the line before, "Montag 20" to a range
  with "Total: 15". The weekday is still written beside its date, so this
  library checks it.
- **A bare number beside a date is not named as its hour.** chrono reads the
  "15" of "mardi 7 nov 15 personnes" as 15:00 and the TypeScript check names
  the pair with it; this library names a time beside a date only when it is
  written as one (with minutes, a meridiem, or a word such as "at", "à",
  "Uhr" or "時"), so the finding names "mardi 7 nov".
- **A part of the day after a weekday names no date.** chrono's Chinese
  parser dates "上午" (morning) today, and its German one reads "morgen"
  after a weekday as tomorrow, so the TypeScript check holds "星期二上午"
  (Tuesday morning) and "Freitag morgen" (Friday morning) to today and
  tomorrow, while it misses "Freitag, morgen" (Friday, tomorrow), whose
  " morgen" it reads as a morning. This library reads a part of the day as
  no date, and "morgen" after a weekday and a comma, in German and Dutch, as
  tomorrow.
- **A dotted date is day first, in English too.** chrono's English parser
  reads "5.1.2025" as May 1, so the TypeScript check flags "Sunday 5.1.2025"
  (5 January 2025 was a Sunday) and opens the window "3.1.2025-9.1.2025" on
  March 1. No language writes dotted dates month first; this library reads
  them day first, and lets a weekday pair be when it is right month first.
- **A Japanese slash date is read whole.** chrono rejects "13/05" as a month
  13 and retries one digit in, so the TypeScript check names "3/05(木)" and
  checks March 5, and reads "31/10-11月7日" as a window from January 10. A
  number that cannot be a month is the day; this library reads May 13 and
  October 31.
- **A two-digit year that no hour can be is a year.** chrono reads the "24"
  of "samedi 13 juillet 24" as 24:00 and drops the date, so the TypeScript
  check passes "dimanche 13 juillet 24"; 13 July 2024 was a Saturday, and
  this library flags it.
- **A count after a range is no year.** chrono reads "Aug 4 - Aug 5, 12
  posts" as running to August 5, 2012, so the TypeScript check flags a row
  dated August 5 under that very window. A one- or two-digit number after
  the comma that a word follows, with no other number on the line, is a
  count here.
- **"a" and "até" join a Spanish or Portuguese range.** chrono's refiners
  for those languages join two dates with a dash alone, so "28 de março a 4
  abr." opens no window there. This library reads the range words the two
  languages write, "a", "al", "hasta", "até" and "ao".
- **A heading names the window of the rows under it.** The TypeScript check
  reads each language apart, so a heading one language cannot read keeps
  that language's earlier window open over the rows under it, and flags a
  row against a window its heading does not name. Here a line that any
  language reads as a window ends the window every other language had open.
- **A two-digit year before a Chinese date is this century's.** chrono keeps
  "03 11月5日" in the year 3; this library reads 2003, as chrono reads a
  two-digit year everywhere else.

The duration check replaces a TypeScript original that reads a second, a
minute and an hour after digits, and was fuzzed against it on 75,000
generated replies and corpora. Every difference that remained is a
deliberate one, listed under [known gaps](known-gaps.md#where-this-library-reads-durations-more-strictly-than-the-hosts-typescript):
words, phrases, fractions and glued runs are read too; a decimal is held to
its place; "and" joins parts, in a reply and in a corpus; a full stop after
a full unit name, a line break and a Latin letter after a unit end what the
original reads on through; a range written high to low is read as a span.
Like the original, a reply is held to no time of day ("15h30"), to no length
in days or longer units alone, and to the grouped reading of its digits.
