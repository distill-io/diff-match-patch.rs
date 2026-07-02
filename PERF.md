# Performance baseline and optimization candidates

Baseline for the `perf-2` effort. Numbers from 2026-07-02 on an i9-12900HK
(WSL2), default `bench` profile (release, thin-local LTO), criterion medians.
Profiles from the pprof harness at 997 Hz on the same datasets
(`cargo run --profile profiling -p dmp-bench --bin profile -- <scenario>`;
flamegraphs land in `target/profiles/`).

Reproduce: `cargo bench -p dmp-bench` (add `--profile dist` for fat-LTO
numbers before/after any change). The pre-optimization run is saved as the
criterion baseline `pre-opt`; compare any change with
`cargo bench -p dmp-bench -- --baseline pre-opt`.

## Results: candidates 1–8 + follow-ups 9–15 (implemented 2026-07-02, this branch)

Cumulative, measured against the `pre-opt` baseline (criterion medians,
final full-suite run):

| bench | before | after | change |
|---|---|---|---|
| patch/apply_clean_50k | 5.09 ms | 107 µs | **−97.8% (47×)** |
| cleanup/merge_churn_1500 | 4.83 ms | 140 µs | **−97.2% (34×)** |
| cleanup/lossless_slide_2k | 1.27 ms | 50 µs | **−95.4% (25×)** |
| diff_heavy/code_rename_18k_words (opt-in) | 578 ms | 10.4 ms | **−98.2% (56×)** |
| diff_pathological/repetitive_16k | 5.4 ms | 618 µs | **−88.6%** |
| diff_pathological/repetitive_16k_deadline | 82.5 ms | 9.4 ms | **−88.5%** |
| patch/apply_fuzzed_50k | 6.22 ms | 942 µs | **−84.8%** |
| patch/make_html_90k | 7.23 ms | 1.7 ms | **−77.5%** |
| patch/make_scattered_50k | 8.85 ms | 2.2 ms | **−74.9%** |
| patch_make+apply/interleaved-2k | 326 µs | 103 µs | **−69.5%** |
| patch/apply_shifted_50k | 8.12 ms | 2.9 ms | **−65.6%** |
| diff_heavy/unique_lines_180k | 13.0 ms | 6.9 ms | **−47.4%** |
| diff/html_churn_90k | 1.27 ms | 799 µs | **−38.6%** |
| diff/cjk_scattered_60k | 597 µs | 400 µs | −32.8% |
| diff_main/interleaved-2k | 83.5 µs | 60 µs | −32.8% |
| diff/scattered_edits_50k | 1.00 ms | 722 µs | −28.5% |
| diff_heavy/many_small_edits_22k | 8.4 ms | 6.1 ms | −27.5% |
| diff/block_move_60k | 598 µs | 438 µs | −27.5% |
| diff_heavy/one_line_soup_16k | 1.18 ms | 939 µs | −25.5% |
| cleanup/semantic_scattered | 153 µs | 115 µs | −17.5% |
| match/exact_near_50k | 147 µs | 126 µs | −16.5% |
| match/bitap_fuzzy_50k | 155 µs | 128 µs | −15.4% |
| diff/append_50k | 144 µs | 128 µs | −14.6% |
| diff/small_edit_50k | 158 µs | 134 µs | −14.4% |
| diff/prepend_50k | 148 µs | 130 µs | −12.8% |
| diff/block_delete_60k | 212 µs | 182 µs | −12.4% |

Unchanged within this laptop's observed ±2–5% run variance: identical_100k
(the str-equality fast path is untouched; readings on this 4 µs bench drift
with code layout), the wire formats (untouched), and the bisect-walk-bound
floors — disjoint (−3.9%), random_binary (swings −8…+5 across runs),
code_rename (−4.5%). Every realistic diff shape improved 12–47%.

What was done (one commit per item):

1. Diff recursion runs on `&[char]` end-to-end (`main_slices`); bisect and
   half-match splits recurse on subslices, line mode packs from slices.
   Strings materialize only when a `Diff` is emitted.
2. `patch_apply` works in one `Vec<char>` buffer with in-place `splice`
   (perfect and imperfect paths), pattern matching goes through slice-based
   `match_chars`/`bitap` with no per-call text copies, and `patch_make4`
   maintains `postpatch` by splice. Public `match_main` keeps its historical
   byte-length loc clamp; the internal char-space entry clamps on scalars.
3. `patch_add_context` checks pattern uniqueness with one early-exit KMP pass
   (`engine::occurs_twice`) instead of `find_sub != rfind_sub` (two full
   scans, one always from position 0).
4. `common_prefix`/`common_suffix` compare token-by-token for the first 16
   tokens, then switch to 16-token block compares (vectorizable) for long
   runs. **Abandoned:** restructuring the bisect snake walk itself to call
   these (with the negative-coordinate wrap on a cold path) — it regressed
   the probe-dominated cases (disjoint +122%/+38% in two variants,
   code_rename +66%/+25%) because most probes mismatch within ~1 token and
   the added setup/branches outweigh everything; the walk stays verbatim.
5. `half_match_at` visits all seed occurrences in one KMP pass (failure
   table built once) and prunes extensions that cannot beat the running
   best. This is what collapsed the repetitive-deadline trap (6×).
6. `diff_cleanup_merge_impl` phase 1 folds into a fresh vector instead of
   in-place `Vec::insert`/`remove` (which went quadratic on many-run
   diffs). This also carried many_small (−23%) and unique_lines (−39%),
   since merge runs at every recursion flush.
7. The lossless-slide pass slides split indices over one concatenated buffer
   (invariant under both the left shift and each right slide) instead of
   rebuilding three char vectors per step.
8. Line packing hashes each line once (`get` instead of `contains_key` +
   index) and clones once per unique line. **Abandoned:** interning by
   `&[char]` slice keys — 4× the hash traffic on ASCII lines (scattered
   +16% vs −8%); String keys with single lookup won everywhere.
9. `occurs_twice` dropped KMP for a skip+verify scan: a 16-token OR-reduction
   block scan for the first token (vectorizes) plus a slice-equality tail
   verify. O(n·m) worst case is deliberate — the only caller grows needles
   only below match_maxbits − 2·margin (≈ a couple dozen tokens). Took
   patch_make from −54/−66% to −62/−76%.
10. The internal line-interning map uses an FxHash-style hasher (std-only,
    ~10 lines) instead of SipHash; NOT collision-hardened, acceptable because
    the map only interns one diff call's own lines and the public
    String-keyed API keeps std's hasher. Took scattered/html from −13/−22%
    to −22/−29%.
11. `occurs_twice` skip-scans on the needle's RAREST token (a static
    frequency-tier rank; a bad pick costs speed, never correctness) at its
    offset instead of always token 0 — on prose the first token recurs every
    few chars, the rarest every hundreds. patch_make −62% → −76%.
12. `types::find_char` (the per-line newline hunt in line packing) delegates
    to the same 16-token OR-reduction scan (`engine::skip_to`).
13. `find_sub`/`rfind_sub` jump with the chunked scan wherever classic KMP
    single-steps at automaton state 0 (identical positions visited, O(n+m)
    preserved — safe for their unbounded needles), and `common_overlap` is
    one KMP pass whose end state IS the overlap, replacing the reference's
    grow-and-search loop and its per-probe table rebuilds. The latter
    collapsed repetitive_16k (5.4 ms → 618 µs): on periodic text the old
    loop re-searched a recurring suffix once per period. Match group −15/−17%.
14. Line packing interns into a `LineArena`: one UTF-8 arena holds every
    line's bytes, id slots map to byte spans, fx-hash buckets dedup by byte
    compare. A line is encoded once into the arena tip and rolled back on a
    hit — no per-line String exists at all. html −31→−39%, cjk −20→−33%,
    unique_lines −40→−47%; the public `Vec<String>` API is rebuilt from the
    arena only for external callers.
15. **Opt-in `Dmp::word_mode`** (default off — default output stays
    oracle-identical): large edit blocks are diffed over packed word tokens
    (words = non-whitespace runs, each whitespace char its own token,
    interned in the same arena) and only the changed words are rediffed
    char-by-char; word-level rediffs and packed diffs never re-enter word
    mode (a whitespace-free block packs into one token and would recurse).
    **code_rename_18k: 578 ms → 10.4 ms (56×)** — bench id
    `diff_heavy/code_rename_18k_words`. Key finding: unlike line mode, word
    mode must NOT run semantic cleanup before its rediff — word-level
    equalities between changes are short, the pass eliminated them all and
    collapsed the diff back into one giant char-rediffed block (the first
    attempt measured a 0% win because of exactly this). Output remains a
    valid diff (reconstructs both inputs; validity-, patch-roundtrip- and
    grapheme-atomicity-tested in tests/word_mode.rs) but edit boundaries
    snap to word boundaries first, so it is not byte-identical to the
    reference — hence the flag.

Remaining hotspots after 1–15 (from the profiles):

- diff_scattered is near this design's floor: interning compare/hash ~35%,
  KMP + chunked scans ~29%, arena UTF-8 encode ~13%, bisect ~4%. A further
  step change would need line identity without UTF-8 materialization
  (hashing `&[char]` directly was measured at 4× the hash bytes) — 
  speculative.
- The truly-disjoint floors (disjoint, random_binary) are inherent
  O((N+M)·D): no shared tokens at any granularity. Rename-shaped documents
  now have the opt-in word_mode answer (item 15); default-config
  code_rename stays at the bisect floor by design.

## Baseline (criterion medians)

| bench | time | throughput |
|---|---|---|
| diff/small_edit_50k | 158 µs | 740 MiB/s |
| diff/scattered_edits_50k | 1.00 ms | 117 MiB/s |
| diff/html_churn_90k | 1.27 ms | 98 MiB/s |
| diff/append_50k | 144 µs | 847 MiB/s |
| diff/prepend_50k | 148 µs | 833 MiB/s |
| diff/block_move_60k | 598 µs | 239 MiB/s |
| diff/block_delete_60k | 212 µs | 648 MiB/s |
| diff/cjk_scattered_60k | 597 µs | 201 MiB/s |
| diff/identical_100k | 3.7 µs | 62 GiB/s |
| diff_heavy/code_rename_18k | **599 ms** | **80 KiB/s** |
| diff_heavy/many_small_edits_22k | 8.4 ms | 6.2 MiB/s |
| diff_heavy/unique_lines_180k | 13.0 ms | 21 MiB/s |
| diff_heavy/one_line_soup_16k | 1.18 ms | 31 MiB/s |
| diff_pathological/disjoint_2k | 10.8 ms | 360 KiB/s |
| diff_pathological/random_binary_4k | 19.7 ms | 396 KiB/s |
| diff_pathological/random_binary_4k_deadline | 19.9 ms | 393 KiB/s |
| diff_pathological/repetitive_16k | 5.4 ms | 5.6 MiB/s |
| diff_pathological/repetitive_16k_deadline | **82.5 ms** | 374 KiB/s |
| cleanup/semantic_scattered | 153 µs | |
| cleanup/efficiency_scattered | 53 µs | |
| cleanup/lossless_slide_2k | 1.27 ms | |
| cleanup/merge_churn_1500 | 4.83 ms | |
| patch/make_scattered_50k | 8.85 ms | |
| patch/make_html_90k | 7.23 ms | |
| patch/apply_clean_50k | 5.09 ms | |
| patch/apply_shifted_50k | 8.12 ms | |
| patch/apply_fuzzed_50k | 6.22 ms | |
| wire/todelta_scattered | 10.7 µs | |
| wire/fromdelta_scattered | 85 µs | |
| wire/patch_to_text_scattered | 11.2 µs | |
| wire/patch_from_text_scattered | 7.9 µs | |
| match/bitap_fuzzy_50k | 155 µs | |
| match/exact_near_50k | 147 µs | |
| diff_main/interleaved-2k (legacy) | 83.5 µs | |
| patch_make+apply/interleaved-2k (legacy) | 326 µs | |

## What the profiles say

- **diff_scattered** (the representative realistic case): only ~2% of time is
  in `engine::bisect`. The rest is materialization and tokenization overhead —
  `ptr::write` 34%, `Vec` bookkeeping 23%, SipHash (line-packing `HashMap`)
  ~15%, UTF-8 encode/decode ~7%. The diff algorithm is not the realistic
  bottleneck; the representation is.
- **patch_make**: 58% `rfind_sub` + 18% `find_sub` — `patch_add_context`'s
  uniqueness check runs two whole-text KMP scans per iteration per patch, and
  `rfind_sub` always scans from position 0.
- **patch_apply_clean**: ~99% conversion churn (`encode_utf8` 50%,
  `next_code_point` 25%, `ptr::write` 23%) — the per-patch full-document
  `String` rebuilds and `match_main`'s per-call full-text copies.
- **diff_code_rename**: 98.5% inside `engine::bisect`, of which 28.7% is slice
  bounds-check/index overhead (`SliceIndex::index`). A rename touching every
  line defeats line mode, so bisect runs on a ~40K-char replacement block with
  D ≈ 10K.
- **diff_repetitive_deadline**: 61% `find_sub` + 19% `char::ne` + 17%
  `common_prefix` — `half_match_at` re-runs KMP (rebuilding a 4000-entry
  failure table) for every occurrence of a seed that recurs every 10 chars.
  The "speedup" makes this input 15× slower than no deadline.
- **match_bitap**: 39% `ptr::write` (full-haystack copies per call), 21%
  hashbrown lookups (per-char alphabet `HashMap` in the scan loop), 20%
  `find_sub`.
- **cleanup_merge / cleanup_lossless**: undersampled by pprof (the libc
  blocklist absorbs memmove-dominated stacks), but the shape is unambiguous in
  code: `Vec::remove`/`insert` shifting in a loop, and per-step
  `edit_vec[1..].to_vec()` while sliding. The bench points (4.8 ms for 4500
  tiny diffs; 1.3 ms for a 6KB constructed diff) corroborate.

## Ranked candidates

Items 1–3 are DONE (see results above); 4–10 remain open.

Tier 1 — representation (biggest realistic-workload wins):

1. **[DONE] Run the diff recursion on `&[char]` end-to-end.** `main_internal`
   re-collects `Vec<char>` from `String` at every recursion level, and
   `bisect_diff` materializes four `String`s per split
   (`src/diff.rs:177`, `src/diff.rs:270-276`). Across a D-deep recursion this
   is O(N·D) copying; it is 40-60% of every realistic diff bench. Materialize
   `String`s only when emitting `Diff`s.
2. **[DONE] Apply patches into one `Vec<char>` buffer.** `patch_apply` rebuilds the
   whole document via `String` concat + re-collect per patch
   (`src/patch.rs:322-326`, `349-366`), and `match_main` copies the full
   haystack per call (`src/match_.rs:28-29`). Splice in place; pass `&[char]`
   into match. This is essentially all of apply_clean's 5.1 ms.

Tier 2 — algorithmic hot spots:

3. **[DONE] `patch_add_context` uniqueness scan** (`src/patch.rs:32-44`): replace
   `find_sub(..) != rfind_sub(..)` (two full scans, one always from 0) with a
   single scan that stops at the second occurrence. ~76% of patch_make.
4. **[DONE, reduced] Bisect snake walk** (`src/engine.rs:262-352`): usize indices with a
   slice-zip run scan so the equality run vectorizes and bounds checks
   disappear; keep the negative-index wrap semantics on the cold path.
   ~29% measured overhead on bisect-bound workloads (code_rename, disjoint,
   random_binary), likely more with SIMD on the runs.
5. **[DONE] `half_match_at` seed scan** (`src/engine.rs:203-222`): one KMP pass
   collecting all occurrences (build the failure table once), plus a
   can't-beat-best prune before extending prefix/suffix. Fixes the 15×
   deadline regression on repetitive text.
6. **[DONE] `diff_cleanup_merge` single-pass rebuild** (`src/cleanup.rs:443-583`):
   the pass edits in place via `Vec::insert`/`remove` (O(n) shifts each, plus
   `insert(0, ..)`), going quadratic on many-run diffs. Fold into a fresh Vec.
7. **[DONE] Lossless slide by index** (`src/cleanup.rs:214-233`): the
   slide loop clones three char vectors per step (`to_vec()` on every shift) —
   quadratic on a single big edit. Slide split points over stable buffers.

Tier 3 — local:

8. **[DONE, reduced] Line packing** (`src/tokenize.rs:119-179`): entry-API single hash lookup
   (currently `contains_key` + index), avoid cloning each unique line twice,
   consider hashing spans instead of owned `String`s. 10-20% of
   line-mode-bound diffs (scattered/html/unique_lines).
9. **Bitap** (`src/match_.rs`): array-based alphabet for ASCII patterns
   (pattern ≤ 32 chars, so a `[u64; 128]` + overflow map), window `rfind_sub`
   by `loc` + score radius instead of scanning from 0.
10. **Wire formats** (`src/delta.rs`, `src/patch.rs`): pre-reserve output
    strings; decode deltas by `char_indices` over `text1` instead of
    collecting the full `Vec<char>`. Small absolute costs (≤85 µs).

Experimental — DONE as item 15: the word-mode intermediate pass shipped
behind the opt-in `Dmp::word_mode` flag (default off keeps output
oracle-identical); it collapses rename-shaped documents 56× (578 ms →
10.4 ms). See item 15 for the semantic-cleanup finding that made it work.

Non-goals: `diff/identical`, `append`/`prepend`, and the wire formats are
already at or near memory bandwidth for their shapes; the disjoint/random
pathological floors are inherent to Myers O((N+M)·D) and only move with
tier-2 item 4 constants or a deadline.
