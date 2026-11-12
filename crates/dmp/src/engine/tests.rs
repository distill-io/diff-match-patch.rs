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

/// The search the backward blocks replaced: one scan from the front of
/// the text that kept the final hit.
fn rfind_sub_from_front<T: Eq>(hay: &[T], needle: &[T], until: usize) -> Option<usize> {
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

#[test]
fn rfind_sub_matches_scan_from_front() {
    // Every text of up to 11 bytes over two letters.
    let texts = |len: usize| {
        (0..1usize << len).map(move |bits| {
            (0..len)
                .map(|i| if bits >> i & 1 == 0 { b'a' } else { b'b' })
                .collect::<Vec<u8>>()
        })
    };
    // A hit or a miss is "far" when the search had to leave the first
    // block of starts, which is as long as the needle.
    let (mut near, mut far, mut missed_near, mut missed_far) = (0, 0, 0, 0);
    for hay in (0..=11).flat_map(texts) {
        for needle in (1..=4).flat_map(texts) {
            for until in 0..=hay.len() + 1 {
                let expected = rfind_sub_from_front(&hay, &needle, until);
                assert_eq!(
                    rfind_sub(&hay, &needle, until),
                    expected,
                    "{hay:?} {needle:?} {until}"
                );
                let end = (until + 1).min(hay.len());
                match expected {
                    Some(at) if at + 2 * needle.len() <= end => far += 1,
                    Some(_) => near += 1,
                    None if 2 * needle.len() <= end => missed_far += 1,
                    None => missed_near += 1,
                }
            }
        }
    }
    assert_eq!(
        (near, far, missed_near, missed_far),
        (339182, 199798, 606746, 328834)
    );
}

#[test]
fn occurs_twice_over_chars() {
    let c = |s: &str| s.chars().collect::<Vec<char>>();
    let ot = |hay: &str, needle: &str| occurs_twice(&c(hay), &[], &c(needle));
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
fn occurs_twice_at_every_split() {
    let count = |hay: &[char], needle: &[char]| {
        hay.windows(needle.len())
            .filter(|window| window == &needle)
            .count()
    };
    // Every text of up to 8 chars over a common and a rare letter.
    let texts = |len: usize| {
        (0..1usize << len).map(move |bits| {
            (0..len)
                .map(|i| if bits >> i & 1 == 0 { 'e' } else { 'Q' })
                .collect::<Vec<char>>()
        })
    };
    let (mut twice, mut by_seam) = (0, 0);
    for hay in (0..=8).flat_map(texts) {
        for needle in (1..=3).flat_map(texts) {
            let expected = count(&hay, &needle) >= 2;
            for split in 0..=hay.len() {
                let (head, tail) = hay.split_at(split);
                assert_eq!(
                    occurs_twice(head, tail, &needle),
                    expected,
                    "{head:?} {tail:?} {needle:?}"
                );
                twice += expected as usize;
                by_seam += (expected && count(head, &needle) + count(tail, &needle) < 2) as usize;
            }
        }
    }
    assert_eq!((twice, by_seam), (19302, 2644));
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
    let unlimited = Limit {
        deadline: None,
        budget: None,
    };
    assert_eq!(
        bisect(b"cat", b"map", &unlimited, &mut scratch),
        Split::Middle(2, 2)
    );
    // No shared tokens at all: no overlap to split on.
    assert_eq!(
        bisect(b"abc", b"xyz", &unlimited, &mut scratch),
        Split::None
    );
    // Expired deadline: give up immediately.
    let past = Instant::now() - std::time::Duration::from_secs(1);
    let expired = Limit {
        deadline: Some(past),
        budget: None,
    };
    assert_eq!(bisect(b"cat", b"map", &expired, &mut scratch), Split::None);
}

#[test]
fn only_a_spent_budget_splits_at_the_ends() {
    let old = [b"p".as_slice(), &[b'a'; 30], b"pqrs"].concat();
    let new = [b"q".as_slice(), &[b'a'; 30], b"wxyz"].concat();
    let mut scratch = Vec::new();
    // Rounds 0 and 1 cost 6 of the 8 steps; round 2 walks the 30-token run
    // and stops there, keeping its end as the head.
    let budgeted = Limit {
        deadline: None,
        budget: Some(Budget::new(4)),
    };
    assert_eq!(
        bisect(&old, &new, &budgeted, &mut scratch),
        Split::Ends {
            head: (31, 31),
            tail: (35, 35)
        }
    );
    let unlimited = Limit {
        deadline: None,
        budget: None,
    };
    assert_eq!(
        bisect(&old, &new, &unlimited, &mut scratch),
        Split::Middle(34, 31)
    );
    let far = Instant::now() + std::time::Duration::from_secs(3600);
    let timed = Limit {
        deadline: Some(far),
        budget: None,
    };
    assert_eq!(
        bisect(&old, &new, &timed, &mut scratch),
        Split::Middle(34, 31)
    );
    let past = Instant::now() - std::time::Duration::from_secs(1);
    for budget in [None, Some(Budget::new(4))] {
        let expired = Limit {
            deadline: Some(past),
            budget,
        };
        assert_eq!(bisect(&old, &new, &expired, &mut scratch), Split::None);
    }
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
