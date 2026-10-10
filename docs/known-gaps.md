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

## Links

- A link without a scheme ("acme.io/x") is not read; a tool returns full
  links.
- A one-letter top-level domain is not a host (`https://a.b/c`).

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
  Slack ids ("U01ABCDEF2") land among them.

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
