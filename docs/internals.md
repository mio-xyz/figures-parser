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
| `number` | `scan` (tokens), `readings`, `scale` (scale words), `spelled` (numbers in words), `digits`, `context` (the unit words around a figure), `printed` (the figures a clause states and a line prints) | `ground` (figures, and amounts against one print), `ratio`, `arithmetic`, `per_unit` |
| `url` | `scan`, `items` (link targets, the items named beside links), `opened` (pages to open) | `ground`, `shapes` (links built from printed ids), `printed` (an address a text carried) |
| `date` | `languages`, `names` (a port of chrono-node, and Arabic), `clock` (its times of day, `east` for Japanese, Chinese and Vietnamese, `relative` for lengths from now) | `calendar`, `bare` (a weekday beside a bare day), `casual` (a weekday beside a casual word for a day), `window` (rows under a date window) |
| `email` | `emails` | `ungrounded_emails` |
| `id` | `identifiers` | `ungrounded_identifiers`, `unseen_tokens` |
| `duration` | `grammar`, `words`, `groups` | `corpus` (the forms a corpus writes a length in) |
| `internals` | word segments | `internal_names`, `scrub_memory_layout` |
| `sheet` | | `sheet_findings` |
| `memory` | `grounding_tokens` | `memory_grounding` |
| `page` | | `ungrounded_on_page`, `PageHolds` |
| `reply` | | `speaker_prefix`, `memory_claim`, `silent_signal` |
| `message` | | `ground_reply`, `ground_message`, `ground_page` |

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

- a currency sign (`\p{Sc}`) or an ISO 4217 code before it, with at most one
  space between, or after it, with at most one space of the same line. A
  mark before another figure is that figure's ("12 $750"), and a code set
  apart between two figures marks neither ("2,685 USD 2026");
- a sign before it, read by a fast one-byte check, then a full look-back
  that crosses a currency mark and the bidi marks Arabic and Persian print
  ("-€1,234.50", "€ -1.234,50", "EUR-1'234.50"). A dash after a digit or a
  word letter joins instead ("CORE-1710", "10-12");
- an approximation mark (`~`, `∼`, `≈`) before the figure, its sign or its
  currency, with at most one space of the same line between. A tilde
  between two figures ("10~20") marks the second too, read as a range or as
  "about"; the CJK wave dashes (`〜`, `～`) mean a range or "up to" and mark
  nothing;
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
yields both, the decimal marked `ambiguous`: it grounds, and a reply states
it only before a scale word whose language writes that decimal mark (see
[Scale words](#scale-words)), since "$206,788" never states $206.788. No
grouping writes a zero lead, so "0,850" is a decimal only.

### What a reply is held to

`Reading::stated` decides which readings a reply states, by floors:

- a bare figure under 10,000 passes, being usually a count or the model's own
  arithmetic;
- a thousands separator lowers the floor to 1,000, and a currency mark to
  100;
- a compact scale after the digits (a thousand or more) holds the figure
  whatever its value, with or without a currency, so a count written "3.2M
  teachers", "4K" or "0.95k" is held as "$8k" is; "hundred" after digits is
  a number in words, held from 1,000 up;
- a decimal with four fractional digits or more is always held, and is
  grounded by any corpus value that starts with the same digits ("1.1785" is
  carried by "1.17853");
- a derived reading, the decimal reading of a token that also reads as
  thousands, or a number in words opened by an article, is never held.

A reply's figure with several whole readings is one claim, grounded by any
of them and reported once.

### Signs

A reply's "-12,500" is not grounded by a printed "+12,500", nor its
"+12,500" by a printed "-12,500". An unsigned value grounds either, since a
source puts the sign in words ("fell by 12,500", "fee: 2,000") as often as in
digits, and an unsigned reply figure takes either sign. Accounting's
parentheses, "($1,234.50)", ground a negative in the corpus, but hold a reply
to no sign, since prose writes an aside the same way.

The corpus walk reads no sign up front. A corpus token's sign is read only
when one of its readings matches a signed reply figure's value, so a signed
reply costs no more than an unsigned one.

### Approximation

A figure written in words is grounded by any corpus value that rounds to it
at the precision it is written with: its last nonzero digit for a whole
number, its last digit for a decimal. A scaled figure rounds at the last
digit its digits are written to, which the zeros the scale adds do not show:
"1.2 million" takes 1,150,000 to 1,249,999, "34,0M" 33,950,000 to
34,049,999 and "10 million" 9,500,000 to 10,499,999. A power of ten with one
significant digit takes from where a value rounds up to it, so "1 million"
takes 950,000 to 1,499,999.

A figure behind an approximation mark takes a whole unit of that digit on
either side, both ends included, since a model writes "~3,440" for a 3,449 it
cut as often as for one it rounded: "~12,500" takes 12,400 to 12,600,
"~12,000" takes 11,000 to 13,000, "~3M" takes 2M to 4M and "~1.1785" takes
1.1784 to 1.1786. The mark is read off the reading's full text, which opens
with it.

The range is added to the exact key, so a mark never grounds less than the
bare figure, and an approximate figure that is not scaled rounds a printed
value at two significant digits as an exact one does ("~$1,000.7" is
grounded by 1,001.7). An approximate figure is reported as written, mark
included.

### Scale words

A scale word after the digits multiplies the figure: "1.2 million",
"$4.5bn", "12.5K", "2,5 Mio.", "1,5 Mrd.", "3 milliards", "1,2 mil millones",
"5 lakh", "₹2 Cr", "1.2万", "3億", "5千万". The long forms are read case aside
in English, French, German, Spanish, Italian and Portuguese; the CLDR
abbreviations are read with their case. A scaled figure is stated as rounded,
so "$1.2 million" is grounded by 1,150,000 to 1,249,999.

The abbreviations are those of the CLDR compact formats of the locales the
host reads, and a few more ("Tsd", "Mds", "MM", "crores"): "jt", "rb", "tr",
"nt", "lcr", "tri", "bi", "Md", "Mn", "mln", "bn", "bln" and their kin read
wherever they stand, a multi-letter one case aside, and so does "Mio". An
abbreviation that also names a unit or is also a word reads as a scale
unless the words around the figure say it is the other thing
(`number::context`): "3.2m users" and "5 mn people" scale, while "took 5m",
"5m ago", "1h 5m" and "en 5 mn" are minutes, "5m tall", "3m x 4m" and "ran 5
mi" lengths, "a 2481t truck" and "weighs 25 tn" tonnes, "300l of water"
litres, "5 md files" a file type, "earned 92 cr this term" credits, "5 Mr
Smith" a title, "5 bin bags" a bin and "3 Bio eggs" organic. A currency
beside the figure settles it as a scale, written as a sign or code ("$5m"),
a local symbol ("Rs 92 cr", "5 bin TL") or a currency's name ("3.2m
dollars", "100,000m d'euros"). "mm" and "CR" in capitals (a credit after a
balance) read only beside a currency. A lone letter after a space ("500 m",
"1750 N", "5 M users") reads only beside a currency, as the capitals "M",
"K", "B" and "T", and "k" and "b", read glued to the digits ("3.2M", "7B",
"2.8T"). A long form reads glued to the digits too ("12million").

The long forms of Dutch, Polish, Swedish, Turkish, Indonesian and Vietnamese
("3 miljoen", "4 miliony", "5 milyar", "7 juta", "3 tỷ") are read with the
others, and the scale words of other scripts after the digits, glued or after
a space: Russian ("3 млн", "2 миллиарда"), Arabic, Persian, Hebrew, Hindi
("2 लाख") and Thai, which glues them to the next word as well ("5ล้านคน"). A
word after a thousand or a lakh can multiply it: "mil millones", "mila
miliardi", "mille miliardi", "lakh crore", "nghìn tỷ". A Han or Hangul
multiplier reads after a space too ("200 万"), since it is never a unit.

A name some locale gives another power ("billion" is 10¹² in French and
German, "bn" in French and Swedish, "b" a thousand in Turkish, "tn" a
thousand in Swedish, "m" a billion in Indonesian, "t" a billion in
Vietnamese) reads at each where the figure is written as those locales
write it: a currency mark after the scale ("2 bn €", "1 billion EUR") or
the "of" of French, Spanish, Italian or Portuguese ("1 billion d'euros").
Elsewhere it reads at the power English gives it, as English puts the
currency first ("$2 billion" is 10⁹, "2tn" 10¹²). A decimal read through
the scale word's own mark takes only the word's own power: "9.007 tn €" is
English, so it is 9.007 × 10¹² and never the Swedish 9.007 thousand.

A run that also groups ("1,250", "8.566") is a decimal before a scale word
too where the word's language writes that decimal mark: a comma before a
Latin word other than English ("8,566 Mrd.", "12,500 rb") or a Cyrillic one
("8,566 млрд"); a point before an English word ("1.250 million") or a word
of another script. The thousands reading is kept beside it, so the corpus
grounds either. Elsewhere the run is thousands: "£1,250 million" is 1,250
million, and so is a letter every locale writes alike, a currency after it
included ("1,250 M€", "46,886b €"), since an English report writes "1,250
M€" for 1.25 billion as a French one writes it for 1.25 million.

A corpus reads scale words loosely (`Tokens::loose_scales`): an abbreviation
in any case and without the currency or the dot a reply needs ("38.08 M",
"2,5 Mio", "took 5 m", "12 k rows"), and a name glued to the digits even when
it runs into the next word, as a scraped page prints "43Kmembers" or
"1.5bnviews", read as its longest name. Such a token yields its digits' own
readings beside the scaled ones, since the letters may be a unit after all,
and every power a locale gives a name; a reading taken more widely only ever
grounds more. A spelled number that starts inside a token's scale words
("mil millones" of "78.78 mil millones") is that token's, and is not read
again.

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
vector, against each print (`shared_with`), ratios skipped as well. An amount
is matched by its exact digits, whatever its sign or approximation mark (a
refund printed unsigned is where a reply's "-1,040" came from), named as the
reply writes its digits, and once however often the reply writes it. The keys
stay apart from the claims for that: `Needles` holds what a walk reads,
`Claims` what it closes. `Amounts` also reads a scaled figure's digits as
printed (`Tokens::keep_digits`), since a print that carries "9.162" vouches
for the reply's "9.162 jt" as much as one that carries 9,162,000,000.
`states_figure` reads a text's scaled figures both ways too.

Once the walk leaves a claim open, the reply's round ranges are read: two
tokens with nothing but a dash between them (a currency mark and spaces
aside), each a whole number of at most two significant digits, low to high,
the second written without a sign of its own. One corpus pass then marks each range a printed value falls inside, and the
claims stated as exactly its bounds close.

A claim still open after that, stated as a whole number of at most two
significant digits ("$3,200", "45,000"), is then read at that precision: one
more corpus pass rounds each printed value to the claim's unit (100 for a
four-digit figure, halves up) and closes the claim the result equals, the
claim's sign still held; a scaled claim keeps the place it is written to
instead. A figure of one significant digit is read at two,
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
"%20"), unless a currency mark stands between ("% $45" is an amount) or the
sign is set apart from the figure after it and stands one space after a
figure, which it marks ("84 % 8,322"). A digit, or two hex digits of a
link's escape, right after a sign ends it ("%25", "%2C"), and a letter after
a percent word ("12 percentage points"); a word glued to a sign still
follows a percentage ("50%off"). A sign on the next line marks nothing. The
reading against the mark is the ratio's figure: the
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
never grounds one, since it may be either operand of the division. The
person's words (`said`) ground a ratio too: the
numbers they stated, and for each pair the quotient and the change, each
also in percent, compared in floating point with the same tolerance, over
at most thirty numbers. The numbers are read once; the pairs are tried per
stated ratio and never stored. Sizes are compared, so a sign on either side does
not matter; the reply's sign is kept in what is reported, and one ratio is
named once however its mark is written, by its figure's text, a sign against
its figure and a word after a space ("12 percent"). `ungrounded_ratios_with`
takes the person's words as a function, called only when a claim is left
once the corpus is read.

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
a reply with none never reads it; `mispaired_per_unit_with` takes the
corpus as a function, called only then. The corpus is then read line by line, its
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

### Readers for the host's other checks

The host's other checks read figures the way these comparisons do, and the
library gives them those readings. `stated_figures` reads a reply's clause
as the per-unit check does: its links, ratios and scaled figures left out, a
run that groups read as thousands, each figure with its value, its places
and the currency mark before it. `printed_figures` reads a printed line as
the per-unit check reads the corpus: every reading of every figure, a
letter allowed after one, and apart the amounts (behind a currency mark, or
with a decimal part). `numeric_tokens` gives each token of a result with its
span and values, the decimal reading of a run that also groups only when
read loosely.

`sheet_findings` walks a turn's script runs for the worksheet checks. The
caller says what each run did (it listed its rows, took a ledger frame, read
a grid by hand) and what rows it listed; a run sharing no amount with the
reply (`Amounts`) holds it to nothing. An amount shared with a run that
lists its rows is checked; one shared with a run that took a ledger frame,
when some run read a grid, or with two amounts or more a run read off a grid
itself, is unchecked. The rows listed by a run the reply draws on are owed,
and those whose amount the reply does not state (`states_figure`) are
returned apart.

## Links

`url::scan::urls` reads http(s) links the way linkify-it, the host's former
reader, reads a link with a scheme. The `<url|label>` wrapping chat apps use
is blanked in place first, whatever the case of its scheme, so a link's span
is its place in the text as given. A character reads as linkify-it classes
it: a space or control, Unicode punctuation, a separator (`<`, `>`, `｜`), or
anything else, which counts as a letter. After `http:` or `https:` and `//`:

- an optional user name and `@` (ASCII letters, digits and the marks a user
  name takes unescaped, opening and closing on a letter or digit);
- a host: a bracketed IPv6 address, or one to eleven dot-separated labels,
  each `xn--` and up to 59 letters, digits and dashes, or one to 63
  characters that neither open nor close on a dash; then an optional port up
  to 65535; then the host has to end on the text's end, a space, a
  separator or punctuation other than `-`, `_`, a port's `:` or a dot before
  more of a host, else there is no link;
- a path after `/`, `?` or `#`: brackets, braces and parentheses nested up
  to four deep and balanced, quotes holding up to a hundred characters
  without a space, `.`, `,`, `;`, `!` and `?` only before more of the link,
  dashes, the marks `\/:%@#&=_~*`, and any other letter; a bracket that
  closes nothing ends it.

A link is read wherever no ASCII letter or digit is glued before it, where
linkify-it opens one only after a space, punctuation other than `_`, a
control character or a separator; so a link in code, in emphasis or after a
script written without spaces is read. A backtick, a `|` or `\|` that opens
a chat link's `label>` on the same line, and a `[label](` that opens a
markdown link end a link, where linkify-it reads on. The `>` must close a
label rather than be prose: not after a space, `-` or `=` (`filter > 5`,
`->`, `=>`), not before `=` or `>`, and with no `)` or `]` in the label that
it did not open (`(draft|v2)>`). The scan follows where
linkify-it would look next (past the last scheme it tried, or the last
character of the link it read, read without those cuts), so a link read where
linkify-it reads none, or read with a user name, ends before any link
linkify-it reads on its own, and an `ftp:` link or a `//host` link without a
scheme takes in what linkify-it lets it take. The markup and punctuation that
close around a link (`*`, `_`, a backtick, `~`, `.`, `,`, `;` and `:`) are
then dropped.

`placeholder_targets(text)` lists each markdown link target that is no URL,
mail address or anchor, and `unopened_links(text, opened)` the links of a
message, once each, that are on no host `slack.com` or under it (the host
parsed as a browser parses it) and not among `opened`.

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
a fragment. A link's path is read as a browser resolves it: `.` and `..`
segments (plain or escaped) read and `\` taken as `/`, so `facebook.com/.`
is the root. A segment's characters are compared as written, where Node
percent-encodes them, so a page name outside ASCII holds no digit. A
query's keys and values are decoded as a browser decodes them (`%31` is
`1`, `+` a space) before they are looked for. Only an http(s) link is taken,
its slashes after the scheme read as a browser reads them (none, one, three
or `\`, so `https:/acme.io` is `https://acme.io`).
`built_from_corpus(url, corpus, hosts)` is this test alone.

`ungrounded_urls(reply, corpus, prompt, hosts)` adds four checks for a reply
about to be sent:

- a link on a reserved example domain that only the prompt carried, since a
  prompt's examples are not facts;
- a link target that is not a URL, such as `[PR 512](url)`;
- an item's label on a link that stands in for it: `[#87]` on the
  repository's root, or on another number's page (`/pull/418`, `/418/files`),
  read off the URL's last path segment (no digit, or all digits), or on a
  link that does not parse (a port past 65535, a host a browser rejects); a number
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
that `shapes::undotted_message_link` reads as a message link in the first
shape above with its timestamp's dot kept (`/archives/<channel>/p<10 digits>.<6 digits>`, the query
and fragment as written) is replaced throughout, then the items the prose
names without a link (the same walk as the unlinked check, over the corpus
alone, since the prompt's examples are not links to write) are linked as
`[id](url)` where the prose first names them, when the corpus gives the id
exactly one item's link. The splices run from the last item back, so each
earlier offset holds. Every repair is listed as `from → to`.

`url_printed_in(url, read)` parses the address with the `url` crate, which
follows the WHATWG URL standard as browsers and Node do, so the host is
lowercased and IDNA-encoded and a default port dropped. The path is parsed
again by the standard's path rules, since the crate lags it: a `file:` path
keeps its empty segments (`file:////x` is `//x`), `^` is escaped, and a
drive letter is read only in a `file:` path. An address whose scheme has no
special meaning and whose port is no number does not parse, as in Node. It
then looks for three plain substrings of `read`: the address as given less a
trailing `/`, the host (less one leading `www.`, with a non-default port)
followed by the path and query, and the path and query alone when they are
not empty. An address that does not parse is looked for only as given.

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

- a typed id: a word of two to eight lowercase letters, then up to two
  words that name a key's mode (`test`, `live`, `sandbox`), each closed by
  `_` (`prod_`, `cs_test_`), then ten or more letters and digits
  mixing both, in mixed case unless sixteen or more long, so a snake-case
  name (`df_sales2024q1`) is not one, nor one of several words
  (`utm_campaign_BlackFriday2024`, `user_id_v2Final2024`);
- a UUID;
- a hex digest of 8 to 64 digits mixing digits and letters.

The reply's links are blanked first, each distinct link everywhere it
occurs, longest first, so a run ends where a link starts and a run after a
link follows the blank. A run right after `@` or `#` is skipped as a chat
mention. An id is grounded by the corpus holding it, ASCII case aside; a
corpus with a Kelvin sign or a capital dotted I, which lowercase to `k` and
`i`, is also read lowercased.

## Memory

`memory_grounding(response, dossiers)` counts the dossier lines a reply
draws on. A line is each non-empty line of a dossier, trimmed as JavaScript
trims, that is no `#` heading. `grounding_tokens` reads a text's words: its
`[derived …]` and `[as of …]` markers and its `*`, `_`, backtick, `~`, `>`
and `#` marks blanked, the
rest lowercased and cut at every character that is no letter, number or
`:`, a `:` at either end of a token dropped, and tokens under three UTF-16
units or among the stopwords dropped. A line of two or more distinct tokens
is drawn on when the reply shares two or more of them and at least half, or
repeats three of the line's tokens in a row.

## A reply's form

Whitespace here is JavaScript's `\s`: Unicode's `White_Space` without
U+0085, with U+FEFF.

- `speaker_prefix` reads, after leading whitespace, a `[`, a name of 1 to 80
  UTF-16 units holding no `]` and no `\n`, then `]:` and one whitespace
  character. Only the reply's start is read.
- `memory_claim` reads, from a word boundary, "I'll remember" or "I will
  remember", "I've" or "I have" then "noted" or "remembered", "committed
  this/that/it to memory", or "noted/saved/added/stored/updated this/that/it
  to/in" before an optional "my/your/the" and "memory", "preference(s)",
  "note(s)", "dossier" or "profile". Case is folded in ASCII only, and the
  apostrophe is straight or curly.
- `silent_signal` reads past a `[CURRENT MESSAGE]` framing echoed at the
  start (whitespace before it, one space after). The whole text, trimmed, is
  the marker when it is `silent:`, optional whitespace and one run without
  whitespace; otherwise the first line that opens with such a marker, its
  run followed by whitespace up to a line end (`\n`, `\r`, U+2028, U+2029)
  or the end, is the marker leaked.

## Durations

`durations(text)` reads an amount and a unit: a printed number, "1½", "½",
a fraction "1/2", "1/4" or "3/4" after an optional whole number ("1 1/2
hours"), number words ("forty-five", "vingt et un", "trentacinque"), glued runs of units
("1h30", "1h5m30s"), and fixed phrases ("half an hour", "un quart d'heure",
"mezz'ora", "a fortnight"). A printed number is digits of any script in
groups joined by the symbols a locale prints ("1,250", "1.234,5", "1'250",
"1 250", "12,50,000", "٣٫٥", "३"), read as the figure reader reads one: a
lone dot or comma before three digits reads both ways ("1.250 Sekunden" is
1,250 seconds or 1.25), and groups that fit no one number are none. A part
does not start inside a word or right after a number's symbol or a fraction
slash ("x7,59 min", the "2 hours" of "1/2 hours"), and a letter of a script written without
spaces is no word ("合計5分钟", "30 minutes後").

Parts in falling units join into one length when a space, a hyphen, a comma
or "and" alone stands between them, within a line ("1 hour and 30
minutes", "5-minute-50-second", "2 hours - 30 minutes", "2 jours, 4
heures"); a full stop ends the length ("2 hours. 30 minutes" is two), unless
it closes an abbreviation ("1 hr. 15 min." is one). A bare
number and one dash within the line before a part in digits open a range
("10–30 min", "2 - 3 hours"); written high to low ("30-10 minutes", "step 3
- 2 minutes") it is read as the span between the two numbers, and two equal
numbers ("18-18 seconds") as the part alone. A dash after a line break opens
a list item, not a range. The words are curated from rustling-ontology's
grammar.

A second, a minute and an hour are also read under the names CLDR prints
them with, long and short, in the figure reader's locales (German, Dutch,
Polish, Swedish, Turkish, Indonesian, Vietnamese, Russian, Hebrew, Persian,
Hindi, Japanese, Korean, Chinese, Thai) and in Finnish and Ukrainian, with
the Finnish genitive ("3 tunnin kokous") and the Russian and Ukrainian
accusative ("за 1 хвилину"). A name that ends in ASCII joins the
part pattern; one that ends outside it ("минут", "時間", "giờ") has a
pattern of its own, run only over a text that is not all ASCII, since the
part pattern's ASCII word boundary cannot end it. A name in a cased script
ends before no letter, mark or digit; a name in a script written without
spaces ends before an uncased letter ("3時間かかった"); a name in another
uncased script may go on in letters and marks of its own, an inflection or
short vowels ("۳ ساعته", "5 मिनटों", "5 دقائقَ"), but not into a cased letter
or a digit ("วินาทีly"). An abbreviation, CLDR's dotted ones ("Std.",
"godz.", "min.") and the short forms "hr", "hrs", "mins", "secs", "tim" and
their like, takes the dot after it, so a part after it still joins ("2 Std.
30 min", "1 hr. 15 min.").

Arabic has a pattern of its own, run only over a text that holds Arabic
letters (one `memchr` scan decides):

- an amount in digits of any script, a space or a hyphen after it, or a
  number word up to ninety-nine ("خمس", "خمس عشرة", "خمس وأربعون");
- a unit in its singular, plural or dual form; a dual counts two by itself
  ("ساعتين") and a number before it as a plural does ("2 ساعتين"), while a lone singular counts one only with "واحد" after it,
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
("6 minutes" to the minute), the decimal place its digits stop at ("1.2
hours" to the tenth of an hour), or the fraction added to it ("1½ hours",
"1.5 hours", "an hour and a half" to the half hour; "2.25 hours" to the
quarter; "half an hour" to the half hour, "a fortnight" to the week), since
"1.5" is "1½" spelled in digits.

`ungrounded_durations` grounds a reply's duration with a printed length that
rounds to it at its place: within half a step ("6 minutes" takes 5:31 to
6:29, so 350 seconds grounds it and 90 seconds grounds neither "1 minute"
nor "2 minutes"), or within a whole step when an approximation mark (`~`,
`≈`, `∼`) stands before it. A range is grounded by a printed length, or a
printed range, that lies inside it widened by as much; a single length by a
printed range only when both bounds round to it. Only the component of the
step rounds: the other one, clock or calendar, must match. The corpus
prints a length in any of these forms:

- written out, a range as its two bounds (its upper part is not also read
  alone), a lone dot or comma before three digits both ways ("1,250
  secondes" is 1.25 seconds as well as 1,250); a part joined to another is
  one length with it ("1 hour 30 minutes", "25 minutes, 59 seconds" and "1
  hour and 30 minutes" do not print "30 minutes", "25 minutes" or "1 hour");
- as ISO 8601 (`PT1H30M`);
- as a clock reading labelled a length (`"duration": "1:30:00"`, "took
  1:30:00", "elapsed 3:00:00"), or one whose first field is past a day's
  hours ("75:30"); `12:30` reads both as hours and minutes and as minutes
  and seconds. A reading with no such label ("at 10:05", "[10:05:12] INFO",
  "starts 14:00:00") is as often a time of day;
- as a number under a field whose name ends in a unit's word or symbol
  (`"duration_ms": 5400000`, `"elapsedHours": 1.5`; a bare `duration` reads
  as seconds or minutes).

A clock reading right after a date (`2026-10-09 14:30`) is a time of day,
not a length, and so is an hour written with its symbol after a clock word
("à 3 h", "um 15h"), before a part of the day ("3 h du matin") or joined to
another such hour ("de 14h à 15h30"). Rates ("once a day"), dimensions ("3D") and a bare `m`, as
often a metre or a million, are not durations, nor is a spaced "y", the
Spanish "and" ("entre 1 y 2 horas"). A number before a one-letter symbol
("30s", "24h", "3d", "24 h") may be a plural ("the 90s"), a time of day
("15h", "à 15 h", and "15h30", the hours and minutes French and German
write a time of day with) or a size, so it grounds a reply's duration but is
never reported itself. A length in days or longer units alone ("30 days",
"2 weeks", "1 year") is not reported either, since a window ("the last 30
days") and a plan ("in 2 weeks") are written the same way; joined to a
shorter part ("2 days and 3 hours") it is. A reply is held to the first
reading of its digits, never the loose one: "1,250 seconds" is not grounded
by 1.25 seconds. `Duration::stated` says which readings a reply is held to.
A part inside one of the reply's links is the link's: it is never reported
and joins no part after it, and a range whose low is inside a link is the
part after the link alone.

## Pages

`ungrounded_on_page(text, corpus, said)` is the figure, ratio and opaque-id
checks run over a page's text, their findings in that order: a page made
for a person is held to what was read, its links, dates and wording left to
its design. `PageHolds` keeps, per file name, the claims a run was told
about and how many times the file was held back; `hold` holds a page back
when one of its claims is new for that file and the file was held back
fewer than `HOLDS_PER_PAGE` (three) times, and records the claims.

## An agent's own internals

`internal_names` reads a reply for the names of the machinery an agent runs
on, from a `Vocabulary` the caller gives: its tool and sandbox function
names, its memory file names, the directories its code and memory live in
(roots) and other fixed names (programs, environment variables). A name the
people in the thread wrote passes, read by the same rules as the reply.

- A code name counts as a word of the reply: a word segment, lowercased,
  equal to the name. A name with an underscore counts wherever it is a
  word; one without ("learn") only as a call, the word right before a
  segment that opens on `(`, since such a name is also a verb.
- Segments follow the Unicode word rules, which keep `web_search` and
  `workspace.md` whole and part `webSearch` from nothing, so another
  program's camelCase or a repository named `web-search` is not read. Two
  departures: Hangul is parted from letters of another script, since Korean
  writes a particle onto a name ("workspace.md에"), and Thai, Lao, Khmer and
  Myanmar, which the rules keep apart from Latin, stay apart, so a name
  written against Thai text is still a word.
- The rules part the letters of those four scripts, and of the Tai scripts,
  one from the next, so a name holding one ("ไทย_tool") is looked for as
  written instead, lowercased: it counts where no digit, underscore or letter
  of a spaced script stands right before it (a combining mark there belongs
  to the letter before) and none, nor a mark, right after it.
- A memory file counts as a word too, and passes when the vouching text
  (the tool results other than the memory tools', which list the files
  themselves) holds it whole, lowercased: no file-name character (a-z,
  digits, `-`, `_`, `.`) on either side, so `old-workspace.md` vouches for
  nothing.
- A root counts only where it begins a path: one after a path character
  (a-z, digits, `-._~%/:`) is inside someone else's link. A fixed name
  counts anywhere, case aside. Neither is vouched for by a tool result,
  since code a run executes can print either.

`InternalsCheck::new` reads the person's words once, for every reply of a
turn; `names_in` reads the vouching text only when the reply names a file.

`scrub_memory_layout` puts the memory layout of a text in plain words, from a
`MemoryLayout`: a path that starts with the root (or is the root without its
slash) and runs on through path characters, less a trailing sentence mark,
becomes the phrase of its folder, then of the file it ends in, then the
root's or the other paths' phrase; a memory file named alone, whole, not
after `/`, becomes its file's phrase; a name that starts with the tool
prefix, not after an identifier character, becomes the tool phrase. A path
inside a longer one and anything the person wrote is left as written.
Matching is case aside, character by character, so every offset stays one of
the text's: a character that lowercases to two (`İ`) matches nothing, and the
Kelvin sign matches `k`. A text that shows no layout is returned borrowed.

## Amounts taken from a worksheet

`sheet_findings` reads the turn's script runs, each with what it printed and
three facts the caller knows about it: whether it called a helper that lists
the rows its figures rest on, whether it took a sheet as a frame of rows, and
whether it read a worksheet's cells itself, with the rows a helper listed for
the reply. A run counts only when the reply states one of its amounts
(`Amounts`); its listed rows are then owed, and a listed row whose amount the
reply does not state (`states_figure`) is unstated. A run that lists its rows
checks every amount it shares with the reply. An amount is unchecked when a
run printed it after taking a frame (when any run of the turn read cells,
since a frame of a price series is no ledger), or when a run that read the
cells itself printed it with another of the reply's amounts, one alone being
possibly a single cell looked up; a checking run anywhere in the turn
releases it.

## Memory a reply draws on

`grounding_tokens` drops freshness markers (`[derived …]`, `[as of …]`,
ASCII case aside) and markdown marks, lowercases, splits on anything but a
letter, a number or `:`, trims colons at either end, and drops stopwords and
tokens under three UTF-16 units. `memory_grounding` reads each note line by
line (trimmed, not empty, not a `#` heading) and counts a line the response
draws on: at least two of its distinct tokens shared and at least half of
them, or three of its tokens in a row repeated.

## A turn's invariants

`evaluate_invariants` gives each invariant a turn is held to a verdict:
pass, fail with its reason, or not evaluable when the input it reads (the
raw text, the tool results, the step count) was not given.

- `claims_unbacked_action`: an English past-tense action claim ("I've
  created", "sent the email", "done! I") with no pending action and no
  write tool called: an integration's create, update, comment, assign,
  merge, send, draft, reply or delete tool, or the message post.
- `claims_unwritten_memory`: a claim `memory_claim` reads ("I'll
  remember", "saved it to my notes", the apostrophe straight or curly) with
  no `memory_write`, `memory_edit` or `memory_delete` call.
- the speaker label: `speaker_prefix` finds one.
- the abstention marker: `silent_signal` reads it, and the invariant fails
  when it leaked.
- `fabricated_urls` for the links, with no host trusted.

The phrases compare letters in ASCII case and find word edges by ASCII word
characters, and whitespace is what JavaScript's `\s` reads, so a verdict is
the same as the host's patterns give. `claims_refusal` reads the refusal
phrases a turn's score counts.

## A text with its links taken out

`without_links` puts every link a text holds out as a space, wherever the
link's text recurs, the longer first, for a reader whose items a link's
digits would be misread as.

## A turn's checks in one call

`ground_message` runs on a message a run sends itself what the correction
round runs on a reply: links mended (`mended_links`), the memory layout put
in plain words, then links (`link_findings`, a placeholder target naming a
file the run made passing), figures, ids, ratios and per-unit amounts (none
on a turn that read pixels only), unchecked sheet amounts, weekdays, date
windows and internals, in that order, each finding keyed by its kind.
`ground_reply` is the reply's round: no layout scrub (a reply naming it is
asked for a rewrite, and the delivered text is scrubbed after), a link to a
file the run made reported as `attached`, and listed sheet rows left
unstated reported as `unstated`. `ground_page` holds a page to its figures,
ratios and ids only.

A finding carries what a correction names beside its value (`Detail`): a
per-unit amount its stated and printed quotients, an unstated row the
helper's line for it, an unchecked amount whether a frame was summed by
hand.

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
