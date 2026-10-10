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
| `ungrounded_figures` | 459 µs | 1.41 ms | 1.98 ms | 557 µs | 13 |
| `ungrounded_urls` | 83 µs | 51 µs | 44 µs | 57 µs | 14 |
| `ungrounded_emails` | 34 µs | 79 µs | 39 µs | 20 µs | 3 |
| `ungrounded_identifiers` | 9 µs | 73 µs | 24 µs | 64 µs | 5 |
| `ungrounded_durations` | 1.22 ms | 1.65 ms | 1.21 ms | 1.86 ms | 13 |

Times are the median of three criterion runs on a shared desktop machine,
release build. The same code moves by up to 15% between builds with code
layout alone, so a smaller difference between two versions is noise.

## What to expect in practice

- **A grounded reply returns sooner.** The comparison stops once the corpus
  has carried every item.
- **Signs are free.** A corpus token's sign is read only beside a value the
  reply states, so a signed reply costs what an unsigned one does.
- **Numbers in words cost little.** A corpus is searched for them only when
  its digits leave a reply figure unfound, by one vector search (about 40 µs
  on 300 KB).
- **Weekdays read only the reply** and allocate nothing: about 50 µs for a
  4 KB reply that names no weekday and 100 µs for one that does, after a
  one-time compile of 5 to 13 ms for each language a reply names.
