use diff_match_patch;
use std::collections::HashMap;
use core::char;

pub fn diff_rebuildtexts( diffs: Vec<diff_match_patch::Diff>) -> Vec<String> {
    let mut text1: String = "".to_string();
    let mut text2: String = "".to_string();
    for x in 0..diffs.len() {
        if diffs[x].operation != 1 {
            text1 += diffs[x].text.as_str();
        }
        if diffs[x].operation != -1 {
            text2 += diffs[x].text.as_str();
        }
    }
    
    vec![text1, text2]
}

#[test]
pub fn test_diff_common_prefix() {
    let mut dmp = diff_match_patch::Dmp::new();
    assert_eq!(0, dmp.diff_common_prefix(&("abc".to_string().chars().collect()), &("xyz".to_string().chars().collect())));

    assert_eq!(4, dmp.diff_common_prefix(&("1234abcdef".to_string().chars().collect()), &("1234xyz".to_string().chars().collect())));

    assert_eq!(4, dmp.diff_common_prefix(&("1234".to_string().chars().collect()), &("1234xyz".to_string().chars().collect())));
}


#[test]
pub fn test_diff_common_suffix() {
    let mut dmp = diff_match_patch::Dmp::new();
    assert_eq!(0, dmp.diff_common_suffix(&("abc".to_string().chars().collect()), &("xyz".to_string().chars().collect())));

    assert_eq!(4, dmp.diff_common_suffix(&("abcdef1234".to_string().chars().collect()), &("xyz1234".to_string().chars().collect())));

    assert_eq!(4, dmp.diff_common_suffix(&("1234".to_string().chars().collect()), &("xyz1234".to_string().chars().collect())));
}


#[test]
pub fn test_diff_common_overlap() {
    let mut dmp = diff_match_patch::Dmp::new();
    assert_eq!(0, dmp.diff_common_overlap(&("".to_string().chars().collect()), &("abcd".to_string().chars().collect())));

    assert_eq!(3, dmp.diff_common_overlap(&("abc".to_string().chars().collect()), &("abcd".to_string().chars().collect())));

    assert_eq!(0, dmp.diff_common_overlap(&("123456".to_string().chars().collect()), &("abcd".to_string().chars().collect())));

    assert_eq!(3, dmp.diff_common_overlap(&("123456xxx".to_string().chars().collect()), &("xxxabcd".to_string().chars().collect())));
}

#[test]
pub fn test_diff_half_match() {
    let mut dmp = diff_match_patch::Dmp::new();
    let temp: Vec<String> = vec![];
    assert_eq!(temp, dmp.diff_half_match(&("1234567890".to_string().chars().collect()), &("abcdef".to_string().chars().collect())));
    assert_eq!(temp, dmp.diff_half_match(&("12345".to_string().chars().collect()), &("23".to_string().chars().collect())));
    assert_eq!(dmp.split_by_char("12,90,a,z,345678", ','), dmp.diff_half_match(&("1234567890".to_string().chars().collect()), &("a345678z".to_string().chars().collect())));
    assert_eq!(dmp.split_by_char("a,z,12,90,345678", ','), dmp.diff_half_match(&("a345678z".to_string().chars().collect()), &("1234567890".to_string().chars().collect())));
    assert_eq!(dmp.split_by_char("abc,z,1234,0,56789", ','), dmp.diff_half_match(&("abc56789z".to_string().chars().collect()), &("1234567890".to_string().chars().collect())));
    assert_eq!(dmp.split_by_char("a,xyz,1,7890,23456", ','), dmp.diff_half_match(&("a23456xyz".to_string().chars().collect()), &("1234567890".to_string().chars().collect())));
    assert_eq!(dmp.split_by_char("12123,123121,a,z,1234123451234", ','), dmp.diff_half_match(&("121231234123451234123121".to_string().chars().collect()), &("a1234123451234z".to_string().chars().collect())));
    assert_eq!(dmp.split_by_char(",-=-=-=-=-=,x,,x-=-=-=-=-=-=-=", ','), dmp.diff_half_match(&("x-=-=-=-=-=-=-=-=-=-=-=-=".to_string().chars().collect()), &("xx-=-=-=-=-=-=-=".to_string().chars().collect())));
    assert_eq!(dmp.split_by_char("-=-=-=-=-=,,,y,-=-=-=-=-=-=-=y", ','), dmp.diff_half_match(&("-=-=-=-=-=-=-=-=-=-=-=-=y".to_string().chars().collect()), &("-=-=-=-=-=-=-=yy".to_string().chars().collect())));
    assert_eq!(dmp.split_by_char("qHillo,w,x,Hulloy,HelloHe", ','), dmp.diff_half_match(&("qHilloHelloHew".to_string().chars().collect()), &("xHelloHeHulloy".to_string().chars().collect())));
}


#[test]
pub fn test_diff_lines_tochars() {
    let mut dmp = diff_match_patch::Dmp::new();
    assert_eq!(("\x01\x02\x01".to_string(), "\x02\x01\x02".to_string(), vec!["".to_string(), "alpha\n".to_string(), "beta\n".to_string()]),
                dmp.diff_lines_tochars(&("alpha\nbeta\nalpha\n".to_string().chars().collect()), &("beta\nalpha\nbeta\n".to_string().chars().collect())));
    assert_eq!(("".to_string(), "\x01\x02\x03\x03".to_string(), vec!["".to_string(), "alpha\r\n".to_string(), "beta\r\n".to_string(), "\r\n".to_string()]), dmp.diff_lines_tochars(&("".to_string().chars().collect()), &("alpha\r\nbeta\r\n\r\n\r\n".to_string().chars().collect())));
    assert_eq!(("\x01".to_string(), "\x02".to_string(), vec!["".to_string(), "a".to_string(), "b".to_string()]), dmp.diff_lines_tochars(&("a".to_string().chars().collect()), &("b".to_string().chars().collect())));
    let n: u32 = 300;
    let mut line_list: Vec<String> = vec![];
    let mut char_list: Vec<char> = vec![];
    for i in 1..n+1{
        line_list.push(i.to_string() + "\n");
        match char::from_u32(i) {
            Some(ch) => {
                char_list.push(ch);
            },
            None => {}
        }
    }
    let chars: String = char_list.into_iter().collect();
    assert_eq!(n as usize, line_list.len());
    let lines = line_list.join("");
    let lines_vec: Vec<char> = lines.chars().collect();
    assert_eq!(n as usize, chars.chars().count());
    line_list.insert(0, "".to_string());
    assert_eq!((chars, "".to_string(), line_list), dmp.diff_lines_tochars(&lines_vec, &vec![]))
}

#[test]
pub fn test_diff_words_tochars() {
    let mut dmp = diff_match_patch::Dmp::new();
    assert_eq!(("\x01\x02\x03\x02\x01".to_string(), "\x03\x02\x01\x02\x03".to_string(), vec!["".to_string(), "alpha".to_string(), " ".to_string(), "beta".to_string()]),
                dmp.diff_words_tochars(&"alpha beta alpha".to_string(), &"beta alpha beta".to_string())
               );               
    assert_eq!(("\x01\x02".to_string(), "\x03\x02\x01".to_string(), vec!["".to_string(), "alpha".to_string(), "\n".to_string(), "beta".to_string()]),
                dmp.diff_words_tochars(&"alpha\n".to_string(), &"beta\nalpha".to_string())
               );
}


#[test]
pub fn test_diff_chars_tolines() {
    let mut dmp = diff_match_patch::Dmp::new();
    let mut diffs = vec![diff_match_patch::Diff::new(0, "\x01\x02\x01".to_string()), diff_match_patch::Diff::new(1, "\x02\x01\x02".to_string())];
    dmp.diff_chars_tolines(&mut diffs, &vec!["".to_string(), "alpha\n".to_string(), "beta\n".to_string()]);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "alpha\nbeta\nalpha\n".to_string()), diff_match_patch::Diff::new(1, "beta\nalpha\nbeta\n".to_string())], diffs);
    let n: u32 = 30;
    let mut line_list: Vec<String> = vec![];
    let mut char_list: Vec<char> = vec![];
    for i in 1..n + 1 {
        line_list.push(i.to_string() + "\n");
        match char::from_u32(i) {
            Some(ch) => {
                char_list.push(ch);
            }
            None => {

            }
        }
    }
    let chars: String = char_list.into_iter().collect();
    assert_eq!(n as usize, line_list.len());
    let lines = line_list.join("");
    assert_eq!(n as usize, chars.chars().count());
    line_list.insert(0, "".to_string());
    let mut diffs = vec![diff_match_patch::Diff::new(-1, chars)];
    dmp.diff_chars_tolines(&mut diffs, &line_list);
    assert_eq!(diffs, vec![diff_match_patch::Diff::new(-1, lines)]);
    // line_list = vec![];
    // for i in 1..1115000 + 1 {
    //     line_list.push(i.to_string() + "\n");
    // }
    // chars = line_list.join("");
    // let mut temp: Vec<char> = chars.chars().collect();
    // let (temp1, temp2, results) = dmp.diff_lines_tochars(&temp, &vec![]);
    // diffs = vec![diff_match_patch::Diff::new(1, results[0].clone())];
    // dmp.diff_chars_tolines(&mut diffs, results[2].clone().chars().collect()));
    // assert_eq!(chars, diffs[0].text);
}



#[test]
pub fn test_diff_cleanup_merge() {
    let mut dmp = diff_match_patch::Dmp::new();
    let mut diffs: Vec<diff_match_patch::Diff> = vec![];
    let temp: Vec<diff_match_patch::Diff> = vec![];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(temp, diffs);

    // No change case.
    diffs = vec![diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(-1, "b".to_string()), diff_match_patch::Diff::new(1, "c".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(-1, "b".to_string()), diff_match_patch::Diff::new(1, "c".to_string())], diffs);

    // Merge equalities.
    diffs = vec![diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(0, "b".to_string()), diff_match_patch::Diff::new(0, "c".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "abc".to_string())], diffs);

    // Merge deletions.
    diffs = vec![diff_match_patch::Diff::new(-1, "a".to_string()), diff_match_patch::Diff::new(-1, "b".to_string()), diff_match_patch::Diff::new(-1, "c".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "abc".to_string())], diffs);

    // Merge insertions.
    diffs = vec![diff_match_patch::Diff::new(1, "a".to_string()), diff_match_patch::Diff::new(1, "b".to_string()), diff_match_patch::Diff::new(1, "c".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(1, "abc".to_string())], diffs);

    // Merge interweave.
    diffs = vec![diff_match_patch::Diff::new(-1, "a".to_string()), diff_match_patch::Diff::new(1, "b".to_string()), diff_match_patch::Diff::new(-1, "c".to_string()), diff_match_patch::Diff::new(1, "d".to_string()), diff_match_patch::Diff::new(0, "e".to_string()), diff_match_patch::Diff::new(0, "f".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "ac".to_string()), diff_match_patch::Diff::new(1, "bd".to_string()), diff_match_patch::Diff::new(0, "ef".to_string())], diffs);


    // Prefix and suffix detection.
    diffs = vec![diff_match_patch::Diff::new(-1, "a".to_string()), diff_match_patch::Diff::new(1, "abc".to_string()), diff_match_patch::Diff::new(-1, "dc".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(-1, "d".to_string()), diff_match_patch::Diff::new(1, "b".to_string()), diff_match_patch::Diff::new(0, "c".to_string())], diffs);


    // Prefix and suffix detection with equalities.
    diffs = vec![diff_match_patch::Diff::new(0, "x".to_string()), diff_match_patch::Diff::new(-1, "a".to_string()), diff_match_patch::Diff::new(1, "abc".to_string()), diff_match_patch::Diff::new(-1, "dc".to_string()), diff_match_patch::Diff::new(0, "y".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "xa".to_string()), diff_match_patch::Diff::new(-1, "d".to_string()), diff_match_patch::Diff::new(1, "b".to_string()), diff_match_patch::Diff::new(0, "cy".to_string())], diffs);

    // Slide edit left.
    diffs = vec![diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(1, "ba".to_string()), diff_match_patch::Diff::new(0, "c".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(1, "ab".to_string()), diff_match_patch::Diff::new(0, "ac".to_string())], diffs);

    // Slide edit right.
    diffs = vec![diff_match_patch::Diff::new(0, "c".to_string()), diff_match_patch::Diff::new(1, "ab".to_string()), diff_match_patch::Diff::new(0, "a".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "ca".to_string()), diff_match_patch::Diff::new(1, "ba".to_string())], diffs);

    // # Slide edit left recursive.
    diffs = vec![diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(-1, "b".to_string()), diff_match_patch::Diff::new(0, "c".to_string()), diff_match_patch::Diff::new(-1, "ac".to_string()), diff_match_patch::Diff::new(0, "x".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "abc".to_string()), diff_match_patch::Diff::new(0, "acx".to_string())], diffs);

    // # Slide edit right recursive.
    diffs = vec![diff_match_patch::Diff::new(0, "x".to_string()), diff_match_patch::Diff::new(-1, "ca".to_string()), diff_match_patch::Diff::new(0, "c".to_string()), diff_match_patch::Diff::new(-1, "b".to_string()), diff_match_patch::Diff::new(0, "a".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "xca".to_string()), diff_match_patch::Diff::new(-1, "cba".to_string())], diffs);

    // # Empty merge.
    diffs = vec![diff_match_patch::Diff::new(-1, "b".to_string()), diff_match_patch::Diff::new(1, "ab".to_string()), diff_match_patch::Diff::new(0, "c".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(1, "a".to_string()), diff_match_patch::Diff::new(0, "bc".to_string())], diffs);

    // # Empty equality.
    diffs = vec![diff_match_patch::Diff::new(0, "".to_string()), diff_match_patch::Diff::new(1, "a".to_string()), diff_match_patch::Diff::new(0, "b".to_string())];
    dmp.diff_cleanup_merge(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(1, "a".to_string()), diff_match_patch::Diff::new(0, "b".to_string())], diffs);
}


#[test]
pub fn test_diff_cleanup_semantic_lossless() {
    // Slide diffs to match logical boundaries.
    // Null case.
    let mut dmp = diff_match_patch::Dmp::new();
    let mut diffs: Vec<diff_match_patch::Diff> = vec![];
    let temp: Vec<diff_match_patch::Diff> = vec![];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(temp, diffs);

    // Blank lines.
    diffs = vec![diff_match_patch::Diff::new(0, "AAA\r\n\r\nBBB".to_string()), diff_match_patch::Diff::new(1, "\r\nDDD\r\n\r\nBBB".to_string()), diff_match_patch::Diff::new(0, "\r\nEEE".to_string())];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "AAA\r\n\r\n".to_string()), diff_match_patch::Diff::new(1, "BBB\r\nDDD\r\n\r\n".to_string()), diff_match_patch::Diff::new(0, "BBB\r\nEEE".to_string())], diffs);

    // # Line boundaries.
    diffs = vec![diff_match_patch::Diff::new(0, "AAA\r\nBBB".to_string()), diff_match_patch::Diff::new(1, " DDD\r\nBBB".to_string()), diff_match_patch::Diff::new(0, " EEE".to_string())];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "AAA\r\n".to_string()), diff_match_patch::Diff::new(1, "BBB DDD\r\n".to_string()), diff_match_patch::Diff::new(0, "BBB EEE".to_string())], diffs);

    // # Word boundaries.
    diffs = vec![diff_match_patch::Diff::new(0, "The c".to_string()), diff_match_patch::Diff::new(1, "ow and the c".to_string()), diff_match_patch::Diff::new(0, "at.".to_string())];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "The ".to_string()), diff_match_patch::Diff::new(1, "cow and the ".to_string()), diff_match_patch::Diff::new(0, "cat.".to_string())], diffs);

    // # Alphanumeric boundaries.
    diffs = vec![diff_match_patch::Diff::new(0, "The-c".to_string()), diff_match_patch::Diff::new(1, "ow-and-the-c".to_string()), diff_match_patch::Diff::new(0, "at.".to_string())];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "The-".to_string()), diff_match_patch::Diff::new(1, "cow-and-the-".to_string()), diff_match_patch::Diff::new(0, "cat.".to_string())], diffs);

    // # Hitting the start.
    diffs = vec![diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(-1, "a".to_string()), diff_match_patch::Diff::new(0, "ax".to_string())];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "a".to_string()), diff_match_patch::Diff::new(0, "aax".to_string())], diffs);

    // # Hitting the end.
    diffs = vec![diff_match_patch::Diff::new(0, "xa".to_string()), diff_match_patch::Diff::new(-1, "a".to_string()), diff_match_patch::Diff::new(0, "a".to_string())];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "xaa".to_string()), diff_match_patch::Diff::new(-1, "a".to_string())], diffs);

    // # Sentence boundaries.
    diffs = vec![diff_match_patch::Diff::new(0, "The xxx. The ".to_string()), diff_match_patch::Diff::new(1, "zzz. The ".to_string()), diff_match_patch::Diff::new(0, "yyy.".to_string())];
    dmp.diff_cleanup_semantic_lossless(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "The xxx.".to_string()), diff_match_patch::Diff::new(1, " The zzz.".to_string()), diff_match_patch::Diff::new(0, " The yyy.".to_string())], diffs);

}


#[test]
pub fn test_diff_cleanup_semantic() {
    let mut dmp = diff_match_patch::Dmp::new();

    //  Null case.
    let mut diffs: Vec<diff_match_patch::Diff> = vec![];
    let temp: Vec<diff_match_patch::Diff> = vec![];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(diffs, temp);

    // No elimination #1.
    diffs = vec![diff_match_patch::Diff::new(-1, "ab".to_string()), diff_match_patch::Diff::new(1, "cd".to_string()), diff_match_patch::Diff::new(0, "c12".to_string()), diff_match_patch::Diff::new(-1, "e".to_string())];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "ab".to_string()), diff_match_patch::Diff::new(1, "cd".to_string()), diff_match_patch::Diff::new(0, "c12".to_string()), diff_match_patch::Diff::new(-1, "e".to_string())], diffs);

    // No elimination #2.
    diffs = vec![diff_match_patch::Diff::new(-1, "abc".to_string()), diff_match_patch::Diff::new(1, "ABC".to_string()), diff_match_patch::Diff::new(0, "1234".to_string()), diff_match_patch::Diff::new(-1, "wxyz".to_string())];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "abc".to_string()), diff_match_patch::Diff::new(1, "ABC".to_string()), diff_match_patch::Diff::new(0, "1234".to_string()), diff_match_patch::Diff::new(-1, "wxyz".to_string())], diffs);

    // Simple elimination.
    diffs = vec![diff_match_patch::Diff::new(-1, "a".to_string()), diff_match_patch::Diff::new(0, "b".to_string()), diff_match_patch::Diff::new(-1, "c".to_string())];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "abc".to_string()), diff_match_patch::Diff::new(1, "b".to_string())], diffs);

    // Backpass elimination.
    diffs = vec![diff_match_patch::Diff::new(-1, "ab".to_string()), diff_match_patch::Diff::new(0, "cd".to_string()), diff_match_patch::Diff::new(-1, "e".to_string()), diff_match_patch::Diff::new(0, "f".to_string()), diff_match_patch::Diff::new(1, "g".to_string())];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "abcdef".to_string()), diff_match_patch::Diff::new(1, "cdfg".to_string())], diffs);

    // Multiple eliminations.
    diffs = vec![diff_match_patch::Diff::new(1, "1".to_string()), diff_match_patch::Diff::new(0, "A".to_string()), diff_match_patch::Diff::new(-1, "B".to_string()), diff_match_patch::Diff::new(1, "2".to_string()), diff_match_patch::Diff::new(0, "_".to_string()),  diff_match_patch::Diff::new(1, "1".to_string()),  diff_match_patch::Diff::new(0, "A".to_string()),  diff_match_patch::Diff::new(-1, "B".to_string()),  diff_match_patch::Diff::new(1, "2".to_string())];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "AB_AB".to_string()), diff_match_patch::Diff::new(1, "1A2_1A2".to_string())], diffs);

    // Word boundaries.
    diffs = vec![diff_match_patch::Diff::new(0, "The c".to_string()), diff_match_patch::Diff::new(-1, "ow and the c".to_string()), diff_match_patch::Diff::new(0, "at.".to_string())];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(0, "The ".to_string()), diff_match_patch::Diff::new(-1, "cow and the ".to_string()), diff_match_patch::Diff::new(0, "cat.".to_string())], diffs);

    // No overlap elimination.
    diffs = vec![diff_match_patch::Diff::new(-1, "abcxx".to_string()), diff_match_patch::Diff::new(1, "xxdef".to_string())];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "abcxx".to_string()), diff_match_patch::Diff::new(1, "xxdef".to_string())], diffs);

    // Overlap elimination.
    diffs = vec![diff_match_patch::Diff::new(-1, "abcxxx".to_string()), diff_match_patch::Diff::new(1, "xxxdef".to_string())];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "abc".to_string()), diff_match_patch::Diff::new(0, "xxx".to_string()), diff_match_patch::Diff::new(1, "def".to_string())], diffs);

    // Reverse overlap elimination.
    diffs = vec![diff_match_patch::Diff::new(-1, "xxxabc".to_string()), diff_match_patch::Diff::new(1, "defxxx".to_string())];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(1, "def".to_string()), diff_match_patch::Diff::new(0, "xxx".to_string()), diff_match_patch::Diff::new(-1, "abc".to_string())], diffs);

    // Two overlap eliminations.
    diffs = vec![diff_match_patch::Diff::new(-1, "abcd1212".to_string()), diff_match_patch::Diff::new(1, "1212efghi".to_string()), diff_match_patch::Diff::new(0, "----".to_string()), diff_match_patch::Diff::new(-1, "A3".to_string()), diff_match_patch::Diff::new(1, "3BC".to_string())];
    dmp.diff_cleanup_semantic(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "abcd".to_string()), diff_match_patch::Diff::new(0, "1212".to_string()), diff_match_patch::Diff::new(1, "efghi".to_string()), diff_match_patch::Diff::new(0, "----".to_string()), diff_match_patch::Diff::new(-1, "A".to_string()), diff_match_patch::Diff::new(0, "3".to_string()), diff_match_patch::Diff::new(1, "BC".to_string())], diffs);
}

#[test]
pub fn test_diff_cleanup_efficiency() { 
    let mut dmp = diff_match_patch::Dmp::new();  
    dmp.edit_cost = 4;
    // Null case.
    let mut diffs: Vec<diff_match_patch::Diff> = vec![];
    let temp: Vec<diff_match_patch::Diff> = vec![];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(temp, diffs);

    // No elimination.
    diffs = vec![diff_match_patch::Diff::new(-1, "ab".to_string()), diff_match_patch::Diff::new(1, "12".to_string()), diff_match_patch::Diff::new(0, "wxyz".to_string()), diff_match_patch::Diff::new(-1, "cd".to_string()), diff_match_patch::Diff::new(1, "34".to_string())];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "ab".to_string()), diff_match_patch::Diff::new(1, "12".to_string()), diff_match_patch::Diff::new(0, "wxyz".to_string()), diff_match_patch::Diff::new(-1, "cd".to_string()), diff_match_patch::Diff::new(1, "34".to_string())], diffs);

    // Four-edit elimination.
    diffs = vec![diff_match_patch::Diff::new(-1, "ab".to_string()), diff_match_patch::Diff::new(1, "12".to_string()), diff_match_patch::Diff::new(0, "xyz".to_string()), diff_match_patch::Diff::new(-1, "cd".to_string()), diff_match_patch::Diff::new(1, "34".to_string())];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "abxyzcd".to_string()), diff_match_patch::Diff::new(1, "12xyz34".to_string())], diffs);

    // Three-edit elimination.
    diffs = vec![diff_match_patch::Diff::new(1, "12".to_string()), diff_match_patch::Diff::new(0, "x".to_string()), diff_match_patch::Diff::new(-1, "cd".to_string()), diff_match_patch::Diff::new(1, "34".to_string())];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "xcd".to_string()), diff_match_patch::Diff::new(1, "12x34".to_string())], diffs);

    // Backpass elimination.
    diffs = vec![diff_match_patch::Diff::new(-1, "ab".to_string()), diff_match_patch::Diff::new(1, "12".to_string()), diff_match_patch::Diff::new(0, "xy".to_string()), diff_match_patch::Diff::new(1, "34".to_string()), diff_match_patch::Diff::new(0, "z".to_string()), diff_match_patch::Diff::new(-1, "cd".to_string()), diff_match_patch::Diff::new(1, "56".to_string())];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "abxyzcd".to_string()), diff_match_patch::Diff::new(1, "12xy34z56".to_string())], diffs);

    // High cost elimination.
    dmp.edit_cost = 5;
    diffs = vec![diff_match_patch::Diff::new(-1, "ab".to_string()), diff_match_patch::Diff::new(1, "12".to_string()), diff_match_patch::Diff::new(0, "wxyz".to_string()), diff_match_patch::Diff::new(-1, "cd".to_string()), diff_match_patch::Diff::new(1, "34".to_string())];
    dmp.diff_cleanup_efficiency(&mut diffs);
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "abwxyzcd".to_string()), diff_match_patch::Diff::new(1, "12wxyz34".to_string())], diffs);
}


#[test]
pub fn test_diff_text() {
    let mut dmp = diff_match_patch::Dmp::new();
    let mut diffs: Vec<diff_match_patch::Diff> = vec![diff_match_patch::Diff::new(0, "jump".to_string()), diff_match_patch::Diff::new(-1, "s".to_string()), diff_match_patch::Diff::new(1, "ed".to_string()), diff_match_patch::Diff::new(0, " over ".to_string()), diff_match_patch::Diff::new(-1, "the".to_string()), diff_match_patch::Diff::new(1, "a".to_string()), diff_match_patch::Diff::new(0, " lazy".to_string())];
    assert_eq!("jumps over the lazy".to_string(), dmp.diff_text1(&mut diffs));
    assert_eq!("jumped over a lazy".to_string(), dmp.diff_text2(&mut diffs));
}


#[test]
pub fn test_diff_delta() {

    let mut dmp = diff_match_patch::Dmp::new();
    let mut diffs = vec![diff_match_patch::Diff::new(0, "jump".to_string()), diff_match_patch::Diff::new(-1, "s".to_string()), diff_match_patch::Diff::new(1, "ed".to_string()), diff_match_patch::Diff::new(0, " over ".to_string()), diff_match_patch::Diff::new(-1, "the".to_string()), diff_match_patch::Diff::new(1, "a".to_string()), diff_match_patch::Diff::new(0, " lazy".to_string()), diff_match_patch::Diff::new(1, "old dog".to_string())];
    let mut text1 = dmp.diff_text1(&mut diffs);
    assert_eq!("jumps over the lazy".to_string(), text1);
    let mut delta = dmp.diff_todelta(&mut diffs);
    assert_eq!("=4\t-1\t+ed\t=6\t-3\t+a\t=5\t+old dog".to_string(), delta);

    // Convert delta string into a diff.
    assert_eq!(diffs, dmp.diff_from_delta(&text1, &delta));

    // # Generates error (19 != 20).
    // try:
    //     self.dmp.diff_fromDelta(text1 + "x", delta)
    //     self.assertFalse(True)
    // except ValueError:
    //     # Exception expected.
    //     pass

    // # Generates error (19 != 18).
    // try:
    //     self.dmp.diff_fromDelta(text1[1:], delta)
    //     self.assertFalse(True)
    // except ValueError:
    //     # Exception expected.
    //     pass

    // # Generates error (%c3%xy invalid Unicode).
    // # Note: Python 3 can decode this.
    // #try:
    // #  self.dmp.diff_fromDelta("", "+%c3xy")
    // #  self.assertFalse(True)
    // #except ValueError:
    // #  # Exception expected.
    // #  pass

    // Test deltas with special characters.
    diffs = vec![diff_match_patch::Diff::new(0, "\u{0680} \x00 \t %".to_string()), diff_match_patch::Diff::new(-1, "\u{0681} \x01 \n ^".to_string()), diff_match_patch::Diff::new(1, "\u{0682} \x02 \\ |".to_string())];
    text1 = dmp.diff_text1(&mut diffs);
    assert_eq!("\u{0680} \x00 \t %\u{0681} \x01 \n ^".to_string(), text1);

    delta = dmp.diff_todelta(&mut diffs);
    assert_eq!("=7\t-7\t+%DA%82 %02 %5C %7C".to_string(), delta);
    // Convert delta string into a diff.
    assert_eq!(diffs, dmp.diff_from_delta(&text1, &delta));

    // Verify pool of unchanged characters.
    diffs = vec![diff_match_patch::Diff::new(1, "A-Z a-z 0-9 - _ . ! ~ * ' ( ) ; / ? : @ & = + $ , # ".to_string())];
    let text2 = dmp.diff_text2(&mut diffs);
    assert_eq!("A-Z a-z 0-9 - _ . ! ~ * \' ( ) ; / ? : @ & = + $ , # ".to_string(), text2);

    delta = dmp.diff_todelta(&mut diffs);
    assert_eq!("+A-Z a-z 0-9 - _ . ! ~ * \' ( ) ; / ? : @ & = + $ , # ".to_string(), delta);

    // Convert delta string into a diff.
    assert_eq!(diffs, dmp.diff_from_delta(&("".to_string()), &delta));

    // 160 kb string.
    let mut a = "abcdefghij".to_string();
    for _i in 0..14 {
        a += a.clone().as_str();
    }
    diffs = vec![diff_match_patch::Diff::new(1, a.clone())];
    delta = dmp.diff_todelta(&mut diffs);
    assert_eq!('+'.to_string() + a.as_str(), delta);

    // Convert delta string into a diff.
    assert_eq!(diffs, dmp.diff_from_delta(&"".to_string(), &delta));
}


#[test]
pub fn test_diff_xindex() {

}


#[test]
pub fn test_diff_levenshtein() {

    let mut dmp = diff_match_patch::Dmp::new();
    assert_eq!(4, dmp.diff_levenshtein(&mut vec![diff_match_patch::Diff::new(-1, "abc".to_string()), diff_match_patch::Diff::new(1, "1234".to_string()), diff_match_patch::Diff::new(0, "xyz".to_string())]));
    // Levenshtein with leading equality.
    assert_eq!(4, dmp.diff_levenshtein(&mut vec![diff_match_patch::Diff::new(0, "xyz".to_string()), diff_match_patch::Diff::new(-1, "abc".to_string()), diff_match_patch::Diff::new(1, "1234".to_string())]));
    // # Levenshtein with middle equality.
    assert_eq!(7, dmp.diff_levenshtein(&mut vec![diff_match_patch::Diff::new(-1, "abc".to_string()), diff_match_patch::Diff::new(0, "xyz".to_string()), diff_match_patch::Diff::new(1, "1234".to_string())]));
}


#[test]
pub fn test_diff_bisect() {
    let mut dmp = diff_match_patch::Dmp::new();
    let a = "cat".to_string();
    let b = "map".to_string();
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "c".to_string()), diff_match_patch::Diff::new(1, "m".to_string()), diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(-1, "t".to_string()), diff_match_patch::Diff::new(1, "p".to_string())] , dmp.diff_bisect(&a.chars().collect(), &b.chars().collect()));
}


#[test]
pub fn test_diff_main() {
    let mut new_dmp = diff_match_patch::Dmp::new();
    let temp: Vec<diff_match_patch::Diff> = Vec::new();
    assert_eq!(temp, new_dmp.diff_main("", "", true));
    assert_eq!(vec![diff_match_patch::Diff::new(0, "abc".to_string())], new_dmp.diff_main("abc", "abc", true));
    assert_eq!(vec![diff_match_patch::Diff::new(0, "ab".to_string()), diff_match_patch::Diff::new(1, "123".to_string()), diff_match_patch::Diff::new(0, "c".to_string())], new_dmp.diff_main("abc", "ab123c", true));
    assert_eq!(vec![diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(-1, "123".to_string()), diff_match_patch::Diff::new(0, "bc".to_string())], new_dmp.diff_main("a123bc", "abc", true));
    assert_eq!(vec![diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(1, "123".to_string()), diff_match_patch::Diff::new(0, "b".to_string()), diff_match_patch::Diff::new(1, "456".to_string()), diff_match_patch::Diff::new(0, "c".to_string())], new_dmp.diff_main("abc", "a123b456c", true));
    assert_eq!(vec![diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(-1, "123".to_string()), diff_match_patch::Diff::new(0, "b".to_string()), diff_match_patch::Diff::new(-1, "456".to_string()), diff_match_patch::Diff::new(0, "c".to_string())], new_dmp.diff_main("a123b456c", "abc", true));
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "a".to_string()), diff_match_patch::Diff::new(1, "b".to_string())], new_dmp.diff_main("a", "b", true));
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "Apple".to_string()), diff_match_patch::Diff::new(1, "Banana".to_string()), diff_match_patch::Diff::new(0, "s are a".to_string()), diff_match_patch::Diff::new(1, "lso".to_string()), diff_match_patch::Diff::new(0, " fruit.".to_string())], new_dmp.diff_main("Apples are a fruit.", "Bananas are also fruit.", true));
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "a".to_string()), diff_match_patch::Diff::new(1, "\u{0680}".to_string()), diff_match_patch::Diff::new(0, "x".to_string()), diff_match_patch::Diff::new(-1, "\t".to_string()), diff_match_patch::Diff::new(1, "\n".to_string())], new_dmp.diff_main("ax\t", "\u{0680}x\n", false));
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "1".to_string()), diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(-1, "y".to_string()), diff_match_patch::Diff::new(0, "b".to_string()), diff_match_patch::Diff::new(-1, "2".to_string()), diff_match_patch::Diff::new(1, "xab".to_string())], new_dmp.diff_main("1ayb2", "abxab", false));
    assert_eq!(vec![diff_match_patch::Diff::new(1, "xaxcx".to_string()), diff_match_patch::Diff::new(0, "abc".to_string()), diff_match_patch::Diff::new(-1, "y".to_string())], new_dmp.diff_main("abcy", "xaxcxabc", false));
    assert_eq!(vec![diff_match_patch::Diff::new(-1, "ABCD".to_string()), diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(-1, "=".to_string()), diff_match_patch::Diff::new(1, "-".to_string()), diff_match_patch::Diff::new(0, "bcd".to_string()), diff_match_patch::Diff::new(-1, "=".to_string()), diff_match_patch::Diff::new(1, "-".to_string()), diff_match_patch::Diff::new(0, "efghijklmnopqrs".to_string()), diff_match_patch::Diff::new(-1, "EFGHIJKLMNOefg".to_string())], new_dmp.diff_main("ABCDa=bcd=efghijklmnopqrsEFGHIJKLMNOefg", "a-bcd-efghijklmnopqrs", false));
    assert_eq!(vec![diff_match_patch::Diff::new(1, " ".to_string()), diff_match_patch::Diff::new(0, "a".to_string()), diff_match_patch::Diff::new(1, "nd".to_string()), diff_match_patch::Diff::new(0, " [[Pennsylvania]]".to_string()), diff_match_patch::Diff::new(-1, " and [[New".to_string())], new_dmp.diff_main("a [[Pennsylvania]] and [[New", " and [[Pennsylvania]]", false));

    // let mut a: String = "`Twas brillig, and the slithy toves\nDid gyre and gimble in the wabe:\nAll mimsy were the borogoves,\nAnd the mome raths outgrabe.\n".to_string();
    // let mut b: String = "I am the very model of a modern major general,\nI've information vegetable, animal, and mineral,\nI know the kings of England, and I quote the fights historical,\nFrom Marathon to Waterloo, in order categorical.\n".to_string();
    // for x in 0..10 {
    //     a += a.clone().as_str();
    //     b += b.clone().as_str();
    // }

    let mut a = "1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n";
    let mut b = "abcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\n";
    assert_eq!(new_dmp.diff_main(a, b, true), new_dmp.diff_main(a, b, false));

    a = "1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890";
    b = "abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghij";
    assert_eq!(new_dmp.diff_main(a, b, true), new_dmp.diff_main(a, b, false));
    a = "1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n1234567890\n";
    b = "abcdefghij\n1234567890\n1234567890\n1234567890\nabcdefghij\n1234567890\n1234567890\n1234567890\nabcdefghij\n1234567890\n1234567890\n1234567890\nabcdefghij\n";
    let texts_linemode = diff_rebuildtexts(new_dmp.diff_main(a, b, true));
    let texts_textmode = diff_rebuildtexts(new_dmp.diff_main(a, b, false));
    assert_eq!(texts_linemode, texts_textmode);
}


#[test]
pub fn test_match_apphabet() {
    let mut dmp = diff_match_patch::Dmp::new();
    let mut s: HashMap<char,i32> = HashMap::new();
    s.insert('a', 4);
    s.insert('b', 2);
    s.insert('c', 1);
    assert_eq!(s, dmp.match_alphabet(&("abc".chars().collect())));
    s.insert('a', 37);
    s.insert('b', 18);
    s.insert('c', 8);
    assert_eq!(s, dmp.match_alphabet(&("abcaba".chars().collect())));
}


#[test]
pub fn test_match_bitap() {
    let mut dmp = diff_match_patch::Dmp::new();
    dmp.match_distance = 100;
    dmp.match_threshold = 0.5;
    assert_eq!(5, dmp.match_bitap(&("abcdefghijk".chars().collect()), &("fgh".chars().collect()), 5));
    assert_eq!(5, dmp.match_bitap(&("abcdefghijk".chars().collect()), &("fgh".chars().collect()), 0));

    // Fuzzy matches.
    assert_eq!(4, dmp.match_bitap(&("abcdefghijk".chars().collect()), &("efxhi".chars().collect()), 0));

    assert_eq!(2, dmp.match_bitap(&("abcdefghijk".chars().collect()), &("cdefxyhijk".chars().collect()), 5));

    assert_eq!(-1, dmp.match_bitap(&("abcdefghijk".chars().collect()), &("bxy".chars().collect()), 1));

    // Overflow.
    assert_eq!(2, dmp.match_bitap(&("123456789xx0".chars().collect()), &("3456789x0".chars().collect()), 2));

    assert_eq!(0, dmp.match_bitap(&("abcdef".chars().collect()), &("xxabc".chars().collect()), 4));

    assert_eq!(3, dmp.match_bitap(&("abcdef".chars().collect()), &("defyy".chars().collect()), 4));

    assert_eq!(0, dmp.match_bitap(&("abcdef".chars().collect()), &("xabcdefy".chars().collect()), 0));

    // Threshold test.
    dmp.match_threshold = 0.3;
    assert_eq!(-1, dmp.match_bitap(&("abcdefghijk".chars().collect()), &("efxyhi".chars().collect()), 1));

    dmp.match_threshold = 0.0;
    assert_eq!(1, dmp.match_bitap(&("abcdefghijk".chars().collect()), &("bcdef".chars().collect()), 1));
    dmp.match_threshold = 0.5;

    // Multiple select.
    assert_eq!(0, dmp.match_bitap(&("abcdexyzabcde".chars().collect()), &("abccde".chars().collect()), 3));

    assert_eq!(8, dmp.match_bitap(&("abcdexyzabcde".chars().collect()), &("abccde".chars().collect()), 5));

    // Distance test.
    dmp.match_distance = 10;
    assert_eq!(-1, dmp.match_bitap(&("abcdefghijklmnopqrstuvwxyz".chars().collect()), &("abcdefg".chars().collect()), 24));

    assert_eq!(0, dmp.match_bitap(&("abcdefghijklmnopqrstuvwxyz".chars().collect()), &("abcdxxefg".chars().collect()), 1));

    dmp.match_distance = 1000;
    assert_eq!(0, dmp.match_bitap(&("abcdefghijklmnopqrstuvwxyz".chars().collect()), &("abcdefg".chars().collect()), 24));


}


#[test]
pub fn test_match_main() {
    let mut dmp = diff_match_patch::Dmp::new();
    assert_eq!(0, dmp.match_main("abcdef", "abcdef", 1000));

    assert_eq!(-1, dmp.match_main("", "abcdef", 1));

    assert_eq!(3, dmp.match_main("abcdef", "", 3));

    assert_eq!(3, dmp.match_main("abcdef", "de", 3));

    assert_eq!(3, dmp.match_main("abcdef", "defy", 4));
    
    assert_eq!(0, dmp.match_main("abcdef", "abcdefy", 0));

    dmp.match_threshold = 0.7;
    assert_eq!(4, dmp.match_main("I am the very model of a modern major general.", " that berry ", 5));
    dmp.match_threshold = 0.5;
}


#[test]
pub fn test_patch_obj() {
    let mut patch = diff_match_patch::Patch::new(vec![], 0, 0, 0, 0);
    patch.start1 = 20;
    patch.start2 = 21;
    patch.length1 = 18;
    patch.length2 = 17;
    patch.diffs = vec![diff_match_patch::Diff::new(0, "jump".to_string()), diff_match_patch::Diff::new(-1, "s".to_string()), diff_match_patch::Diff::new(1, "ed".to_string()), diff_match_patch::Diff::new(0, " over ".to_string()), diff_match_patch::Diff::new(-1, "the".to_string()), diff_match_patch::Diff::new(1, "a".to_string()), diff_match_patch::Diff::new(0, "\nlaz".to_string())];
    assert_eq!("@@ -21,18 +22,17 @@\n jump\n-s\n+ed\n  over \n-the\n+a\n %0Alaz\n".to_string(), patch.to_string());
}


#[test]
pub fn test_patch_from_text() {
    let mut dmp = diff_match_patch::Dmp::new();
    let diffs: Vec<diff_match_patch::Patch> = vec![];
    assert_eq!(diffs, dmp.patch_from_text("".to_string()));
    
    let strp = "@@ -21,18 +22,17 @@\n jump\n-s\n+ed\n  over \n-the\n+a\n %0Alaz\n".to_string();
    assert_eq!(strp, dmp.patch_from_text(strp.clone())[0].to_string());

    assert_eq!("@@ -1,1 +1,1 @@\n-a\n+b\n".to_string(), dmp.patch_from_text("@@ -1 +1 @@\n-a\n+b\n".to_string())[0].to_string());

    assert_eq!("@@ -1,3 +0,0 @@\n-abc\n".to_string(), dmp.patch_from_text("@@ -1,3 +0,0 @@\n-abc\n".to_string())[0].to_string());

    assert_eq!("@@ -0,0 +1,3 @@\n+abc\n".to_string(), dmp.patch_from_text("@@ -0,0 +1,3 @@\n+abc\n".to_string())[0].to_string());
}

#[test]
pub fn test_patch_to_text() {
    let mut dmp = diff_match_patch::Dmp::new();
    let  mut strp = "@@ -21,18 +22,17 @@\n jump\n-s\n+ed\n  over \n-the\n+a\n  laz\n".to_string();
    let mut p = dmp.patch_from_text(strp.clone());
    assert_eq!(strp, dmp.patch_to_text(&mut p));

    strp = "@@ -1,9 +1,9 @@\n-f\n+F\n oo+fooba\n@@ -7,8 +7,8 @@\n obar\n-,\n+.\n tes\n".to_string();
    p = dmp.patch_from_text(strp.clone());
    assert_eq!(strp, dmp.patch_to_text(&mut p));
}


#[test]
pub fn test_patch_add_context()
{
    let mut dmp = diff_match_patch::Dmp::new();
    dmp.patch_margin = 4;
    let mut p = dmp.patch_from_text("@@ -21,4 +21,10 @@\n-jump\n+somersault\n".to_string())[0].clone();
    dmp.patch_add_context(&mut p, &mut ("The quick brown fox jumps over the lazy dog.".chars().collect()));
    assert_eq!(p.to_string(), "@@ -17,12 +17,18 @@\n fox \n-jump\n+somersault\n s ov\n".to_string());

    // Same, but not enough trailing context.
    p = dmp.patch_from_text("@@ -21,4 +21,10 @@\n-jump\n+somersault\n".to_string())[0].clone();
    dmp.patch_add_context(&mut p, &mut ("The quick brown fox jumps.".chars().collect()));
    assert_eq!(p.to_string(), "@@ -17,10 +17,16 @@\n fox \n-jump\n+somersault\n s.\n".to_string());

    // Same, but not enough leading context.
    let mut p = dmp.patch_from_text("@@ -3 +3,2 @@\n-e\n+at\n".to_string())[0].clone();
    dmp.patch_add_context(&mut p, &mut ("The quick brown fox jumps.".chars().collect()));
    assert_eq!(p.to_string(), "@@ -1,7 +1,8 @@\n Th\n-e\n+at\n  qui\n".to_string());

    // # Same, but with ambiguity.
    p = dmp.patch_from_text("@@ -3 +3,2 @@\n-e\n+at\n".to_string())[0].clone();
    dmp.patch_add_context(&mut p, &mut ("The quick brown fox jumps.  The quick brown fox crashes.".chars().collect()));
    assert_eq!(p.to_string(), "@@ -1,27 +1,28 @@\n Th\n-e\n+at\n  quick brown fox jumps. \n".to_string());
}


#[test]
pub fn test_patch_make() {
    let mut dmp = diff_match_patch::Dmp::new();
    // Null case.
    let mut patches = dmp.patch_make1("", "");
    assert_eq!("".to_string(), dmp.patch_to_text(&mut patches));

    let text1 = "The quick brown fox jumps over the lazy dog.";
    let text2 = "That quick brown fox jumped over a lazy dog.";
    // Text2+Text1 inputs.
    let mut expected_patch = "@@ -1,8 +1,7 @@\n Th\n-at\n+e\n  qui\n@@ -21,17 +21,18 @@\n jump\n-ed\n+s\n  over \n-a\n+the\n  laz\n".to_string();
    // The second patch must be "-21,17 +21,18", not "-22,17 +21,18" due to rolling context.
    patches = dmp.patch_make1(text2, text1);
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

    // Text1+Text2 inputs.
    expected_patch = "@@ -1,11 +1,12 @@\n Th\n-e\n+at\n  quick b\n@@ -22,18 +22,17 @@\n jump\n-s\n+ed\n  over \n-the\n+a\n  laz\n".to_string();
    patches = dmp.patch_make1(text1, text2);
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

    // Diff input.
    let mut diffs = dmp.diff_main(text1, text2, false);
    patches = dmp.patch_make2(&mut diffs);
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

    // Text1+Diff inputs.
    patches = dmp.patch_make4(text1, &mut diffs);
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

    // Text1+Text2+Diff inputs (deprecated).
    patches = dmp.patch_make3(text1, text2, &mut diffs);
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

    // Character encoding.
    patches = dmp.patch_make1("`1234567890-=[]\\;',./", "~!@#$%^&*()_+{}|:\"<>?");
    assert_eq!("@@ -1,21 +1,21 @@\n-%601234567890-=%5B%5D%5C;',./\n+~!@#$%25%5E&*()_+%7B%7D%7C:%22%3C%3E?\n".to_string(), dmp.patch_to_text(&mut patches));

    // Character decoding.
    diffs = vec![diff_match_patch::Diff::new(-1, "`1234567890-=[]\\;',./".to_string()), diff_match_patch::Diff::new(1, "~!@#$%^&*()_+{}|:\"<>?".to_string())];
    assert_eq!(diffs, dmp.patch_from_text("@@ -1,21 +1,21 @@\n-%601234567890-=%5B%5D%5C;',./\n+~!@#$%25%5E&*()_+%7B%7D%7C:%22%3C%3E?\n".to_string())[0].diffs);

    // Long string with repeats.
    let mut text1 = "".to_string();
    for _x in 0..100 {
        text1 += "abcdef";
    }
    let text2 = text1.clone() + "123";
    expected_patch = "@@ -573,28 +573,31 @@\n cdefabcdefabcdefabcdefabcdef\n+123\n".to_string();
    patches = dmp.patch_make1(text1.as_str(), text2.as_str());
    assert_eq!(expected_patch, dmp.patch_to_text(&mut patches));

}


#[test]
pub fn test_patch_splitmax() {
    let mut dmp = diff_match_patch::Dmp::new();
    // Assumes that Match_MaxBits is 31.
    dmp.match_maxbits = 32;
    let mut patches = dmp.patch_make1("abcdefghijklmnopqrstuvwxyz01234567890", "XabXcdXefXghXijXklXmnXopXqrXstXuvXwxXyzX01X23X45X67X89X0");
    dmp.patch_splitmax(&mut patches);
    assert_eq!("@@ -1,32 +1,46 @@\n+X\n ab\n+X\n cd\n+X\n ef\n+X\n gh\n+X\n ij\n+X\n kl\n+X\n mn\n+X\n op\n+X\n qr\n+X\n st\n+X\n uv\n+X\n wx\n+X\n yz\n+X\n 012345\n@@ -25,13 +39,18 @@\n zX01\n+X\n 23\n+X\n 45\n+X\n 67\n+X\n 89\n+X\n 0\n".to_string(), dmp.patch_to_text(&mut patches));

    patches = dmp.patch_make1("abcdef1234567890123456789012345678901234567890123456789012345678901234567890uvwxyz", "abcdefuvwxyz");
    let old_totext = dmp.patch_to_text(&mut patches);
    dmp.patch_splitmax(&mut patches);
    assert_eq!(old_totext, dmp.patch_to_text(&mut patches));

    patches = dmp.patch_make1("1234567890123456789012345678901234567890123456789012345678901234567890", "abc");
    dmp.patch_splitmax(&mut patches);
    assert_eq!("@@ -1,32 +1,4 @@\n-1234567890123456789012345678\n 9012\n@@ -29,32 +1,4 @@\n-9012345678901234567890123456\n 7890\n@@ -57,14 +1,3 @@\n-78901234567890\n+abc\n", dmp.patch_to_text(&mut patches));

    patches = dmp.patch_make1("abcdefghij , h : 0 , t : 1 abcdefghij , h : 0 , t : 1 abcdefghij , h : 0 , t : 1", "abcdefghij , h : 1 , t : 1 abcdefghij , h : 1 , t : 1 abcdefghij , h : 0 , t : 1");
    dmp.patch_splitmax(&mut patches);
    assert_eq!("@@ -2,32 +2,32 @@\n bcdefghij , h : \n-0\n+1\n  , t : 1 abcdef\n@@ -29,32 +29,32 @@\n bcdefghij , h : \n-0\n+1\n  , t : 1 abcdef\n".to_string(), dmp.patch_to_text(&mut patches));
}


#[test]
pub fn test_patch_add_padding() {
    // Both edges full.
    let mut dmp = diff_match_patch::Dmp::new();
    let mut patches = dmp.patch_make1("", "test");
    assert_eq!("@@ -0,0 +1,4 @@\n+test\n".to_string(), dmp.patch_to_text(&mut patches));
    dmp.patch_add_padding(&mut patches);
    assert_eq!("@@ -1,8 +1,12 @@\n %01%02%03%04\n+test\n %01%02%03%04\n".to_string(), dmp.patch_to_text(&mut patches));

    // Both edges partial.
    patches = dmp.patch_make1("XY", "XtestY");
    assert_eq!("@@ -1,2 +1,6 @@\n X\n+test\n Y\n".to_string(), dmp.patch_to_text(&mut patches));
    dmp.patch_add_padding(&mut patches);
    assert_eq!("@@ -2,8 +2,12 @@\n %02%03%04X\n+test\n Y%01%02%03\n".to_string(), dmp.patch_to_text(&mut patches));

    // Both edges none.
    patches = dmp.patch_make1("XXXXYYYY", "XXXXtestYYYY");
    assert_eq!("@@ -1,8 +1,12 @@\n XXXX\n+test\n YYYY\n".to_string(), dmp.patch_to_text(&mut patches));
    dmp.patch_add_padding(&mut patches);
    assert_eq!("@@ -5,8 +5,12 @@\n XXXX\n+test\n YYYY\n".to_string(), dmp.patch_to_text(&mut patches));
}


#[test]
pub fn test_patch_apply() {
    let mut dmp = diff_match_patch::Dmp::new();
    dmp.match_distance = 1000;
    dmp.match_threshold = 0.5;
    dmp.patch_delete_threshold = 0.5;
    // Null case.
    let mut patches = dmp.patch_make1("", "");
    let mut results = dmp.patch_apply(&mut patches, "Hello world.");
    assert_eq!(("Hello world.".chars().collect(), vec![]), results);

    // Exact match.
    patches = dmp.patch_make1("The quick brown fox jumps over the lazy dog.", "That quick brown fox jumped over a lazy dog.");
    results = dmp.patch_apply(&mut patches, "The quick brown fox jumps over the lazy dog.");
    assert_eq!(("That quick brown fox jumped over a lazy dog.".chars().collect(), vec![true, true]), results);

    // Partial match.
    results = dmp.patch_apply(&mut patches, "The quick red rabbit jumps over the tired tiger.");
    assert_eq!(("That quick red rabbit jumped over a tired tiger.".chars().collect(), vec![true, true]), results);

    // Failed match.
    results = dmp.patch_apply(&mut patches, "I am the very model of a modern major general.");
    assert_eq!(("I am the very model of a modern major general.".chars().collect(), vec![false, false]), results);

    // Big delete, small change.
    patches = dmp.patch_make1("x1234567890123456789012345678901234567890123456789012345678901234567890y", "xabcy");
    results = dmp.patch_apply(&mut patches, "x123456789012345678901234567890-----++++++++++-----123456789012345678901234567890y");
    assert_eq!(("xabcy".chars().collect(), vec![true, true]), results);

    // Big delete, big change 1.
    patches = dmp.patch_make1("x1234567890123456789012345678901234567890123456789012345678901234567890y", "xabcy");
    results = dmp.patch_apply(&mut patches, "x12345678901234567890---------------++++++++++---------------12345678901234567890y");
    assert_eq!(("xabc12345678901234567890---------------++++++++++---------------12345678901234567890y".chars().collect(), vec![false, true]), results);

    // Big delete, big change 2.
    dmp.patch_delete_threshold = 0.6;
    patches = dmp.patch_make1("x1234567890123456789012345678901234567890123456789012345678901234567890y", "xabcy");
    results = dmp.patch_apply(&mut patches, "x12345678901234567890---------------++++++++++---------------12345678901234567890y");
    assert_eq!(("xabcy".chars().collect(), vec![true, true]), results);
    dmp.patch_delete_threshold = 0.5;

    // Compensate for failed patch.
    dmp.match_threshold = 0.0;
    dmp.match_distance = 0;
    patches = dmp.patch_make1("abcdefghijklmnopqrstuvwxyz--------------------1234567890", "abcXXXXXXXXXXdefghijklmnopqrstuvwxyz--------------------1234567YYYYYYYYYY890");
    results = dmp.patch_apply(&mut patches, "ABCDEFGHIJKLMNOPQRSTUVWXYZ--------------------1234567890");
    assert_eq!(("ABCDEFGHIJKLMNOPQRSTUVWXYZ--------------------1234567YYYYYYYYYY890".chars().collect(), vec![false, true]), results);
    dmp.match_threshold = 0.5;
    dmp.match_distance = 1000;

    // No side effects.
    patches = dmp.patch_make1("", "test");
    let mut patchstr = dmp.patch_to_text(&mut patches);
    results = dmp.patch_apply(&mut patches, "");
    assert_eq!(patchstr, dmp.patch_to_text(&mut patches));

    // No side effects with major delete.
    patches = dmp.patch_make1("The quick brown fox jumps over the lazy dog.", "Woof");
    patchstr = dmp.patch_to_text(&mut patches);
    dmp.patch_apply(&mut patches, "The quick brown fox jumps over the lazy dog.");
    assert_eq!(patchstr, dmp.patch_to_text(&mut patches));

    // Edge exact match.
    patches = dmp.patch_make1("", "test");
    dmp.patch_apply(&mut patches, "");
    assert_eq!(("test".chars().collect(), vec![true]), results);

    // Near edge exact match.
    patches = dmp.patch_make1("XY", "XtestY");
    results = dmp.patch_apply(&mut patches, "XY");
    assert_eq!(("XtestY".chars().collect(), vec![true]), results);

    // Edge partial match.
    patches = dmp.patch_make1("y", "y123");
    results = dmp.patch_apply(&mut patches, "x");
    assert_eq!(("x123".chars().collect(), vec![true]), results);
}
