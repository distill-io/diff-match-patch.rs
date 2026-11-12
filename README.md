# diff-match-patch for Rust

A Rust port of Neil Fraser's
[diff-match-patch](https://github.com/google/diff-match-patch). It does three
things with plain text:

- **Diff**: find the differences between two texts.
- **Match**: find a pattern near an expected position, allowing for errors.
- **Patch**: turn a diff into patches and apply them, even to text that has
  drifted.

What this port adds:

- **Reference output.** Diffs, cleanups, deltas and patches are byte-identical
  to the reference JavaScript implementation for text in the Basic
  Multilingual Plane. CI checks this against a golden corpus generated from
  the vendored reference. The exceptions are opt-in and listed under
  [Compatibility](#compatibility).
- **Speed.** The internals are rewritten: slice-based search, interned line
  and word tokens, single-pass cleanups, in-place patching. See
  [Performance](#performance).
- **A small build.** No `unsafe` code (`#![forbid(unsafe_code)]`), one
  dependency by default, Rust 1.73 or newer.

The crate is in [`crates/dmp/`](crates/dmp/). Benchmarks and a profiling
harness are in [`crates/dmp-bench/`](crates/dmp-bench/).

## Install

```toml
# Diff, match and patch on Unicode scalar values.
diff_match_patch = "0.3"

# Adds Segmentation::Grapheme. Pulls in unicode-segmentation (~51 KB of tables).
diff_match_patch = { version = "0.3", features = ["grapheme"] }
```

The default build depends on `percent-encoding`, plus `web-time` on
`wasm32-unknown-unknown`. The linker drops what you do not call: a binary that
only diffs carries no match or patch code.

## Diff

```rust
use diff_match_patch::Dmp;

let mut dmp = Dmp::new();
let diffs = dmp.diff_main("The quick brown fox.", "The quick red fox.", true);

for d in &diffs {
    // operation: -1 delete, 0 equal, 1 insert
    println!("({}, {:?})", d.operation, d.text);
}
// (0, "The quick ") (-1, "b") (0, "r") (-1, "own") (1, "ed") (0, " fox.")
```

A diff is a `Vec<Diff>`. Joining the chunks that are not inserts gives the old
text (`diff_text1`); joining those that are not deletes gives the new text
(`diff_text2`).

The third argument, `checklines`, turns on a line-level first pass when both
texts are longer than 100 chars: the lines are diffed first, then the changed
lines are diffed by char. It is faster on large inputs and can return a
slightly longer diff.

### Cleanup

The search returns the shortest edit script it finds, which is rarely the one
a person would write: above, `brown` → `red` keeps the shared `r`. The cleanup
passes rewrite a diff in place. The diff still describes the same two texts.

| Pass | What it does | Use it for |
|---|---|---|
| `diff_cleanup_semantic` | Removes an equality that is no longer than the larger edit on each side of it, moves edit boundaries to word and line breaks, and turns text that a delete ends with and the next insert starts with into an equality. | Diffs shown to people. |
| `diff_cleanup_efficiency` | Removes an equality shorter than `edit_cost` chars that has a delete and an insert on both sides, or three of those four if it is shorter than `edit_cost / 2`. `edit_cost` defaults to 0, which makes the pass a no-op, so set it first. | Smaller patches and deltas. |
| `diff_cleanup_semantic_lossless` | Moves edit boundaries to word and line breaks without removing anything. | Run by the semantic pass. |
| `diff_cleanup_merge` | Joins neighbouring edits of the same kind and moves text that a delete and an insert share into the equalities around them. | Normalizing a diff you built by hand. Run by the other passes. |

```rust
let mut diffs = dmp.diff_main("The quick brown fox.", "The quick red fox.", true);
dmp.diff_cleanup_semantic(&mut diffs);
// (0, "The quick ") (-1, "brown") (1, "red") (0, " fox.")
```

### Limits

`diff_main` runs Myers' O(ND) search. The work grows with the square of the
number of differences: about D²/2 steps for D inserted plus deleted tokens, at
2 to 3 ns per step on the benchmark machine below. Two settings bound the
search:

| Setting | Default | Counts | Same output on every machine |
|---|---|---|---|
| `diff_timeout` | `Some(1.0)` | Seconds of wall-clock time | No |
| `diff_step_budget` | `None` | Search steps | Yes |

```rust
dmp.diff_timeout = None; // no clock
dmp.diff_step_budget = Some(400_000_000); // ~1 s of exact search, ~2 s at worst
```

`None` turns a limit off. The budget counts search steps only: packing lines
or words and the cleanup passes are not counted. With both set, the search
stops at whichever runs out first; a budget does not lift the timeout. The
result is always a valid diff of the two texts, just not the shortest one:

- **Timeout.** The region still being searched comes back as one delete and
  one insert.
- **Step budget.** The search runs on until it has walked a matched run of at
  least 20 tokens or looked 1,024 edits ahead, splits there, and diffs the
  parts with a second allowance the size of the budget. What is still unsolved
  after that comes back as delete and insert. Search work stays within about
  twice the budget. A larger budget usually gives a shorter diff, but not
  always.

With a timeout set and no step budget, `diff_main` also tries the reference's
half-match shortcut, which can return a longer diff. Its work is not counted
in steps, so setting `diff_step_budget` turns the shortcut off. So does
`diff_timeout = None`, as in the reference.

### Word and line diffs

To diff by word or by line, pack each distinct token into one placeholder
char, diff the packed strings, and unpack the result:

```rust
let mut dmp = Dmp::new();
let old = "the quick brown fox".to_string();
let new = "the quick red fox".to_string();

let (chars1, chars2, words) = dmp.diff_words_tochars(&old, &new);
let mut diffs = dmp.diff_main(&chars1, &chars2, false);
dmp.diff_chars_tolines(&mut diffs, &words);
// (0, "the quick ") (-1, "brown") (1, "red") (0, " fox")
```

A word token is either one whitespace char or a run of non-whitespace chars.
`diff_lines_tochars` packs lines the same way; a line keeps its trailing
`\n`. Pass `checklines: false` here: the packed strings hold placeholders, not
text.

### `word_mode`

`word_mode` is a speedup inside `diff_main`, not a granularity. When both
texts are longer than 100 chars, words are diffed first and only the changed
words are diffed by char. This is much faster on documents where most lines
change a little, such as a rename (see the benchmark below).

```rust
dmp.word_mode = true;
```

The result is a valid char-level diff, but its edit boundaries start from
word boundaries, so it is not byte-identical to the reference. Off by default.

## Match

```rust
let mut dmp = Dmp::new();
let at = dmp.match_main("The quick brown fox.", "quikc", 0);
assert_eq!(at, 4);
```

`match_main(text, pattern, loc)` returns the index of the best match near
`loc`, or -1. A match scores from 0.0 (exact, at `loc`) upwards; errors and
distance from `loc` both add to the score. `match_threshold` is the highest
score accepted. `match_distance` is the distance that adds 1.0.

## Patch

```rust
let mut dmp = Dmp::new();
let mut patches = dmp.patch_make1("old text", "new text");

// Send it somewhere as text.
let text = dmp.patch_to_text(&mut patches);
// "@@ -1,7 +1,7 @@\n-old\n+new\n  tex\n"

// Later, parse it and apply it. Each patch is located with a fuzzy match,
// so it still applies when the target has moved or changed a little.
let mut patches = dmp.patch_from_text(text);
let (patched, applied) = dmp.patch_apply(&mut patches, "old text");
assert_eq!(patched.into_iter().collect::<String>(), "new text");
assert_eq!(applied, vec![true]);
```

`patch_apply` returns the new text and one `bool` per patch. A patch that
finds no acceptable match is skipped and reported as `false`.

## Grapheme clusters

Diffs work on Unicode scalar values by default. A code point is never split,
but an edit boundary can land inside a cluster of several scalars, such as a
flag or an emoji with a modifier. With the `grapheme` feature, clusters stay
whole:

```rust
use diff_match_patch::{Dmp, Segmentation};

let mut dmp = Dmp::new();
dmp.segmentation = Segmentation::Grapheme;

// 🇷🇺 and 🇺🇸 share a scalar. Char mode would keep it and split both flags.
let diffs = dmp.diff_main("🇷🇺", "🇺🇸", false);
assert_eq!(diffs.len(), 2); // one delete, one insert
```

## API at a glance

| Method | What it does |
|---|---|
| `diff_main(text1, text2, checklines)` | Diff two texts. |
| `diff_cleanup_semantic(&mut diffs)` | Make a diff readable. See [Cleanup](#cleanup). |
| `diff_cleanup_efficiency(&mut diffs)` | Make a diff cheaper to store or send. Needs `edit_cost`. |
| `diff_cleanup_semantic_lossless`, `diff_cleanup_merge` | The two building blocks of the passes above. |
| `diff_words_tochars`, `diff_lines_tochars`, `diff_chars_tolines` | Pack words or lines into placeholder chars, and unpack a diff of them. |
| `diff_text1`, `diff_text2` | Rebuild the old or the new text from a diff. |
| `diff_levenshtein(&diffs)` | Edit distance of a diff, in chars. |
| `diff_xindex(&diffs, loc)` | Map a position in the old text to the new text. |
| `diff_todelta`, `diff_from_delta` | Encode a diff as a compact delta string, and back. |
| `match_main(text, pattern, loc)` | Find `pattern` near `loc`. Returns the best index, or -1. |
| `patch_make1(text1, text2)` | Build patches from two texts. `patch_make2` and `patch_make4` build them from diffs. |
| `patch_to_text`, `patch_from_text` | Write patches in the standard patch text format, and parse them. |
| `patch_apply(&mut patches, text)` | Apply patches. Returns the new text and one `bool` per patch. |

Configuration is plain fields on `Dmp`:

| Field | Default | Meaning |
|---|---|---|
| `diff_timeout` | `Some(1.0)` | Seconds before the diff search gives up. `None`: no limit. |
| `diff_step_budget` | `None` | Search steps before the diff search gives up. `None`: no limit. |
| `edit_cost` | `0` | Length below which `diff_cleanup_efficiency` may remove an equality. |
| `match_threshold` | `0.5` | Highest score `match_main` accepts (0.0 exact, 1.0 very loose). |
| `match_distance` | `1000` | Distance from `loc`, in chars, that adds 1.0 to a match's score. |
| `match_maxbits` | `32` | Longest pattern the fuzzy matcher takes, in chars. Patches are split to fit. |
| `patch_margin` | `4` | Chars of context kept around each patch. |
| `patch_delete_threshold` | `0.5` | How closely a large deleted block must match the target (0.0 exact). |
| `segmentation` | `Char` | Unit the diff never splits. |
| `word_mode` | `false` | Diff words first. See [`word_mode`](#word_mode). |

## Performance

Criterion medians on an i9-12900HK (default `bench` profile). Datasets are
defined in [`crates/dmp-bench/`](crates/dmp-bench/); reproduce with
`cargo bench -p dmp-bench`.

**Diff**

| Scenario | Time |
|---|---|
| Identical text, 100 KB | 3.6 µs |
| One small edit, 50 KB | 41 µs |
| Append / prepend, 50 KB | 43 / 44 µs |
| Block moved / deleted, 60 KB | 195 / 50 µs |
| Scattered word edits, 50 KB | 335 µs |
| HTML price churn, 90 KB | 463 µs |
| CJK scattered edits, 60 KB | 298 µs |
| Single-line document, 16 KB | 208 µs |
| Many small edits, 22 KB | 3.5 ms |
| All-unique lines, 180 KB | 4.1 ms |
| Rename touching every line, 18 KB | 263 ms |
| — same, with `word_mode` | 7.7 ms |
| Highly repetitive, 16 KB | 210 µs |
| Disjoint alphabets, 2 K tokens | 8.7 ms |
| Random bytes, 4 K | 11.9 ms |

**Patch, match, and cleanup**

| Scenario | Time |
|---|---|
| Build patches, scattered 50 KB | 1.7 ms |
| Apply patches, clean 50 KB | 93 µs |
| Apply patches, target shifted | 2.7 ms |
| Apply patches, contexts fuzzed | 906 µs |
| Fuzzy match, 50 KB | 128 µs |
| Near-exact match, 50 KB | 117 µs |
| Semantic cleanup, scattered | 107 µs |
| Merge cleanup, 1500 runs | 308 µs |

### How

Every optimization keeps the output byte-identical to the reference, checked
by the oracle and characterization suites. The main levers:

- **Slice-based core.** The diff recursion runs on `&[char]`, or on `&[u8]`
  when both inputs are ASCII, and builds a `String` only when it emits a
  chunk. Patches splice one `Vec<char>` in place instead of rebuilding the
  document per patch.
- **Interned tokens.** Lines, and words in `word_mode`, pack into one arena
  (`LineArena`: byte spans and hash buckets, no `String` per line). Diff chunks
  and all four cleanup passes carry runs of chars and encode to UTF-8 once, at
  the end.
- **Substring search.** Chunked skip scans that vectorize; skip-and-verify
  with a bounded KMP fallback for the containment check; KMP failure tables
  built lazily and grown as needed; a one-pass common overlap; uniqueness
  scans anchored on the rarest token.
- **Bisect.** Check-free snake walks on `usize`: the loop conditions are the
  bounds proofs. Score arrays are read through guarded, non-panicking access.
  One scratch buffer is reused across the whole recursion.
- **Cleanup.** A scan records what to change and rebuilds the list once, so
  nothing is inserted into or removed from the middle of the list. Only the
  merge pass can scan more than once: it repeats while a shift exposes
  another merge.
- **Opt-in `word_mode`.** Rename-shaped documents are diffed by word first,
  then only the changed words are diffed by char.

The full per-commit list is in [`crates/dmp/PERF.md`](crates/dmp/PERF.md).

## Compatibility

- **Units.** Every length and position counts Unicode scalar values. The
  reference counts UTF-16 code units, so delta and patch text match it byte
  for byte only where the two counts agree. A char outside the BMP, such as
  most emoji, is one unit here and two there.
- **Wire formats.** Delta and patch text do not change with `segmentation`.
- **Output that differs from the reference by design.** `word_mode`,
  `Segmentation::Grapheme`, and a diff that runs out of `diff_step_budget`.
  A diff that runs out of `diff_timeout` depends on the machine, here as in
  the reference.
- **Panics.** `diff_from_delta` and `patch_from_text` panic on malformed
  input. `match_main` panics on a pattern longer than `match_maxbits` chars,
  unless the pattern sits exactly at `loc`. Word and line packing panics on
  more than 1,112,063 distinct tokens in one call.

## Development

- `cargo test` runs everything: canonical vectors, a golden corpus generated
  from the vendored reference implementation, characterization pins, and
  property tests. `cargo test --no-default-features` covers the char-only
  build.
- The golden corpus (`crates/dmp/tests/golden/corpus.json`) comes from the
  vendored oracle (`crates/dmp/oracle/vendor/`, Apache-2.0). Regenerate it
  with `node oracle/generate.mjs` from the crate directory. CI fails if the
  checked-in corpus drifts from what the oracle produces.
- `cargo bench -p dmp-bench` runs the criterion suite (realistic and
  pathological datasets; see `crates/dmp-bench/benches/dmp.rs`).
  `cargo run --profile profiling -p dmp-bench --bin profile -- --list` shows
  the matching profiler scenarios; each run writes a flamegraph to
  `target/profiles/` and prints hotspot tables. Build with `--profile dist`
  (fat LTO) for final numbers.
