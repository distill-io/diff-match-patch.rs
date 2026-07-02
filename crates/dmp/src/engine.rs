// Generic DMP diff primitives over token slices: common prefix/suffix, KMP
// substring search, half-match, and the Myers middle-snake bisect. Pure
// functions over `&[T: Eq]` — no Dmp state, no text; orchestration and
// text materialization live in diff.rs.

use std::time::Instant;

// Chunk width for the common-run scans. Runs are scanned token by token
// first — most probes (e.g. the bisect snake walk) mismatch within a couple
// of tokens, where chunk compares would read far more than they save — and
// switch to block compares (a memcmp shape the compiler vectorizes) only
// once a run survives one full chunk.
const RUN_CHUNK: usize = 16;

/// Number of leading tokens common to `a` and `b`.
pub(crate) fn common_prefix<T: Eq>(a: &[T], b: &[T]) -> usize {
    let n = a.len().min(b.len());
    let scalar_end = n.min(RUN_CHUNK);
    let mut i = 0;
    while i < scalar_end && a[i] == b[i] {
        i += 1;
    }
    if i == RUN_CHUNK {
        while i + RUN_CHUNK <= n && a[i..i + RUN_CHUNK] == b[i..i + RUN_CHUNK] {
            i += RUN_CHUNK;
        }
        while i < n && a[i] == b[i] {
            i += 1;
        }
    }
    i
}

/// Number of trailing tokens common to `a` and `b`.
pub(crate) fn common_suffix<T: Eq>(a: &[T], b: &[T]) -> usize {
    let n = a.len().min(b.len());
    let scalar_end = n.min(RUN_CHUNK);
    let mut i = 0;
    while i < scalar_end && a[a.len() - 1 - i] == b[b.len() - 1 - i] {
        i += 1;
    }
    if i == RUN_CHUNK {
        while i + RUN_CHUNK <= n
            && a[a.len() - i - RUN_CHUNK..a.len() - i] == b[b.len() - i - RUN_CHUNK..b.len() - i]
        {
            i += RUN_CHUNK;
        }
        while i < n && a[a.len() - 1 - i] == b[b.len() - 1 - i] {
            i += 1;
        }
    }
    i
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
    // One KMP pass of `b` over `a`: the automaton state after consuming all
    // of `a` is exactly the longest prefix of `b` that is a suffix of `a` —
    // the same answer as the reference's iterative grow-and-search loop
    // (https://neil.fraser.name/news/2010/11/04/) without its per-probe
    // failure-table rebuilds. A full occurrence of `b` inside `a` cannot
    // happen mid-scan: the windows have equal length and a == b returned
    // above, so the state peaks at the final position.
    let mut table: Vec<usize> = vec![0];
    let mut state = 0;
    let mut i = 1;
    while i < b.len() {
        if b[i] == b[state] {
            state += 1;
            table.push(state);
            i += 1;
        } else if state == 0 {
            table.push(0);
            i += 1;
        } else {
            state = table[state - 1];
        }
    }
    let mut state = 0;
    let mut i = 0;
    while i < a.len() {
        if a[i] == b[state] {
            state += 1;
            i += 1;
        } else if state == 0 {
            match skip_to(a, i + 1, &b[0]) {
                Some(j) => i = j,
                None => return 0,
            }
        } else {
            state = table[state - 1];
        }
    }
    state
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
            // Classic KMP single-steps through every position that cannot
            // start a match; hunt the next candidate with the chunked scan
            // instead (identical positions skipped, O(n+m) preserved).
            match skip_to(hay, i + 1, &needle[0]) {
                Some(j) => i = j,
                None => return None,
            }
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
            // As in find_sub: jump to the next possible start. No further
            // occurrence also covers the historical i-past-the-text spin
            // (until may exceed the text; nothing could match there).
            match skip_to(hay, i + 1, &needle[0]) {
                Some(j) => i = j,
                None => return last,
            }
        } else {
            len = table[len - 1];
        }
    }
    last
}

/// First index at or after `from` where `target` occurs in `hay`. Scans in
/// 16-token blocks with a branch-free OR-reduction per block (vectorizes),
/// refining the hit position token by token.
pub(crate) fn skip_to<T: Eq>(hay: &[T], from: usize, target: &T) -> Option<usize> {
    const SKIP_CHUNK: usize = 16;
    let mut i = from;
    while i + SKIP_CHUNK <= hay.len() {
        let mut any = false;
        for token in &hay[i..i + SKIP_CHUNK] {
            any |= token == target;
        }
        if any {
            break;
        }
        i += SKIP_CHUNK;
    }
    while i < hay.len() {
        if hay[i] == *target {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Coarse frequency rank for the skip-token pick: HIGHER = more common in
/// prose/markup, so the scanner prefers the LOWEST-ranked needle token. Only
/// the ordering quality matters (a bad pick costs speed, never correctness),
/// so tiers are enough.
fn skip_rank(c: char) -> u8 {
    match c {
        ' ' => 255,
        'e' | 't' | 'a' | 'o' | 'i' | 'n' | 's' | 'r' | 'h' | 'l' => 250,
        'd' | 'c' | 'u' | 'm' | 'f' | 'g' | 'p' | 'w' | 'y' | 'b' | '\n' => 240,
        'v' | 'k' | '.' | ',' | '-' | '\'' | '"' | ';' | ':' | '=' | '/' | '<' | '>' => 220,
        '0'..='9' => 200,
        'A'..='Z' => 190,
        c if c.is_ascii_graphic() => 150,
        c if c.is_ascii() => 120,
        // Non-ASCII: rare in mostly-ASCII documents; in fully non-ASCII
        // documents every token ties and the pick degrades to token order.
        _ => 100,
    }
}

/// Index of the needle token the skip scan should hunt for.
fn rarest_index(needle: &[char]) -> usize {
    let mut best = 0;
    let mut best_rank = u8::MAX;
    for (i, &c) in needle.iter().enumerate() {
        let rank = skip_rank(c);
        if rank < best_rank {
            best_rank = rank;
            best = i;
        }
    }
    best
}

/// Does `needle` occur at least twice in `hay`? Skip-scan for the needle's
/// rarest token at its offset and verify candidates by slice equality,
/// stopping at the second hit — the streaming equivalent of
/// `find_sub(..) != rfind_sub(..)` for non-empty needles (overlapping
/// occurrences count, exactly as distinct first/last indices do). Callers
/// special-case empty needles.
///
/// O(n·m) worst case, unlike KMP's O(n+m) — chosen deliberately: the only
/// caller (patch_add_context) grows needles only while they are shorter than
/// match_maxbits − 2·patch_margin (≈ a couple dozen tokens), so the verify
/// is a short vectorized compare and the skip scan dominates. Char-concrete
/// (unlike its siblings) because the rarity heuristic is alphabet-specific.
pub(crate) fn occurs_twice(hay: &[char], needle: &[char]) -> bool {
    if needle.is_empty() || hay.len() < needle.len() {
        return false;
    }
    let k = rarest_index(needle);
    let target = &needle[k];
    let last_start = hay.len() - needle.len();
    let mut found = false;
    let mut p = k;
    while let Some(hit) = skip_to(hay, p, target) {
        // hit >= p >= k, so the candidate start never underflows.
        let j = hit - k;
        if j > last_start {
            break;
        }
        if hay[j..j + needle.len()] == *needle {
            if found {
                return true;
            }
            found = true;
        }
        p = hit + 1;
    }
    false
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
    // Callers guarantee long.len() >= 4, so the seed is never empty.
    let seed = &long[i..i + long.len() / 4];
    let mut best_common = 0;
    let mut best = (0, 0);

    // One KMP pass over `short` (failure table built once) visits every seed
    // occurrence in ascending order — the same visits as restarting find_sub
    // at jv + 1, without rebuilding the table per occurrence. On repetitive
    // text the seed recurs every period, so this is what keeps the deadline
    // path from degenerating.
    let mut table: Vec<usize> = vec![0];
    let mut len = 0;
    let mut k = 1;
    while k < seed.len() {
        if seed[k] == seed[len] {
            len += 1;
            table.push(len);
            k += 1;
        } else if len == 0 {
            table.push(0);
            k += 1;
        } else {
            len = table[len - 1];
        }
    }
    let mut pos = 0;
    let mut len = 0;
    while pos < short.len() {
        if short[pos] == seed[len] {
            pos += 1;
            len += 1;
            if len == seed.len() {
                let jv = pos - len;
                // Upper bound on what this occurrence could score; skip the
                // O(n) prefix/suffix extensions when it cannot beat the best
                // (best updates require strictly-greater, preserving the
                // first-best-wins tie behavior).
                let cap = (long.len() - i).min(short.len() - jv) + i.min(jv);
                if cap > best_common {
                    let prefix = common_prefix(&long[i..], &short[jv..]);
                    let suffix = common_suffix(&long[..i], &short[..jv]);
                    if best_common < suffix + prefix {
                        best_common = suffix + prefix;
                        best = (i - suffix, jv - suffix);
                    }
                }
                len = table[len - 1];
            }
        } else if len == 0 {
            pos += 1;
        } else {
            len = table[len - 1];
        }
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
    fn occurs_twice_over_chars() {
        let c = |s: &str| s.chars().collect::<Vec<char>>();
        let ot = |hay: &str, needle: &str| occurs_twice(&c(hay), &c(needle));
        assert!(!ot("abcdef", "cde")); // exactly one occurrence
        assert!(ot("abcdefabc", "abc")); // two, disjoint
        assert!(ot("aaaa", "aaa")); // two, overlapping
        assert!(!ot("abc", "xyz")); // zero
        assert!(!ot("", "a"));
        assert!(!ot("ab", "abc")); // needle longer than hay
                                   // A trailing occurrence must count even when the skip hit is past
                                   // the last possible start for a *second* one.
        assert!(ot("abxxxxxxxxxxxxxxxxxxab", "ab"));
        assert!(!ot("axxxxxxxxxxxxxxxxxxxab", "ab"));
        // The skip token is the rare one mid-needle ('Q' here), and
        // occurrences sharing that token's neighbors still all count.
        assert!(ot("eee eQe eee eQe", "eQe"));
        assert!(!ot("eee eQe eee eee", "eQe"));
        assert!(ot("aQaQa", "aQa")); // overlapping around a mid-needle pick
                                     // Empty needles are the caller's special case, never "twice" here.
        assert!(!ot("ab", ""));
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
