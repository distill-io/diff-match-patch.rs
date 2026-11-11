// The step budget bounds diff work by a count, not a clock, so the same
// inputs give the same output on every machine.

use diff_match_patch::{Diff, Dmp};

fn differ(budget: Option<u64>) -> Dmp {
    let mut dmp = Dmp::new();
    dmp.diff_timeout = None;
    dmp.diff_step_budget = budget;
    dmp
}

#[test]
fn a_spent_budget_falls_back_to_delete_and_insert() {
    let (old, new) = ("abcdefgh1xyz2", "stuvw1mnop2qr");
    let unlimited = differ(None).diff_main(old, new, false);
    assert_eq!(unlimited.len(), 7);

    assert_eq!(
        differ(Some(0)).diff_main(old, new, false),
        vec![
            Diff::new(-1, old.to_string()),
            Diff::new(1, new.to_string())
        ]
    );
    // The searches of this diff cost 232 steps together. One step short, the
    // first search has found its split but a later one runs dry.
    let short = differ(Some(231)).diff_main(old, new, false);
    assert_eq!(short.len(), 5);
    assert_ne!(short, unlimited);
    assert_eq!(differ(Some(232)).diff_main(old, new, false), unlimited);
}

#[test]
fn a_budget_alone_does_not_turn_on_half_match() {
    // Half-match trades optimality for speed and belongs to the timeout.
    let (old, new) = ("qHilloHelloHew", "xHelloHeHulloy");
    let mut timed = Dmp::new();
    timed.diff_timeout = Some(1.0);
    let optimal = differ(None).diff_main(old, new, false);
    assert_ne!(timed.diff_main(old, new, false), optimal);
    assert_eq!(differ(Some(u64::MAX)).diff_main(old, new, false), optimal);
}
