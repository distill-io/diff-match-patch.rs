# Performance optimizations

The techniques applied to the diff/patch/match core. Every one is
output-byte-identical to the reference (oracle + characterization suites green)
except the opt-in `word_mode` (item 15). Benchmark and profile with
`cargo bench -p dmp-bench -- --baseline pre-opt` and
`cargo run --profile profiling -p dmp-bench --bin profile -- <scenario>`.

Each item is one commit.

1. Diff recursion runs on `&[char]` slices end-to-end — strings are built only when a `Diff` is emitted, not re-collected at every recursion level.
2. `patch_apply` splices one `Vec<char>` in place and matches on slices, dropping the per-patch full-document string rebuilds.
3. `patch_add_context` proves pattern uniqueness in one early-exit scan (`occurs_twice`) instead of two full `find_sub`/`rfind_sub` passes.
4. `common_prefix`/`common_suffix` scan token-by-token, then switch to vectorizable 16-token block compares for long runs.
5. `half_match_at` collects every seed hit in one KMP pass and skips extensions that can't beat the best — removes the deadline blow-up on periodic text.
6. `diff_cleanup_merge` folds forward into a fresh vec instead of quadratic in-place `Vec::insert`/`remove` shifting.
7. The lossless-slide pass moves split indices over one shared buffer instead of rebuilding three char vecs per step.
8. Line packing does a single hash lookup per line with `String` keys (slice keys hash 4× the bytes on ASCII).
9. `occurs_twice` swaps KMP for skip+verify (vectorized first-token block scan + slice-eq tail); its needles are bounded, so O(n·m) is safe.
10. The internal line-interning map uses a std-only FxHash-style hasher instead of SipHash (the public API keeps SipHash).
11. `occurs_twice` anchors its skip scan on the needle's rarest token (static frequency tiers), not always token 0.
12. `find_char`'s per-line newline hunt reuses the same 16-token OR-reduction skip scan.
13. `find_sub`/`rfind_sub` jump via the chunked scan at KMP state 0, and `common_overlap` became one KMP pass whose end state *is* the overlap.
14. Line packing interns into a `LineArena` (one UTF-8 arena + byte-span ids + fx-hash buckets), so a repeated line costs no `String`.
15. **Opt-in `word_mode`** diffs over packed word tokens then rediffs only the changed words — and must skip semantic cleanup before that rediff, or short word-level equalities get eliminated and the diff collapses back into one giant block.
16. Bisect snake walks became tiny check-free `usize` loops — the dead Python negative-index wrap-guards deleted, v-array reads/writes de-panicked via guarded `get`/`get_mut`.
17. Line mode skips packing entirely for single-line inputs, whose packed diff is deterministically delete+insert.
18. A single bisect scratch buffer threads through the whole recursion, so every nested bisect is a memset into retained capacity — no per-level alloc.
19. KMP failure tables build lazily (only a state>0 mismatch consults one) with exact capacity.
20. ASCII fast path: when both sides are ASCII the same recursion runs zero-copy on `&[u8]` — a bijection with chars, so output stays byte-identical at a quarter the token traffic.
21. The internal diff is carried as token runs (`TDiff`) through recursion and all four cleanup passes; UTF-8 is encoded exactly once, at `materialize`.
22. The containment check uses a budgeted skip+verify (no KMP table), falling back to KMP only when a periodic needle keeps matching deeply.
23. A lazy incremental KMP table (`Kmp::fail`) builds only up to the highest automaton state reached, shrinking `common_overlap`'s single-line 16 KB table builds to almost nothing.

**Tried and dropped:** restructuring the bisect snake walk to call the block
compares (regressed probe-bound cases); slice-key line interning (4× hashing on
ASCII); a chunked/vectorized snake walk (a net loss — the extra setup outweighs
the wins since most probes mismatch within a token). Chunking that ultra-hot
loop loses, confirmed three times.

**Known tradeoff:** the single-source token cleanup (item 21) means the public
`diff_cleanup_efficiency` decodes the whole diff where the old String pass
decoded only the merged runs — the pass is never invoked in the pipeline's
token space, so its wrapper conversion is pure overhead. Undone only by
duplicating `merge` in String form (a drift hazard), so it stays; it is
invisible in its real caller, `patch_make`.

**Left open:** the disjoint/random-binary floors are the irreducible Myers
O((N+M)·D) cost. CJK's residual is the line arena's UTF-8 round-trip
(pack-encode + rehydrate-decode), removable with a generic `LineArena<T>`
storing `Vec<T>` — its only snag, hashing `&[char]` under
`#![forbid(unsafe_code)]`, is solved by a safe per-char byte write.
