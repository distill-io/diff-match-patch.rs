use super::*;
use crate::engine::Budget;

/// The diff of `old` and `new` under `budget`, with the steps it used.
fn run(old: &[u8], new: &[u8], budget: Option<u64>) -> (Vec<(i32, Vec<char>)>, Option<u64>) {
    let limit = Limit {
        deadline: None,
        budget: budget.map(Budget::new),
    };
    let diffs = main_slices(
        &mut Dmp::new(),
        old,
        new,
        false,
        false,
        &limit,
        &mut Vec::new(),
    );
    let diffs = diffs.into_iter().map(|d| (d.operation, d.data)).collect();
    (diffs, limit.budget.map(|budget| budget.used.get()))
}

struct Lcg(u64);
impl Lcg {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }

    fn text(&mut self, len: u64, alpha: &[u8]) -> Vec<u8> {
        (0..len)
            .map(|_| alpha[self.below(alpha.len() as u64) as usize])
            .collect()
    }
}

/// Unrelated texts and lightly edited copies, over small alphabets.
fn pairs() -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut rng = Lcg(7);
    let alphabets: [&[u8]; 3] = [b"ab", b"abcd", b"abcdefghijklmnopqrstuvwxyz \n"];
    (0..40)
        .map(|i| {
            let alpha = alphabets[i % 3];
            let len = 1 + rng.below(400);
            let old = rng.text(len, alpha);
            let new = if i % 2 == 0 {
                let len = 1 + rng.below(400);
                rng.text(len, alpha)
            } else {
                old.iter()
                    .flat_map(|&c| match rng.below(12) {
                        0 => vec![],
                        1 => {
                            let len = 1 + rng.below(9);
                            rng.text(len, alpha)
                        }
                        _ => vec![c],
                    })
                    .collect()
            };
            (old, new)
        })
        .collect()
}

#[test]
fn work_stays_within_twice_the_budget() {
    let mut rng = Lcg(11);
    let (mut past_twice, mut tightest) = (0, u64::MAX);
    for (old, new) in pairs() {
        let shorter = old.len().min(new.len()) as u64;
        let (_, need) = run(&old, &new, Some(u64::MAX / 4));
        let need = need.unwrap();
        for exact in [0, 1, need / 3, rng.below(need + 1)] {
            let (_, used) = run(&old, &new, Some(exact));
            let used = used.unwrap();
            // The last diagonal walked may run past the total by one snake.
            let bound = 2 * exact + shorter + 1;
            assert!(used <= bound, "used {used} of {exact} on {old:?} {new:?}");
            past_twice += (used > 2 * exact) as usize;
            tightest = tightest.min(bound - used);
        }
    }
    assert_eq!((past_twice, tightest), (50, 4));
}

#[test]
fn a_budget_that_covers_the_work_changes_nothing() {
    for (old, new) in pairs() {
        let (unlimited, none) = run(&old, &new, None);
        assert_eq!(none, None);
        let (_, need) = run(&old, &new, Some(u64::MAX / 4));
        let need = need.unwrap();
        for budget in [need, need + 1] {
            let (diffs, used) = run(&old, &new, Some(budget));
            assert_eq!(used, Some(need), "{old:?} {new:?}");
            assert_eq!(diffs, unlimited, "budget {budget} on {old:?} {new:?}");
        }
    }
}

#[test]
fn a_spent_budget_finishes_on_a_small_stack() {
    let mut rng = Lcg(3);
    let alpha = b"abcdefghijklmnopqrstuvwxyz     \n";
    let old = rng.text(200_000, alpha);
    let new = rng.text(200_000, alpha);
    let (diffs, used) = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let (diffs, used) = run(&old, &new, Some(2_000_000));
            let rebuilt = |skip: i32| -> Vec<u8> {
                diffs
                    .iter()
                    .filter(|d| d.0 != skip)
                    .flat_map(|d| d.1.iter().map(|&c| c as u8))
                    .collect()
            };
            assert_eq!((rebuilt(1), rebuilt(-1)), (old, new));
            (diffs.len(), used)
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!((diffs, used), (1366, Some(4_000_001)));
}
