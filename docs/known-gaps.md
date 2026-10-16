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
  ("1.2万5000"). Korean multipliers are not read, so "3억 5천만" reads as 3
  and 5.
- "billion" is read on the short scale, 10⁹, as English writes it; the
  French "billion" and the German "Billion" are 10¹², so "1 billion" in a
  French reply is read a thousand times too small. The plural forms that only
  those languages write ("Billionen", "billones") are read as 10¹².
- A long scale word glued to the digits ("12million") is not read, nor is a
  scale word after more than one space.
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

## Ratios

- A percentage in words ("twelve percent"), a per mille ("‰"), percentage
  points ("12 pp") and a multiple in words ("three times") are not read.
- A decimal written without its lead (".1429") is not read, so it grounds
  nothing.
- A space-grouped figure before a percentage reads as one ("3 118.380 %" is
  3,118.38%), as French and SI write it.
- A ratio is reported as written, so a fullwidth or small percent sign
  ("５０％", "50﹪") is not folded to "%" in what is named.
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
- A one-letter top-level domain is not a host (`https://a.b/c`).
- A link's query is read as written: an encoded value in a thread link is
  not decoded before it is looked for in the corpus.
- A built link's host list is the caller's: with none given, only a known
  shape's own host passes unprinted, and only at its root.

## Weekdays

- Languages outside the fifteen, such as Korean, Polish, Persian, Hindi or
  Turkish, are not read.
- Arabic: a Hijri date is not checked, nor a day written as an ordinal word
  ("السادس والعشرين من سبتمبر"), a weekday without its article ("جمعة"), a
  weekday after its date, or names written with their short vowels
  ("الجُمُعَة").
- Two-letter abbreviations ("Mo", "Fr") are not read, since they are words in
  other languages ("to", "so", "di").
- Only a weekday written next to its date is paired. A weekday and a date
  apart in a sentence ("Friday we meet; the date is 26 Sep"), or a relative
  weekday ("next Friday"), are not checked.
- In English, a lowercase "may" and a slash date whose two numbers could each
  be the month ("3/4") are skipped as ambiguous.
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

## Durations

- German, Dutch, Swedish, Finnish, Russian, Ukrainian, Japanese, Chinese and
  Vietnamese words are not read, though the weekday check reads those
  languages.
- Arabic: number words past ninety-nine ("مئة ساعة"), dialect numbers
  ("تلات ساعات"), and units written with their short vowels are not read. A
  lone singular unit ("استغرق ساعة", "it took an hour") is not read either,
  for the same reason "an hour" alone is not.
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
