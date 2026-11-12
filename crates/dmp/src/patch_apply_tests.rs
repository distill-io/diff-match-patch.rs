// The patch-applying loop against the loop it replaced.

use super::super::{apply_exact_run, diff_text1_chars, diff_text2_chars};
use super::{random_text, Lcg};
use crate::types::{max, min, Diff, Dmp, Patch};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Which paths of the applying loop a run took. While every patch so far was
/// a perfect match that changed nothing before the previous change ended, a
/// patch either is one more (`in_order`) or ends that run for the stated reason.
#[derive(Debug, Default, PartialEq)]
struct ApplyHits {
    in_order: usize,
    in_order_moved: usize,
    in_order_empty_pattern: usize,
    in_order_overlap: usize,
    whole_text: usize,
    stayed_in_order: usize,
    left_after_in_order: usize,
    left_oversized: usize,
    left_empty_text: usize,
    left_behind: usize,
    left_not_exact: usize,
    failed: usize,
    fuzzy_insertion: usize,
    fuzzy_deletion: usize,
    panicked: usize,
}

/// The loop that spliced every replacement into the whole text.
fn apply_by_splicing(
    dmp: &mut Dmp,
    patches: &mut Vec<Patch>,
    source_text: &str,
    hits: &mut ApplyHits,
) -> (Vec<char>, Vec<bool>) {
    if patches.is_empty() {
        return (source_text.chars().collect(), vec![]);
    }
    let mut patches_copy: Vec<Patch> = dmp.patch_deep_copy(patches);
    let null_padding: Vec<char> = dmp.patch_add_padding(&mut patches_copy);
    let mut text = null_padding.clone();
    text.extend(source_text.chars());
    text.extend(&null_padding);
    dmp.patch_splitmax(&mut patches_copy);
    let mut delta: i32 = 0;
    let mut results: Vec<bool> = vec![false; patches_copy.len()];
    let (_, fast_run) = apply_exact_run(
        dmp.match_maxbits,
        &patches_copy,
        &text,
        &mut results.clone(),
        &mut 0,
    );
    let mut run = 0;
    // End of the previous change while the run of in-order patches lasts.
    let mut cursor = Some(0);
    let mut kept_end = 0;
    for x in 0..patches_copy.len() {
        let expected_loc: i32 = patches_copy[x].start2 + delta;
        let text1: Vec<char> = diff_text1_chars(&patches_copy[x].diffs);
        if let Some(at) = cursor {
            let m = text1.len();
            let text2 = diff_text2_chars(&patches_copy[x].diffs);
            let same = text1.iter().zip(&text2).take_while(|(a, b)| a == b).count();
            kept_end = text1[same..]
                .iter()
                .rev()
                .zip(text2[same..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            let loc = max(0, expected_loc) as usize;
            hits.whole_text += (text1 == text) as usize;
            let left = if m as i32 > dmp.match_maxbits {
                Some(&mut hits.left_oversized)
            } else if text.is_empty() && m > 0 {
                Some(&mut hits.left_empty_text)
            } else if loc + same < at {
                Some(&mut hits.left_behind)
            } else if loc + m > text.len() || text[loc..loc + m] != text1[..] {
                Some(&mut hits.left_not_exact)
            } else {
                None
            };
            match left {
                Some(count) => {
                    *count += 1;
                    hits.left_after_in_order += (x > 0) as usize;
                    cursor = None;
                }
                None => {
                    hits.in_order += 1;
                    run += 1;
                    hits.in_order_moved += (loc as i32 != expected_loc) as usize;
                    hits.in_order_empty_pattern += (m == 0) as usize;
                    hits.in_order_overlap += (loc < at) as usize;
                }
            }
        }
        let mut start_loc: i32;
        let mut end_loc = -1;
        if text1.len() as i32 > dmp.match_maxbits {
            start_loc = crate::match_::match_chars(
                dmp,
                &text,
                &text1[..dmp.match_maxbits as usize],
                expected_loc,
            );
            if start_loc != -1 {
                end_loc = crate::match_::match_chars(
                    dmp,
                    &text,
                    &text1[text1.len() - dmp.match_maxbits as usize..],
                    expected_loc + text1.len() as i32 - dmp.match_maxbits,
                );
                if end_loc == -1 || start_loc >= end_loc {
                    start_loc = -1;
                }
            }
        } else {
            start_loc = crate::match_::match_chars(dmp, &text, &text1, expected_loc);
        }
        if start_loc == -1 {
            hits.failed += 1;
            results[x] = false;
            delta -= patches_copy[x].length2 - patches_copy[x].length1;
        } else {
            results[x] = true;
            delta = start_loc - expected_loc;
            let mut end_index: usize;
            if end_loc == -1 {
                end_index = start_loc as usize + text1.len();
            } else {
                end_index = (end_loc + dmp.match_maxbits) as usize;
            }
            end_index = std::cmp::min(text.len(), end_index);
            if text1[..] == text[start_loc as usize..end_index] {
                let replacement = diff_text2_chars(&patches_copy[x].diffs);
                if let Some(at) = cursor.as_mut() {
                    *at = start_loc as usize + replacement.len() - kept_end;
                }
                text.splice(
                    start_loc as usize..start_loc as usize + text1.len(),
                    replacement,
                );
            } else {
                let mut diffs: Vec<Diff> = crate::diff::diff_main_chars(
                    dmp,
                    &text1,
                    &text[start_loc as usize..end_index],
                    false,
                );
                if text1.len() as i32 > dmp.match_maxbits
                    && (dmp.diff_levenshtein(&diffs) as f32 / (text1.len() as f32)
                        > dmp.patch_delete_threshold)
                {
                    hits.failed += 1;
                    results[x] = false;
                } else {
                    dmp.diff_cleanup_semantic_lossless(&mut diffs);
                    let mut index1: i32 = 0;
                    for y in 0..patches_copy[x].diffs.len() {
                        let op = patches_copy[x].diffs[y].operation;
                        let mod_len = patches_copy[x].diffs[y].text.chars().count() as i32;
                        if op != 0 {
                            let index2: i32 = dmp.diff_xindex(&diffs, index1);
                            if op == 1 {
                                hits.fuzzy_insertion += 1;
                                let at = (start_loc + index2) as usize;
                                text.splice(at..at, patches_copy[x].diffs[y].text.chars());
                            } else if op == -1 {
                                hits.fuzzy_deletion += 1;
                                let from = (start_loc + index2) as usize;
                                let until = (start_loc + dmp.diff_xindex(&diffs, index1 + mod_len))
                                    as usize;
                                text.splice(from..until, std::iter::empty());
                            }
                        }
                        if op != -1 {
                            index1 += mod_len;
                        }
                    }
                }
            }
        }
    }
    hits.stayed_in_order += cursor.is_some() as usize;
    assert_eq!(fast_run, run);
    text.drain(..null_padding.len());
    text.truncate(text.len() - null_padding.len());
    (text, results)
}

// The first text with a few random edits; sometimes one deletion longer
// than any pattern.
fn edited(rng: &mut Lcg, text: &str) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    for _ in 0..rng.below(6) {
        let at = rng.below(chars.len() as u64 + 1);
        match rng.below(4) {
            0 => {
                let len = if rng.below(4) == 0 {
                    40
                } else {
                    1 + rng.below(3)
                };
                chars.drain(at..min(chars.len() as i32, (at + len) as i32) as usize);
            }
            1 if at < chars.len() => chars[at] = 'c',
            _ => {
                let len = 1 + rng.below(4);
                let insert: Vec<char> = random_text(rng, len).chars().collect();
                chars.splice(at..at, insert);
            }
        }
    }
    chars.into_iter().collect()
}

// The patches out of order, with one repeated, or with one patch's start
// moved before or past the text.
fn shuffled(rng: &mut Lcg, patches: &[Patch], text_len: usize) -> Vec<Patch> {
    let mut patches = patches.to_vec();
    if patches.is_empty() {
        return patches;
    }
    let at = rng.below(patches.len() as u64);
    match rng.below(4) {
        0 => patches.reverse(),
        1 => patches.insert(at, patches[at].clone()),
        2 => patches[at].start2 = -1 - rng.below(20) as i32,
        _ => patches[at].start2 = (text_len + 1 + rng.below(20)) as i32,
    }
    patches
}

#[test]
fn in_order_apply_matches_splicing() {
    let mut rng = Lcg(5);
    let mut hits = ApplyHits::default();
    let mut applies = 0;
    // (match_maxbits, patch_margin)
    let settings = [(32, 4), (32, 0), (16, 2), (8, 1), (0, 4)];
    for round in 0..3_000 {
        let mut dmp = Dmp::new();
        (dmp.match_maxbits, dmp.patch_margin) = settings[round % settings.len()];
        let len = rng.below(301);
        let text1 = random_text(&mut rng, len);
        let text2 = edited(&mut rng, &text1);
        let patches = dmp.patch_make1(&text1, &text2);
        let prepended = 1 + rng.below(3);
        let mut pairs = vec![
            (patches.clone(), text1.clone()),
            (patches.clone(), random_text(&mut rng, prepended) + &text1),
            (patches.clone(), edited(&mut rng, &text1)),
            (
                shuffled(&mut rng, &patches, text1.chars().count()),
                text1.clone(),
            ),
            (patches.clone(), String::new()),
        ];
        if !patches.is_empty() {
            let one = &patches[rng.below(patches.len() as u64)];
            let whole: String = diff_text1_chars(&one.diffs).into_iter().collect();
            pairs.push((patches.clone(), whole));
        }
        for (patches, text) in pairs {
            applies += 1;
            let expected = catch_unwind(AssertUnwindSafe(|| {
                apply_by_splicing(&mut dmp, &mut patches.clone(), &text, &mut hits)
            }))
            .ok();
            let actual = catch_unwind(AssertUnwindSafe(|| {
                dmp.patch_apply(&mut patches.clone(), &text)
            }))
            .ok();
            hits.panicked += expected.is_none() as usize;
            assert_eq!(
                actual,
                expected,
                "{:?} {text:?} {patches:?}",
                settings[round % settings.len()]
            );
        }
    }
    assert_eq!(applies, 17512);
    assert_eq!(
        hits,
        ApplyHits {
            in_order: 10006,
            in_order_moved: 47,
            in_order_empty_pattern: 1968,
            in_order_overlap: 198,
            whole_text: 636,
            stayed_in_order: 2986,
            left_after_in_order: 1931,
            left_oversized: 3354,
            left_empty_text: 333,
            left_behind: 728,
            left_not_exact: 7671,
            failed: 9332,
            fuzzy_insertion: 3276,
            fuzzy_deletion: 2603,
            panicked: 1,
        }
    );
}
