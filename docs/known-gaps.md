# Known gaps

What each reader does not read yet. Each was checked against the library as
it stands; a contribution that closes one should remove it here.

## Numbers

- A number in words below a hundred is not read ("twelve users"), nor an
  ordinal ("the thousandth"), a fraction other than a half ("a quarter
  million"), a number in capitals ("TWELVE THOUSAND") or a number in another
  language.
- "At one point two million people" reads "one point two million", since
  "point" between number words is read as a decimal point.
- Han digits after Arabic ones ("3億五千万") read as two figures, "3億" and
  "五千万", and so do digits after a Han multiplier that follows a decimal
  ("1.2万5000"). Korean sections a space parts ("3억 5천만") read as two
  figures, 300,000,000 and 50,000,000.
- A name some locale gives another power ("billion" is 10¹² in French) reads
  at that power only where a currency or the "of" of a Romance language
  follows the scale, so a French "1 billion" with neither is read at 10⁹.
- A scale word after more than one space is not read.
- An abbreviation that is also a unit is told from a scale by a short list
  of words around the figure ("took", "ago", "tall", "truck", "files"), in
  English and French, so a unit with none of them is read as a scale, as the
  host reads it: "5m of cable" and "a 5m wall" are millions, and so is "spent
  5m", which is as often money.
- A sign written in words ("minus 12,500"), with an en dash ("–12,500"), after
  the figure ("12,500-") or as an accounting suffix ("12,500 CR") is not
  read, so such a figure holds the corpus to no sign.
- Approximation in words ("about 12,500", "roughly", "environ", "約") or after
  the figure ("12,500-ish", "12,500 or so") is not read, so such a figure is
  held to its exact value.
- Three-digit values a plain space lists ("120 340 560") read as one
  thousands figure, as French writes 120,340,560; the corpus still grounds
  each of them.
- A corpus grounds by its digit tokens as printed, before its numbers in
  words are read: "2 million three hundred thousand" in a corpus grounds
  2,000,000 as well as 2,300,000. A reply writing it is held to 2,300,000.
- A round range is read only between two tokens a dash joins: "$200 to
  $300" and "between 200 and 300" are checked bound by bound, and a bound
  read with its own sign ("$200 -$300") is a signed figure, not a range.
- A lone round figure of one significant digit is read at two, so "$3,000"
  takes 2,950 to 3,049, while the same figure behind an approximation mark
  ("~$3,000") is read at its own precision and takes 2,500 to 3,499. A short
  decimal is held as its whole part ("$3,200.50" as 3,200) and so rounds a
  printed 3,225 as well.
- A bare digit run after an approximation mark ("~12500") is held, where
  the same run without the mark is not; a mark in words ("about 12500") is
  not read, so such a run passes.
- "half a hundred thousand" is not read, since a hundred does not follow a
  half; "a hundred thousand" is read from it, as an idiom.

## Per-unit amounts

- An amount is a figure with a currency mark before it ("$45.83", "USD
  45.83"); one written with its mark after ("45,83 €") is read as a count.
- A clause ends where Unicode's sentence rules end one, approximately: a full
  stop glued to the next word ("lifetime.Next") ends none, and abbreviations
  ("e.g. the") end one.
- Three amounts and two counts in one clause are paired as any total over any
  count, so a clause listing two windows side by side may pair across them.
- A sum of printed amounts over a count is not derived.

## Ratios

- A percentage in words ("twelve percent"), a per mille ("‰"), percentage
  points ("12 pp") and a multiple in words ("three times") are not read.
- A decimal written without its lead (".1429") is not read, so it grounds
  nothing.
- A space-grouped figure before a percentage reads as one ("3 118.380 %" is
  3,118.38%), as French and SI write it.
- A ratio is reported as written, so a fullwidth or small percent sign
  ("５０％", "50﹪") is not folded to "%" in what is named.
- A percent sign between two figures, set apart from the first and glued to
  the second ("18.65 %59.01"), is read as the second's, as Turkish writes
  it; the host's TypeScript reads it as the first's. Only one of the two is
  checked.
- The person's numbers are read at most thirty at a time, and a ratio of
  three of them (a share of a sum) is not derived.

## Arithmetic in prose

- A plain-space thousands run counts once per group as operands ("12 500"
  counts twice), since a list is written the same way, while a reply's is
  read as one figure.
- An unsigned result is grounded by the same value printed negative, as a
  figure is, since a reply may put the sign in words.
- A result written in words below a hundred is not read, as no number in
  words below a hundred is.

## Links

- A link without a scheme ("acme.io/x") is not read in a reply; a tool
  returns full links. In a corpus one grounds the reply's schemed link.
- A built link's host list is the caller's: with none given, only a known
  shape's own host passes unprinted, and only at its root.
- `url_printed_in` grounds an address by its path alone wherever the text
  holds it, and a root address with a query by its query alone (`?page=2`),
  as a page's relative link gives them, so a page on another host that links
  the same path grounds it.
- A link printed without its scheme grounds every link it starts, as one
  printed with it does: "github.com/org/repository" grounds
  "https://github.com/org/repo".

## Memory

- A dossier line is matched by its words alone: a reply that repeats two of
  a line's words in another sense ("Tuesday" and "standup" about another
  team) draws on it.

## Weekdays

- Languages outside the fifteen, such as Korean, Polish, Persian, Hindi or
  Turkish, are not read.
- Arabic: a Hijri date is not checked, nor a day written as an ordinal word
  ("السادس والعشرين من سبتمبر"), a weekday without its article ("جمعة"), a
  weekday after its date, or names written with their short vowels
  ("الجُمُعَة").
- Two-letter abbreviations ("Mo", "Fr") are not read, since they are words in
  other languages ("to", "so", "di").
- Only a weekday written next to its date, next to a bare day number, or
  before a casual word for a day ("Friday, tomorrow") is paired. A weekday
  and a date apart in a sentence ("Friday we meet; the date is 26 Sep"), a
  casual word before its weekday outside Japanese ("tomorrow (Friday)"), or
  a relative weekday ("next Friday"), are not checked.
- A bare day number followed by a word is checked only when the rest of the
  line opens on a time of day. Of chrono's readings there, a date with a
  length added ("Nov 6 - 14h", chrono's relative-after-date merge) is not
  read, nor is a weekday joined with a time into a range of times ("thứ ba -
  13giờ tối - 16:00"). An Arabic weekday beside a bare day number is not
  checked.
- A bare day number is read against a window from three weeks back to five
  weeks ahead, so a plan more than five weeks out ("Tuesday 4" for a meeting
  two months away) is read against the wrong month.
- In English, a lowercase "may" and a slash date whose two numbers could each
  be the month ("3/4") are skipped as ambiguous.
- A two-digit number after a month-name date, below 24, is read as chrono
  reads it, as an hour, so "samedi 13 juillet 23" is checked against this
  year, not 2023.
- A date without a year takes the year chrono-node would give it, which can
  be the wrong one.

## Email addresses

- An address written out to dodge scrapers ("ana at acme dot io") is not
  read.

## Identifiers

- A lowercase tracker key ("core-1710") is not read as a key.
- A key or a prefixed id is checked only when the corpus holds another of the
  same prefix, so an invented one under a prefix the corpus never uses
  passes.
- Long mixed tokens are never checked, and ISO durations ("PT1H30M45S") and
  chat user ids ("U01ABCDEF2") land among them.
- `unprinted_ids` reads no tracker key (`ungrounded_identifiers` holds one
  to the corpus's other keys of its prefix), and no typed id whose prefix
  holds a digit (`s3_…`), or a second word that names no key's mode
  (`test`, `live`, `sandbox`).
- A one-word prefix before a mixed body is read as a typed id, so a name a
  reply coins in that shape (`campaign_BlackFriday2024`) is held to the
  corpus, as the TypeScript check holds it.

## Durations

- Days, weeks, months and years are read only in English, French, Spanish,
  Italian, Portuguese and Arabic, and number words only in those languages
  ("fünf Minuten", "пять минут" are not read). Russian and Polish case forms
  past CLDR's and the accusative ("минутах", "godzinach") are not read.
- Some CLDR short names are words too: "jam" (Indonesian hours), "tim"
  (Swedish), "std" (German) and "sa" (Turkish), so "3 jam sessions" and "3
  std above" read as three hours, as the host's TypeScript reads them.
- Arabic: number words past ninety-nine ("مئة ساعة"), dialect numbers
  ("تلات ساعات"), and units written with their short vowels are not read. A
  lone singular unit ("استغرق ساعة", "it took an hour") is not read either,
  for the same reason "an hour" alone is not.
- Number words past ninety-nine ("a hundred hours") are not read, and a
  decimal in words is misread: "one point five hours" reads as five hours.
- "An hour" alone is not read, only "an hour and a half", since "once a day"
  uses the same article.
- A range is read only with a bare number and a dash before a part in
  digits: "2 to 3 hours", "between 2 and 3 hours" and "two-three hours"
  read their upper end only, as the host's TypeScript reads them.
- Minutes after a spaced hour are dropped ("2h 30" is 2 hours), and "1h30s"
  is not read.
- A reply is not held to a length in days or longer units alone ("a 14-day
  trial", "took 2 months"), as the host's TypeScript holds none, since a
  window ("the last 30 days") and a plan ("in 2 weeks") read the same way.
  Calendar lengths do not convert to days ("4 weeks" is not "1 month", "365
  days" not "1 year"), nor a corpus's date range to its span, which holding
  them would need.
- A clock reading in a corpus grounds a length only under a label ("took",
  "duration", "elapsed") or past a day's hours, so a bare one ("Intro 3:45")
  grounds nothing, as in the host's TypeScript, which reads no clock
  reading.
- A printed length rounds to a reply's at the reply's place, so a long unit
  is loose: "2 hours" is grounded by any printed length from 1:30:01 to
  2:29:59, and "1 year" by 6.01 to 17.99 months.
- A reply's duration is reported without the full stop after its unit
  ("5 minutes", where the host's TypeScript reports "5 minutes."), unless
  the unit's own abbreviation takes the dot ("2 Std.", "15 min.").
- A number straddling the end of a link ("https://x.io/11 528 seconds") is
  read whole, so it starts inside the link and is not checked.

### Where this library reads durations more strictly than the host's TypeScript

Each of these is a reading the TypeScript check gets wrong; the library's
is pinned in `tests/it/durations.rs`.

- A decimal is held to the place it is written to: "1.5 hours" is the half
  hour, so 70 minutes, nearer an hour, does not ground it, and "0.5 hours"
  is not grounded by 50 seconds. The TypeScript check rounds every length at
  its unit, so any length from 61 to 119 minutes grounds "1.5 hours".
- Parts "and" joins are one length: "2 minutes and 30 seconds" is grounded
  by 150 seconds, where the TypeScript check reads two lengths and flags
  both. In a corpus too: "1 hour and 30 minutes" is ninety minutes, so it
  grounds no reply's "30 minutes", nor "3 hours and 58 minutes" a reply's "3
  hours", where the TypeScript check reads each part as a length printed on
  its own.
- A part in days joins the parts after it ("2 days and 3 hours" is 51
  hours), where the TypeScript check reads no day and holds the reply to "3
  hours" alone.
- A full stop ends a length: "2 hours. 30 minutes" is two lengths, where the
  TypeScript check joins them across the sentence. The dot after a short
  form ("1 hr. 15 min.") closes the abbreviation, so there the parts join as
  in the TypeScript check.
- A range written high to low ("30-10 minutes", "step 3 - 2 minutes") is
  read as the span between its numbers, where the TypeScript check drops it
  and so holds the reply to nothing; "18-18 seconds" is 18 seconds.
- A dash after a line break opens a list item: "Total: 10\n- 30 min" is no
  range from 10 to 30 minutes.
- A fraction before a unit is read whole: "1/2 hours" is half an hour and "1
  1/2 hours" an hour and a half, where the TypeScript check reads "2 hours"
  in both. A number after another number's symbol is not read: "x7,59 min"
  is not 59 minutes.
- A unit name in an uncased script may go on in letters and marks of its
  own ("۳ ساعته", "5 मिनटों"), as in the TypeScript check, but not into a
  Latin letter: "วินาทีly" or "ساعتانly" hold no unit, where the TypeScript
  check reads "วิ" and "ساعت" inside them.
- Two numbers a plain space sets apart are two ("1:52:39 1.75 seconds"
  prints 1.75 seconds), where the TypeScript check reads one token and
  neither number.
- A number before a one-letter symbol ("24 h", "30 s") grounds a reply's
  duration ("3 hours" by "took 3h", "pendant 3 h"), but not as a time of day
  ("à 3 h du matin", "um 15h", "de 14h à 15h30"); neither the TypeScript
  check nor this library holds a reply to one, nor to "15h30", since "à 15
  h" and "15h30" are times of day.
- A printed part joined to another is one length with it in a corpus: "30
  minutes" is not grounded by "130 minutes", nor by "1 hour 30 minutes",
  nor by the upper bound of "10-30 minutes".
- A corpus also prints a length as an ISO 8601 duration, a field named for
  its unit, or a clock reading labelled as a length ("Duration: 1:30:00",
  "took 1:30:00"), which the TypeScript check does not read.

## Rows under a date window

- A heading that names its window in words alone ("this week", "the last 7
  days") opens none, as it opens none in the TypeScript check: chrono reads
  one date there, not a range. One that runs from or to a casual day or a
  length from now ("Oct 20 to today", "3 days ago - today") opens it.
- A heading that chains several ranges and times ("Oct 10 9:00-17:00 to Nov 8
  9:00-17:00") is read as chrono's refiners would join it in English, in
  every language, so its window can differ from the TypeScript check's. On
  a generated corpus of 24,000 such headings, about 260 open a window under
  which only the TypeScript check flags a row; most are Chinese, Japanese
  or Vietnamese chains whose end runs before their start, and most of the
  rest are the readings [Guarantees](guarantees.md) lists as deliberate.
- A date with a length added ("Nov 6, 2026 - 14h", chrono's
  relative-after-date merge) is not read as one date.
- Arabic is not read, nor a window or a row in a language whose date words
  the weekday check does not read.

## Not ported

- The host's check that a spreadsheet read is a ledger summed by period
  (records in a column of words, placed by a column of dates or laid across
  a header of months) is not ported. It judges structured cells, from a
  workbook, a delimited file or an API's records, some typed as dates by the
  file itself, and decides which helpers the host loads for them; it reads
  no reply or corpus text.
