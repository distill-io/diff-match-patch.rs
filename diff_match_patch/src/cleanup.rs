// Diff cleanup passes (merge, semantic, efficiency) and diff-derived
// metrics (xindex, text1/text2 reconstruction, Levenshtein).

#[cfg(feature = "grapheme")]
use crate::types::Segmentation;
use crate::types::{max, Diff, Dmp};

// The historic public API takes &Vec/&mut Vec/&String; frozen by the
// drop-in compatibility contract.
#[allow(clippy::ptr_arg)]
impl Dmp {
    /// Reduce the number of edits by eliminating semantically trivial
    /// equalities.
    ///
    /// Args:
    /// diffs: Vectors of diff object.
    pub(crate) fn diff_cleanup_semantic_impl(&mut self, diffs: &mut Vec<Diff>) {
        let mut changes = false;
        let mut equalities: Vec<i32> = vec![]; // Stack of indices where equalities are found.
        let mut last_equality = "".to_string(); // Always equal to diffs[equalities[-1]][1]
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
                last_equality = diffs[pointer as usize].text.clone();
            } else {
                // An insertion or deletion.
                if diffs[pointer as usize].operation == 1 {
                    length_insertions2 += diffs[pointer as usize].text.chars().count() as i32;
                } else {
                    length_deletions2 += diffs[pointer as usize].text.chars().count() as i32;
                    // Eliminate an equality that is smaller or equal to the edits on both
                    // sides of it.
                }
                let last_equality_len = last_equality.chars().count() as i32;
                if last_equality_len > 0
                    && last_equality_len <= max(length_insertions1, length_deletions1)
                    && last_equality_len <= max(length_insertions2, length_deletions2)
                {
                    // Duplicate record.
                    diffs.insert(
                        equalities[equalities.len() - 1] as usize,
                        Diff::new(-1, last_equality.clone()),
                    );
                    // Change second copy to insert.
                    diffs[equalities[equalities.len() - 1] as usize + 1] = Diff::new(
                        1,
                        diffs[equalities[equalities.len() - 1] as usize + 1]
                            .text
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
                    last_equality = "".to_string();
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
                let deletion_vec: Vec<char> = diffs[pointer as usize - 1].text.chars().collect();
                let insertion_vec: Vec<char> = diffs[pointer as usize].text.chars().collect();
                overlap_length1 = self.diff_common_overlap(&deletion_vec, &insertion_vec);
                overlap_length2 = self.diff_common_overlap(&insertion_vec, &deletion_vec);
                if overlap_length1 >= overlap_length2 {
                    if (overlap_length1 as f32) >= (deletion_vec.len() as f32 / 2.0)
                        || (overlap_length1 as f32) >= (insertion_vec.len() as f32 / 2.0)
                    {
                        // Overlap found.  Insert an equality and trim the surrounding edits.
                        diffs.insert(
                            pointer as usize,
                            Diff::new(
                                0,
                                insertion_vec[..(overlap_length1 as usize)].iter().collect(),
                            ),
                        );
                        diffs[pointer as usize - 1] = Diff::new(
                            -1,
                            deletion_vec[..(deletion_vec.len() - overlap_length1 as usize)]
                                .iter()
                                .collect(),
                        );
                        diffs[pointer as usize + 1] = Diff::new(
                            1,
                            insertion_vec[(overlap_length1 as usize)..].iter().collect(),
                        );
                        pointer += 1;
                    }
                } else if (overlap_length2 as f32) >= (deletion_vec.len() as f32 / 2.0)
                    || (overlap_length2 as f32) >= (insertion_vec.len() as f32 / 2.0)
                {
                    // Reverse overlap found.
                    // Insert an equality and swap and trim the surrounding edits.
                    diffs.insert(
                        pointer as usize,
                        Diff::new(
                            0,
                            deletion_vec[..(overlap_length2 as usize)].iter().collect(),
                        ),
                    );
                    let insertion_vec_len = insertion_vec.len();
                    diffs[pointer as usize - 1] = Diff::new(
                        1,
                        insertion_vec[..(insertion_vec_len - overlap_length2 as usize)]
                            .iter()
                            .collect(),
                    );
                    diffs[pointer as usize + 1] = Diff::new(
                        -1,
                        deletion_vec[(overlap_length2 as usize)..].iter().collect(),
                    );
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
    pub(crate) fn diff_cleanup_semantic_lossless_impl(&mut self, diffs: &mut Vec<Diff>) {
        let mut pointer = 1;
        let mut equality1;
        let mut equality2;
        let mut edit: String;
        let mut common_offset;
        let mut common_string: String;
        let mut best_equality1;
        let mut best_edit;
        let mut best_equality2;
        let mut best_score;
        let mut score;

        //Intentionally ignore the first and last element (don't need checking).
        while pointer < diffs.len() as i32 - 1 {
            if diffs[pointer as usize - 1].operation == 0
                && diffs[pointer as usize + 1].operation == 0
            {
                //  This is a single edit surrounded by equalities.
                equality1 = diffs[pointer as usize - 1].text.clone();
                edit = diffs[pointer as usize].text.clone();
                equality2 = diffs[pointer as usize + 1].text.clone();
                let mut edit_vec: Vec<char> = edit.chars().collect();
                let mut equality1_vec: Vec<char> = equality1.chars().collect();
                let mut equality2_vec: Vec<char> = equality2.chars().collect();

                // First, shift the edit as far left as possible.
                common_offset = self.diff_common_suffix(&equality1_vec, &edit_vec);
                if common_offset != 0 {
                    common_string = edit_vec[(edit_vec.len() - common_offset as usize)..]
                        .iter()
                        .collect();
                    equality1 = equality1_vec[..(equality1_vec.len() - common_offset as usize)]
                        .iter()
                        .collect();
                    let temp7: String = edit_vec[..(edit_vec.len() - common_offset as usize)]
                        .iter()
                        .collect();
                    edit = common_string.clone() + temp7.as_str();
                    equality2 = common_string + equality2.as_str();
                    edit_vec = edit.chars().collect();
                    equality2_vec = equality2.chars().collect();
                    equality1_vec = equality1.chars().collect();
                }
                // Second, step character by character right, looking for the best fit.
                best_equality1 = equality1.clone();
                best_edit = edit;
                best_equality2 = equality2;
                best_score = self.diff_cleanup_semantic_score(&equality1_vec, &edit_vec)
                    + self.diff_cleanup_semantic_score(&edit_vec, &equality2_vec);
                let edit_len = edit_vec.len();
                let mut equality2_len = equality2_vec.len();
                while equality2_len > 0 && edit_len > 0 {
                    if edit_vec[0] != equality2_vec[0] {
                        break;
                    }
                    let ch = edit_vec[0];
                    equality1_vec.push(ch);
                    edit_vec.push(ch);
                    edit_vec = edit_vec[1..].to_vec();
                    equality2_len -= 1;
                    equality2_vec = equality2_vec[1..].to_vec();
                    score = self.diff_cleanup_semantic_score(&equality1_vec, &edit_vec)
                        + self.diff_cleanup_semantic_score(&edit_vec, &equality2_vec);
                    // The >= encourages trailing rather than leading whitespace on edits.
                    if score >= best_score {
                        best_score = score;
                        best_equality1 = equality1_vec[0..].iter().collect();
                        best_edit = edit_vec[..].iter().collect();
                        best_equality2 = equality2_vec[..].iter().collect();
                    }
                }
                if diffs[pointer as usize - 1].text != best_equality1 {
                    // We have an improvement, save it back to the diff.
                    if !best_equality1.is_empty() {
                        diffs[pointer as usize - 1] =
                            Diff::new(diffs[pointer as usize - 1].operation, best_equality1);
                    } else {
                        diffs.remove(pointer as usize - 1);
                        pointer -= 1;
                    }
                    diffs[pointer as usize] =
                        Diff::new(diffs[pointer as usize].operation, best_edit);
                    if !best_equality2.is_empty() {
                        diffs[pointer as usize + 1] =
                            Diff::new(diffs[pointer as usize + 1].operation, best_equality2);
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
    pub(crate) fn diff_cleanup_efficiency_impl(&mut self, diffs: &mut Vec<Diff>) {
        if diffs.is_empty() {
            return;
        }
        let mut changes: bool = false;
        let mut equalities: Vec<i32> = vec![]; //Stack of indices where equalities are found.
        let mut last_equality: String = "".to_string(); // Always equal to diffs[equalities[-1]][1]
        let mut pointer: i32 = 0; // Index of current position.
        let mut pre_ins = false; // Is there an insertion operation before the last equality.
        let mut pre_del = false; // Is there a deletion operation before the last equality.
        let mut post_ins = false; // Is there an insertion operation after the last equality.
        let mut post_del = false; // Is there a deletion operation after the last equality.
        while (pointer as usize) < diffs.len() {
            if diffs[pointer as usize].operation == 0 {
                if diffs[pointer as usize].text.chars().count() < self.edit_cost as usize
                    && (post_del || post_ins)
                {
                    // Candidate found.
                    equalities.push(pointer);
                    pre_ins = post_ins;
                    pre_del = post_del;
                    last_equality = diffs[pointer as usize].text.clone();
                } else {
                    // Not a candidate, and can never become one.
                    equalities = vec![];
                    last_equality = "".to_string();
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
                        || ((last_equality.chars().count() as i32) < self.edit_cost / 2
                            && (pre_ins as i32
                                + pre_del as i32
                                + post_del as i32
                                + post_ins as i32)
                                == 3))
                {
                    // Duplicate record.
                    diffs.insert(
                        equalities[equalities.len() - 1] as usize,
                        Diff::new(-1, last_equality),
                    );
                    // Change second copy to insert.
                    diffs[equalities[equalities.len() - 1] as usize + 1] = Diff::new(
                        1,
                        diffs[equalities[equalities.len() - 1] as usize + 1]
                            .text
                            .clone(),
                    );
                    equalities.pop(); // Throw away the equality we just deleted.
                    last_equality = "".to_string();
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
    pub(crate) fn diff_cleanup_merge_impl(&mut self, diffs: &mut Vec<Diff>) {
        if diffs.is_empty() {
            return;
        }
        diffs.push(Diff::new(0, "".to_string()));
        let mut text_insert: String = "".to_string();
        let mut text_delete: String = "".to_string();
        let mut i: i32 = 0;
        let mut count_insert = 0;
        let mut count_delete = 0;
        while (i as usize) < diffs.len() {
            if diffs[i as usize].operation == -1 {
                text_delete += diffs[i as usize].text.as_str();
                count_delete += 1;
                i += 1;
            } else if diffs[i as usize].operation == 1 {
                text_insert += diffs[i as usize].text.as_str();
                count_insert += 1;
                i += 1;
            } else {
                // Upon reaching an equality, check for prior redundancies.
                if count_delete + count_insert > 1 {
                    let mut delete_vec: Vec<char> = text_delete.chars().collect();
                    let mut insert_vec: Vec<char> = text_insert.chars().collect();
                    if count_delete > 0 && count_insert > 0 {
                        // Factor out any common prefixies.
                        let mut commonlength = self.diff_common_prefix(&insert_vec, &delete_vec);
                        if commonlength != 0 {
                            let temp1: String =
                                (&insert_vec)[..(commonlength as usize)].iter().collect();
                            let x = i - count_delete - count_insert - 1;
                            if x >= 0 && diffs[x as usize].operation == 0 {
                                diffs[x as usize] = Diff::new(
                                    diffs[x as usize].operation,
                                    diffs[x as usize].text.clone() + temp1.as_str(),
                                );
                            } else {
                                diffs.insert(0, Diff::new(0, temp1));
                                i += 1;
                            }
                            insert_vec = insert_vec[(commonlength as usize)..].to_vec();
                            delete_vec = delete_vec[(commonlength as usize)..].to_vec();
                        }

                        // Factor out any common suffixies.
                        commonlength = self.diff_common_suffix(&insert_vec, &delete_vec);
                        if commonlength != 0 {
                            let temp1: String = (&insert_vec)
                                [(insert_vec.len() - commonlength as usize)..]
                                .iter()
                                .collect();
                            diffs[i as usize] = Diff::new(
                                diffs[i as usize].operation,
                                temp1 + diffs[i as usize].text.as_str(),
                            );
                            insert_vec =
                                insert_vec[..(insert_vec.len() - commonlength as usize)].to_vec();
                            delete_vec =
                                delete_vec[..(delete_vec.len() - commonlength as usize)].to_vec();
                        }
                    }

                    // Delete the offending records and add the merged ones.
                    i -= count_delete + count_insert;
                    for _j in 0..(count_delete + count_insert) as usize {
                        diffs.remove(i as usize);
                    }
                    if !delete_vec.is_empty() {
                        diffs.insert(i as usize, Diff::new(-1, delete_vec.iter().collect()));
                        i += 1;
                    }
                    if !insert_vec.is_empty() {
                        diffs.insert(i as usize, Diff::new(1, insert_vec.iter().collect()));
                        i += 1;
                    }
                    i += 1;
                } else if i != 0 && diffs[i as usize - 1].operation == 0 {
                    // Merge this equality with the previous one.
                    diffs[i as usize - 1] = Diff::new(
                        diffs[i as usize - 1].operation,
                        diffs[i as usize - 1].text.clone() + diffs[i as usize].text.as_str(),
                    );
                    diffs.remove(i as usize);
                } else {
                    i += 1;
                }
                count_delete = 0;
                text_delete = "".to_string();
                text_insert = "".to_string();
                count_insert = 0;
            }
        }
        // Remove the dummy entry at the end.
        if diffs[diffs.len() - 1].text.is_empty() {
            diffs.pop();
        }

        /*
        Second pass: look for single edits surrounded on both sides by equalities
        which can be shifted sideways to eliminate an equality.
        e.g: A<ins>BA</ins>C -> <ins>AB</ins>AC
        */
        let mut changes = false;
        i = 1;
        // Intentionally ignore the first and last element (don't need checking).
        while (i as usize) < diffs.len() - 1 {
            if diffs[i as usize - 1].operation == 0 && diffs[i as usize + 1].operation == 0 {
                // This is a single edit surrounded by equalities.
                let text_vec: Vec<char> = diffs[i as usize].text.chars().collect();
                let text1_vec: Vec<char> = diffs[i as usize - 1].text.chars().collect();
                let text2_vec: Vec<char> = diffs[i as usize + 1].text.chars().collect();
                if text_vec.ends_with(&text1_vec) {
                    // Shift the edit over the previous equality.
                    if !diffs[i as usize - 1].text.is_empty() {
                        let temp1: String = diffs[i as usize - 1].text.clone();
                        let temp2: String = text_vec[..(text_vec.len() - text1_vec.len())]
                            .iter()
                            .collect();
                        diffs[i as usize].text = temp1 + temp2.as_str();
                        diffs[i as usize + 1].text = diffs[i as usize - 1].text.clone()
                            + diffs[i as usize + 1].text.as_str();
                    }
                    diffs.remove(i as usize - 1);
                    changes = true;
                } else if text_vec.starts_with(&text2_vec) {
                    // Shift the edit over the next equality.
                    diffs[i as usize - 1].text =
                        diffs[i as usize - 1].text.clone() + diffs[i as usize + 1].text.as_str();
                    let temp1: String = text_vec[text2_vec.len()..].iter().collect();
                    diffs[i as usize].text = temp1 + diffs[i as usize + 1].text.as_str();
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

fn dispatch(dmp: &mut Dmp, diffs: &mut Vec<Diff>, pass: fn(&mut Dmp, &mut Vec<Diff>)) {
    #[cfg(feature = "grapheme")]
    {
        if dmp.segmentation == Segmentation::Grapheme {
            let texts: Vec<&str> = diffs.iter().map(|d| d.text.as_str()).collect();
            let mut packer = crate::tokenize::GraphemePacker::new(&texts);
            for diff in diffs.iter_mut() {
                diff.text = packer.pack(&diff.text);
            }
            pass(dmp, diffs);
            packer.unpack_diffs(diffs);
            return;
        }
    }
    pass(dmp, diffs);
}
