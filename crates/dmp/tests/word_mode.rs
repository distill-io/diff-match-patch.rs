// Opt-in word mode. The contract is validity — the diff must reconstruct
// both inputs — not byte-identity with the reference implementation (word
// mode deliberately snaps edit boundaries to word boundaries first, which is
// why it is off by default and the oracle corpus never sees it).

use diff_match_patch::Dmp;

/// A rename-shaped fixture: every line changes by a few characters, the
/// worst case for line mode and the case word mode exists for.
fn rename_pair() -> (String, String) {
    let mut t1 = String::new();
    for i in 0..40 {
        t1.push_str(&format!(
            "    let metric_{i:02} = rows[{}].metric * cfg.metric_scale + offset_{i:02};\n",
            i % 7
        ));
    }
    let t2 = t1.replace("metric", "aggregate_value");
    (t1, t2)
}

fn assert_valid_diff(dmp: &mut Dmp, t1: &str, t2: &str, checklines: bool) {
    let mut diffs = dmp.diff_main(t1, t2, checklines);
    assert!(!diffs.is_empty());
    assert_eq!(dmp.diff_text1(&mut diffs), t1, "diff must rebuild text1");
    assert_eq!(dmp.diff_text2(&mut diffs), t2, "diff must rebuild text2");
}

#[test]
fn word_mode_is_off_by_default() {
    assert!(!Dmp::new().word_mode);
}

#[test]
fn word_mode_diffs_reconstruct_both_inputs() {
    let (t1, t2) = rename_pair();
    let mut dmp = Dmp::new();
    dmp.word_mode = true;
    assert_valid_diff(&mut dmp, &t1, &t2, true);
    assert_valid_diff(&mut dmp, &t1, &t2, false);
}

#[test]
fn word_mode_finds_the_unchanged_words() {
    // The point of the mode: shared words between changed lines survive as
    // equalities instead of dissolving into one giant replacement.
    let (t1, t2) = rename_pair();
    let mut dmp = Dmp::new();
    dmp.word_mode = true;
    let diffs = dmp.diff_main(&t1, &t2, true);
    let equal_chars: usize = diffs
        .iter()
        .filter(|d| d.operation == 0)
        .map(|d| d.text.chars().count())
        .sum();
    // "rows", "cfg", "let", the operators and indentation are all shared.
    assert!(
        equal_chars * 2 > t1.chars().count(),
        "most of the text is unchanged words; got {equal_chars} equal chars"
    );
}

#[test]
fn word_mode_patch_roundtrip() {
    let (t1, t2) = rename_pair();
    let mut dmp = Dmp::new();
    dmp.word_mode = true;
    let mut patches = dmp.patch_make1(&t1, &t2);
    let (out, ok) = dmp.patch_apply(&mut patches, &t1);
    assert!(ok.iter().all(|&x| x), "all patches must apply");
    assert_eq!(out.into_iter().collect::<String>(), t2);
}

#[test]
fn word_mode_survives_whitespace_free_text() {
    // A block with no whitespace packs into a single word token; the rediff
    // must fall through to the char diff instead of recursing onto itself.
    let a: String = "abcdefghij".repeat(30);
    let b: String = format!("{}XYZ{}", "abcdefghij".repeat(14), "abcdefghij".repeat(16));
    let mut dmp = Dmp::new();
    dmp.word_mode = true;
    assert_valid_diff(&mut dmp, &a, &b, true);
    assert_valid_diff(&mut dmp, &a, &b, false);
}

#[test]
fn word_mode_trivial_cases() {
    let mut dmp = Dmp::new();
    dmp.word_mode = true;
    let t = "the same text with words ".repeat(10);
    let diffs = dmp.diff_main(&t, &t, true);
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].operation, 0);
    assert_eq!(diffs[0].text, t);

    let diffs = dmp.diff_main("", &t, true);
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].operation, 1);
}

#[cfg(feature = "grapheme")]
#[test]
fn word_mode_stays_cluster_atomic_in_grapheme_mode() {
    use diff_match_patch::Segmentation;
    let family = "👨\u{200d}👩\u{200d}👧";
    let t1 = format!("greeting {family} and farewell words here ").repeat(8);
    let t2 = t1.replace("farewell", "welcome");
    let mut dmp = Dmp::new();
    dmp.word_mode = true;
    dmp.segmentation = Segmentation::Grapheme;
    let mut diffs = dmp.diff_main(&t1, &t2, true);
    assert_eq!(dmp.diff_text1(&mut diffs), t1);
    assert_eq!(dmp.diff_text2(&mut diffs), t2);
    // No diff may tear the ZWJ cluster: the inputs only contain ZWJs inside
    // whole families, so every ZWJ in a diff must belong to a whole family.
    for d in &diffs {
        assert_eq!(
            d.text.matches(family).count() * 2,
            d.text.matches('\u{200d}').count(),
            "cluster torn in {:?}",
            d.text
        );
    }
}
