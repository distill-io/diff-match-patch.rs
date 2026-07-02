// Tokenization: packing lines and words into one placeholder char per unique
// token so the char-based diff core can operate on coarser granularities.

use crate::types::{find_char, Diff, Dmp};
use core::char;
use std::collections::HashMap;

// The historic public API takes &Vec/&mut Vec/&String; frozen by the
// drop-in compatibility contract.
#[allow(clippy::ptr_arg)]
impl Dmp {
    pub fn diff_words_tochars(
        &mut self,
        text1: &String,
        text2: &String,
    ) -> (String, String, Vec<String>) {
        /*
        Split two texts into an array of strings.  Reduce the texts to a string
        of hashes where each Unicode character represents one word.

        Args:
            text1: First chars.
            text2: Second chars.

        Returns:
            Three element tuple, containing the encoded text1, the encoded text2 and
            the array of unique strings.  The zeroth element of the array of unique
            strings is intentionally blank.
        */

        let mut wordarray: Vec<String> = vec!["".to_string()];
        let mut wordhash: HashMap<String, u32> = HashMap::new();
        let chars1 = self.diff_words_tochars_munge(text1, &mut wordarray, &mut wordhash);
        let mut dmp = Dmp::new();
        let chars2 = dmp.diff_words_tochars_munge(text2, &mut wordarray, &mut wordhash);
        (chars1, chars2, wordarray)
    }

    pub fn diff_words_tochars_munge(
        &mut self,
        text: &String,
        wordarray: &mut Vec<String>,
        wordhash: &mut HashMap<String, u32>,
    ) -> String {
        /*
        Split a text into an array of strings.  Reduce the texts to a string
        of hashes where each Unicode character represents one word.
        Modifies wordarray and wordhash through being a closure.

        Args:
            text: chars to encode.

        Returns:
            Encoded string.
        */
        let mut chars = "".to_string();

        // Split into words and single-char whitespace tokens: each Unicode
        // whitespace char is its own token (the regex-\s semantics pinned by
        // test_diff_words_tochars_unicode_whitespace).
        let mut word_start: Option<usize> = None;
        for (idx, ch) in text.char_indices() {
            if ch.is_whitespace() {
                if let Some(start) = word_start.take() {
                    chars += &self.make_token_dict(&text[start..idx], wordarray, wordhash);
                }
                chars +=
                    &self.make_token_dict(&text[idx..idx + ch.len_utf8()], wordarray, wordhash);
            } else if word_start.is_none() {
                word_start = Some(idx);
            }
        }
        if let Some(start) = word_start {
            chars += &self.make_token_dict(&text[start..], wordarray, wordhash);
        }
        chars
    }

    fn make_token_dict(
        &mut self,
        word: &str,
        wordarray: &mut Vec<String>,
        wordhash: &mut HashMap<String, u32>,
    ) -> String {
        if !wordhash.contains_key(word) {
            wordarray.push(word.to_string());
            wordhash.insert(word.to_string(), wordarray.len() as u32 - 1);
        }
        char::from_u32(wordhash[word]).unwrap().to_string()
    }

    pub fn diff_lines_tochars(
        &mut self,
        text1: &Vec<char>,
        text2: &Vec<char>,
    ) -> (String, String, Vec<String>) {
        /*
        Split two texts into an array of strings.  Reduce the texts to a string
        of hashes where each Unicode character represents one line.

        Args:
            text1: First chars.
            text2: Second chars.

        Returns:
            Three element tuple, containing the encoded text1, the encoded text2 and
            the array of unique strings.  The zeroth element of the array of unique
            strings is intentionally blank.
        */

        let mut linearray: Vec<String> = vec!["".to_string()];
        let mut linehash: HashMap<String, i32> = HashMap::new();
        let chars1 = self.diff_lines_tochars_munge(text1, &mut linearray, &mut linehash);
        let mut dmp = Dmp::new();
        let chars2 = dmp.diff_lines_tochars_munge(text2, &mut linearray, &mut linehash);
        (chars1, chars2, linearray)
    }

    pub fn diff_lines_tochars_munge(
        &mut self,
        text: &Vec<char>,
        linearray: &mut Vec<String>,
        linehash: &mut HashMap<String, i32>,
    ) -> String {
        /*
        Split a text into an array of strings.  Reduce the texts to a string
        of hashes where each Unicode character represents one line.
        Modifies linearray and linehash through being a closure.

        Args:
            text: chars to encode.

        Returns:
            Encoded string.
        */
        let mut chars = "".to_string();
        // Walk the text, pulling out a substring for each line.
        // text.split('\n') would would temporarily double our memory footprint.
        // Modifying text would create many large strings to garbage collect.
        let mut line_start = 0;
        let mut line_end = -1;
        let mut line: String;
        while line_end < (text.len() as i32 - 1) {
            line_end = find_char('\n', text, line_start as usize);
            if line_end == -1 {
                line_end = text.len() as i32 - 1;
            }
            line = text[line_start as usize..=line_end as usize]
                .iter()
                .collect();
            if linehash.contains_key(&line) {
                if let Some(char1) = char::from_u32(linehash[&line] as u32) {
                    chars.push(char1);
                    line_start = line_end + 1;
                }
            } else {
                let mut u32char = linearray.len() as i32;

                // skip reserved range - U+D800 to U+DFFF
                // unicode code points in this range can't be converted to unicode scalars
                if u32char >= 55296 {
                    u32char += 2048;
                }

                // 1114111 is the biggest unicode scalar, so stop here
                if u32char == 1114111 {
                    line = text[(line_start as usize)..].iter().collect();
                    line_end = text.len() as i32 - 1;
                }

                linearray.push(line.clone());
                linehash.insert(line.clone(), u32char);

                chars.push(char::from_u32(u32char as u32).unwrap());
                line_start = line_end + 1;
            }
        }
        chars
    }

    /// Rehydrate the text in a diff from a string of line hashes to real lines
    /// of text.
    ///
    /// Args:
    /// diffs: Vector of diffs as changes.
    /// lineArray: Vector of unique strings.
    pub fn diff_chars_tolines(&mut self, diffs: &mut Vec<Diff>, line_array: &Vec<String>) {
        for diff in diffs.iter_mut() {
            let mut text: String = "".to_string();
            for ch in diff.text.chars() {
                text += line_array[ch as usize].as_str();
            }
            diff.text = text;
        }
    }
}

/// Packs texts so each extended grapheme cluster becomes exactly one char:
/// single-char clusters map to themselves (so plain text is untouched) and
/// multi-char clusters get placeholder chars guaranteed absent from every
/// input, recorded in `reverse` for unpacking.
///
/// Identity mapping is what makes single-char-cluster text behave exactly
/// like char mode, but it means ids share one namespace with real input
/// chars — so ids must be scanned against `used`, unlike the line packer's
/// sequential ids which never mix with raw text.
#[cfg(feature = "grapheme")]
pub(crate) struct GraphemePacker {
    forward: HashMap<String, char>,
    reverse: HashMap<char, String>,
    used: std::collections::HashSet<char>,
    cursor: u32,
}

#[cfg(feature = "grapheme")]
impl GraphemePacker {
    pub fn new(texts: &[&str]) -> GraphemePacker {
        GraphemePacker {
            forward: HashMap::new(),
            reverse: HashMap::new(),
            used: texts.iter().flat_map(|t| t.chars()).collect(),
            // Start in the private use area to make fresh ids likely on the
            // first probe; correctness comes from the `used` check alone.
            cursor: 0xE000,
        }
    }

    fn fresh_id(&mut self) -> char {
        loop {
            if self.cursor > 0x0010_FFFF {
                // > 1M distinct multi-char clusters plus input alphabet.
                panic!("too many distinct grapheme clusters to pack");
            }
            let candidate = char::from_u32(self.cursor);
            self.cursor += 1;
            if let Some(ch) = candidate {
                if !self.used.contains(&ch) {
                    self.used.insert(ch);
                    return ch;
                }
            }
        }
    }

    pub fn pack(&mut self, text: &str) -> String {
        use unicode_segmentation::UnicodeSegmentation;
        let mut packed = String::with_capacity(text.len());
        for cluster in text.graphemes(true) {
            let mut chars = cluster.chars();
            let first = chars.next().expect("graphemes are non-empty");
            if chars.next().is_none() {
                packed.push(first);
            } else if let Some(&id) = self.forward.get(cluster) {
                packed.push(id);
            } else {
                let id = self.fresh_id();
                self.forward.insert(cluster.to_string(), id);
                self.reverse.insert(id, cluster.to_string());
                packed.push(id);
            }
        }
        packed
    }

    pub fn unpack(&self, packed: &str) -> String {
        let mut text = String::with_capacity(packed.len());
        for ch in packed.chars() {
            match self.reverse.get(&ch) {
                Some(cluster) => text += cluster,
                None => text.push(ch),
            }
        }
        text
    }

    pub fn unpack_diffs(&self, diffs: &mut [Diff]) {
        for diff in diffs {
            diff.text = self.unpack(&diff.text);
        }
    }
}
