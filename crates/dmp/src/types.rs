// Public data types (Dmp, Diff, Patch) and crate-internal helpers
// shared by the diff, match and patch modules.

use std::fmt;

/// Which unit of text the diff engine treats as atomic.
// non_exhaustive keeps the cfg-gated Grapheme variant feature-additive:
// downstream matches always need a wildcard arm, so enabling the "grapheme"
// feature (e.g. via Cargo feature unification) cannot break compilation.
#[non_exhaustive]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Segmentation {
    /// One Unicode scalar per token (the historical behavior).
    #[default]
    Char,
    /// One extended grapheme cluster per token: `diff_main`, `diff_linemode`
    /// and the cleanup passes never split a cluster. Low-level char utilities
    /// (`diff_bisect`, `diff_common_prefix`/`suffix`/`overlap`) keep their
    /// char-token contracts regardless of this setting. Packing supports
    /// roughly one million distinct multi-char clusters per call and panics
    /// beyond that.
    #[cfg(feature = "grapheme")]
    Grapheme,
}

pub struct Dmp {
    // Number of seconds to map a diff before giving up (None for infinity).
    pub diff_timeout: Option<f32>,
    // Cost of an empty edit operation in terms of edit characters.
    pub edit_cost: i32,
    /*How far to search for a match (0 = exact location, 1000+ = broad match).
    A match this many characters away from the expected location will add
    1.0 to the score (0.0 is a perfect match).*/
    pub match_distance: i32,
    // Chunk size for context length.
    pub patch_margin: i32,
    /*The number of bits in an int.
    Python has no maximum, thus to disable patch splitting set to 0.
    However to avoid long patches in certain pathological cases, use 32.
    Multiple short patches (using native ints) are much faster than long ones.*/
    pub match_maxbits: i32,
    // At what point is no match declared (0.0 = perfection, 1.0 = very loose).
    pub match_threshold: f32,
    /*When deleting a large block of text (over ~64 characters), how close do
    the contents have to be to match the expected contents. (0.0 = perfection,
    1.0 = very loose).  Note that Match_Threshold controls how closely the
    end points of a delete need to match.*/
    pub patch_delete_threshold: f32,
    // Unit of text the diff engine treats as atomic.
    pub segmentation: Segmentation,
    /*Opt-in word-mode speedup: large edit blocks are diffed over packed word
    tokens first (the word-level analog of the line-mode speedup), then the
    changed words are rediffed character by character. Dramatically faster on
    rename-shaped documents where every line changes by a few characters.
    The output is still a valid diff (it reconstructs both inputs), but edit
    boundaries snap to word boundaries first, so it is NOT byte-identical to
    the reference implementation's output — hence off by default.*/
    pub word_mode: bool,
}

pub struct Diff {
    // diff object
    pub operation: i32,
    pub text: String,
}
pub struct Patch {
    //patch object
    pub diffs: Vec<Diff>,
    pub start1: i32,
    pub start2: i32,
    pub length1: i32,
    pub length2: i32,
}
impl Diff {
    // A new diff diff object created.
    pub fn new(operation: i32, text: String) -> Diff {
        Diff { operation, text }
    }
}

impl PartialEq for Diff {
    // it will return if two diff objects are equal.
    fn eq(&self, other: &Self) -> bool {
        (self.operation == other.operation) & (self.text == other.text)
    }
}

impl PartialEq for Patch {
    // it will return if two patch objects are equal or not.
    fn eq(&self, other: &Self) -> bool {
        (self.diffs == other.diffs)
            & (self.start1 == other.start1)
            & (self.start2 == other.start2)
            & (self.length1 == other.length1)
            & (self.length2 == other.length2)
    }
}

impl Patch {
    // A new diff patch object created.
    pub fn new(diffs: Vec<Diff>, start1: i32, start2: i32, length1: i32, length2: i32) -> Patch {
        Patch {
            diffs,
            start1,
            start2,
            length1,
            length2,
        }
    }
}
pub(crate) fn min(x: i32, y: i32) -> i32 {
    // return minimum element.
    if x > y {
        return y;
    }
    x
}

pub(crate) fn min1(x: f32, y: f32) -> f32 {
    // return minimum element.
    if x > y {
        return y;
    }
    x
}

pub(crate) fn max(x: i32, y: i32) -> i32 {
    // return maximum element.
    if x > y {
        return x;
    }
    y
}

pub(crate) fn find_char(cha: char, text: &[char], start: usize) -> i32 {
    // it will return the first index of a character after a index or return -1 if not found.
    // Chunk-scanned: line packing calls this once per line, making the
    // newline hunt one of the hottest loops in realistic diffs.
    match crate::engine::skip_to(text, start, &cha) {
        Some(i) => i as i32,
        None => -1,
    }
}

/// Token space for the internal diff recursion. The general path runs on
/// chars; when both inputs are ASCII the dispatcher runs the same recursion
/// on bytes instead — a bijection there, so every comparison, index, and
/// length matches and the output is byte-identical, while the text is used
/// zero-copy (no `Vec<char>` materialization, no UTF-8 encode/decode in the
/// line arena, and a quarter of the memory traffic in bisect).
pub(crate) trait DiffToken: Copy + Eq {
    const NEWLINE: Self;
    /// Widen a token run to `char`s: the diff pieces are carried as
    /// `TDiff { data: Vec<char> }` through the cleanup passes so text is
    /// encoded to UTF-8 exactly once, at the final `materialize`. On the u8
    /// (ASCII) recursion this is a byte→char widen; on char it is a copy.
    fn to_tokens(tokens: &[Self]) -> Vec<char>;
    /// Append a token run to the line arena (always UTF-8).
    fn append_to_arena(tokens: &[Self], arena: &mut String);
    /// Word-mode separator, matching `char::is_whitespace` (which on ASCII
    /// is exactly {\t, \n, VT, FF, \r, space} — note `u8::is_ascii_whitespace`
    /// omits VT, so the byte impl cannot delegate to it).
    fn is_word_sep(self) -> bool;
}

impl DiffToken for char {
    const NEWLINE: Self = '\n';
    fn to_tokens(tokens: &[char]) -> Vec<char> {
        tokens.to_vec()
    }
    fn append_to_arena(tokens: &[char], arena: &mut String) {
        arena.extend(tokens.iter());
    }
    fn is_word_sep(self) -> bool {
        self.is_whitespace()
    }
}

impl DiffToken for u8 {
    const NEWLINE: Self = b'\n';
    fn to_tokens(tokens: &[u8]) -> Vec<char> {
        tokens.iter().map(|&b| b as char).collect()
    }
    fn append_to_arena(tokens: &[u8], arena: &mut String) {
        arena.push_str(std::str::from_utf8(tokens).expect("ascii fast path"));
    }
    fn is_word_sep(self) -> bool {
        matches!(self, b'\t' | b'\n' | 0x0B | 0x0C | b'\r' | b' ')
    }
}

/// Internal token-carrying diff piece. The recursion and every cleanup pass
/// operate on these — text stays as a `Vec<char>` run so the redundant
/// UTF-8 encode/decode round-trips that dominated the char-path profiles are
/// gone; the public `Diff` (owned `String`) is built once, at `materialize`.
pub(crate) struct TDiff {
    pub operation: i32,
    pub data: Vec<char>,
}

impl TDiff {
    pub(crate) fn new(operation: i32, data: Vec<char>) -> TDiff {
        TDiff { operation, data }
    }

    /// One equal/delete/insert piece as public `Diff` text (the single
    /// UTF-8 encode point).
    pub(crate) fn into_diff(self) -> Diff {
        Diff::new(self.operation, self.data.iter().collect())
    }
}

impl fmt::Debug for Diff {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\n  {{ {}: {} }}", self.operation, self.text)
    }
}

impl fmt::Debug for Patch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{diffs:\n {:?},\n start1: {},\n start2: {},\n length1: {},\n length2: {} }}",
            self.diffs, self.start1, self.start2, self.length1, self.length2
        )
    }
}

impl Clone for Diff {
    fn clone(&self) -> Self {
        Diff {
            operation: self.operation,
            text: self.text.clone(),
        }
    }
}

impl Clone for Patch {
    fn clone(&self) -> Self {
        Patch {
            diffs: self.diffs.clone(),
            start1: self.start1,
            start2: self.start2,
            length1: self.length1,
            length2: self.length2,
        }
    }
}

impl Default for Dmp {
    fn default() -> Self {
        Self::new()
    }
}

impl Dmp {
    pub fn new() -> Self {
        // it will give a new dmp object.
        Dmp {
            diff_timeout: None,
            patch_delete_threshold: 0.5,
            edit_cost: 0,
            match_distance: 1000,
            patch_margin: 4,
            match_maxbits: 32,
            match_threshold: 0.5,
            segmentation: Segmentation::default(),
            word_mode: false,
        }
    }
}
