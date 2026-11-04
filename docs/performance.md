# Performance

Every check is linear in the corpus and stops as soon as the corpus has
carried every item of the reply. Allocations depend on the reply, never on
the size of the corpus.

## Worst case on a 300 KB corpus

The worst case is a reply whose every item is ungrounded, so the whole corpus
is read. Each corpus is 300 KB; see [Benchmarks](benchmarks.md) for what they
hold and how the times are measured.

| Check | Prose | Dumped table | Every script | Log of ids | Allocations per call |
|---|---|---|---|---|---|
| `ungrounded_figures` | 546 µs | 2.01 ms | 2.51 ms | 1.14 ms | 13 to 15 |
| `ungrounded_ratios` | 437 µs | 1.72 ms | 1.08 ms | 1.16 ms | 9 |
| `ungrounded_ratios`, thirty numbers said | 382 µs | 1.83 ms | 1.26 ms | 1.41 ms | 11 |
| `prose_arithmetic` | 571 µs | 2.17 ms | 2.28 ms | 1.20 ms | 18 to 20 |
| `Amounts::shared_with`, one print | 575 µs | 1.81 ms | 2.69 ms | 1.20 ms | 2 to 4 |
| `mispaired_per_unit` | 1.59 ms | 6.64 ms | 5.60 ms | 4.14 ms | 10 |
| `ungrounded_urls` | 102 µs | 64 µs | 51 µs | 82 µs | 14 |
| `link_findings` | 102 µs | 55 µs | 56 µs | 71 µs | 9 |
| `mended_links` | 27 µs | 25 µs | 24 µs | 58 µs | 20 |
| `ungrounded_emails` | 30 µs | 64 µs | 30 µs | 19 µs | 3 |
| `ungrounded_identifiers` | 14 µs | 65 µs | 25 µs | 63 µs | 5 |
| `ungrounded_durations` | 1.21 ms | 1.64 ms | 1.87 ms | 2.17 ms | 13 |

Times are the medians of one criterion run on a shared desktop machine,
release build. The same code moves by 10 to 20% between runs on that
machine, so a smaller difference between two versions is noise; compare two
versions in one sitting, as [Benchmarks](benchmarks.md) says. Measured that
way against 0.2.0, the figure walk costs 10 to 40% more on a corpus full of
ids, since it now reads a digit run a letter ends ("7,103MW"); the ratio
check up to 70% more on a table of decimals, since every printed decimal is
now a candidate; the link check 25 to 40 µs more for its four added rules;
the other checks the same, with the same allocations.
Reading a corpus's scale words loosely (in any case, without a currency,
glued into a word) costs the figure walk about 10% on the log of ids, whose
"latency_ms" and the like now read as scaled, and two allocations per call
there for the scaled digits' buffer, a count that does not grow with the corpus;
the other corpora move within noise.

## What to expect in practice

- **A grounded reply returns sooner.** The comparison stops once the corpus
  has carried every item.
- **Ratios and arithmetic in prose read the corpus's figures as the
  figure check does.** A reply without a ratio returns before the
  corpus is read, and so does a message that gave fewer than four values.
- **Most corpus tokens are passed over unread.** A token with too few
  digits for the shortest figure, or a decimal whose integer part no ratio
  can round to, costs one scan of its bytes and nothing more.
- **The per-unit check reads the corpus only for a candidate.** A reply
  with no clause holding two amounts and a count returns before the corpus
  is read; one with a candidate parses every printed figure of the corpus
  and meets it against each open question, a few milliseconds on 300 KB.
- **Amounts are read once.** `Amounts::of` reads the reply; each print then
  costs one corpus walk and one small clone, two allocations in all.
- **Signs are free.** A corpus token's sign is read only beside a value the
  reply states, so a signed reply costs what an unsigned one does.
- **Numbers in words cost little.** A corpus is searched for them only when
  its digits leave a reply figure unfound, by one vector search (about 40 µs
  on 300 KB).
- **Weekdays read only the reply** and allocate nothing: about 50 µs for a
  4 KB reply that names no weekday and 100 µs for one that does, after a
  one-time compile of 5 to 13 ms for each language a reply names. A wrong
  pair reads the times beside it to be named, so a reply of 900 wrong pairs,
  each with a time, takes about 9 ms where its dates alone take 5.
- **Date windows read only the reply**, line by line in every language, and
  allocate one copy of it: under 0.25 ms on each 300 KB corpus read as a
  reply, after a one-time compile of every language's patterns. The times
  of day are read only beside a date, so a table of 300 rows each dated with
  a time takes about 2.4 ms where its dates alone take 1.2, and the corpus
  of every script, whose dates sit beside digits, about 18% more. The casual
  words for a day and the lengths from now that can end a window are read on
  every line too, which costs about a fifth more on generated replies made
  of headings and dated rows, 0.44 ms each from 0.37.
