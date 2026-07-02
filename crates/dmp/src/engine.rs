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
    let mut kmp: Option<Kmp<T>> = None;
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
            state = kmp.get_or_insert_with(|| Kmp::new(b)).fail(state);
        }
    }
    state
}

/// Lazily-built KMP failure automaton. `fail(state)` returns the table entry
/// for `state` (the length to fall back to on a mismatch), extending the
/// table only as far as the highest state actually consulted — so a scan that
/// never sustains a long partial match (common_overlap on unrelated prose
/// ends, find_sub on non-repetitive text) builds a handful of entries instead
/// of the whole O(m) table. Only constructed on the first state > 0 mismatch,
/// so scans that never backtrack allocate nothing.
struct Kmp<'a, T> {
    needle: &'a [T],
    /// table[k] = longest proper prefix of needle[..=k] that is also a suffix.
    table: Vec<usize>,
    /// Running prefix-function length and next index for incremental extension.
    len: usize,
    next: usize,
}

impl<'a, T: Eq> Kmp<'a, T> {
    fn new(needle: &'a [T]) -> Self {
        Kmp {
            needle,
            table: vec![0],
            len: 0,
            next: 1,
        }
    }

    /// The failure value consulted on a mismatch at automaton `state` (> 0):
    /// `table[state - 1]`, building the prefix function up to that index on
    /// demand. Total build work across all calls is O(highest state reached).
    fn fail(&mut self, state: usize) -> usize {
        let k = state - 1;
        while self.table.len() <= k {
            let i = self.next;
            while self.len > 0 && self.needle[i] != self.needle[self.len] {
                self.len = self.table[self.len - 1];
            }
            if self.needle[i] == self.needle[self.len] {
                self.len += 1;
            }
            self.table.push(self.len);
            self.next += 1;
        }
        self.table[k]
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
    let mut kmp: Option<Kmp<T>> = None;
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
            len = kmp.get_or_insert_with(|| Kmp::new(needle)).fail(len);
        }
    }
    None
}

/// First index of `needle` in `hay`, or None — the same result as
/// `find_sub(hay, needle, 0)`, but tuned for the diff containment check where
/// `needle` (the shorter side) is often nearly as long as `hay` and shares no
/// prefix or suffix with it.
///
/// It anchors on `needle[0]` with the chunked skip scan and verifies each
/// candidate with the vectorized `common_prefix`, building no KMP automaton
/// at all — the failure-table build dominated single-line re-diffs, because a
/// large needle drove a large table at every recursion level. A comparison
/// budget caps the verify work; a periodic needle that keeps matching deeply
/// exhausts it and falls back to KMP `find_sub`, so the worst case stays
/// O(n + m) (the naive windowed search dmp-rs uses here has no such guard and
/// is ~5× slower on the repetitive/periodic shape).
pub(crate) fn contains<T: Eq>(hay: &[T], needle: &[T]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > hay.len() {
        return None;
    }
    let last_start = hay.len() - needle.len();
    // Verify-char budget: total prefix-compare work before conceding the
    // needle is periodic enough to want KMP. The monotonic skip scan is O(n)
    // on its own and not charged here.
    let mut budget: i64 = (hay.len() + needle.len()) as i64;
    let mut i = 0;
    while i <= last_start {
        let j = match skip_to(hay, i, &needle[0]) {
            Some(j) if j <= last_start => j,
            _ => return None,
        };
        let matched = common_prefix(&hay[j..], needle);
        if matched == needle.len() {
            return Some(j);
        }
        budget -= matched as i64 + 1;
        if budget < 0 {
            return find_sub(hay, needle, 0);
        }
        i = j + 1;
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
    let mut kmp: Option<Kmp<T>> = None;
    let mut i = 0;
    let mut len = 0;
    let mut last: Option<usize> = None;
    while i <= until {
        if i < hay.len() && hay[i] == needle[len] {
            len += 1;
            i += 1;
            if len == needle.len() {
                last = Some(i - len);
                len = kmp.get_or_insert_with(|| Kmp::new(needle)).fail(len);
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
            len = kmp.get_or_insert_with(|| Kmp::new(needle)).fail(len);
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

    // One KMP pass over `short` visits every seed occurrence in ascending
    // order — the same visits as restarting find_sub at jv + 1, without
    // rebuilding the table per occurrence. On repetitive text the seed recurs
    // every period, so this is what keeps the deadline path from degenerating.
    // Every occurrence reaches the full seed length, so the lazy table fills
    // to seed.len() here rather than staying short.
    let mut kmp = Kmp::new(seed);
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
                len = kmp.fail(len);
            }
        } else if len == 0 {
            pos += 1;
        } else {
            len = kmp.fail(len);
        }
    }
    if best_common * 2 >= long.len() {
        Some((best.0, best.1, best_common))
    } else {
        None
    }
}

/// Extend a forward snake: match old[x]/new[y] ascending until the tokens
/// differ. The loop conditions are the bounds proofs, so the indexing
/// compiles check-free — keep this a plain scalar loop (chunked variants
/// measured slower here, in dmp-rs, and in this crate's own abandoned
/// restructure).
#[inline]
fn snake_fwd<T: Eq>(old: &[T], new: &[T], mut x: usize, mut y: usize) -> (usize, usize) {
    while x < old.len() && y < new.len() && old[x] == new[y] {
        x += 1;
        y += 1;
    }
    (x, y)
}

/// Extend a reverse snake: match from the tails, old[len-1-x]/new[len-1-y].
/// x < len bounds len-1-x, so the indexing compiles check-free.
#[inline]
fn snake_rev<T: Eq>(old: &[T], new: &[T], mut x: usize, mut y: usize) -> (usize, usize) {
    while x < old.len() && y < new.len() && old[old.len() - 1 - x] == new[new.len() - 1 - y] {
        x += 1;
        y += 1;
    }
    (x, y)
}

/// DMP diff_bisect middle-snake search. Returns the split point (x, y) where
/// the forward and reverse d-paths overlap, or None when the inputs share no
/// overlap (or the deadline expired) and the caller should emit delete+insert.
///
/// The classic ports guard the snake walks against negative x/y (Python's
/// arr[-1] wraps from the end), but the Myers invariants make that
/// unreachable: every stored v entry is a prior walk endpoint with x >= 0
/// and y = x - k >= 0, and the seed is 0. The walks therefore index usize
/// slices directly (snake_fwd/snake_rev), with the invariant pinned by
/// debug_asserts; the old guarded form spent ~18% of a bisect-bound profile
/// in bounds-check machinery. v entries stay i32 to halve the working set.
pub(crate) fn bisect<T: Eq>(
    old: &[T],
    new: &[T],
    deadline: Option<Instant>,
    scratch: &mut Vec<i32>,
) -> Option<(usize, usize)> {
    let text1_length = old.len() as i32;
    let text2_length = new.len() as i32;
    let max_d: i32 = (text1_length + text2_length + 1) / 2;
    let v_offset: i32 = max_d;
    let v_length: i32 = 2 * max_d;
    // The caller-provided scratch backs both k-line arrays: recursion and
    // rediff loops reuse one allocation (children are never larger, so after
    // the first call this is a pure memset).
    scratch.clear();
    scratch.resize(2 * v_length as usize, -1);
    let (v1, v2) = scratch.split_at_mut(v_length as usize);
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
        //
        // The v accesses go through guarded get/get_mut rather than indexing:
        // every offset is provably in range (debug_asserted), but the
        // optimizer can't see that, and the panic plumbing of checked
        // indexing costs measurably at ~one access per k-step. The -1 arm of
        // each read is dead; the k1 == d short-circuit in the branch
        // selection keeps the +1 read's value unused exactly where the old
        // indexed form would have been out of bounds.
        let mut k1 = -d + k1start;
        while k1 < d + 1 - k1end {
            let k1_offset = (v_offset + k1) as usize;
            debug_assert!(k1_offset >= 1 && k1_offset < v_length as usize);
            let v1_prev = v1.get(k1_offset - 1).copied().unwrap_or(-1);
            let v1_next = v1.get(k1_offset + 1).copied().unwrap_or(-1);
            let x_start = if k1 == -d || (k1 != d && v1_prev < v1_next) {
                v1_next
            } else {
                v1_prev + 1
            };
            let y_start = x_start - k1;
            debug_assert!(
                x_start >= 0 && y_start >= 0,
                "fwd walk start ({x_start},{y_start})"
            );
            let (x1, y1) = snake_fwd(old, new, x_start as usize, y_start as usize);
            if let Some(cell) = v1.get_mut(k1_offset) {
                *cell = x1 as i32;
            }
            if x1 > old.len() {
                // Ran off the right of the graph.
                k1end += 2;
            } else if y1 > new.len() {
                // Ran off the bottom of the graph.
                k1start += 2;
            } else if front {
                let k2_offset = v_offset + delta - k1;
                if k2_offset >= 0 {
                    if let Some(&v2k) = v2.get(k2_offset as usize) {
                        if v2k != -1 {
                            // Mirror x2 onto top-left coordinate system.
                            let x2 = text1_length - v2k;
                            if x1 as i32 >= x2 {
                                // Overlap detected.
                                return Some((x1, y1));
                            }
                        }
                    }
                }
            }
            k1 += 2;
        }

        // Walk the reverse path one step.
        let mut k2 = -d + k2start;
        while k2 < d + 1 - k2end {
            let k2_offset = (v_offset + k2) as usize;
            debug_assert!(k2_offset >= 1 && k2_offset < v_length as usize);
            let v2_prev = v2.get(k2_offset - 1).copied().unwrap_or(-1);
            let v2_next = v2.get(k2_offset + 1).copied().unwrap_or(-1);
            let x_start = if k2 == -d || (k2 != d && v2_prev < v2_next) {
                v2_next
            } else {
                v2_prev + 1
            };
            let y_start = x_start - k2;
            debug_assert!(
                x_start >= 0 && y_start >= 0,
                "rev walk start ({x_start},{y_start})"
            );
            let (x2, y2) = snake_rev(old, new, x_start as usize, y_start as usize);
            if let Some(cell) = v2.get_mut(k2_offset) {
                *cell = x2 as i32;
            }
            if x2 > old.len() {
                // Ran off the left of the graph.
                k2end += 2;
            } else if y2 > new.len() {
                // Ran off the top of the graph.
                k2start += 2;
            } else if !front {
                let k1_offset = v_offset + delta - k2;
                if k1_offset >= 0 {
                    if let Some(&v1k) = v1.get(k1_offset as usize) {
                        if v1k != -1 {
                            let y1 = v_offset + v1k - k1_offset;
                            // Mirror x2 onto top-left coordinate system.
                            if v1k >= text1_length - x2 as i32 {
                                // Overlap detected.
                                return Some((v1k as usize, y1 as usize));
                            }
                        }
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
        let mut scratch = Vec::new();
        assert_eq!(bisect(b"cat", b"map", None, &mut scratch), Some((2, 2)));
        // No shared tokens at all: no overlap to split on.
        assert_eq!(bisect(b"abc", b"xyz", None, &mut scratch), None);
        // Expired deadline: give up immediately.
        let past = Instant::now() - std::time::Duration::from_secs(1);
        assert_eq!(bisect(b"cat", b"map", Some(past), &mut scratch), None);
    }

    #[test]
    fn snake_walks() {
        // Forward: extends the match run from (x, y), stops at the mismatch.
        assert_eq!(snake_fwd(b"abcX", b"abcY", 0, 0), (3, 3));
        assert_eq!(snake_fwd(b"abc", b"abc", 1, 1), (3, 3));
        // Offsets are per-side: old[2..] vs new[0..].
        assert_eq!(snake_fwd(b"xxab", b"abyy", 2, 0), (4, 2));
        // Start at/past an end: nothing to extend.
        assert_eq!(snake_fwd(b"ab", b"ab", 2, 2), (2, 2));
        assert_eq!(snake_fwd(b"ab", b"ab", 3, 3), (3, 3));

        // Reverse: x/y count matched tokens from the tails
        // (old[len-1-x] vs new[len-1-y]).
        assert_eq!(snake_rev(b"Xabc", b"Yabc", 0, 0), (3, 3));
        assert_eq!(snake_rev(b"abcd", b"cd", 0, 0), (2, 2));
        assert_eq!(snake_rev(b"ab", b"ab", 2, 2), (2, 2));
    }

    #[test]
    fn contains_matches_find_sub() {
        // `contains` must return the exact same first-occurrence index as
        // `find_sub(.., .., 0)` on every shape — the diff output depends on
        // it matching the reference's `indexOf`.
        let cases: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"abc", b""),
            (b"abcdef", b"cd"),        // interior hit
            (b"abcdef", b"abc"),       // prefix hit
            (b"abcdef", b"def"),       // suffix hit
            (b"abcdef", b"xyz"),       // no hit
            (b"abcdef", b"abcdefg"),   // needle longer than hay
            (b"XabcY", b"abc"),        // hit after a mismatch at 0
            (b"aaaab", b"aab"),        // repeated first token
            (b"abababab", b"ababab"),  // periodic near-equal: budget-fallback path
            (b"abababab", b"babab"),   // periodic, offset-1 hit
            (b"aaaaaaaa", b"aaaaaaa"), // fully periodic, prefix hit at 0
            (b"aaaaaaab", b"aaaab"),   // periodic tail, no full hit
        ];
        for &(hay, needle) in cases {
            assert_eq!(
                contains(hay, needle),
                find_sub(hay, needle, 0),
                "contains disagrees with find_sub on hay={hay:?} needle={needle:?}"
            );
        }
    }
}
