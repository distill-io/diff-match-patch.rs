use super::*;

/// The loop the one-pass form replaced: after each elimination, rescan
/// from two equalities back.
fn eliminate_by_rescanning(diffs: &mut Vec<TDiff>) -> bool {
    let mut changes = false;
    let mut equalities: Vec<i32> = vec![];
    let mut last_equality: Vec<char> = vec![];
    let mut pointer: i32 = 0;
    let (mut insertions1, mut deletions1) = (0, 0);
    let (mut insertions2, mut deletions2) = (0, 0);
    while (pointer as usize) < diffs.len() {
        let len = diffs[pointer as usize].data.len() as i32;
        if diffs[pointer as usize].operation == 0 {
            equalities.push(pointer);
            (insertions1, deletions1) = (insertions2, deletions2);
            (insertions2, deletions2) = (0, 0);
            last_equality = diffs[pointer as usize].data.clone();
        } else {
            if diffs[pointer as usize].operation == 1 {
                insertions2 += len;
            } else {
                deletions2 += len;
            }
            let equality_len = last_equality.len() as i32;
            if equality_len > 0
                && equality_len <= max(insertions1, deletions1)
                && equality_len <= max(insertions2, deletions2)
            {
                let at = equalities[equalities.len() - 1] as usize;
                diffs.insert(at, TDiff::new(-1, last_equality.clone()));
                diffs[at + 1].operation = 1;
                equalities.pop();
                equalities.pop();
                pointer = equalities.last().copied().unwrap_or(-1);
                (insertions1, deletions1) = (0, 0);
                (insertions2, deletions2) = (0, 0);
                last_equality = vec![];
                changes = true;
            }
        }
        pointer += 1;
    }
    changes
}

struct Lcg(u64);
impl Lcg {
    fn below(&mut self, bound: u64) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) % bound) as usize
    }
}

// Any list the public cleanup accepts: empty pieces and repeated ops included.
fn random_diffs(rng: &mut Lcg) -> Vec<(i32, Vec<char>)> {
    let max_len = [1, 2, 4, 9][rng.below(4)];
    (0..rng.below(14))
        .map(|_| {
            let operation = [-1, 0, 1, 0][rng.below(4)];
            let len = rng.below(max_len + 1);
            (
                operation,
                (0..len).map(|_| ['a', 'b'][rng.below(2)]).collect(),
            )
        })
        .collect()
}

fn tokens(pieces: &[(i32, Vec<char>)]) -> Vec<TDiff> {
    pieces
        .iter()
        .map(|(operation, data)| TDiff::new(*operation, data.clone()))
        .collect()
}

fn pieces(diffs: Vec<TDiff>) -> Vec<(i32, Vec<char>)> {
    diffs.into_iter().map(|d| (d.operation, d.data)).collect()
}

#[test]
fn one_pass_elimination_matches_rescanning() {
    let mut rng = Lcg(1);
    // Inputs that lose one equality, and inputs that lose several.
    let (mut single, mut several) = (0, 0);
    for _ in 0..50_000 {
        let input = random_diffs(&mut rng);
        let (mut expected, mut actual) = (tokens(&input), tokens(&input));
        let changed = eliminate_by_rescanning(&mut expected);
        assert_eq!(
            eliminate_small_equalities(&mut actual),
            changed,
            "{input:?}"
        );
        let eliminated = actual.len() - input.len();
        assert_eq!(pieces(actual), pieces(expected), "{input:?}");
        single += (eliminated == 1) as usize;
        several += (eliminated > 1) as usize;
    }
    assert_eq!((single, several), (6735, 1289));
}

#[test]
fn merge_leaves_nothing_of_empty_pieces() {
    for input in [
        vec![(0, vec![])],
        vec![(0, vec![]), (0, vec![])],
        vec![(-1, vec![]), (1, vec![])],
    ] {
        let mut diffs = tokens(&input);
        Dmp::new().diff_cleanup_merge_impl(&mut diffs);
        assert_eq!(pieces(diffs), vec![], "{input:?}");
    }
}

/// The sweep the one-pass form replaced: remove each crossed equality in place.
fn shift_by_removal(diffs: &mut Vec<TDiff>) -> bool {
    let mut changes = false;
    let mut i = 1;
    while i + 1 < diffs.len() {
        if diffs[i - 1].operation == 0 && diffs[i + 1].operation == 0 {
            if diffs[i].data.ends_with(&diffs[i - 1].data) {
                if !diffs[i - 1].data.is_empty() {
                    let eq1 = diffs[i - 1].data.clone();
                    let keep = diffs[i].data.len() - eq1.len();
                    let mut shifted = eq1.clone();
                    shifted.extend_from_slice(&diffs[i].data[..keep]);
                    diffs[i].data = shifted;
                    let mut next = eq1;
                    next.extend_from_slice(&diffs[i + 1].data);
                    diffs[i + 1].data = next;
                }
                diffs.remove(i - 1);
                changes = true;
            } else if diffs[i].data.starts_with(&diffs[i + 1].data) {
                let eq2 = diffs[i + 1].data.clone();
                diffs[i - 1].data.extend_from_slice(&eq2);
                let mut edit = diffs[i].data[eq2.len()..].to_vec();
                edit.extend_from_slice(&eq2);
                diffs[i].data = edit;
                diffs.remove(i + 1);
                changes = true;
            }
        }
        i += 1;
    }
    changes
}

#[test]
fn one_pass_shift_matches_removal() {
    let mut rng = Lcg(2);
    // Inputs that lose one equality, and inputs that lose several.
    let (mut single, mut several) = (0, 0);
    for _ in 0..50_000 {
        let input = random_diffs(&mut rng);
        let (mut expected, mut actual) = (tokens(&input), tokens(&input));
        let changed = shift_by_removal(&mut expected);
        assert_eq!(
            shift_edits_over_equalities(&mut actual),
            changed,
            "{input:?}"
        );
        let removed = input.len() - actual.len();
        assert_eq!(pieces(actual), pieces(expected), "{input:?}");
        single += (removed == 1) as usize;
        several += (removed > 1) as usize;
    }
    assert_eq!((single, several), (13176, 7192));
}
