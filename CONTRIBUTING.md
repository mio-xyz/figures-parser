# Contributing

Thanks for helping. This guide covers setup, the checks a change has to
pass, and the conventions the code follows. [Internals](docs/internals.md)
explains how each reader works.

## Reporting a problem

Open an issue with the smallest input that shows it: the call, the reply,
corpus and prompt you passed, what it returned and what you expected. A
reading in another language or script is welcome; say which locale writes
it that way.

## Setup

`rust-toolchain.toml` pins the Rust release, so you and CI lint with the
same clippy. rustup installs it on the first `cargo` command in the
repository, or ahead of time with:

```sh
rustup toolchain install
```

The Node binding's smoke test needs Node 22 or later.

## Checks

CI runs these on every push and pull request
([`.github/workflows/ci.yml`](.github/workflows/ci.yml)). Run them before
you open one:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace

# Docs, with every warning an error.
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items

# The panic-freedom tests, in a release build that panics on overflow.
# PANIC_CHECK_CASES sets the number of generated inputs (default 200,000).
cargo test --profile panic-check -p figures-parser --test it -- --ignored

# The Node binding (on Linux; see the README for other platforms).
cargo build --release -p figures-parser-napi
cp target/release/libfigures_parser_napi.so figures-parser.linux-x64-gnu.node
node smoke.mjs
```

CI also runs the library's tests on macOS and Windows.

## Lint policy

Every warning is an error. The workspace manifest denies clippy's `all`,
`pedantic` and `nursery` groups, and restriction lints chosen one by one:
`unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `string_slice`,
`as_conversions`, `shadow_unrelated`, `wildcard_enum_match_arm` and more
(see `Cargo.toml`). Every item, private ones included, is documented.

When a lint has to yield, put `#[expect(lint, reason = "...")]` on the
narrowest item and say why; an `allow` without a reason is itself denied.

A new Rust release can bring new lints. Moving to it is a change of its own:
bump `rust-toolchain.toml` and the badge in the README, and fix what the new
clippy finds.

## Conventions

- **No `unsafe`, no panics.** Arithmetic on offsets is checked or
  saturating, and a lookup goes through `get`. A new entry point or reader
  gets its inputs into the panic-freedom pool in `tests/it/panic_free.rs`.
- **Allocation is bounded by the reply.** A reader allocates nothing per
  token, and a comparison allocates what it keeps from the reply, never per
  match of the corpus.
- **Comments state a constraint or a reason**, in the present tense. What the
  code does is the code's job; how it came to be is the commit's.
- **Plain punctuation in docs**, without em dashes.

## Tests

- Integration tests live in `figures-parser/tests/it/`, one file per kind.
  A change in what a reader reads states both what it now reads and what it
  must still leave alone.
- Every public item carries a doc example, and the README's examples run as
  doc tests, so a README example that stops being true fails `cargo test`.
- A change to a comparison is worth a generated-input check of its own: for
  numbers, every generated text must ground its own figures.

## Performance

A change on a hot path runs both [benchmarks](docs/benchmarks.md) before and
after, on the same idle machine, and states the difference in the pull
request. The allocation counts must not grow with the corpus. Update the
table in [Performance](docs/performance.md) when the numbers move beyond
noise.

## Documentation

- The README shows how to use the library; keep it to usage.
- [Internals](docs/internals.md) explains how a reader decides; update it
  when behavior changes.
- [Known gaps](docs/known-gaps.md) lists what is not read; remove an entry
  your change closes, and add one you find.

## Commits and pull requests

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/):
`feat:`, `fix:`, `perf:`, `refactor:`, `docs:`, `test:`, `ci:` or `chore:`,
then what the library now does. The body says why, and what was measured.
A pull request needs a green CI.

## License

By contributing, you agree that your contribution is licensed under the
project's license, the GNU Affero General Public License, version 3 only
(`AGPL-3.0-only`). Code ported from another project must carry a license
compatible with it, and its notice goes in `THIRD_PARTY_NOTICES.md`.
