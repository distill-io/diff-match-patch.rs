// Patch construction, splitting, padding, apply, and the patch text wire
// format (Patch::to_string / patch_to_text / patch_from_text).

use crate::delta::encode_uri;
use crate::engine;
use crate::types::{max, min, Diff, Dmp, Patch};
use core::char;
use percent_encoding::percent_decode;
use std::fmt;

// The historic public API takes &Vec/&mut Vec/&String; frozen by the
// drop-in compatibility contract.
#[allow(clippy::ptr_arg)]
impl Dmp {
    /// Increase the context until it is unique,
    /// but don't let the pattern expand beyond Match_MaxBits.
    ///
    /// Args:
    /// patch: The patch to grow.
    /// text: Source text.
    pub fn patch_add_context(&mut self, patch: &mut Patch, text: &mut Vec<char>) {
        if text.is_empty() {
            return;
        }
        let mut pattern: Vec<char> =
            text[patch.start2 as usize..(patch.length1 as usize + patch.start2 as usize)].to_vec();
        let mut padding: i32 = 0;

        // Look for the first and last matches of pattern in text.  If two different
        // matches are found, increase the pattern length.
        let mut rst = 0;
        while engine::find_sub(text, &pattern, 0)
            != engine::rfind_sub(text, &pattern, text.len() - 1)
            && (pattern.len() as i32) < (self.match_maxbits - self.patch_margin * 2)
        {
            padding += self.patch_margin;
            pattern = text[max(0, patch.start2 - padding) as usize
                ..min(text.len() as i32, patch.start2 + patch.length1 + padding) as usize]
                .to_vec();
            rst += 1;
            if rst > 5 {
                break;
            }
        }
        // Add one chunk for good luck.
        padding += self.patch_margin;

        // Add the prefix.
        let prefix: String = text[max(0, patch.start2 - padding) as usize..patch.start2 as usize]
            .iter()
            .collect();
        let prefix_length = prefix.chars().count() as i32;
        if !prefix.is_empty() {
            patch.diffs.insert(0, Diff::new(0, prefix.clone()));
        }

        // Add the suffix.
        let suffix: String = text[(patch.start2 + patch.length1) as usize
            ..min(text.len() as i32, patch.start2 + patch.length1 + padding) as usize]
            .iter()
            .collect();
        let suffix_length = suffix.chars().count() as i32;
        if !suffix.is_empty() {
            patch.diffs.push(Diff::new(0, suffix));
        }
        // Roll back the start points.
        patch.start1 -= prefix_length;
        patch.start2 -= prefix_length;
        // Extend lengths.
        patch.length1 += prefix_length + suffix_length;
        patch.length2 += prefix_length + suffix_length;
    }

    /// Compute a list of patches to turn text1 into text2.
    /// compute diffs.
    /// Args:
    /// text1: First string.
    /// text2: Second string.
    /// Returns:
    /// Vector of Patch objects.
    pub fn patch_make1(&mut self, text1: &str, text2: &str) -> Vec<Patch> {
        let mut diffs: Vec<Diff> = self.diff_main(text1, text2, true);
        if diffs.len() > 2 {
            self.diff_cleanup_semantic(&mut diffs);
            self.diff_cleanup_efficiency(&mut diffs);
        }
        self.patch_make4(text1, &mut diffs)
    }

    /// Compute a list of patches to turn text1 into text2.
    /// Use diffs to compute first text.
    ///
    /// Args:
    /// diffs: Vector od diff object.
    /// Returns:
    /// Vector of Patch objects.
    pub fn patch_make2(&mut self, diffs: &mut Vec<Diff>) -> Vec<Patch> {
        let text1 = self.diff_text1(diffs);
        self.patch_make4(text1.as_str(), diffs)
    }

    /// Compute a list of patches to turn text1 into text2.
    ///
    /// Args:
    /// text1: First string.
    /// text2: Second string.
    /// diffs: Vector of diff.
    ///
    /// Returns:
    /// Vector of Patch objects.
    pub fn patch_make3(&mut self, text1: &str, _text2: &str, diffs: &mut Vec<Diff>) -> Vec<Patch> {
        self.patch_make4(text1, diffs)
    }
    /// Compute a list of patches to turn text1 into text2.
    ///
    /// Args:
    /// text1: First string.
    /// diffs: Vector of diff object.
    /// Returns:
    /// Array of Patch objects.
    pub fn patch_make4(&mut self, text1: &str, diffs: &mut Vec<Diff>) -> Vec<Patch> {
        let mut patches: Vec<Patch> = vec![];
        if diffs.is_empty() {
            return patches; // Get rid of the None case.
        }
        let mut patch: Patch = Patch::new(vec![], 0, 0, 0, 0);
        let mut char_count1 = 0; // Number of characters into the text1 string.
        let mut char_count2 = 0; // Number of characters into the text2 string.
        let mut prepatch: Vec<char> = (text1.to_string()).chars().collect(); // Recreate the patches to determine context info.
        let mut postpatch: Vec<char> = (text1.to_string()).chars().collect();
        for i in 0..diffs.len() {
            let temp1: &Vec<char> = &(diffs[i].text.chars().collect());
            if patch.diffs.is_empty() && diffs[i].operation != 0 {
                // A new patch starts here.
                patch.start1 = char_count1;
                patch.start2 = char_count2;
            }
            if diffs[i].operation == 1 {
                // Insertion
                patch
                    .diffs
                    .push(Diff::new(diffs[i].operation, diffs[i].text.clone()));
                let temp: Vec<char> = postpatch[char_count2 as usize..].to_vec();
                postpatch = postpatch[..char_count2 as usize].to_vec();
                patch.length2 += temp1.len() as i32;
                for ch in temp1 {
                    postpatch.push(*ch);
                }
                for ch in temp {
                    postpatch.push(ch);
                }
            } else if diffs[i].operation == -1 {
                // Deletion.
                patch
                    .diffs
                    .push(Diff::new(diffs[i].operation, diffs[i].text.clone()));
                let temp: Vec<char> = postpatch[(temp1.len() + char_count2 as usize)..].to_vec();
                postpatch = postpatch[..char_count2 as usize].to_vec();
                patch.length1 += temp1.len() as i32;
                for ch in &temp {
                    postpatch.push(*ch);
                }
            } else if temp1.len() as i32 <= self.patch_margin * 2
                && !patch.diffs.is_empty()
                && i != diffs.len() - 1
            {
                // Small equality inside a patch.
                patch
                    .diffs
                    .push(Diff::new(diffs[i].operation, diffs[i].text.clone()));
                patch.length1 += temp1.len() as i32;
                patch.length2 += temp1.len() as i32;
            } else if temp1.len() as i32 >= 2 * self.patch_margin {
                // Time for a new patch.
                if !patch.diffs.is_empty() {
                    self.patch_add_context(&mut patch, &mut prepatch);
                    patches.push(patch);
                    patch = Patch::new(vec![], 0, 0, 0, 0);
                    prepatch = postpatch.clone();
                    char_count1 = char_count2;
                }
            }

            // Update the current character count.
            if diffs[i].operation != 1 {
                char_count1 += temp1.len() as i32;
            }
            if diffs[i].operation != -1 {
                char_count2 += temp1.len() as i32;
            }
        }

        // Pick up the leftover patch if not empty.
        if !patch.diffs.is_empty() {
            self.patch_add_context(&mut patch, &mut prepatch);
            // println!("{:?}", prepatch);
            patches.push(patch);
        }
        patches
    }

    /// Given an Vector of patches, return another Vector that is identical.
    ///
    /// Args:
    /// patches: Vector of Patch objects.
    ///
    /// Returns:
    /// Vector of Patch objects.
    pub fn patch_deep_copy(&mut self, patches: &mut Vec<Patch>) -> Vec<Patch> {
        let mut patches_copy: Vec<Patch> = vec![];
        for patches_item in patches {
            let mut patch_copy = Patch::new(vec![], 0, 0, 0, 0);
            for j in 0..patches_item.diffs.len() {
                let diff_copy = Diff::new(
                    patches_item.diffs[j].operation,
                    patches_item.diffs[j].text.clone(),
                );
                patch_copy.diffs.push(diff_copy);
            }
            patch_copy.start1 = patches_item.start1;
            patch_copy.start2 = patches_item.start2;
            patch_copy.length1 = patches_item.length1;
            patch_copy.length2 = patches_item.length2;
            patches_copy.push(patch_copy);
        }
        patches_copy
    }

    pub fn patch_apply(
        &mut self,
        patches: &mut Vec<Patch>,
        source_text: &str,
    ) -> (Vec<char>, Vec<bool>) {
        /*
          Merge a set of patches onto the text.  Return a patched text, as well
          as a list of true/false values indicating which patches were applied.

          Args:
              patches: Vector of Patch objects.
              text: Old text.

          Returns:
              Two element Vector, containing the new chars and an Vector of boolean values.
        */

        if patches.is_empty() {
            return (source_text.chars().collect(), vec![]);
        }

        // Deep copy the patches so that no changes are made to originals.
        let mut patches_copy: Vec<Patch> = self.patch_deep_copy(patches);

        let null_padding: Vec<char> = self.patch_add_padding(&mut patches_copy);

        let mut text = null_padding.clone();
        text.extend(source_text.chars());
        text.extend(&null_padding);

        self.patch_splitmax(&mut patches_copy);

        // delta keeps track of the offset between the expected and actual location
        // of the previous patch.  If there are patches expected at positions 10 and
        // 20, but the first patch was found at 12, delta is 2 and the second patch
        // has an effective expected position of 22.
        let mut delta: i32 = 0;
        let mut results: Vec<bool> = vec![false; patches_copy.len()];
        for x in 0..patches_copy.len() {
            let expected_loc: i32 = patches_copy[x].start2 + delta;
            let text1: Vec<char> = self
                .diff_text1(&mut patches_copy[x].diffs)
                .chars()
                .collect();
            let mut start_loc: i32;
            let mut end_loc = -1;
            if text1.len() as i32 > self.match_maxbits {
                // patch_splitMax will only provide an oversized pattern in the case of
                // a monster delete.
                let first: String = (text[..]).iter().collect();
                let second: String = text1[..self.match_maxbits as usize].iter().collect();
                let second1: String = text1[text1.len() - self.match_maxbits as usize..]
                    .iter()
                    .collect();
                start_loc = self.match_main(first.as_str(), second.as_str(), expected_loc);
                if start_loc != -1 {
                    end_loc = self.match_main(
                        first.as_str(),
                        second1.as_str(),
                        expected_loc + text1.len() as i32 - self.match_maxbits,
                    );
                    if end_loc == -1 || start_loc >= end_loc {
                        // Can't find valid trailing context.  Drop this patch.
                        start_loc = -1;
                    }
                }
            } else {
                let first: String = text[..].iter().collect();
                let second: String = text1[..].iter().collect();
                start_loc = self.match_main(first.as_str(), second.as_str(), expected_loc);
            }
            if start_loc == -1 {
                // No match found.  :(
                results[x] = false;
                // Subtract the delta for this failed patch from subsequent patches.
                delta -= patches_copy[x].length2 - patches_copy[x].length1;
            } else {
                // Found a match.  :)
                results[x] = true;
                delta = start_loc - expected_loc;

                let mut end_index: usize;
                if end_loc == -1 {
                    end_index = start_loc as usize + text1.len();
                } else {
                    end_index = (end_loc + self.match_maxbits) as usize;
                }
                end_index = std::cmp::min(text.len(), end_index);

                let text2: Vec<char> = text[start_loc as usize..end_index].to_vec();

                if text1 == text2 {
                    // Perfect match, just shove the replacement text in.
                    let temp3: String = text[..start_loc as usize].iter().collect();
                    let temp4 = self.diff_text2(&mut patches_copy[x].diffs);
                    let temp5: String = text[(start_loc as usize + text1.len())..].iter().collect();
                    let temp6 = temp3 + temp4.as_str() + temp5.as_str();
                    text = temp6.chars().collect();
                } else {
                    // Imperfect match.
                    // Run a diff to get a framework of equivalent indices.
                    let temp3: String = text1[..].iter().collect();
                    let temp4: String = text2[..].iter().collect();
                    let mut diffs: Vec<Diff> =
                        self.diff_main(temp3.as_str(), temp4.as_str(), false);
                    if text1.len() as i32 > self.match_maxbits
                        && (self.diff_levenshtein(&diffs) as f32 / (text1.len() as f32)
                            > self.patch_delete_threshold)
                    {
                        // The end points match, but the content is unacceptably bad.
                        results[x] = false;
                    } else {
                        self.diff_cleanup_semantic_lossless(&mut diffs);
                        let mut index1: i32 = 0;
                        for y in 0..patches_copy[x].diffs.len() {
                            let mod1 = patches_copy[x].diffs[y].clone();
                            if mod1.operation != 0 {
                                let index2: i32 = self.diff_xindex(&diffs, index1);
                                if mod1.operation == 1 {
                                    // Insertion
                                    let temp3: String =
                                        text[..(start_loc + index2) as usize].iter().collect();
                                    let temp4: String =
                                        text[(start_loc + index2) as usize..].iter().collect();
                                    let temp5 = temp3 + mod1.text.as_str() + temp4.as_str();
                                    text = temp5.chars().collect();
                                } else if mod1.operation == -1 {
                                    // Deletion
                                    let temp3: String =
                                        text[..(start_loc + index2) as usize].iter().collect();
                                    let diffs_text_len = mod1.text.chars().count();
                                    let temp4: String = text[(start_loc
                                        + self.diff_xindex(&diffs, index1 + diffs_text_len as i32))
                                        as usize..]
                                        .iter()
                                        .collect();
                                    let temp5 = temp3 + temp4.as_str();
                                    text = temp5.chars().collect();
                                }
                            }
                            if mod1.operation != -1 {
                                index1 += mod1.text.chars().count() as i32;
                            }
                        }
                    }
                }
            }
        }
        // Strip the padding off.
        text = text[null_padding.len()..(text.len() - null_padding.len())].to_vec();
        (text, results)
    }

    /// Add some padding on text start and end so that edges can match
    /// something.  Intended to be called only from within patch_apply.
    ///
    /// Args:
    /// patches: Array of Patch objects.
    ///
    /// Returns:
    /// The padding chars added to each side.
    pub fn patch_add_padding(&mut self, patches: &mut Vec<Patch>) -> Vec<char> {
        let padding_length = self.patch_margin;
        let mut nullpadding: Vec<char> = vec![];
        for i in 0..padding_length {
            if let Some(ch) = char::from_u32(1 + i as u32) {
                nullpadding.push(ch);
            }
        }

        // Bump all the patches forward.
        for patch in patches.iter_mut() {
            patch.start1 += padding_length;
            patch.start2 += padding_length;
        }
        let mut patch = patches[0].clone();
        let mut diffs = patch.diffs;
        if diffs.is_empty() || diffs[0].operation != 0 {
            // Add nullPadding equality.
            diffs.insert(0, Diff::new(0, nullpadding.clone().iter().collect()));
            patch.start1 -= padding_length; // Should be 0.
            patch.start2 -= padding_length; // Should be 0.
            patch.length1 += padding_length;
            patch.length2 += padding_length;
        } else {
            let text_len = diffs[0].text.chars().count() as i32;
            if padding_length > text_len {
                // Grow first equality.
                let extra_length = padding_length - text_len;
                let mut new_text: String = nullpadding[text_len as usize..].iter().collect();
                new_text += diffs[0].text.as_str();
                diffs[0] = Diff::new(diffs[0].operation, new_text);
                patch.start1 -= extra_length;
                patch.start2 -= extra_length;
                patch.length1 += extra_length;
                patch.length2 += extra_length;
            }
        }

        // Add some padding on end of last diff.
        patch.diffs = diffs;
        patches[0] = patch;
        patch = patches[patches.len() - 1].clone();
        diffs = patch.diffs;
        if diffs.is_empty() || diffs[diffs.len() - 1].operation != 0 {
            // Add nullPadding equality.
            diffs.push(Diff::new(0, nullpadding.clone().iter().collect()));
            patch.length1 += padding_length;
            patch.length2 += padding_length;
        } else {
            let text_len = diffs[diffs.len() - 1].text.chars().count() as i32;
            if padding_length > text_len {
                // Grow last equality.
                let extra_length = padding_length - text_len;
                let mut new_text: String = nullpadding[..extra_length as usize].iter().collect();
                let diffs_len = diffs.len();
                new_text = diffs[diffs_len - 1].text.clone() + new_text.as_str();
                diffs[diffs_len - 1] = Diff::new(diffs[diffs_len - 1].operation, new_text);
                patch.length1 += extra_length;
                patch.length2 += extra_length;
            }
        }
        patch.diffs = diffs;
        let patches_len = patches.len();
        patches[patches_len - 1] = patch;
        nullpadding
    }

    /// Look through the patches and break up any which are longer than the
    /// maximum limit of the match algorithm.
    /// Intended to be called only from within patch_apply.
    ///
    /// Args:
    /// patches: Array of Patch objects.
    pub fn patch_splitmax(&mut self, patches: &mut Vec<Patch>) {
        let patch_size = self.match_maxbits;
        if patch_size == 0 {
            return;
        }
        let mut x: i32 = 0;
        while (x as usize) < patches.len() {
            if patches[x as usize].length1 <= patch_size {
                x += 1;
                continue;
            }
            // Remove the big old patch.
            let mut bigpatch = patches.remove(x as usize);
            x -= 1;
            let mut start1 = bigpatch.start1;
            let mut start2 = bigpatch.start2;
            let mut precontext: Vec<char> = vec![];
            while !bigpatch.diffs.is_empty() {
                // Create one of several smaller patches.
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
                while !bigpatch.diffs.is_empty() && patch.length1 < patch_size - self.patch_margin {
                    let diff_type = bigpatch.diffs[0].operation;
                    let mut diff_text: Vec<char> = bigpatch.diffs[0].text.chars().collect();
                    if diff_type == 1 {
                        // Insertions are harmless.
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
                        // This is a large deletion.  Let it pass in one chunk.
                        patch.length1 += diff_text.len() as i32;
                        start1 += diff_text.len() as i32;
                        empty = false;
                        patch
                            .diffs
                            .push(Diff::new(diff_type, diff_text.iter().collect()));
                        bigpatch.diffs.remove(0);
                    } else {
                        // Deletion or equality.  Only take as much as we can stomach.
                        let diff_text_len: i32 = diff_text.len() as i32;
                        diff_text = diff_text[..min(
                            diff_text_len,
                            patch_size - patch.length1 - self.patch_margin,
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
                            let temp1: Vec<char> = bigpatch.diffs[0].text.chars().collect();
                            bigpatch.diffs[0].text = temp1[diff_text.len()..].iter().collect();
                        }
                    }
                }
                // Compute the head context for the next patch.
                precontext = self.diff_text2(&mut patch.diffs).chars().collect();
                precontext = precontext[(precontext.len()
                    - min(self.patch_margin, precontext.len() as i32) as usize)..]
                    .to_vec();
                // Append the end context for this patch.
                let postcontext = if self.diff_text1(&mut bigpatch.diffs).chars().count() as i32
                    > self.patch_margin
                {
                    let temp: Vec<char> = self.diff_text1(&mut bigpatch.diffs).chars().collect();
                    temp[..self.patch_margin as usize].iter().collect()
                } else {
                    self.diff_text1(&mut bigpatch.diffs)
                };
                let postcontext_len = postcontext.chars().count() as i32;
                if !postcontext.is_empty() {
                    patch.length1 += postcontext_len;
                    patch.length2 += postcontext_len;
                    if !patch.diffs.is_empty() && patch.diffs[patch.diffs.len() - 1].operation == 0
                    {
                        let len = patch.diffs.len();
                        patch.diffs[len - 1].text += postcontext.as_str();
                    } else {
                        patch.diffs.push(Diff::new(0, postcontext));
                    }
                }
                if !empty {
                    x += 1;
                    patches.insert(x as usize, patch);
                }
            }
            x += 1;
        }
    }

    /// Take a list of patches and return a textual representation.
    ///
    /// Args:
    /// patches: Vector of Patch objects.
    ///
    /// Returns:
    /// Text representation of patches.
    pub fn patch_to_text(&mut self, patches: &mut Vec<Patch>) -> String {
        let mut text: String = "".to_string();
        for patches_item in patches {
            text += (patches_item.to_string()).as_str();
        }
        text
    }

    /// Parse a textual representation of patches and return a list of patch
    /// objects.
    ///
    /// Args:
    /// textline: Text representation of patches.
    ///
    /// Returns:
    /// Vector of Patch objects.
    ///
    /// Panics on malformed patch text.
    pub fn patch_from_text(&mut self, textline: String) -> Vec<Patch> {
        try_patch_from_text(&textline).unwrap_or_else(|e| panic!("{}", e))
    }

    pub fn patch1_from_text(&mut self, textline: String) -> Patch {
        // Parse full patch text and return its FIRST patch (any further hunks
        // are ignored); panics on malformed input.
        try_patch_from_text(&textline)
            .unwrap_or_else(|e| panic!("{}", e))
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("Invalid patch string: {}", textline))
    }
}

#[derive(Debug)]
pub(crate) struct PatchError(String);

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One header coordinate pair, with the oracle's exact semantics: "N" means
/// start N-1 length 1, "N,0" keeps the raw start with length 0, "N,L" means
/// start N-1 length L. Only plain digit runs are accepted, like the oracle's
/// `\d+` (no sign, no overflow wrap).
fn parse_coords(part: &str) -> Result<(i32, i32), PatchError> {
    let err = || PatchError(format!("Invalid patch coordinates: {}", part));
    let num = |s: &str| {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(err());
        }
        s.parse::<i32>().map_err(|_| err())
    };
    let (start, length) = part.split_once(',').unwrap_or((part, ""));
    match length {
        "" => Ok((num(start)? - 1, 1)),
        "0" => Ok((num(start)?, 0)),
        _ => Ok((num(start)? - 1, num(length)?)),
    }
}

fn parse_header(line: &str) -> Result<(i32, i32, i32, i32), PatchError> {
    let err = || PatchError(format!("Invalid patch string: {}", line));
    let coords = line
        .strip_prefix("@@ -")
        .and_then(|r| r.strip_suffix(" @@"))
        .ok_or_else(err)?;
    let (c1, c2) = coords.split_once(" +").ok_or_else(err)?;
    let (start1, length1) = parse_coords(c1)?;
    let (start2, length2) = parse_coords(c2)?;
    Ok((start1, length1, start2, length2))
}

pub(crate) fn try_patch_from_text(text: &str) -> Result<Vec<Patch>, PatchError> {
    let mut patches: Vec<Patch> = vec![];
    let lines: Vec<&str> = text.split('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].is_empty() {
            i += 1;
            continue;
        }
        let (start1, length1, start2, length2) = parse_header(lines[i])?;
        let mut patch = Patch::new(vec![], start1, start2, length1, length2);
        i += 1;
        while i < lines.len() {
            let line = lines[i];
            if line.starts_with('@') {
                // Start of next patch.
                break;
            }
            if line.is_empty() {
                // Blank line?  Whatever.
                i += 1;
                continue;
            }
            let mut line_chars = line.chars();
            let sign = line_chars.next().unwrap();
            let body = line_chars.as_str();
            let decoded = percent_decode(body.as_bytes())
                .decode_utf8()
                .map_err(|_| PatchError(format!("Illegal escape in patch_from_text: {}", body)))?
                .to_string();
            match sign {
                '+' => patch.diffs.push(Diff::new(1, decoded)),
                '-' => patch.diffs.push(Diff::new(-1, decoded)),
                ' ' => patch.diffs.push(Diff::new(0, decoded)),
                _ => return Err(PatchError(format!("Invalid patch mode: {}", line))),
            }
            i += 1;
        }
        patches.push(patch);
    }
    Ok(patches)
}

/// Renders the patch text wire format, byte-compatible with the oracle:
/// length 0 keeps the raw start, length 1 omits the length. `to_string()`
/// call sites keep working through the blanket `ToString`.
impl fmt::Display for Patch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let coords = |start: i32, length: i32| match length {
            0 => format!("{},0", start),
            1 => format!("{}", start + 1),
            _ => format!("{},{}", start + 1, length),
        };
        writeln!(
            f,
            "@@ -{} +{} @@",
            coords(self.start1, self.length1),
            coords(self.start2, self.length2)
        )?;
        for diff in &self.diffs {
            let sign = match diff.operation {
                0 => ' ',
                -1 => '-',
                _ => '+',
            };
            writeln!(f, "{}{}", sign, encode_uri(&diff.text))?;
        }
        Ok(())
    }
}

// The historic public API takes &Vec/&mut Vec/&String; frozen by the
// drop-in compatibility contract.
#[allow(clippy::ptr_arg)]
impl Dmp {
    /// split the string accoring to given character
    ///
    /// Args:
    /// text: string we have to split
    /// ch: character by which we have to split string
    ///
    /// Returns:
    /// Vector of string after spliting according to character.
    pub fn split_by_char(&mut self, text: &str, ch: char) -> Vec<String> {
        let temp: Vec<&str> = text.split(ch).collect();
        let mut temp1: Vec<String> = vec![];
        for temp_item in &temp {
            temp1.push(temp_item.to_string());
        }
        temp1
    }

    /// split the string accoring to given characters "@@ ".
    ///
    /// Args:
    /// text: string we have to split
    ///
    /// Returns:
    /// Vector of string after spliting according to characters.
    pub fn split_by_chars(&mut self, text: &str) -> Vec<String> {
        let temp: Vec<&str> = text.split("@@ ").collect();
        let mut temp1: Vec<String> = vec![];
        for temp_item in &temp {
            temp1.push(temp_item.to_string());
        }
        temp1
    }
}
