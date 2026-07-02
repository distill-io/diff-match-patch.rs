# diff-match-patch for Rust

A fast, heavily optimized Rust port of Neil Fraser's diff-match-patch. The
internals are reworked for performance — slice-based diffing, arena-interned
tokens, in-place patching, an ASCII fast path — while every result stays
byte-identical to the reference (see [Performance](#performance)).

It does three things:

- **Diff**: find the differences between two texts.
- **Match**: find a pattern in text, even a fuzzy one.
- **Patch**: turn diffs into patches, and apply them — even when the target text has drifted.

The crate lives in [`crates/dmp/`](crates/dmp/); perf tooling (criterion
benches and a profiling harness) lives in
[`crates/dmp-bench/`](crates/dmp-bench/).

## Diff

```rust
use diff_match_patch::Dmp;

let mut dmp = Dmp::new();
let diffs = dmp.diff_main("The quick brown fox.", "The quick red fox.", true);

for d in &diffs {
    // d.operation: -1 = delete, 0 = equal, 1 = insert
    // d.text: the text of this chunk
    println!("{}: {}", d.operation, d.text);
}
```

Diffs can be noisy. Clean them up for human eyes:

```rust
let (old, new) = ("The quick brown fox.", "The quick red fox.");
let mut diffs = dmp.diff_main(old, new, true);
dmp.diff_cleanup_semantic(&mut diffs);
```

Need a cap on diff time? Set a timeout in seconds:

```rust
dmp.diff_timeout = Some(1.0); // give up refining after 1 second
```

Diffing documents where nearly every line changes a little (renames,
reformatting)? Opt into word mode — large edit blocks are diffed word-by-word
first, which can be orders of magnitude faster. The result is still a valid
diff, but edit boundaries snap to word boundaries, so the output is not
byte-identical to the reference implementation's (hence off by default):

```rust
dmp.word_mode = true;
```

## Patch

```rust
let mut dmp = Dmp::new();
let mut patches = dmp.patch_make1("old text", "new text");

// Send it somewhere as text.
let text = dmp.patch_to_text(&mut patches);

// Later, parse and apply it. Application is fuzzy: it still works
// if the target text moved or changed a little.
let mut patches = dmp.patch_from_text(text);
let (patched, ok) = dmp.patch_apply(&mut patches, "old text");
let result: String = patched.into_iter().collect();
assert_eq!(ok, vec![true]);
```

## Emoji-safe diffs

By default, diffs work on Unicode scalars. That is fully Unicode-correct — a
code point is never split — but a diff boundary can land inside a multi-scalar
emoji. If that matters to you, enable the `grapheme` feature and switch modes;
clusters then stay whole:

```rust
use diff_match_patch::{Dmp, Segmentation};

let mut dmp = Dmp::new();
dmp.segmentation = Segmentation::Grapheme;

// 🇷🇺 and 🇺🇸 share a scalar. Char mode would split both flags on it.
// Grapheme mode never puts a boundary inside a cluster.
let diffs = dmp.diff_main("🇷🇺", "🇺🇸", false);
assert_eq!(diffs.len(), 2); // one delete, one insert
```

## Build options

The default build is char-only and lean.

```toml
# Default: smallest build. Diff/match/patch on Unicode scalars.
diff_match_patch = "0.3"

# Opt in to grapheme-cluster diffing (Segmentation::Grapheme).
# Adds the unicode-segmentation dependency (~51 KB of cluster tables).
diff_match_patch = { version = "0.3", features = ["grapheme"] }
```

Unused halves of the crate are removed at link time: a binary that only
diffs carries no patch or match code.

## API at a glance

| Method | What it does |
|---|---|
| `diff_main(text1, text2, checklines)` | Diff two texts. `checklines: true` uses a faster line-level first pass on large inputs. |
| `diff_cleanup_semantic(&mut diffs)` | Merge trivial edits so the diff reads well for humans. |
| `diff_cleanup_efficiency(&mut diffs)` | Merge edits to make patches cheaper. Set `dmp.edit_cost` first — the default is 0, which makes this a no-op. |
| `diff_text1 / diff_text2` | Rebuild the source / result text from a diff. |
| `diff_levenshtein(&diffs)` | Edit distance of a diff, in chars. |
| `diff_todelta / diff_from_delta` | Encode a diff as a compact delta string, and back. |
| `match_main(text, pattern, loc)` | Find `pattern` near position `loc`. Returns the best index, or -1. Fuzziness is tuned by `match_threshold` and `match_distance`. |
| `patch_make1(text1, text2)` | Build patches from two texts. (`patch_make2`/`patch_make4` build from diffs.) |
| `patch_to_text / patch_from_text` | Serialize patches to the standard patch text format, and back. |
| `patch_apply(&mut patches, text)` | Apply patches. Returns the new text and a `Vec<bool>` of per-patch success. |

Configuration lives on `Dmp` as plain fields: `diff_timeout`, `edit_cost`,
`match_threshold`, `match_distance`, `patch_margin`, `match_maxbits`,
`patch_delete_threshold`, `segmentation`, `word_mode`.

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

Every optimization is output-byte-identical to the reference (oracle +
characterization suites) except the opt-in `word_mode`. The main levers:

- **Slice-based core.** The diff recursion runs on `&[char]` — or `&[u8]` when
  both inputs are ASCII, a byte-identical fast path — and materializes a
  `String` only when emitting a chunk. Patches splice a single `Vec<char>` in
  place rather than rebuilding the document per patch.
- **Interned tokens.** Lines and words pack into one arena (`LineArena`: byte
  spans + hash buckets, no per-line `String`); diff chunks and all four cleanup
  passes carry char-run tokens and encode to UTF-8 exactly once, at the end.
- **Substring search.** Chunked, SIMD-friendly skip scans; skip-and-verify with
  a budget-bounded KMP fallback for the containment check; lazily built,
  incrementally grown KMP failure tables; a one-pass common-overlap; rarest-
  token anchoring for uniqueness scans.
- **Bisect.** Check-free `usize` snake walks whose loop conditions are the
  bounds proofs; guarded (non-panicking) score-array access; one scratch buffer
  reused across the whole recursion.
- **Cleanup.** A forward-fold merge (no quadratic insert/remove shifting); the
  lossless-slide pass moves split indices over a shared buffer.
- **Opt-in `word_mode`** diffs rename-shaped documents at word granularity
  first, then rediffs only the changed words.

The full per-commit list is in [`crates/dmp/PERF.md`](crates/dmp/PERF.md).

## Compatibility notes

- Delta and patch text are byte-compatible with the reference JavaScript
  implementation wherever scalar and UTF-16 lengths agree. Astral characters
  (emoji and other non-BMP text) count as one scalar here, two units there.
- Wire formats never change with `segmentation`. Lengths always count
  Unicode scalars of the original text.
- The parsers (`diff_from_delta`, `patch_from_text`) panic on malformed input.

## Development

- `cargo test` runs everything: canonical vectors, a golden corpus generated
  from the vendored reference implementation, characterization pins, and
  property tests. `cargo test --no-default-features` covers the char-only build.
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
