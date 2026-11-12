// The bitap search against the loop it replaced.

use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

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

/// Which paths of the bitap loop a run took.
#[derive(Debug, Default, PartialEq)]
struct BitapHits {
    no_match: usize,
    exact: usize,
    fuzzy: usize,
    multi_level: usize,
    start_lowered: usize,
    passed_loc: usize,
    unwritten_read: usize,
    no_hope: usize,
    negative_loc: usize,
    panicked: usize,
}

/// The loop that zero-filled each level's bit arrays from index 0.
fn bitap_whole_arrays(
    dmp: &Dmp,
    text: &[char],
    patern: &[char],
    loc: i32,
    hits: &mut BitapHits,
) -> i32 {
    if !(dmp.match_maxbits == 0 || patern.len() as i32 <= dmp.match_maxbits) {
        panic!("patern too long for this application");
    }
    if patern.len() > 64 {
        panic!("patern too long for this application");
    }
    let s: HashMap<char, u64> = alphabet(patern);
    let mut score_threshold: f32 = dmp.match_threshold;
    let mut best_loc = match engine::find_sub(text, patern, loc as usize) {
        Some(i) => i as i32,
        None => -1,
    };
    if best_loc != -1 {
        score_threshold = min1(
            bitap_score(dmp, 0, best_loc, loc, patern.len()),
            score_threshold,
        );
        best_loc = match engine::rfind_sub(text, patern, loc as usize + patern.len()) {
            Some(i) => i as i32,
            None => -1,
        };
        if best_loc != -1 {
            score_threshold = min1(
                score_threshold,
                bitap_score(dmp, 0, best_loc, loc, patern.len()),
            );
        }
    }
    let matchmask: u64 = 1 << (patern.len() - 1);
    best_loc = -1;
    let mut best_level = 0;
    let mut bin_min: i32;
    let mut bin_mid: i32;
    let mut bin_max: i32 = (patern.len() + text.len()) as i32;
    let mut last_rd: Vec<u64> = vec![];
    // Lowest index the previous level wrote.
    let mut last_low = 0;
    for d in 0..patern.len() {
        hits.multi_level += (d == 1) as usize;
        let mut rd: Vec<u64> = vec![];
        bin_min = 0;
        bin_mid = bin_max;
        while bin_min < bin_mid {
            if bitap_score(dmp, d as i32, loc + bin_mid, loc, patern.len()) <= score_threshold {
                bin_min = bin_mid;
            } else {
                bin_max = bin_mid;
            }
            bin_mid = bin_min + (bin_max - bin_min) / 2;
        }
        bin_max = bin_mid;
        let mut start = max(1, loc - bin_mid + 1);
        let first_start = start;
        let finish = min(loc + bin_mid, text.len() as i32) + patern.len() as i32;
        rd.resize((finish + 2) as usize, 0);
        rd[(finish + 1) as usize] = (1u64 << d) - 1;
        let mut low = finish + 1;
        let mut read_unwritten = false;
        let mut j = finish;
        while j >= start {
            let char_match: u64;
            if text.len() < j as usize {
                char_match = 0;
            } else {
                match s.get(&(text[j as usize - 1])) {
                    Some(num) => {
                        char_match = *num;
                    }
                    None => {
                        char_match = 0;
                    }
                }
            }
            if d == 0 {
                rd[j as usize] = ((rd[j as usize + 1] << 1) | 1) & char_match;
            } else {
                read_unwritten |= j < last_low;
                rd[j as usize] = (((rd[j as usize + 1] << 1) | 1) & char_match)
                    | (((last_rd[j as usize + 1] | last_rd[j as usize]) << 1) | 1)
                    | last_rd[j as usize + 1];
            }
            low = j;
            if (rd[j as usize] & matchmask) != 0 {
                let score: f32 = bitap_score(dmp, d as i32, j - 1, loc, patern.len());
                if score <= score_threshold {
                    score_threshold = score;
                    best_loc = j - 1;
                    best_level = d;
                    if best_loc > loc {
                        start = max(1, 2 * loc - best_loc);
                        hits.start_lowered += (start < first_start) as usize;
                    } else {
                        hits.passed_loc += 1;
                        break;
                    }
                }
            }
            j -= 1;
        }
        hits.unwritten_read += read_unwritten as usize;
        if bitap_score(dmp, d as i32 + 1, loc, loc, patern.len()) > score_threshold {
            hits.no_hope += 1;
            break;
        }
        last_rd = rd;
        last_low = low;
    }
    match (best_loc, best_level) {
        (-1, _) => hits.no_match += 1,
        (_, 0) => hits.exact += 1,
        _ => hits.fuzzy += 1,
    }
    best_loc
}

// A text over two or three letters, mostly short, sometimes far longer than
// the search window; a pattern cut from it with a few chars changed, or
// random, sometimes empty or past the length limit; a location in or near the text,
// sometimes far before or past it.
fn random_case(rng: &mut Lcg, max_m: usize) -> (Vec<char>, Vec<char>, i32) {
    let letters = [['a', 'b', 'a'], ['a', 'b', 'c']][rng.below(2)];
    let len = if rng.below(12) == 0 {
        2000 + rng.below(2001)
    } else {
        rng.below(201)
    };
    let text: Vec<char> = (0..len).map(|_| letters[rng.below(3)]).collect();
    let m = match rng.below(30) {
        0 => 0,
        1 | 2 => max_m + 1 + rng.below(6),
        _ => 1 + rng.below(max_m as u64),
    };
    let mut patern: Vec<char> = if m <= len && rng.below(4) != 0 {
        let at = rng.below((len - m + 1) as u64);
        text[at..at + m].to_vec()
    } else {
        (0..m).map(|_| letters[rng.below(3)]).collect()
    };
    for _ in 0..rng.below(4) {
        if m > 0 {
            patern[rng.below(m as u64)] = letters[rng.below(3)];
        }
    }
    let loc = match rng.below(40) {
        0 => -6 - rng.below(1000) as i32,
        1 => (len + 6 + rng.below(1000)) as i32,
        2 => (1 << 30) + rng.below(1000) as i32,
        _ => rng.below(len as u64 + 11) as i32 - 5,
    };
    (text, patern, loc)
}

#[test]
fn bitap_window_matches_whole_arrays() {
    let mut rng = Lcg(3);
    let mut hits = BitapHits::default();
    let thresholds = [0.0, 0.2, 0.5, 0.8, 1.0];
    let distances = [0, 10, 100, 1000];
    let maxbits = [32, 8, 0];
    for round in 0..6_000 {
        let mut dmp = Dmp::new();
        dmp.match_threshold = thresholds[round % 5];
        dmp.match_distance = distances[round / 5 % 4];
        dmp.match_maxbits = maxbits[round / 20 % 3];
        let max_m = [32, 8, 64][round / 20 % 3];
        let (text, patern, loc) = random_case(&mut rng, max_m);
        let expected = catch_unwind(AssertUnwindSafe(|| {
            bitap_whole_arrays(&dmp, &text, &patern, loc, &mut hits)
        }))
        .ok();
        let actual = catch_unwind(AssertUnwindSafe(|| dmp.match_bitap(&text, &patern, loc))).ok();
        hits.negative_loc += (loc < 0) as usize;
        hits.panicked += expected.is_none() as usize;
        assert_eq!(
            actual, expected,
            "{} {} {} {text:?} {patern:?} {loc}",
            dmp.match_threshold, dmp.match_distance, dmp.match_maxbits
        );
    }
    assert_eq!(
        hits,
        BitapHits {
            no_match: 2226,
            exact: 1107,
            fuzzy: 1948,
            multi_level: 3199,
            start_lowered: 583,
            passed_loc: 3939,
            unwritten_read: 10,
            no_hope: 5142,
            negative_loc: 529,
            panicked: 719,
        }
    );
}
