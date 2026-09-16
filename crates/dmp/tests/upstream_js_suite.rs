// Faithful port of Google's upstream diff-match-patch JavaScript test suite:
// https://github.com/google/diff-match-patch/blob/master/javascript/tests/diff_match_patch_test.js
// (vendored at crates/dmp/oracle/vendor/diff_match_patch_test.js).
//
// One #[test] per upstream test function, in the same order as the JS file.
// Inputs and expected values are copied verbatim from the JS; failing tests
// are deliberate breakage discoveries, not porting bugs.
//
// Skipped as untypable in Rust (null-input throw tests):
//   - testDiffMain:   dmp.diff_main(null, null)
//   - testMatchMain:  dmp.match_main(null, null, 0)
//   - testPatchMake:  dmp.patch_make(null)

use std::collections::HashMap;

use diff_match_patch::{Diff, Dmp, Patch};

fn d(operation: i32, text: &str) -> Diff {
    Diff::new(operation, text.to_string())
}

fn chars(text: &str) -> Vec<char> {
    text.chars().collect()
}

fn svec(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

// Port of the JS diff_rebuildtexts helper.
fn diff_rebuildtexts(diffs: &[Diff]) -> (String, String) {
    let mut text1 = String::new();
    let mut text2 = String::new();
    for diff in diffs {
        if diff.operation != 1 {
            text1 += &diff.text;
        }
        if diff.operation != -1 {
            text2 += &diff.text;
        }
    }
    (text1, text2)
}

// DIFF TEST FUNCTIONS

#[test]
fn upstream_diff_common_prefix() {
    let mut dmp = Dmp::new();
    // Null case.
    assert_eq!(0, dmp.diff_common_prefix(&chars("abc"), &chars("xyz")));

    // Non-null case.
    assert_eq!(
        4,
        dmp.diff_common_prefix(&chars("1234abcdef"), &chars("1234xyz"))
    );

    // Whole case.
    assert_eq!(4, dmp.diff_common_prefix(&chars("1234"), &chars("1234xyz")));
}

#[test]
fn upstream_diff_common_suffix() {
    let mut dmp = Dmp::new();
    // Null case.
    assert_eq!(0, dmp.diff_common_suffix(&chars("abc"), &chars("xyz")));

    // Non-null case.
    assert_eq!(
        4,
        dmp.diff_common_suffix(&chars("abcdef1234"), &chars("xyz1234"))
    );

    // Whole case.
    assert_eq!(4, dmp.diff_common_suffix(&chars("1234"), &chars("xyz1234")));
}

#[test]
fn upstream_diff_common_overlap() {
    let mut dmp = Dmp::new();
    // Null case.
    assert_eq!(0, dmp.diff_common_overlap(&chars(""), &chars("abcd")));

    // Whole case.
    assert_eq!(3, dmp.diff_common_overlap(&chars("abc"), &chars("abcd")));

    // No overlap.
    assert_eq!(0, dmp.diff_common_overlap(&chars("123456"), &chars("abcd")));

    // Overlap.
    assert_eq!(
        3,
        dmp.diff_common_overlap(&chars("123456xxx"), &chars("xxxabcd"))
    );

    // Unicode.
    // Some overly clever languages (C#) may treat ligatures as equal to their
    // component letters.  E.g. U+FB01 == 'fi'
    assert_eq!(
        0,
        dmp.diff_common_overlap(&chars("fi"), &chars("\u{fb01}i"))
    );
}

#[test]
fn upstream_diff_half_match() {
    let mut dmp = Dmp::new();
    dmp.diff_timeout = Some(1.0);
    // JS returns null for "no half-match"; the crate returns an empty Vec.
    let no_match: Vec<String> = vec![];
    // No match.
    assert_eq!(
        no_match,
        dmp.diff_half_match(&chars("1234567890"), &chars("abcdef"))
    );

    assert_eq!(no_match, dmp.diff_half_match(&chars("12345"), &chars("23")));

    // Single Match.
    assert_eq!(
        svec(&["12", "90", "a", "z", "345678"]),
        dmp.diff_half_match(&chars("1234567890"), &chars("a345678z"))
    );

    assert_eq!(
        svec(&["a", "z", "12", "90", "345678"]),
        dmp.diff_half_match(&chars("a345678z"), &chars("1234567890"))
    );

    assert_eq!(
        svec(&["abc", "z", "1234", "0", "56789"]),
        dmp.diff_half_match(&chars("abc56789z"), &chars("1234567890"))
    );

    assert_eq!(
        svec(&["a", "xyz", "1", "7890", "23456"]),
        dmp.diff_half_match(&chars("a23456xyz"), &chars("1234567890"))
    );

    // Multiple Matches.
    assert_eq!(
        svec(&["12123", "123121", "a", "z", "1234123451234"]),
        dmp.diff_half_match(
            &chars("121231234123451234123121"),
            &chars("a1234123451234z")
        )
    );

    assert_eq!(
        svec(&["", "-=-=-=-=-=", "x", "", "x-=-=-=-=-=-=-="]),
        dmp.diff_half_match(
            &chars("x-=-=-=-=-=-=-=-=-=-=-=-="),
            &chars("xx-=-=-=-=-=-=-=")
        )
    );

    assert_eq!(
        svec(&["-=-=-=-=-=", "", "", "y", "-=-=-=-=-=-=-=y"]),
        dmp.diff_half_match(
            &chars("-=-=-=-=-=-=-=-=-=-=-=-=y"),
            &chars("-=-=-=-=-=-=-=yy")
        )
    );

    // Non-optimal halfmatch.
    // Optimal diff would be -q+x=H-i+e=lloHe+Hu=llo-Hew+y not -qHillo+x=HelloHe-w+Hulloy
    assert_eq!(
        svec(&["qHillo", "w", "x", "Hulloy", "HelloHe"]),
        dmp.diff_half_match(&chars("qHilloHelloHew"), &chars("xHelloHeHulloy"))
    );

    // Optimal no halfmatch.
    dmp.diff_timeout = None; // JS Diff_Timeout = 0 maps to None (no deadline).
    assert_eq!(
        no_match,
        dmp.diff_half_match(&chars("qHilloHelloHew"), &chars("xHelloHeHulloy"))
    );
}

#[test]
fn upstream_diff_lines_to_chars() {
    let mut dmp = Dmp::new();
    // Convert lines down to characters.
    assert_eq!(
        (
            "\x01\x02\x01".to_string(),
            "\x02\x01\x02".to_string(),
            svec(&["", "alpha\n", "beta\n"])
        ),
        dmp.diff_lines_tochars(
            &chars("alpha\nbeta\nalpha\n"),
            &chars("beta\nalpha\nbeta\n")
        )
    );

    assert_eq!(
        (
            "".to_string(),
            "\x01\x02\x03\x03".to_string(),
            svec(&["", "alpha\r\n", "beta\r\n", "\r\n"])
        ),
        dmp.diff_lines_tochars(&chars(""), &chars("alpha\r\nbeta\r\n\r\n\r\n"))
    );

    assert_eq!(
        (
            "\x01".to_string(),
            "\x02".to_string(),
            svec(&["", "a", "b"])
        ),
        dmp.diff_lines_tochars(&chars("a"), &chars("b"))
    );

    // More than 256 to reveal any 8-bit limitations.
    let n = 300;
    let mut line_list: Vec<String> = vec![];
    let mut char_list: Vec<char> = vec![];
    for i in 1..n + 1 {
        line_list.push(i.to_string() + "\n");
        char_list.push(char::from_u32(i).expect("codepoints 1..=300 are valid scalars"));
    }
    assert_eq!(n as usize, line_list.len());
    let lines = line_list.join("");
    let joined_chars: String = char_list.into_iter().collect();
    assert_eq!(n as usize, joined_chars.chars().count());
    line_list.insert(0, "".to_string());
    assert_eq!(
        (joined_chars, "".to_string(), line_list),
        dmp.diff_lines_tochars(&chars(&lines), &vec![])
    );
}

#[test]
fn upstream_diff_chars_to_lines() {
    let mut dmp = Dmp::new();
    // Convert chars up to lines.
    let mut diffs = vec![d(0, "\x01\x02\x01"), d(1, "\x02\x01\x02")];
    dmp.diff_chars_tolines(&mut diffs, &svec(&["", "alpha\n", "beta\n"]));
    assert_eq!(
        vec![d(0, "alpha\nbeta\nalpha\n"), d(1, "beta\nalpha\nbeta\n")],
        diffs
    );

    // More than 256 to reveal any 8-bit limitations.
    let n = 300;
    let mut line_list: Vec<String> = vec![];
    let mut char_list: Vec<char> = vec![];
    for i in 1..n + 1 {
        line_list.push(i.to_string() + "\n");
        char_list.push(char::from_u32(i).expect("codepoints 1..=300 are valid scalars"));
    }
    assert_eq!(n as usize, line_list.len());
    let lines = line_list.join("");
    let joined_chars: String = char_list.into_iter().collect();
    assert_eq!(n as usize, joined_chars.chars().count());
    line_list.insert(0, "".to_string());
    let mut diffs = vec![Diff::new(-1, joined_chars)];
    dmp.diff_chars_tolines(&mut diffs, &line_list);
    assert_eq!(vec![Diff::new(-1, lines)], diffs);

    // More than 65536 to verify any 16-bit limitation.
    // ADAPTED (UTF-16 -> scalar): JS exercises surrogate-pair line hashes; in
    // Rust the packer must instead skip the U+D800..U+DFFF scalar gap. The
    // roundtrip assertion itself is verbatim.
    let mut line_list: Vec<String> = vec![];
    for i in 0..66000 {
        line_list.push(i.to_string() + "\n");
    }
    let text = line_list.join("");
    let results = dmp.diff_lines_tochars(&chars(&text), &vec![]);
    let mut diffs = vec![Diff::new(1, results.0)];
    dmp.diff_chars_tolines(&mut diffs, &results.2);
    assert_eq!(text, diffs[0].text);
}

#[test]
fn upstream_diff_cleanup_merge() {
    let mut dmp = Dmp::new();
    // Cleanup a messy diff.
    // Null case.
    let mut diffs: Vec<Diff> = vec![];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(Vec::<Diff>::new(), diffs);

    // No change case.
    diffs = vec![d(0, "a"), d(-1, "b"), d(1, "c")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(0, "a"), d(-1, "b"), d(1, "c")], diffs);

    // Merge equalities.
    diffs = vec![d(0, "a"), d(0, "b"), d(0, "c")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(0, "abc")], diffs);

    // Merge deletions.
    diffs = vec![d(-1, "a"), d(-1, "b"), d(-1, "c")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(-1, "abc")], diffs);

    // Merge insertions.
    diffs = vec![d(1, "a"), d(1, "b"), d(1, "c")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(1, "abc")], diffs);

    // Merge interweave.
    diffs = vec![
        d(-1, "a"),
        d(1, "b"),
        d(-1, "c"),
        d(1, "d"),
        d(0, "e"),
        d(0, "f"),
    ];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(-1, "ac"), d(1, "bd"), d(0, "ef")], diffs);

    // Prefix and suffix detection.
    diffs = vec![d(-1, "a"), d(1, "abc"), d(-1, "dc")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(0, "a"), d(-1, "d"), d(1, "b"), d(0, "c")], diffs);

    // Prefix and suffix detection with equalities.
    diffs = vec![d(0, "x"), d(-1, "a"), d(1, "abc"), d(-1, "dc"), d(0, "y")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(0, "xa"), d(-1, "d"), d(1, "b"), d(0, "cy")], diffs);

    // Slide edit left.
    diffs = vec![d(0, "a"), d(1, "ba"), d(0, "c")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(1, "ab"), d(0, "ac")], diffs);

    // Slide edit right.
    diffs = vec![d(0, "c"), d(1, "ab"), d(0, "a")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(0, "ca"), d(1, "ba")], diffs);

    // Slide edit left recursive.
    diffs = vec![d(0, "a"), d(-1, "b"), d(0, "c"), d(-1, "ac"), d(0, "x")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(-1, "abc"), d(0, "acx")], diffs);

    // Slide edit right recursive.
    diffs = vec![d(0, "x"), d(-1, "ca"), d(0, "c"), d(-1, "b"), d(0, "a")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(0, "xca"), d(-1, "cba")], diffs);

    // Empty merge.
    diffs = vec![d(-1, "b"), d(1, "ab"), d(0, "c")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(1, "a"), d(0, "bc")], diffs);

    // Empty equality.
    diffs = vec![d(0, ""), d(1, "a"), d(0, "b")];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![d(1, "a"), d(0, "b")], diffs);
}

#[test]
fn upstream_diff_cleanup_semantic_lossless() {
    let mut dmp = Dmp::new();
    // Slide diffs to match logical boundaries.
    // Null case.
    let mut diffs: Vec<Diff> = vec![];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(Vec::<Diff>::new(), diffs);

    // Blank lines.
    diffs = vec![
        d(0, "AAA\r\n\r\nBBB"),
        d(1, "\r\nDDD\r\n\r\nBBB"),
        d(0, "\r\nEEE"),
    ];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(
        vec![
            d(0, "AAA\r\n\r\n"),
            d(1, "BBB\r\nDDD\r\n\r\n"),
            d(0, "BBB\r\nEEE")
        ],
        diffs
    );

    // Line boundaries.
    diffs = vec![d(0, "AAA\r\nBBB"), d(1, " DDD\r\nBBB"), d(0, " EEE")];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(
        vec![d(0, "AAA\r\n"), d(1, "BBB DDD\r\n"), d(0, "BBB EEE")],
        diffs
    );

    // Word boundaries.
    diffs = vec![d(0, "The c"), d(1, "ow and the c"), d(0, "at.")];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(
        vec![d(0, "The "), d(1, "cow and the "), d(0, "cat.")],
        diffs
    );

    // Alphanumeric boundaries.
    diffs = vec![d(0, "The-c"), d(1, "ow-and-the-c"), d(0, "at.")];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(
        vec![d(0, "The-"), d(1, "cow-and-the-"), d(0, "cat.")],
        diffs
    );

    // Hitting the start.
    diffs = vec![d(0, "a"), d(-1, "a"), d(0, "ax")];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(vec![d(-1, "a"), d(0, "aax")], diffs);

    // Hitting the end.
    diffs = vec![d(0, "xa"), d(-1, "a"), d(0, "a")];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(vec![d(0, "xaa"), d(-1, "a")], diffs);

    // Sentence boundaries.
    diffs = vec![d(0, "The xxx. The "), d(1, "zzz. The "), d(0, "yyy.")];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(
        vec![d(0, "The xxx."), d(1, " The zzz."), d(0, " The yyy.")],
        diffs
    );
}

#[test]
fn upstream_diff_cleanup_semantic() {
    let mut dmp = Dmp::new();
    // Cleanup semantically trivial equalities.
    // Null case.
    let mut diffs: Vec<Diff> = vec![];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(Vec::<Diff>::new(), diffs);

    // No elimination #1.
    diffs = vec![d(-1, "ab"), d(1, "cd"), d(0, "12"), d(-1, "e")];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![d(-1, "ab"), d(1, "cd"), d(0, "12"), d(-1, "e")], diffs);

    // No elimination #2.
    diffs = vec![d(-1, "abc"), d(1, "ABC"), d(0, "1234"), d(-1, "wxyz")];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(
        vec![d(-1, "abc"), d(1, "ABC"), d(0, "1234"), d(-1, "wxyz")],
        diffs
    );

    // Simple elimination.
    diffs = vec![d(-1, "a"), d(0, "b"), d(-1, "c")];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![d(-1, "abc"), d(1, "b")], diffs);

    // Backpass elimination.
    diffs = vec![d(-1, "ab"), d(0, "cd"), d(-1, "e"), d(0, "f"), d(1, "g")];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![d(-1, "abcdef"), d(1, "cdfg")], diffs);

    // Multiple eliminations.
    diffs = vec![
        d(1, "1"),
        d(0, "A"),
        d(-1, "B"),
        d(1, "2"),
        d(0, "_"),
        d(1, "1"),
        d(0, "A"),
        d(-1, "B"),
        d(1, "2"),
    ];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![d(-1, "AB_AB"), d(1, "1A2_1A2")], diffs);

    // Word boundaries.
    diffs = vec![d(0, "The c"), d(-1, "ow and the c"), d(0, "at.")];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(
        vec![d(0, "The "), d(-1, "cow and the "), d(0, "cat.")],
        diffs
    );

    // No overlap elimination.
    diffs = vec![d(-1, "abcxx"), d(1, "xxdef")];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![d(-1, "abcxx"), d(1, "xxdef")], diffs);

    // Overlap elimination.
    diffs = vec![d(-1, "abcxxx"), d(1, "xxxdef")];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![d(-1, "abc"), d(0, "xxx"), d(1, "def")], diffs);

    // Reverse overlap elimination.
    diffs = vec![d(-1, "xxxabc"), d(1, "defxxx")];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![d(1, "def"), d(0, "xxx"), d(-1, "abc")], diffs);

    // Two overlap eliminations.
    diffs = vec![
        d(-1, "abcd1212"),
        d(1, "1212efghi"),
        d(0, "----"),
        d(-1, "A3"),
        d(1, "3BC"),
    ];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(
        vec![
            d(-1, "abcd"),
            d(0, "1212"),
            d(1, "efghi"),
            d(0, "----"),
            d(-1, "A"),
            d(0, "3"),
            d(1, "BC")
        ],
        diffs
    );
}

#[test]
fn upstream_diff_cleanup_efficiency() {
    let mut dmp = Dmp::new();
    // Cleanup operationally trivial equalities.
    dmp.edit_cost = 4; // JS default Diff_EditCost = 4; crate default is 0 (documented deviation)
                       // Null case.
    let mut diffs: Vec<Diff> = vec![];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(Vec::<Diff>::new(), diffs);

    // No elimination.
    diffs = vec![
        d(-1, "ab"),
        d(1, "12"),
        d(0, "wxyz"),
        d(-1, "cd"),
        d(1, "34"),
    ];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(
        vec![
            d(-1, "ab"),
            d(1, "12"),
            d(0, "wxyz"),
            d(-1, "cd"),
            d(1, "34")
        ],
        diffs
    );

    // Four-edit elimination.
    diffs = vec![
        d(-1, "ab"),
        d(1, "12"),
        d(0, "xyz"),
        d(-1, "cd"),
        d(1, "34"),
    ];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(vec![d(-1, "abxyzcd"), d(1, "12xyz34")], diffs);

    // Three-edit elimination.
    diffs = vec![d(1, "12"), d(0, "x"), d(-1, "cd"), d(1, "34")];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(vec![d(-1, "xcd"), d(1, "12x34")], diffs);

    // Backpass elimination.
    diffs = vec![
        d(-1, "ab"),
        d(1, "12"),
        d(0, "xy"),
        d(1, "34"),
        d(0, "z"),
        d(-1, "cd"),
        d(1, "56"),
    ];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(vec![d(-1, "abxyzcd"), d(1, "12xy34z56")], diffs);

    // High cost elimination.
    dmp.edit_cost = 5;
    diffs = vec![
        d(-1, "ab"),
        d(1, "12"),
        d(0, "wxyz"),
        d(-1, "cd"),
        d(1, "34"),
    ];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(vec![d(-1, "abwxyzcd"), d(1, "12wxyz34")], diffs);
    // JS restores the shared global dmp (Diff_EditCost = 4); moot with a
    // per-test Dmp, and porting it verbatim trips unused_assignments.
}

// MISSING API: diff_pretty_html not implemented in this crate.
// Upstream testDiffPrettyHtml:
//
// #[test]
// fn upstream_diff_pretty_html() {
//     let mut dmp = Dmp::new();
//     // Pretty print.
//     let mut diffs = vec![d(0, "a\n"), d(-1, "<B>b</B>"), d(1, "c&d")];
//     assert_eq!(
//         "<span>a&para;<br></span><del style=\"background:#ffe6e6;\">&lt;B&gt;b&lt;/B&gt;</del><ins style=\"background:#e6ffe6;\">c&amp;d</ins>",
//         dmp.diff_pretty_html(&mut diffs)
//     );
// }

#[test]
fn upstream_diff_text() {
    let mut dmp = Dmp::new();
    // Compute the source and destination texts.
    let mut diffs = vec![
        d(0, "jump"),
        d(-1, "s"),
        d(1, "ed"),
        d(0, " over "),
        d(-1, "the"),
        d(1, "a"),
        d(0, " lazy"),
    ];
    assert_eq!("jumps over the lazy", dmp.diff_text1(&mut diffs));

    assert_eq!("jumped over a lazy", dmp.diff_text2(&mut diffs));
}

#[test]
fn upstream_diff_delta() {
    let mut dmp = Dmp::new();
    // Convert a diff into delta string.
    let mut diffs = vec![
        d(0, "jump"),
        d(-1, "s"),
        d(1, "ed"),
        d(0, " over "),
        d(-1, "the"),
        d(1, "a"),
        d(0, " lazy"),
        d(1, "old dog"),
    ];
    let mut text1 = dmp.diff_text1(&mut diffs);
    assert_eq!("jumps over the lazy", text1);

    let mut delta = dmp.diff_todelta(&mut diffs);
    assert_eq!("=4\t-1\t+ed\t=6\t-3\t+a\t=5\t+old dog", delta);

    // Convert delta string into a diff.
    assert_eq!(diffs, dmp.diff_from_delta(&text1, &delta));

    // The three "Generates error" cases are ported as the #[should_panic]
    // tests immediately below this function.

    // Test deltas with special characters.
    diffs = vec![
        d(0, "\u{0680} \x00 \t %"),
        d(-1, "\u{0681} \x01 \n ^"),
        d(1, "\u{0682} \x02 \\ |"),
    ];
    text1 = dmp.diff_text1(&mut diffs);
    assert_eq!("\u{0680} \x00 \t %\u{0681} \x01 \n ^", text1);

    delta = dmp.diff_todelta(&mut diffs);
    assert_eq!("=7\t-7\t+%DA%82 %02 %5C %7C", delta);

    // Convert delta string into a diff.
    assert_eq!(diffs, dmp.diff_from_delta(&text1, &delta));

    // Verify pool of unchanged characters.
    diffs = vec![d(1, "A-Z a-z 0-9 - _ . ! ~ * ' ( ) ; / ? : @ & = + $ , # ")];
    let text2 = dmp.diff_text2(&mut diffs);
    assert_eq!(
        "A-Z a-z 0-9 - _ . ! ~ * ' ( ) ; / ? : @ & = + $ , # ",
        text2
    );

    delta = dmp.diff_todelta(&mut diffs);
    assert_eq!(
        "+A-Z a-z 0-9 - _ . ! ~ * ' ( ) ; / ? : @ & = + $ , # ",
        delta
    );

    // Convert delta string into a diff.
    assert_eq!(diffs, dmp.diff_from_delta("", &delta));

    // 160 kb string.
    let mut a = "abcdefghij".to_string();
    for _i in 0..14 {
        let doubled = a.clone();
        a += &doubled;
    }
    diffs = vec![Diff::new(1, a.clone())];
    delta = dmp.diff_todelta(&mut diffs);
    assert_eq!("+".to_string() + &a, delta);

    // Convert delta string into a diff.
    assert_eq!(diffs, dmp.diff_from_delta("", &delta));
}

// JS: "Generates error (19 != 20)." — dmp.diff_fromDelta(text1 + 'x', delta)
#[test]
#[should_panic(expected = "Delta length (19) does not equal source text length (20)")]
fn upstream_diff_delta_error_text_too_long() {
    let mut dmp = Dmp::new();
    let mut diffs = vec![
        d(0, "jump"),
        d(-1, "s"),
        d(1, "ed"),
        d(0, " over "),
        d(-1, "the"),
        d(1, "a"),
        d(0, " lazy"),
        d(1, "old dog"),
    ];
    let text1 = dmp.diff_text1(&mut diffs);
    let delta = dmp.diff_todelta(&mut diffs);
    dmp.diff_from_delta(&(text1 + "x"), &delta);
}

// JS: "Generates error (19 != 18)." — dmp.diff_fromDelta(text1.substring(1), delta)
#[test]
#[should_panic(expected = "Delta length (19) larger than source text length (18)")]
fn upstream_diff_delta_error_text_too_short() {
    let mut dmp = Dmp::new();
    let mut diffs = vec![
        d(0, "jump"),
        d(-1, "s"),
        d(1, "ed"),
        d(0, " over "),
        d(-1, "the"),
        d(1, "a"),
        d(0, " lazy"),
        d(1, "old dog"),
    ];
    let text1 = dmp.diff_text1(&mut diffs);
    let delta = dmp.diff_todelta(&mut diffs);
    dmp.diff_from_delta(&text1[1..], &delta);
}

// JS: "Generates error (%c3%xy invalid Unicode)." — dmp.diff_fromDelta('', '+%c3%xy')
#[test]
#[should_panic(expected = "Illegal escape in diff_from_delta: %c3%xy")]
fn upstream_diff_delta_error_invalid_unicode() {
    let mut dmp = Dmp::new();
    dmp.diff_from_delta("", "+%c3%xy");
}

#[test]
fn upstream_diff_x_index() {
    let mut dmp = Dmp::new();
    // Translate a location in text1 to text2.
    // Translation on equality.
    assert_eq!(
        5,
        dmp.diff_xindex(&vec![d(-1, "a"), d(1, "1234"), d(0, "xyz")], 2)
    );

    // Translation on deletion.
    assert_eq!(
        1,
        dmp.diff_xindex(&vec![d(0, "a"), d(-1, "1234"), d(0, "xyz")], 3)
    );
}

#[test]
fn upstream_diff_levenshtein() {
    let mut dmp = Dmp::new();
    // Levenshtein with trailing equality.
    assert_eq!(
        4,
        dmp.diff_levenshtein(&vec![d(-1, "abc"), d(1, "1234"), d(0, "xyz")])
    );
    // Levenshtein with leading equality.
    assert_eq!(
        4,
        dmp.diff_levenshtein(&vec![d(0, "xyz"), d(-1, "abc"), d(1, "1234")])
    );
    // Levenshtein with middle equality.
    assert_eq!(
        7,
        dmp.diff_levenshtein(&vec![d(-1, "abc"), d(0, "xyz"), d(1, "1234")])
    );
}

#[test]
fn upstream_diff_bisect() {
    let mut dmp = Dmp::new();
    // Normal.
    let a = chars("cat");
    let b = chars("map");
    // Since the resulting diff hasn't been normalized, it would be ok if
    // the insertion and deletion pairs are swapped.
    // If the order changes, tweak this test as required.
    // JS passes deadline = Number.MAX_VALUE; the crate's diff_bisect derives
    // its deadline from diff_timeout, so None is the no-deadline equivalent.
    dmp.diff_timeout = None;
    assert_eq!(
        vec![d(-1, "c"), d(1, "m"), d(0, "a"), d(-1, "t"), d(1, "p")],
        dmp.diff_bisect(&a, &b)
    );

    // Timeout.
    // JS passes deadline = 0 (already expired); Some(0.0) expires immediately.
    dmp.diff_timeout = Some(0.0);
    assert_eq!(vec![d(-1, "cat"), d(1, "map")], dmp.diff_bisect(&a, &b));
}

#[test]
fn upstream_diff_main() {
    let mut dmp = Dmp::new();
    // Perform a trivial diff.
    // Null case.
    assert_eq!(Vec::<Diff>::new(), dmp.diff_main("", "", false));

    // Equality.
    assert_eq!(vec![d(0, "abc")], dmp.diff_main("abc", "abc", false));

    // Simple insertion.
    assert_eq!(
        vec![d(0, "ab"), d(1, "123"), d(0, "c")],
        dmp.diff_main("abc", "ab123c", false)
    );

    // Simple deletion.
    assert_eq!(
        vec![d(0, "a"), d(-1, "123"), d(0, "bc")],
        dmp.diff_main("a123bc", "abc", false)
    );

    // Two insertions.
    assert_eq!(
        vec![d(0, "a"), d(1, "123"), d(0, "b"), d(1, "456"), d(0, "c")],
        dmp.diff_main("abc", "a123b456c", false)
    );

    // Two deletions.
    assert_eq!(
        vec![d(0, "a"), d(-1, "123"), d(0, "b"), d(-1, "456"), d(0, "c")],
        dmp.diff_main("a123b456c", "abc", false)
    );

    // Perform a real diff.
    // Switch off the timeout.
    dmp.diff_timeout = None; // JS Diff_Timeout = 0 maps to None (no deadline).
                             // Simple cases.
    assert_eq!(vec![d(-1, "a"), d(1, "b")], dmp.diff_main("a", "b", false));

    assert_eq!(
        vec![
            d(-1, "Apple"),
            d(1, "Banana"),
            d(0, "s are a"),
            d(1, "lso"),
            d(0, " fruit.")
        ],
        dmp.diff_main("Apples are a fruit.", "Bananas are also fruit.", false)
    );

    assert_eq!(
        vec![
            d(-1, "a"),
            d(1, "\u{0680}"),
            d(0, "x"),
            d(-1, "\t"),
            d(1, "\0")
        ],
        dmp.diff_main("ax\t", "\u{0680}x\0", false)
    );

    // Overlaps.
    assert_eq!(
        vec![
            d(-1, "1"),
            d(0, "a"),
            d(-1, "y"),
            d(0, "b"),
            d(-1, "2"),
            d(1, "xab")
        ],
        dmp.diff_main("1ayb2", "abxab", false)
    );

    assert_eq!(
        vec![d(1, "xaxcx"), d(0, "abc"), d(-1, "y")],
        dmp.diff_main("abcy", "xaxcxabc", false)
    );

    assert_eq!(
        vec![
            d(-1, "ABCD"),
            d(0, "a"),
            d(-1, "="),
            d(1, "-"),
            d(0, "bcd"),
            d(-1, "="),
            d(1, "-"),
            d(0, "efghijklmnopqrs"),
            d(-1, "EFGHIJKLMNOefg")
        ],
        dmp.diff_main(
            "ABCDa=bcd=efghijklmnopqrsEFGHIJKLMNOefg",
            "a-bcd-efghijklmnopqrs",
            false
        )
    );

    // Large equality.
    assert_eq!(
        vec![
            d(1, " "),
            d(0, "a"),
            d(1, "nd"),
            d(0, " [[Pennsylvania]]"),
            d(-1, " and [[New")
        ],
        dmp.diff_main(
            "a [[Pennsylvania]] and [[New",
            " and [[Pennsylvania]]",
            false
        )
    );

    // Timeout.
    // The JS wall-clock assertions (elapsed >= Diff_Timeout and elapsed <
    // 2 * Diff_Timeout) are non-deterministic and are not ported; the setup
    // and the timed diff_main call are kept verbatim.
    dmp.diff_timeout = Some(0.1); // 100ms
    let mut a = "`Twas brillig, and the slithy toves\nDid gyre and gimble in the wabe:\nAll mimsy were the borogoves,\nAnd the mome raths outgrabe.\n".to_string();
    let mut b = "I am the very model of a modern major general,\nI've information vegetable, animal, and mineral,\nI know the kings of England, and I quote the fights historical,\nFrom Marathon to Waterloo, in order categorical.\n".to_string();
    // Increase the text lengths by 1024 times to ensure a timeout.
    for _i in 0..10 {
        let doubled_a = a.clone();
        a += &doubled_a;
        let doubled_b = b.clone();
        b += &doubled_b;
    }
    dmp.diff_main(&a, &b, true);
    dmp.diff_timeout = None; // JS Diff_Timeout = 0 maps to None (no deadline).

    // Test the linemode speedup.
    // Must be long to pass the 100 char cutoff.
    // Simple line-mode.
    let a = "1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n";
    let b = "abcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\n";
    assert_eq!(dmp.diff_main(a, b, false), dmp.diff_main(a, b, true));

    // Single line-mode.
    let a = "1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890";
    let b = "abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghij";
    assert_eq!(dmp.diff_main(a, b, false), dmp.diff_main(a, b, true));

    // Overlap line-mode.
    let a = "1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n";
    let b = "abcdefghij\n1234567890\n1234567890\n1234567890\nabcdefghij\n1234567890\n1234567890\n1234567890\nabcdefghij\n1234567890\n1234567890\n1234567890\nabcdefghij\n";
    let texts_linemode = diff_rebuildtexts(&dmp.diff_main(a, b, true));
    let texts_textmode = diff_rebuildtexts(&dmp.diff_main(a, b, false));
    assert_eq!(texts_textmode, texts_linemode);

    // Test null inputs: skipped, untypable in Rust (see file header).
}

// MATCH TEST FUNCTIONS

#[test]
fn upstream_match_alphabet() {
    let mut dmp = Dmp::new();
    // Initialise the bitmasks for Bitap.
    // Unique.
    let mut expected: HashMap<char, i32> = HashMap::new();
    expected.insert('a', 4);
    expected.insert('b', 2);
    expected.insert('c', 1);
    assert_eq!(expected, dmp.match_alphabet(&chars("abc")));

    // Duplicates.
    let mut expected: HashMap<char, i32> = HashMap::new();
    expected.insert('a', 37);
    expected.insert('b', 18);
    expected.insert('c', 8);
    assert_eq!(expected, dmp.match_alphabet(&chars("abcaba")));
}

#[test]
fn upstream_match_bitap() {
    let mut dmp = Dmp::new();
    // Bitap algorithm.
    dmp.match_distance = 100;
    dmp.match_threshold = 0.5;
    // Exact matches.
    assert_eq!(5, dmp.match_bitap(&chars("abcdefghijk"), &chars("fgh"), 5));

    assert_eq!(5, dmp.match_bitap(&chars("abcdefghijk"), &chars("fgh"), 0));

    // Fuzzy matches.
    assert_eq!(
        4,
        dmp.match_bitap(&chars("abcdefghijk"), &chars("efxhi"), 0)
    );

    assert_eq!(
        2,
        dmp.match_bitap(&chars("abcdefghijk"), &chars("cdefxyhijk"), 5)
    );

    assert_eq!(-1, dmp.match_bitap(&chars("abcdefghijk"), &chars("bxy"), 1));

    // Overflow.
    assert_eq!(
        2,
        dmp.match_bitap(&chars("123456789xx0"), &chars("3456789x0"), 2)
    );

    // Threshold test.
    dmp.match_threshold = 0.4;
    assert_eq!(
        4,
        dmp.match_bitap(&chars("abcdefghijk"), &chars("efxyhi"), 1)
    );

    dmp.match_threshold = 0.3;
    assert_eq!(
        -1,
        dmp.match_bitap(&chars("abcdefghijk"), &chars("efxyhi"), 1)
    );

    dmp.match_threshold = 0.0;
    assert_eq!(
        1,
        dmp.match_bitap(&chars("abcdefghijk"), &chars("bcdef"), 1)
    );
    dmp.match_threshold = 0.5;

    // Multiple select.
    assert_eq!(
        0,
        dmp.match_bitap(&chars("abcdexyzabcde"), &chars("abccde"), 3)
    );

    assert_eq!(
        8,
        dmp.match_bitap(&chars("abcdexyzabcde"), &chars("abccde"), 5)
    );

    // Distance test.
    dmp.match_distance = 10; // Strict location.
    assert_eq!(
        -1,
        dmp.match_bitap(&chars("abcdefghijklmnopqrstuvwxyz"), &chars("abcdefg"), 24)
    );

    assert_eq!(
        0,
        dmp.match_bitap(&chars("abcdefghijklmnopqrstuvwxyz"), &chars("abcdxxefg"), 1)
    );

    dmp.match_distance = 1000; // Loose location.
    assert_eq!(
        0,
        dmp.match_bitap(&chars("abcdefghijklmnopqrstuvwxyz"), &chars("abcdefg"), 24)
    );
}

#[test]
fn upstream_match_main() {
    let mut dmp = Dmp::new();
    // Full match.
    // Shortcut matches.
    assert_eq!(0, dmp.match_main("abcdef", "abcdef", 1000));

    assert_eq!(-1, dmp.match_main("", "abcdef", 1));

    assert_eq!(3, dmp.match_main("abcdef", "", 3));

    assert_eq!(3, dmp.match_main("abcdef", "de", 3));

    // Beyond end match.
    assert_eq!(3, dmp.match_main("abcdef", "defy", 4));

    // Oversized pattern.
    assert_eq!(0, dmp.match_main("abcdef", "abcdefy", 0));

    // Complex match.
    assert_eq!(
        4,
        dmp.match_main(
            "I am the very model of a modern major general.",
            " that berry ",
            5
        )
    );

    // Test null inputs: skipped, untypable in Rust (see file header).
}

// PATCH TEST FUNCTIONS

#[test]
fn upstream_patch_obj() {
    // Patch Object.
    let mut patch = Patch::new(vec![], 0, 0, 0, 0);
    patch.start1 = 20;
    patch.start2 = 21;
    patch.length1 = 18;
    patch.length2 = 17;
    patch.diffs = vec![
        d(0, "jump"),
        d(-1, "s"),
        d(1, "ed"),
        d(0, " over "),
        d(-1, "the"),
        d(1, "a"),
        d(0, "\nlaz"),
    ];
    assert_eq!(
        "@@ -21,18 +22,17 @@\n jump\n-s\n+ed\n  over \n-the\n+a\n %0Alaz\n",
        patch.to_string()
    );
}

#[test]
fn upstream_patch_from_text() {
    let mut dmp = Dmp::new();
    // JS passes the hoisted-but-unassigned `strp` (undefined, falsy), which
    // patch_fromText treats as empty input; the Rust equivalent is "".
    assert_eq!(Vec::<Patch>::new(), dmp.patch_from_text("".to_string()));

    let strp = "@@ -21,18 +22,17 @@\n jump\n-s\n+ed\n  over \n-the\n+a\n %0Alaz\n";
    assert_eq!(strp, dmp.patch_from_text(strp.to_string())[0].to_string());

    assert_eq!(
        "@@ -1 +1 @@\n-a\n+b\n",
        dmp.patch_from_text("@@ -1 +1 @@\n-a\n+b\n".to_string())[0].to_string()
    );

    assert_eq!(
        "@@ -1,3 +0,0 @@\n-abc\n",
        dmp.patch_from_text("@@ -1,3 +0,0 @@\n-abc\n".to_string())[0].to_string()
    );

    assert_eq!(
        "@@ -0,0 +1,3 @@\n+abc\n",
        dmp.patch_from_text("@@ -0,0 +1,3 @@\n+abc\n".to_string())[0].to_string()
    );

    // The "Generates error" case is ported as the #[should_panic] test below.
}

// JS: "Generates error." — dmp.patch_fromText('Bad\nPatch\n')
#[test]
#[should_panic(expected = "Invalid patch string: Bad")]
fn upstream_patch_from_text_error_bad_patch() {
    let mut dmp = Dmp::new();
    dmp.patch_from_text("Bad\nPatch\n".to_string());
}

#[test]
fn upstream_patch_to_text() {
    let mut dmp = Dmp::new();
    let strp = "@@ -21,18 +22,17 @@\n jump\n-s\n+ed\n  over \n-the\n+a\n  laz\n";
    let mut patches = dmp.patch_from_text(strp.to_string());
    assert_eq!(strp, dmp.patch_to_text(&mut patches));

    let strp = "@@ -1,9 +1,9 @@\n-f\n+F\n oo+fooba\n@@ -7,9 +7,9 @@\n obar\n-,\n+.\n  tes\n";
    let mut patches = dmp.patch_from_text(strp.to_string());
    assert_eq!(strp, dmp.patch_to_text(&mut patches));
}

#[test]
fn upstream_patch_add_context() {
    let mut dmp = Dmp::new();
    dmp.patch_margin = 4;
    let mut p =
        dmp.patch_from_text("@@ -21,4 +21,10 @@\n-jump\n+somersault\n".to_string())[0].clone();
    dmp.patch_add_context(
        &mut p,
        &mut chars("The quick brown fox jumps over the lazy dog."),
    );
    assert_eq!(
        "@@ -17,12 +17,18 @@\n fox \n-jump\n+somersault\n s ov\n",
        p.to_string()
    );

    // Same, but not enough trailing context.
    let mut p =
        dmp.patch_from_text("@@ -21,4 +21,10 @@\n-jump\n+somersault\n".to_string())[0].clone();
    dmp.patch_add_context(&mut p, &mut chars("The quick brown fox jumps."));
    assert_eq!(
        "@@ -17,10 +17,16 @@\n fox \n-jump\n+somersault\n s.\n",
        p.to_string()
    );

    // Same, but not enough leading context.
    let mut p = dmp.patch_from_text("@@ -3 +3,2 @@\n-e\n+at\n".to_string())[0].clone();
    dmp.patch_add_context(&mut p, &mut chars("The quick brown fox jumps."));
    assert_eq!("@@ -1,7 +1,8 @@\n Th\n-e\n+at\n  qui\n", p.to_string());

    // Same, but with ambiguity.
    let mut p = dmp.patch_from_text("@@ -3 +3,2 @@\n-e\n+at\n".to_string())[0].clone();
    dmp.patch_add_context(
        &mut p,
        &mut chars("The quick brown fox jumps.  The quick brown fox crashes."),
    );
    assert_eq!(
        "@@ -1,27 +1,28 @@\n Th\n-e\n+at\n  quick brown fox jumps. \n",
        p.to_string()
    );
}

#[test]
fn upstream_patch_make() {
    let mut dmp = Dmp::new();
    // patch_make runs diff_cleanup_efficiency internally, so the JS
    // constructor default matters here.
    dmp.edit_cost = 4; // JS default Diff_EditCost = 4; crate default is 0 (documented deviation)
                       // Null case.
    let mut patches = dmp.patch_make1("", "");
    assert_eq!("", dmp.patch_to_text(&mut patches));

    let text1 = "The quick brown fox jumps over the lazy dog.";
    let text2 = "That quick brown fox jumped over a lazy dog.";
    // Text2+Text1 inputs.
    let expected_patch = "@@ -1,8 +1,7 @@\n Th\n-at\n+e\n  qui\n@@ -21,17 +21,18 @@\n jump\n-ed\n+s\n  over \n-a\n+the\n  laz\n";
    // The second patch must be "-21,17 +21,18", not "-22,17 +21,18" due to rolling context.
    let mut patches = dmp.patch_make1(text2, text1);
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

    // Text1+Text2 inputs.
    let expected_patch = "@@ -1,11 +1,12 @@\n Th\n-e\n+at\n  quick b\n@@ -22,18 +22,17 @@\n jump\n-s\n+ed\n  over \n-the\n+a\n  laz\n";
    let mut patches = dmp.patch_make1(text1, text2);
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

    // Diff input.
    let mut diffs = dmp.diff_main(text1, text2, false);
    let mut patches = dmp.patch_make2(&mut diffs);
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

    // Text1+Diff inputs.
    let mut patches = dmp.patch_make4(text1, &mut diffs);
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

    // Text1+Text2+Diff inputs (deprecated).
    let mut patches = dmp.patch_make3(text1, text2, &mut diffs);
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

    // Character encoding.
    let mut patches = dmp.patch_make1("`1234567890-=[]\\;',./", "~!@#$%^&*()_+{}|:\"<>?");
    assert_eq!(
        "@@ -1,21 +1,21 @@\n-%601234567890-=%5B%5D%5C;',./\n+~!@#$%25%5E&*()_+%7B%7D%7C:%22%3C%3E?\n",
        dmp.patch_to_text(&mut patches)
    );

    // Character decoding.
    let diffs = vec![
        d(-1, "`1234567890-=[]\\;',./"),
        d(1, "~!@#$%^&*()_+{}|:\"<>?"),
    ];
    assert_eq!(
        diffs,
        dmp.patch_from_text(
            "@@ -1,21 +1,21 @@\n-%601234567890-=%5B%5D%5C;',./\n+~!@#$%25%5E&*()_+%7B%7D%7C:%22%3C%3E?\n"
                .to_string()
        )[0]
        .diffs
    );

    // Long string with repeats.
    let mut text1 = String::new();
    for _x in 0..100 {
        text1 += "abcdef";
    }
    let text2 = text1.clone() + "123";
    let expected_patch = "@@ -573,28 +573,31 @@\n cdefabcdefabcdefabcdefabcdef\n+123\n";
    let mut patches = dmp.patch_make1(&text1, &text2);
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

    // Test null inputs: skipped, untypable in Rust (see file header).
}

#[test]
fn upstream_patch_split_max() {
    // Assumes that dmp.Match_MaxBits is 32. (Crate default is 32.)
    let mut dmp = Dmp::new();
    dmp.edit_cost = 4; // JS default Diff_EditCost = 4; crate default is 0 (documented deviation)
    let mut patches = dmp.patch_make1(
        "abcdefghijklmnopqrstuvwxyz01234567890",
        "XabXcdXefXghXijXklXmnXopXqrXstXuvXwxXyzX01X23X45X67X89X0",
    );
    dmp.patch_splitmax(&mut patches);
    assert_eq!(
        "@@ -1,32 +1,46 @@\n+X\n ab\n+X\n cd\n+X\n ef\n+X\n gh\n+X\n ij\n+X\n kl\n+X\n mn\n+X\n op\n+X\n qr\n+X\n st\n+X\n uv\n+X\n wx\n+X\n yz\n+X\n 012345\n@@ -25,13 +39,18 @@\n zX01\n+X\n 23\n+X\n 45\n+X\n 67\n+X\n 89\n+X\n 0\n",
        dmp.patch_to_text(&mut patches)
    );

    let mut patches = dmp.patch_make1(
        "abcdef1234567890123456789012345678901234567890123456789012345678901234567890uvwxyz",
        "abcdefuvwxyz",
    );
    let old_to_text = dmp.patch_to_text(&mut patches);
    dmp.patch_splitmax(&mut patches);
    assert_eq!(old_to_text, dmp.patch_to_text(&mut patches));

    let mut patches = dmp.patch_make1(
        "1234567890123456789012345678901234567890123456789012345678901234567890",
        "abc",
    );
    dmp.patch_splitmax(&mut patches);
    assert_eq!(
        "@@ -1,32 +1,4 @@\n-1234567890123456789012345678\n 9012\n@@ -29,32 +1,4 @@\n-9012345678901234567890123456\n 7890\n@@ -57,14 +1,3 @@\n-78901234567890\n+abc\n",
        dmp.patch_to_text(&mut patches)
    );

    let mut patches = dmp.patch_make1(
        "abcdefghij , h : 0 , t : 1 abcdefghij , h : 0 , t : 1 abcdefghij , h : 0 , t : 1",
        "abcdefghij , h : 1 , t : 1 abcdefghij , h : 1 , t : 1 abcdefghij , h : 0 , t : 1",
    );
    dmp.patch_splitmax(&mut patches);
    assert_eq!(
        "@@ -2,32 +2,32 @@\n bcdefghij , h : \n-0\n+1\n  , t : 1 abcdef\n@@ -29,32 +29,32 @@\n bcdefghij , h : \n-0\n+1\n  , t : 1 abcdef\n",
        dmp.patch_to_text(&mut patches)
    );
}

#[test]
fn upstream_patch_add_padding() {
    let mut dmp = Dmp::new();
    dmp.edit_cost = 4; // JS default Diff_EditCost = 4; crate default is 0 (documented deviation)
                       // Both edges full.
    let mut patches = dmp.patch_make1("", "test");
    assert_eq!("@@ -0,0 +1,4 @@\n+test\n", dmp.patch_to_text(&mut patches));
    dmp.patch_add_padding(&mut patches);
    assert_eq!(
        "@@ -1,8 +1,12 @@\n %01%02%03%04\n+test\n %01%02%03%04\n",
        dmp.patch_to_text(&mut patches)
    );

    // Both edges partial.
    let mut patches = dmp.patch_make1("XY", "XtestY");
    assert_eq!(
        "@@ -1,2 +1,6 @@\n X\n+test\n Y\n",
        dmp.patch_to_text(&mut patches)
    );
    dmp.patch_add_padding(&mut patches);
    assert_eq!(
        "@@ -2,8 +2,12 @@\n %02%03%04X\n+test\n Y%01%02%03\n",
        dmp.patch_to_text(&mut patches)
    );

    // Both edges none.
    let mut patches = dmp.patch_make1("XXXXYYYY", "XXXXtestYYYY");
    assert_eq!(
        "@@ -1,8 +1,12 @@\n XXXX\n+test\n YYYY\n",
        dmp.patch_to_text(&mut patches)
    );
    dmp.patch_add_padding(&mut patches);
    assert_eq!(
        "@@ -5,8 +5,12 @@\n XXXX\n+test\n YYYY\n",
        dmp.patch_to_text(&mut patches)
    );
}

#[test]
fn upstream_patch_apply() {
    let mut dmp = Dmp::new();
    dmp.edit_cost = 4; // JS default Diff_EditCost = 4; crate default is 0 (documented deviation)
    dmp.match_distance = 1000;
    dmp.match_threshold = 0.5;
    dmp.patch_delete_threshold = 0.5;
    // Null case.
    let mut patches = dmp.patch_make1("", "");
    let results = dmp.patch_apply(&mut patches, "Hello world.");
    assert_eq!((chars("Hello world."), vec![]), results);

    // Exact match.
    let mut patches = dmp.patch_make1(
        "The quick brown fox jumps over the lazy dog.",
        "That quick brown fox jumped over a lazy dog.",
    );
    let results = dmp.patch_apply(&mut patches, "The quick brown fox jumps over the lazy dog.");
    assert_eq!(
        (
            chars("That quick brown fox jumped over a lazy dog."),
            vec![true, true]
        ),
        results
    );

    // Partial match.
    let results = dmp.patch_apply(
        &mut patches,
        "The quick red rabbit jumps over the tired tiger.",
    );
    assert_eq!(
        (
            chars("That quick red rabbit jumped over a tired tiger."),
            vec![true, true]
        ),
        results
    );

    // Failed match.
    let results = dmp.patch_apply(
        &mut patches,
        "I am the very model of a modern major general.",
    );
    assert_eq!(
        (
            chars("I am the very model of a modern major general."),
            vec![false, false]
        ),
        results
    );

    // Big delete, small change.
    let mut patches = dmp.patch_make1(
        "x1234567890123456789012345678901234567890123456789012345678901234567890y",
        "xabcy",
    );
    let results = dmp.patch_apply(
        &mut patches,
        "x123456789012345678901234567890-----++++++++++-----123456789012345678901234567890y",
    );
    assert_eq!((chars("xabcy"), vec![true, true]), results);

    // Big delete, big change 1.
    let mut patches = dmp.patch_make1(
        "x1234567890123456789012345678901234567890123456789012345678901234567890y",
        "xabcy",
    );
    let results = dmp.patch_apply(
        &mut patches,
        "x12345678901234567890---------------++++++++++---------------12345678901234567890y",
    );
    assert_eq!(
        (
            chars("xabc12345678901234567890---------------++++++++++---------------12345678901234567890y"),
            vec![false, true]
        ),
        results
    );

    // Big delete, big change 2.
    dmp.patch_delete_threshold = 0.6;
    let mut patches = dmp.patch_make1(
        "x1234567890123456789012345678901234567890123456789012345678901234567890y",
        "xabcy",
    );
    let results = dmp.patch_apply(
        &mut patches,
        "x12345678901234567890---------------++++++++++---------------12345678901234567890y",
    );
    assert_eq!((chars("xabcy"), vec![true, true]), results);
    dmp.patch_delete_threshold = 0.5;

    // Compensate for failed patch.
    dmp.match_threshold = 0.0;
    dmp.match_distance = 0;
    let mut patches = dmp.patch_make1(
        "abcdefghijklmnopqrstuvwxyz--------------------1234567890",
        "abcXXXXXXXXXXdefghijklmnopqrstuvwxyz--------------------1234567YYYYYYYYYY890",
    );
    let results = dmp.patch_apply(
        &mut patches,
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ--------------------1234567890",
    );
    assert_eq!(
        (
            chars("ABCDEFGHIJKLMNOPQRSTUVWXYZ--------------------1234567YYYYYYYYYY890"),
            vec![false, true]
        ),
        results
    );
    dmp.match_threshold = 0.5;
    dmp.match_distance = 1000;

    // No side effects.
    let mut patches = dmp.patch_make1("", "test");
    let patchstr = dmp.patch_to_text(&mut patches);
    dmp.patch_apply(&mut patches, "");
    assert_eq!(patchstr, dmp.patch_to_text(&mut patches));

    // No side effects with major delete.
    let mut patches = dmp.patch_make1("The quick brown fox jumps over the lazy dog.", "Woof");
    let patchstr = dmp.patch_to_text(&mut patches);
    dmp.patch_apply(&mut patches, "The quick brown fox jumps over the lazy dog.");
    assert_eq!(patchstr, dmp.patch_to_text(&mut patches));

    // Edge exact match.
    let mut patches = dmp.patch_make1("", "test");
    let results = dmp.patch_apply(&mut patches, "");
    assert_eq!((chars("test"), vec![true]), results);

    // Near edge exact match.
    let mut patches = dmp.patch_make1("XY", "XtestY");
    let results = dmp.patch_apply(&mut patches, "XY");
    assert_eq!((chars("XtestY"), vec![true]), results);

    // Edge partial match.
    let mut patches = dmp.patch_make1("y", "y123");
    let results = dmp.patch_apply(&mut patches, "x");
    assert_eq!((chars("x123"), vec![true]), results);
}
