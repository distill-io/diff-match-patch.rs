// The patch-building loop against the loop it replaced.

use super::super::add_context;
use super::{random_text, Lcg};
use crate::types::{max, min, Diff, Dmp, Patch};
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

/// Which paths of the context search a run took.
#[derive(Debug, Default, PartialEq)]
struct ContextHits {
    empty_text: usize,
    empty_pattern: usize,
    grown: usize,
    gave_up: usize,
    prefix: usize,
    suffix: usize,
}

/// The context search over one contiguous text, with the uniqueness check
/// spelled out as a count of matching windows.
fn add_context_contiguous(dmp: &Dmp, patch: &mut Patch, text: &[char], hits: &mut ContextHits) {
    if text.is_empty() {
        hits.empty_text += 1;
        return;
    }
    let mut pattern: Vec<char> =
        text[patch.start2 as usize..(patch.length1 as usize + patch.start2 as usize)].to_vec();
    let mut padding: i32 = 0;
    hits.empty_pattern += pattern.is_empty() as usize;
    let ambiguous = |text: &[char], pattern: &[char]| {
        if pattern.is_empty() {
            text.len() != 1
        } else {
            text.windows(pattern.len())
                .filter(|window| *window == pattern)
                .count()
                >= 2
        }
    };
    let mut rst = 0;
    while ambiguous(text, &pattern)
        && (pattern.len() as i32) < (dmp.match_maxbits - dmp.patch_margin * 2)
    {
        hits.grown += 1;
        padding += dmp.patch_margin;
        pattern = text[max(0, patch.start2 - padding) as usize
            ..min(text.len() as i32, patch.start2 + patch.length1 + padding) as usize]
            .to_vec();
        rst += 1;
        if rst > 5 {
            hits.gave_up += 1;
            break;
        }
    }
    padding += dmp.patch_margin;
    let prefix: String = text[max(0, patch.start2 - padding) as usize..patch.start2 as usize]
        .iter()
        .collect();
    let prefix_length = prefix.chars().count() as i32;
    if !prefix.is_empty() {
        hits.prefix += 1;
        patch.diffs.insert(0, Diff::new(0, prefix.clone()));
    }
    let suffix: String = text[(patch.start2 + patch.length1) as usize
        ..min(text.len() as i32, patch.start2 + patch.length1 + padding) as usize]
        .iter()
        .collect();
    let suffix_length = suffix.chars().count() as i32;
    if !suffix.is_empty() {
        hits.suffix += 1;
        patch.diffs.push(Diff::new(0, suffix));
    }
    patch.start1 -= prefix_length;
    patch.start2 -= prefix_length;
    patch.length1 += prefix_length + suffix_length;
    patch.length2 += prefix_length + suffix_length;
}

#[test]
fn split_context_matches_contiguous() {
    let mut rng = Lcg(2);
    let mut hits = ContextHits::default();
    let (mut straddled, mut panicked) = (0, 0);
    // (match_maxbits, patch_margin)
    let settings = [(32, 4), (32, 0), (16, 2), (8, 1), (0, 4), (64, 3)];
    for round in 0..4_000 {
        let mut dmp = Dmp::new();
        (dmp.match_maxbits, dmp.patch_margin) = settings[round % settings.len()];
        let len = [0, 1, 2, 9, 30, 60][rng.below(6)];
        let text: Vec<char> = random_text(&mut rng, len).chars().collect();
        // The patched span mostly lies in the text; some start before it or
        // run past its end.
        let start =
            rng.below(len as u64 + 2) as i32 - (rng.below(25) == 0) as i32 * (len as i32 + 2);
        let length = [0, 0, 1, 2, 5, 12][rng.below(6)];
        let patch = Patch::new(
            vec![Diff::new(1, "x".to_string())],
            start,
            start,
            length,
            length + 1,
        );
        let mut expected = patch.clone();
        let expected_ok = catch_unwind(AssertUnwindSafe(|| {
            add_context_contiguous(&dmp, &mut expected, &text, &mut hits)
        }))
        .is_ok();
        let mut whole = patch.clone();
        let whole_ok = catch_unwind(AssertUnwindSafe(|| {
            dmp.patch_add_context(&mut whole, &mut text.clone())
        }))
        .is_ok();
        assert_eq!((whole_ok, &whole), (expected_ok, &expected), "{text:?}");
        let split = rng.below(len as u64 + 1) as i32;
        let (head, tail) = text.split_at(split as usize);
        let mut actual = patch.clone();
        let actual_ok = catch_unwind(AssertUnwindSafe(|| {
            add_context(&dmp, &mut actual, head, tail)
        }))
        .is_ok();
        assert_eq!(
            (actual_ok, &actual),
            (expected_ok, &expected),
            "{head:?} {tail:?}"
        );
        panicked += !expected_ok as usize;
        straddled += (expected_ok
            && expected.start2 < split
            && split < expected.start2 + expected.length1) as usize;
    }
    assert_eq!(
        hits,
        ContextHits {
            empty_text: 681,
            empty_pattern: 919,
            grown: 2368,
            gave_up: 221,
            prefix: 1419,
            suffix: 1420,
        }
    );
    assert_eq!((straddled, panicked), (504, 1322));
}

/// The loop the edit cursor replaced: it spliced every edit into a copy of
/// the whole text and cloned that copy for every patch.
fn make4_by_splicing(
    dmp: &mut Dmp,
    text1: &str,
    diffs: &[Diff],
    hits: &mut MakeHits,
) -> Vec<Patch> {
    let mut context = ContextHits::default();
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
            add_context_contiguous(dmp, &mut patch, &prepatch, &mut context);
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
        add_context_contiguous(dmp, &mut patch, &prepatch, &mut context);
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
