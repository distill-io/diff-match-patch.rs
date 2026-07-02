//! Regression tests for two historical bugs fixed during the rewrite (see the
//! plan's intentional-change ledger). These assert the CORRECT behavior and
//! must stay green permanently.

use diff_match_patch::{Diff, Dmp};

/// Two large multi-line texts sharing big common blocks interleaved with small
/// distinct lines. Both sides exceed 100 chars so `checklines = true` takes the
/// line-mode path. A fully-refined diff keeps the common blocks as equalities;
/// a genuinely timed-out diff must be cruder (fewer equalities).
fn interleaved_blocks() -> (String, String) {
    let mut t1 = String::from("HEAD1\n");
    let mut t2 = String::from("HEAD2\n");
    for b in 0..12 {
        for l in 0..8 {
            let line = format!("common-block{b}-line{l}\n");
            t1.push_str(&line);
            t2.push_str(&line);
        }
        t1.push_str(&format!("aaa-distinct-{b}\n"));
        t2.push_str(&format!("bbb-distinct-{b}\n"));
    }
    t1.push_str("TAIL1\n");
    t2.push_str("TAIL2\n");
    (t1, t2)
}

fn equality_count(diffs: &[Diff]) -> usize {
    diffs.iter().filter(|d| d.operation == 0).count()
}

/// Rebuild (text1, text2) from a diff: equalities+deletions form text1,
/// equalities+insertions form text2. Any valid diff must reconstruct its inputs.
fn rebuild(diffs: &[Diff]) -> (String, String) {
    let mut t1 = String::new();
    let mut t2 = String::new();
    for d in diffs {
        if d.operation != 1 {
            t1 += &d.text;
        }
        if d.operation != -1 {
            t2 += &d.text;
        }
    }
    (t1, t2)
}

/// Regression test for bug #1: the line-mode path historically dropped the
/// caller's deadline (a fresh `Dmp::new()` with `diff_timeout: None` ran the
/// line-level diff), so a zero-budget diff refined exactly as if no timeout
/// were set.
///
/// Timeout contract: `None` = no deadline; `Some(0.0)` = zero budget, give up
/// immediately. (The JS reference instead treats `Diff_Timeout <= 0` as "no
/// deadline" — js:91 sets the deadline to MAX_VALUE. The crate's Option-based
/// contract must NOT adopt that mapping; `None` already expresses "no
/// deadline".)
#[test]
fn bug1_line_mode_respects_diff_timeout() {
    let (t1, t2) = interleaved_blocks();

    let mut refined = Dmp::new();
    refined.diff_timeout = None;
    let diffs_refined = refined.diff_main(&t1, &t2, true);

    let mut zero_budget = Dmp::new();
    zero_budget.diff_timeout = Some(0.0);
    let diffs_zero = zero_budget.diff_main(&t1, &t2, true);

    // Validity: whatever the refinement level, a diff must reconstruct its inputs.
    assert_eq!(rebuild(&diffs_refined), (t1.clone(), t2.clone()));
    assert_eq!(rebuild(&diffs_zero), (t1.clone(), t2.clone()));

    let eq_no_timeout = equality_count(&diffs_refined);
    let eq_zero_timeout = equality_count(&diffs_zero);
    assert_eq!(eq_no_timeout, 15);
    // A zero budget must make every level of the diff give up immediately; the
    // only surviving equalities are the char-level common prefix ("HEAD") and
    // suffix ("\n") trimmed before the deadline is consulted. Deterministic:
    // the pre-expired deadline fires on the first `Instant::now() >= deadline`
    // check, with no clock race.
    assert_eq!(eq_zero_timeout, 2);
}

/// Regression test for bug #2: `patch_add_padding` indexed `diffs[0]` one line
/// before its `is_empty()` guard, so a header-only patch (empty diffs) panicked
/// inside `patch_apply` instead of applying as a no-op.
#[test]
fn bug2_patch_apply_handles_header_only_patch() {
    let mut dmp = Dmp::new();
    let mut patches = dmp.patch_from_text("@@ -1,0 +1,0 @@\n".to_string());
    assert_eq!(patches.len(), 1);
    let (patched, results) = dmp.patch_apply(&mut patches, "hello world");
    let patched: String = patched.into_iter().collect();
    assert_eq!(patched, "hello world");
    assert_eq!(results, vec![true]);
}

/// The last-diff twin of bug #2: `patch_add_padding` also indexed
/// `diffs[diffs.len() - 1]` before that block's guard. A single header-only
/// patch cannot reach it (the first block's inserted padding makes diffs
/// non-empty), so this pins the multi-hunk case where only the LAST hunk is
/// header-only. Expected output verified identical to the JS oracle.
#[test]
fn bug2_patch_apply_handles_trailing_header_only_hunk() {
    let mut dmp = Dmp::new();
    let mut patches =
        dmp.patch_from_text("@@ -1,8 +1,9 @@\n abcd\n+X\n efgh\n@@ -20,0 +20,0 @@\n".to_string());
    assert_eq!(patches.len(), 2);
    let (patched, results) = dmp.patch_apply(&mut patches, "abcdefgh--------------------");
    let patched: String = patched.into_iter().collect();
    assert_eq!(patched, "abcdXefgh--------------------");
    assert_eq!(results, vec![true, true]);
}
