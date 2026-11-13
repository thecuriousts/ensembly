//! Fail-closed cloud redaction gate.
//!
//! If any pattern matches, the cloud call is refused. Text is never partly
//! redacted and then sent. Tests build adversarial samples at runtime so this
//! file does not embed phone numbers, civic numbers, tokens, or home paths.

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
    for (idx, _) in text.match_indices('@') {
        let prefix = &text[..idx];
        let start = prefix
            .rfind(|c: char| !c.is_ascii_alphanumeric() && !matches!(c, '.' | '_' | '-' | '+'))
            .map(|i| i + 1)
            .unwrap_or(0);
        let suffix = &text[idx + 1..];
        let end_rel = suffix
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-')
            .unwrap_or(suffix.len());
        let token = &text[start..idx + 1 + end_rel];
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
}
