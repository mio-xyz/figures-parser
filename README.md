# figures-parser

Rust reader for the figures and links a reply states and a turn carries. It
is a library with no consumer yet: the agent's grounding gates still run
their TypeScript readers, and plugging this in is a later round.

`figures-parser` is pure Rust and speaks byte offsets. Each kind is a module
with its own reader and its own `ungrounded_*` comparison, since a figure and
a link are grounded by different rules and different inputs.

- `number`, the number kind.
  - `digits` maps every decimal digit Unicode encodes to its value, from a
    table of the 77 runs of ten it encodes them in. The table only grows
    (Unicode's stability policy), and a test fails when the
    `unicode-general-category` tables know a digit it lacks.
  - `scan` is an iterator over numeric tokens, found by a byte cursor in one
    pass with no allocation. A token records whether a sign (`\p{Sc}`) or an
    ISO 4217 code (from the `iso_currency` crate) stands before it and after
    it, with at most one space between; a whole reading carries either mark,
    a neighbour only the mark beside it. `without_currency()` skips the marks
    for a text whose readings only ground.
  - A token's `for_each_reading` yields thousands, decimal or neighbour
    readings. `Figure` is one reading with its digits held as `D`: a
    `Reading` borrows the text and one scratch buffer the caller keeps, a
    `Number` (what `numbers(text)` iterates lazily) borrows its digits from
    the text wherever they are a slice of it and owns them otherwise.
  - `Reading::stated()` is what a reply is held to (the floors live there) and
    `Key::grounded_by` is what carries it. `ungrounded_figures(reply, corpus)`
    keeps the reply's distinct figures in one vector sorted by their
    integer's hash, behind a mask of their integer lengths, and streams the
    corpus through it until every figure is found, allocating only that
    vector and its keys.
  - Group lengths are counted in code points, so astral digits (Adlam) group
    like any other. Letters and currency signs come from
    `unicode-general-category`, a Unicode version behind ICU, which only
    affects whether a letter of a script newer than its tables bounds a token.
- `url`, the url kind.
  - `urls(text)` finds the http(s) links with `linkify`, Slack's
    `<url|label>` included, without the markup or punctuation that closed
    around them. Slack's wrapping is blanked in place, so a link's span is
    its place in the text as given.
  - `fabricated_urls(reply, corpus)` is the fabricated-link check: a link is grounded when the corpus holds it verbatim, or holds its
    path without a host (an API's `htmlPath`) and puts that path under no
    other host.
  - `ungrounded_urls(reply, corpus, prompt)` is what the link gate reads: the
    fabricated links the prompt does not carry either, an example-domain link
    only the prompt carried, link targets that are not URLs, an item's label
    on a link that is not the item's own, and an item named without the link
    a result carried.
- `kind` names the kinds a caller selects; `text` holds the character lookups
  both kinds use; `utf16` converts offsets for a JavaScript caller.

`figures-parser-napi` is the napi-rs binding: `extract` returns number and url
entities (UTF-16 offsets, a union on `kind`, an error for an unknown kind
name), beside `ungroundedFigures`, `ungroundedUrls` and `fabricatedUrls`.

`cargo build --release -p figures-parser-napi` builds the addon, which
`smoke.mjs` loads as `figures-parser.linux-x64-gnu.node` (`rust-toolchain.toml`
pins stable). `cargo test` runs the tests.
`cargo bench` times the hot paths of both kinds over four 300 KB corpora
(prose, a dumped table, every script, a log of ids) with criterion and prints
the exact allocations per call from `stats_alloc`
(`figures-parser/benches/`): the gate path allocates its needles and nothing
per token, and a grounded reply returns as soon as the corpus has carried it.
