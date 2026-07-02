#![forbid(unsafe_code)]

//! Functions for diff, match and patch.
//!
//! Computes the difference between two texts to create a patch.
//! Applies the patch onto another text, allowing for errors.
//!
//! All lengths in the delta and patch text wire formats count Unicode
//! scalars, and the outputs are byte-compatible with Neil Fraser's reference
//! diff-match-patch for text where scalar and UTF-16 counts agree (the
//! reference counts UTF-16 code units).
//!
//! ```
//! use diff_match_patch::Dmp;
//!
//! let mut dmp = Dmp::new();
//! let mut diffs = dmp.diff_main("The quick brown fox.", "The quick red fox.", true);
//! assert_eq!(dmp.diff_text1(&mut diffs), "The quick brown fox.");
//! assert_eq!(dmp.diff_text2(&mut diffs), "The quick red fox.");
//!
//! let mut patches = dmp.patch_make1("The quick brown fox.", "The quick red fox.");
//! let (patched, applied) = dmp.patch_apply(&mut patches, "The quick brown fox.");
//! assert_eq!(patched.into_iter().collect::<String>(), "The quick red fox.");
//! assert_eq!(applied, vec![true]);
//! ```
//!
//! With the `grapheme` feature enabled, [`Segmentation::Grapheme`] makes
//! diffs treat extended grapheme clusters (emoji ZWJ sequences, flags,
//! combining marks) as atomic:
//!
#![cfg_attr(feature = "grapheme", doc = "```")]
#![cfg_attr(not(feature = "grapheme"), doc = "```ignore")]
//! use diff_match_patch::{Dmp, Segmentation};
//!
//! let mut dmp = Dmp::new();
//! dmp.segmentation = Segmentation::Grapheme;
//! let diffs = dmp.diff_main("🇷🇺", "🇺🇸", false);
//! // The flags share a scalar but not a cluster: no boundary lands inside one.
//! assert_eq!(diffs.len(), 2);
//! ```

mod cleanup;
mod delta;
mod diff;
mod engine;
mod match_;
mod patch;
mod tokenize;
mod types;

pub use types::{Diff, Dmp, Patch, Segmentation};
