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
fn random_diffs(rng: &mut Lcg, alphabet: &[char]) -> Vec<(i32, Vec<char>)> {
    let max_len = [1, 2, 4, 9][rng.below(4)];
    (0..rng.below(14))
        .map(|_| {
            let operation = [-1, 0, 1, 0][rng.below(4)];
            let len = rng.below(max_len + 1);
            let text = (0..len).map(|_| alphabet[rng.below(alphabet.len() as u64)]);
            (operation, text.collect())
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
        let input = random_diffs(&mut rng, &['a', 'b']);
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
        let input = random_diffs(&mut rng, &['a', 'b']);
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

#[test]
fn merge_rounds_do_not_grow_the_stack() {
    // Each shift lines up the next one, so this takes one round per pair.
    let rounds = 1_000;
    let mut input = vec![];
    for _ in 0..rounds {
        input.push((0, vec!['a']));
        input.push((1, vec!['b']));
    }
    input.push((0, vec!['a']));
    input.push((1, vec!['a'; rounds]));
    input.push((0, vec!['c']));
    let merged = std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || {
            let mut diffs = tokens(&input);
            Dmp::new().diff_cleanup_merge_impl(&mut diffs);
            pieces(diffs)
        })
        .expect("spawn")
        .join()
        .expect("merge thread");
    let mut equality = vec!['a'; rounds + 1];
    equality.push('c');
    assert_eq!(
        merged,
        vec![(1, "ab".repeat(rounds).chars().collect()), (0, equality)]
    );
}

/// The loop the one-pass form replaced: remove each emptied equality in place.
fn slide_by_removal(dmp: &mut Dmp, diffs: &mut Vec<TDiff>) {
    let mut pointer: i32 = 1;
    while pointer < diffs.len() as i32 - 1 {
        if diffs[pointer as usize - 1].operation == 0 && diffs[pointer as usize + 1].operation == 0
        {
            let mut buf: Vec<char> = vec![];
            buf.extend_from_slice(&diffs[pointer as usize - 1].data);
            buf.extend_from_slice(&diffs[pointer as usize].data);
            buf.extend_from_slice(&diffs[pointer as usize + 1].data);
            let mut a = diffs[pointer as usize - 1].data.len();
            let mut b = a + diffs[pointer as usize].data.len();
            let common_offset = engine::common_suffix(&buf[..a], &buf[a..b]);
            a -= common_offset;
            b -= common_offset;
            let mut best = (a, b);
            let mut best_score = dmp.diff_cleanup_semantic_score(&buf[..a], &buf[a..b])
                + dmp.diff_cleanup_semantic_score(&buf[a..b], &buf[b..]);
            while a < b && b < buf.len() && buf[a] == buf[b] {
                a += 1;
                b += 1;
                let score = dmp.diff_cleanup_semantic_score(&buf[..a], &buf[a..b])
                    + dmp.diff_cleanup_semantic_score(&buf[a..b], &buf[b..]);
                if score >= best_score {
                    best_score = score;
                    best = (a, b);
                }
            }
            if diffs[pointer as usize - 1].data != buf[..best.0] {
                if best.0 > 0 {
                    diffs[pointer as usize - 1].data = buf[..best.0].to_vec();
                } else {
                    diffs.remove(pointer as usize - 1);
                    pointer -= 1;
                }
                diffs[pointer as usize].data = buf[best.0..best.1].to_vec();
                if best.1 < buf.len() {
                    diffs[pointer as usize + 1].data = buf[best.1..].to_vec();
                } else {
                    diffs.remove(pointer as usize + 1);
                    pointer += 1;
                }
            }
        }
        pointer += 1;
    }
}

#[test]
fn one_pass_slide_matches_removal() {
    let mut rng = Lcg(3);
    // Inputs that lose one equality, and inputs that lose several.
    let (mut single, mut several) = (0, 0);
    for _ in 0..50_000 {
        let input = random_diffs(&mut rng, &['a', ' ', '\n', '.']);
        let (mut expected, mut actual) = (tokens(&input), tokens(&input));
        slide_by_removal(&mut Dmp::new(), &mut expected);
        Dmp::new().diff_cleanup_semantic_lossless_impl(&mut actual);
        let removed = input.len() - actual.len();
        assert_eq!(pieces(actual), pieces(expected), "{input:?}");
        single += (removed == 1) as usize;
        several += (removed > 1) as usize;
    }
    assert_eq!((single, several), (4016, 207));
}

/// The loop the one-pass form replaced: insert each shared middle in place.
fn overlaps_by_insertion(diffs: &mut Vec<TDiff>) {
    let mut pointer = 1;
    while pointer < diffs.len() {
        if diffs[pointer - 1].operation == -1 && diffs[pointer].operation == 1 {
            let deletion = diffs[pointer - 1].data.clone();
            let insertion = diffs[pointer].data.clone();
            let overlap1 = engine::common_overlap(&deletion, &insertion);
            let overlap2 = engine::common_overlap(&insertion, &deletion);
            if overlap1 >= overlap2 {
                if (overlap1 as f32) >= (deletion.len() as f32 / 2.0)
                    || (overlap1 as f32) >= (insertion.len() as f32 / 2.0)
                {
                    diffs.insert(pointer, TDiff::new(0, insertion[..overlap1].to_vec()));
                    diffs[pointer - 1].data = deletion[..deletion.len() - overlap1].to_vec();
                    diffs[pointer + 1].data = insertion[overlap1..].to_vec();
                    pointer += 1;
                }
            } else if (overlap2 as f32) >= (deletion.len() as f32 / 2.0)
                || (overlap2 as f32) >= (insertion.len() as f32 / 2.0)
            {
                diffs.insert(pointer, TDiff::new(0, deletion[..overlap2].to_vec()));
                diffs[pointer - 1] =
                    TDiff::new(1, insertion[..insertion.len() - overlap2].to_vec());
                diffs[pointer + 1] = TDiff::new(-1, deletion[overlap2..].to_vec());
                pointer += 1;
            }
            pointer += 1;
        }
        pointer += 1;
    }
}

#[test]
fn one_pass_overlap_split_matches_insertion() {
    let mut rng = Lcg(4);
    // Inputs that gain one equality, and inputs that gain several.
    let (mut single, mut several) = (0, 0);
    for _ in 0..50_000 {
        let input = random_diffs(&mut rng, &['a', 'b']);
        let (mut expected, mut actual) = (tokens(&input), tokens(&input));
        overlaps_by_insertion(&mut expected);
        split_overlapping_edits(&mut actual);
        let added = actual.len() - input.len();
        assert_eq!(pieces(actual), pieces(expected), "{input:?}");
        single += (added == 1) as usize;
        several += (added > 1) as usize;
    }
    assert_eq!((single, several), (10440, 1724));
}

/// The loop the one-pass form replaced: after each split, rescan from two
/// candidates back, or from the start.
fn split_by_insertion(diffs: &mut Vec<TDiff>, edit_cost: i32) -> bool {
    let mut changes = false;
    let mut equalities: Vec<i32> = vec![];
    let mut last_equality: Vec<char> = vec![];
    let mut pointer: i32 = 0;
    let (mut pre_ins, mut pre_del) = (false, false);
    let (mut post_ins, mut post_del) = (false, false);
    while (pointer as usize) < diffs.len() {
        if diffs[pointer as usize].operation == 0 {
            if diffs[pointer as usize].data.len() < edit_cost as usize && (post_del || post_ins) {
                equalities.push(pointer);
                (pre_ins, pre_del) = (post_ins, post_del);
                last_equality = diffs[pointer as usize].data.clone();
            } else {
                equalities = vec![];
                last_equality = vec![];
            }
            (post_ins, post_del) = (false, false);
        } else {
            if diffs[pointer as usize].operation == -1 {
                post_del = true;
            } else {
                post_ins = true;
            }
            if !last_equality.is_empty()
                && ((pre_ins && pre_del && post_del && post_ins)
                    || ((last_equality.len() as i32) < edit_cost / 2
                        && (pre_ins as i32 + pre_del as i32 + post_del as i32 + post_ins as i32)
                            == 3))
            {
                let at = equalities[equalities.len() - 1] as usize;
                diffs.insert(at, TDiff::new(-1, last_equality));
                diffs[at + 1].operation = 1;
                equalities.pop();
                last_equality = vec![];
                if pre_ins && pre_del {
                    (post_ins, post_del) = (true, true);
                    equalities = vec![];
                } else {
                    equalities.pop();
                    pointer = equalities.last().copied().unwrap_or(-1);
                    (post_ins, post_del) = (false, false);
                }
                changes = true;
            }
        }
        pointer += 1;
    }
    changes
}

#[test]
fn one_pass_split_matches_rescanning() {
    let mut rng = Lcg(5);
    // Inputs that lose one equality, and inputs that lose several.
    let (mut single, mut several) = (0, 0);
    for _ in 0..50_000 {
        // Long enough for chains of candidates.
        let mut input = vec![];
        for _ in 0..=rng.below(4) {
            input.extend(random_diffs(&mut rng, &['a', 'b']));
        }
        let edit_cost = [-1, 0, 1, 2, 3, 4, 6, 9, 14, 20][rng.below(10)];
        let (mut expected, mut actual) = (tokens(&input), tokens(&input));
        let changed = split_by_insertion(&mut expected, edit_cost);
        assert_eq!(
            split_cheap_equalities(&mut actual, edit_cost),
            changed,
            "{edit_cost} {input:?}"
        );
        let split = actual.len() - input.len();
        assert_eq!(pieces(actual), pieces(expected), "{edit_cost} {input:?}");
        single += (split == 1) as usize;
        several += (split > 1) as usize;
    }
    assert_eq!((single, several), (6085, 3836));
}
