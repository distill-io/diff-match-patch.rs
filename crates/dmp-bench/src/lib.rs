//! Deterministic dataset generators for the diff_match_patch perf suite.
//!
//! Both consumers — the criterion bench (`benches/dmp.rs`) and the pprof
//! harness (`src/bin/profile.rs`) — build their workloads from these
//! generators, so profiles measure the same inputs the benches time.
//! Everything is seeded xorshift, no ambient entropy: criterion baselines
//! and flamegraphs stay comparable across runs and machines.

use diff_match_patch::{Diff, Patch};

/// xorshift64. Deterministic, dependency-free; NOT for anything but datasets.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    pub fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const WORDS: &[&str] = &[
    "the", "of", "and", "monitor", "change", "report", "value", "signal", "page", "update",
    "track", "alert", "daily", "weekly", "source", "filter", "window", "content", "version",
    "history", "archive", "render", "browser", "select", "region", "text", "element", "visible",
    "hidden", "status", "error", "retry", "queue", "worker", "batch", "cache", "index", "field",
    "record", "export", "import", "label", "group", "policy", "limit", "usage", "quota", "owner",
    "member", "device", "channel", "email", "push", "webhook", "digest", "summary", "detail",
    "preview", "snapshot", "schedule",
];

/// Sentence-per-line ASCII prose with paragraph breaks; ~6.4 bytes per word.
pub fn prose(n_words: usize, seed: u64) -> String {
    let mut rng = Rng::new(seed);
    let mut out = String::with_capacity(n_words * 7);
    let mut in_sentence = 0;
    let mut sentence_len = 8 + rng.below(7);
    let mut in_para = 0;
    let mut para_len = 4 + rng.below(4);
    for _ in 0..n_words {
        let w = rng.pick(WORDS);
        if in_sentence == 0 {
            let mut chars = w.chars();
            out.extend(chars.next().unwrap().to_uppercase());
            out.push_str(chars.as_str());
        } else {
            out.push_str(w);
        }
        in_sentence += 1;
        if in_sentence == sentence_len {
            out.push('.');
            in_sentence = 0;
            sentence_len = 8 + rng.below(7);
            in_para += 1;
            if in_para == para_len {
                out.push_str("\n\n");
                in_para = 0;
                para_len = 4 + rng.below(4);
            } else {
                out.push('\n');
            }
        } else {
            out.push(' ');
        }
    }
    out.push('\n');
    out
}

/// Byte spans of ASCII-alphanumeric word runs.
fn word_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = vec![];
    let mut start: Option<usize> = None;
    for (i, ch) in text.char_indices() {
        if ch.is_ascii_alphanumeric() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            spans.push((s, i));
        }
    }
    if let Some(s) = start {
        spans.push((s, text.len()));
    }
    spans
}

/// `n` word-level edits (replace / insert / delete) spread evenly across the
/// text — the "someone touched the document in n places" shape.
pub fn scattered_word_edits(text: &str, n: usize, seed: u64) -> String {
    let mut rng = Rng::new(seed);
    let spans = word_spans(text);
    assert!(spans.len() >= n * 2, "text too small for {n} edits");
    let mut chosen: Vec<usize> = (0..n)
        .map(|k| {
            let lo = k * spans.len() / n;
            let hi = ((k + 1) * spans.len() / n).max(lo + 1);
            lo + rng.below(hi - lo)
        })
        .collect();
    chosen.dedup();
    let mut out = String::with_capacity(text.len() + 16 * n);
    let mut last = 0;
    for &si in &chosen {
        let (s, e) = spans[si];
        out.push_str(&text[last..s]);
        let orig = &text[s..e];
        let mut w = rng.pick(WORDS);
        while w == orig {
            w = rng.pick(WORDS);
        }
        match rng.below(6) {
            // Insert a word before the original.
            0 => {
                out.push_str(w);
                out.push(' ');
                out.push_str(orig);
            }
            // Delete the word (leaves the separator).
            1 => {}
            // Replace it.
            _ => out.push_str(w),
        }
        last = e;
    }
    out.push_str(&text[last..]);
    out
}

/// ~50KB prose with a single replaced word in the middle.
pub fn pair_small_edit() -> (String, String) {
    let t1 = prose(9_000, 101);
    let spans = word_spans(&t1);
    let (s, e) = spans[spans.len() / 2];
    let t2 = format!("{}dashboard{}", &t1[..s], &t1[e..]);
    (t1, t2)
}

/// ~50KB prose with 30 word-level edits spread across it.
pub fn pair_scattered() -> (String, String) {
    let t1 = prose(9_000, 102);
    let t2 = scattered_word_edits(&t1, 30, 103);
    (t1, t2)
}

/// A product-listing page where prices/stock change between snapshots, one
/// item disappears and one appears — the web-monitoring shape (a Distill
/// "source" between two checks). ~90KB per side.
pub fn pair_html_churn() -> (String, String) {
    const N: usize = 400;
    let mut rng = Rng::new(7);
    let mut prices: Vec<u32> = Vec::with_capacity(N);
    let mut stock: Vec<bool> = Vec::with_capacity(N);
    for _ in 0..N {
        prices.push(199 + rng.below(99_800) as u32);
        stock.push(rng.below(4) != 0);
    }
    let mut prices2 = prices.clone();
    let mut stock2 = stock.clone();
    for _ in 0..25 {
        let i = rng.below(N);
        prices2[i] = 199 + rng.below(99_800) as u32;
    }
    for _ in 0..6 {
        let i = rng.below(N);
        stock2[i] = !stock2[i];
    }

    let render = |prices: &[u32],
                  stock: &[bool],
                  stamp: &str,
                  drop: Option<usize>,
                  add: Option<usize>| {
        let mut s = String::with_capacity(N * 220);
        s.push_str("<html><body>\n<div id=\"app\">\n");
        s.push_str(&format!("<p class=\"updated\">Last updated {stamp}</p>\n"));
        for i in 0..N {
            if Some(i) == drop {
                continue;
            }
            s.push_str(&format!(
                "<div class=\"item\" id=\"item-{i}\">\n  <span class=\"name\">Product {i} edition</span>\n  <span class=\"price\">${}.{:02}</span>\n  <span class=\"stock\">{}</span>\n</div>\n",
                prices[i] / 100,
                prices[i] % 100,
                if stock[i] { "in stock" } else { "out of stock" },
            ));
            if Some(i) == add {
                s.push_str("<div class=\"item item-new\" id=\"item-new\">\n  <span class=\"name\">Brand new listing</span>\n  <span class=\"price\">$1.99</span>\n  <span class=\"stock\">preorder</span>\n</div>\n");
            }
        }
        s.push_str("</div>\n</body></html>\n");
        s
    };
    let t1 = render(&prices, &stock, "2026-07-01 08:00", None, None);
    let t2 = render(&prices2, &stock2, "2026-07-02 08:00", Some(137), Some(300));
    (t1, t2)
}

/// ~5KB appended to a ~50KB base (log/feed growth).
pub fn pair_append() -> (String, String) {
    let t1 = prose(9_000, 104);
    let extra = prose(900, 105);
    let t2 = format!("{t1}{extra}");
    (t1, t2)
}

/// ~5KB prepended to a ~50KB base (new items on top of a feed).
pub fn pair_prepend() -> (String, String) {
    let t1 = prose(9_000, 106);
    let extra = prose(900, 107);
    let t2 = format!("{extra}{t1}");
    (t1, t2)
}

/// ~60KB prose with a ~6KB run of whole lines moved from the first quarter to
/// the third quarter.
pub fn pair_block_move() -> (String, String) {
    let t1 = prose(11_000, 108);
    let s = t1[..t1.len() / 4].rfind('\n').unwrap() + 1;
    let e = s + t1[s..s + 6_000].rfind('\n').unwrap() + 1;
    let block = t1[s..e].to_string();
    let rest = format!("{}{}", &t1[..s], &t1[e..]);
    let at = rest[..rest.len() * 3 / 4].rfind('\n').unwrap() + 1;
    let t2 = format!("{}{}{}", &rest[..at], block, &rest[at..]);
    (t1, t2)
}

/// ~60KB prose with a ~6KB run of whole lines deleted.
pub fn pair_block_delete() -> (String, String) {
    let t1 = prose(11_000, 109);
    let s = t1[..t1.len() / 3].rfind('\n').unwrap() + 1;
    let e = s + t1[s..s + 6_000].rfind('\n').unwrap() + 1;
    let t2 = format!("{}{}", &t1[..s], &t1[e..]);
    (t1, t2)
}

/// ~18KB of code where an identifier is renamed globally: nearly every line
/// changes by a few chars. Line mode buys nothing (every line is unique to
/// its side), so this stresses the char-level rediff of a big edit block.
pub fn pair_code_rename() -> (String, String) {
    let mut t1 = String::with_capacity(20_000);
    for i in 0..320 {
        match i % 16 {
            0 => t1.push_str(&format!(
                "fn compute_metric_{i:03}(rows: &[Row], cfg: &Config) -> Metric {{\n"
            )),
            15 => t1.push_str("}\n\n"),
            _ => t1.push_str(&format!(
                "    let metric_{i:03} = rows[{}].metric * cfg.metric_scale + offset_{i:03};\n",
                i % 7
            )),
        }
    }
    let t2 = t1.replace("metric", "aggregate_value");
    (t1, t2)
}

/// ~20K CJK scalars (~60KB UTF-8) with 30 scattered single-char edits.
pub fn pair_cjk() -> (String, String) {
    let mut rng = Rng::new(11);
    let mut gen_char = {
        let mut r = Rng::new(12);
        move || char::from_u32(0x4E00 + r.below(0x51A5) as u32).unwrap()
    };
    let n = 20_000;
    let mut chars: Vec<char> = Vec::with_capacity(n + n / 20);
    for i in 1..=n {
        chars.push(gen_char());
        if i % 23 == 0 {
            chars.push('。');
        }
        if i % 61 == 0 {
            chars.push('\n');
        }
    }
    let mut c2 = chars.clone();
    for k in 0..30 {
        let pos = k * (c2.len() - 2) / 30 + 1 + rng.below(c2.len() / 40);
        match rng.below(3) {
            0 => c2[pos] = gen_char(),
            1 => c2.insert(pos, gen_char()),
            _ => {
                c2.remove(pos);
            }
        }
    }
    (chars.into_iter().collect(), c2.into_iter().collect())
}

/// Two identical ~100KB texts (the no-change fast path).
pub fn pair_identical() -> (String, String) {
    let t = prose(18_000, 110);
    (t.clone(), t)
}

/// The legacy bench dataset: multi-line texts sharing large common blocks
/// between distinct lines — exercises line mode, bisect, the cleanups, and
/// patch construction.
pub fn pair_interleaved(blocks: usize) -> (String, String) {
    let mut t1 = String::from("HEAD1\n");
    let mut t2 = String::from("HEAD2\n");
    for b in 0..blocks {
        for l in 0..8 {
            let line = format!("common-block{b}-line{l}\n");
            t1.push_str(&line);
            t2.push_str(&line);
        }
        t1.push_str(&format!("aaa-distinct-{b}\n"));
        t2.push_str(&format!("bbb-distinct-{b}\n"));
    }
    t1.push_str("TAIL1\n");
    t2.push_str("TAIL2\n");
    (t1, t2)
}

/// Disjoint alphabets: no token is shared, so Myers bisect must exhaust its
/// entire D range before concluding "no commonality" — the quadratic floor.
pub fn pair_disjoint(n: usize) -> (String, String) {
    let mut r1 = Rng::new(21);
    let mut r2 = Rng::new(22);
    let t1: String = (0..n)
        .map(|_| (b'a' + r1.below(13) as u8) as char)
        .collect();
    let t2: String = (0..n)
        .map(|_| (b'n' + r2.below(13) as u8) as char)
        .collect();
    (t1, t2)
}

/// Independent random binary strings: ~50% commonality scattered char by
/// char, which drives Myers to a deep D with no long snakes to ride.
pub fn pair_random_binary(n: usize) -> (String, String) {
    let mut r1 = Rng::new(31);
    let mut r2 = Rng::new(32);
    let t1: String = (0..n)
        .map(|_| if r1.below(2) == 0 { 'a' } else { 'b' })
        .collect();
    let t2: String = (0..n)
        .map(|_| if r2.below(2) == 0 { 'a' } else { 'b' })
        .collect();
    (t1, t2)
}

/// 16KB of a 10-char period with edits pinned at both ends and the middle.
/// With a deadline set, half-match's quarter-length seed recurs at every
/// period, so the seed scan degenerates; without one, it's a bisect over
/// highly repetitive text.
pub fn pair_repetitive() -> (String, String) {
    let unit = "abcdefghij";
    let reps = 1_600;
    let t1 = unit.repeat(reps);
    let mut t2 = String::with_capacity(t1.len());
    t2.push_str("zzz");
    t2.push_str(&unit.repeat(reps / 2 - 20));
    t2.push_str("MIDDLEBLOCKCHANGED");
    t2.push_str(&unit.repeat(reps / 2 - 20));
    t2.push_str("qqq");
    (t1, t2)
}

/// ~22KB prose with a substitution every ~50 chars: nearly every line is
/// touched, so line mode degenerates and the char rediff carries ~450 edits.
pub fn pair_many_small_edits() -> (String, String) {
    let t1 = prose(4_000, 51);
    let mut chars: Vec<char> = t1.chars().collect();
    let mut i = 25;
    while i < chars.len() {
        if chars[i] != '\n' {
            chars[i] = if chars[i] == 'q' { 'z' } else { 'q' };
        }
        i += 50;
    }
    (t1, chars.into_iter().collect())
}

/// ~180KB per side of short unique lines; 250 lines edited, 20 deleted, 20
/// inserted. The diff itself is easy in line space — the cost is the line
/// packing (hashing every line) and rehydration.
pub fn pair_unique_lines() -> (String, String) {
    const N: usize = 4_000;
    let mut rng = Rng::new(61);
    let mut lines: Vec<String> = Vec::with_capacity(N);
    for i in 0..N {
        lines.push(format!(
            "row-{i:04} {} {} value={}\n",
            rng.pick(WORDS),
            rng.pick(WORDS),
            rng.below(1_000_000)
        ));
    }
    let mut lines2 = lines.clone();
    for k in 0..250 {
        let i = k * N / 250 + rng.below(N / 250);
        lines2[i] = format!(
            "row-{i:04} {} edited value={}\n",
            rng.pick(WORDS),
            rng.below(1_000_000)
        );
    }
    for _ in 0..20 {
        lines2.remove(rng.below(lines2.len()));
    }
    for _ in 0..20 {
        let at = rng.below(lines2.len());
        lines2.insert(
            at,
            format!(
                "inserted-row {} value={}\n",
                rng.pick(WORDS),
                rng.below(1_000_000)
            ),
        );
    }
    (lines.concat(), lines2.concat())
}

/// ~16KB on a single line (no '\n' anywhere) with 20 scattered word edits:
/// checklines=true still routes through line mode, which packs each side
/// into one token and immediately falls back to a char diff of everything.
pub fn pair_one_line_soup() -> (String, String) {
    let t1 = prose(2_800, 71).replace('\n', " ");
    let t2 = scattered_word_edits(&t1, 20, 72);
    (t1, t2)
}

/// [EQ xⁿ, INS xⁿ, EQ xⁿ]: cleanup_semantic_lossless can slide this edit one
/// position at a time across the entire right equality, rebuilding its char
/// vectors at every step — the pass's quadratic worst case.
pub fn diffs_lossless_slide(n: usize) -> Vec<Diff> {
    vec![
        Diff::new(0, "x".repeat(n)),
        Diff::new(1, "x".repeat(n)),
        Diff::new(0, "x".repeat(n)),
    ]
}

/// `runs` × [DEL, INS, EQ] of distinct single chars: nothing merges, so
/// cleanup_merge pays its Vec::insert/remove churn on every triple.
pub fn diffs_merge_churn(runs: usize) -> Vec<Diff> {
    let mut v = Vec::with_capacity(runs * 3);
    for i in 0..runs {
        v.push(Diff::new(-1, ((b'a' + (i % 13) as u8) as char).to_string()));
        v.push(Diff::new(1, ((b'n' + (i % 13) as u8) as char).to_string()));
        v.push(Diff::new(0, ((b'A' + (i % 26) as u8) as char).to_string()));
    }
    v
}

/// The source text shifted by a ~400-char prepend: every patch must be found
/// via match_main at a displaced location.
pub fn shifted_source(t1: &str) -> String {
    let prefix = prose(70, 81);
    format!("{prefix}{t1}")
}

/// Flip one context char inside every other patch so those patches take the
/// imperfect-match path in patch_apply (per-patch diff + xindex) instead of
/// the perfect splice. ASCII sources only (char position == byte position).
pub fn fuzz_patch_contexts(t1: &str, patches: &[Patch]) -> String {
    let mut chars: Vec<char> = t1.chars().collect();
    for p in patches.iter().step_by(2) {
        let at = p.start2 as usize + 2;
        chars[at] = if chars[at] == 'y' { 'w' } else { 'y' };
    }
    chars.into_iter().collect()
}

/// (haystack, pattern, loc) for match_main: a 28-char pattern lifted from the
/// text with one char mutated (so no exact occurrence exists) and an expected
/// location 300 chars before the true position. 300 keeps the fuzzy hit
/// inside the default scoring budget: 1 error/28 + 300/match_distance ≈ 0.34,
/// under the 0.5 threshold.
pub fn match_case() -> (String, String, i32) {
    let hay = prose(9_000, 91);
    let pos = {
        let target = hay.len() * 3 / 5;
        hay[..target].rfind(' ').unwrap() + 1
    };
    let mut pattern: Vec<char> = hay[pos..pos + 28].chars().collect();
    pattern[14] = 'Q';
    (hay, pattern.into_iter().collect(), pos as i32 - 300)
}
