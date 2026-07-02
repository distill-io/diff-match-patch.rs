//! Grapheme segmentation mode: diffs never split an extended grapheme cluster,
//! and the wire formats stay identical to char mode (lengths always count
//! Unicode scalars of the original text).

#![cfg(feature = "grapheme")]

use diff_match_patch::{Diff, Dmp, Segmentation};

fn tuples(diffs: &[Diff]) -> Vec<(i32, String)> {
    diffs
        .iter()
        .map(|d| (d.operation, d.text.clone()))
        .collect()
}

/// Family-4 vs family-3 emoji share a scalar-level prefix (the first three
/// people and two ZWJs), so char mode splits the cluster; grapheme mode must
/// treat each family as atomic.
#[test]
fn grapheme_diff_never_splits_zwj_cluster() {
    let t1 = "a\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}b"; // a👩‍👩‍👧‍👦b
    let t2 = "a\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}b"; // a👩‍👩‍👧b

    // Char mode (the default) splits inside the ZWJ sequence.
    let mut char_mode = Dmp::new();
    let char_diffs = char_mode.diff_main(t1, t2, false);
    assert_eq!(
        tuples(&char_diffs),
        vec![
            (
                0,
                "a\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}".to_string()
            ),
            (-1, "\u{200D}\u{1F466}".to_string()),
            (0, "b".to_string()),
        ]
    );

    // Grapheme mode keeps each family cluster whole.
    let mut grapheme_mode = Dmp::new();
    grapheme_mode.segmentation = Segmentation::Grapheme;
    let grapheme_diffs = grapheme_mode.diff_main(t1, t2, false);
    assert_eq!(
        tuples(&grapheme_diffs),
        vec![
            (0, "a".to_string()),
            (
                -1,
                "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}".to_string()
            ),
            (1, "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}".to_string()),
            (0, "b".to_string()),
        ]
    );
}

/// Combining marks stay attached to their base char.
#[test]
fn grapheme_diff_keeps_combining_marks_attached() {
    let t1 = "cafe\u{0301} noir"; // café with combining acute
    let t2 = "cafe\u{0300} noir"; // cafè with combining grave

    let mut d = Dmp::new();
    d.segmentation = Segmentation::Grapheme;
    let diffs = d.diff_main(t1, t2, false);
    assert_eq!(
        tuples(&diffs),
        vec![
            (0, "caf".to_string()),
            (-1, "e\u{0301}".to_string()),
            (1, "e\u{0300}".to_string()),
            (0, " noir".to_string()),
        ]
    );
}

/// For text whose clusters are all single chars, grapheme mode is
/// byte-identical to char mode.
#[test]
fn grapheme_mode_matches_char_mode_on_single_char_clusters() {
    let t1 = "The quick brown fox jumps over the lazy dog.";
    let t2 = "The quick brown cat jumps over the sleepy dog.";

    let mut char_mode = Dmp::new();
    let mut grapheme_mode = Dmp::new();
    grapheme_mode.segmentation = Segmentation::Grapheme;
    assert_eq!(
        tuples(&char_mode.diff_main(t1, t2, false)),
        tuples(&grapheme_mode.diff_main(t1, t2, false))
    );
}

/// Wire formats are segmentation-independent: deltas count Unicode scalars of
/// the original text, so a grapheme-mode delta decodes against the same text1
/// with plain diff_from_delta, and patches apply identically.
#[test]
fn grapheme_wire_formats_stay_scalar() {
    let t1 = "x\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}y flag \u{1F1F7}\u{1F1FA} end";
    let t2 = "x\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}y flag \u{1F1FA}\u{1F1F8} end";

    let mut g = Dmp::new();
    g.segmentation = Segmentation::Grapheme;
    let mut diffs = g.diff_main(t1, t2, false);

    // Delta round-trips through the scalar decoder.
    let delta = g.diff_todelta(&mut diffs);
    let decoded = g.diff_from_delta(t1, &delta);
    assert_eq!(tuples(&decoded), tuples(&diffs));

    // Reconstruction from the diff itself.
    assert_eq!(g.diff_text1(&mut diffs), t1);
    assert_eq!(g.diff_text2(&mut diffs), t2);

    // Patches built from grapheme diffs apply cleanly.
    let mut patches = g.patch_make1(t1, t2);
    let (applied, results) = g.patch_apply(&mut patches, t1);
    let applied: String = applied.into_iter().collect();
    assert_eq!(applied, t2);
    assert!(results.iter().all(|&r| r));
}

/// The cleanup passes must also respect cluster boundaries in grapheme mode.
/// 🇷🇺 (RI-R, RI-U) deleted and 🇺🇸 (RI-U, RI-S) inserted share the scalar
/// RI-U: char mode's overlap trimming splits both flags on it, grapheme mode
/// must leave the clusters whole.
#[test]
fn grapheme_cleanup_semantic_respects_clusters() {
    let make = || {
        vec![
            Diff::new(-1, "\u{1F1F7}\u{1F1FA}".to_string()),
            Diff::new(1, "\u{1F1FA}\u{1F1F8}".to_string()),
        ]
    };

    // Char mode extracts the shared regional indicator, splitting both flags.
    let mut char_mode = Dmp::new();
    let mut char_diffs = make();
    char_mode.diff_cleanup_semantic(&mut char_diffs);
    assert_eq!(
        tuples(&char_diffs),
        vec![
            (-1, "\u{1F1F7}".to_string()),
            (0, "\u{1F1FA}".to_string()),
            (1, "\u{1F1F8}".to_string()),
        ]
    );

    // Grapheme mode sees two distinct atomic clusters: nothing to extract.
    let mut grapheme_mode = Dmp::new();
    grapheme_mode.segmentation = Segmentation::Grapheme;
    let mut grapheme_diffs = make();
    grapheme_mode.diff_cleanup_semantic(&mut grapheme_diffs);
    assert_eq!(
        tuples(&grapheme_diffs),
        vec![
            (-1, "\u{1F1F7}\u{1F1FA}".to_string()),
            (1, "\u{1F1FA}\u{1F1F8}".to_string()),
        ]
    );
}
