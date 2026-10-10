# Internals

How each kind reads a text, and how a comparison decides what grounds a
reply's item. The [README](../README.md) covers usage; this page is for
contributors and for anyone who needs to know why an item was or was not
reported.

## Layout

The workspace holds two crates:

- `figures-parser`, the library, in pure Rust with `#![forbid(unsafe_code)]`.
- `figures-parser-napi`, a Node binding over it, built with napi-rs.

The library has one module per kind, each with a reader and a comparison:

| Module | Reader | Comparison |
|---|---|---|
| `number` | `scan` (tokens), `readings`, `scale` (scale words), `spelled` (numbers in words), `digits` | `ground` |
| `url` | `scan` | `ground`, `items` (the items named beside links) |
| `date` | `languages`, `names` (a port of chrono-node, and Arabic) | `calendar` |
| `email` | `emails` | `ungrounded_emails` |
| `id` | `identifiers` | `ungrounded_identifiers`, `unseen_tokens` |
| `duration` | `grammar`, `words`, `groups` | `corpus` (the forms a corpus writes a length in) |

`text` holds the character lookups and helpers the kinds share, `kind` names
the kinds a caller selects, and `utf16` converts byte offsets to UTF-16.

Every comparison follows one shape. It reads the reply's items into a small
set of keys, then streams the corpus through that set and stops as soon as
every key is found. What is left is reported once each, as the reply wrote
it. Allocation is bounded by the reply: nothing is allocated per corpus
token or per match.

## Numbers

### Tokens

`number::scan::tokens` walks the text with a byte cursor and yields numeric
tokens: digit groups joined by single symbols (comma, dot, apostrophe, the
spaces French and Russian group with, the Arabic separators, the fullwidth
comma). A token that continues a word is not one: the inside of an id
(`p1789368220119`), a version (`v3.1`) or a unit suffix (`12.5kg`). A run
that ends against a letter gives back its last group, so "12.5kg" yields
"12". A dash is not a symbol, so "2026-11-05" reads as its parts.

A token records what stands around it:

- a currency sign (`\p{Sc}`) or an ISO 4217 code before or after it, with at
  most one space between;
- a sign before it, read by a fast one-byte check, then a full look-back
  that crosses a currency mark and the bidi marks Arabic and Persian print
  ("-€1,234.50", "€ -1.234,50", "EUR-1'234.50"). A dash after a digit or a
  word letter joins instead ("CORE-1710", "10-12");
- an approximation mark (`~`, `∼`, `≈`) before the figure, its sign or its
  currency, on the same line. A tilde after a digit is a range ("10~20"),
  and the CJK wave dashes (`〜`, `～`) mean a range or "up to";
- a scale word after the digits (see [Scale words](#scale-words)).

The digit table holds the 77 runs of ten decimal digits Unicode encodes,
astral scripts such as Adlam included. Unicode's stability policy only adds
runs, and a test fails when the `unicode-general-category` tables know a
digit the table lacks. Group lengths are counted in code points, so astral
digits group like any other.

### Readings

A token yields every value it may mean:

- groups joined by one repeated symbol that fit thousands grouping are one
  integer ("1,250,000"; Indian "12,50,000");
- a last symbol that may end a decimal, after a single group or a whole
  grouped by another symbol ("1.234.567,89", "8,22"), ends a decimal part;
- groups that fit neither are neighbours printed side by side
  ("15977,10140,26117").

A thousands run also yields its groups as neighbours, marked `derived`: they
ground a reply's figure, since a printed table may mean either, but a reply
is never held to them. A single dot or comma before three digits fits both
thousands and a decimal ("1.250" is 1,250 in Berlin and 1.25 in Boston) and
yields both.

### What a reply is held to

`Reading::stated` decides which readings a reply states, by floors:

- a bare figure under 10,000 passes, being usually a count or the model's own
  arithmetic;
- a thousands separator lowers the floor to 1,000, and a currency mark to
  100;
- a decimal with four fractional digits or more is always held, and is
  grounded by any corpus value that starts with the same digits ("1.1785" is
  carried by "1.17853");
- a derived reading, or a number in words opened by an article, is never
  held.

A reply's figure with several whole readings is one claim, grounded by any
of them and reported once.

### Signs

A reply's "-12,500" needs a negative value in the corpus, and its "+12,500"
a value that is not negative. An unsigned figure takes either sign, since a
reply may put the sign in words ("fell by 12,500"). Accounting's parentheses,
"($1,234.50)", ground a negative in the corpus, but hold a reply to no sign,
since prose writes an aside the same way.

The corpus walk reads no sign up front. A corpus token's sign is read only
when one of its readings matches a signed reply figure's value, so a signed
reply costs no more than an unsigned one.

### Approximation

A figure marked approximate, scaled or written in words is grounded by any
corpus value that rounds to it at the precision it is written with: its last
nonzero digit for a whole number, its last digit for a decimal. "~12,500"
takes 12,450 to 12,549, "~12,000" takes 11,500 to 12,499, and "~1.1785"
takes 1.17846. A power of ten with one significant digit takes from where a
value rounds up to it, so "~10,000" takes 9,500 to 14,999. The range is added
to the exact key, so a mark never grounds less than the bare figure. An
approximate figure is reported as written, mark included.

### Scale words

A scale word after the digits multiplies the figure: "1.2 million",
"$4.5bn", "12.5K", "2,5 Mio.", "1,5 Mrd.", "3 milliards", "1,2 mil millones",
"5 lakh", "₹2 Cr", "1.2万", "3億", "5千万". The long forms are read case aside
in English, French, German, Spanish, Italian and Portuguese; the CLDR
abbreviations are read with their case. A scaled figure is stated as rounded,
so "$1.2 million" is grounded by 1,150,000 to 1,249,999.

An abbreviation that also names a unit or a size reads as a scale only beside
a currency: "$5m" and "€3B" scale, "5m" (minutes) and "7B" (a model's size)
do not. "M" and "K" read glued to the digits, or after a space beside a
currency ("5 M€"). "Mio" and "Bio" need their dot or a currency, since the
bare letters are also a name.

After whole digits, a Han multiplier may be followed by sections of digits
and smaller multipliers, which add to it: "3億5000万" is 350,000,000, and
"1億2345万6789" is 123,456,789. Digits alone end the sections only after 万
or a smaller multiplier, with two digits or more ("2万5000" is 25,000), since
"3万5" is spoken shorthand for 35,000 and a text writes the places below 億
through 万.

The scale check is a one-byte inline test on the byte after a token, so a
figure followed by nothing that may begin a scale costs nothing more.

### Numbers in words

`number::spelled` reads numbers written in words in English, French, German,
Spanish, Italian and Portuguese, and in Han numerals: "twelve thousand five
hundred", "quatre-vingt-dix-sept mille", "zweihundertfünfzigtausend", "doce
mil millones", "un milione duecentomila", "um milhão e duzentos mil",
"三億五千万".

One Aho-Corasick search finds the pieces of hundreds and scale words, their
first letter left out where a capital may stand ("illion" finds "Millionen").
Leftmost-first matching lets the search run on a vector searcher. Around each
find, the reader takes the number words joined by a space or a hyphen and
sums them left to right:

- each word carries the languages that write it, and a number continues only
  while its words share one ("twenty cents" mixes English and French and is
  not a number);
- a value fills the group's tens or units, a hundred multiplies the value
  before it, and a scale closes the group, smaller scales after larger ones
  ("mil millones" multiplies);
- French "quatre-vingt" and the teens after sixty and eighty, and German's
  unit before its ten ("fünfundzwanzig"), are read as written;
- German and Italian glue a number into one word, which is split into its
  pieces ("zweihundertfünfzig", "duecentomila");
- a half is read beside its scale ("two and a half million", "a million and
  a half", "deux millions et demi", "zweieinhalb Millionen", "anderthalb
  Millionen"), and a decimal point before one ("one point two five million",
  "zwei Komma fünf Millionen");
- whole digits of one to three places may lead the words ("2 million three
  hundred thousand", "1 millón y medio"); the number then stands for the
  digit token, and carries its sign.

A number must hold a hundred or a scale and two words at least, so a lone
scale word ("mille grazie", "thousands of users") is not read, nor a Han run
that is a word ("万一", "百貨店"). A number opened by an article ("a
million", "a hundred thousand") grounds a reply's figure but is never held,
since prose writes it as an idiom or a round guess ("thanks a million").

A corpus is searched for numbers in words only when its digits leave a reply
figure unfound, since most corpora write their numbers in digits.

### The comparison

`ungrounded_figures` keeps the reply's stated keys in one vector sorted by
the hash of their integer part, behind a mask of their integer lengths, and
streams the corpus's readings through it. A corpus reading whose length is
not in the mask costs one bit test. The walk reads the corpus without
currency marks and without signs, since no floor applies to a corpus, and
stops as soon as every claim is grounded.

## Links

`url::scan::urls` finds http(s) links with the `linkify` crate. Slack's
`<url|label>` wrapping is blanked in place first, so a link's span is its
place in the text as given, and the markup and punctuation that close around
a link are dropped.

`fabricated_urls(reply, corpus)` grounds a link the corpus holds verbatim, or
whose path the corpus holds without a host, as an API's `htmlPath` does, and
puts under no other host.

`ungrounded_urls(reply, corpus, prompt)` adds four checks for a reply about
to be sent:

- a link on a reserved example domain that only the prompt carried, since a
  prompt's examples are not facts;
- a link target that is not a URL, such as `[PR 512](url)`;
- an item's label on a link that is not the item's own, such as `[#87]` on
  the repository's root;
- an item named without the link a result gave for it.

## Weekdays

`misdated_weekdays(reply, today)` is a port of chrono-node 2.10.2's parsers,
run per language in chrono's order with its retry and overlap rules. A
language's patterns compile the first time a text names one of its weekdays,
behind a per-language gate. A date without a year takes the year chrono would
give it: the nearest one, the current year in Chinese, or the era a Japanese
date names. Each pair is checked where it stands, so both ends of "Mon 3 Aug
to Thu 7 Aug" are read, and a pair is reported once, as written. `jiff`
computes the calendar.

Arabic, which chrono does not read, has parsers in the same shape:

- a weekday with its article ("الجمعة"), its common spellings included
  (hamza written or not, a final `ة` written `ه`), after `يوم` ("day") or
  the conjunction `و` written onto it, with "next" or "last" after it;
- a date with a month name, in the four systems Arab countries write (يناير,
  كانون الثاني, جانفي and Morocco's يناير…دجنبر), "من" or "شهر" before the
  month, a second name after a slash ("أيلول/سبتمبر"), and a year after
  "عام" or "سنة", with or without the Gregorian `م`;
- a date in numbers, day first ("26/09/2026") or year first, in Western,
  Arabic-Indic or extended Arabic-Indic digits.

A weekday and a date merge across whitespace, one comma (`,` or `،`), the
bidi marks a formatter puts around numbers, and `الموافق` ("corresponding
to"). A Hijri date has no month these parsers read, so a weekday beside one
("الجمعة 14 ربيع الأول 1448هـ") is never paired with the Gregorian date that
may follow it.

## Email addresses

`emails(text)` finds addresses with `linkify`, `mailto:` links included.
`ungrounded_emails` compares them case aside. An address on a reserved
example domain (`example.com`, `.org` or `.net`, bare or under `www.`, the
rule links follow) counts only when the corpus carries it.

## Identifiers

`identifiers(text)` reads five shapes, leaving an id inside a link or an
address to those kinds:

- a UUID;
- a hash: 7 to 64 hexadecimal digits mixing letters and digits;
- a tracker key: capitals, a dash, digits (`CORE-1710`);
- a prefixed id: a lowercase prefix, an underscore, then letters and digits
  (`cus_9aZ3kLm2`);
- a long mixed token: ten or more letters, digits, `_` and `-`, mixing
  letters and digits.

A UUID or a hash is always held, and a short hash is grounded by the full one
it starts. A tracker key or a prefixed id is held only when the corpus holds
another of the same prefix. Long mixed tokens share their shape with product
and model names, so they are never held; `unseen_tokens` returns them apart.

## Durations

`durations(text)` reads an amount and a unit: digits, decimals with a point
or a comma, "1½", number words ("forty-five", "vingt et un",
"trentacinque"), glued runs of units ("1h30", "1h5m30s"), and fixed phrases
("half an hour", "un quart d'heure", "mezz'ora", "a fortnight"). Parts in
falling units join into one length ("1 hour and 30 minutes", "2 jours, 4
heures"). The words are curated from rustling-ontology's grammar.

Arabic has a pattern of its own, run only over a text that holds Arabic
letters (one `memchr` scan decides):

- an amount in Western or Arabic-Indic digits (`٫` as the decimal mark), or
  a number word up to ninety-nine ("خمس", "خمس عشرة", "خمس وأربعون");
- a unit in its singular, plural or dual form; a dual counts two by itself
  ("ساعتين"), while a lone singular counts one only with "واحد" after it,
  a fraction after it ("ساعة ونصف", "ساعة إلا ربع"), or another amount
  joined to it ("ساعة و20 دقيقة"), since "يوم" and "ساعة" alone are as often
  "the day of" and "the clock";
- a fraction before a unit ("نصف ساعة", "ربع ساعة", "ثلث ساعة", "ثلاثة أرباع
  الساعة");
- the conjunction `و` and the prepositions `ب`, `ل`, `ف`, `ك` written onto a
  part ("بساعتين", "و30 دقيقة"), and `و` or `،` joining two parts.

A length keeps its clock part in milliseconds and its calendar part in
hundredths of a month, since a month and a year have no fixed length.
`ungrounded_durations` grounds a length the corpus carries in any of these
forms:

- written out, or as one part of a longer duration;
- as ISO 8601 (`PT1H30M`);
- as a clock reading (`1:30:00`; `12:30` reads both as hours and minutes and
  as minutes and seconds);
- as a number under a field whose name ends in a unit's word or symbol
  (`"duration_ms": 5400000`, `"elapsedHours": 1.5`; a bare `duration` reads
  as seconds or minutes).

A clock reading right after a date (`2026-10-09 14:30`) is a time of day,
not a length. Rates ("once a day"), dimensions ("3D") and a bare `m`, as
often a metre or a million, are not durations. A number glued to a bare `s`
("30s") may be a plural ("the 90s"), so it grounds a reply's duration but is
never reported itself; `Duration::stated` says which readings a reply is held
to.

## The Node binding

`figures-parser-napi` exposes each comparison and an `extract(text, {kinds})`
reader. Every export runs under `#[napi(catch_unwind)]`, so a panic reaches
JavaScript as a thrown `Error` instead of aborting Node. Offsets are
converted to UTF-16 in one forward pass with `utf16::Utf16Cursor`.
