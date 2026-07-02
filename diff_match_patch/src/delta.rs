// Delta wire format: encode (diff_todelta) and decode (diff_from_delta) with
// lengths in Unicode scalars, plus the encodeURI-style escaping shared with
// the patch text format.

use crate::types::{Diff, Dmp};
use percent_encoding::{percent_decode, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use std::fmt;

/// Characters JavaScript's `encodeURI` leaves unescaped (beyond
/// alphanumerics), plus space: both wire formats post-process `%20` back to a
/// raw space, which is equivalent to never escaping it.
const ENCODE_URI_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')')
    .remove(b';')
    .remove(b'/')
    .remove(b'?')
    .remove(b':')
    .remove(b'@')
    .remove(b'&')
    .remove(b'=')
    .remove(b'+')
    .remove(b'$')
    .remove(b',')
    .remove(b'#')
    .remove(b' ');

/// Percent-encode `text` exactly as the oracle's `encodeURI` + `%20`→space.
pub(crate) fn encode_uri(text: &str) -> String {
    utf8_percent_encode(text, ENCODE_URI_SET).collect()
}

#[derive(Debug)]
pub(crate) struct DeltaError(String);

impl fmt::Display for DeltaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// The historic public API takes &Vec/&mut Vec/&String; frozen by the
// drop-in compatibility contract.
#[allow(clippy::ptr_arg)]
impl Dmp {
    /// Crush the diff into an encoded string which describes the operations
    /// required to transform text1 into text2.
    /// E.g. =3\t-2\t+ing  -> Keep 3 chars, delete 2 chars, insert 'ing'.
    /// Operations are tab-separated.  Inserted text is escaped using %xx notation.
    ///
    /// Args:
    /// diffs: Vector of diff object.
    ///
    /// Returns:
    /// Delta text.
    pub fn diff_todelta(&mut self, diffs: &mut Vec<Diff>) -> String {
        let tokens: Vec<String> = diffs
            .iter()
            .map(|diff| match diff.operation {
                1 => format!("+{}", encode_uri(&diff.text)),
                -1 => format!("-{}", diff.text.chars().count()),
                _ => format!("={}", diff.text.chars().count()),
            })
            .collect();
        tokens.join("\t")
    }

    /// Given the original text1, and an encoded string which describes the
    /// operations required to transform text1 into text2, compute the full diff.
    ///
    /// Args:
    /// text1: Source string for the diff.
    /// delta: Delta text.
    ///
    /// Returns:
    /// Vector of diff object.
    ///
    /// Panics on invalid input (malformed escape, bad length, or a delta that
    /// does not consume text1 exactly).
    pub fn diff_from_delta(&mut self, text1: &str, delta: &str) -> Vec<Diff> {
        try_from_delta(text1, delta).unwrap_or_else(|e| panic!("{}", e))
    }
}

pub(crate) fn try_from_delta(text1: &str, delta: &str) -> Result<Vec<Diff>, DeltaError> {
    let chars: Vec<char> = text1.chars().collect();
    let mut diffs: Vec<Diff> = vec![];
    let mut pointer = 0usize;
    for token in delta.split('\t') {
        if token.is_empty() {
            // Blank tokens are ok (from a trailing \t).
            continue;
        }
        // Each token begins with a one character parameter which specifies the
        // operation of this token (delete, insert, equality).
        let mut token_chars = token.chars();
        let op = token_chars.next().unwrap();
        let param = token_chars.as_str();
        match op {
            '+' => {
                let text = percent_decode(param.as_bytes())
                    .decode_utf8()
                    .map_err(|_| {
                        DeltaError(format!("Illegal escape in diff_from_delta: {}", param))
                    })?;
                diffs.push(Diff::new(1, text.to_string()));
            }
            '-' | '=' => {
                let n: usize = param.parse().map_err(|_| {
                    DeltaError(format!("Invalid number in diff_from_delta: {}", param))
                })?;
                if pointer + n > chars.len() {
                    return Err(DeltaError(format!(
                        "Delta length ({}) larger than source text length ({})",
                        pointer + n,
                        chars.len()
                    )));
                }
                let text: String = chars[pointer..pointer + n].iter().collect();
                pointer += n;
                diffs.push(Diff::new(if op == '=' { 0 } else { -1 }, text));
            }
            _ => {
                return Err(DeltaError(format!(
                    "Invalid diff operation in diff_from_delta: {}",
                    token
                )))
            }
        }
    }
    if pointer != chars.len() {
        return Err(DeltaError(format!(
            "Delta length ({}) does not equal source text length ({})",
            pointer,
            chars.len()
        )));
    }
    Ok(diffs)
}
