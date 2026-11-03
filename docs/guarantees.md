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
were fuzzed against them with generated inputs. For numbers the last round
ran every call the TypeScript tests make (246 of them, for figures, ratios,
shared amounts, stated figures and per-unit amounts) and 20,000 generated
inputs or more per function: single figures in every notation and scale word
the TypeScript reads, mixed replies, ratios with and without the person's
words, per-unit clauses in every grouping, worksheet runs, and the readers
the other checks use (stated and printed figures, numeric tokens). The
library flags every input the TypeScript flags and passes every input it
passes, except where the TypeScript reads the text wrongly, listed below;
where the library flags more, it is a value no source carries (an amount
behind a currency code, a scale beside a currency written in letters ("24,7
M d'euros", "31.53 t rupees"), a scale word the TypeScript does not know, "Tsd",
"Mds", "MM", "crores", a Han multiplier after a space, a number in words,
an approximation mark before a bare run, a sign against the opposite sign),
and where it passes more, a value a source carries in the
reading the reply's language gives it. Links were fuzzed with 120,000
inputs, weekdays with 220,000 (60,000 of them with a time of day, a length
from now or a zone after a bare day, in all fourteen of chrono's languages),
the time-of-day reading alone with 65,000 and date windows with 112,000
(40,000 of them with times beside the dates). The per-unit check was run
beside its TypeScript original on 28,000 generated replies and prints, and
agrees on all of them once a plain space between two figures sets them
apart, as it does everywhere in the library.

## Where the library reads more accurately than the TypeScript

Each of these is pinned by a test in `figures-parser/tests/it/host.rs` or
`scales.rs`.

- **A unit or a word is no scale where the words around say so.** "took
  5m", "5m ago" and "1h 5m" (minutes), "5m tall" (metres), "a 2481t truck"
  and "weighs 25 tn" (tonnes), "300l of water" (litres), "ran 5 mi" (miles),
  "took 165489 mn" (French minutes), "5 bin bags", "earned 92 cr this term"
  (credits), "5 md files", "5 Mr Smith" and "3 Bio eggs" (organic) are read
  as units or words; the TypeScript reads each as a multiplier, and holds
  "took 5m" as five million. Without such words the abbreviation is a scale,
  as the TypeScript reads it ("3.2m users", "Revenue 5 Mio", "Gelir 729
  bin"). "3,000 CR" after a balance is accounting's credit, a scale only
  beside a currency.
- **English reads its own scale.** "2tn" is 10¹² and "$2 billion" 10⁹, as
  English writes them; the TypeScript also reads them at the Swedish thousand
  and the French 10¹², so "2tn" is grounded by a printed 2,000. Written as the
  continental locales write it ("2 tn €", "1 billion d'euros"), a name reads
  at every power they give it, as the TypeScript does.
- **A decimal mark before a scale word whose language writes it.** "8,566
  млрд" is 8.566 billion as well as 8,566 billion, Russian writing a decimal
  comma; so are "1,250 Mio." (German), "12,500 rb" (Indonesian) and "1.250
  million" (English, a decimal point). The TypeScript reads such a run as
  thousands only. Before a letter every locale writes alike ("1,250 M€",
  "46,886b €") the library reads the run as thousands too, as the TypeScript
  does, since English and continental reports write it for values a
  thousand apart. A decimal read through a word's own mark takes only that
  word's own power: "9.007 tn €" is English, never the Swedish 9.007
  thousand that would land on a printed 9,007.
- **A compound scale after a plain space.** "2 lakh crore" is 2 × 10¹²; the
  TypeScript joins the compound only with the no-break space a locale
  formats it with, and reads "2 lakh".
- **A figure is held on its own.** "Paid $500 of a 500 k€ budget" holds the
  $500, which the TypeScript lets the scaled figure of the same digits cover.
- **A plain space joins only thousands.** "3442 12,480" in a print, or "95.11
  57.44x" in a reply, are two figures; the TypeScript joins any groups a
  space sets side by side into one token, and so misses a value a script
  printed beside another, or names "95.1157.44x".
- **An approximation mark before a currency written with letters.** "≈
  R$3,000", "~US$882,915" and "≈ EUR 466,841" are approximate; the
  TypeScript reads the mark only right before a currency sign.
- **The piece of a version or a hex literal.** "5" of "v2.5" and "32" of
  "0x66.32" are no figures; the TypeScript reads them.
- **A percent sign is one figure's.** "2027 %95.24" is the Turkish "%95.24",
  and a sign on the next line marks nothing; the TypeScript reads "2027%",
  and "2,343\n%" as a percentage.

Other deliberate differences, which change what is named, not what is
flagged:

- digit groups are counted in code points, so astral digits group;
- a figure is named as written, from its sign or approximation mark on, and
  a scaled one through the currency after it ("~$1.2M", "500 k€");
- a ratio's percent sign is named as written ("19％"), a percent word after
  a space ("12 percent");
- a per-unit amount is named with its currency mark as written ("R$45.83",
  "USD 45.83");
- a link right after a backtick or an unspaced script is read;
- a `<url|label>` wrapped link is unwrapped whatever the case of its scheme;
- both ends of a weekday range are checked;
- a weekday pair is reported once, as written;
- a name of the agent's internals written against Thai, Lao, Khmer or
  Myanmar text is still a word, in the reply and in the person's words
  alike: "ใช้web_searchค้นหา" names `web_search`, where the original's
  segmenter reads it as one Thai word.

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

### Durations

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

### Links, identifiers and memory: where the library reads more accurately

The link, identifier and memory checks were run beside their TypeScript
originals on 240,000 generated link readings, 160,000 link checks (findings,
mends, fabricated links), 220,000 address prints, 160,000 built links and
message links, 40,000 opaque-id checks, 20,000 each of page lists, link
targets, memory lines and memory tokens, and every call the TypeScript tests
make. Every link the TypeScript reads, the library reads, whole or cut by one
of the rules below; every other difference is one of these, each a place
where the TypeScript reads the text wrongly:

- a link is read with its user name (`https://user@acme.io/x`), where
  linkify-it reads `https://user`;
- a backtick, a chat link's `|label>` without its `<`, and a markdown
  link's `[label](` end a link that linkify-it reads on through, so
  `https://acme.io/88a|the doc>` is `https://acme.io/88a` and
  `https://acme.io/x[docs](https://b.io/y)` is two links. A `>` that is
  prose closes no label: an arrow (`->`, `=>`), a comparison with a space
  before it (`filter > 5`), `>=`, or one after a `)` or `]` the label never
  opened (`(draft|v2)>`), so `?family=Roboto|Inter -> used` stays one link,
  as linkify-it reads it;
- a link glued to a letter of another script, an emoji or `_`, or after a
  scheme that opens no link (`http:https://b.io`), is read, and ends before
  any link linkify-it reads on its own;
- a page name outside ASCII is read as written, where Node percent-encodes
  it and the TypeScript check finds digits in `%C3%A9`: `[#4]` on
  `…/x/é_x` is misdirected, and `github.com/équipe` names no item;
- a workspace host printed in another case grounds its links;
- a link printed without its scheme grounds the same link under any host, a
  port or an address included (`acme.app/x`, `acme.io:8080/x`,
  `127.0.0.1/x`), where the TypeScript check finds one only under the
  top-level domains linkify-it lists;
- an opaque id whose prefix names a key's mode after its first word
  (`cs_test_…`, `sk_live_…`, `…_sandbox_…`) is held to the corpus, where the
  TypeScript reads none; a prefix of several words that names no mode
  (`utm_campaign_…`, `user_id_…`) is a name the reply coins and is not read,
  as the TypeScript reads none;
- `built_from_corpus` takes an http(s) link only, its slashes read as a
  browser reads them (`https:/acme.io`, `https:///acme.io`), where the
  TypeScript check also takes `ftp:`, `ws:`, `file:` or a defanged `hxxp:`
  link on a service's host as one the reply may build: the service serves
  no page there, so such a link is no page of its, and no reader opens a
  defanged one;
- `memory_claim` reads a claim written with a curly apostrophe ("I’ll
  remember"), where the TypeScript check reads a straight one only; the
  claim is the same.

The internals check, the memory layout scrub, the memory-use count and the
sheet checks were run beside their TypeScript originals on 20,000 generated
inputs each (the sheet checks on the 16,500 whose amounts both read alike),
and agree on all of them except the Thai cases above. Korean is parted from a
name as the original's segmenter parts it ("workspace.md에").
