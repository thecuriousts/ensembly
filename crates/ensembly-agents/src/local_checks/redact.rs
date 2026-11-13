//! Fail-closed cloud redaction gate.
//!
//! If any pattern matches, the cloud call is refused. Text is never partly
//! redacted and then sent. Detectors run on several views of the input: the
//! raw text; a Unicode-folded form (NFKC, format/zero-width characters
//! stripped, confusables mapped to ASCII); repeated percent and HTML-entity
//! decoding; whitespace collapsed around `@./:-` (and removed entirely when
//! matching token prefixes); and UTF-8 payloads of base64 / base64url runs.
//! A hit on any view refuses the call.
//!
//! Tests build adversarial samples at runtime so this file does not embed
//! phone numbers, civic numbers, tokens, or home paths.

/// Keys treated as name-like when the input is JSON or `key=value` / `"key":`.
pub const NAME_LIKE_FIELDS: &[&str] = &[
    "name",
    "full_name",
    "first_name",
    "last_name",
    "given_name",
    "family_name",
    "surname",
    "display_name",
    "legal_name",
    "person_name",
    "customer_name",
    "real_name",
];

/// Patterns that must never ride a cloud fallback. Synthetic fixtures avoid them.
pub fn cloud_redaction_hits(text: &str) -> Vec<&'static str> {
    let mut hits = Vec::new();
    let (general, token_only) = expand_views(text);
    for view in &general {
        merge_hits(&mut hits, hits_in(view));
    }
    // The all-whitespace-stripped form is only used for token prefixes, so a
    // hyphenated sentence is not treated as one high-entropy secret.
    for view in &token_only {
        merge_hits(&mut hits, token_prefix_hits(view));
    }
    hits
}

fn token_prefix_hits(text: &str) -> Vec<&'static str> {
    let mut hits = Vec::new();
    if looks_like_api_token(text) {
        hits.push("api-token");
    }
    if looks_like_bearer(text) {
        hits.push("bearer-token");
    }
    hits
}

fn hits_in(text: &str) -> Vec<&'static str> {
    let mut hits = Vec::new();
    if looks_like_api_token(text) {
        hits.push("api-token");
    }
    if looks_like_bearer(text) {
        hits.push("bearer-token");
    }
    if looks_like_jwt(text) {
        hits.push("jwt");
    }
    if looks_like_high_entropy_secret(text) {
        hits.push("high-entropy");
    }
    match swedish_civic_kind(text) {
        Some(CivicKind::Ordinary) => hits.push("civic-number"),
        Some(CivicKind::Coordination) => hits.push("coordination-number"),
        None => {}
    }
    if looks_like_phone(text) {
        hits.push("phone");
    }
    if looks_like_email(text) {
        hits.push("email");
    }
    if looks_like_home_path(text) {
        hits.push("home-path");
    }
    if looks_like_ip(text) {
        hits.push("ip-address");
    }
    if looks_like_mac(text) {
        hits.push("mac-address");
    }
    if looks_like_hostname(text) {
        hits.push("hostname");
    }
    if looks_like_name_field(text) {
        hits.push("name-field");
    }
    hits
}

fn merge_hits(into: &mut Vec<&'static str>, extra: Vec<&'static str>) {
    for hit in extra {
        if !into.contains(&hit) {
            into.push(hit);
        }
    }
}

const MAX_VIEWS: usize = 256;
const MAX_B64_RUNS: usize = 32;
const MAX_B64_DECODED: usize = 64 * 1024;

/// Build the detector views. Order is stable; duplicates are dropped.
fn expand_views(text: &str) -> (Vec<String>, Vec<String>) {
    let mut views: Vec<String> = Vec::new();
    let push = |s: String, views: &mut Vec<String>| {
        if s.is_empty() || views.iter().any(|v| v == &s) || views.len() >= MAX_VIEWS {
            return;
        }
        views.push(s);
    };

    push(text.to_string(), &mut views);
    let folded = fold_unicode(text);
    push(folded.clone(), &mut views);

    // Repeated percent-decoding (up to 3 rounds) and HTML-entity decoding,
    // in both orders, starting from the raw and folded text.
    for start in [text.to_string(), folded] {
        for order in [DecodeOrder::PercentThenHtml, DecodeOrder::HtmlThenPercent] {
            let mut cur = start.clone();
            for _ in 0..3 {
                let next = match order {
                    DecodeOrder::PercentThenHtml => html_decode(&percent_decode(&cur)),
                    DecodeOrder::HtmlThenPercent => percent_decode(&html_decode(&cur)),
                };
                if next == cur {
                    break;
                }
                push(next.clone(), &mut views);
                push(fold_unicode(&next), &mut views);
                cur = next;
            }
        }
    }

    // Base64 runs are taken from views that still have natural spacing, not
    // from the all-whitespace-stripped form, so glued English is not decoded.
    let encoded_sources = views.clone();

    let current = views.clone();
    for v in &current {
        push(strip_ws_around_punct(v), &mut views);
    }

    let mut token_only: Vec<String> = Vec::new();
    for v in &views {
        push(strip_all_ws(v), &mut token_only);
    }

    let mut decoded_n = 0usize;
    for v in &encoded_sources {
        for decoded in decode_base64_runs(v) {
            if decoded_n >= MAX_B64_RUNS {
                break;
            }
            decoded_n += 1;
            push(decoded.clone(), &mut views);
            let folded = fold_unicode(&decoded);
            push(folded.clone(), &mut views);
            push(strip_ws_around_punct(&decoded), &mut views);
            push(strip_all_ws(&decoded), &mut views);
            let mut cur = decoded;
            for _ in 0..3 {
                let next = html_decode(&percent_decode(&cur));
                if next == cur {
                    break;
                }
                push(next.clone(), &mut views);
                push(fold_unicode(&next), &mut views);
                push(strip_ws_around_punct(&next), &mut views);
                cur = next;
            }
        }
    }
    for v in &views {
        push(strip_all_ws(v), &mut token_only);
    }
    (views, token_only)
}

#[derive(Clone, Copy)]
enum DecodeOrder {
    PercentThenHtml,
    HtmlThenPercent,
}

fn fold_unicode(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let stripped: String = text
        .chars()
        .filter(|c| !is_format_or_zero_width(*c))
        .collect();
    let nfkc: String = stripped.nfkc().collect();
    let again: String = nfkc
        .chars()
        .filter(|c| !is_format_or_zero_width(*c))
        .collect();
    again.chars().map(fold_confusable).collect()
}

fn is_format_or_zero_width(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
    ) || (0xE0100..=0xE01EF).contains(&(c as u32))
}

fn fold_confusable(c: char) -> char {
    if let Some(latin) = math_alnum(c) {
        return latin;
    }
    match c {
        'а' | 'ɑ' | 'α' => 'a',
        'А' | 'Α' => 'A',
        'е' | 'ε' | 'ё' | 'ϵ' => 'e',
        'Е' | 'Ε' | 'Ё' => 'E',
        'о' | 'ο' | 'օ' => 'o',
        'О' | 'Ο' => 'O',
        'р' | 'ρ' => 'p',
        'Р' | 'Ρ' => 'P',
        'с' | 'ϲ' | 'ҫ' => 'c',
        'С' | 'Ϲ' | 'Ҫ' => 'C',
        'у' | 'γ' | 'ү' => 'y',
        'У' | 'Ү' => 'Y',
        'х' | 'χ' => 'x',
        'Х' | 'Χ' => 'X',
        'і' | 'ι' | 'ɪ' | 'ı' => 'i',
        'І' | 'Ι' | 'Ⅰ' => 'I',
        'ѕ' => 's',
        'Ѕ' => 'S',
        'ӏ' | 'ⅼ' | 'ℓ' => 'l',
        'Ӏ' => 'I',
        'к' | 'κ' | 'ĸ' => 'k',
        'К' | 'Κ' => 'K',
        'т' | 'τ' => 't',
        'Т' | 'Τ' => 'T',
        'н' | 'η' => 'h',
        'Н' | 'Η' => 'H',
        'м' | 'μ' => 'm',
        'М' | 'Μ' => 'M',
        'в' => 'b',
        'В' | 'Β' => 'B',
        'ј' => 'j',
        'Ј' => 'J',
        'ԁ' => 'd',
        'ɡ' => 'g',
        other => other,
    }
}

fn math_alnum(c: char) -> Option<char> {
    let u = c as u32;
    let ranges: &[(u32, u32, u32)] = &[
        (0x1D400, 0x1D419, 'A' as u32),
        (0x1D41A, 0x1D433, 'a' as u32),
        (0x1D434, 0x1D44D, 'A' as u32),
        (0x1D44E, 0x1D467, 'a' as u32),
        (0x1D468, 0x1D481, 'A' as u32),
        (0x1D482, 0x1D49B, 'a' as u32),
        (0x1D4D0, 0x1D4E9, 'A' as u32),
        (0x1D4EA, 0x1D503, 'a' as u32),
        (0x1D504, 0x1D51C, 'A' as u32),
        (0x1D51E, 0x1D537, 'a' as u32),
        (0x1D538, 0x1D550, 'A' as u32),
        (0x1D552, 0x1D56B, 'a' as u32),
        (0x1D56C, 0x1D585, 'A' as u32),
        (0x1D586, 0x1D59F, 'a' as u32),
        (0x1D5A0, 0x1D5B9, 'A' as u32),
        (0x1D5BA, 0x1D5D3, 'a' as u32),
        (0x1D5D4, 0x1D5ED, 'A' as u32),
        (0x1D5EE, 0x1D607, 'a' as u32),
        (0x1D608, 0x1D621, 'A' as u32),
        (0x1D622, 0x1D63B, 'a' as u32),
        (0x1D63C, 0x1D655, 'A' as u32),
        (0x1D656, 0x1D66F, 'a' as u32),
        (0x1D670, 0x1D689, 'A' as u32),
        (0x1D68A, 0x1D6A3, 'a' as u32),
        (0x1D7CE, 0x1D7D7, '0' as u32),
        (0x1D7D8, 0x1D7E1, '0' as u32),
        (0x1D7E2, 0x1D7EB, '0' as u32),
        (0x1D7EC, 0x1D7F5, '0' as u32),
        (0x1D7F6, 0x1D7FF, '0' as u32),
    ];
    for &(start, end, base) in ranges {
        if (start..=end).contains(&u) {
            return char::from_u32(base + (u - start));
        }
    }
    None
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn html_decode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        if let Some(end) = rest.find(';') {
            if end > 1 && end <= 32 {
                if let Some(ch) = decode_entity(&rest[1..end]) {
                    out.push(ch);
                    rest = &rest[end + 1..];
                    continue;
                }
            }
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

fn decode_entity(ent: &str) -> Option<char> {
    if let Some(hex) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    if let Some(dec) = ent.strip_prefix('#') {
        return dec.parse::<u32>().ok().and_then(char::from_u32);
    }
    match ent {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "commat" => Some('@'),
        "sol" => Some('/'),
        "colon" => Some(':'),
        "period" => Some('.'),
        "minus" | "ndash" | "mdash" => Some('-'),
        "nbsp" | "tab" => Some(' '),
        "bsol" => Some('\\'),
        "tilde" => Some('~'),
        _ => None,
    }
}

fn strip_ws_around_punct(s: &str) -> String {
    const PUNCT: &[char] = &['@', '.', '/', ':', '-'];
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, &c) in chars.iter().enumerate() {
        if c.is_whitespace() {
            let prev_punct = out.chars().last().is_some_and(|p| PUNCT.contains(&p));
            let next_punct = chars[i + 1..]
                .iter()
                .find(|ch| !ch.is_whitespace())
                .is_some_and(|n| PUNCT.contains(n));
            if prev_punct || next_punct {
                continue;
            }
        }
        out.push(c);
    }
    out
}

fn strip_all_ws(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn decode_base64_runs(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && out.len() < MAX_B64_RUNS {
        if !is_b64_alnum(bytes[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && is_b64_alnum(bytes[i]) {
            i += 1;
        }
        let mut end = i;
        let mut pad = 0usize;
        while end < bytes.len() && bytes[end] == b'=' && pad < 2 {
            end += 1;
            pad += 1;
        }
        let run = &s[start..end];
        if looks_like_base64_candidate(run) {
            if let Some(text) = decode_b64_utf8(run) {
                if text.len() <= MAX_B64_DECODED && !out.contains(&text) {
                    out.push(text);
                }
            }
        }
        i = end.max(start + 1);
    }
    out
}

fn is_b64_alnum(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'-' | b'_')
}

fn looks_like_base64_candidate(run: &str) -> bool {
    let body = run.trim_end_matches('=');
    if body.len() < 16 || !body.len().is_multiple_of(4) {
        return false;
    }
    if !body
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '-' | '_'))
    {
        return false;
    }
    let up = body.chars().filter(|c| c.is_ascii_uppercase()).count();
    let low = body.chars().filter(|c| c.is_ascii_lowercase()).count();
    let dig = body.chars().filter(|c| c.is_ascii_digit()).count();
    let plus = body.chars().filter(|c| matches!(c, '+' | '/')).count();
    let us = body.chars().filter(|c| *c == '_').count();
    let hyph = body.chars().filter(|c| *c == '-').count();
    // Standard base64 of text is usually mixed-case and often has digits, or
    // uses '+' / '/'. Reject kebab-case or camel-case English.
    if plus > 0 {
        return true;
    }
    if up > 0 && low > 0 && dig > 0 {
        return true;
    }
    if up > 0 && low > 0 && up * 4 >= body.len() && low * 4 >= body.len() {
        return true;
    }
    if us > 0 && (up > 0 || dig > 0) {
        return true;
    }
    if hyph > 0 && up > 0 && low > 0 && dig > 0 {
        return true;
    }
    false
}

fn decode_b64_utf8(run: &str) -> Option<String> {
    let has_std = run.bytes().any(|b| matches!(b, b'+' | b'/'));
    let has_url = run.bytes().any(|b| matches!(b, b'-' | b'_'));
    let attempts: &[bool] = if has_std && !has_url {
        &[false]
    } else if has_url && !has_std {
        &[true]
    } else {
        &[false, true]
    };
    for &url in attempts {
        if let Some(bytes) = decode_b64(run, url) {
            if bytes.len() <= MAX_B64_DECODED {
                if let Ok(text) = String::from_utf8(bytes) {
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }
    }
    None
}

fn decode_b64(run: &str, url: bool) -> Option<Vec<u8>> {
    let mut vals = Vec::new();
    for b in run.bytes() {
        if b == b'=' {
            continue;
        }
        vals.push(b64_val(b, url)?);
    }
    if vals.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    let (chunks, rem) = vals.as_chunks::<4>();
    for chunk in chunks {
        let n = ((chunk[0] as u32) << 18)
            | ((chunk[1] as u32) << 12)
            | ((chunk[2] as u32) << 6)
            | (chunk[3] as u32);
        out.push((n >> 16) as u8);
        out.push((n >> 8) as u8);
        out.push(n as u8);
    }
    match rem.len() {
        0 => {}
        2 => {
            let n = ((rem[0] as u32) << 18) | ((rem[1] as u32) << 12);
            out.push((n >> 16) as u8);
        }
        3 => {
            let n = ((rem[0] as u32) << 18) | ((rem[1] as u32) << 12) | ((rem[2] as u32) << 6);
            out.push((n >> 16) as u8);
            out.push((n >> 8) as u8);
        }
        _ => return None,
    }
    Some(out)
}

fn b64_val(b: u8, url: bool) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' if !url => Some(62),
        b'/' if !url => Some(63),
        b'-' if url => Some(62),
        b'_' if url => Some(63),
        _ => None,
    }
}

pub fn assert_safe_for_cloud(parts: &[&str]) -> Result<(), String> {
    let mut joined = String::new();
    for part in parts {
        joined.push_str(part);
        joined.push('\n');
    }
    let hits = cloud_redaction_hits(&joined);
    if hits.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "refusing to send text to the cloud fallback ({})",
            hits.join(", ")
        ))
    }
}

fn looks_like_api_token(text: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "sk-",
        "sk-proj-",
        "sk-live-",
        "sk-ant-",
        "sk-or-",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "github_pat_",
        "glpat-",
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "xoxr-",
        "xoxs-",
        "AKIA",
        "ASIA",
        "AIza",
        "ya29.",
        "hf_",
        "xai-",
        "npm_",
        "pypi-",
        "shpat_",
        "gsk_",
    ];
    for tok in tokens(text) {
        for prefix in PREFIXES {
            if tok.starts_with(prefix) && tok.len() > prefix.len() + 8 {
                return true;
            }
        }
    }
    false
}

fn looks_like_bearer(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let Some(idx) = lower.find("bearer") else {
        return false;
    };
    let rest = text[idx + "bearer".len()..].trim_start();
    let tok = rest.split_whitespace().next().unwrap_or("");
    tok.len() >= 8
}

fn looks_like_jwt(text: &str) -> bool {
    for tok in tokens(text) {
        if jwt_shape(tok) {
            return true;
        }
    }
    false
}

fn jwt_shape(tok: &str) -> bool {
    let mut parts = tok.split('.');
    let Some(header) = parts.next() else {
        return false;
    };
    let Some(payload) = parts.next() else {
        return false;
    };
    let Some(sig) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    if header.len() < 8 || payload.len() < 8 || sig.is_empty() {
        return false;
    }
    if !header.starts_with("eyJ") && !header.starts_with("eyj") {
        return false;
    }
    [header, payload, sig].iter().all(|p| {
        p.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    })
}

fn looks_like_high_entropy_secret(text: &str) -> bool {
    for tok in tokens(text) {
        if jwt_shape(tok) {
            continue;
        }
        if is_high_entropy_token(tok) {
            return true;
        }
    }
    false
}

fn is_high_entropy_token(tok: &str) -> bool {
    if tok.len() < 24 {
        return false;
    }
    if !tok
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '/' | '='))
    {
        return false;
    }
    let mut has_upper = false;
    let mut has_lower = false;
    let mut has_digit = false;
    let mut has_other = false;
    for c in tok.chars() {
        if c.is_ascii_uppercase() {
            has_upper = true;
        } else if c.is_ascii_lowercase() {
            has_lower = true;
        } else if c.is_ascii_digit() {
            has_digit = true;
        } else {
            has_other = true;
        }
    }
    let classes = [has_upper, has_lower, has_digit, has_other]
        .iter()
        .filter(|v| **v)
        .count();
    if tok.len() >= 32 && tok.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    classes >= 3
}

#[derive(Clone, Copy)]
enum CivicKind {
    Ordinary,
    Coordination,
}

fn swedish_civic_kind(text: &str) -> Option<CivicKind> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut found = None;
    while i < n {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < n && chars[i].is_ascii_digit() {
            i += 1;
        }
        let first = i - start;
        let mut sep = false;
        if i < n && matches!(chars[i], '-' | '+' | ' ') {
            sep = true;
            i += 1;
        }
        let mid = i;
        while i < n && chars[i].is_ascii_digit() {
            i += 1;
        }
        let second = i - mid;
        let mut digits = String::new();
        for ch in &chars[start..start + first] {
            digits.push(*ch);
        }
        for ch in &chars[mid..mid + second] {
            digits.push(*ch);
        }
        if let Some(kind) = classify_civic_digits(&digits, sep, first, second) {
            found = Some(kind);
            // Prefer coordination if any match is one; otherwise keep ordinary.
            if matches!(kind, CivicKind::Coordination) {
                return Some(kind);
            }
        } else if !sep {
            // already consumed only the first run
        }
    }
    found
}

fn classify_civic_digits(
    digits: &str,
    had_sep: bool,
    first: usize,
    second: usize,
) -> Option<CivicKind> {
    let body = match digits.len() {
        10 => digits,
        12 => &digits[2..],
        _ => return None,
    };
    // With a separator, accept 8+4 (12-digit date) or 6+4 (10-digit date).
    // Without a separator, the whole run must be 10 or 12 digits.
    if had_sep {
        let ok = (first == 8 && second == 4) || (first == 6 && second == 4);
        if !ok {
            return None;
        }
        if first == 8 && !plausible_year(&digits[..4]) {
            return None;
        }
    } else if (digits.len() != 10 && digits.len() != 12)
        || (digits.len() == 12 && !plausible_year(&digits[..4]))
    {
        return None;
    }
    if body.len() != 10 {
        return None;
    }
    let month: u32 = body[2..4].parse().ok()?;
    let day: u32 = body[4..6].parse().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    if (1..=31).contains(&day) {
        Some(CivicKind::Ordinary)
    } else if (61..=91).contains(&day) {
        Some(CivicKind::Coordination)
    } else {
        None
    }
}

fn plausible_year(yyyy: &str) -> bool {
    let year: u32 = match yyyy.parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    (1850..=2099).contains(&year)
}

fn looks_like_phone(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lower.contains("tel:") {
        let after = lower.split("tel:").nth(1).unwrap_or("");
        let digits: String = after
            .chars()
            .take_while(|c| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '(' | ')'))
            .filter(|c| c.is_ascii_digit())
            .collect();
        if digits.len() >= 8 {
            return true;
        }
    }
    // +country or 00-country forms, allowing spaces, dashes, and parentheses.
    if scan_international_phone(&lower) {
        return true;
    }
    // National forms: leading 0, 8–12 digits (Swedish 07x / 08 / 0xx).
    if scan_national_phone(&lower) {
        return true;
    }
    false
}

fn scan_international_phone(lower: &str) -> bool {
    let chars: Vec<char> = lower.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        let plus = chars[i] == '+';
        let zerozero = chars[i] == '0' && i + 1 < n && chars[i + 1] == '0';
        if plus || zerozero {
            let start = if plus { i + 1 } else { i + 2 };
            let mut j = start;
            let mut digits = 0u32;
            while j < n {
                let c = chars[j];
                if c.is_ascii_digit() {
                    digits += 1;
                    j += 1;
                } else if matches!(c, ' ' | '-' | '(' | ')') {
                    j += 1;
                } else {
                    break;
                }
            }
            if (8..=15).contains(&digits) {
                return true;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    false
}

fn scan_national_phone(lower: &str) -> bool {
    let chars: Vec<char> = lower.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        if chars[i] == '0' && i + 1 < n && chars[i + 1] != '0' {
            let boundary_ok = i == 0 || !chars[i - 1].is_ascii_digit();
            if boundary_ok {
                let mut j = i;
                let mut digits = 0u32;
                let mut seps = 0u32;
                while j < n {
                    let c = chars[j];
                    if c.is_ascii_digit() {
                        digits += 1;
                        j += 1;
                    } else if matches!(c, ' ' | '-' | '(' | ')') {
                        seps += 1;
                        j += 1;
                    } else {
                        break;
                    }
                }
                // Swedish mobiles are 10 digits (07xxxxxxxx). Landlines 8–10.
                // Also accept a bare 10-digit run that starts with 07.
                let starts_mobile = digits >= 2 && chars[i + 1] == '7';
                if (8..=12).contains(&digits) && (seps > 0 || starts_mobile || digits == 10) {
                    // Avoid treating a civic-number 10-digit run starting with 0
                    // as a phone only when it already looks like a date; civic
                    // detection covers those separately.
                    return true;
                }
                i = j;
                continue;
            }
        }
        // US-style (xxx) yyy-zzzz
        if chars[i] == '(' && i + 4 < n && chars[i + 4] == ')' {
            let area: String = chars[i + 1..i + 4].iter().collect();
            if area.chars().all(|c| c.is_ascii_digit()) {
                let rest: String = chars[i + 5..]
                    .iter()
                    .take_while(|c| c.is_ascii_digit() || matches!(c, ' ' | '-'))
                    .filter(|c| c.is_ascii_digit())
                    .collect();
                if rest.len() == 7 {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

fn looks_like_email(text: &str) -> bool {
    for (idx, ch) in text.char_indices() {
        if ch != '@' {
            continue;
        }
        let prefix = &text[..idx];
        let start = prefix
            .char_indices()
            .rev()
            .find(|(_, c)| !c.is_ascii_alphanumeric() && !matches!(c, '.' | '_' | '-' | '+'))
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        let suffix = &text[idx + ch.len_utf8()..];
        let end_rel = suffix
            .char_indices()
            .find(|(_, c)| !c.is_ascii_alphanumeric() && *c != '.' && *c != '-')
            .map(|(i, _)| i)
            .unwrap_or(suffix.len());
        let token = &text[start..idx + ch.len_utf8() + end_rel];
        if is_email_shape(token) {
            return true;
        }
    }
    false
}

fn is_email_shape(tok: &str) -> bool {
    let tok = tok.trim_matches(|c: char| {
        !c.is_ascii_alphanumeric() && !matches!(c, '@' | '.' | '_' | '-' | '+')
    });
    let Some((user, rest)) = tok.split_once('@') else {
        return false;
    };
    let Some((host, tld)) = rest.rsplit_once('.') else {
        return false;
    };
    !user.is_empty()
        && !host.is_empty()
        && (2..=24).contains(&tld.len())
        && tld.chars().all(|c| c.is_ascii_alphabetic())
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
}

fn looks_like_home_path(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lower.contains("/users/")
        || lower.contains("/users\\")
        || lower.contains("/home/")
        || lower.contains("\\users\\")
        || lower.contains("/users") && trailing_or_sep(&lower, "/users")
        || lower.contains("c:\\users")
        || lower.contains("c:/users")
        || lower.contains("%userprofile%")
        || lower.contains("%homepath%")
        || lower.contains("$home")
        || lower.contains("${home}")
        || lower.contains("/private/var/folders")
        || lower.contains("/root/")
    {
        return true;
    }
    // ~ / ~/ ~name / ~name/
    for (i, c) in text.char_indices() {
        if c != '~' {
            continue;
        }
        let before_ok = i == 0
            || text[..i]
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_whitespace() || matches!(ch, '"' | '\'' | '=' | ':'));
        if !before_ok {
            continue;
        }
        let after = &text[i + 1..];
        if after.is_empty() || after.starts_with('/') || after.starts_with('\\') {
            return true;
        }
        // ~username or ~username/path
        let name: String = after
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
            .collect();
        if !name.is_empty() {
            let rest = &after[name.len()..];
            if rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\') {
                return true;
            }
        }
    }
    false
}

fn trailing_or_sep(lower: &str, needle: &str) -> bool {
    if let Some(idx) = lower.find(needle) {
        let after = &lower[idx + needle.len()..];
        after.is_empty() || after.starts_with(|c: char| !c.is_ascii_alphanumeric())
    } else {
        false
    }
}

fn looks_like_ip(text: &str) -> bool {
    for tok in tokens(text) {
        let tok = tok.trim_matches(|c: char| matches!(c, ',' | ';' | ')' | '('));
        if ipv4(tok) || ipv6(tok) {
            return true;
        }
    }
    false
}

fn ipv4(tok: &str) -> bool {
    let parts: Vec<&str> = tok.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| {
        if p.is_empty() || p.len() > 3 || !p.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        // No leading zeros except "0" itself? Still accept 10.0.0.1; reject 1.2.3 only via len.
        p.parse::<u16>().ok().is_some_and(|n| n <= 255)
    })
}

fn ipv6(tok: &str) -> bool {
    if tok == "::1" || tok == "::" {
        return true;
    }
    if !tok.contains(':') {
        return false;
    }
    // Do not treat MAC or times (12:30:00) as IPv6.
    if tok.chars().filter(|c| *c == ':').count() < 2 {
        return false;
    }
    let compressed = tok.contains("::");
    let parts: Vec<&str> = tok.split(':').collect();
    if !parts
        .iter()
        .all(|p| p.is_empty() || (p.len() <= 4 && p.chars().all(|c| c.is_ascii_hexdigit())))
    {
        return false;
    }
    let lettered = tok
        .chars()
        .any(|c| c.is_ascii_hexdigit() && !c.is_ascii_digit());
    if compressed {
        return parts.len() >= 3 && parts.len() <= 8;
    }
    // Full form is 8 groups. Require a hex letter or 8 groups so 12:30:00 is out.
    parts.len() == 8 || (lettered && parts.len() >= 4)
}

fn looks_like_mac(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i + 16 < n + 1 {
        if i + 16 < n + 1 && mac_at(&chars, i, ':') || mac_at(&chars, i, '-') {
            return true;
        }
        i += 1;
    }
    false
}

fn mac_at(chars: &[char], i: usize, sep: char) -> bool {
    // 6 hex pairs + 5 seps = 17 chars
    if i + 17 > chars.len() {
        return false;
    }
    for g in 0..6 {
        let a = chars[i + g * 3];
        let b = chars[i + g * 3 + 1];
        if !a.is_ascii_hexdigit() || !b.is_ascii_hexdigit() {
            return false;
        }
        if g < 5 && chars[i + g * 3 + 2] != sep {
            return false;
        }
    }
    let before_ok = i == 0 || !chars[i - 1].is_ascii_hexdigit();
    let after_ok = i + 17 == chars.len() || !chars[i + 17].is_ascii_hexdigit();
    before_ok && after_ok
}

fn looks_like_hostname(text: &str) -> bool {
    const SUSPECT_TLDS: &[&str] = &[
        ".local",
        ".lan",
        ".internal",
        ".corp",
        ".intranet",
        ".localdomain",
        ".home",
    ];
    let lower = text.to_ascii_lowercase();
    for tld in SUSPECT_TLDS {
        if let Some(idx) = lower.find(tld) {
            // Allow "mac-mini" / "laptop-1" (no such suffix).
            let after = &lower[idx + tld.len()..];
            let after_ok = after.is_empty()
                || after.starts_with(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
            if after_ok {
                return true;
            }
        }
    }
    for tok in tokens(text) {
        let tok = tok.trim_matches(|c: char| matches!(c, '.' | ',' | ';' | ':' | ')' | '('));
        if is_allowed_alias(tok) {
            continue;
        }
        if looks_like_fqdn(tok) {
            return true;
        }
    }
    false
}

const PUBLIC_TLDS: &[&str] = &[
    "com", "org", "net", "edu", "gov", "mil", "int", "io", "ai", "app", "dev", "info", "biz",
    "name", "se", "nu", "uk", "de", "fr", "es", "it", "nl", "be", "dk", "no", "fi", "us", "ca",
    "au", "nz", "jp", "cn", "ru", "ch", "at", "pl", "cz", "co", "me", "tv", "cc", "xyz", "online",
    "site", "tech", "cloud",
];

fn is_allowed_alias(tok: &str) -> bool {
    matches!(
        tok.to_ascii_lowercase().as_str(),
        "mac-mini" | "laptop-1" | "laptop-2"
    )
}

fn looks_like_fqdn(tok: &str) -> bool {
    const FILE_EXT: &[&str] = &[
        "json", "md", "rs", "toml", "gguf", "lock", "sh", "py", "txt", "csv", "yml", "yaml",
        "html", "css", "js", "ts", "wasm", "so", "bin", "log", "cpp", "h", "hpp", "c", "o", "a",
        "rlib", "dylib", "exe", "sha256", "svg", "png", "jpg", "webp",
    ];
    let lower = tok.to_ascii_lowercase();
    let Some((host, tld)) = lower.rsplit_once('.') else {
        return false;
    };
    if host.is_empty()
        || !(2..=24).contains(&tld.len())
        || !tld.chars().all(|c| c.is_ascii_alphabetic())
    {
        return false;
    }
    if FILE_EXT.contains(&tld) {
        return false;
    }
    if !PUBLIC_TLDS.contains(&tld) {
        return false;
    }
    // host must look like a DNS label path
    host.split('.')
        .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && host.chars().any(|c| c.is_ascii_alphabetic())
}

fn looks_like_name_field(text: &str) -> bool {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text.trim()) {
        if json_has_name_field(&value) {
            return true;
        }
    }
    // Embedded objects: try each '{...}' span cheaply.
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text[start..=end]) {
                    if json_has_name_field(&value) {
                        return true;
                    }
                }
            }
        }
    }
    let lower = text.to_ascii_lowercase();
    for key in NAME_LIKE_FIELDS {
        let quoted = format!("\"{key}\"");
        if let Some(idx) = lower.find(&quoted) {
            let after = lower[idx + quoted.len()..].trim_start();
            if after.starts_with(':') {
                let val = after.trim_start_matches(':').trim_start();
                if val.starts_with('"') && val.len() > 1 {
                    return true;
                }
            }
        }
        // key=value
        if let Some(idx) = lower.find(&format!("{key}=")) {
            let val = lower[idx + key.len() + 1..].trim_start();
            if !val.is_empty() && !val.starts_with(|c: char| c.is_whitespace()) {
                return true;
            }
        }
    }
    false
}

fn json_has_name_field(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if NAME_LIKE_FIELDS.iter().any(|n| n.eq_ignore_ascii_case(k))
                    && v.as_str().is_some_and(|s| !s.trim().is_empty())
                {
                    return true;
                }
                if json_has_name_field(v) {
                    return true;
                }
            }
            false
        }
        serde_json::Value::Array(items) => items.iter().any(json_has_name_field),
        _ => false,
    }
}

fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\'' | '<' | '>'
            )
    })
    .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_business() -> &'static str {
        "The Northbridge depot of River Co. holds 12 teal crates."
    }

    #[test]
    fn clean_synthetic_text_is_allowed() {
        assert!(cloud_redaction_hits(clean_business()).is_empty());
        assert!(assert_safe_for_cloud(&[clean_business()]).is_ok());
    }

    #[test]
    fn aliases_are_allowed() {
        let hits = cloud_redaction_hits("checked on mac-mini and laptop-1 and laptop-2");
        assert!(hits.is_empty(), "{hits:?}");
    }

    #[test]
    fn adversarial_api_token_is_refused() {
        let prefix = ["sk", "live"].join("-") + "-";
        let token = format!("{prefix}{}", "ABCDEFGHIJKLMNOPQRST");
        let text = format!("use key {token} when counting crates");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"api-token"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn adversarial_bearer_token_is_refused() {
        let token = format!("{}{}", "ghp_", "aaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let text = format!("{} {}", "Bearer", token);
        let hits = cloud_redaction_hits(&text);
        assert!(
            hits.contains(&"bearer-token") || hits.contains(&"api-token"),
            "{hits:?}"
        );
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn adversarial_jwt_is_refused() {
        let jwt = format!(
            "{}.{}.{}",
            "eyJhbGciOiJub25lIn0", "eyJzdWIiOiJyaXZlci1jbyJ9", "c2ln"
        );
        let text = format!("token {jwt}");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"jwt"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn adversarial_high_entropy_is_refused() {
        // Mixed-class 32-char secret, not a known prefix.
        let token: String = "A9b8C7d6E5f4G3h2I1j0K9l8M7n6O5p4".into();
        let text = format!("secret {token}");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"high-entropy"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn adversarial_civic_number_is_refused() {
        // Fictional 12-digit form (century + date + four). Built in parts.
        let text = format!("id {}{}{}-{}", "19", "90", "0101", "2391");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"civic-number"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn adversarial_coordination_number_is_refused() {
        // Day 01 + 60 = 61, fictional four-digit suffix.
        let text = format!("id {}{}{}-{}", "19", "90", "0161", "2391");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"coordination-number"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn adversarial_phone_is_refused() {
        let intl = format!("+{} {} {} {} {}", "46", "70", "123", "45", "67");
        let national = format!("{}-{} {} {}", "070", "123", "45", "67");
        let bare = format!("{}{}", "070", "1234567");
        for sample in [intl, national, bare] {
            let text = format!("call {sample} about the crates");
            let hits = cloud_redaction_hits(&text);
            assert!(hits.contains(&"phone"), "{sample} -> {hits:?}");
            assert!(assert_safe_for_cloud(&[&text]).is_err());
        }
    }

    #[test]
    fn adversarial_email_is_refused() {
        let mail = format!("{}@{}", "ada", "example.com");
        // Buried in punctuation, not a whitespace token.
        let text = format!("write to <{mail}> please");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"email"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn adversarial_home_path_is_refused() {
        let unix = format!("/{}/{}/notes", "Users", "someone");
        let linux = format!("/{}/{}/notes", "home", "someone");
        let win = format!("{}\\{}\\notes", "C:\\Users", "someone");
        let tilde = format!("{}{}", "~", "/notes");
        for sample in [unix, linux, win, tilde] {
            let text = format!("see {sample}");
            let hits = cloud_redaction_hits(&text);
            assert!(hits.contains(&"home-path"), "{sample} -> {hits:?}");
            assert!(assert_safe_for_cloud(&[&text]).is_err());
        }
    }

    #[test]
    fn adversarial_ip_is_refused() {
        // Documentation range, built in parts so the source is not a literal address.
        let ip = format!("{}.{}.{}.{}", "192", "0", "2", "1");
        let text = format!("host {ip}");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"ip-address"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn adversarial_mac_is_refused() {
        let mac = format!("{}:{}:{}:{}:{}:{}", "00", "1A", "2B", "3C", "4D", "5E");
        let text = format!("nic {mac}");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"mac-address"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn adversarial_hostname_is_refused() {
        let host = format!("{}{}", "depot", ".local");
        let text = format!("checked on {host}");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"hostname"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn adversarial_name_field_is_refused() {
        let json = format!("{{\"{}\":\"{}\"}}", "full_name", "Ada Example");
        let hits = cloud_redaction_hits(&json);
        assert!(hits.contains(&"name-field"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&json]).is_err());
    }

    #[test]
    fn a_hit_refuses_the_whole_call_never_partial() {
        let mail = format!("{}@{}", "ada", "example.com");
        let text = format!("{}, and also {mail}", clean_business());
        let err = assert_safe_for_cloud(&[&text]).unwrap_err();
        assert!(err.contains("refusing"));
        assert!(err.contains("email"));
        // The function does not return a scrubbed string.
        assert!(!err.contains(&mail) || err.starts_with("refusing"));
    }

    fn assert_email_refused(text: &str) {
        let hits = cloud_redaction_hits(text);
        assert!(hits.contains(&"email"), "{text:?} -> {hits:?}");
        assert!(assert_safe_for_cloud(&[text]).is_err());
    }

    #[test]
    fn bypass_percent_encoded_at_is_refused() {
        let text = format!("{}%40{}.{}", "ada", "example", "com");
        assert_email_refused(&text);
    }

    #[test]
    fn bypass_base64_email_is_refused() {
        // Standard base64 of ada@example.com (20 chars, under the high-entropy floor).
        let encoded = "YWRhQGV4YW1wbGUuY29t";
        let text = format!("payload {encoded}");
        assert_email_refused(&text);
    }

    #[test]
    fn bypass_fullwidth_at_is_refused() {
        let at = char::from_u32(0xFF20).unwrap();
        let text = format!("{}{}{}.{}", "ada", at, "example", "com");
        assert_email_refused(&text);
    }

    #[test]
    fn bypass_split_token_prefix_is_refused() {
        let text = format!("{} {} {}{}", "sk-", "live-", "ABCDEFGHIJKLMNOPQRST", "UVW");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"api-token"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn bypass_percent_encoded_home_path_is_refused() {
        let text = format!("see %2F{}%2F{}%2Fnotes", "home", "someone");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"home-path"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn bypass_homoglyph_token_prefix_is_refused() {
        // Cyrillic dze and ka stand in for Latin s and k.
        let s = char::from_u32(0x0455).unwrap();
        let k = char::from_u32(0x043A).unwrap();
        let text = format!(
            "use key {}{}-live-{} when counting crates",
            s, k, "ABCDEFGHIJKLMNOPQRST"
        );
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"api-token"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }

    #[test]
    fn bypass_spaced_email_dots_is_refused() {
        let text = format!("{} . @ {} . {}", "ada", "example", "com");
        assert_email_refused(&text);
    }

    #[test]
    fn bypass_html_entity_at_is_refused() {
        let text = format!("{}&#64;{}.{}", "ada", "example", "com");
        assert_email_refused(&text);
        let named = format!("{}&commat;{}.{}", "ada", "example", "com");
        assert_email_refused(&named);
    }

    #[test]
    fn bypass_zero_width_in_email_is_refused() {
        let zw = char::from_u32(0x200B).unwrap();
        let text = format!("{}{}@{}.{}", "ada", zw, "example", "com");
        assert_email_refused(&text);
    }

    #[test]
    fn bypass_double_percent_encoded_home_path_is_refused() {
        // %252F decodes to %2F, then to /
        let text = format!("see %252F{}%252F{}%252Fnotes", "home", "someone");
        let hits = cloud_redaction_hits(&text);
        assert!(hits.contains(&"home-path"), "{hits:?}");
        assert!(assert_safe_for_cloud(&[&text]).is_err());
    }
}
