//! Pinned deviations of the CURRENT crate from the canonical oracle (googlediff).
//!
//! Each test asserts today's actual behavior so any accidental change during the
//! staged rewrite fails loudly; the stage that intentionally changes the behavior
//! flips the assertion to the oracle's value. This is the "honest baseline": every
//! divergence is recorded, none is silently blessed.

use diff_match_patch::{Diff, Dmp};

/// The delta encoder escapes '%' as `%25`, matching the oracle (JS `encodeURI`).
/// The pre-rewrite encoder left it bare — fixed in the Stage-4 delta rewrite.
#[test]
fn delta_percent_escaped() {
    let mut d = Dmp::new();
    let mut diffs = vec![Diff::new(
        1,
        "tag <x> \"q\" {y} 100% back\\slash".to_string(),
    )];
    assert_eq!(
        d.diff_todelta(&mut diffs),
        "+tag %3Cx%3E %22q%22 %7By%7D 100%25 back%5Cslash",
    );
}

/// Zero-length hunks keep their raw start and round-trip unchanged, matching
/// the oracle. The pre-rewrite parser decremented the start unconditionally
/// and re-serialized "@@ -5,0 +5,0 @@" as "@@ -5 +5 @@" (semantically length
/// 1) — fixed in the Stage-4 patch rewrite.
#[test]
fn zero_length_hunk_roundtrips() {
    let mut d = Dmp::new();
    let input = "@@ -5,0 +5,0 @@\n".to_string();
    let mut patches = d.patch_from_text(input.clone());
    assert_eq!(d.patch_to_text(&mut patches), input);
}

/// An equality of exactly `2 * patch_margin` (8) chars between two edit groups
/// is absorbed into the patch as context WITHOUT closing it, matching the
/// oracle's if/else-if. The pre-rewrite port ran both branches (absorb AND
/// close), double-counting the equality and emitting five hunks with hunk 5
/// starting 21 chars inside hunk 4 with text2-contaminated context. The pin is
/// byte equality with the oracle's four-hunk output (adjacent hunks may
/// legitimately share up to a margin of rolling context, so a no-overlap
/// assertion would be wrong).
#[test]
fn patch_make_absorbs_margin_sized_equality() {
    let t1 = "alpha line one\nbeta line two\ngamma line three\ndelta line four\nepsilon line five\nzeta line six\neta line seven\ntheta line eight\n";
    let t2 = "alpha line one\nbeta line 2 changed\ngamma line three\ndelta line four\nnew line inserted here\nepsilon line five\nzeta line six\ntheta line eight\niota line nine\n";
    let mut d = Dmp::new();
    let mut patches = d.patch_make1(t1, t2);
    assert_eq!(patches.len(), 4);
    assert_eq!(
        d.patch_to_text(&mut patches),
        "@@ -22,11 +22,17 @@\n ine \n-two\n+2 changed\n %0Agam\n@@ -61,16 +61,39 @@\n ne four%0A\n+new line inserted here%0A\n epsilon \n@@ -116,16 +116,18 @@\n ine six%0A\n+th\n eta line\n@@ -131,27 +131,25 @@\n ine \n-seven%0Athe\n+eight%0Aio\n ta line \n-eight\n+nine\n %0A\n",
    );
}

/// `Dmp::new()` defaults `edit_cost` to 0 (upstream diff-match-patch defaults to 4),
/// which makes `diff_cleanup_efficiency` a structural no-op. The golden corpus is
/// generated with `Diff_EditCost = 0` to match; if this default ever changes, the
/// corpus and this pin must change together.
#[test]
fn edit_cost_defaults_to_zero() {
    assert_eq!(Dmp::new().edit_cost, 0);
}

/// The half-match speedup is COST-aware, not just consent-aware — a deliberate
/// extension over upstream, where any non-zero `Diff_Timeout` enables half-match
/// even on inputs the optimal search solves in microseconds. Here half-match
/// additionally requires the combined input length to reach
/// `half_match_min_len` (tokens); below it, a deadline caller still gets the
/// OPTIMAL diff. `half_match_min_len = 0` restores upstream semantics.
#[test]
fn small_input_with_deadline_still_gets_the_optimal_diff() {
    // The repeated-run shift, word-tokenized (each word packs into one char, so
    // half-match splits on the repeated middle and produces substitution
    // confetti on both flanks); the optimal diff is one deletion at the front
    // and one insertion at the back. Char-level diff_main of the same strings
    // happens to be optimal either way — the token path is the honest fire.
    let word_diff = |dmp: &mut Dmp| -> Vec<(i32, String)> {
        let (chars1, chars2, words) =
            dmp.diff_words_tochars(&"word1 noop noop noop".to_string(), &"noop noop noop word1".to_string());
        let mut diffs = dmp.diff_main(&chars1, &chars2, false);
        dmp.diff_chars_tolines(&mut diffs, &words);
        diffs.into_iter().map(|d| (d.operation, d.text)).collect()
    };
    let mut optimal = Dmp {
        diff_timeout: None,
        ..Dmp::new()
    };
    let want = word_diff(&mut optimal);
    assert_eq!(
        want,
        vec![
            (-1, "word1 ".to_string()),
            (0, "noop noop noop".to_string()),
            (1, " word1".to_string()),
        ],
        "sanity: the optimal shape this test defends"
    );
    let mut timed = Dmp::new(); // diff_timeout Some(1.0), default half_match_min_len
    assert_eq!(word_diff(&mut timed), want);
}

/// Boundary pair for `half_match_min_len`, plus the `0` = upstream-semantics
/// escape hatch. The word-packed inputs are 7 + 7 = 14 tokens: at a threshold
/// of exactly 14 half-match is allowed (the upstream confetti split); at 15 the
/// optimal path runs; at 0 upstream behavior applies regardless of size.
#[test]
fn half_match_min_len_boundary_and_upstream_escape_hatch() {
    let word_diff = |dmp: &mut Dmp| -> Vec<(i32, String)> {
        let (chars1, chars2, words) = dmp.diff_words_tochars(
            &"word1 noop noop noop".to_string(),
            &"noop noop noop word1".to_string(),
        );
        let mut diffs = dmp.diff_main(&chars1, &chars2, false);
        dmp.diff_chars_tolines(&mut diffs, &words);
        diffs.into_iter().map(|d| (d.operation, d.text)).collect()
    };
    let optimal = vec![
        (-1, "word1 ".to_string()),
        (0, "noop noop noop".to_string()),
        (1, " word1".to_string()),
    ];
    let half_match_split = vec![
        (-1, "word1".to_string()),
        (1, "noop".to_string()),
        (0, " noop noop ".to_string()),
        (-1, "noop".to_string()),
        (1, "word1".to_string()),
    ];

    let mut at = Dmp {
        half_match_min_len: 14,
        ..Dmp::new()
    };
    assert_eq!(word_diff(&mut at), half_match_split, "at the threshold");

    let mut below = Dmp {
        half_match_min_len: 15,
        ..Dmp::new()
    };
    assert_eq!(word_diff(&mut below), optimal, "one below the threshold");

    let mut upstream = Dmp {
        half_match_min_len: 0,
        ..Dmp::new()
    };
    assert_eq!(word_diff(&mut upstream), half_match_split, "0 = upstream");
}
