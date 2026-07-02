// Generic DMP diff primitives over token slices: common prefix/suffix, KMP
// substring search, half-match, and the Myers middle-snake bisect. Pure
// functions over `&[T: Eq]` — no Dmp state, no text; orchestration and
// text materialization live in diff.rs.

use std::time::Instant;

/// Number of leading tokens common to `a` and `b`.
pub(crate) fn common_prefix<T: Eq>(a: &[T], b: &[T]) -> usize {
    let n = a.len().min(b.len());
    for i in 0..n {
        if a[i] != b[i] {
            return i;
        }
    }
    n
}

/// Number of trailing tokens common to `a` and `b`.
pub(crate) fn common_suffix<T: Eq>(a: &[T], b: &[T]) -> usize {
    let n = a.len().min(b.len());
    for i in 0..n {
        if a[a.len() - 1 - i] != b[b.len() - 1 - i] {
            return i;
        }
    }
    n
}

/// Longest suffix of `a` that is a prefix of `b` (DMP diff_commonOverlap).
pub(crate) fn common_overlap<T: Eq>(a: &[T], b: &[T]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    // Truncate the longer side so both windows have equal length.
    let len = a.len().min(b.len());
    let a = &a[a.len() - len..];
    let b = &b[..len];
    if a == b {
        return len;
    }
    /*Start by looking for a single token match
    and increase length until no match is found.
    Performance analysis: https://neil.fraser.name/news/2010/11/04/ */
    let mut best = 0;
    let mut length = 1;
    loop {
        let pattern = &a[len - length..];
        match find_sub(b, pattern, 0) {
            None => return best,
            Some(found) => {
                length += found;
                if found == 0 {
                    best = length;
                    length += 1;
                }
            }
        }
    }
}

/// First index of `needle` in `hay` at or after `from` (KMP), or None.
pub(crate) fn find_sub<T: Eq>(hay: &[T], needle: &[T], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from);
    }
    if hay.is_empty() {
        return None;
    }
    // Preprocess the pattern's failure table.
    let mut table: Vec<usize> = vec![0];
    let mut len = 0;
    let mut i = 1;
    while i < needle.len() {
        if needle[i] == needle[len] {
            len += 1;
            table.push(len);
            i += 1;
        } else if len == 0 {
            table.push(0);
            i += 1;
        } else {
            len = table[len - 1];
        }
    }
    let mut i = from;
    let mut len = 0;
    while i < hay.len() {
        if hay[i] == needle[len] {
            len += 1;
            i += 1;
            if len == needle.len() {
                return Some(i - len);
            }
        } else if len == 0 {
            i += 1;
        } else {
            len = table[len - 1];
        }
    }
    None
}

/// Last index at or before `until` where `needle` starts in `hay` (KMP from
/// the front, keeping the final hit), or None. A match may end at `until + 1`,
/// i.e. its last token at index `until`.
pub(crate) fn rfind_sub<T: Eq>(hay: &[T], needle: &[T], until: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(until);
    }
    if hay.is_empty() {
        return None;
    }
    let mut table: Vec<usize> = vec![0];
    let mut len = 0;
    let mut i = 1;
    while i < needle.len() {
        if needle[i] == needle[len] {
            len += 1;
            table.push(len);
            i += 1;
        } else if len == 0 {
            table.push(0);
            i += 1;
        } else {
            len = table[len - 1];
        }
    }
    let mut i = 0;
    let mut len = 0;
    let mut last: Option<usize> = None;
    while i <= until {
        if i < hay.len() && hay[i] == needle[len] {
            len += 1;
            i += 1;
            if len == needle.len() {
                last = Some(i - len);
                len = table[len - 1];
            }
        } else if len == 0 {
            i += 1;
        } else {
            len = table[len - 1];
        }
    }
    last
}

/// A half-match split: `old = old[..old_a] + common + old[old_a+common..]`
/// and `new = new[..new_a] + common + new[new_a+common..]`, where the shared
/// `common` run is at least half the longer input.
pub(crate) struct HalfMatch {
    pub old_a: usize,
    pub new_a: usize,
    pub common: usize,
}

/// DMP diff_halfMatch over token slices. Caller gates this on the timeout
/// setting (half-match trades optimality for speed).
pub(crate) fn half_match<T: Eq>(old: &[T], new: &[T]) -> Option<HalfMatch> {
    let old_is_long = old.len() > new.len();
    let (long, short) = if old_is_long { (old, new) } else { (new, old) };
    if long.len() < 4 || short.len() * 2 < long.len() {
        return None;
    }

    // First check if the second quarter is the seed for a half-match,
    // then check again based on the third quarter.
    let hm1 = half_match_at(long, short, long.len().div_ceil(4));
    let hm2 = half_match_at(long, short, long.len().div_ceil(2));
    let hm = match (hm1, hm2) {
        (None, None) => return None,
        (Some(h), None) => h,
        (None, Some(h)) => h,
        // Both matched: select the longest (ties prefer the second, as upstream).
        (Some(h1), Some(h2)) => {
            if h1.2 > h2.2 {
                h1
            } else {
                h2
            }
        }
    };
    let (long_a, short_a, common) = hm;
    Some(if old_is_long {
        HalfMatch {
            old_a: long_a,
            new_a: short_a,
            common,
        }
    } else {
        HalfMatch {
            old_a: short_a,
            new_a: long_a,
            common,
        }
    })
}

/// Does a substring of `short` exist within `long` such that the substring is
/// at least half the length of `long`, seeded at long[i..]? Returns
/// (long_prefix_len, short_prefix_len, common_len).
fn half_match_at<T: Eq>(long: &[T], short: &[T], i: usize) -> Option<(usize, usize, usize)> {
    let seed = &long[i..i + long.len() / 4];
    let mut best_common = 0;
    let mut best = (0, 0);
    let mut j = find_sub(short, seed, 0);
    while let Some(jv) = j {
        let prefix = common_prefix(&long[i..], &short[jv..]);
        let suffix = common_suffix(&long[..i], &short[..jv]);
        if best_common < suffix + prefix {
            best_common = suffix + prefix;
            best = (i - suffix, jv - suffix);
        }
        j = find_sub(short, seed, jv + 1);
    }
    if best_common * 2 >= long.len() {
        Some((best.0, best.1, best_common))
    } else {
        None
    }
}

/// DMP diff_bisect middle-snake search. Returns the split point (x, y) where
/// the forward and reverse d-paths overlap, or None when the inputs share no
/// overlap (or the deadline expired) and the caller should emit delete+insert.
///
/// Faithful port including the negative-index wrap guards: the k-walk can
/// probe with x or y at -1, and the original (Python-derived) indexing wraps
/// from the end of the slice in that case.
pub(crate) fn bisect<T: Eq>(
    old: &[T],
    new: &[T],
    deadline: Option<Instant>,
) -> Option<(usize, usize)> {
    let text1_length = old.len() as i32;
    let text2_length = new.len() as i32;
    let max_d: i32 = (text1_length + text2_length + 1) / 2;
    let v_offset: i32 = max_d;
    let v_length: i32 = 2 * max_d;
    let mut v1: Vec<i32> = vec![-1; v_length as usize];
    let mut v2: Vec<i32> = vec![-1; v_length as usize];
    v1[v_offset as usize + 1] = 0;
    v2[v_offset as usize + 1] = 0;
    let delta: i32 = text1_length - text2_length;
    // If the total number of characters is odd, then the front path will
    // collide with the reverse path.
    let front: bool = delta % 2 != 0;
    // Offsets for start and end of k loop.
    // Prevents mapping of space beyond the grid.
    let mut k1start: i32 = 0;
    let mut k1end: i32 = 0;
    let mut k2start: i32 = 0;
    let mut k2end: i32 = 0;
    for d in 0..max_d {
        if let Some(deadline) = deadline {
            if Instant::now() >= deadline {
                break;
            }
        }

        // Walk the front path one step.
        let mut k1 = -d + k1start;
        while k1 < d + 1 - k1end {
            let k1_offset = v_offset + k1;
            let mut x1: i32;
            if k1 == -d || (k1 != d && v1[k1_offset as usize - 1] < v1[k1_offset as usize + 1]) {
                x1 = v1[k1_offset as usize + 1];
            } else {
                x1 = v1[k1_offset as usize - 1] + 1;
            }
            let mut y1 = x1 - k1;
            while x1 < text1_length && y1 < text2_length {
                let i1 = if x1 < 0 { text1_length + x1 } else { x1 };
                let i2 = if y1 < 0 { text2_length + y1 } else { y1 };
                if old[i1 as usize] != new[i2 as usize] {
                    break;
                }
                x1 += 1;
                y1 += 1;
            }
            v1[k1_offset as usize] = x1;
            if x1 > text1_length {
                // Ran off the right of the graph.
                k1end += 2;
            } else if y1 > text2_length {
                // Ran off the bottom of the graph.
                k1start += 2;
            } else if front {
                let k2_offset = v_offset + delta - k1;
                if k2_offset >= 0 && k2_offset < v_length && v2[k2_offset as usize] != -1 {
                    // Mirror x2 onto top-left coordinate system.
                    let x2 = text1_length - v2[k2_offset as usize];
                    if x1 >= x2 {
                        // Overlap detected.
                        return Some((x1 as usize, y1 as usize));
                    }
                }
            }
            k1 += 2;
        }

        // Walk the reverse path one step.
        let mut k2 = -d + k2start;
        while k2 < d + 1 - k2end {
            let k2_offset = v_offset + k2;
            let mut x2: i32;
            if k2 == -d || (k2 != d && v2[k2_offset as usize - 1] < v2[k2_offset as usize + 1]) {
                x2 = v2[k2_offset as usize + 1];
            } else {
                x2 = v2[k2_offset as usize - 1] + 1;
            }
            let mut y2 = x2 - k2;
            while x2 < text1_length && y2 < text2_length {
                let i1 = if text1_length - x2 > 0 {
                    text1_length - x2 - 1
                } else {
                    x2 + 1
                };
                let i2 = if text2_length - y2 > 0 {
                    text2_length - y2 - 1
                } else {
                    y2 + 1
                };
                if old[i1 as usize] != new[i2 as usize] {
                    break;
                }
                x2 += 1;
                y2 += 1;
            }
            v2[k2_offset as usize] = x2;
            if x2 > text1_length {
                // Ran off the left of the graph.
                k2end += 2;
            } else if y2 > text2_length {
                // Ran off the top of the graph.
                k2start += 2;
            } else if !front {
                let k1_offset = v_offset + delta - k2;
                if k1_offset >= 0 && k1_offset < v_length && v1[k1_offset as usize] != -1 {
                    let x1 = v1[k1_offset as usize];
                    let y1 = v_offset + x1 - k1_offset;
                    // Mirror x2 onto top-left coordinate system.
                    let x2 = text1_length - x2;
                    if x1 >= x2 {
                        // Overlap detected.
                        return Some((x1 as usize, y1 as usize));
                    }
                }
            }
            k2 += 2;
        }
    }
    // Number of diffs equals number of tokens, no commonality at all — or the
    // deadline expired.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_prefix_suffix_over_bytes() {
        assert_eq!(common_prefix(b"1234abcdef", b"1234xyz"), 4);
        assert_eq!(common_prefix(b"abc", b"xyz"), 0);
        assert_eq!(common_prefix(b"1234", b"1234xyz"), 4);
        assert_eq!(common_suffix(b"abcdef1234", b"xyz1234"), 4);
        assert_eq!(common_suffix(b"abc", b"xyz"), 0);
        assert_eq!(common_suffix(b"1234", b"xyz1234"), 4);
        let empty: &[u8] = b"";
        assert_eq!(common_prefix(empty, b"a"), 0);
        assert_eq!(common_suffix(empty, b"a"), 0);
    }

    #[test]
    fn common_overlap_over_bytes() {
        let empty: &[u8] = b"";
        assert_eq!(common_overlap(empty, b"abcd"), 0);
        assert_eq!(common_overlap(b"abc", b"abcd"), 3);
        assert_eq!(common_overlap(b"123456", b"abcd"), 0);
        assert_eq!(common_overlap(b"123456xxx", b"xxxabcd"), 3);
        // Unicode-motivated case from the canonical suite, expressed on bytes.
        assert_eq!(common_overlap(b"fi", b"\x01ab"), 0);
    }

    #[test]
    fn find_sub_over_bytes() {
        assert_eq!(find_sub(b"abcdefabcdef", b"cde", 0), Some(2));
        assert_eq!(find_sub(b"abcdefabcdef", b"cde", 3), Some(8));
        assert_eq!(find_sub(b"abcdef", b"xyz", 0), None);
        assert_eq!(find_sub(b"abc", b"", 1), Some(1));
        assert_eq!(find_sub(b"", b"a", 0), None);
        assert_eq!(find_sub(b"aaa", b"aaaa", 0), None);
    }

    #[test]
    fn rfind_sub_over_bytes() {
        assert_eq!(rfind_sub(b"abcabcabc", b"abc", 8), Some(6));
        assert_eq!(rfind_sub(b"abcabcabc", b"abc", 5), Some(3));
        // A match may end exactly at until + 1 (last token at index `until`).
        assert_eq!(rfind_sub(b"xxabc", b"abc", 4), Some(2));
        assert_eq!(rfind_sub(b"xxabc", b"abc", 3), None);
        // `until` at or past hay.len() must scan safely to the end.
        assert_eq!(rfind_sub(b"ab", b"b", 2), Some(1));
        assert_eq!(rfind_sub(b"ab", b"b", 9), Some(1));
        assert_eq!(rfind_sub(b"abc", b"", 2), Some(2));
        assert_eq!(rfind_sub(b"", b"a", 0), None);
    }

    #[test]
    fn half_match_over_bytes() {
        // No half-match: shared run shorter than half the longer input.
        assert!(half_match(b"1234567890".as_slice(), b"abcdef".as_slice()).is_none());
        assert!(half_match(b"12345".as_slice(), b"23".as_slice()).is_none());
        // Single half-match: 1234567890 vs a345678z shares "345678".
        let hm = half_match(b"1234567890".as_slice(), b"a345678z".as_slice()).unwrap();
        assert_eq!((hm.old_a, hm.new_a, hm.common), (2, 1, 6));
        // Swapped sides mirror the split.
        let hm = half_match(b"a345678z".as_slice(), b"1234567890".as_slice()).unwrap();
        assert_eq!((hm.old_a, hm.new_a, hm.common), (1, 2, 6));
    }

    #[test]
    fn bisect_over_bytes() {
        // "cat" vs "map": the reverse d=2 walk detects overlap at (2, 2)
        // (hand-traced against the pre-rewrite implementation).
        assert_eq!(bisect(b"cat", b"map", None), Some((2, 2)));
        // No shared tokens at all: no overlap to split on.
        assert_eq!(bisect(b"abc", b"xyz", None), None);
        // Expired deadline: give up immediately.
        let past = Instant::now() - std::time::Duration::from_secs(1);
        assert_eq!(bisect(b"cat", b"map", Some(past)), None);
    }
}
