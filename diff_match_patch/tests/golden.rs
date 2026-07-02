//! Golden-reference tests: validate the crate against vectors emitted by the vendored
//! googlediff oracle (oracle/vendor/, regenerate with `node oracle/generate.mjs`).
//!
//! Config parity: `Dmp::new()` defaults (diff_timeout None, edit_cost 0, margin 4,
//! maxbits 32, thresholds 0.5, distance 1000) equal the generator's oracle settings,
//! so plain `Dmp::new()` is the correct configuration for every test here.
//!
//! Known deviations from the oracle are pinned in tests/characterization.rs, not here.

use diff_match_patch::{Diff, Dmp};
use serde::Deserialize;

/// Keep in sync with CASES in oracle/generate.mjs; an empty or truncated corpus must
/// fail loudly instead of letting the `for` loops below pass vacuously.
const EXPECTED_CASES: usize = 21;

#[derive(Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    text1: String,
    text2: String,
    #[serde(rename = "applyTo")]
    apply_to: String,
    diff: Vec<(i32, String)>,
    #[serde(rename = "diffSemantic")]
    diff_semantic: Vec<(i32, String)>,
    delta: String,
    #[serde(rename = "patchText")]
    patch_text: String,
    #[serde(rename = "patchApplied")]
    patch_applied: String,
    #[serde(rename = "patchResults")]
    patch_results: Vec<bool>,
}

fn corpus() -> Vec<Case> {
    let c: Corpus = serde_json::from_str(include_str!("golden/corpus.json")).unwrap();
    assert_eq!(
        c.cases.len(),
        EXPECTED_CASES,
        "corpus.json case count changed — update EXPECTED_CASES"
    );
    c.cases
}

fn to_tuples(diffs: &[Diff]) -> Vec<(i32, String)> {
    diffs
        .iter()
        .map(|d| (d.operation, d.text.clone()))
        .collect()
}

fn from_tuples(rows: &[(i32, String)]) -> Vec<Diff> {
    rows.iter()
        .map(|(op, t)| Diff::new(*op, t.clone()))
        .collect()
}

#[test]
fn golden_diff_main() {
    for c in corpus() {
        let got = to_tuples(&Dmp::new().diff_main(&c.text1, &c.text2, true));
        assert_eq!(got, c.diff, "diff mismatch in case '{}'", c.name);
    }
}

#[test]
fn golden_diff_cleanup_semantic() {
    for c in corpus() {
        let mut diffs = from_tuples(&c.diff);
        Dmp::new().diff_cleanup_semantic(&mut diffs);
        assert_eq!(
            to_tuples(&diffs),
            c.diff_semantic,
            "semantic cleanup mismatch in case '{}'",
            c.name
        );
    }
}

#[test]
fn golden_delta_encode() {
    for c in corpus() {
        let mut diffs = from_tuples(&c.diff);
        assert_eq!(
            Dmp::new().diff_todelta(&mut diffs),
            c.delta,
            "delta mismatch in case '{}'",
            c.name
        );
    }
}

#[test]
fn golden_delta_decode() {
    for c in corpus() {
        let diffs = Dmp::new().diff_from_delta(&c.text1, &c.delta);
        assert_eq!(
            to_tuples(&diffs),
            c.diff,
            "from_delta mismatch in case '{}'",
            c.name
        );
    }
}

#[test]
fn golden_patch_make() {
    for c in corpus() {
        let mut d = Dmp::new();
        let mut patches = d.patch_make1(&c.text1, &c.text2);
        assert_eq!(
            d.patch_to_text(&mut patches),
            c.patch_text,
            "patch_make mismatch in case '{}'",
            c.name
        );
    }
}

#[test]
fn golden_patch_text_roundtrip() {
    for c in corpus() {
        let mut d = Dmp::new();
        let mut patches = d.patch_from_text(c.patch_text.clone());
        assert_eq!(
            d.patch_to_text(&mut patches),
            c.patch_text,
            "patch roundtrip mismatch in '{}'",
            c.name
        );
    }
}

#[test]
fn golden_patch_apply() {
    for c in corpus() {
        let mut d = Dmp::new();
        let mut patches = d.patch_from_text(c.patch_text.clone());
        let (applied, results) = d.patch_apply(&mut patches, &c.apply_to);
        let applied: String = applied.into_iter().collect();
        assert_eq!(
            applied, c.patch_applied,
            "patch apply text mismatch in '{}'",
            c.name
        );
        assert_eq!(
            results, c.patch_results,
            "patch apply results mismatch in '{}'",
            c.name
        );
    }
}
