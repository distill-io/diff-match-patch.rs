// Bitap fuzzy match: locate a pattern in text near an expected location.

use crate::engine;
use crate::types::{max, min, min1, Dmp};
use std::collections::HashMap;

// The historic public API takes &Vec/&mut Vec/&String; frozen by the
// drop-in compatibility contract.
#[allow(clippy::ptr_arg)]
impl Dmp {
    /// Locate the best instance of 'pattern' in 'text' near 'loc'.
    ///
    /// Args:
    /// text: The text to search.
    /// pattern: The pattern to search for.
    /// loc: The location to search around.
    ///
    /// Returns:
    /// Best match index or -1.
    pub fn match_main(&mut self, text1: &str, patern1: &str, mut loc: i32) -> i32 {
        loc = max(0, min(loc, text1.len() as i32));
        if patern1.is_empty() {
            return loc;
        }
        if text1.is_empty() {
            return -1;
        }
        let text: Vec<char> = (text1.to_string()).chars().collect();
        let patern: Vec<char> = (patern1.to_string()).chars().collect();
        if text == patern {
            // Shortcut (potentially not guaranteed by the algorithm)
            return 0;
        } else if loc as usize + patern.len() <= text.len()
            && text[(loc as usize)..(loc as usize + patern.len())].to_vec() == patern
        {
            // Perfect match at the perfect spot!  (Includes case of null pattern)
            return loc;
        }
        self.match_bitap(&text, &patern, loc)
    }

    /// Locate the best instance of 'pattern' in 'text' near 'loc' using the
    /// Bitap algorithm.
    ///
    /// Args:
    /// text: The text to search.
    /// pattern: The pattern to search for.
    /// loc: The location to search around.
    ///
    /// Returns:
    /// Best match index or -1.
    pub fn match_bitap(&mut self, text: &Vec<char>, patern: &Vec<char>, loc: i32) -> i32 {
        // check for maxbits limit.
        if !(self.match_maxbits == 0 || patern.len() as i32 <= self.match_maxbits) {
            panic!("patern too long for this application");
        }
        // With match_maxbits = 0 ("no limit") the u64 bit vectors still cap the
        // pattern at 64 tokens; fail clearly instead of overflowing the shifts.
        if patern.len() > 64 {
            panic!("patern too long for this application");
        }
        // Initialise the alphabet.
        let s: HashMap<char, u64> = alphabet(patern);

        // Highest score beyond which we give up.
        let mut score_threshold: f32 = self.match_threshold;
        // Is there a nearby exact match? (speedup)
        let mut best_loc = match engine::find_sub(text, patern, loc as usize) {
            Some(i) => i as i32,
            None => -1,
        };
        if best_loc != -1 {
            score_threshold = min1(
                self.match_bitap_score(0, best_loc, loc, patern),
                score_threshold,
            );
            // What about in the other direction? (speedup)
            best_loc = match engine::rfind_sub(text, patern, loc as usize + patern.len()) {
                Some(i) => i as i32,
                None => -1,
            };
            if best_loc != -1 {
                score_threshold = min1(
                    score_threshold,
                    self.match_bitap_score(0, best_loc, loc, patern),
                );
            }
        }
        // Initialise the bit arrays.
        let matchmask: u64 = 1 << (patern.len() - 1);
        best_loc = -1;
        let mut bin_min: i32;
        let mut bin_mid: i32;
        let mut bin_max: i32 = (patern.len() + text.len()) as i32;
        // Empty initialization added to appease pychecker.
        let mut last_rd: Vec<u64> = vec![];
        for d in 0..patern.len() {
            /*
            Scan for the best match each iteration allows for one more error.
            Run a binary search to determine how far from 'loc' we can stray at
            this error level.
            */
            let mut rd: Vec<u64> = vec![];
            bin_min = 0;
            bin_mid = bin_max;
            // Use the result from this iteration as the maximum for the next.
            while bin_min < bin_mid {
                if self.match_bitap_score(d as i32, loc + bin_mid, loc, patern) <= score_threshold {
                    bin_min = bin_mid;
                } else {
                    bin_max = bin_mid;
                }
                bin_mid = bin_min + (bin_max - bin_min) / 2;
            }
            bin_max = bin_mid;
            let mut start = max(1, loc - bin_mid + 1);
            let finish = min(loc + bin_mid, text.len() as i32) + patern.len() as i32;
            rd.resize((finish + 2) as usize, 0);
            rd[(finish + 1) as usize] = (1u64 << d) - 1;
            let mut j = finish;
            while j >= start {
                let char_match: u64;
                if text.len() < j as usize {
                    // Out of range.
                    char_match = 0;
                } else {
                    // Subsequent passes: fuzzy match.
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
                    // First pass: exact match.
                    rd[j as usize] = ((rd[j as usize + 1] << 1) | 1) & char_match;
                } else {
                    rd[j as usize] = (((rd[j as usize + 1] << 1) | 1) & char_match)
                        | (((last_rd[j as usize + 1] | last_rd[j as usize]) << 1) | 1)
                        | last_rd[j as usize + 1];
                }
                if (rd[j as usize] & matchmask) != 0 {
                    let score: f32 = self.match_bitap_score(d as i32, j - 1, loc, patern);
                    // This match will almost certainly be better than any existing match.
                    // But check anyway.
                    if score <= score_threshold {
                        // Told you so.
                        score_threshold = score;
                        best_loc = j - 1;
                        if best_loc > loc {
                            // When passing loc, don't exceed our current distance from loc.
                            start = max(1, 2 * loc - best_loc);
                        } else {
                            // Already passed loc, downhill from here on in.
                            break;
                        }
                    }
                }
                j -= 1;
            }
            // No hope for a (better) match at greater error levels.
            if self.match_bitap_score(d as i32 + 1, loc, loc, patern) > score_threshold {
                break;
            }
            last_rd = rd;
        }
        best_loc
    }

    /// Compute and return the score for a match with e errors and x location.
    /// Accesses loc and pattern through being a closure.
    ///
    /// Args:
    /// e: Number of errors in match.
    /// x: Location of match.
    ///
    /// Returns:
    /// Overall score for match (0.0 = good, 1.0 = bad).
    pub fn match_bitap_score(&mut self, e: i32, x: i32, loc: i32, patern: &Vec<char>) -> f32 {
        let accuracy: f32 = (e as f32) / (patern.len() as f32);
        let proximity: i32 = (loc - x).abs();
        if self.match_distance == 0 {
            // Dodge divide by zero error.
            if proximity == 0 {
                return accuracy;
            } else {
                return 1.0;
            }
        }
        accuracy + ((proximity as f32) / (self.match_distance as f32))
    }
    /// Initialise the alphabet for the Bitap algorithm.
    ///
    /// Args:
    /// pattern: The text to encode.
    ///
    /// Returns:
    /// Hash of character locations. Public i32 view of the internal u64
    /// masks; bit 31 lands in the sign bit exactly as the pre-rewrite
    /// release builds computed it.
    pub fn match_alphabet(&mut self, patern: &Vec<char>) -> HashMap<char, i32> {
        alphabet(patern)
            .into_iter()
            .map(|(ch, mask)| (ch, mask as u32 as i32))
            .collect()
    }
}

/// Bitap alphabet over u64 masks (safe for patterns up to match_maxbits = 32,
/// and beyond up to 64 tokens).
fn alphabet(patern: &[char]) -> HashMap<char, u64> {
    let mut s: HashMap<char, u64> = HashMap::new();
    for &ch in patern {
        s.insert(ch, 0);
    }
    for (i, &ch) in patern.iter().enumerate() {
        let mask = s[&ch] | (1u64 << (patern.len() - i - 1));
        s.insert(ch, mask);
    }
    s
}
