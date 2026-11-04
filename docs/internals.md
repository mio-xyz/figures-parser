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
| `number` | `scan` (tokens), `readings`, `scale` (scale words), `spelled` (numbers in words), `digits` | `ground` (figures, and amounts against one print), `ratio`, `arithmetic`, `per_unit` |
| `url` | `scan` | `ground`, `items` (the items named beside links), `shapes` (links built from printed ids) |
| `date` | `languages`, `names` (a port of chrono-node, and Arabic), `clock` (its times of day, `east` for Japanese, Chinese and Vietnamese, `relative` for lengths from now) | `calendar`, `bare` (a weekday beside a bare day), `casual` (a weekday beside a casual word for a day), `window` (rows under a date window) |
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

A plain space joins groups only into thousands: a lead of one to three
digits, then groups of three ("12 500", "1 128 616,50"), never after the
piece of a date or a time. Anywhere else it sets numbers side by side, as a
script's `print` does, so "1,250,000 980,000" and "2026 25%" are two numbers
each. The no-break spaces locales print between groups always join.

Arabic glues a conjunction or a preposition to the next word, a number
included: "و٣٤٠" is "and 340", "ب١٢٠" "for 120", "وب٥٠٠" "and for 500". One
of و, ف, ب, ل or ك standing as a word, or two of them led by و or ف, does not
make the digits after it part of a word.

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
- a scale word after the digits lowers it to 1,000, with or without a
  currency, so a count written "3.2M teachers" or "4K" is held as "$8k" is;
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

A figure scaled or written in words is grounded by any corpus value that
rounds to it at the precision it is written with: its last nonzero digit for
a whole number, its last digit for a decimal. "1.2 million" takes 1,150,000
to 1,249,999. A power of ten with one significant digit takes from where a
value rounds up to it, so "10 million" takes 9,500,000 to 14,999,999.

A figure behind an approximation mark takes a whole unit of that digit on
either side, both ends included, since a model writes "~3,440" for a 3,449 it
cut as often as for one it rounded: "~12,500" takes 12,400 to 12,600,
"~12,000" takes 11,000 to 13,000, "~3M" takes 2M to 4M and "~1.1785" takes
1.1784 to 1.1786. The mark is read off the reading's full text, which opens
with it.

The range is added to the exact key, so a mark never grounds less than the
bare figure. An approximate figure is reported as written, mark included.

### Scale words

A scale word after the digits multiplies the figure: "1.2 million",
"$4.5bn", "12.5K", "2,5 Mio.", "1,5 Mrd.", "3 milliards", "1,2 mil millones",
"5 lakh", "₹2 Cr", "1.2万", "3億", "5千万". The long forms are read case aside
in English, French, German, Spanish, Italian and Portuguese; the CLDR
abbreviations are read with their case. A scaled figure is stated as rounded,
so "$1.2 million" is grounded by 1,150,000 to 1,249,999.

An abbreviation that also names a unit reads as a scale in a reply only
beside a currency: "$5m" scales, "5m" (minutes), "5mm" and "5 mi" do not. The
capitals "M", "K", "B" and "T", and "k", "b" and "tn", read glued to the
digits ("3.2M", "7B", "2.8T"), and after a space only beside a currency ("5
M€"), where a lone letter is otherwise a unit or a word ("500 m", "1750 N").
"Mio" and "Bio" need their dot or a currency, since the bare letters are also
a name. A long form reads glued to the digits too ("12million").

The long forms of Dutch, Polish, Swedish, Turkish, Indonesian and Vietnamese
("3 miljoen", "4 miliony", "5 milyar", "7 juta", "3 tỷ") are read with the
others, and the scale words of other scripts after the digits, glued or after
a space: Russian ("3 млн", "2 миллиарда"), Arabic, Persian, Hebrew, Hindi
("2 लाख") and Thai, which glues them to the next word as well ("5ล้านคน"). A
word after a thousand or a lakh can multiply it: "mil millones", "mila
miliardi", "lakh crore". Each abbreviation reads at the one scale of the
language that writes it so: "tn" is 10¹², though Swedish writes a thousand
"tn".

A corpus reads scale words loosely (`Tokens::loose_scales`): an abbreviation
in any case and without the currency or the dot a reply needs ("38.08 M",
"2,5 Mio", "took 5 m", "12 k rows"), and a name glued to the digits even when
it runs into the next word, as a scraped page prints "43Kmembers" or
"1.5bnviews", read as its longest name. Such a token yields its digits' own
readings beside the scaled ones, since the letters may be a unit after all; a
reading taken more widely only ever grounds more.

Hangul writes the same multipliers (천, 만, 억, 조) and reads as Han does
("3억5000만" is 350,000,000).

After whole digits, a Han multiplier may be followed by sections of digits
and smaller multipliers, which add to it: "3億5000万" is 350,000,000, and
"1億2345万6789" is 123,456,789. Digits alone end the sections with two digits
or more ("2万5000" is 25,000), since "3万5" is spoken shorthand for 35,000. A
section after 億 or 兆 that stops short of the myriad is such shorthand too
and fills the place a text writes through 万: "3億5千" and "3億5000" are
3億5000万. A large multiplier after small ones multiplies all before it:
"1千200万" is 1,200万, 12,000,000. The scanner keeps what the sections add
on the token, beside the power of its multipliers, and keeps them only when
the whole fits a `u128`.

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
  "zwei Komma fünf Millionen"); the fraction's words may be two-digit values
  ("deux virgule vingt-cinq millions", "fünfundzwanzig") or digits ("2 point
  5 million"), and a half or a decimal ends the number as a scale does;
- "zero" opens a number only before a decimal point ("zero point five
  million"), and never fills a group;
- a value under a hundred standing bare after a million or more is left out,
  since prose writes it for what follows ("two million three years ago");
- a number word hyphenated to a word that is none is a compound of its own
  ("two-bedroom", "forty-foot") and no part of a number, unless a hyphen
  joins it to a number word on its other side ("two-million-dollar");
- whole digits of one to three places may lead the words ("2 million three
  hundred thousand", "1 millón y medio"), where the scanner reads the digits
  as a figure of their own; only a hundred, a scale, a join or a point
  continues them, so "June 30 two hundred thousand" is a date and a count.
  The number then stands for the digit token and for every token inside it,
  and carries its sign.

When the words after an anchor stop continuing one number, the reader starts
again after the last complete prefix, so "5 and a hundred thousand" yields
"a hundred thousand" with its article.

A number must hold a hundred or a scale and two words at least, so a lone
scale word ("mille grazie", "thousands of users") is not read, nor a Han run
that is a word ("万一", "百貨店"). A number opened by an article ("a
million", "a hundred thousand") grounds a reply's figure but is never held,
since prose writes it as an idiom or a round guess ("thanks a million"); "a
half million" counts, as "half a million" does.

A corpus is searched for numbers in words only when its digits leave a reply
figure unfound, since most corpora write their numbers in digits.

### The comparison

`ungrounded_figures` keeps the reply's stated keys in one vector sorted by
the hash of their integer part, behind a mask of their integer lengths, and
streams the corpus's readings through it. A corpus reading whose length is
not in the mask costs one bit test. The walk reads the corpus without
currency marks and without signs, since no floor applies to a corpus, and
against letters, since a result that glues a unit on ("7,103MW") still
printed the figure, and stops as soon as every claim is grounded. A corpus
token is passed over before its readings are computed when it has too few
digits for the shortest key: a reading's integer part has at most the
token's digits, one more when a decimal rounds up, and a bare run (no
symbol) grounds no precise decimal, so it is held to the shortest whole or
approximate key. A corpus
decimal whose fraction starts with five or more also grounds the whole
above it ("327.76" grounds 328): the walk writes the incremented integer
into a buffer the needles keep and looks it up like any reading, against
exact whole keys only.

The reply is read with its links skipped, by span: a link's digits (a Slack
ts, an id) are the link check's. A bare digit run is never a claim
(`Reading::stated`), since a code or an order number is one as often as an
amount; `Reading::stated_amount(true)` holds such a run from 10,000 up, for
`Amounts`, which reads the reply once and grounds a clone of its claims, one
vector, against each print (`shared_with`), ratios skipped as well. The keys
stay apart from the claims for that: `Needles` holds what a walk reads,
`Claims` what it closes.

Once the walk leaves a claim open, the reply's round ranges are read: two
tokens with nothing but a dash between them (a currency mark and spaces
aside), each a whole number of at most two significant digits, low to high.
One corpus pass then marks each range a printed value falls inside, and the
claims stated as exactly its bounds close.

A claim still open after that, stated as a whole number of at most two
significant digits ("$3,200", "45,000"), is then read at that precision: one
more corpus pass rounds each printed value to the claim's unit (100 for a
four-digit figure, halves up) and closes the claim the result equals, the
claim's sign still held. A figure of one significant digit is read at two,
so "3,000" takes 2,950 to 3,049 and never 2,500 to 3,499; a turn printing
thousands of prices thus grounds few round figures by chance.

### Ratios

`number::ratio` reads a ratio as a numeric token with a percent mark after
it: a sign (`%`, the Arabic `٪`, the fullwidth `％`, the small `﹪`) after at
most one space of the line and any bidi marks, or a percent word after one
space ("percent", "per cent", "pour cent", "Prozent", "por ciento", "por
cento", "per cento"); or a multiplication sign glued after it (`x`, `X`,
`×`), which the scanner leaves out of a word when no letter or digit follows
it ("8.9x" is a figure and a sign, "1920x1080" and "0x1F" are words); or a
percent sign before it, as Turkish and Basque write one, standing after
nothing, whitespace or an opening bracket or quote ("%11", never a link's
"%20"). A letter or a digit right after a mark ends it ("50%off", "12
percentage points"). The reading against the mark is the ratio's figure: the
whole token, or of numbers a run sets side by side the one against the mark;
a group of a thousands run is none.

A reply's ratio is grounded by a corpus ratio, or by a decimal a script
printed, as is or a hundred times it ("0.1429" and "14.29" for "14.3%"),
that rounds to it at the places the reply wrote: within half a unit of its
last digit, both bounds included. A decimal is a token with one decimal
symbol between two groups; one a comma may group as thousands ("19,850")
grounds nothing, unless its lead is a zero, which no grouping writes
("0,850"). The comparison is exact, in scaled integers, and runs only where
it can succeed: a decimal whose integer part has a digit count no claim's
can round to, as is or a hundred times it, is passed over unread, and a
claim whose whole part is more than one away from the printed value's is
skipped before the exact comparison. The mark readers turn most tokens
away on one byte: a percent mark, a times sign, a space before a sign or
a bidi mark each start with one of a few bytes. A bare whole number
never grounds one, since it may be either operand of the division, and 0%
and 100% are never held. The person's words (`said`) ground a ratio too: the
numbers they stated, and for each pair the quotient and the change, each
also in percent, compared in floating point with the same tolerance, over
at most thirty numbers. The numbers are read once; the pairs are tried per
stated ratio and never stored. Sizes are compared, so a sign on either side does
not matter; the reply's sign is kept in what is reported, and one ratio is
named once however its mark is written, by its figure's text.

### Arithmetic in prose

`number::arithmetic` holds a reply to the values a message gave once it gave
four or more, counted where each stands ("25 + 25 + 25 + 25" gives four):

- a token counts once, and once more for each plain space inside it, since a
  list is written so as often as French thousands ("120 340 560 780");
- a number in words counts once;
- the pieces of a date, a time or a range ("2026-10-01", "12:30", "10-12")
  do not count.

A reply's reading is then held when it is a result: past ten, or a decimal,
or a negative other than zero. A count of ten or less, a year from 1900 to
2100 written without separators, a piece of a date or a time, and a group a
symbol joins to more digits of its run ("12" of "12.5kg") are not. A result's
key is exact: the same integer and the same fraction, trailing zeros aside,
with the sign held as `ungrounded_figures` holds it. Only a corpus reading
printed as a value grounds one: not a derived group of a thousands run, nor
the piece of a time, both of which the figure check accepts.

The comparison is the figure check's own walk (`ground::ungrounded_by`),
given what a reading states and which corpus readings ground.

### Per-unit amounts

`number::per_unit::mispaired_per_unit` splits the reply into clauses: lines,
and sentences as Unicode's sentence rules end them (a full stop, a question
or exclamation mark, closers and spaces after it, unless a comma, a colon, a
semicolon, a dash or another end mark follows, and for a full stop unless a
lower-case word, or a letter or digit glued to it, does), plus a break at
every full stop before a space, since a segmenter runs on across one before
a lower-case word ("$35 CPL. $984 lifetime"). A clause's figures are read
with its links, ratios and scaled figures left out, a token's thousands
reading taken over its decimal one; an amount is a figure behind a currency
mark, above zero, and a count a whole figure of two or more without one.

A clause with two amounts and a count holds its candidates: amounts of three
significant digits or more, with a larger amount beside them, that no amount
of the clause over one of its counts gives (the total cut to a whole last
place, the quotient rounded to half of the per-unit amount's). The
candidates and the questions they raise (is this amount, over this count, a
printed quotient; and for each larger amount of the clause taken as its
total, of what other printed amount) are set before the corpus is read, so
a reply with none never reads it. The corpus is then read line by line, its
digits as printed (no scale word), each figure met against every open
question as it is read, so nothing of the corpus is kept: a line answers a
question when it carries the count, a value the amount rounds, and a larger
printed amount (behind a currency mark, or a decimal) whose quotient over
the count is the amount, the first such being the one kept, and for a total
other than the total's own value. An amount a line shows as a quotient
itself is no clause's total. The values are compared in floating point, as
the host's TypeScript does, and the worked quotient is printed as English
prints an amount, rounding half up on the shortest decimal that reads back
as the value.

## Links

`url::scan::urls` finds http(s) links with the `linkify` crate. The
`<url|label>` wrapping chat apps use is blanked in place first, so a link's span is its
place in the text as given, and the markup and punctuation that close around
a link are dropped.

`fabricated_urls(reply, corpus, hosts)` grounds a link the corpus holds
verbatim, or without its scheme where the occurrence stands apart from what
precedes it (so "acme.io/org/repo" grounds the https link, and
"x@acme.io/org/repo" or "foo.acme.io/org/repo" does not), or whose path the
corpus holds without a host, as an API that gives a page's path without its
host does, and puts under no other host. `url::shapes` then takes the links
a reply may build rather than copy: a page naming no item on a host in
`hosts` (the hosts the caller trusts such pages on; a host root, one segment
without a digit, or one under `github.com/settings`), and three shapes built
from ids the corpus prints standing apart from other letters and digits:

- `<workspace>.slack.com/archives/<channel>/p<16 digits>`, the workspace
  host printed, the timestamp printed with its dot, a thread's `thread_ts`
  and `cid` the only query;
- `app.pipedrive.com/<deal|person|organization>/<id>`, or the same path on a
  printed `<company>.pipedrive.com` host, with no query;
- `facebook.com/ads/library/?view_all_page_id=<id>`, with the library's
  other filters in the query.

A shape's host passes unprinted at its root only, and no built link carries
a fragment.

`ungrounded_urls(reply, corpus, prompt, hosts)` adds four checks for a reply
about to be sent:

- a link on a reserved example domain that only the prompt carried, since a
  prompt's examples are not facts;
- a link target that is not a URL, such as `[PR 512](url)`;
- an item's label on a link that stands in for it: `[#87]` on the
  repository's root, or on another number's page (`/pull/418`, `/418/files`),
  read off the URL's last path segment (no digit, or all digits); a number
  on a chat message link is that message's own; a key (`CORE-1710`) on any
  link that does not hold it;
- an item named in prose without the link a source gave for it, as "#87
  (url)". The prose is the reply with its markdown links and its code
  blanked (`items::code_spans` reads fences and code spans as CommonMark
  does); an item is linked once any link of the reply holds its tail
  (`/pull/N` or `/issues/N` for `#N`, `/KEY-N` for a key) under any label.
  Its links come from the corpus and the prompt, one per item in the first
  tail's form (`/pull/N` before `/issues/N`), never an API's (`api.` host or
  `/api/` path), and an example-domain link only the prompt carried is no
  item's; two items give "#12 (url or url)".

`link_findings` is that check with the four kinds kept apart in a
`LinkFindings`, whose `all()` is `ungrounded_urls`'s list; a placeholder
target is kept as written there and parenthesised only in the list.

`mended_links(reply, corpus)` repairs what a rule can: each link of the reply
that `shapes::undotted_slack_link` reads as a message link in the first
shape above with its timestamp's dot kept (`/archives/<channel>/p<10 digits>.<6 digits>`, the query
and fragment as written) is replaced throughout, then the items the prose
names without a link (the same walk as the unlinked check, over the corpus
alone, since the prompt's examples are not links to write) are linked as
`[id](url)` where the prose first names them, when the corpus gives the id
exactly one item's link. The splices run from the last item back, so each
earlier offset holds. Every repair is listed as `from → to`.

`url_printed_in(url, read)` parses the address with the `url` crate, which
follows the WHATWG URL standard as browsers and Node do, so the host is
lowercased and IDNA-encoded, a default port dropped, the path's dot segments
resolved and its characters percent-encoded. It then looks for three plain
substrings of `read`: the address as given less a trailing `/`, the host
(less one leading `www.`, with a non-default port) followed by the path and
query, and the path and query alone when they are not empty. An address that
does not parse is looked for only as given.

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

A weekday followed by a casual word for a day ("Friday, tomorrow", "vendredi
demain", "пятница, завтра", "星期五今天"), or in Japanese a casual word
followed by its weekday ("今日(金)"), is checked against the day the word
names, today counted from `today`, as chrono's weekday refiner merges the
two (`date/casual.rs`). A part of the day names no day ("this morning",
"上午"), and "morgen" right after a weekday in German and Dutch is that
day's morning ("Freitag morgen"); after a comma it is tomorrow. The pair is
named with the time and the zone written after it ("TUESDAY tomorrow 5:30
PM").

Three readings depart from chrono's, each where chrono reads the date as no
reader would:

- a date written as three dotted numbers is day first in English too
  ("Sunday 5.1.2025" is January 5); a pair that is right month first is let
  be, and one wrong either way is named by its day-first date;
- a Japanese slash date whose first number cannot be a month is day first
  ("13/05(木)" is May 13), and no slash date is read from inside a number,
  where chrono's retry reads "3/05" out of "13/05";
- a two-digit number that ends a month-name date's clause is its year when
  chrono reads it as an hour past the clock and drops the date ("samedi 13
  juillet 24" is 13 July 2024); a word after it makes it a count, and the
  date goes unread as chrono leaves it. chrono reads the same number before
  23 as an hour, so "samedi 13 juillet 23" is checked against this year.

### A weekday beside a bare day number

A weekday a language reads alone, followed by one same-line run of spaces
and a day number (one or two digits, 1 to 31, with an ordinal suffix `st`,
`nd`, `rd`, `th`, `er`, `e`, `º` or `ª`), names no month: it is read against
the dates from 21 days before `today` to 35 days after it that carry that
number, and is wrong only when none of them falls on that weekday. The
window leans ahead, as a reply plans a month out more often than it looks
back. The finding names the nearest such date, the later of two as near.
The weekday word is one of the language's Intl names (its gate words),
found as a word segment (UAX #29); a name in a script without spaces is
found inside the text, as ICU's dictionary keeps "火曜日" one word.

The pair is skipped when:

- the weekday is a lowercase English short name ("sat 2+ weeks");
- the number is followed by `:`, `/`, `-`, `–`, `+`, `%` or `h` (a time, a
  range, a count or a slash date);
- the number is followed by a space and a word, unless "number word" does
  not read as an hour and the rest of the line opens on a time of day
  ("mardi 4 à 17h" and "Tue 4 at 5pm" are checked; "Monday 3 meetings", "Tue
  4 pm" and "Mer 15 oct" are not);
- English reads the pair as a month date ("Mar 11");
- the weekday sits inside a span some language read as a full date, which is
  checked as such ("Mer 15 oct" is French, never a bare Italian "Mer 15");
- the language's time parser reads the number as 24:00 ("Tue 24"): chrono
  merges a weekday with the hour after it and drops the pair when that hour
  is no time a clock shows. In English and Italian a signed length after the
  number ("Friday 24 +11mins") merges into that hour instead, which then
  names its own day, so the pair is checked.

A pair is reported once, as written, the full-date pairs included.

The time of day is `date/clock/`, a port of chrono's time parsers for the
fourteen languages, read as chrono's first result at the start of the
rest of the line:

- the time expression (an optional prefix such as "at", "à", "um", "a las",
  "alle", "às", "om", "klo", "в", the hour, minutes and seconds, a meridiem,
  the language's suffix such as "o'clock" or "Uhr", and a range's second
  time), French and German written hours ("17h30", "17 Uhr abends"), and
  the casual words that set an hour ("noon", "midnight", "now",
  "maintenant");
- Japanese, Chinese and Vietnamese times (`east.rs`): "15時30分", "午後3時",
  "下午3点半", "15:00", "lúc 7 giờ 30 phút sáng", with their numerals and
  a range's second time;
- a length from now (`relative.rs`): chrono's within, ago, later and casual
  relative parsers in every language that has them, with their number
  words ("in 2 hours", "for an hour", "il y a une heure", "vor 2 Stunden",
  "через час", "两小时内"). A length sets an hour when it holds hours,
  minutes or seconds, or a fraction of a day that rounds to some, as
  chrono's `addDuration` carries it; a length of days sets only a date;
- an ISO date with its time, and a date, a weekday or a casual date
  ("tomorrow", "demain", "明日") merged with the time after it, as the
  language's date-and-time refiner merges them ("tomorrow 15", "Nov 5 at
  3pm"), or a time merged with the date after it. A range word joins two
  results into a range first in the languages whose refiners do that
  first (German, Finnish, Chinese, Vietnamese); a range starts on a time
  only when the date it runs to falls on or after the day the time is
  read on;
- a zone after the result: an offset ("+02:00", "GMT+1") of at most 14
  hours, which Chinese does not read, and a zone name chrono knows ("CET",
  "UTC"), in capitals after a date with no time.

The result at the start is the longest one there, compared in UTF-16 units
as chrono compares them, unless a longer one starting inside it removes it,
as chrono's overlap removal does ("de 9h à 12h" reads "9h à 12h", which does
not start the text). A date with a day or a month of 0 is a result too, which
removes what it overlaps and is dropped with whatever merged into it, as
chrono drops it last. chrono's own checks follow: a lone digit, a three-digit
number and a text of digits and a dot are no time, and a time at 24:00 drops
the result.

A full-date pair is named with the time written beside it on its line ("Fri
26 Sep at 5pm", "at 5pm, Fri 26 Sep") and the zone after it, an offset after
a date with no time included ("WEDNESDAY 15 December +02:00"). A bare number
beside a date ("mardi 7 nov 15 personnes") is no time to a reader, so it is
left out of the name. In English and Italian a signed length after the date
or its time ("+11mins") is a length, not the zone "+11"; the other languages
read it as chrono does.

## Rows under a date window

`rows_outside_window(reply, today)` splits the reply into lines, with en and
em dashes read as hyphens since chrono reads "Oct 23–30" as its first day
alone. A line is a row when it is a table row (its lead is the first cell)
or a list item (`- `, `* `, `•`, `◦`, `‣`, `1. `, `1) `; its lead is what
follows), each without the emphasis around it. Each of chrono's fourteen
languages reads every line, ungated, as chrono's refiners read it:

1. every date parser's results, the casual words for a day ("today",
   "hier", "сегодня", "今天") and the lengths from now that fix a day ("3
   days ago", "in 2 weeks"; one of months or years only implies its day, so
   it fixes none), and the times of day around them (inside a date, glued
   to its start, and where the language's words join a date and a time),
   the longer of two overlapping results kept, compared in UTF-16 units. No
   time starts after the first dot of a date of three dotted numbers
   ("25.11.25 bis ..."), as chrono's scan reads its first numbers and
   resumes past them;
2. weekdays merged into their dates;
3. two times a range word joins merged into one range of times ("2/11 9:00
   - 17:00");
4. a date merged with the time beside it: a range of hours makes a one-day
   window ("Nov 4, 9:00-17:00"), or a two-day one when it ends before it
   starts ("Nov 4, 22:00-02:00");
5. two dates a range word joins merged into a range ("Oct 20 to today",
   "3 days ago - today"), Spanish and Portuguese joining dates with "a",
   "al", "hasta", "até" and "ao" as well, which chrono's refiners for them
   do not; with chrono's year rules: a year written on one end is implied on the other, and a range
   whose start falls after its end moves the end a year on, or the start a
   year back, where the year is unwritten. English and Italian first take
   in a zone name, drop a lone time in digits and a dot, and merge dates
   and times again ("10:00 UTC Sep 14"), as their refiners run in that
   order.

Every language merges a date with its time before it joins ranges, so "4.
Nov. - 15 Uhr" is one date and "10月2号15点30分-10月23日" runs to the 23rd,
as English reads them. A date with a day or a month of 0 is dropped with
whatever merged into it.

Any line that is not a row opens a window when it reads a range of dates
that runs forward. A blank line, a heading, a label ending in a colon or a
wholly emphasised line keeps the window open; other prose closes it, and so
does a line some other language reads as a window, so the rows under it are
never held to the window an earlier heading named. A month-name date that
ends a range takes no count after a comma as its year ("Aug 4 - Aug 5, 12
posts"), while one or two digits with another number later on the line
("Aug 5, 27 at 3pm") still read as one. A row
whose lead starts with a single date outside the window is reported once,
as `"<date>" under "<window>"`; a row whose lead is a range, of dates or of
hours, is left alone. A row and a window are named with the times and the
zone written beside their dates ("Oct 20 at 5pm", "5pm Oct 20", "2026-10-20
at 5pm CET"), a bare number beside a date ("Oct 20 - 12 posts") left out.
Arabic is not read.

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

`unprinted_ids(reply, corpus)` reads its own three shapes, each a whole run
of ASCII letters, digits, `_` and `-` (no outer `_` or `-` dropped):

- a typed id: two to eight lowercase letters, the first `_`, then ten or more
  letters and digits mixing both, in mixed case unless sixteen or more long,
  so a snake-case name (`df_sales2024q1`) is not one;
- a UUID;
- a hex digest of 8 to 64 digits mixing digits and letters.

The reply's links are blanked first, each distinct link everywhere it
occurs, longest first, so a run ends where a link starts and a run after a
link follows the blank. A run right after `@` or `#` is skipped as a chat
mention. An id is grounded by the corpus holding it, ASCII case aside; a
corpus with a Kelvin sign or a capital dotted I, which lowercase to `k` and
`i`, is also read lowercased.

## Durations

`durations(text)` reads an amount and a unit: digits, decimals with a point
or a comma, "1½", number words ("forty-five", "vingt et un",
"trentacinque"), glued runs of units ("1h30", "1h5m30s"), and fixed phrases
("half an hour", "un quart d'heure", "mezz'ora", "a fortnight"). Parts in
falling units join into one length ("1 hour and 30 minutes",
"5-minute-50-second", "2 jours, 4 heures"). A bare number and one dash
before a part in digits open a range ("10–30 min", "2 - 3 hours"), when the
number is below the part's amount; otherwise the part reads alone ("step 3 -
2 minutes"). The words are curated from rustling-ontology's grammar.

A second, a minute and an hour are also read under the names CLDR prints
them with, long and short, in the figure reader's locales (German, Dutch,
Polish, Swedish, Turkish, Indonesian, Vietnamese, Russian, Hebrew, Persian,
Hindi, Japanese, Korean, Chinese, Thai). A name that ends in ASCII joins the
part pattern; one that ends outside it ("минут", "時間", "giờ") has a
pattern of its own, run only over a text that is not all ASCII, since the
part pattern's ASCII word boundary cannot end it. Such a name ends before
no letter, mark or digit, except that a name in a script written without
spaces ends before an uncased letter ("3時間かかった"). An abbreviation CLDR
closes with a dot ("Std.", "godz.", "min.") takes the dot, so a part after
it still joins ("2 Std. 30 min").

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
hundredths of a month, since a month and a year have no fixed length. Each
duration also keeps its place, the step it is written to: its smallest unit
("6 minutes" to the minute), the decimal place its digits stop at ("1.5
hours" to the tenth of an hour), or the fraction added to it ("1½ hours",
"an hour and a half" to the half hour; "half an hour" to the half hour, "a
fortnight" to the week).

`ungrounded_durations` grounds a reply's duration with a printed length that
rounds to it at its place: within half a step ("6 minutes" takes 5:31 to
6:29, so 350 seconds grounds it and 90 seconds grounds neither "1 minute"
nor "2 minutes"), or within a whole step when an approximation mark (`~`,
`≈`, `∼`) stands before it. A range is grounded by a printed length, or a
printed range, that lies inside it widened by as much; a single length by a
printed range only when both bounds round to it. Only the component of the
step rounds: the other one, clock or calendar, must match. The corpus
prints a length in any of these forms:

- written out, or as one part of a longer duration, a range as its two
  bounds (its upper part is not also read alone), and a comma before three
  digits as a decimal comma too ("1,250 secondes" is 1.25 seconds as well as
  1,250);
- as ISO 8601 (`PT1H30M`);
- as a clock reading (`1:30:00`; `12:30` reads both as hours and minutes and
  as minutes and seconds);
- as a number under a field whose name ends in a unit's word or symbol
  (`"duration_ms": 5400000`, `"elapsedHours": 1.5`; a bare `duration` reads
  as seconds or minutes).

A clock reading right after a date (`2026-10-09 14:30`) is a time of day,
not a length. Rates ("once a day"), dimensions ("3D") and a bare `m`, as
often a metre or a million, are not durations. A number glued to a
one-letter symbol ("30s", "24h", "3d") may be a plural ("the 90s"), a time
of day ("15h") or a size, so it grounds a reply's duration but is never
reported itself; spaced from it ("24 h") it is a unit symbol and is held.
`Duration::stated` says which readings a reply is held to. A duration that
starts inside one of the reply's links is never reported either.

## The Node binding

`figures-parser-napi` exposes each comparison and an `extract(text, {kinds})`
reader. Every export runs under `#[napi(catch_unwind)]`, so a panic reaches
JavaScript as a thrown `Error` instead of aborting Node. Offsets are
converted to UTF-16 in one forward pass with `utf16::Utf16Cursor`.

The npm package follows napi-rs's layout. `index.js`, generated by
`napi build`, loads `figures-parser.<platform>.node` from beside it when one
is there, as in a checkout, and otherwise the platform package npm installed
for this OS, CPU and C library (`@mio-xyz/figures-parser-linux-x64-musl`, for
one), each an optional dependency of the root package. It tells musl from
glibc by `/usr/bin/ldd`, else by Node's process report. The glibc builds link against glibc 2.17
through napi-rs's cross toolchains, and the musl builds through zig, so one
build serves every distribution of its kind.
