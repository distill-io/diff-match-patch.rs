// The linear patch loops against the loops they replaced.

use super::*;

#[path = "patch_make_tests.rs"]
mod make;

#[path = "patch_apply_tests.rs"]
mod apply;

struct Lcg(u64);
impl Lcg {
    fn below(&mut self, bound: u64) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) % bound) as usize
    }
}

fn random_text(rng: &mut Lcg, len: usize) -> String {
    (0..len)
        .map(|_| ['a', 'b', 'é', '😀'][rng.below(4)])
        .collect()
}

/// Which paths of the splitting loop a run took.
#[derive(Debug, Default, PartialEq)]
struct SplitHits {
    kept: usize,
    split: usize,
    dropped: usize,
    whole_deletion: usize,
    partial: usize,
    merged_context: usize,
    pushed_context: usize,
}

/// The loop the linear split replaced: it removed each big patch from the
/// list, shifted its diffs down one at a time, and rebuilt its remaining
/// text for every piece.
fn splitmax_by_shifting(dmp: &mut Dmp, patches: &mut Vec<Patch>, hits: &mut SplitHits) {
    let patch_size = dmp.match_maxbits;
    if patch_size == 0 {
        return;
    }
    let mut x: i32 = 0;
    while (x as usize) < patches.len() {
        if patches[x as usize].length1 <= patch_size {
            hits.kept += 1;
            x += 1;
            continue;
        }
        hits.split += 1;
        let mut bigpatch = patches.remove(x as usize);
        x -= 1;
        let mut start1 = bigpatch.start1;
        let mut start2 = bigpatch.start2;
        let mut precontext: Vec<char> = vec![];
        while !bigpatch.diffs.is_empty() {
            let mut patch = Patch::new(vec![], 0, 0, 0, 0);
            let mut empty = true;
            patch.start1 = start1 - precontext.len() as i32;
            patch.start2 = start2 - precontext.len() as i32;
            if !precontext.is_empty() {
                patch.length1 = precontext.len() as i32;
                patch.length2 = precontext.len() as i32;
                patch
                    .diffs
                    .push(Diff::new(0, precontext.clone().iter().collect()));
            }
            while !bigpatch.diffs.is_empty() && patch.length1 < patch_size - dmp.patch_margin {
                let diff_type = bigpatch.diffs[0].operation;
                let mut diff_text: Vec<char> = bigpatch.diffs[0].text.chars().collect();
                if diff_type == 1 {
                    patch.length2 += diff_text.len() as i32;
                    start2 += diff_text.len() as i32;
                    patch.diffs.push(bigpatch.diffs[0].clone());
                    bigpatch.diffs.remove(0);
                    empty = false;
                } else if diff_type == -1
                    && patch.diffs.len() == 1
                    && patch.diffs[0].operation == 0
                    && (diff_text.len() as i32) > 2 * patch_size
                {
                    hits.whole_deletion += 1;
                    patch.length1 += diff_text.len() as i32;
                    start1 += diff_text.len() as i32;
                    empty = false;
                    patch
                        .diffs
                        .push(Diff::new(diff_type, diff_text.iter().collect()));
                    bigpatch.diffs.remove(0);
                } else {
                    let diff_text_len: i32 = diff_text.len() as i32;
                    diff_text = diff_text[..min(
                        diff_text_len,
                        patch_size - patch.length1 - dmp.patch_margin,
                    ) as usize]
                        .to_vec();
                    patch.length1 += diff_text.len() as i32;
                    start1 += diff_text.len() as i32;
                    if diff_type == 0 {
                        patch.length2 += diff_text.len() as i32;
                        start2 += diff_text.len() as i32;
                    } else {
                        empty = false;
                    }
                    patch
                        .diffs
                        .push(Diff::new(diff_type, diff_text.clone().iter().collect()));
                    let temp: String = diff_text[..].iter().collect();
                    if temp == bigpatch.diffs[0].text.clone() {
                        bigpatch.diffs.remove(0);
                    } else {
                        hits.partial += 1;
                        let temp1: Vec<char> = bigpatch.diffs[0].text.chars().collect();
                        bigpatch.diffs[0].text = temp1[diff_text.len()..].iter().collect();
                    }
                }
            }
            precontext = dmp.diff_text2(&mut patch.diffs).chars().collect();
            precontext = precontext
                [(precontext.len() - min(dmp.patch_margin, precontext.len() as i32) as usize)..]
                .to_vec();
            let postcontext =
                if dmp.diff_text1(&mut bigpatch.diffs).chars().count() as i32 > dmp.patch_margin {
                    let temp: Vec<char> = dmp.diff_text1(&mut bigpatch.diffs).chars().collect();
                    temp[..dmp.patch_margin as usize].iter().collect()
                } else {
                    dmp.diff_text1(&mut bigpatch.diffs)
                };
            let postcontext_len = postcontext.chars().count() as i32;
            if !postcontext.is_empty() {
                patch.length1 += postcontext_len;
                patch.length2 += postcontext_len;
                if !patch.diffs.is_empty() && patch.diffs[patch.diffs.len() - 1].operation == 0 {
                    hits.merged_context += 1;
                    let len = patch.diffs.len();
                    patch.diffs[len - 1].text += postcontext.as_str();
                } else {
                    hits.pushed_context += 1;
                    patch.diffs.push(Diff::new(0, postcontext));
                }
            }
            if empty {
                hits.dropped += 1;
            } else {
                x += 1;
                patches.insert(x as usize, patch);
            }
        }
        x += 1;
    }
}

// Any patch list the public split accepts: empty and oversized pieces, an
// operation outside -1..=1, and a declared length1 that disagrees with the text.
fn random_patches(rng: &mut Lcg) -> Vec<Patch> {
    (0..rng.below(4))
        .map(|_| {
            let diffs: Vec<Diff> = (0..rng.below(9))
                .map(|_| {
                    let operation = [-1, 0, 1, 0, 1, -1, 0, 2][rng.below(8)];
                    let len = [0, 1, 3, 7, 12, 30, 70, 150][rng.below(8)];
                    Diff::new(operation, random_text(rng, len))
                })
                .collect();
            let length1 = match rng.below(8) {
                0 => 0,
                1 => 1000,
                _ => diff_text1_chars(&diffs).len() as i32,
            };
            let length2 = diff_text2_chars(&diffs).len() as i32;
            Patch::new(
                diffs,
                rng.below(50) as i32,
                rng.below(50) as i32,
                length1,
                length2,
            )
        })
        .collect()
}

#[test]
fn linear_split_matches_shifting() {
    let mut rng = Lcg(1);
    let mut hits = SplitHits::default();
    // (match_maxbits, patch_margin): twice the margin stays under the size,
    // past which the split never ends; a zero size turns it off.
    let settings = [
        (32, 4),
        (64, 4),
        (16, 4),
        (9, 4),
        (8, 2),
        (5, 2),
        (3, 1),
        (4, 0),
        (1, 0),
        (0, 4),
    ];
    for round in 0..3_000 {
        let mut dmp = Dmp::new();
        (dmp.match_maxbits, dmp.patch_margin) = settings[round % settings.len()];
        let input = random_patches(&mut rng);
        let (mut expected, mut actual) = (input.clone(), input.clone());
        splitmax_by_shifting(&mut dmp, &mut expected, &mut hits);
        dmp.patch_splitmax(&mut actual);
        assert_eq!(
            actual,
            expected,
            "{:?} {input:?}",
            settings[round % settings.len()]
        );
    }
    assert_eq!(
        hits,
        SplitHits {
            kept: 1427,
            split: 2667,
            dropped: 94717,
            whole_deletion: 668,
            partial: 143153,
            merged_context: 70188,
            pushed_context: 29336,
        }
    );
}
