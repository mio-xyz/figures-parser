# figures-parser

Rust reader for the figures a reply states and a turn carries. It is a library
with no consumer yet: the agent's grounding gate still runs its
TypeScript reader, and plugging this in is a later round.

- `figures-parser`: pure Rust, speaking byte offsets. `number` is the number
  kind: `digits` maps every decimal digit the platform's ICU formats to its
  value, from the run starts `scripts/dump-digit-values.mjs` generates into
  `src/number/digits_table.rs` (regenerate it on a Node upgrade); `scan` finds
  numeric tokens, their digit groups and symbols, and their currency sign in
  one pass; a token's `readings()` decide thousands, decimal or neighbours,
  each a `Number` borrowing the text; `Number::stated()` is what a reply is
  held to (the floors live there). `ground` is the comparison:
  `ungrounded_figures(reply, corpus)` builds its needles from the reply and
  streams the corpus once. `kind` names the kinds a caller selects, `utf16`
  converts offsets for a JavaScript caller. Group lengths are counted in code
  points, so astral digits (Adlam) group like any other. Letters and currency
  signs come from `unicode-general-category`, a Unicode version behind ICU,
  which only affects whether a letter of a script newer than its tables
  bounds a token.
- `figures-parser-napi`: the napi-rs binding, with `extract` (UTF-16 offsets,
  an error for an unknown kind name) and `ungrounded_figures`.

The fixtures under `figures-parser/tests/fixtures` are generated from Node's
Intl: `digits.json` (with the digit table) by `scripts/dump-digit-values.mjs`
and `locale-numbers.json` (how each locale prints 1234567.89) by
`scripts/dump-locale-numbers.mjs`. Each script takes `--check` and exits 1
while its files differ from what the running Node generates.

`cargo build --release -p figures-parser-napi` builds the addon, which
`smoke.mjs` loads as `figures-parser.linux-x64-gnu.node` (`rust-toolchain.toml`
pins stable). `cargo test` runs the tests.
