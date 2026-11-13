//! Shared multi-view normaliser for the scam-gate.
//!
//! Cue matching and host extraction run on every view. A hit on any view
//! is a hit. Every pass is linear in the input length: no backtracking
//! regex in this module, and the host extractor uses the `regex` crate's
//! guaranteed-linear engine.

use super::html5_entities::{lookup_named_entity, MAX_NAMED_ENTITY};
use regex::Regex;
use std::sync::OnceLock;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Maximum percent-decode / HTML-entity rounds for the standalone
/// decode helper (view d). The fixed-point loop uses [`CANON_PASSES`].
const DECODE_ROUNDS: usize = 3;
/// Hard cap on fixed-point canonicalisation passes.
const CANON_PASSES: usize = 4;

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

/// Parse an HTML character reference at `at` (which must be `&`).
///
/// Named references use the full vendored WHATWG HTML5 table
/// (case-sensitive). In text, a known name is accepted with or without a
/// trailing semicolon: that is a fail-closed extension of the official
/// 106 legacy no-semicolon names, so a cue hidden as
/// `wire&ndashmoney today` cannot slip through. Longest prefix wins.
/// Numeric references (`&#8211;` / `&#x2013;`) are accepted without a
/// semicolon when the next character is a non-digit (or the input ends),
/// so `wire&#8211money` is fail-closed too.
fn parse_entity_into(s: &str, at: usize, out: &mut String) -> Option<usize> {
    let rest = &s[at..];
    let bytes = rest.as_bytes();
    if bytes.first() != Some(&b'&') {
        return None;
    }
    // Numeric references: &#123; / &#x1A;, semicolon optional when the
    // next character is not a digit (end of input counts as non-digit).
    if bytes.len() > 2 && bytes[1] == b'#' {
        let hex = bytes[2] == b'x' || bytes[2] == b'X';
        let digits_at = if hex { 3 } else { 2 };
        if digits_at >= bytes.len() {
            return None;
        }
        let mut nlen = 0usize;
        while digits_at + nlen < bytes.len() && nlen < 8 {
            let b = bytes[digits_at + nlen];
            let ok = if hex {
                from_hex(b).is_some()
            } else {
                b.is_ascii_digit()
            };
            if !ok {
                break;
            }
            nlen += 1;
        }
        if nlen == 0 {
            return None;
        }
        let after = digits_at + nlen;
        let has_semi = after < bytes.len() && bytes[after] == b';';
        let digits = &rest[digits_at..after];
        let n = if hex {
            u32::from_str_radix(digits, 16).ok()?
        } else {
            digits.parse().ok()?
        };
        let ch = char::from_u32(n)?;
        out.push(ch);
        return Some(after + usize::from(has_semi));
    }

    // Named references: `&` + alphanumeric name, optional `;`.
    let mut name_len = 0usize;
    let limit = MAX_NAMED_ENTITY.min(bytes.len().saturating_sub(1));
    while name_len < limit && bytes[1 + name_len].is_ascii_alphanumeric() {
        name_len += 1;
    }
    if name_len == 0 {
        return None;
    }
    let name_bytes = &rest[1..1 + name_len];
    let has_semi = 1 + name_len < bytes.len() && bytes[1 + name_len] == b';';
    for n in (1..=name_len).rev() {
        if let Some(repl) = lookup_named_entity(&name_bytes[..n]) {
            out.push_str(repl);
            let consume_semi = has_semi && n == name_len;
            return Some(1 + n + usize::from(consume_semi));
        }
    }
    None
}

fn html_decode_round(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s.as_bytes()[i] == b'&' {
            if let Some(n) = parse_entity_into(s, i, &mut out) {
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

fn canon_size_bound(input_len: usize) -> usize {
    input_len.saturating_mul(2).saturating_add(64)
}

fn strip_format_chars(text: &str) -> String {
    text.chars()
        .filter(|c| !is_format_or_zero_width(*c))
        .collect()
}

fn decode_then_maybe_nfkc(text: &str) -> String {
    let after_html = if text.as_bytes().contains(&b'&') {
        html_decode_round(text)
    } else {
        text.to_string()
    };
    let decoded = if after_html.as_bytes().contains(&b'%') {
        percent_decode_round(&after_html)
    } else {
        after_html
    };
    // NFKC is a no-op on ASCII and is the expensive part of a 1 MB pass.
    if decoded.is_ascii() {
        decoded
    } else {
        nfkc_strip_marks_fold(&decoded)
    }
}

/// One decode step: strip format/zero-width characters, decode HTML
/// named and numeric references, percent-decode, then unicode-fold.
fn canonicalize_decode_pass(text: &str) -> String {
    if text.chars().any(is_format_or_zero_width) {
        decode_then_maybe_nfkc(&strip_format_chars(text))
    } else {
        decode_then_maybe_nfkc(text)
    }
}

struct CanonForms {
    /// Strip + HTML-decode + percent-decode + unicode fold. Non-ASCII
    /// dashes are kept so a decoded en-dash can still stand in for a
    /// host label separator.
    decoded: String,
    /// `decoded` after separator class folds (Pd and extra dashes to
    /// `-`, Zs to space, slash and period lookalikes to ASCII).
    folded: String,
}

/// Fixed-point canonicalisation for the scam-gate.
///
/// A single left-to-right pass cannot cover every interleaving of hiding
/// tricks: a format character can split a named entity (`&n` + ZWSP +
/// `dash;`), a percent-encoded octet can hide a `&` or a format
/// character, and an entity can hide a `%` or another entity. Each pass
/// therefore strips format and zero-width characters, decodes HTML
/// named and numeric character references, percent-decodes, and applies
/// the unicode and separator class folds. The loop repeats until a pass
/// makes no change, or until [`CANON_PASSES`], and it stops if a pass
/// would exceed a size bound, so a pathological input cannot expand
/// without limit or run forever. Views are derived from this fixed
/// point and from the raw text, so a cheap reordering cannot skip a fold.
fn canonicalize_forms(text: &str) -> CanonForms {
    let bound = canon_size_bound(text.len());
    let mut cur = text.to_string();
    let mut decoded = cur.clone();
    let mut folded = fold_separators(&decoded);
    for _ in 0..CANON_PASSES {
        let next = canonicalize_decode_pass(&cur);
        if next.len() > bound {
            break;
        }
        let next_folded = fold_separators(&next);
        if next_folded.len() > bound {
            break;
        }
        decoded = next;
        folded = next_folded;
        if decoded == cur {
            break;
        }
        cur = decoded.clone();
    }
    CanonForms { decoded, folded }
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
    let close = match chars[i] {
        '[' => ']',
        '(' => ')',
        '{' => '}',
        _ => return None,
    };
    // Tolerate spaces inside brackets: "[.]", "[ dot ]", "( . )".
    let mut j = skip_ws_right(chars, i + 1);
    if j >= chars.len() {
        return None;
    }
    if chars[j] == '.' {
        j = skip_ws_right(chars, j + 1);
        if j < chars.len() && chars[j] == close {
            return Some(('.', j + 1));
        }
        return None;
    }
    if eq_ignore_ascii(chars, j, "dot") {
        j = skip_ws_right(chars, j + 3);
        if j < chars.len() && chars[j] == close {
            return Some(('.', j + 1));
        }
        return None;
    }
    if eq_ignore_ascii(chars, j, "at") {
        j = skip_ws_right(chars, j + 2);
        if j < chars.len() && chars[j] == close {
            return Some(('@', j + 1));
        }
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

/// Map IDN / fullwidth / halfwidth stops and period lookalikes to ASCII '.'.
pub fn fold_idn_dots(text: &str) -> String {
    text.chars().map(fold_dot_char).collect()
}

fn fold_dot_char(c: char) -> char {
    if is_period_lookalike(c) {
        '.'
    } else {
        c
    }
}

/// Unicode 15.1 Dash_Punctuation (Pd). ASCII hyphen-minus is included.
pub fn is_dash_punctuation(c: char) -> bool {
    matches!(
        c,
        '\u{002D}' | '\u{058A}' | '\u{05BE}' | '\u{1400}' | '\u{1806}' | '\u{2010}'
            ..='\u{2015}'
                | '\u{2E17}'
                | '\u{2E1A}'
                | '\u{2E3A}'
                | '\u{2E3B}'
                | '\u{2E40}'
                | '\u{2E5D}'
                | '\u{301C}'
                | '\u{3030}'
                | '\u{30A0}'
                | '\u{FE31}'
                | '\u{FE32}'
                | '\u{FE58}'
                | '\u{FE63}'
                | '\u{FF0D}'
                | '\u{10EAD}'
    )
}

/// Minus and hyphen bullet. Small and fullwidth hyphens (U+FE63, U+FF0D)
/// are Dash_Punctuation and are already matched by [`is_dash_punctuation`].
pub fn is_extra_dash(c: char) -> bool {
    matches!(c, '\u{2212}' | '\u{2043}')
}

pub fn is_dash_lookalike(c: char) -> bool {
    is_dash_punctuation(c) || is_extra_dash(c)
}

pub fn is_non_ascii_dash(c: char) -> bool {
    c != '-' && is_dash_lookalike(c)
}

/// Unicode 15.1 Space_Separator (Zs).
pub fn is_space_separator(c: char) -> bool {
    matches!(
        c,
        '\u{0020}' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

pub fn is_slash_lookalike(c: char) -> bool {
    matches!(c, '\u{2044}' | '\u{2215}' | '\u{FF0F}')
}

pub fn is_period_lookalike(c: char) -> bool {
    matches!(
        c,
        '\u{2024}' | '\u{FE52}' | '\u{3002}' | '\u{FF61}' | '\u{FF0E}'
    )
}

/// Every code point in the separator classes the gate folds (unique, stable order).
#[cfg(test)]
pub fn separator_class_codepoints() -> &'static [char] {
    const CPS: &[char] = &[
        // Pd
        '\u{002D}',
        '\u{058A}',
        '\u{05BE}',
        '\u{1400}',
        '\u{1806}',
        '\u{2010}',
        '\u{2011}',
        '\u{2012}',
        '\u{2013}',
        '\u{2014}',
        '\u{2015}',
        '\u{2E17}',
        '\u{2E1A}',
        '\u{2E3A}',
        '\u{2E3B}',
        '\u{2E40}',
        '\u{2E5D}',
        '\u{301C}',
        '\u{3030}',
        '\u{30A0}',
        '\u{FE31}',
        '\u{FE32}',
        '\u{FE58}',
        '\u{FE63}',
        '\u{FF0D}',
        '\u{10EAD}',
        // extra dashes
        '\u{2212}',
        '\u{2043}',
        // Zs
        '\u{0020}',
        '\u{00A0}',
        '\u{1680}',
        '\u{2000}',
        '\u{2001}',
        '\u{2002}',
        '\u{2003}',
        '\u{2004}',
        '\u{2005}',
        '\u{2006}',
        '\u{2007}',
        '\u{2008}',
        '\u{2009}',
        '\u{200A}',
        '\u{202F}',
        '\u{205F}',
        '\u{3000}',
        // slash lookalikes
        '\u{2044}',
        '\u{2215}',
        '\u{FF0F}',
        // period lookalikes (including IDN dots)
        '\u{2024}',
        '\u{FE52}',
        '\u{3002}',
        '\u{FF61}',
        '\u{FF0E}',
    ];
    CPS
}

/// Characters the gate folds in cue or host views: Pd and extra dashes,
/// Zs spaces, slash and period lookalikes, format / zero-width characters,
/// and the ASCII `.` and `/` that punct_and_join treats as join punctuation.
/// ASCII `:` and `@` are decoded as named entities but are not separator folds.
#[cfg(test)]
pub fn is_folded_class_char(c: char) -> bool {
    is_dash_lookalike(c)
        || is_space_separator(c)
        || is_slash_lookalike(c)
        || is_period_lookalike(c)
        || is_format_or_zero_width(c)
        || matches!(c, '.' | '/')
}

#[cfg(test)]
pub fn html5_named_entities() -> &'static [(&'static str, &'static str)] {
    super::html5_entities::HTML5_NAMED_ENTITIES
}

/// Every format / zero-width code point the gate strips: the named
/// ranges in [`is_format_or_zero_width`], plus C0/C1 controls that are
/// not whitespace. Used by the ZWSP-split entity property test.
#[cfg(test)]
pub fn format_class_codepoints() -> Vec<char> {
    let mut out = Vec::new();
    let mut push_range = |lo: u32, hi: u32| {
        for o in lo..=hi {
            if let Some(c) = char::from_u32(o) {
                if is_format_or_zero_width(c) {
                    out.push(c);
                }
            }
        }
    };
    for o in [
        0x00AD, 0x034F, 0x061C, 0x180E, 0xFEFF, 0x110BD, 0x110CD, 0xE0001,
    ] {
        push_range(o, o);
    }
    push_range(0x200B, 0x200F);
    push_range(0x202A, 0x202E);
    push_range(0x2060, 0x2064);
    push_range(0x2066, 0x206F);
    push_range(0xFFF9, 0xFFFB);
    push_range(0x1BCA0, 0x1BCA3);
    push_range(0x1D173, 0x1D17A);
    push_range(0xE0020, 0xE007F);
    push_range(0xE0100, 0xE01EF);
    // C0 / DEL / C1 controls that are not whitespace
    push_range(0x0000, 0x001F);
    push_range(0x007F, 0x009F);
    out.sort_unstable();
    out.dedup();
    out
}

/// Cue-view separator fold: Pd + extra dashes → '-', Zs → ' ', slash/period lookalikes → ASCII.
pub fn fold_separators(text: &str) -> String {
    text.chars()
        .map(|c| {
            // ASCII '-' is Dash_Punctuation and stays '-'.
            if is_dash_lookalike(c) {
                '-'
            } else if is_space_separator(c) {
                ' '
            } else if is_slash_lookalike(c) {
                '/'
            } else if is_period_lookalike(c) {
                '.'
            } else {
                c
            }
        })
        .collect()
}

fn is_host_label_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// NON-ASCII dashes between labels become '.'. ASCII '-' is never changed.
pub fn fold_non_ascii_dashes_between_labels_to_dots(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if is_non_ascii_dash(chars[i]) {
            let start = i;
            while i < chars.len() && is_non_ascii_dash(chars[i]) {
                i += 1;
            }
            let prev_label = start > 0 && is_host_label_char(chars[start - 1]);
            let next_label = i < chars.len() && is_host_label_char(chars[i]);
            if prev_label && next_label {
                out.push('.');
            } else {
                for _ in start..i {
                    out.push('-');
                }
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn fold_non_ascii_dashes_to_hyphen(text: &str) -> String {
    text.chars()
        .map(|c| if is_non_ascii_dash(c) { '-' } else { c })
        .collect()
}

fn has_non_ascii_dash_or_dot(text: &str) -> bool {
    text.chars()
        .any(|c| is_non_ascii_dash(c) || is_period_lookalike(c))
}

/// Strip simple `<...>` tags, replacing each tag with a space (linear, bounded look-ahead).
pub fn strip_html_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < text.len() {
        if bytes[i] == b'<' {
            let limit = (i + 1 + 256).min(text.len());
            if let Some(rel) = text[i + 1..limit].find('>') {
                out.push(' ');
                i = i + 1 + rel + 1;
                continue;
            }
        }
        let c = text[i..].chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
    }
    out
}

fn is_join_punct(c: char) -> bool {
    matches!(c, '-' | '.' | '/' | '_')
}

/// Turn `-` `.` `/` `_` next to letters into a space, then join single-letter runs
/// (`w.i.r.e` becomes `wire`, `wire-money` becomes `wire money`).
pub fn punct_and_join(text: &str) -> String {
    let text = fold_separators(text);
    let chars: Vec<char> = text.chars().collect();
    let mut spaced = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        if is_join_punct(c) {
            let prev_letter = i > 0 && chars[i - 1].is_alphabetic();
            let next_letter = chars.get(i + 1).is_some_and(|n| n.is_alphabetic());
            if prev_letter || next_letter {
                spaced.push(' ');
                continue;
            }
        }
        spaced.push(c);
    }
    join_single_letter_runs(&spaced)
}

fn join_single_letter_runs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let flush_run = |run: &mut String, out: &mut String| {
        if run.is_empty() {
            return;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(run);
        run.clear();
    };
    for tok in text.split_whitespace() {
        if tok.chars().count() == 1 && tok.chars().next().is_some_and(|c| c.is_alphabetic()) {
            run.push_str(tok);
        } else {
            flush_run(&mut run, &mut out);
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(tok);
        }
    }
    flush_run(&mut run, &mut out);
    out
}

/// Small leet map, used only in a separate cue view: 1>i 0>o 3>e 4>a 5>s 7>t.
pub fn leet_fold(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '1' => 'i',
            '0' => 'o',
            '3' => 'e',
            '4' => 'a',
            '5' => 's',
            '7' => 't',
            _ => c,
        })
        .collect()
}

fn has_leet_digit(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c, '0' | '1' | '3' | '4' | '5' | '7'))
}

/// Cue-only views: shared host/cue views plus HTML-stripped, punctuation-join,
/// and a separate leet view. A hit on any view is a hit.
#[allow(dead_code)]
pub fn cue_views(text: &str) -> Vec<String> {
    extend_cue_views(text, views(text))
}

/// Extend already-built host/cue views with cue-only folds, without
/// running [`views`] again.
pub fn extend_cue_views(text: &str, mut out: Vec<String>) -> Vec<String> {
    let mut seen: std::collections::HashSet<String> = out.iter().cloned().collect();
    let push = |s: String, out: &mut Vec<String>, seen: &mut std::collections::HashSet<String>| {
        if seen.insert(s.clone()) {
            out.push(s);
        }
    };

    if text.contains('<') {
        for v in views(&strip_html_tags(text)) {
            push(v, &mut out, &mut seen);
        }
        let existing = out.clone();
        for v in existing {
            if v.contains('<') {
                push(
                    collapse_and_lower(&strip_html_tags(&v)),
                    &mut out,
                    &mut seen,
                );
            }
        }
    }

    let base = out.clone();
    for v in &base {
        push(punct_and_join(v), &mut out, &mut seen);
    }

    let mid = out.clone();
    for v in &mid {
        if has_leet_digit(v) {
            push(join_single_letter_runs(&leet_fold(v)), &mut out, &mut seen);
        }
    }
    out
}

/// Every composed view of `text`. A later check treats a hit on any view as a hit.
///
/// Views are built from the raw text (as before) and from the
/// fixed-point canonicalisation of that text, so a format character
/// inside a name, a percent-encoded `&`, or a nested entity still
/// reaches the class folds. Combinations of (d) decode, (e)
/// de-obfuscation, (c) unicode fold, and (a)/(b) format handling.
/// Transforms that cannot change this input are skipped, and identical
/// views are dropped. Linear in `text`.
pub fn views(text: &str) -> Vec<String> {
    let mut out = compose_views(text);
    let mut seen: std::collections::HashSet<String> = out.iter().cloned().collect();
    let forms = canonicalize_forms(text);
    for src in [&forms.decoded, &forms.folded] {
        if src.as_str() == text {
            continue;
        }
        for v in compose_views(src) {
            if seen.insert(v.clone()) {
                out.push(v);
            }
        }
    }
    out
}

fn compose_views(text: &str) -> Vec<String> {
    let feat = input_features(text);
    let mut out = Vec::with_capacity(16);
    let mut seen = std::collections::HashSet::with_capacity(16);
    let decode_opts: &[bool] = if feat.decode {
        &[false, true]
    } else {
        &[false]
    };
    let deobf_opts: &[bool] = if feat.deobf { &[false, true] } else { &[false] };
    for &do_decode in decode_opts {
        let decoded = if do_decode {
            percent_and_html_decode(text)
        } else {
            text.to_string()
        };
        // Decode can introduce format characters and non-ASCII (named
        // entities are ASCII in the source). Re-check after each
        // transform so the decoded text still goes through folding.
        for &do_deobf in deobf_opts {
            let deobf = if do_deobf {
                deobfuscate(&decoded)
            } else {
                decoded.clone()
            };
            let need_unicode = feat.unicode || !deobf.is_ascii();
            let unicode_opts: &[bool] = if need_unicode {
                &[false, true]
            } else {
                &[false]
            };
            for &do_unicode in unicode_opts {
                let folded = if do_unicode {
                    nfkc_strip_marks_fold(&deobf)
                } else {
                    deobf.clone()
                };
                let need_format = feat.format || folded.chars().any(is_format_or_zero_width);
                let format_opts: &[bool] = if need_format {
                    &[false, true]
                } else {
                    &[false]
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

/// Canonical host (hyphen-preserving view): decode, IDN/period lookalikes to
/// ASCII '.', non-ASCII dashes to ASCII '-', punycode, unicode-fold, lowercase,
/// no brackets, no trailing dots, no port. ASCII '-' is never turned into '.'.
pub fn canonicalize_host(raw: &str) -> String {
    finish_host(&fold_non_ascii_dashes_to_hyphen(&host_prepare(raw)))
}

/// Host views used against the flag list. The hyphen view always exists.
/// If the host contains a non-ASCII dash or dot, a second view treats
/// non-ASCII dashes between labels as '.'. ASCII '-' is never turned into '.'.
pub fn canonicalize_host_views(raw: &str) -> Vec<String> {
    let prepared = host_prepare(raw);
    let hyphen = canonicalize_host(raw);
    let mut out = Vec::with_capacity(2);
    if !hyphen.is_empty() {
        out.push(hyphen);
    }
    if has_non_ascii_dash_or_dot(&prepared) {
        let dotted = finish_host(&fold_non_ascii_dashes_between_labels_to_dots(&prepared));
        if !dotted.is_empty() && !out.iter().any(|h| h == &dotted) {
            out.push(dotted);
        }
    }
    out
}

fn host_prepare(raw: &str) -> String {
    let decoded = canonicalize_forms(raw).decoded;
    fold_idn_dots(&decoded)
}

fn finish_host(prepared: &str) -> String {
    let puny = super::punycode::decode_host(prepared);
    let folded = nfkc_strip_marks_fold(&puny);
    let mut h = folded
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    h = h.chars().map(map_lookalike).map(fold_dot_char).collect();
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

fn authority_host(url: &str) -> String {
    let decoded = percent_and_html_decode(url);
    let dotted = fold_idn_dots(&decoded);
    let rest = dotted.split("://").nth(1).unwrap_or(dotted.as_str());
    let authority = rest.split('/').next().unwrap_or("");
    authority.split('@').next_back().unwrap_or("").to_string()
}

pub fn host_from_url_views(url: &str) -> Vec<String> {
    canonicalize_host_views(&authority_host(url))
}

pub fn flag_matches(candidate: &str, flag: &str) -> bool {
    let flags = canonicalize_host_views(flag);
    canonicalize_host_views(candidate).iter().any(|h| {
        flags
            .iter()
            .any(|f| !h.is_empty() && !f.is_empty() && (h == f || h.ends_with(&format!(".{f}"))))
    })
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
/// IDN / period lookalikes fold to ASCII '.' before the extractor runs. A second
/// pass treats non-ASCII dashes between labels as '.', so a cheap dash-for-dot
/// swap cannot hide a flagged host. Each capture is then expanded to both
/// hyphen and dash-as-dot canonical views. ASCII '-' is never turned into '.'.
pub fn extract_hosts(text: &str) -> Vec<String> {
    let dotted = fold_idn_dots(text);
    let mut out = extract_hosts_from(&dotted);
    if dotted.chars().any(is_non_ascii_dash) {
        let as_dots = fold_non_ascii_dashes_between_labels_to_dots(&dotted);
        out.extend(extract_hosts_from(&as_dots));
    }
    out
}

fn extract_hosts_from(dotted: &str) -> Vec<String> {
    host_extract()
        .captures_iter(dotted)
        .flat_map(|caps| {
            caps.get(1)
                .or_else(|| caps.get(2))
                .or_else(|| caps.get(3))
                .map(|m| canonicalize_host_views(m.as_str()))
                .into_iter()
                .flatten()
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
    fn decode_whatwg_named_entities_in_folded_classes() {
        assert_eq!(
            percent_and_html_decode("wire&ndash;money"),
            "wire\u{2013}money"
        );
        assert_eq!(
            percent_and_html_decode("wire&mdash;money"),
            "wire\u{2014}money"
        );
        assert_eq!(
            percent_and_html_decode("wire&minus;money"),
            "wire\u{2212}money"
        );
        assert_eq!(
            percent_and_html_decode("wire&hyphen;money"),
            "wire\u{2010}money"
        );
        assert_eq!(
            percent_and_html_decode("wire&nbsp;money"),
            "wire\u{00a0}money"
        );
        // Fail-closed: known names are accepted without a trailing semicolon.
        assert_eq!(
            percent_and_html_decode("wire&ndashmoney"),
            "wire\u{2013}money"
        );
        assert_eq!(
            percent_and_html_decode("wire&nbspmoney"),
            "wire\u{00a0}money"
        );
        assert_eq!(percent_and_html_decode("a&colon;b"), "a:b");
        assert_eq!(percent_and_html_decode("a&sol;b"), "a/b");
        assert_eq!(percent_and_html_decode("a&zwj;b"), "a\u{200d}b");
        assert_eq!(percent_and_html_decode("a&zwnj;b"), "a\u{200c}b");
        assert_eq!(percent_and_html_decode("a&shy;b"), "a\u{00ad}b");
        // Unknown names are left intact.
        assert_eq!(percent_and_html_decode("keep&zzzzz;it"), "keep&zzzzz;it");
        // Longest official match: notin, not the shorter legacy not.
        assert_eq!(percent_and_html_decode("&notin;"), "\u{2209}");
        // Numeric without a semicolon, when followed by a non-digit.
        assert_eq!(
            percent_and_html_decode("wire&#8211money"),
            "wire\u{2013}money"
        );
        assert_eq!(
            percent_and_html_decode("wire&#x2013money"),
            "wire\u{2013}money"
        );
        // Case-sensitive: &NDASH; is not a WHATWG name.
        assert_eq!(
            percent_and_html_decode("wire&NDASH;money"),
            "wire&NDASH;money"
        );
    }

    #[test]
    fn canonicalize_reunites_format_split_and_nested_encodings() {
        let forms = canonicalize_forms("wire&n\u{200b}dash;money today");
        assert_eq!(forms.folded, "wire-money today");
        let forms = canonicalize_forms("wire&\u{200b}ndash;money today");
        assert_eq!(forms.folded, "wire-money today");
        let forms = canonicalize_forms("wire%26ndash%3Bmoney today");
        assert_eq!(forms.folded, "wire-money today");
        let forms = canonicalize_forms("wire&n%E2%80%8Bdash;money today");
        assert_eq!(forms.folded, "wire-money today");
        let forms = canonicalize_forms("wire&amp;ndash;money today");
        assert_eq!(forms.folded, "wire-money today");
        let forms = canonicalize_forms("wire&#8211money today");
        assert_eq!(forms.folded, "wire-money today");
        // Uppercase is not a name; the fixed point must not lower it.
        let forms = canonicalize_forms("wire&NDASH;money today");
        assert!(
            forms.folded.contains("&NDASH;") || forms.decoded.contains("&NDASH;"),
            "case-sensitive: &NDASH; must not become an en-dash, got folded={:?} decoded={:?}",
            forms.folded,
            forms.decoded
        );
    }

    #[test]
    fn html5_named_entity_table_is_sorted_and_covers_folded_names() {
        let table = html5_named_entities();
        for pair in table.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "table must stay sorted for binary search"
            );
        }
        for name in [
            "ndash", "mdash", "minus", "hyphen", "dash", "horbar", "hybull", "nbsp", "ensp",
            "emsp", "thinsp", "hairsp", "period", "sol", "frasl", "colon", "commat", "zwj", "zwnj",
            "shy", "Tab",
        ] {
            assert!(
                lookup_named_entity(name).is_some(),
                "expected {name} in the WHATWG table"
            );
        }
        // Names are case-sensitive, as in the standard.
        assert!(lookup_named_entity("nbsp").is_some());
        assert!(lookup_named_entity("NBSP").is_none());
    }

    #[test]
    fn extra_dash_does_not_repeat_pd_codepoints() {
        for c in ['\u{FE63}', '\u{FF0D}'] {
            assert!(is_dash_punctuation(c), "U+{:04X} is Pd", c as u32);
            assert!(
                !is_extra_dash(c),
                "U+{:04X} must not be duplicated in is_extra_dash",
                c as u32
            );
            assert!(is_dash_lookalike(c));
        }
        assert!(is_extra_dash('\u{2212}'));
        assert!(is_extra_dash('\u{2043}'));
        assert!(!is_dash_punctuation('\u{2212}'));
        assert!(!is_dash_punctuation('\u{2043}'));
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

    #[test]
    fn idn_dots_fold_to_ascii_period() {
        assert_eq!(
            fold_idn_dots("phish\u{3002}example\u{3002}test"),
            "phish.example.test"
        );
        assert_eq!(
            fold_idn_dots("phish\u{FF61}example\u{FF61}test"),
            "phish.example.test"
        );
        assert_eq!(
            fold_idn_dots("phish\u{FF0E}example\u{FF0E}test"),
            "phish.example.test"
        );
        assert_eq!(
            canonicalize_host("phish\u{3002}example\u{3002}test"),
            "phish.example.test"
        );
    }

    #[test]
    fn spaced_brackets_deobfuscate() {
        assert_eq!(
            deobfuscate("phish [ dot ] example.test"),
            "phish.example.test"
        );
        assert_eq!(
            deobfuscate("phish ( . ) example.test"),
            "phish.example.test"
        );
    }

    #[test]
    fn punct_join_and_leet() {
        assert_eq!(punct_and_join("w.i.r.e money today"), "wire money today");
        assert_eq!(punct_and_join("wire-money today"), "wire money today");
        assert_eq!(punct_and_join("wire/money today"), "wire money today");
        assert_eq!(leet_fold("w1re m0ney"), "wire money");
    }

    #[test]
    fn html_tags_are_stripped() {
        assert_eq!(
            collapse_and_lower(&strip_html_tags("wire <b>money</b> today")),
            "wire money today"
        );
    }

    #[test]
    fn unicode_dashes_fold_before_punct_join() {
        assert_eq!(
            punct_and_join("wire\u{2011}money today"),
            "wire money today"
        );
        assert_eq!(
            punct_and_join("wire\u{2013}money today"),
            "wire money today"
        );
        assert_eq!(
            punct_and_join("wire\u{2212}money today"),
            "wire money today"
        );
        assert_eq!(
            punct_and_join("wire\u{2044}money today"),
            "wire money today"
        );
        assert_eq!(
            punct_and_join("w\u{2024}i\u{2024}r\u{2024}e money today"),
            "wire money today"
        );
    }

    #[test]
    fn ascii_hyphen_host_is_not_turned_into_dots() {
        let views = canonicalize_host_views("phish-example.test");
        assert_eq!(views, vec!["phish-example.test".to_string()]);
        assert!(!flag_matches("phish-example.test", "phish.example.test"));
    }

    #[test]
    fn non_ascii_dash_host_has_hyphen_and_dot_views() {
        let views = canonicalize_host_views("phish\u{2011}example.test");
        assert!(views.contains(&"phish-example.test".to_string()));
        assert!(views.contains(&"phish.example.test".to_string()));
        assert!(flag_matches(
            "phish\u{2011}example.test",
            "phish.example.test"
        ));
        // hyphenated flag still matches when written with a unicode dash
        assert!(flag_matches(
            "wire\u{2013}now.example.test",
            "wire-now.example.test"
        ));
    }
}
