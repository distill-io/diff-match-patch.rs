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
