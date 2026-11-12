// The step budget bounds diff work by a count, not a clock, so the same
// inputs give the same output on every machine.

use diff_match_patch::{Diff, Dmp};

fn differ(budget: Option<u64>) -> Dmp {
    let mut dmp = Dmp::new();
    dmp.diff_timeout = None;
    dmp.diff_step_budget = budget;
    dmp
}

fn pieces(diffs: &[(i32, &str)]) -> Vec<Diff> {
    diffs
        .iter()
        .map(|&(op, text)| Diff::new(op, text.to_string()))
        .collect()
}

fn edit_chars(diffs: &[Diff]) -> usize {
    diffs
        .iter()
        .filter(|d| d.operation != 0)
        .map(|d| d.text.chars().count())
        .sum()
}

/// Run `diff` twice and check it rebuilds both texts from non-empty pieces
/// with no two neighbours sharing an op.
fn assert_well_formed(
    old: &str,
    new: &str,
    mut diff: impl FnMut() -> Vec<Diff>,
    ctx: &str,
) -> Vec<Diff> {
    let diffs = diff();
    assert_eq!(diff(), diffs, "{ctx}");
    let side = |skip: i32| -> String {
        diffs
            .iter()
            .filter(|d| d.operation != skip)
            .map(|d| d.text.as_str())
            .collect()
    };
    assert_eq!(side(1), old, "{ctx}");
    assert_eq!(side(-1), new, "{ctx}");
    assert!(diffs.iter().all(|d| !d.text.is_empty()), "{ctx}");
    assert!(
        diffs.windows(2).all(|w| w[0].operation != w[1].operation),
        "{ctx}"
    );
    diffs
}

/// Forty sentences, every fifth with one char inserted.
fn fox_pair() -> (String, String) {
    let mut old = String::new();
    let mut new = String::new();
    for i in 0..40 {
        old += &format!("the quick brown fox jumps over the lazy dog. {i} ");
        let mark = if i % 5 == 0 { "x" } else { "" };
        new += &format!("the quick brown fox jumps over the lazy dog. {mark}{i} ");
    }
    (old, new)
}

#[test]
fn a_spent_budget_keeps_what_it_found() {
    let (old, new) = ("abcdefgh1xyz2", "stuvw1mnop2qr");
    let unlimited = differ(None).diff_main(old, new, false);
    assert_eq!(unlimited.len(), 7);

    let whole = pieces(&[(-1, old), (1, new)]);
    assert_eq!(differ(Some(0)).diff_main(old, new, false), whole);
    assert_eq!(differ(Some(117)).diff_main(old, new, false), whole);
    // From here the first search finds its split and only a later one runs dry.
    let partial = pieces(&[
        (-1, "abcdefgh"),
        (1, "stuvw"),
        (0, "1"),
        (-1, "xyz2"),
        (1, "mnop2qr"),
    ]);
    assert_eq!(differ(Some(118)).diff_main(old, new, false), partial);
    assert_eq!(differ(Some(125)).diff_main(old, new, false), partial);
    assert_eq!(differ(Some(126)).diff_main(old, new, false), unlimited);
}

#[test]
fn an_over_budget_diff_keeps_long_matches() {
    let (old, new) = fox_pair();
    assert_eq!((old.len(), new.len()), (1910, 1918));
    let shape = |budget: Option<u64>| {
        let diffs = differ(budget).diff_main(&old, &new, false);
        (diffs.len(), edit_chars(&diffs))
    };
    assert_eq!(shape(None), (17, 8));
    assert_eq!(shape(Some(0)), (4, 3348));
    assert_eq!(shape(Some(2)), (6, 2878));
    assert_eq!(shape(Some(360)), (12, 1448));
    let unlimited = differ(None).diff_main(&old, &new, false);
    assert_eq!(differ(Some(1208)).diff_main(&old, &new, false), unlimited);
    assert_eq!(differ(Some(1691)).diff_main(&old, &new, false), unlimited);
}

#[test]
fn a_budget_turns_off_half_match() {
    // Half-match does work the budget does not count, so a budget skips it.
    let (old, new) = ("qHilloHelloHew", "xHelloHeHulloy");
    let mut timed = Dmp::new();
    timed.diff_timeout = Some(1.0);
    let optimal = differ(None).diff_main(old, new, false);
    assert_ne!(timed.diff_main(old, new, false), optimal);
    assert_eq!(differ(Some(u64::MAX)).diff_main(old, new, false), optimal);
    timed.diff_timeout = Some(1.0e6);
    timed.diff_step_budget = Some(u64::MAX);
    assert_eq!(timed.diff_main(old, new, false), optimal);
}

#[test]
fn a_zero_budget_bounds_a_diff_that_half_match_would_scan() {
    let old = "ab".repeat(20_000);
    let new = "ba".repeat(20_000) + "c";
    let mut timed = Dmp::new();
    timed.diff_step_budget = Some(0);
    let diffs = timed.diff_main(&old, &new, false);
    assert_eq!(diffs, differ(Some(0)).diff_main(&old, &new, false));
    assert_eq!(diffs, pieces(&[(-1, &old), (1, &new)]));
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn text(&mut self, len: usize, alpha: &[u8]) -> Vec<u8> {
        (0..len).map(|_| alpha[self.below(alpha.len())]).collect()
    }
}

fn random_pair(rng: &mut Lcg) -> (String, String) {
    let alphabets: [&[u8]; 4] = [b"ab", b"abcd", b"abcdefghijklmnopqrstuvwxyz \n", b"ab\n"];
    let alpha = alphabets[rng.below(4)];
    let (old, new) = if rng.below(3) == 0 {
        let (n, m) = (rng.below(61), rng.below(61));
        (rng.text(n, alpha), rng.text(m, alpha))
    } else {
        let base_len = 20 + rng.below(601);
        let base = rng.text(base_len, alpha);
        let every = 3 + rng.below(60);
        let mut new = Vec::new();
        for &c in &base {
            if rng.below(every) != 0 {
                new.push(c);
                continue;
            }
            match rng.below(4) {
                0 => new.push(alpha[rng.below(alpha.len())]),
                1 => {}
                2 => new.extend([c, c]),
                _ => {
                    let len = rng.below(40);
                    new.extend(rng.text(len, alpha));
                    new.push(c);
                }
            }
        }
        if rng.below(4) == 0 && new.len() > 10 {
            let cut = rng.below(new.len() / 2);
            let len = 1 + rng.below(new.len() / 2);
            let block: Vec<u8> = new.drain(cut..cut + len).collect();
            let at = rng.below(new.len() + 1);
            new.splice(at..at, block);
        }
        (base, new)
    };
    (
        String::from_utf8(old).expect("ascii"),
        String::from_utf8(new).expect("ascii"),
    )
}

#[test]
fn over_budget_diffs_are_valid_and_deterministic() {
    let mut rng = Lcg(0x005e_edb0_d9e7);
    let mut cleaner = Dmp::new();
    let (mut runs, mut differ_from_unlimited) = (0, 0);
    for _ in 0..150 {
        let (old, new) = random_pair(&mut rng);
        for checklines in [false, true] {
            let unlimited = differ(None).diff_main(&old, &new, checklines);
            assert_eq!(
                differ(Some(u64::MAX)).diff_main(&old, &new, checklines),
                unlimited
            );
            let random = rng.next() % 4000;
            for budget in [0, 1, random] {
                let ctx = format!("budget {budget} checklines {checklines} {old:?} {new:?}");
                let diffs = assert_well_formed(
                    &old,
                    &new,
                    || differ(Some(budget)).diff_main(&old, &new, checklines),
                    &ctx,
                );
                let mut merged = diffs.clone();
                cleaner.diff_cleanup_merge(&mut merged);
                assert_eq!(merged, diffs, "{ctx}");
                runs += 1;
                differ_from_unlimited += (diffs != unlimited) as usize;
            }
        }
    }
    assert_eq!((runs, differ_from_unlimited), (900, 663));
}

#[test]
fn an_over_budget_diff_under_a_timeout_matches_the_budget_alone() {
    let (old, new) = fox_pair();
    let diffs = assert_well_formed(
        &old,
        &new,
        || {
            let mut dmp = differ(Some(360));
            dmp.diff_timeout = Some(1.0e6);
            dmp.diff_main(&old, &new, false)
        },
        "timeout",
    );
    assert_ne!(diffs, differ(None).diff_main(&old, &new, false));
    assert_eq!(diffs, differ(Some(360)).diff_main(&old, &new, false));
}

#[test]
fn an_over_budget_word_diff_is_well_formed() {
    let (old, new) = fox_pair();
    let words = |budget: Option<u64>| {
        let mut dmp = differ(budget);
        dmp.word_mode = true;
        dmp.diff_main(&old, &new, false)
    };
    let diffs = assert_well_formed(&old, &new, || words(Some(100)), "words");
    assert_ne!(diffs, words(None));
}

#[cfg(feature = "grapheme")]
#[test]
fn an_over_budget_grapheme_diff_is_well_formed() {
    let (old, new) = fox_pair();
    // Every "o" becomes a two-scalar cluster.
    let (old, new) = (old.replace('o', "o\u{301}"), new.replace('o', "o\u{301}"));
    let clusters = |budget: Option<u64>| {
        let mut dmp = differ(budget);
        dmp.segmentation = diff_match_patch::Segmentation::Grapheme;
        dmp.diff_main(&old, &new, false)
    };
    let diffs = assert_well_formed(&old, &new, || clusters(Some(360)), "grapheme");
    assert_ne!(diffs, clusters(None));
}

#[test]
fn an_over_budget_bisect_is_well_formed() {
    let (old, new) = fox_pair();
    let (old_chars, new_chars): (Vec<char>, Vec<char>) =
        (old.chars().collect(), new.chars().collect());
    let bisect = |budget: Option<u64>| differ(budget).diff_bisect(&old_chars, &new_chars);
    let diffs = assert_well_formed(&old, &new, || bisect(Some(360)), "bisect");
    assert_ne!(diffs, bisect(None));
}

#[test]
fn an_over_budget_line_diff_is_well_formed() {
    let (old, new) = fox_pair();
    let (old, new) = (
        old.replace("dog. ", "dog.\n"),
        new.replace("dog. ", "dog.\n"),
    );
    let lines = |budget: Option<u64>| differ(budget).diff_main(&old, &new, true);
    let diffs = assert_well_formed(&old, &new, || lines(Some(100)), "lines");
    assert_ne!(diffs, lines(None));
}
