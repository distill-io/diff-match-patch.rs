// The patch-building loop against the loop it replaced.

use super::{random_text, Lcg};
use crate::types::{Diff, Dmp, Patch};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Which paths of the patch-building loop a run took.
#[derive(Debug, Default, PartialEq)]
struct MakeHits {
    insertion: usize,
    insertion_past_end: usize,
    deletion: usize,
    deletion_past_end: usize,
    small_equality: usize,
    closed: usize,
    leftover: usize,
    panicked: usize,
}

/// The loop the edit cursor replaced: it spliced every edit into a copy of
/// the whole text and cloned that copy for every patch.
fn make4_by_splicing(
    dmp: &mut Dmp,
    text1: &str,
    diffs: &[Diff],
    hits: &mut MakeHits,
) -> Vec<Patch> {
    let mut patches: Vec<Patch> = vec![];
    if diffs.is_empty() {
        return patches;
    }
    let mut patch: Patch = Patch::new(vec![], 0, 0, 0, 0);
    let mut char_count1 = 0;
    let mut char_count2 = 0;
    let mut prepatch: Vec<char> = text1.chars().collect();
    let mut postpatch: Vec<char> = prepatch.clone();
    for i in 0..diffs.len() {
        let temp1: &Vec<char> = &(diffs[i].text.chars().collect());
        if patch.diffs.is_empty() && diffs[i].operation != 0 {
            patch.start1 = char_count1;
            patch.start2 = char_count2;
        }
        if diffs[i].operation == 1 {
            hits.insertion += 1;
            patch
                .diffs
                .push(Diff::new(diffs[i].operation, diffs[i].text.clone()));
            patch.length2 += temp1.len() as i32;
            hits.insertion_past_end += (char_count2 as usize > postpatch.len()) as usize;
            postpatch.splice(
                char_count2 as usize..char_count2 as usize,
                temp1.iter().copied(),
            );
        } else if diffs[i].operation == -1 {
            hits.deletion += 1;
            patch
                .diffs
                .push(Diff::new(diffs[i].operation, diffs[i].text.clone()));
            patch.length1 += temp1.len() as i32;
            hits.deletion_past_end +=
                (char_count2 as usize + temp1.len() > postpatch.len()) as usize;
            postpatch.splice(
                char_count2 as usize..char_count2 as usize + temp1.len(),
                std::iter::empty(),
            );
        } else if temp1.len() as i32 <= dmp.patch_margin * 2
            && !patch.diffs.is_empty()
            && i != diffs.len() - 1
        {
            hits.small_equality += 1;
            patch
                .diffs
                .push(Diff::new(diffs[i].operation, diffs[i].text.clone()));
            patch.length1 += temp1.len() as i32;
            patch.length2 += temp1.len() as i32;
        } else if temp1.len() as i32 >= 2 * dmp.patch_margin && !patch.diffs.is_empty() {
            hits.closed += 1;
            dmp.patch_add_context(&mut patch, &mut prepatch);
            patches.push(patch);
            patch = Patch::new(vec![], 0, 0, 0, 0);
            prepatch = postpatch.clone();
            char_count1 = char_count2;
        }
        if diffs[i].operation != 1 {
            char_count1 += temp1.len() as i32;
        }
        if diffs[i].operation != -1 {
            char_count2 += temp1.len() as i32;
        }
    }
    if !patch.diffs.is_empty() {
        hits.leftover += 1;
        dmp.patch_add_context(&mut patch, &mut prepatch);
        patches.push(patch);
    }
    patches
}

// A diff list and a first text for it. The text is the one the diffs
// describe, that text cut short so the diffs run past its end, or that text
// with more after it. One operation is outside -1..=1.
fn random_edit_script(rng: &mut Lcg) -> (String, Vec<Diff>) {
    let diffs: Vec<Diff> = (0..rng.below(14))
        .map(|_| {
            let operation = [0, 0, 0, -1, 1, 1, -1, 2][rng.below(8)];
            let len = [0, 1, 2, 5, 8, 9, 20, 60][rng.below(8)];
            Diff::new(operation, random_text(rng, len))
        })
        .collect();
    let described: Vec<char> = diffs
        .iter()
        .filter(|diff| diff.operation != 1)
        .flat_map(|diff| diff.text.chars())
        .collect();
    let text1 = match rng.below(6) {
        0 => described[..rng.below(described.len() as u64 + 1)]
            .iter()
            .collect(),
        1 => described.iter().collect::<String>() + &random_text(rng, 30),
        _ => described.iter().collect(),
    };
    (text1, diffs)
}

#[test]
fn edit_cursor_matches_splicing() {
    let mut rng = Lcg(1);
    let mut hits = MakeHits::default();
    // (match_maxbits, patch_margin)
    let settings = [(32, 4), (32, 0), (16, 2), (8, 1), (0, 4), (64, 9)];
    for round in 0..3_000 {
        let mut dmp = Dmp::new();
        (dmp.match_maxbits, dmp.patch_margin) = settings[round % settings.len()];
        let (text1, diffs) = random_edit_script(&mut rng);
        let expected = catch_unwind(AssertUnwindSafe(|| {
            make4_by_splicing(&mut dmp, &text1, &diffs, &mut hits)
        }))
        .ok();
        let actual = catch_unwind(AssertUnwindSafe(|| {
            dmp.patch_make4(&text1, &mut diffs.clone())
        }))
        .ok();
        hits.panicked += expected.is_none() as usize;
        assert_eq!(
            actual,
            expected,
            "{:?} {text1:?} {diffs:?}",
            settings[round % settings.len()]
        );
    }
    assert_eq!(
        hits,
        MakeHits {
            insertion: 4568,
            insertion_past_end: 100,
            deletion: 4660,
            deletion_past_end: 197,
            small_equality: 2123,
            closed: 2588,
            leftover: 1517,
            panicked: 302,
        }
    );
}
