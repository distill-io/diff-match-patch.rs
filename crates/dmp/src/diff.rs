// Diff orchestration: diff_main and its recursion (compute, line mode, bisect
// splitting), the cleanup passes, and diff-derived metrics. The token-generic
// primitives live in engine.rs; text materialization happens here.

use crate::engine;
use crate::types::{Diff, DiffToken, Dmp, TDiff};
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
                let mut scratch = Vec::new();
                let mut diffs = main_internal(
                    self,
                    &packed1,
                    &packed2,
                    checklines,
                    true,
                    deadline,
                    &mut scratch,
                );
                packer.unpack_diffs(&mut diffs);
                return diffs;
            }
        }
        main_internal(
            self,
            text1,
            text2,
            checklines,
            true,
            deadline,
            &mut Vec::new(),
        )
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
        materialize(line_mode(self, text1, text2, deadline, &mut Vec::new()))
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
        materialize(bisect_diff(
            self,
            char1,
            char2,
            true,
            deadline,
            &mut Vec::new(),
        ))
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

/// Build the public `Diff` list from the internal token pieces: this is the
/// single point where text is encoded to UTF-8 `String`, once per final piece.
fn materialize(tokens: Vec<TDiff>) -> Vec<Diff> {
    tokens.into_iter().map(TDiff::into_diff).collect()
}

fn main_internal(
    dmp: &mut Dmp,
    text1: &str,
    text2: &str,
    checklines: bool,
    allow_words: bool,
    deadline: Option<Instant>,
    scratch: &mut Vec<i32>,
) -> Vec<Diff> {
    // Trivial outcomes build their single `Diff` straight from the `&str`
    // (one copy), skipping the token round-trip: decoding the whole input to
    // a char run only to re-encode it is pure overhead on identical or
    // empty-vs-full inputs.
    if text1.is_empty() && text2.is_empty() {
        return vec![];
    } else if text1.is_empty() {
        return vec![Diff::new(1, text2.to_string())];
    } else if text2.is_empty() {
        return vec![Diff::new(-1, text1.to_string())];
    }
    if text1 == text2 {
        return vec![Diff::new(0, text1.to_string())];
    }
    materialize(diff_str_tokens(
        dmp,
        text1,
        text2,
        checklines,
        allow_words,
        deadline,
        scratch,
    ))
}

/// The diff core over two `&str`s, returning token pieces (text not yet
/// encoded). Splits the ASCII fast path from the general char path; every
/// caller that stays in token space (line/word mode, rediff) uses this
/// instead of `main_internal` so no intermediate `String` is built.
fn diff_str_tokens(
    dmp: &mut Dmp,
    text1: &str,
    text2: &str,
    checklines: bool,
    allow_words: bool,
    deadline: Option<Instant>,
    scratch: &mut Vec<i32>,
) -> Vec<TDiff> {
    // str-level fast paths: the trivial outcomes skip the char materialization.
    if text1.is_empty() && text2.is_empty() {
        return vec![];
    } else if text1.is_empty() {
        return vec![TDiff::new(1, text2.chars().collect())];
    } else if text2.is_empty() {
        return vec![TDiff::new(-1, text1.chars().collect())];
    }
    if text1 == text2 {
        return vec![TDiff::new(0, text1.chars().collect())];
    }

    // ASCII fast path: bytes are a bijection with chars there, so the same
    // recursion over `&[u8]` produces the identical diff with zero-copy
    // inputs, no UTF-8 round-trips, and a quarter of the token traffic.
    // The bijection cares only about token identity, not meaning, so even
    // packed placeholder texts (ids happen to be < 128 for small documents)
    // are eligible.
    if text1.is_ascii() && text2.is_ascii() {
        return main_slices(
            dmp,
            text1.as_bytes(),
            text2.as_bytes(),
            checklines,
            allow_words,
            deadline,
            scratch,
        );
    }

    let char1: Vec<char> = text1.chars().collect();
    let char2: Vec<char> = text2.chars().collect();
    main_slices(
        dmp,
        &char1,
        &char2,
        checklines,
        allow_words,
        deadline,
        scratch,
    )
}

/// The diff core over two `&[char]`s (crate internals that already hold char
/// buffers: patch_apply, the rediff blocks). Applies the same ASCII gate as
/// `diff_str_tokens` — an all-ASCII pair recurses over bytes — and returns
/// token pieces.
fn diff_char_tokens(
    dmp: &mut Dmp,
    old: &[char],
    new: &[char],
    checklines: bool,
    allow_words: bool,
    deadline: Option<Instant>,
    scratch: &mut Vec<i32>,
) -> Vec<TDiff> {
    if old.is_empty() && new.is_empty() {
        return vec![];
    } else if old.is_empty() {
        return vec![TDiff::new(1, new.to_vec())];
    } else if new.is_empty() {
        return vec![TDiff::new(-1, old.to_vec())];
    }
    if old == new {
        return vec![TDiff::new(0, old.to_vec())];
    }
    if old.iter().all(char::is_ascii) && new.iter().all(char::is_ascii) {
        let b1: Vec<u8> = old.iter().map(|&c| c as u8).collect();
        let b2: Vec<u8> = new.iter().map(|&c| c as u8).collect();
        return main_slices(dmp, &b1, &b2, checklines, allow_words, deadline, scratch);
    }
    main_slices(dmp, old, new, checklines, allow_words, deadline, scratch)
}

/// `diff_main` over char slices, for crate internals that already hold char
/// buffers (patch_apply): same deadline setup, same grapheme dispatch —
/// grapheme mode round-trips through `diff_main` to keep cluster packing —
/// but the char-mode path skips the String materialization entirely.
pub(crate) fn diff_main_chars(
    dmp: &mut Dmp,
    old: &[char],
    new: &[char],
    checklines: bool,
) -> Vec<Diff> {
    #[cfg(feature = "grapheme")]
    {
        if dmp.segmentation == crate::types::Segmentation::Grapheme {
            let t1: String = old.iter().collect();
            let t2: String = new.iter().collect();
            return dmp.diff_main(&t1, &t2, checklines);
        }
    }
    let deadline = dmp.deadline_from_now();
    materialize(diff_char_tokens(
        dmp,
        old,
        new,
        checklines,
        true,
        deadline,
        &mut Vec::new(),
    ))
}

/// The diff core over char slices. All recursion — bisect splits, half-match
/// halves — stays in token space; text becomes a String only when a Diff is
/// emitted. Materializing per recursion level instead costs O(N·D) copies,
/// which is what the realistic diff benchmarks are most sensitive to.
///
/// `allow_words` gates the opt-in word-mode speedup: packed-token diffs and
/// word-level rediffs pass false, both because packed placeholder chars must
/// never be re-tokenized (some ids alias whitespace chars) and because a
/// whitespace-free block would recurse onto itself.
fn main_slices<T: DiffToken>(
    dmp: &mut Dmp,
    old: &[T],
    new: &[T],
    checklines: bool,
    allow_words: bool,
    deadline: Option<Instant>,
    scratch: &mut Vec<i32>,
) -> Vec<TDiff> {
    // check for empty text
    if old.is_empty() && new.is_empty() {
        return vec![];
    } else if old.is_empty() {
        return vec![TDiff::new(1, T::to_tokens(new))];
    } else if new.is_empty() {
        return vec![TDiff::new(-1, T::to_tokens(old))];
    }

    // check for equality
    if old == new {
        return vec![TDiff::new(0, T::to_tokens(old))];
    }

    // Trim off common prefix and suffix (speedup).
    let prefix = engine::common_prefix(old, new);
    let suffix = engine::common_suffix(&old[prefix..], &new[prefix..]);
    let mid1 = &old[prefix..old.len() - suffix];
    let mid2 = &new[prefix..new.len() - suffix];

    let mut diffs: Vec<TDiff> = Vec::new();
    // Restore the prefix, compute the diff on the middle block, restore the suffix.
    if prefix > 0 {
        diffs.push(TDiff::new(0, T::to_tokens(&old[..prefix])));
    }
    diffs.extend(compute(
        dmp,
        mid1,
        mid2,
        checklines,
        allow_words,
        deadline,
        scratch,
    ));
    if suffix > 0 {
        diffs.push(TDiff::new(0, T::to_tokens(&old[old.len() - suffix..])));
    }
    dmp.diff_cleanup_merge_impl(&mut diffs);
    diffs
}

/// Find the differences between two token slices that share no common prefix
/// or suffix and are not both empty.
fn compute<T: DiffToken>(
    dmp: &mut Dmp,
    old: &[T],
    new: &[T],
    checklines: bool,
    allow_words: bool,
    deadline: Option<Instant>,
    scratch: &mut Vec<i32>,
) -> Vec<TDiff> {
    if old.is_empty() {
        // Just add some text (speedup).
        return vec![TDiff::new(1, T::to_tokens(new))];
    }
    if new.is_empty() {
        // Just delete some text (speedup).
        return vec![TDiff::new(-1, T::to_tokens(old))];
    }
    {
        let (long, short) = if old.len() >= new.len() {
            (old, new)
        } else {
            (new, old)
        };
        if let Some(i) = engine::contains(long, short) {
            // Shorter text is inside the longer text (speedup).
            let op = if old.len() > new.len() { -1 } else { 1 };
            let mut diffs: Vec<TDiff> = Vec::new();
            if i != 0 {
                diffs.push(TDiff::new(op, T::to_tokens(&long[..i])));
            }
            diffs.push(TDiff::new(0, T::to_tokens(short)));
            if i + short.len() != long.len() {
                diffs.push(TDiff::new(op, T::to_tokens(&long[i + short.len()..])));
            }
            return diffs;
        }
        if short.len() == 1 {
            // Single character string.
            // After the previous speedup, the character can't be an equality.
            return vec![
                TDiff::new(-1, T::to_tokens(old)),
                TDiff::new(1, T::to_tokens(new)),
            ];
        }
    }

    // Check to see if the problem can be split in two (only when a deadline is
    // set: half-match trades optimality for speed).
    if deadline.is_some() {
        if let Some(hm) = engine::half_match(old, new) {
            // A half-match was found, send both pairs off for separate processing.
            let mid_common: Vec<char> = T::to_tokens(&old[hm.old_a..hm.old_a + hm.common]);
            let mut diffs = main_slices(
                dmp,
                &old[..hm.old_a],
                &new[..hm.new_a],
                checklines,
                allow_words,
                deadline,
                scratch,
            );
            diffs.push(TDiff::new(0, mid_common));
            diffs.extend(main_slices(
                dmp,
                &old[hm.old_a + hm.common..],
                &new[hm.new_a + hm.common..],
                checklines,
                allow_words,
                deadline,
                scratch,
            ));
            return diffs;
        }
    }

    if checklines && old.len() > 100 && new.len() > 100 {
        return line_mode(dmp, old, new, deadline, scratch);
    }
    if allow_words && dmp.word_mode && old.len() > 100 && new.len() > 100 {
        return word_mode(dmp, old, new, deadline, scratch);
    }
    bisect_diff(dmp, old, new, allow_words, deadline, scratch)
}

/// Split on the Myers middle snake and recurse, or emit delete+insert when
/// there is no overlap (or the deadline expired).
fn bisect_diff<T: DiffToken>(
    dmp: &mut Dmp,
    old: &[T],
    new: &[T],
    allow_words: bool,
    deadline: Option<Instant>,
    scratch: &mut Vec<i32>,
) -> Vec<TDiff> {
    match engine::bisect(old, new, deadline, scratch) {
        Some((x, y)) => {
            // Compute both diffs serially on the split halves.
            let mut diffs = main_slices(
                dmp,
                &old[..x],
                &new[..y],
                false,
                allow_words,
                deadline,
                scratch,
            );
            diffs.extend(main_slices(
                dmp,
                &old[x..],
                &new[y..],
                false,
                allow_words,
                deadline,
                scratch,
            ));
            diffs
        }
        None => {
            // Number of diffs equals number of characters, no commonality at all.
            vec![
                TDiff::new(-1, T::to_tokens(old)),
                TDiff::new(1, T::to_tokens(new)),
            ]
        }
    }
}

/// Line-mode speedup: diff on packed line ids first, then rediff the
/// replacement blocks character by character.
fn line_mode<T: DiffToken>(
    dmp: &mut Dmp,
    old: &[T],
    new: &[T],
    deadline: Option<Instant>,
    scratch: &mut Vec<i32>,
) -> Vec<TDiff> {
    let mut diffs: Vec<TDiff> =
        if crate::tokenize::packs_to_one_line(old) && crate::tokenize::packs_to_one_line(new) {
            // Single-line inputs pack to one fresh token each (the texts differ,
            // so the ids do too), and the packed diff of two distinct single
            // chars is deterministically delete+insert. Start from the
            // rehydrated form directly and skip the packing.
            vec![
                TDiff::new(-1, T::to_tokens(old)),
                TDiff::new(1, T::to_tokens(new)),
            ]
        } else {
            // Scan the text on a line-by-line basis first.
            let (text3, text4, store) = crate::tokenize::lines_tochars_arena(old, new);

            // Packed placeholder chars must never be re-tokenized: no line mode
            // (checklines = false) and no word mode (allow_words = false).
            let mut diffs: Vec<TDiff> =
                diff_str_tokens(dmp, &text3, &text4, false, false, deadline, scratch);

            // Convert the diff back to original text.
            crate::tokenize::chars_tolines_arena(&mut diffs, &store);
            diffs
        };
    // Eliminate freak matches (e.g. blank lines)
    dmp.diff_cleanup_semantic_impl(&mut diffs);

    // Rediff any replacement blocks, this time character-by-character —
    // where the opt-in word mode may engage on large blocks.
    rediff_blocks(dmp, diffs, true, deadline, scratch)
}

/// Word-mode speedup (opt-in via `Dmp::word_mode`): the word-level analog of
/// line mode. Pack unique words into tokens, diff in word space, then rediff
/// the replacement blocks character by character. The output reconstructs
/// both inputs exactly like line mode's does, but edit boundaries snap to
/// word boundaries first, so it is not byte-identical to the reference
/// implementation's char-level diff.
fn word_mode<T: DiffToken>(
    dmp: &mut Dmp,
    old: &[T],
    new: &[T],
    deadline: Option<Instant>,
    scratch: &mut Vec<i32>,
) -> Vec<TDiff> {
    let (text3, text4, store) = crate::tokenize::words_tochars_arena(old, new);

    let mut diffs: Vec<TDiff> =
        diff_str_tokens(dmp, &text3, &text4, false, false, deadline, scratch);

    crate::tokenize::chars_tolines_arena(&mut diffs, &store);
    // Unlike line mode, NO semantic cleanup before the rediff: word-level
    // equalities between changes are short, the pass would eliminate them
    // and collapse the diff back into one giant block — whose char-level
    // rediff is exactly the cost this mode exists to avoid. Callers run
    // their own cleanup passes on the final diff.

    // Word-level rediffs must not re-enter word mode: a whitespace-free
    // block packs into a single token and would recurse onto itself.
    rediff_blocks(dmp, diffs, false, deadline, scratch)
}

/// Shared tail of the token-mode speedups: rediff every replacement block of
/// `diffs` at the next-finer granularity.
fn rediff_blocks(
    dmp: &mut Dmp,
    mut diffs: Vec<TDiff>,
    allow_words: bool,
    deadline: Option<Instant>,
    scratch: &mut Vec<i32>,
) -> Vec<TDiff> {
    // Add a dummy entry at the end.
    diffs.push(TDiff::new(0, vec![]));
    let mut count_delete = 0;
    let mut count_insert = 0;
    let mut text_delete: Vec<char> = vec![];
    let mut text_insert: Vec<char> = vec![];
    let mut pointer = 0;
    let mut temp: Vec<TDiff> = vec![];
    while pointer < diffs.len() {
        if diffs[pointer].operation == 1 {
            count_insert += 1;
            text_insert.extend_from_slice(&diffs[pointer].data);
        } else if diffs[pointer].operation == -1 {
            count_delete += 1;
            text_delete.extend_from_slice(&diffs[pointer].data);
        } else {
            // Upon reaching an equality, check for prior redundancies.
            if count_delete >= 1 && count_insert >= 1 {
                // Delete the offending records and add the merged ones.
                let sub_diff = diff_char_tokens(
                    dmp,
                    &text_delete,
                    &text_insert,
                    false,
                    allow_words,
                    deadline,
                    scratch,
                );
                for z in sub_diff {
                    temp.push(z);
                }
                temp.push(TDiff::new(
                    diffs[pointer].operation,
                    diffs[pointer].data.clone(),
                ));
            } else {
                if !text_delete.is_empty() {
                    temp.push(TDiff::new(-1, std::mem::take(&mut text_delete)));
                }
                if !text_insert.is_empty() {
                    temp.push(TDiff::new(1, std::mem::take(&mut text_insert)));
                }
                temp.push(TDiff::new(
                    diffs[pointer].operation,
                    diffs[pointer].data.clone(),
                ));
            }
            count_delete = 0;
            count_insert = 0;
            text_delete = vec![];
            text_insert = vec![];
        }
        pointer += 1;
    }
    temp.pop(); //Remove the dummy entry at the end.
    temp
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dispatcher's u8 fast path must be indistinguishable from the char
    /// recursion on ASCII inputs (it is a bijection; this pins it).
    #[test]
    fn ascii_byte_path_matches_char_path() {
        let long_multiline = (0..60)
            .map(|i| format!("row {i}: alpha beta gamma delta\n"))
            .collect::<String>();
        let long_multiline_edited = long_multiline.replace("beta", "BETA");
        let long_single = "words and more words ".repeat(12);
        let long_single_edited = format!("{}TAIL", &long_single[..long_single.len() - 6]);
        let cases: &[(&str, &str)] = &[
            (
                "The quick brown fox jumps over the lazy dog.",
                "The quick red fox leaps over the sleepy dog!",
            ),
            ("abcdefghij", "abcxyzghij"),
            (&long_multiline, &long_multiline_edited),
            (&long_single, &long_single_edited),
        ];
        for word_mode in [false, true] {
            for (t1, t2) in cases {
                let mut dmp = Dmp::new();
                dmp.word_mode = word_mode;
                let byte_diff = main_slices(
                    &mut dmp,
                    t1.as_bytes(),
                    t2.as_bytes(),
                    true,
                    true,
                    None,
                    &mut Vec::new(),
                );
                let c1: Vec<char> = t1.chars().collect();
                let c2: Vec<char> = t2.chars().collect();
                let char_diff = main_slices(&mut dmp, &c1, &c2, true, true, None, &mut Vec::new());
                assert_eq!(byte_diff.len(), char_diff.len(), "{t1:?} vs {t2:?}");
                for (b, c) in byte_diff.iter().zip(char_diff.iter()) {
                    assert_eq!(b.operation, c.operation, "{t1:?} vs {t2:?}");
                    assert_eq!(b.data, c.data, "{t1:?} vs {t2:?}");
                }
            }
        }
    }
}
