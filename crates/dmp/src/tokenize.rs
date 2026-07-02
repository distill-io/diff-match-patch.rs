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

        let (chars1, chars2, store) = lines_tochars_arena(text1, text2);
        (chars1, chars2, store.into_linearray())
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
        lines_munge(text, linearray, linehash)
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

/// FxHash-style hasher for the internal line-interning map: line packing
/// hashes every line of both inputs, and SipHash's per-call overhead was
/// ~13% of realistic diffs. NOT collision-hardened — acceptable here because
/// the map only interns the lines of one diff call (an adversarial document
/// degrades that document's own diff, nothing shared or persistent), and the
/// public String-keyed API keeps std's SipHash.
#[derive(Default)]
struct FxHasher(u64);

impl std::hash::Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        let mut h = self.0;
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let v = u64::from_le_bytes(chunk.try_into().expect("exact chunk"));
            h = (h.rotate_left(5) ^ v).wrapping_mul(SEED);
        }
        let rem = chunks.remainder();
        if !rem.is_empty() {
            let mut buf = [0u8; 8];
            buf[..rem.len()].copy_from_slice(rem);
            h = (h.rotate_left(5) ^ u64::from_le_bytes(buf)).wrapping_mul(SEED);
        }
        self.0 = h;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type FxMap<K, V> = HashMap<K, V, std::hash::BuildHasherDefault<FxHasher>>;

fn fx_hash_bytes(bytes: &[u8]) -> u64 {
    use std::hash::Hasher;
    let mut h = FxHasher::default();
    h.write(bytes);
    h.finish()
}

/// Interned line storage for the internal line packer: every line's bytes
/// live in one UTF-8 arena, id slots map to byte spans, and an fx-hash
/// bucket index dedups by byte compare — so a repeated line costs one encode
/// into the arena tip (rolled back on hit) and one hash, with no per-line
/// String at all.
pub(crate) struct LineArena {
    arena: String,
    /// Byte span per id slot; slot 0 is the intentionally blank sentinel.
    spans: Vec<(usize, usize)>,
    /// Content hash → slots with that hash (each verified by byte compare).
    buckets: FxMap<u64, Vec<usize>>,
}

impl LineArena {
    fn new() -> LineArena {
        LineArena {
            arena: String::new(),
            spans: vec![(0, 0)],
            buckets: FxMap::default(),
        }
    }

    /// The `Vec<String>` shape of the public API.
    fn into_linearray(self) -> Vec<String> {
        self.spans
            .iter()
            .map(|&(s, e)| self.arena[s..e].to_string())
            .collect()
    }

    /// Slot whose bytes equal the arena tip starting at `start`, if any.
    fn find_tip(&self, start: usize, h: u64) -> Option<usize> {
        let bucket = self.buckets.get(&h)?;
        for &slot in bucket {
            let (s, e) = self.spans[slot];
            if self.arena.as_bytes()[s..e] == self.arena.as_bytes()[start..] {
                return Some(slot);
            }
        }
        None
    }
}

/// The placeholder char for an id slot: sequential, skipping the surrogate
/// range exactly as the historical id assignment did.
fn slot_to_id(slot: usize) -> u32 {
    if slot >= 55296 {
        slot as u32 + 2048
    } else {
        slot as u32
    }
}

/// Pack both texts into one placeholder char per unique line (the slice core
/// behind `diff_lines_tochars`, shared with line mode so it can pass slices
/// without copying its inputs).
pub(crate) fn lines_tochars_arena(text1: &[char], text2: &[char]) -> (String, String, LineArena) {
    let mut store = LineArena::new();
    let chars1 = munge_arena(text1, &mut store);
    let chars2 = munge_arena(text2, &mut store);
    (chars1, chars2, store)
}

fn munge_arena(text: &[char], store: &mut LineArena) -> String {
    let mut chars = "".to_string();
    // Walk the text, pulling out a substring for each line.
    let mut line_start = 0;
    let mut line_end = -1;
    while line_end < (text.len() as i32 - 1) {
        line_end = find_char('\n', text, line_start as usize);
        if line_end == -1 {
            line_end = text.len() as i32 - 1;
        }
        // Encode the line straight into the arena tip; a hit rolls it back.
        let start = store.arena.len();
        store
            .arena
            .extend(text[line_start as usize..=line_end as usize].iter());
        let mut h = fx_hash_bytes(&store.arena.as_bytes()[start..]);
        match store.find_tip(start, h) {
            Some(slot) => {
                store.arena.truncate(start);
                if let Some(char1) = char::from_u32(slot_to_id(slot)) {
                    chars.push(char1);
                    line_start = line_end + 1;
                }
            }
            None => {
                let slot = store.spans.len();
                let mut u32char = slot as i32;

                // skip reserved range - U+D800 to U+DFFF
                // unicode code points in this range can't be converted to unicode scalars
                if u32char >= 55296 {
                    u32char += 2048;
                }

                // 1114111 is the biggest unicode scalar, so stop here
                if u32char == 1114111 {
                    store.arena.truncate(start);
                    store.arena.extend(text[(line_start as usize)..].iter());
                    line_end = text.len() as i32 - 1;
                    h = fx_hash_bytes(&store.arena.as_bytes()[start..]);
                }

                store.spans.push((start, store.arena.len()));
                store.buckets.entry(h).or_default().push(slot);

                chars.push(char::from_u32(u32char as u32).unwrap());
                line_start = line_end + 1;
            }
        }
    }
    chars
}

/// Word-packing counterpart of `lines_tochars_arena`, for the opt-in word
/// mode: words are maximal non-whitespace runs and every whitespace char is
/// its own token — the public word munge's regex-\s semantics.
pub(crate) fn words_tochars_arena(text1: &[char], text2: &[char]) -> (String, String, LineArena) {
    let mut store = LineArena::new();
    let chars1 = munge_words_arena(text1, &mut store);
    let chars2 = munge_words_arena(text2, &mut store);
    (chars1, chars2, store)
}

fn munge_words_arena(text: &[char], store: &mut LineArena) -> String {
    let mut chars = "".to_string();
    let mut word_start: Option<usize> = None;
    for i in 0..text.len() {
        if text[i].is_whitespace() {
            if let Some(start) = word_start.take() {
                if !push_word_token(store, &mut chars, text, start, i) {
                    return chars;
                }
            }
            if !push_word_token(store, &mut chars, text, i, i + 1) {
                return chars;
            }
        } else if word_start.is_none() {
            word_start = Some(i);
        }
    }
    if let Some(start) = word_start {
        push_word_token(store, &mut chars, text, start, text.len());
    }
    chars
}

/// Intern `text[from..to]` and append its placeholder char. When the id
/// space is exhausted the token swallows the rest of the text (the same
/// escape hatch as the line packer) and returns false to stop the walk.
fn push_word_token(
    store: &mut LineArena,
    chars: &mut String,
    text: &[char],
    from: usize,
    to: usize,
) -> bool {
    let start = store.arena.len();
    store.arena.extend(text[from..to].iter());
    let mut h = fx_hash_bytes(&store.arena.as_bytes()[start..]);
    if let Some(slot) = store.find_tip(start, h) {
        store.arena.truncate(start);
        chars.push(char::from_u32(slot_to_id(slot)).expect("interned ids are valid scalars"));
        return true;
    }
    let slot = store.spans.len();
    let mut u32char = slot as i32;
    // skip reserved range - U+D800 to U+DFFF
    if u32char >= 55296 {
        u32char += 2048;
    }
    let mut exhausted = false;
    // 1114111 is the biggest unicode scalar, so stop here
    if u32char == 1114111 {
        store.arena.truncate(start);
        store.arena.extend(text[from..].iter());
        h = fx_hash_bytes(&store.arena.as_bytes()[start..]);
        exhausted = true;
    }
    store.spans.push((start, store.arena.len()));
    store.buckets.entry(h).or_default().push(slot);
    chars.push(char::from_u32(u32char as u32).unwrap());
    !exhausted
}

/// Rehydrate line-packed diffs from the arena — the internal counterpart of
/// `diff_chars_tolines`, indexing spans by raw char value exactly as the
/// public path indexes its linearray.
pub(crate) fn chars_tolines_arena(diffs: &mut [Diff], store: &LineArena) {
    for diff in diffs.iter_mut() {
        let mut total = 0;
        for ch in diff.text.chars() {
            let (s, e) = store.spans[ch as usize];
            total += e - s;
        }
        let mut text = String::with_capacity(total);
        for ch in diff.text.chars() {
            let (s, e) = store.spans[ch as usize];
            text.push_str(&store.arena[s..e]);
        }
        diff.text = text;
    }
}

/// The String-keyed walk behind the public `diff_lines_tochars_munge`, whose
/// signature (a caller-owned `HashMap<String, i32>`) is frozen by the
/// drop-in compatibility contract.
fn lines_munge(
    text: &[char],
    linearray: &mut Vec<String>,
    linehash: &mut HashMap<String, i32>,
) -> String {
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
