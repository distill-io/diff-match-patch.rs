//! Property tests over LCG-generated inputs (self-contained; no external
//! fuzzing crate). Each run is deterministic from the fixed seeds.

use diff_match_patch::{Diff, Dmp};

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

fn rebuild(diffs: &[Diff]) -> (String, String) {
    let mut t1 = String::new();
    let mut t2 = String::new();
    for d in diffs {
        if d.operation != 1 {
            t1 += &d.text;
        }
        if d.operation != -1 {
            t2 += &d.text;
        }
    }
    (t1, t2)
}

fn gen_text(rng: &mut Lcg, alphabet: &[&str], len: usize) -> String {
    (0..len)
        .map(|_| alphabet[(rng.next() as usize) % alphabet.len()])
        .collect()
}

/// Diffs reconstruct their inputs; deltas and patches round-trip (char mode).
#[test]
fn char_mode_roundtrips() {
    let alphabet = ["a", "b", "c", "\n", " ", "%", "@"];
    let mut rng = Lcg(7);
    for _ in 0..400 {
        let len = (rng.next() % 80) as usize;
        let t1 = gen_text(&mut rng, &alphabet, len);
        let len = (rng.next() % 80) as usize;
        let t2 = gen_text(&mut rng, &alphabet, len);
        let mut d = Dmp::new();
        let mut diffs = d.diff_main(&t1, &t2, rng.next().is_multiple_of(2));
        assert_eq!(rebuild(&diffs), (t1.clone(), t2.clone()));

        let delta = d.diff_todelta(&mut diffs);
        let decoded = d.diff_from_delta(&t1, &delta);
        assert_eq!(decoded, diffs);

        let mut patches = d.patch_make1(&t1, &t2);
        let text = d.patch_to_text(&mut patches);
        let mut reparsed = d.patch_from_text(text);
        let (applied, _) = d.patch_apply(&mut reparsed, &t1);
        let applied: String = applied.into_iter().collect();
        assert_eq!(applied, t2);
    }
}

#[cfg(feature = "grapheme")]
mod grapheme_props {
    use super::*;
    use diff_match_patch::Segmentation;
    use unicode_segmentation::UnicodeSegmentation;

    /// Splitting an extended grapheme cluster increases the total cluster
    /// count, so "sum of parts equals the whole" proves no diff boundary ever
    /// lands inside a cluster.
    fn grapheme_count(text: &str) -> usize {
        text.graphemes(true).count()
    }

    #[test]
    fn grapheme_mode_never_splits_clusters() {
        // Cluster-heavy alphabet: ZWJ family, flags, combining marks, ASCII.
        let alphabet = [
            "a",
            "b",
            "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
            "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}",
            "\u{1F1F7}\u{1F1FA}",
            "\u{1F1FA}\u{1F1F8}",
            "e\u{0301}",
            "e\u{0300}",
            " ",
            "\n",
        ];
        let mut rng = Lcg(99);
        for _ in 0..400 {
            let len = (rng.next() % 40) as usize;
            let t1 = gen_text(&mut rng, &alphabet, len);
            let len = (rng.next() % 40) as usize;
            let t2 = gen_text(&mut rng, &alphabet, len);
            let mut d = Dmp::new();
            d.segmentation = Segmentation::Grapheme;
            // checklines varies: the line-mode speedup must also stay cluster-atomic.
            let mut diffs = d.diff_main(&t1, &t2, rng.next().is_multiple_of(2));
            assert_eq!(rebuild(&diffs), (t1.clone(), t2.clone()));

            let parts1: usize = diffs
                .iter()
                .filter(|d| d.operation != 1)
                .map(|d| grapheme_count(&d.text))
                .sum();
            let parts2: usize = diffs
                .iter()
                .filter(|d| d.operation != -1)
                .map(|d| grapheme_count(&d.text))
                .sum();
            assert_eq!(
                parts1,
                grapheme_count(&t1),
                "a diff boundary split a cluster of text1"
            );
            assert_eq!(
                parts2,
                grapheme_count(&t2),
                "a diff boundary split a cluster of text2"
            );

            // The cleanups must preserve the invariant too.
            d.diff_cleanup_semantic(&mut diffs);
            let parts1: usize = diffs
                .iter()
                .filter(|d| d.operation != 1)
                .map(|d| grapheme_count(&d.text))
                .sum();
            assert_eq!(
                parts1,
                grapheme_count(&t1),
                "cleanup_semantic split a cluster"
            );
            assert_eq!(rebuild(&diffs).0, t1);
            assert_eq!(rebuild(&diffs).1, t2);
        }
    }

    /// Grapheme-mode diffs feed the same scalar wire formats: deltas decode
    /// with the plain scalar decoder and patches apply to the original text.
    #[test]
    fn grapheme_wire_roundtrips() {
        let alphabet = [
            "x",
            "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}",
            "\u{1F1F7}\u{1F1FA}",
            "o\u{0308}",
            " ",
        ];
        let mut rng = Lcg(1234);
        for _ in 0..200 {
            let len = (rng.next() % 30) as usize;
            let t1 = gen_text(&mut rng, &alphabet, len);
            let len = (rng.next() % 30) as usize;
            let t2 = gen_text(&mut rng, &alphabet, len);
            let mut d = Dmp::new();
            d.segmentation = Segmentation::Grapheme;
            let mut diffs = d.diff_main(&t1, &t2, false);

            let delta = d.diff_todelta(&mut diffs);
            let decoded = d.diff_from_delta(&t1, &delta);
            assert_eq!(decoded, diffs);

            let mut patches = d.patch_make1(&t1, &t2);
            let patch_text = d.patch_to_text(&mut patches);
            let mut reparsed = d.patch_from_text(patch_text);
            let (applied, _) = d.patch_apply(&mut reparsed, &t1);
            let applied: String = applied.into_iter().collect();
            assert_eq!(applied, t2);
        }
    }

    /// On text whose clusters are all single scalars, grapheme mode produces
    /// byte-identical diffs to char mode.
    #[test]
    fn grapheme_mode_equals_char_mode_on_ascii() {
        let alphabet = ["a", "b", "c", " ", "\n"];
        let mut rng = Lcg(4242);
        for _ in 0..300 {
            let len = (rng.next() % 60) as usize;
            let t1 = gen_text(&mut rng, &alphabet, len);
            let len = (rng.next() % 60) as usize;
            let t2 = gen_text(&mut rng, &alphabet, len);
            let mut char_mode = Dmp::new();
            let mut grapheme_mode = Dmp::new();
            grapheme_mode.segmentation = Segmentation::Grapheme;
            // Single-char clusters pack to themselves, so both modes run the
            // identical pipeline (line mode included) on identical tokens.
            let checklines = rng.next().is_multiple_of(2);
            assert_eq!(
                char_mode.diff_main(&t1, &t2, checklines),
                grapheme_mode.diff_main(&t1, &t2, checklines)
            );
        }
    }
}
