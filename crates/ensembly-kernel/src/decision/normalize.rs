//! Shared multi-view normaliser for the scam-gate.
//!
//! Cue matching and host extraction run on every view. A hit on any view
//! is a hit. Every pass is linear in the input length: no backtracking
//! regex in this module, and the host extractor uses the `regex` crate's
//! guaranteed-linear engine.

use regex::Regex;
use std::sync::OnceLock;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Maximum percent-decode / HTML-entity rounds (view d).
const DECODE_ROUNDS: usize = 3;
const MAX_ENTITY_LEN: usize = 32;

/// Format / zero-width / format characters (Cf and friends).
/// On rustc 1.99, U+200B is neither `is_control()` nor `is_whitespace()`.
pub fn is_format_or_zero_width(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}' // soft hyphen
            | '\u{034F}' // combining grapheme joiner
            | '\u{061C}' // arabic letter mark
            | '\u{180E}' // mongolian vowel separator
            | '\u{200B}'..='\u{200F}' // ZWSP, ZWNJ, ZWJ, LRM, RLM
            | '\u{202A}'..='\u{202E}' // bidi embeddings
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FEFF}' // BOM / ZWNBSP
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{110BD}'
            | '\u{110CD}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0001}'
            | '\u{E0020}'..='\u{E007F}'
    ) || ((c as u32) >= 0xE0100 && (c as u32) <= 0xE01EF)
        || (c.is_control() && !c.is_whitespace())
}

/// Map common Latin lookalikes (Cyrillic, Greek, and a few specials).
pub fn map_lookalike(c: char) -> char {
    match c {
        '\u{0430}' | '\u{03B1}' | '\u{0410}' | '\u{0391}' => 'a', // а, α, А, Α
        '\u{0441}' | '\u{03F2}' | '\u{0421}' => 'c',              // с, ϲ, С
        '\u{0501}' | '\u{0500}' => 'd',                           // ԁ, Ԁ
        '\u{0435}' | '\u{03B5}' | '\u{03F5}' | '\u{0415}' | '\u{0395}' => 'e', // е, ε, ϵ, Е, Ε
        '\u{04BB}' | '\u{04BA}' => 'h',                           // һ, Һ
        '\u{0456}' | '\u{03B9}' | '\u{0131}' | '\u{04CF}' | '\u{0406}' | '\u{0399}' => 'i',
        '\u{043A}' | '\u{03BA}' | '\u{041A}' | '\u{039A}' => 'k', // к, κ, К, Κ
        '\u{043C}' | '\u{041C}' | '\u{039C}' => 'm',              // м, М, Μ
        '\u{03B7}' | '\u{0397}' => 'n',                           // η, Η
        '\u{043E}' | '\u{03BF}' | '\u{041E}' | '\u{039F}' => 'o', // о, ο, О, Ο
        '\u{0440}' | '\u{03C1}' | '\u{0420}' | '\u{03A1}' => 'p', // р, ρ, Р, Ρ
        '\u{051B}' | '\u{051A}' => 'q',                           // ԛ, Ԛ
        '\u{0455}' | '\u{0405}' => 's',                           // ѕ, Ѕ
        '\u{0442}' | '\u{03C4}' | '\u{0422}' | '\u{03A4}' => 't', // т, τ, Т, Τ
        '\u{03C5}' | '\u{03A5}' => 'u',                           // υ, Υ
        '\u{03BD}' | '\u{039D}' => 'v',                           // ν, Ν
        '\u{051D}' | '\u{051C}' => 'w',                           // ԝ, Ԝ
        '\u{0445}' | '\u{03C7}' | '\u{0425}' | '\u{03A7}' => 'x', // х, χ, Х, Χ
        '\u{0443}' | '\u{0423}' => 'y',                           // у, У
        _ => c,
    }
}

/// View (c): NFKC, combining marks removed after NFD, confusables folded to ASCII.
pub fn nfkc_strip_marks_fold(text: &str) -> String {
    let nfkc: String = text.nfkc().collect();
    let stripped: String = nfkc.nfd().filter(|c| !is_combining_mark(*c)).collect();
    stripped.chars().map(map_lookalike).collect()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn percent_decode_round(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h1), Some(h2)) = (from_hex(b[i + 1]), from_hex(b[i + 2])) {
                out.push((h1 << 4) | h2);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn named_entity(name: &str) -> Option<char> {
    match name.to_ascii_lowercase().as_str() {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{00a0}'),
        "tab" => Some('\t'),
        "period" => Some('.'),
        "colon" => Some(':'),
        "comma" => Some(','),
        "sol" => Some('/'),
        "commat" => Some('@'),
        "lpar" => Some('('),
        "rpar" => Some(')'),
        "lsqb" => Some('['),
        "rsqb" => Some(']'),
        "lcub" => Some('{'),
        "rcub" => Some('}'),
        _ => None,
    }
}

fn parse_entity(s: &str, at: usize) -> Option<(char, usize)> {
    let rest = &s[at..];
    let bytes = rest.as_bytes();
    if bytes.first() != Some(&b'&') {
        return None;
    }
    let close = rest[1..]
        .char_indices()
        .take(MAX_ENTITY_LEN)
        .find(|(_, c)| *c == ';')
        .map(|(i, _)| i + 1)?;
    let body = &rest[1..close];
    let consumed = close + 1; // include '&' and ';'
    if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
        if hex.is_empty() || hex.len() > 8 || !hex.bytes().all(|b| from_hex(b).is_some()) {
            return None;
        }
        let n = u32::from_str_radix(hex, 16).ok()?;
        let ch = char::from_u32(n)?;
        return Some((ch, consumed));
    }
    if let Some(dec) = body.strip_prefix('#') {
        if dec.is_empty() || dec.len() > 8 || !dec.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let n = dec.parse::<u32>().ok()?;
        let ch = char::from_u32(n)?;
        return Some((ch, consumed));
    }
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    named_entity(body).map(|ch| (ch, consumed))
}

fn html_decode_round(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s.as_bytes()[i] == b'&' {
            if let Some((ch, n)) = parse_entity(s, i) {
                out.push(ch);
                i += n;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// View (d): up to three rounds of percent-decoding and HTML-entity decoding.
pub fn percent_and_html_decode(text: &str) -> String {
    let mut cur = text.to_string();
    for _ in 0..DECODE_ROUNDS {
        let next = html_decode_round(&percent_decode_round(&cur));
        if next == cur {
            break;
        }
        cur = next;
    }
    cur
}

fn eq_ignore_ascii(chars: &[char], at: usize, pat: &str) -> bool {
    if at >= chars.len() {
        return false;
    }
    for (c, p) in chars[at..].iter().zip(pat.chars()) {
        if !c.eq_ignore_ascii_case(&p) {
            return false;
        }
    }
    // zip stops at the shorter side; require the full pattern
    chars[at..].len() >= pat.chars().count()
}

fn skip_ws_right(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

/// View (e): unwrap bracketed and spelled separators used to hide hosts.
pub fn deobfuscate(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < n {
        // Spelled " dot " / " at " (spaces required).
        if chars[i].is_whitespace() {
            let after = skip_ws_right(&chars, i);
            if eq_ignore_ascii(&chars, after, "dot") {
                let end = after + 3;
                if end < n && chars[end].is_whitespace() {
                    out.push('.');
                    i = skip_ws_right(&chars, end);
                    continue;
                }
            }
            if eq_ignore_ascii(&chars, after, "at") {
                let end = after + 2;
                if end < n && chars[end].is_whitespace() {
                    out.push('@');
                    i = skip_ws_right(&chars, end);
                    continue;
                }
            }
        }

        // Bracketed: [.] (.) {.} and [dot] / [at] (any of [], (), {}), optional surrounding space.
        if let Some((repl, end)) = bracket_sep(&chars, i) {
            while out.ends_with(|c: char| c.is_whitespace()) {
                out.pop();
            }
            out.push(repl);
            i = skip_ws_right(&chars, end);
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }
    out
}

fn bracket_sep(chars: &[char], i: usize) -> Option<(char, usize)> {
    let open = chars[i];
    let close = match open {
        '[' => ']',
        '(' => ')',
        '{' => '}',
        _ => return None,
    };
    // [.] or [dot] or [at]
    if i + 2 < chars.len() && chars[i + 1] == '.' && chars[i + 2] == close {
        return Some(('.', i + 3));
    }
    if eq_ignore_ascii(chars, i + 1, "dot") && i + 4 < chars.len() && chars[i + 4] == close {
        return Some(('.', i + 5));
    }
    if eq_ignore_ascii(chars, i + 1, "at") && i + 3 < chars.len() && chars[i + 3] == close {
        return Some(('@', i + 4));
    }
    None
}

fn collapse_and_lower(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_space = false;
    for c in text.chars() {
        for c in c.to_lowercase() {
            if c.is_whitespace() {
                if !in_space {
                    out.push(' ');
                    in_space = true;
                }
            } else {
                in_space = false;
                out.push(c);
            }
        }
    }
    out.trim().to_string()
}

/// View (a): format and zero-width characters stripped, then whitespace collapsed.
pub fn strip_format(text: &str) -> String {
    let stripped: String = text
        .chars()
        .filter(|c| !is_format_or_zero_width(*c))
        .collect();
    collapse_and_lower(&stripped)
}

/// View (b): format and zero-width characters replaced by a space, then collapsed.
pub fn format_as_space(text: &str) -> String {
    let mapped: String = text
        .chars()
        .map(|c| if is_format_or_zero_width(c) { ' ' } else { c })
        .collect();
    collapse_and_lower(&mapped)
}

/// Every composed view of `text`. A later check treats a hit on any view as a hit.
///
/// Combinations of (d) decode, (e) de-obfuscation, (c) unicode fold, and
/// (a)/(b) format handling. Transforms that cannot change this input are
/// skipped, and identical views are dropped. Linear in `text`.
pub fn views(text: &str) -> Vec<String> {
    let feat = input_features(text);
    let mut out = Vec::with_capacity(16);
    let mut seen = std::collections::HashSet::with_capacity(16);
    let decode_opts: &[bool] = if feat.decode {
        &[false, true]
    } else {
        &[false]
    };
    let deobf_opts: &[bool] = if feat.deobf { &[false, true] } else { &[false] };
    let unicode_opts: &[bool] = if feat.unicode {
        &[false, true]
    } else {
        &[false]
    };
    let format_opts: &[bool] = if feat.format {
        &[false, true]
    } else {
        &[false]
    };
    for &do_decode in decode_opts {
        let decoded = if do_decode {
            percent_and_html_decode(text)
        } else {
            text.to_string()
        };
        for &do_deobf in deobf_opts {
            let deobf = if do_deobf {
                deobfuscate(&decoded)
            } else {
                decoded.clone()
            };
            for &do_unicode in unicode_opts {
                let folded = if do_unicode {
                    nfkc_strip_marks_fold(&deobf)
                } else {
                    deobf.clone()
                };
                for &as_space in format_opts {
                    let view = if as_space {
                        format_as_space(&folded)
                    } else {
                        strip_format(&folded)
                    };
                    if seen.insert(view.clone()) {
                        out.push(view);
                    }
                }
            }
        }
    }
    out
}

struct InputFeatures {
    decode: bool,
    deobf: bool,
    unicode: bool,
    format: bool,
}

fn starts_ignore_ascii(s: &str, word: &str) -> bool {
    let mut it = s.chars();
    for p in word.chars() {
        match it.next() {
            Some(c) if c.eq_ignore_ascii_case(&p) => {}
            _ => return false,
        }
    }
    true
}

fn input_features(s: &str) -> InputFeatures {
    let mut feat = InputFeatures {
        decode: false,
        deobf: false,
        unicode: false,
        format: false,
    };
    let mut prev_ws = true;
    let mut i = 0;
    while i < s.len() {
        let c = s[i..].chars().next().unwrap();
        if c == '%' || c == '&' {
            feat.decode = true;
        }
        if c == '[' || c == '(' || c == '{' {
            feat.deobf = true;
        }
        if is_format_or_zero_width(c) {
            feat.format = true;
        }
        if !c.is_ascii() {
            feat.unicode = true;
        }
        if c.is_whitespace() {
            prev_ws = true;
        } else {
            if prev_ws {
                let rest = &s[i..];
                if starts_ignore_ascii(rest, "dot") {
                    let j = i + 3;
                    if j < s.len() && s[j..].chars().next().is_some_and(|x| x.is_whitespace()) {
                        feat.deobf = true;
                    }
                }
                if starts_ignore_ascii(rest, "at") {
                    let j = i + 2;
                    if j < s.len() && s[j..].chars().next().is_some_and(|x| x.is_whitespace()) {
                        feat.deobf = true;
                    }
                }
            }
            prev_ws = false;
        }
        i += c.len_utf8();
    }
    feat
}

/// Canonical host: decode, unicode-fold, lowercase, no brackets, no trailing dots, no port.
pub fn canonicalize_host(raw: &str) -> String {
    let decoded = percent_and_html_decode(raw);
    let folded = nfkc_strip_marks_fold(&decoded);
    let mut h = folded
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    // lookalikes may already be folded; lowercase again for ASCII flags
    h = h.chars().map(map_lookalike).collect();
    while h.ends_with('.') {
        h.pop();
    }
    if let Some((name, port)) = h.rsplit_once(':') {
        if !name.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            h = name.to_string();
        }
    }
    h
}

pub fn host_from_url(url: &str) -> String {
    let decoded = percent_and_html_decode(url);
    let rest = decoded.split("://").nth(1).unwrap_or(&decoded);
    let authority = rest.split('/').next().unwrap_or("");
    let after_user = authority.split('@').next_back().unwrap_or("");
    canonicalize_host(after_user)
}

pub fn flag_matches(candidate: &str, flag: &str) -> bool {
    let h = canonicalize_host(candidate);
    let f = canonicalize_host(flag);
    !h.is_empty() && !f.is_empty() && (h == f || h.ends_with(&format!(".{f}")))
}

fn host_extract() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| {
        // Linear-time (regex crate). Bounded ASCII label repeats, no nested stars.
        Regex::new(
            r"(?ix)
            (?:https?://|ftps?://)(?:[^/@\s]+@)?([^/\s:?\#]+)
            | \b[A-Z0-9._%+\-]+@([^/\s:?\#]+)
            | \b((?:[A-Z0-9](?:[A-Z0-9\-]{0,61}[A-Z0-9])?\.)+[A-Z]{2,})\.?
            ",
        )
        .expect("host extract regex")
    })
}

/// Hosts mentioned in an already-built view: URL hosts, email hosts, bare domains.
pub fn extract_hosts(text: &str) -> Vec<String> {
    host_extract()
        .captures_iter(text)
        .filter_map(|caps| {
            caps.get(1)
                .or_else(|| caps.get(2))
                .or_else(|| caps.get(3))
                .map(|m| canonicalize_host(m.as_str()))
        })
        .filter(|h| !h.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_keeps_in_word_zwsp_join() {
        assert_eq!(strip_format("W\u{200b}IRE MONEY today"), "wire money today");
    }

    #[test]
    fn space_view_splits_soft_hyphen_between_words() {
        assert_eq!(
            format_as_space("wire\u{00AD}money today"),
            "wire money today"
        );
    }

    #[test]
    fn decode_percent_and_numeric_entity() {
        assert_eq!(
            percent_and_html_decode("ph%69sh&#46;example.test"),
            "phish.example.test"
        );
        assert_eq!(
            percent_and_html_decode("phish&#x2e;example.test"),
            "phish.example.test"
        );
        assert_eq!(
            percent_and_html_decode("user&commat;phish.example.test"),
            "user@phish.example.test"
        );
    }

    #[test]
    fn deobfuscate_dots_and_at() {
        assert_eq!(deobfuscate("phish[.]example.test"), "phish.example.test");
        assert_eq!(deobfuscate("phish(.)example.test"), "phish.example.test");
        assert_eq!(deobfuscate("phish{.}example.test"), "phish.example.test");
        assert_eq!(
            deobfuscate("phish dot example dot test"),
            "phish.example.test"
        );
        assert_eq!(
            deobfuscate("phish [dot] example.test"),
            "phish.example.test"
        );
        assert_eq!(
            deobfuscate("hiring[at]phish.example.test"),
            "hiring@phish.example.test"
        );
        assert_eq!(
            deobfuscate("hiring at phish.example.test"),
            "hiring@phish.example.test"
        );
    }

    #[test]
    fn combining_mark_is_stripped_after_nfd() {
        let s = nfkc_strip_marks_fold("w\u{0069}\u{0301}re");
        assert_eq!(s, "wire");
    }

    #[test]
    fn cyrillic_host_label_folds() {
        let s = nfkc_strip_marks_fold("p\u{0430}hish.example.test");
        assert_eq!(s, "pahish.example.test");
    }
}
