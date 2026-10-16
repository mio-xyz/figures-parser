# Benchmarks

Two benches measure the hot paths over the same inputs: one counts
allocations exactly, the other times each path with criterion. Their
results are in [Performance](performance.md).

## Running them

```sh
# Exact allocations per call of every path over every corpus.
cargo bench -p figures-parser --bench allocations

# Timings, with criterion. A filter selects paths: corpus/path.
cargo bench -p figures-parser --bench figures
cargo bench -p figures-parser --bench figures -- table/ungrounded
```

Criterion keeps its last results under `target/criterion`, so a second run
reports the change against the first. Run both versions on the same idle
machine, and treat a difference under 15% with suspicion: code layout alone
moves timings that much between builds.

## The corpora

`figures-parser/benches/corpus/mod.rs` builds four corpora of 300 KB, each a
short unit repeated, shaped like what a language model reads:

| Corpus | Shape |
|---|---|
| `prose` | Mostly prose with a handful of figures, dates and money: a tool result with a message around it. |
| `table` | A dumped table: nearly every token is a figure. |
| `multiscript` | Figures in every script and notation the reader knows (European, Arabic, Japanese, Indian, Swiss, Russian, fullwidth), around ids and units. |
| `log` | A log of timestamps, request ids and trace ids: digit runs that are not figures. |

Each corpus comes with a grounded reply, whose figures it carries, so the
comparison exits early. The other replies carry items no corpus holds, so a
comparison reads the whole corpus: the worst case.

## The paths

| Path | What it measures |
|---|---|
| `tokens` | The numeric scanner alone. |
| `extract_numbers` | Every reading of every number, collected. |
| `ungrounded/grounded` | `ungrounded_figures` with a reply the corpus grounds. |
| `ungrounded/ungrounded` | `ungrounded_figures` with a reply it does not: the worst case. |
| `ungrounded/signed` | The same reply with signs, so corpus signs are read too. |
| `urls` | The link reader. |
| `ungrounded_urls` | The link comparison, worst case. |
| `ungrounded_emails` | The address comparison, worst case. |
| `ungrounded_identifiers` | The identifier comparison, worst case. |
| `ungrounded_durations` | The duration comparison, worst case. |
| `misdated_weekdays` | The weekday check over the corpus as a reply. |
| `ungrounded_ratios` | The ratio comparison, worst case. |
| `prose_arithmetic` | The arithmetic comparison after a message of four values, worst case. |

## Allocations

The allocation bench wraps the system allocator with `stats_alloc`, calls
each path once to warm any lazily built table, then counts the allocations,
reallocations and bytes of a second call. The count is exact, not sampled,
so a change that adds an allocation per token or per match shows at once.
