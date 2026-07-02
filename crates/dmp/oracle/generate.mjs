// Golden-reference generator: drives the vendored googlediff (Neil Fraser's original
// diff-match-patch, Apache-2.0, oracle/vendor/) to emit expected diff / cleanup /
// delta / patch outputs for an ASCII corpus. Regenerate with:  node oracle/generate.mjs
//
// Config parity with the Rust crate's Dmp::new():
//   Diff_Timeout = 0   — disables half_match AND the deadline (Rust: diff_timeout = None)
//   Diff_EditCost = 0  — Rust's default (upstream default is 4; deliberate crate
//                        deviation, pinned by tests/characterization.rs)
// All other oracle defaults (Match_Threshold .5, Match_Distance 1000, Patch_Margin 4,
// Match_MaxBits 32, Patch_DeleteThreshold .5) equal the Rust defaults.
//
// The corpus is ASCII-only: for ASCII/BMP text the oracle's UTF-16 counts equal the
// crate's Unicode-scalar counts, so googlediff is a true oracle here. Astral behavior
// is pinned separately by hand-authored tests (see the rewrite plan).

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));

const ORACLE_VERSION = 'googlediff@0.1.0';
// The vendored file assigns this['diff_match_patch'] itself; no export shim needed.
const sandbox = {};
vm.runInNewContext(
  readFileSync(join(here, 'vendor/diff_match_patch_uncompressed.js'), 'utf8'),
  sandbox,
);
const DMP = sandbox.diff_match_patch;

// [name, text1, text2, applyTo?]  — applyTo (optional) is the patch_apply target;
// when set and different from text1 it exercises the offset/fuzzy match path.
const CASES = [
  ['identical', 'hello world', 'hello world'],
  ['insert_word', 'the cat', 'the black cat'],
  ['delete_word', 'the black cat', 'the cat'],
  ['replace_word', 'I am the walrus', 'I am the eggman'],
  ['common_prefix_suffix', '1234xyz5678', '1234abc5678'],
  ['multiline', 'line one\nline two\nline three\n', 'line one\nline 2\nline three\n'],
  ['empty_to_text', '', 'brand new content'],
  ['text_to_empty', 'this is gone', ''],
  ['word_shuffle', 'the quick brown fox', 'the brown quick fox'],
  ['classic_pangram', 'The quick brown fox jumps over the lazy dog.', 'The quick brown cat jumps over the sleepy dog.'],
  ['delta_raw_chars', '', "a+b=c & d/e?f #g,h;i:j@k$l!m~n*o'p(q)r"],
  ['delta_escaped_chars', '', 'tag <x> "q" {y} 100% back\\slash'],
  ['repeated', 'aaaaaaaaaa', 'aaaaabaaaaa'],
  ['prose_edit', 'It was the best of times, it was the worst of times.', 'It was the best of times, it was the age of wisdom.'],
  // Distinguishes Diff_EditCost 4 vs 0 — pins the crate's edit_cost=0 semantics.
  ['edit_cost_probe', 'abcxyzde', '12wxyz34'],
  // Both sides > 100 chars → diff_main(checklines=true) takes the line-mode path
  // (and its embedded cleanup_semantic + char-level rediff of replacement blocks).
  [
    'linemode_long',
    'alpha line one\nbeta line two\ngamma line three\ndelta line four\nepsilon line five\nzeta line six\neta line seven\ntheta line eight\n',
    'alpha line one\nbeta line 2 changed\ngamma line three\ndelta line four\nnew line inserted here\nepsilon line five\nzeta line six\ntheta line eight\niota line nine\n',
  ],
  // text1 is a substring of text2 → the containment (kmp) speedup branch.
  ['containment', 'brown fox jumps', 'The quick brown fox jumps over the lazy dog'],
  // A change region far larger than Match_MaxBits (32) → patch_splitmax actually
  // splits during patch_apply; patchResults length then exceeds the input patch count.
  [
    'splitmax_long',
    'Start anchor text. AAAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDDEEEEEEEEEEFFFFFFFFFF. End anchor text.',
    'Start anchor text. 111111111122222222223333333333444444444455555555556666666666777777. End anchor text.',
  ],
  // applyTo differs from text1 → match_main must locate hunks at shifted positions.
  [
    'fuzzy_apply',
    'The quick brown fox jumps over the lazy dog.',
    'The quick brown cat jumps over the sleepy dog.',
    'PREFIX ADDED. The quick brown fox jumps over the lazy dog.',
  ],
  // Tab and newline inside inserted text: %09 / %0A escaping — tab is the delta
  // format's own token separator, so this pins framing safety.
  ['tab_newline_insert', 'row:', 'row:\tcol1\tcol2\nrow2\tval'],
  // "@@ " inside body text: hunk-header lookalikes must survive the patch
  // text round-trip (the pre-rewrite parser split on every "@@ ").
  ['atat_in_text', 'keep a @@ b marker\nsecond line\n', 'keep a @@ B marker\nsecond line\nthird line\n'],
];

const cases = CASES.map(([name, text1, text2, applyTo]) => {
  for (const t of [text1, text2, applyTo ?? '']) {
    if (!/^[\x00-\x7F]*$/.test(t)) {
      throw new Error(`case '${name}': corpus must stay ASCII (oracle counts UTF-16)`);
    }
  }

  const dmp = new DMP();
  dmp.Diff_Timeout = 0;
  dmp.Diff_EditCost = 0;

  const diff = dmp.diff_main(text1, text2); // applies cleanup_merge internally
  const delta = dmp.diff_toDelta(diff);
  const diffSemantic = diff.map((d) => [d[0], d[1]]);
  dmp.diff_cleanupSemantic(diffSemantic);

  const patches = dmp.patch_make(text1, text2);
  const patchText = dmp.patch_toText(patches);
  const target = applyTo ?? text1;
  // patch_apply deep-copies its input, so reusing `patches` is safe.
  const [applied, results] = dmp.patch_apply(patches, target);

  return {
    name,
    text1,
    text2,
    applyTo: target,
    diff, // [[op, text], ...] with op in {-1,0,1}
    diffSemantic,
    delta,
    patchText,
    patchApplied: applied,
    patchResults: results,
  };
});

const out = { generator: `${ORACLE_VERSION} (Diff_Timeout=0, Diff_EditCost=0)`, cases };
const dir = join(here, '../tests/golden');
mkdirSync(dir, { recursive: true });
writeFileSync(join(dir, 'corpus.json'), JSON.stringify(out, null, 2) + '\n');
console.log(`wrote ${cases.length} cases to tests/golden/corpus.json`);
