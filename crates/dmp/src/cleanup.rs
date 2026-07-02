// Diff cleanup passes (merge, semantic, efficiency) and diff-derived
// metrics (xindex, text1/text2 reconstruction, Levenshtein).

use crate::engine;
#[cfg(feature = "grapheme")]
use crate::types::Segmentation;
use crate::types::{max, Diff, Dmp, TDiff};

// The historic public API takes &Vec/&mut Vec/&String; frozen by the
// drop-in compatibility contract.
#[allow(clippy::ptr_arg)]
impl Dmp {
    /// Reduce the number of edits by eliminating semantically trivial
    /// equalities.
    ///
    /// Args:
    /// diffs: Vectors of diff object.
    pub(crate) fn diff_cleanup_semantic_impl(&mut self, diffs: &mut Vec<TDiff>) {
        let mut changes = false;
        let mut equalities: Vec<i32> = vec![]; // Stack of indices where equalities are found.
        let mut last_equality: Vec<char> = vec![]; // Always equal to diffs[equalities[-1]][1]
        let mut pointer: i32 = 0; // Index of current position.
                                  // Number of chars that changed prior to the equality.
        let mut length_insertions1 = 0;
        let mut length_deletions1 = 0;
        // Number of chars that changed after the equality.
        let mut length_insertions2 = 0;
        let mut length_deletions2 = 0;
        while (pointer as usize) < diffs.len() {
            if diffs[pointer as usize].operation == 0 {
                // Equality found.
                equalities.push(pointer);
                length_insertions1 = length_insertions2;
                length_insertions2 = 0;
                length_deletions1 = length_deletions2;
                length_deletions2 = 0;
                last_equality = diffs[pointer as usize].data.clone();
            } else {
                // An insertion or deletion.
                if diffs[pointer as usize].operation == 1 {
                    length_insertions2 += diffs[pointer as usize].data.len() as i32;
                } else {
                    length_deletions2 += diffs[pointer as usize].data.len() as i32;
                    // Eliminate an equality that is smaller or equal to the edits on both
                    // sides of it.
                }
                let last_equality_len = last_equality.len() as i32;
                if last_equality_len > 0
                    && last_equality_len <= max(length_insertions1, length_deletions1)
                    && last_equality_len <= max(length_insertions2, length_deletions2)
                {
                    // Duplicate record.
                    diffs.insert(
                        equalities[equalities.len() - 1] as usize,
                        TDiff::new(-1, last_equality.clone()),
                    );
                    // Change second copy to insert.
                    diffs[equalities[equalities.len() - 1] as usize + 1] = TDiff::new(
                        1,
                        diffs[equalities[equalities.len() - 1] as usize + 1]
                            .data
                            .clone(),
                    );
                    // Throw away the equality we just deleted.
                    equalities.pop();
                    // Throw away the previous equality (it needs to be reevaluated).
                    if !equalities.is_empty() {
                        equalities.pop();
                    }
                    if !equalities.is_empty() {
                        pointer = equalities[equalities.len() - 1];
                    } else {
                        pointer = -1;
                    }
                    // Reset the counters.
                    length_insertions1 = 0;
                    length_deletions1 = 0;
                    length_insertions2 = 0;
                    length_deletions2 = 0;
                    last_equality = vec![];
                    changes = true;
                }
            }
            pointer += 1;
        }
        // Normalize the diff.
        if changes {
            self.diff_cleanup_merge_impl(diffs);
        }
        self.diff_cleanup_semantic_lossless_impl(diffs);

        let mut overlap_length1: i32;
        let mut overlap_length2: i32;
        pointer = 1;
        while (pointer as usize) < diffs.len() {
            if diffs[pointer as usize - 1].operation == -1 && diffs[pointer as usize].operation == 1
            {
                let deletion_vec: Vec<char> = diffs[pointer as usize - 1].data.clone();
                let insertion_vec: Vec<char> = diffs[pointer as usize].data.clone();
                overlap_length1 = self.diff_common_overlap(&deletion_vec, &insertion_vec);
                overlap_length2 = self.diff_common_overlap(&insertion_vec, &deletion_vec);
                if overlap_length1 >= overlap_length2 {
                    if (overlap_length1 as f32) >= (deletion_vec.len() as f32 / 2.0)
                        || (overlap_length1 as f32) >= (insertion_vec.len() as f32 / 2.0)
                    {
                        // Overlap found.  Insert an equality and trim the surrounding edits.
                        diffs.insert(
                            pointer as usize,
                            TDiff::new(0, insertion_vec[..(overlap_length1 as usize)].to_vec()),
                        );
                        diffs[pointer as usize - 1] = TDiff::new(
                            -1,
                            deletion_vec[..(deletion_vec.len() - overlap_length1 as usize)]
                                .to_vec(),
                        );
                        diffs[pointer as usize + 1] =
                            TDiff::new(1, insertion_vec[(overlap_length1 as usize)..].to_vec());
                        pointer += 1;
                    }
                } else if (overlap_length2 as f32) >= (deletion_vec.len() as f32 / 2.0)
                    || (overlap_length2 as f32) >= (insertion_vec.len() as f32 / 2.0)
                {
                    // Reverse overlap found.
                    // Insert an equality and swap and trim the surrounding edits.
                    diffs.insert(
                        pointer as usize,
                        TDiff::new(0, deletion_vec[..(overlap_length2 as usize)].to_vec()),
                    );
                    let insertion_vec_len = insertion_vec.len();
                    diffs[pointer as usize - 1] = TDiff::new(
                        1,
                        insertion_vec[..(insertion_vec_len - overlap_length2 as usize)].to_vec(),
                    );
                    diffs[pointer as usize + 1] =
                        TDiff::new(-1, deletion_vec[(overlap_length2 as usize)..].to_vec());
                    pointer += 1;
                }
                pointer += 1;
            }
            pointer += 1;
        }
    }

    /// Look for single edits surrounded on both sides by equalities
    /// which can be shifted sideways to align the edit to a word boundary.
    /// e.g: The c<ins>at c</ins>ame. -> The <ins>cat </ins>came.
    ///
    /// Args:
    /// diffs: Vector of diff object.
    pub(crate) fn diff_cleanup_semantic_lossless_impl(&mut self, diffs: &mut Vec<TDiff>) {
        let mut pointer: i32 = 1;
        //Intentionally ignore the first and last element (don't need checking).
        while pointer < diffs.len() as i32 - 1 {
            if diffs[pointer as usize - 1].operation == 0
                && diffs[pointer as usize + 1].operation == 0
            {
                // This is a single edit surrounded by equalities. Slide it
                // over one concatenated buffer: the buffer is invariant under
                // both the left shift and each right slide (the moved char is
                // equal on both sides by construction), so only the split
                // points (a, b) move, and each step costs two boundary score
                // probes instead of rebuilding three char vectors.
                let mut buf: Vec<char> = Vec::with_capacity(
                    diffs[pointer as usize - 1].data.len()
                        + diffs[pointer as usize].data.len()
                        + diffs[pointer as usize + 1].data.len(),
                );
                buf.extend_from_slice(&diffs[pointer as usize - 1].data);
                buf.extend_from_slice(&diffs[pointer as usize].data);
                buf.extend_from_slice(&diffs[pointer as usize + 1].data);
                let eq1_len = diffs[pointer as usize - 1].data.len();
                let edit_len = diffs[pointer as usize].data.len();
                let mut a = eq1_len;
                let mut b = eq1_len + edit_len;

                // First, shift the edit as far left as possible.
                let common_offset = engine::common_suffix(&buf[..a], &buf[a..b]);
                a -= common_offset;
                b -= common_offset;

                // Second, step character by character right, looking for the best fit.
                let mut best = (a, b);
                let mut best_score = self.diff_cleanup_semantic_score(&buf[..a], &buf[a..b])
                    + self.diff_cleanup_semantic_score(&buf[a..b], &buf[b..]);
                while a < b && b < buf.len() && buf[a] == buf[b] {
                    a += 1;
                    b += 1;
                    let score = self.diff_cleanup_semantic_score(&buf[..a], &buf[a..b])
                        + self.diff_cleanup_semantic_score(&buf[a..b], &buf[b..]);
                    // The >= encourages trailing rather than leading whitespace on edits.
                    if score >= best_score {
                        best_score = score;
                        best = (a, b);
                    }
                }

                if diffs[pointer as usize - 1].data != buf[..best.0] {
                    // We have an improvement, save it back to the diff.
                    let best_equality1 = buf[..best.0].to_vec();
                    let best_edit = buf[best.0..best.1].to_vec();
                    let best_equality2 = buf[best.1..].to_vec();
                    if !best_equality1.is_empty() {
                        diffs[pointer as usize - 1] =
                            TDiff::new(diffs[pointer as usize - 1].operation, best_equality1);
                    } else {
                        diffs.remove(pointer as usize - 1);
                        pointer -= 1;
                    }
                    diffs[pointer as usize] =
                        TDiff::new(diffs[pointer as usize].operation, best_edit);
                    if !best_equality2.is_empty() {
                        diffs[pointer as usize + 1] =
                            TDiff::new(diffs[pointer as usize + 1].operation, best_equality2);
                    } else {
                        diffs.remove(pointer as usize + 1);
                        pointer += 1;
                    }
                }
            }
            pointer += 1;
        }
    }

    fn diff_cleanup_semantic_score(&mut self, one: &[char], two: &[char]) -> i32 {
        /*
        Given two strings, compute a score representing whether the
        internal boundary falls on logical boundaries.
        Scores range from 6 (best) to 0 (worst).
        Closure, but does not reference any external variables.

        Args:
            one: First chars.
            two: Second chars.

        Returns:
            The score.
        */
        if one.is_empty() || two.is_empty() {
            // Edges are the best.
            return 6;
        }

        // Each port of this function behaves slightly differently due to
        // subtle differences in each language's definition of things like
        // 'whitespace'.  Since this function's purpose is largely cosmetic,
        // the choice has been made to use each language's native features
        // rather than force total conformity.
        let char1 = one[one.len() - 1];
        let char2 = two[0];
        let nonalphanumeric1: bool = !char1.is_alphanumeric();
        let nonalphanumeric2: bool = !char2.is_alphanumeric();
        let whitespace1: bool = nonalphanumeric1 & char1.is_whitespace();
        let whitespace2: bool = nonalphanumeric2 & char2.is_whitespace();
        let linebreak1: bool = whitespace1 & ((char1 == '\r') | (char1 == '\n'));
        let linebreak2: bool = whitespace2 & ((char2 == '\r') | (char2 == '\n'));
        let mut test1: bool = false;
        let mut test2: bool = false;
        if one.len() > 1 && one[one.len() - 1] == '\n' && one[one.len() - 2] == '\n' {
            test1 = true;
        }
        if one.len() > 2
            && one[one.len() - 1] == '\n'
            && one[one.len() - 3] == '\n'
            && one[one.len() - 2] == '\r'
        {
            test1 = true;
        }
        if two.len() > 1 && two[two.len() - 1] == '\n' && two[two.len() - 2] == '\n' {
            test2 = true;
        }
        if two.len() > 2
            && two[two.len() - 1] == '\n'
            && two[two.len() - 3] == '\n'
            && two[two.len() - 2] == '\r'
        {
            test2 = true;
        }
        let blankline1: bool = linebreak1 & test1;
        let blankline2: bool = linebreak2 & test2;
        if blankline1 || blankline2 {
            // Five points for blank lines.
            return 5;
        }
        if linebreak1 || linebreak2 {
            // Four points for line breaks.
            return 4;
        }
        if nonalphanumeric1 && !whitespace1 && whitespace2 {
            // Three points for end of sentences.
            return 3;
        }
        if whitespace1 || whitespace2 {
            // Two points for whitespace.
            return 2;
        }
        if nonalphanumeric1 || nonalphanumeric2 {
            // One point for non-alphanumeric.
            return 1;
        }
        0
    }

    /// Reduce the number of edits by eliminating operationally trivial
    /// equalities.
    ///
    /// Args:
    /// diffs: Vector of diff object.
    pub(crate) fn diff_cleanup_efficiency_impl(&mut self, diffs: &mut Vec<TDiff>) {
        if diffs.is_empty() {
            return;
        }
        let mut changes: bool = false;
        let mut equalities: Vec<i32> = vec![]; //Stack of indices where equalities are found.
        let mut last_equality: Vec<char> = vec![]; // Always equal to diffs[equalities[-1]][1]
        let mut pointer: i32 = 0; // Index of current position.
        let mut pre_ins = false; // Is there an insertion operation before the last equality.
        let mut pre_del = false; // Is there a deletion operation before the last equality.
        let mut post_ins = false; // Is there an insertion operation after the last equality.
        let mut post_del = false; // Is there a deletion operation after the last equality.
        while (pointer as usize) < diffs.len() {
            if diffs[pointer as usize].operation == 0 {
                if diffs[pointer as usize].data.len() < self.edit_cost as usize
                    && (post_del || post_ins)
                {
                    // Candidate found.
                    equalities.push(pointer);
                    pre_ins = post_ins;
                    pre_del = post_del;
                    last_equality = diffs[pointer as usize].data.clone();
                } else {
                    // Not a candidate, and can never become one.
                    equalities = vec![];
                    last_equality = vec![];
                }
                post_ins = false;
                post_del = false;
            } else {
                // An insertion or deletion.
                if diffs[pointer as usize].operation == -1 {
                    post_del = true;
                } else {
                    post_ins = true;
                }

                /*
                Five types to be split:
                <ins>A</ins><del>B</del>XY<ins>C</ins><del>D</del>
                <ins>A</ins>X<ins>C</ins><del>D</del>
                <ins>A</ins><del>B</del>X<ins>C</ins>
                <ins>A</del>X<ins>C</ins><del>D</del>
                <ins>A</ins><del>B</del>X<del>C</del>
                */

                if !last_equality.is_empty()
                    && ((pre_ins && pre_del && post_del && post_ins)
                        || ((last_equality.len() as i32) < self.edit_cost / 2
                            && (pre_ins as i32
                                + pre_del as i32
                                + post_del as i32
                                + post_ins as i32)
                                == 3))
                {
                    // Duplicate record.
                    diffs.insert(
                        equalities[equalities.len() - 1] as usize,
                        TDiff::new(-1, last_equality),
                    );
                    // Change second copy to insert.
                    diffs[equalities[equalities.len() - 1] as usize + 1] = TDiff::new(
                        1,
                        diffs[equalities[equalities.len() - 1] as usize + 1]
                            .data
                            .clone(),
                    );
                    equalities.pop(); // Throw away the equality we just deleted.
                    last_equality = vec![];
                    if pre_ins && pre_del {
                        // No changes made which could affect previous entry, keep going.
                        post_del = true;
                        post_ins = true;
                        equalities = vec![];
                    } else {
                        if !equalities.is_empty() {
                            equalities.pop(); // Throw away the previous equality.
                        }
                        if !equalities.is_empty() {
                            pointer = equalities[equalities.len() - 1];
                        } else {
                            pointer = -1;
                        }
                        post_ins = false;
                        post_del = false;
                    }
                    changes = true;
                }
            }
            pointer += 1;
        }
        if changes {
            self.diff_cleanup_merge_impl(diffs);
        }
    }

    /// Reorder and merge like edit sections.  Merge equalities.
    /// Any edit section can move as long as it doesn't cross an equality.
    ///
    /// Args:
    /// diffs: vectors of diff object.
    pub(crate) fn diff_cleanup_merge_impl(&mut self, diffs: &mut Vec<TDiff>) {
        if diffs.is_empty() {
            return;
        }
        // First pass: one forward fold into a fresh vector — the same
        // accumulate / factor / flush the in-place version did, without its
        // Vec::insert/remove churn (quadratic on many-run diffs). A dummy
        // equality flushes the final run. Text stays as char runs the whole
        // way, so a run of same-op pieces is concatenated by `extend`, never
        // decoded from / re-encoded to UTF-8.
        let mut out: Vec<TDiff> = Vec::with_capacity(diffs.len());
        let mut text_insert: Vec<char> = vec![];
        let mut text_delete: Vec<char> = vec![];
        let mut count_insert = 0;
        let mut count_delete = 0;
        for diff in diffs
            .drain(..)
            .chain(std::iter::once(TDiff::new(0, vec![])))
        {
            if diff.operation == -1 {
                text_delete.extend_from_slice(&diff.data);
                count_delete += 1;
            } else if diff.operation == 1 {
                text_insert.extend_from_slice(&diff.data);
                count_insert += 1;
            } else {
                let mut equality = diff;
                // Upon reaching an equality, check for prior redundancies.
                if count_delete + count_insert > 1 {
                    let mut delete_vec = std::mem::take(&mut text_delete);
                    let mut insert_vec = std::mem::take(&mut text_insert);
                    if count_delete > 0 && count_insert > 0 {
                        // Factor out any common prefixies.
                        let mut commonlength = engine::common_prefix(&insert_vec, &delete_vec);
                        if commonlength != 0 {
                            let prefix = insert_vec[..commonlength].to_vec();
                            match out.last_mut() {
                                Some(prev) if prev.operation == 0 => {
                                    prev.data.extend_from_slice(&prefix)
                                }
                                // No equality before the run to grow: mirrors
                                // the in-place version's insert at the head.
                                _ => out.insert(0, TDiff::new(0, prefix)),
                            }
                            insert_vec.drain(..commonlength);
                            delete_vec.drain(..commonlength);
                        }

                        // Factor out any common suffixies.
                        commonlength = engine::common_suffix(&insert_vec, &delete_vec);
                        if commonlength != 0 {
                            let mut suffix = insert_vec[insert_vec.len() - commonlength..].to_vec();
                            suffix.extend_from_slice(&equality.data);
                            equality.data = suffix;
                            insert_vec.truncate(insert_vec.len() - commonlength);
                            delete_vec.truncate(delete_vec.len() - commonlength);
                        }
                    }
                    // Add the merged records.
                    if !delete_vec.is_empty() {
                        out.push(TDiff::new(-1, delete_vec));
                    }
                    if !insert_vec.is_empty() {
                        out.push(TDiff::new(1, insert_vec));
                    }
                    out.push(equality);
                } else if count_delete + count_insert == 1 {
                    // A single edit passes through untouched (even an
                    // empty-text one, as the in-place version left it).
                    if count_delete == 1 {
                        out.push(TDiff::new(-1, std::mem::take(&mut text_delete)));
                    } else {
                        out.push(TDiff::new(1, std::mem::take(&mut text_insert)));
                    }
                    out.push(equality);
                } else {
                    // No pending edits: merge adjacent equalities.
                    match out.last_mut() {
                        Some(prev) if prev.operation == 0 => {
                            prev.data.extend_from_slice(&equality.data)
                        }
                        _ => out.push(equality),
                    }
                }
                count_delete = 0;
                text_delete = vec![];
                text_insert = vec![];
                count_insert = 0;
            }
        }
        *diffs = out;
        // Remove the dummy entry at the end.
        if diffs[diffs.len() - 1].data.is_empty() {
            diffs.pop();
        }

        /*
        Second pass: look for single edits surrounded on both sides by equalities
        which can be shifted sideways to eliminate an equality.
        e.g: A<ins>BA</ins>C -> <ins>AB</ins>AC
        */
        let mut changes = false;
        let mut i: i32 = 1;
        // Intentionally ignore the first and last element (don't need checking).
        while (i as usize) < diffs.len() - 1 {
            if diffs[i as usize - 1].operation == 0 && diffs[i as usize + 1].operation == 0 {
                // This is a single edit surrounded by equalities.
                if diffs[i as usize]
                    .data
                    .ends_with(&diffs[i as usize - 1].data)
                {
                    // Shift the edit over the previous equality.
                    if !diffs[i as usize - 1].data.is_empty() {
                        let eq1 = diffs[i as usize - 1].data.clone();
                        let keep = diffs[i as usize].data.len() - eq1.len();
                        let mut shifted = eq1.clone();
                        shifted.extend_from_slice(&diffs[i as usize].data[..keep]);
                        diffs[i as usize].data = shifted;
                        let mut next = eq1;
                        next.extend_from_slice(&diffs[i as usize + 1].data);
                        diffs[i as usize + 1].data = next;
                    }
                    diffs.remove(i as usize - 1);
                    changes = true;
                } else if diffs[i as usize]
                    .data
                    .starts_with(&diffs[i as usize + 1].data)
                {
                    // Shift the edit over the next equality.
                    let eq2 = diffs[i as usize + 1].data.clone();
                    diffs[i as usize - 1].data.extend_from_slice(&eq2);
                    let mut edit = diffs[i as usize].data[eq2.len()..].to_vec();
                    edit.extend_from_slice(&eq2);
                    diffs[i as usize].data = edit;
                    diffs.remove(i as usize + 1);
                    changes = true;
                }
            }
            i += 1;
        }
        // If shifts were made, the diff needs reordering and another shift sweep.
        if changes {
            self.diff_cleanup_merge_impl(diffs);
        }
    }

    /// loc is a location in text1, compute and return the equivalent location
    /// in text2.  e.g. "The cat" vs "The big cat", 1->1, 5->8
    ///
    /// Args:
    /// diffs: Vector of diff object.
    /// loc: Location within text1.
    ///
    /// Returns:
    /// Location within text2.
    pub fn diff_xindex(&mut self, diffs: &Vec<Diff>, loc: i32) -> i32 {
        let mut chars1 = 0;
        let mut chars2 = 0;
        let mut last_chars1 = 0;
        let mut last_chars2 = 0;
        let mut lastdiff = Diff::new(0, "".to_string());
        let z = 0;
        for diffs_item in diffs {
            if diffs_item.operation != 1 {
                // Equality or deletion.
                chars1 += diffs_item.text.chars().count() as i32;
            }
            if diffs_item.operation != -1 {
                // Equality or insertion.
                chars2 += diffs_item.text.chars().count() as i32;
            }
            if chars1 > loc {
                // Overshot the location.
                lastdiff = Diff::new(diffs_item.operation, diffs_item.text.clone());
                break;
            }
            last_chars1 = chars1;
            last_chars2 = chars2;
        }
        if lastdiff.operation == -1 && diffs.len() != z {
            // The location was deleted.
            return last_chars2;
        }
        // Add the remaining len(character).
        last_chars2 + (loc - last_chars1)
    }

    /// Compute and return the source text (all equalities and deletions).
    ///
    /// Args:
    /// diffs: Vectoe of diff object.
    ///
    /// Returns:
    /// Source text.
    pub fn diff_text1(&mut self, diffs: &mut Vec<Diff>) -> String {
        let mut text: String = "".to_string();
        for adiff in diffs {
            if adiff.operation != 1 {
                text += adiff.text.as_str();
            }
        }
        text
    }

    /// Compute and return the destination text (all equalities and insertions).
    ///
    /// Args:
    /// diffs: Vector of diff object.
    ///
    /// Returns:
    /// destination text.
    pub fn diff_text2(&mut self, diffs: &mut Vec<Diff>) -> String {
        let mut text: String = "".to_string();
        for adiff in diffs {
            if adiff.operation != -1 {
                text += adiff.text.as_str();
            }
        }
        text
    }

    /// Compute the Levenshtein distance; the number of inserted, deleted or
    /// substituted characters.
    ///
    /// Args:
    /// diffs: Vector of diff object.
    ///
    /// Returns:
    /// Number of changes.
    pub fn diff_levenshtein(&mut self, diffs: &Vec<Diff>) -> i32 {
        let mut levenshtein = 0;
        let mut insertions = 0;
        let mut deletions = 0;
        for adiff in diffs {
            if adiff.operation == 1 {
                insertions += adiff.text.chars().count();
            } else if adiff.operation == -1 {
                deletions += adiff.text.chars().count();
            } else {
                // A deletion and an insertion is one substitution.
                levenshtein += max(insertions as i32, deletions as i32);
                insertions = 0;
                deletions = 0;
            }
        }
        levenshtein += max(insertions as i32, deletions as i32);
        levenshtein
    }
}

// Public cleanup entry points. In grapheme mode every boundary operation runs
// in packed cluster-id space so no pass can split a cluster; char mode calls
// the implementations directly. In packed space the length heuristics (e.g.
// which small equalities semantic cleanup eliminates) count CLUSTERS rather
// than scalars, and lossless boundary scoring sees placeholder ids as plain
// non-alphanumeric chars — deliberate: grapheme mode reasons entirely in
// cluster units.
// The historic public API takes &Vec/&mut Vec/&String; frozen by the
// drop-in compatibility contract.
#[allow(clippy::ptr_arg)]
impl Dmp {
    pub fn diff_cleanup_semantic(&mut self, diffs: &mut Vec<Diff>) {
        dispatch(self, diffs, Dmp::diff_cleanup_semantic_impl);
    }

    pub fn diff_cleanup_semantic_lossless(&mut self, diffs: &mut Vec<Diff>) {
        dispatch(self, diffs, Dmp::diff_cleanup_semantic_lossless_impl);
    }

    pub fn diff_cleanup_efficiency(&mut self, diffs: &mut Vec<Diff>) {
        dispatch(self, diffs, Dmp::diff_cleanup_efficiency_impl);
    }

    pub fn diff_cleanup_merge(&mut self, diffs: &mut Vec<Diff>) {
        dispatch(self, diffs, Dmp::diff_cleanup_merge_impl);
    }
}

/// Run a token-space cleanup pass on public `String`-carrying diffs: decode
/// each piece to a char run once, run the pass, re-encode once. External
/// callers pay one round-trip instead of the per-operation decodes the passes
/// used to do internally. Grapheme mode packs into cluster-id space first so
/// no pass can split a cluster.
fn dispatch(dmp: &mut Dmp, diffs: &mut Vec<Diff>, pass: fn(&mut Dmp, &mut Vec<TDiff>)) {
    #[cfg(feature = "grapheme")]
    {
        if dmp.segmentation == Segmentation::Grapheme {
            let texts: Vec<&str> = diffs.iter().map(|d| d.text.as_str()).collect();
            let mut packer = crate::tokenize::GraphemePacker::new(&texts);
            let mut tokens: Vec<TDiff> = diffs
                .iter()
                .map(|d| TDiff::new(d.operation, packer.pack(&d.text).chars().collect()))
                .collect();
            pass(dmp, &mut tokens);
            *diffs = tokens.into_iter().map(TDiff::into_diff).collect();
            packer.unpack_diffs(diffs);
            return;
        }
    }
    let mut tokens: Vec<TDiff> = diffs
        .iter()
        .map(|d| TDiff::new(d.operation, d.text.chars().collect()))
        .collect();
    pass(dmp, &mut tokens);
    *diffs = tokens.into_iter().map(TDiff::into_diff).collect();
}
