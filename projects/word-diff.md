# Word-diff mode for change monitoring — pending design

**Status:** design agreed in principle, not yet implemented. One prerequisite bug is
already fixed and committed; the rest is waiting on two product decisions (see *Open
questions*).

**Crate:** `deps/diff-match-patch.rs` (the vendored `diff_match_patch` fork). Pre-publish,
so the public API is still ours to shape.

---

## The need

Distill diffs web content and lets users attach **keyword conditions** to the changes
("notify me when *sold out* appears", "alert if the version changes"). The condition
matches against the **changed text** — the inserted/deleted pieces of the diff.

The default diff is character-level (Myers). That's correct for sync/patch, but it
*fragments tokens*, and a fragmented token can't be matched as a keyword:

| edit | char-level diff | what a keyword condition sees |
|---|---|---|
| `committed` → `commits` | `commit[-ted-]{+s+}` | inserted text is just `"s"` — "commits added" **misses** |
| `v1.2.3` → `v1.2.4` | `v1.2.[-3-]{+4+}` | inserted text is `"4"` — "v1.2.4 added" **misses** |

We want a **word/token diff** where every changed token stays whole, so the added/removed
side surfaces complete words and values that conditions can key on. This is a different
objective from the default mode (readability/minimal-patch), so it lives behind a flag, not
as a change to the default.

Two things also have to be true for that mode to be trustworthy:

1. **It must not fragment tokens** — that's the whole point (see the examples).
2. **It must not split a character** — token boundaries have to land on grapheme-cluster
   boundaries, or an edit boundary can fall inside a `\r\n` or a base+accent pair.

---

## Where things stand

**Done and committed:**

- `307b77f` — `vs_similar` bench folded into `dmp-bench` (opt-in, dev-dep only), so we can
  keep measuring against the `similar` crate.
- `8a239ff` — **surrogate-gap fix.** The token packers assign placeholder ids that skip the
  Unicode surrogate range (U+D800–U+DFFF). The public `diff_words_tochars` didn't skip it and
  **panicked at exactly 55,296 unique tokens**; the internal arena rehydration indexed spans
  by the *shifted* id and silently returned the wrong line past the gap; the public line
  tokenizer had the same misalignment. All three fixed (public words packer now delegates to
  the arena packer; blank fillers bridge the gap in the id-indexed arrays; an `id_to_slot`
  inverse un-shifts on rehydration). Six new boundary tests, full suite + grapheme feature
  green, no perf change. This was a real latent bug carried over from the original crate,
  worth having fixed regardless of the word-diff feature.

**Not done:** the word-diff mode itself — the token-atomic redesign of `word_mode`, the
grapheme-safe tokenizer, and the consumer-facing entry point.

---

## The examples that pin the design

These are the cases we've been reasoning against (all run against the real crate / a
prototype of the proposed pipeline). Keep them as the acceptance suite.

**a) Whole tokens must survive (the reason the feature exists)**

```
committed → commits     want  [-committed-]{+commits+}      (not commit[-ted-]{+s+})
v1.2.3    → v1.2.4       want  [-v1.2.3-]{+v1.2.4+}
$19.99    → $24.99       want  [-$19.99-]{+$24.99+}
```

**b) Char-level over-matches coincidental structure; word-level doesn't**

```
0:00 → 30:58    char: {+3+}0:[-00-]{+58+}   (spurious shared "0:")   word: [-0:00-]{+30:58+}  ✓
```

**c) But word-level, done naively, has its own trap — the whitespace freak match**

`Item 1 → First Item`: the tokenizer emits `[Item, ␣, 1]` vs `[First, ␣, Item]`. Myers
matches the **positionally-aligned space** (both at index 1), not the moved `Item`, and the
raw word diff comes out as `[-Item-]{+First+} ␣ [-1-]{+Item+}` — `Item` shown as *both*
deleted and inserted. It's **semantic cleanup's char-level overlap pass** that rescues it back
to `{+First +}Item[- 1-]`. Lesson: the good output depends on cleanup, and cleanup fights the
"keep tokens individually matchable" goal — the two are entangled (see *Decision 2*).

**d) Grapheme safety (what the tokenizer must guarantee)**

```
"a\r\nb"   current: ["a","\r","\n","b"]   splits the CRLF cluster        proposed: ["a","\r\n","b"]  ✓
"x ́y"      current: ["x"," ","\u{301}y"]  splits space+combining         proposed: ["x"," ́","y"]     ✓
```

**e) …without shredding structured values**

```
ABC-123   whitespace-run: ["ABC-123"]        UAX-29 word split: ["ABC","-","123"]
$24.99    whitespace-run: ["$24.99"]         UAX-29 word split: ["$","24.99"]
https://…  whitespace-run: one token          UAX-29: 11 tokens
```

---

## Decision 1 — the tokenizer boundary rule

*What counts as a "word".*

**Option A — whitespace-run vocabulary, walked by grapheme cluster (recommended).**
A word is a maximal run of non-whitespace clusters; each whitespace cluster is its own token.
Same vocabulary we have today, but the scan walks clusters instead of scalars, and a cluster
is a separator iff its *first* scalar is whitespace (whitespace only ever appears as a cluster
base, so this cleanly catches `\r\n` and space+combining).
- **Pros:** fixes CRLF and combining-mark splits; keeps `ABC-123`/`$24.99`/URLs/emoji whole
  (which is what monitoring wants); tiny change (only the walk); ASCII stays on a fast path
  because CRLF is the *only* multi-scalar ASCII cluster; grapheme-safety becomes an
  unconditional invariant provable by a fuzz test.
- **Cons:** still no sub-word tokenization for scripts without spaces (CJK = one run per
  whitespace-delimited chunk); punctuation stays glued (`dog.` is one token → bare-word
  `dog` won't *exact*-match, only substring).

**Option B — UAX-29 word segmentation** (`unicode-segmentation::split_word_bounds`).
- **Pros:** linguistically correct boundaries; splits `dog.`→`dog`,`.` (helps exact-match);
  cluster-safe for free; handles CJK per-ideograph.
- **Cons:** **shreds the values we deliberately keep whole** — `ABC-123`→`ABC`,`-`,`123`,
  `$24.99`→`$`,`24.99`, URLs into ~11 tokens. Directly against the stated goal, so it can't be
  the default. Could be offered later as an opt-in *finer* vocabulary.

**Option C — status quo** (whitespace runs, walked by scalar).
- **Pros:** zero work.
- **Cons:** the CRLF/combining splits remain; every Windows-flavored page hits it.

> Leaning: **A**, with B kept in a back pocket as an optional finer mode. `unicode-segmentation`
> graduates from optional (grapheme-feature-only) to a regular dep — small, pure Rust, zero
> transitive deps.

---

## Decision 2 — cleanup policy for the monitoring diff

*Whether the word diff runs semantic cleanup.*

**Option A — no cleanup (token-atomic, recommended default).**
Each changed token stays a separate `Diff`.
- **Pros:** every changed token is individually matchable (best for exact-token conditions);
  renders as clean per-word highlight; also *faster* (skips the char-rediff `word_mode` does
  today). In the price/stock example it renders *better* than the merged form.
- **Cons:** the `Item 1 → First Item` reorder degrades to `[-Item-]{+First+} [-1-]{+Item+}`
  (Item shown as both removed and added → a naive "Item removed?" condition false-positives).

**Option B — always run semantic cleanup (merged/readable).**
- **Pros:** rescues the reorder case; coarser, "this region changed" display.
- **Cons:** merges adjacent changed tokens into phrases (`[-$19.99 — In stock-]{+…+}`), which
  *hurts* exact-token matching; slower.

**Option C — caller chooses.** Default to no-cleanup; expose cleanup as one extra call.
- **Pros:** both audiences served; matches how the default mode already works (caller runs
  their own cleanup passes).
- **Cons:** one more thing the consumer has to know about.

> Leaning: **C with an A default** — token-atomic out of the box, `diff_cleanup_semantic`
> available for callers who want the coarse view.

---

## Decision 3 — the public API shape

*What the consumer types.* The consumer only ever touches `Vec<Diff>` (`{ operation: -1|0|1,
text: String }`); the question is the entry point.

**Option A — reuse the existing `word_mode` flag.**
```rust
let mut dmp = Dmp::new();
dmp.word_mode = true;
let diffs = dmp.diff_main(old, new, true);
```
- **Pros:** no new surface; the flag already exists.
- **Cons:** mutable-field flip is a bit clumsy; the `checklines` arg is irrelevant here.

**Option B — add `dmp.diff_words(old, new) -> Vec<Diff>`.**
- **Pros:** reads cleanly; drops the flag-flip and the irrelevant arg; a natural home for the
  right defaults (token-atomic, grapheme-safe).
- **Cons:** one more public method to keep.

**Option C — B plus token helpers** (`added_tokens`/`removed_tokens` on `&[Diff]`).
- **Pros:** turnkey for the common monitoring path.
- **Cons:** the moment we ship `contains_added(keyword)` we bake in one matching model and one
  normalization policy — that belongs in the sieve, not the differ. Stop at the two neutral
  helpers.

> Leaning: **B** (add `diff_words`), plus the two token-splitting helpers from C, and nothing
> more opinionated than that.

---

## How the consumer uses it (worked, real output)

For `old = "Price: $19.99 — In stock"`, `new = "Price: $24.99 — Sold out"`, token-atomic mode
gives:

```
Price: [-$19.99-]{+$24.99+} — [-In-]{+Sold+} [-stock-]{+out+}
```

- **Render / highlight:** iterate, wrap `operation ±1`.
- **Exact-token condition** ("was word X added?"): collect tokens **per edit region** and match.
  ```rust
  let added: Vec<String> = diffs.iter().filter(|d| d.operation == 1)
      .flat_map(|d| d.text.split_whitespace()).map(str::to_string).collect();
  // → ["$24.99", "Sold", "out"]      added.iter().any(|t| t == "$24.99")  // true
  ```
- **Phrase / substring condition** ("did *Sold out* appear?"): test the whole reconstructed
  sides — `new.contains(x) && !old.contains(x)`. (This one needs only the two raw strings; the
  diff earns its keep for rendering and *token-level* add/remove.)

**Gotcha to bake into docs/tests:** do **not** concatenate all inserts across the diff. They're
positionally separated by unchanged text, so a global concat fuses `$24.99` + `Sold` + `out`
into `"$24.99Soldout"` and every condition silently returns false. Process each edit region on
its own; use the reconstructed side for phrases.

---

## Open questions (need product answers before coding)

1. **Do Distill's keyword conditions match exact tokens or substrings?**
   - Exact-token → favors *no cleanup* (Decision 2A) and per-region token extraction.
   - Substring → phrases matched on the whole new side; cleanup is then cosmetic.
   This decides Decision 2 and how we document the consumer pattern.
2. **Should the shipped mode run semantic cleanup by default, or leave it to the caller?**
   (Decision 2, A vs C.)

---

## Known limits we're choosing to leave (document, don't fix silently)

- **CJK / no-space scripts:** a whitespace-delimited run is one token; no per-word CJK. Real
  per-word CJK needs UAX-29 (Option 1B) or dictionary segmentation — out of scope for v1.
- **Unicode normalization:** `café` (NFC) and `café` (NFD) are different tokens. Normalization
  belongs upstream in the monitoring pipeline, not silently inside the differ.
- **Id-space exhaustion:** at ~1.11M unique tokens the placeholder space genuinely ends. Text1
  has a swallow-the-rest escape hatch; a second text exhausting after it still fails — now as a
  labeled `expect`, not an anonymous `unwrap`. ~20× beyond any realistic input; left as
  documented fail-fast.

---

## Rough implementation order when we pick this up

1. Grapheme-safe tokenizer: byte-path CRLF peek + char-path cluster walk in the arena word
   munger; add the "token boundaries ⊆ grapheme boundaries" fuzz/property test.
2. Redefine `word_mode` (or the new `diff_words`) as token-atomic — drop the char rediff.
3. Entry point per Decision 3B + the two token helpers.
4. Wire the example set above into `tests/word_mode.rs` as the acceptance suite.
5. Re-run `vs_similar` and the criterion suite to confirm no default-path regression.
