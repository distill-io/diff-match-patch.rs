// Diff orchestration: diff_main and its recursion (compute, line mode, bisect
// splitting), the cleanup passes, and diff-derived metrics. The token-generic
// primitives live in engine.rs; text materialization happens here.

use crate::engine;
use crate::types::{Diff, Dmp};
use std::time::{Duration, Instant};

// The historic public API takes &Vec/&mut Vec/&String; frozen by the
// drop-in compatibility contract.
#[allow(clippy::ptr_arg)]
impl Dmp {
    /// Find the differences between two chars.  Simplifies the problem by
    /// stripping any common prefix or suffix off the texts before diffing.
    ///
    /// Args:
    /// text1: Old chars to be diffed.
    /// text2: New chars to be diffed.
    /// checklines: Optional speedup flag. If present and false, then don't run
    /// a line-level diff first to identify the changed areas.
    /// Defaults to true, which does a faster, slightly less optimal diff.
    /// Returns:
    /// Vector of diffs as changes.
    pub fn diff_main(&mut self, text1: &str, text2: &str, checklines: bool) -> Vec<Diff> {
        let deadline = self.deadline_from_now();
        #[cfg(feature = "grapheme")]
        {
            if self.segmentation == crate::types::Segmentation::Grapheme {
                // The whole pipeline (line packing included) runs on cluster
                // ids, so every pass stays cluster-atomic; ids never collide
                // with '\n' because allocation starts above it.
                let mut packer = crate::tokenize::GraphemePacker::new(&[text1, text2]);
                let packed1 = packer.pack(text1);
                let packed2 = packer.pack(text2);
                let mut diffs = main_internal(self, &packed1, &packed2, checklines, deadline);
                packer.unpack_diffs(&mut diffs);
                return diffs;
            }
        }
        main_internal(self, text1, text2, checklines, deadline)
    }

    /// The deadline equivalent of `diff_timeout` starting now; bisect gives up
    /// once it passes. `Some(0.0)` therefore means "zero budget", while `None`
    /// disables the deadline entirely (and with it the half-match speedup).
    ///
    /// Degenerate values mirror the historical float comparisons instead of
    /// panicking in Duration/Instant math: a negative timeout behaves as zero
    /// budget, NaN as no deadline, and huge values are capped (~30 years).
    fn deadline_from_now(&self) -> Option<Instant> {
        let secs = self.diff_timeout?;
        if secs.is_nan() {
            return None;
        }
        Some(Instant::now() + Duration::from_secs_f32(secs.clamp(0.0, 1.0e9)))
    }

    /// Do a quick line-level diff on both chars, then rediff the parts for
    /// greater accuracy.
    /// This speedup can produce non-minimal diffs.
    ///
    /// Args:
    /// text1: Old chars to be diffed.
    /// text2: New chars to be diffed.
    ///
    /// Returns:
    /// Vector of diffs as changes.
    pub fn diff_linemode(&mut self, text1: &Vec<char>, text2: &Vec<char>) -> Vec<Diff> {
        #[cfg(feature = "grapheme")]
        {
            if self.segmentation == crate::types::Segmentation::Grapheme {
                // Honor the cluster invariant on this public entry point too:
                // rebuild the texts and diff them in packed space.
                let t1: String = text1.iter().collect();
                let t2: String = text2.iter().collect();
                return self.diff_main(&t1, &t2, true);
            }
        }
        let deadline = self.deadline_from_now();
        line_mode(self, text1, text2, deadline)
    }

    /// Find the 'middle snake' of a diff, split the problem in two
    /// and return the recursively constructed diff.
    /// See Myers 1986 paper: An O(ND) Difference Algorithm and Its Variations.
    ///
    /// Args:
    /// text1: Old chars to be diffed.
    /// text2: New chars to be diffed.
    ///
    /// Returns:
    /// Vector of diffs as changes.
    pub fn diff_bisect(&mut self, char1: &Vec<char>, char2: &Vec<char>) -> Vec<Diff> {
        let deadline = self.deadline_from_now();
        bisect_diff(self, char1, char2, deadline)
    }

    pub fn diff_common_prefix(&mut self, text1: &Vec<char>, text2: &Vec<char>) -> i32 {
        // The number of characters common to the start of each text.
        engine::common_prefix(text1, text2) as i32
    }

    pub fn diff_common_suffix(&mut self, text1: &Vec<char>, text2: &Vec<char>) -> i32 {
        // The number of characters common to the end of each text.
        engine::common_suffix(text1, text2) as i32
    }

    pub fn diff_common_overlap(&mut self, text1: &Vec<char>, text2: &Vec<char>) -> i32 {
        // The number of characters common to the end of text1 and the start of text2.
        engine::common_overlap(text1, text2) as i32
    }

    /// Do the two texts share a substring which is at least half the length of
    /// the longer text?
    /// This speedup can produce non-minimal diffs.
    ///
    /// Returns:
    /// Five element Vector, containing the prefix of text1, the suffix of text1,
    /// the prefix of text2, the suffix of text2 and the common middle.  Or empty vector
    /// if there was no match.
    pub fn diff_half_match(&mut self, text1: &Vec<char>, text2: &Vec<char>) -> Vec<String> {
        // Don't risk returning a non-optimal diff if we have unlimited time.
        // Uses the deadline mapping (not diff_timeout directly) so degenerate
        // values like NaN disable half-match exactly as they disable the
        // deadline in compute().
        if self.deadline_from_now().is_none() {
            return vec![];
        }
        match engine::half_match(text1, text2) {
            None => vec![],
            Some(hm) => {
                let (text1_a, text1_b, text2_a, text2_b, mid_common) =
                    split_half_match(text1, text2, &hm);
                vec![text1_a, text1_b, text2_a, text2_b, mid_common]
            }
        }
    }
}

/// Materialize the five half-match pieces (text1 prefix/suffix, text2
/// prefix/suffix, common middle) from a HalfMatch split.
fn split_half_match(
    old: &[char],
    new: &[char],
    hm: &engine::HalfMatch,
) -> (String, String, String, String, String) {
    (
        old[..hm.old_a].iter().collect(),
        old[hm.old_a + hm.common..].iter().collect(),
        new[..hm.new_a].iter().collect(),
        new[hm.new_a + hm.common..].iter().collect(),
        old[hm.old_a..hm.old_a + hm.common].iter().collect(),
    )
}

fn main_internal(
    dmp: &mut Dmp,
    text1: &str,
    text2: &str,
    checklines: bool,
    deadline: Option<Instant>,
) -> Vec<Diff> {
    // check for empty text
    if text1.is_empty() && text2.is_empty() {
        return vec![];
    } else if text1.is_empty() {
        return vec![Diff::new(1, text2.to_string())];
    } else if text2.is_empty() {
        return vec![Diff::new(-1, text1.to_string())];
    }

    // check for equality
    if text1 == text2 {
        return vec![Diff::new(0, text1.to_string())];
    }

    let char1: Vec<char> = text1.chars().collect();
    let char2: Vec<char> = text2.chars().collect();
    // Trim off common prefix and suffix (speedup).
    let prefix = engine::common_prefix(&char1, &char2);
    let suffix = engine::common_suffix(&char1[prefix..], &char2[prefix..]);
    let mid1 = &char1[prefix..char1.len() - suffix];
    let mid2 = &char2[prefix..char2.len() - suffix];

    let mut diffs: Vec<Diff> = Vec::new();
    // Restore the prefix, compute the diff on the middle block, restore the suffix.
    if prefix > 0 {
        diffs.push(Diff::new(0, char1[..prefix].iter().collect()));
    }
    for diff in compute(dmp, mid1, mid2, checklines, deadline) {
        diffs.push(diff);
    }
    if suffix > 0 {
        diffs.push(Diff::new(0, char1[char1.len() - suffix..].iter().collect()));
    }
    dmp.diff_cleanup_merge_impl(&mut diffs);
    diffs
}

/// Find the differences between two token slices that share no common prefix
/// or suffix and are not both empty.
fn compute(
    dmp: &mut Dmp,
    old: &[char],
    new: &[char],
    checklines: bool,
    deadline: Option<Instant>,
) -> Vec<Diff> {
    if old.is_empty() {
        // Just add some text (speedup).
        return vec![Diff::new(1, new.iter().collect())];
    }
    if new.is_empty() {
        // Just delete some text (speedup).
        return vec![Diff::new(-1, old.iter().collect())];
    }
    {
        let (long, short) = if old.len() >= new.len() {
            (old, new)
        } else {
            (new, old)
        };
        if let Some(i) = engine::find_sub(long, short, 0) {
            // Shorter text is inside the longer text (speedup).
            let op = if old.len() > new.len() { -1 } else { 1 };
            let mut diffs: Vec<Diff> = Vec::new();
            if i != 0 {
                diffs.push(Diff::new(op, long[..i].iter().collect()));
            }
            diffs.push(Diff::new(0, short.iter().collect()));
            if i + short.len() != long.len() {
                diffs.push(Diff::new(op, long[i + short.len()..].iter().collect()));
            }
            return diffs;
        }
        if short.len() == 1 {
            // Single character string.
            // After the previous speedup, the character can't be an equality.
            return vec![
                Diff::new(-1, old.iter().collect()),
                Diff::new(1, new.iter().collect()),
            ];
        }
    }

    // Check to see if the problem can be split in two (only when a deadline is
    // set: half-match trades optimality for speed).
    if deadline.is_some() {
        if let Some(hm) = engine::half_match(old, new) {
            // A half-match was found, send both pairs off for separate processing.
            let (text1_a, text1_b, text2_a, text2_b, mid_common) = split_half_match(old, new, &hm);
            let mut diffs = main_internal(dmp, &text1_a, &text2_a, checklines, deadline);
            diffs.push(Diff::new(0, mid_common));
            diffs.extend(main_internal(dmp, &text1_b, &text2_b, checklines, deadline));
            return diffs;
        }
    }

    if checklines && old.len() > 100 && new.len() > 100 {
        return line_mode(dmp, old, new, deadline);
    }
    bisect_diff(dmp, old, new, deadline)
}

/// Split on the Myers middle snake and recurse, or emit delete+insert when
/// there is no overlap (or the deadline expired).
fn bisect_diff(dmp: &mut Dmp, old: &[char], new: &[char], deadline: Option<Instant>) -> Vec<Diff> {
    match engine::bisect(old, new, deadline) {
        Some((x, y)) => {
            let text1a: String = old[..x].iter().collect();
            let text2a: String = new[..y].iter().collect();
            let text1b: String = old[x..].iter().collect();
            let text2b: String = new[y..].iter().collect();
            // Compute both diffs serially.
            let mut diffs = main_internal(dmp, &text1a, &text2a, false, deadline);
            diffs.append(&mut main_internal(dmp, &text1b, &text2b, false, deadline));
            diffs
        }
        None => {
            // Number of diffs equals number of characters, no commonality at all.
            vec![
                Diff::new(-1, old.iter().collect()),
                Diff::new(1, new.iter().collect()),
            ]
        }
    }
}

/// Line-mode speedup: diff on packed line ids first, then rediff the
/// replacement blocks character by character.
fn line_mode(dmp: &mut Dmp, old: &[char], new: &[char], deadline: Option<Instant>) -> Vec<Diff> {
    // Scan the text on a line-by-line basis first.
    let (text3, text4, linearray) = dmp.diff_lines_tochars(&old.to_vec(), &new.to_vec());

    let mut diffs: Vec<Diff> = main_internal(dmp, &text3, &text4, false, deadline);

    // Convert the diff back to original text.
    dmp.diff_chars_tolines(&mut diffs, &linearray);
    // Eliminate freak matches (e.g. blank lines)
    dmp.diff_cleanup_semantic_impl(&mut diffs);

    // Rediff any replacement blocks, this time character-by-character.
    // Add a dummy entry at the end.
    diffs.push(Diff::new(0, "".to_string()));
    let mut count_delete = 0;
    let mut count_insert = 0;
    let mut text_delete: String = "".to_string();
    let mut text_insert: String = "".to_string();
    let mut pointer = 0;
    let mut temp: Vec<Diff> = vec![];
    while pointer < diffs.len() {
        if diffs[pointer].operation == 1 {
            count_insert += 1;
            text_insert += diffs[pointer].text.as_str();
        } else if diffs[pointer].operation == -1 {
            count_delete += 1;
            text_delete += diffs[pointer].text.as_str();
        } else {
            // Upon reaching an equality, check for prior redundancies.
            if count_delete >= 1 && count_insert >= 1 {
                // Delete the offending records and add the merged ones.
                let sub_diff = main_internal(dmp, &text_delete, &text_insert, false, deadline);
                for z in sub_diff {
                    temp.push(z);
                }
                temp.push(Diff::new(
                    diffs[pointer].operation,
                    diffs[pointer].text.clone(),
                ));
            } else {
                if !text_delete.is_empty() {
                    temp.push(Diff::new(-1, text_delete));
                }
                if !text_insert.is_empty() {
                    temp.push(Diff::new(1, text_insert));
                }
                temp.push(Diff::new(
                    diffs[pointer].operation,
                    diffs[pointer].text.clone(),
                ));
            }
            count_delete = 0;
            count_insert = 0;
            text_delete = "".to_string();
            text_insert = "".to_string();
        }
        pointer += 1;
    }
    temp.pop(); //Remove the dummy entry at the end.
    temp
}
