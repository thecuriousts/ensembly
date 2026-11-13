//! Minimal RFC 3492 Punycode decoder (decode only).
//!
//! There is no `idna` / `punycode` crate in the workspace lockfile, so this
//! small decoder turns `xn--` labels into Unicode before confusable folding.
//! Invalid input is left unchanged. Labels are capped at 63 code points.

const BASE: u32 = 36;
const TMIN: u32 = 1;
const TMAX: u32 = 26;
const SKEW: u32 = 38;
const DAMP: u32 = 700;
const INITIAL_BIAS: u32 = 72;
const INITIAL_N: u32 = 128;
const MAX_LABEL: usize = 63;

fn digit(c: u8) -> Option<u32> {
    match c {
        b'a'..=b'z' => Some((c - b'a') as u32),
        b'A'..=b'Z' => Some((c - b'A') as u32),
        b'0'..=b'9' => Some((c - b'0') as u32 + 26),
        _ => None,
    }
}

fn adapt(mut delta: u32, numpoints: u32, first: bool) -> u32 {
    delta = if first { delta / DAMP } else { delta / 2 };
    delta = delta.saturating_add(delta / numpoints);
    let mut k = 0;
    while delta > ((BASE - TMIN) * TMAX) / 2 {
        delta /= BASE - TMIN;
        k += BASE;
    }
    k + ((BASE - TMIN + 1) * delta) / (delta + SKEW)
}

/// Decode a Punycode string (no `xn--` prefix) to Unicode.
pub fn decode(input: &str) -> Option<String> {
    if !input.is_ascii() || input.len() > MAX_LABEL * 4 {
        return None;
    }
    let (mut output, encoded) = match input.rfind('-') {
        Some(i) => {
            let basic = &input[..i];
            if !basic.bytes().all(|b| b.is_ascii()) {
                return None;
            }
            (basic.chars().collect::<Vec<_>>(), &input[i + 1..])
        }
        None => (Vec::new(), input),
    };
    if output.len() > MAX_LABEL {
        return None;
    }
    let mut n = INITIAL_N;
    let mut bias = INITIAL_BIAS;
    let mut i: u32 = 0;
    let bytes = encoded.as_bytes();
    let mut in_idx = 0;
    while in_idx < bytes.len() {
        let old_i = i;
        let mut w: u32 = 1;
        let mut k = BASE;
        loop {
            let d = digit(*bytes.get(in_idx)?)?;
            in_idx += 1;
            i = i.checked_add(d.checked_mul(w)?)?;
            let t = if k <= bias {
                TMIN
            } else if k >= bias + TMAX {
                TMAX
            } else {
                k - bias
            };
            if d < t {
                break;
            }
            w = w.checked_mul(BASE - t)?;
            k = k.checked_add(BASE)?;
        }
        let out_len = (output.len() + 1) as u32;
        bias = adapt(i - old_i, out_len, old_i == 0);
        n = n.checked_add(i / out_len)?;
        i %= out_len;
        let ch = char::from_u32(n)?;
        if output.len() >= MAX_LABEL {
            return None;
        }
        output.insert(i as usize, ch);
        i += 1;
    }
    Some(output.into_iter().collect())
}

/// Decode an `xn--` label, or return the original label if it is not ACE.
pub fn decode_label(label: &str) -> String {
    let lower = label.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("xn--") {
        if let Some(decoded) = decode(rest) {
            if !decoded.is_empty() {
                return decoded;
            }
        }
    }
    label.to_string()
}

/// Decode every `xn--` label in a host already split on ASCII dots.
pub fn decode_host(host: &str) -> String {
    let mut out = String::with_capacity(host.len());
    for (idx, label) in host.split('.').enumerate() {
        if idx > 0 {
            out.push('.');
        }
        out.push_str(&decode_label(label));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_and_known_vectors() {
        assert_eq!(decode("bcher-kva").as_deref(), Some("bücher"));
        assert_eq!(decode("mnchen-3ya").as_deref(), Some("münchen"));
        assert_eq!(decode("hish-f6d").as_deref(), Some("рhish"));
        assert_eq!(decode_label("xn--hish-f6d"), "рhish");
        assert_eq!(decode_label("XN--hish-f6d"), "рhish");
        assert_eq!(decode_label("example"), "example");
        assert_eq!(
            decode_host("xn--hish-f6d.example.test"),
            "рhish.example.test"
        );
    }

    #[test]
    fn invalid_input_is_none_or_unchanged() {
        assert!(decode("!!!").is_none());
        assert_eq!(decode_label("xn--!!!"), "xn--!!!");
        assert_eq!(decode_label("xn--"), "xn--");
    }
}
