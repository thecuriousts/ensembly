//! Canonical pulse pack: `pulse-pack-v1`, canon rule 1 (spec: docs/spec/PULSE-PACK-SPEC.md).
//!
//! One format for both sides. The Python pack id `pulse-pack-v1` is canonical. Rust
//! packs (`ensembly-pulse-pack-v1`, legacy `peram-pulse-pack-v1`) are read through a
//! mapping into the same shape, so old packs keep working. The Rust pack format and
//! its `pack_hash` are not changed here.
//!
//! The Python twin is `scripts/pulse/pulse_pack_check.py`. Both must reproduce the
//! golden vectors under `tests/golden/pulse-pack/` byte for byte.

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::Serialize;
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

pub const CANON_FORMAT: &str = "pulse-pack-v1";
pub const CANON_VERSION: u32 = 1;
pub const RUST_FORMATS: [&str; 2] = ["ensembly-pulse-pack-v1", "peram-pulse-pack-v1"];
pub const MAX_SAFE_INT: i64 = 9_007_199_254_740_991;
pub const HOST_ALIASES: [&str; 4] = ["laptop-1", "laptop-2", "mac-mini", "box"];
const STATUSES: [&str; 4] = ["user-stated", "tool-verified", "inferred", "unknown"];
const KINDS: [&str; 4] = ["data", "context", "fact", "memory"];
const LOCKS: [&str; 2] = ["open", "locked"];
const TRACE_FIELDS: [&str; 13] = [
    "nat_key",
    "snippet",
    "source",
    "time",
    "status",
    "kind",
    "lock",
    "verify_hash",
    "prefetch_key",
    "seen_count",
    "superseded_by",
    "first_seen",
    "last_seen",
];
const NULLABLE: [&str; 3] = ["verify_hash", "prefetch_key", "superseded_by"];

/// One problem, as an RFC 6901 pointer into the input document and a code.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Issue {
    pub path: String,
    pub code: String,
}

impl Issue {
    fn new(path: impl Into<String>, code: &str) -> Self {
        Issue {
            path: path.into(),
            code: code.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub archive: usize,
    pub canon: u32,
    pub content_sha256: Option<String>,
    pub errors: Vec<Issue>,
    pub exit: i32,
    pub format: Option<String>,
    pub pack_sha256: Option<String>,
    pub source_format: Option<String>,
    pub traces: usize,
    pub verdict: String,
    pub warnings: Vec<Issue>,
}

// ------------------------------------------------------------------ strict JSON

struct Strict(Value);

thread_local! {
    /// For each number token in the input, in document order: is it an integer
    /// literal other than `-0`? serde_json hands integers beyond i64/u64 to
    /// `visit_f64`; this tells them apart from fraction or exponent literals.
    static INT_TOKENS: std::cell::RefCell<(Vec<bool>, usize)> =
        const { std::cell::RefCell::new((Vec::new(), 0)) };
}

/// Number tokens outside strings, in order. Only consulted after serde_json accepted
/// the input, so it can assume valid JSON.
fn int_tokens(input: &[u8]) -> Vec<bool> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            b'"' => {
                i += 1;
                while i < input.len() && input[i] != b'"' {
                    i += if input[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            b'-' | b'0'..=b'9' => {
                let start = i;
                while i < input.len()
                    && matches!(input[i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
                {
                    i += 1;
                }
                let tok = &input[start..i];
                out.push(tok != b"-0" && !tok.iter().any(|c| matches!(c, b'.' | b'e' | b'E')));
            }
            _ => i += 1,
        }
    }
    out
}

/// Advance to the next number token; true when it is an integer literal.
fn next_is_int() -> bool {
    INT_TOKENS.with(|t| {
        let mut t = t.borrow_mut();
        let i = t.1;
        t.1 += 1;
        t.0.get(i).copied().unwrap_or(false)
    })
}

impl<'de> de::Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(StrictVisitor).map(Strict)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("JSON")
    }
    fn visit_bool<E>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Value, E> {
        next_is_int();
        Ok(Value::Number(v.into()))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Value, E> {
        next_is_int();
        Ok(Value::Number(v.into()))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        if next_is_int() {
            // An integer literal beyond u64. Any value past MAX_SAFE_INT reads as E_RANGE.
            return Ok(Value::Number(u64::MAX.into()));
        }
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }
    fn visit_str<E>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }
    fn visit_string<E>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }
    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut out = Vec::new();
        while let Some(Strict(v)) = seq.next_element()? {
            out.push(v);
        }
        Ok(Value::Array(out))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut out = Map::new();
        while let Some(k) = map.next_key::<String>()? {
            let Strict(v) = map.next_value()?;
            if out.insert(k, v).is_some() {
                return Err(de::Error::custom("duplicate key"));
            }
        }
        Ok(Value::Object(out))
    }
}

/// Bytes to a value. Bad UTF-8, a BOM, NaN, a lone surrogate, a duplicate key or a
/// number that overflows binary64 (`1E400`) give `E_JSON` at path `""`. An integer
/// literal beyond u64 parses and is later reported as `E_RANGE`.
pub fn parse_strict(input: &[u8]) -> Result<Value, Issue> {
    if input.starts_with(b"\xef\xbb\xbf") {
        return Err(Issue::new("", "E_JSON"));
    }
    INT_TOKENS.with(|t| *t.borrow_mut() = (int_tokens(input), 0));
    let mut de = serde_json::Deserializer::from_slice(input);
    let v = de::Deserialize::deserialize(&mut de)
        .map(|Strict(v)| v)
        .map_err(|_| Issue::new("", "E_JSON"))?;
    de.end().map_err(|_| Issue::new("", "E_JSON"))?;
    Ok(v)
}

fn is_float(n: &Number) -> bool {
    !(n.is_i64() || n.is_u64())
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_value(out: &mut String, v: &Value) -> Result<(), Issue> {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if is_float(n) {
                return Err(Issue::new("", "E_FLOAT"));
            }
            out.push_str(&n.to_string());
        }
        Value::String(s) => write_str(out, s),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, x)?;
            }
            out.push(']');
        }
        Value::Object(m) => {
            // Own sort, so a `preserve_order` feature elsewhere can never change bytes.
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_str(out, k);
                out.push(':');
                write_value(out, &m[k])?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// Keys sorted by code point, `,` and `:` separators, minimal escapes, integers only.
pub fn canonical_json(v: &Value) -> Result<String, Issue> {
    let mut out = String::new();
    write_value(&mut out, v)?;
    Ok(out)
}

/// `canonical_json(v) + "\n"`.
pub fn canonical_bytes(v: &Value) -> Result<Vec<u8>, Issue> {
    let mut s = canonical_json(v)?;
    s.push('\n');
    Ok(s.into_bytes())
}

fn sha256hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// `"sha256:" + hex(sha256(utf8(snippet)))`, as the Python tools write it.
pub fn content_hash(snippet: &str) -> String {
    format!("sha256:{}", sha256hex(snippet.as_bytes()))
}

/// sha256 of the canonical bytes of the whole document.
pub fn pack_sha256(doc: &Value) -> String {
    sha256hex(&canonical_bytes(doc).expect("canonical doc has no floats"))
}

/// sha256 of the canonical bytes of `{"archive", "traces"}` only.
pub fn content_sha256(doc: &Value) -> String {
    let mut m = Map::new();
    m.insert("archive".into(), doc["archive"].clone());
    m.insert("traces".into(), doc["traces"].clone());
    pack_sha256(&Value::Object(m))
}

// ------------------------------------------------------------------ timestamps

fn num(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Strict timestamp to `YYYY-MM-DDTHH:MM:SS[.frac]Z` (UTC), or `None`. Accepts
/// RFC 3339 with `Z` or `±HH:MM` (1 to 9 fraction digits), `YYYY-MM-DD HH:MM` (UTC) and
/// `YYYY-MM-DD` (midnight UTC). ASCII digits only; uppercase `T` and `Z` only.
pub fn canonical_ts(s: &str) -> Option<String> {
    if !s.is_ascii() || s.len() < 10 {
        return None;
    }
    let (date, rest) = s.split_at(10);
    let db = date.as_bytes();
    if db[4] != b'-' || db[7] != b'-' {
        return None;
    }
    let (y, mo, d) = (num(&date[0..4])?, num(&date[5..7])?, num(&date[8..10])?);
    let (mut hh, mut mi, mut ss, mut frac, mut off) = (0, 0, 0, "", 0i64);
    let rb = rest.as_bytes();
    if rest.is_empty() {
    } else if rest.len() == 6 && rb[0] == b' ' && rb[3] == b':' {
        hh = num(&rest[1..3])?;
        mi = num(&rest[4..6])?;
    } else if rest.len() >= 10 && rb[0] == b'T' && rb[3] == b':' && rb[6] == b':' {
        hh = num(&rest[1..3])?;
        mi = num(&rest[4..6])?;
        ss = num(&rest[7..9])?;
        let mut tail = &rest[9..];
        if let Some(t) = tail.strip_prefix('.') {
            let n = t.bytes().take_while(|b| b.is_ascii_digit()).count();
            if !(1..=9).contains(&n) {
                return None;
            }
            frac = &t[..n];
            tail = &t[n..];
        }
        let tb = tail.as_bytes();
        if tail == "Z" {
        } else if tail.len() == 6 && (tb[0] == b'+' || tb[0] == b'-') && tb[3] == b':' {
            let (oh, om) = (num(&tail[1..3])?, num(&tail[4..6])?);
            if oh > 23 || om > 59 {
                return None;
            }
            off = (oh * 60 + om) * if tb[0] == b'+' { 1 } else { -1 };
        } else {
            return None;
        }
    } else {
        return None;
    }
    if !(1..=12).contains(&mo) || d < 1 || d > days_in_month(y, mo) {
        return None;
    }
    if hh > 23 || mi > 59 || ss > 59 {
        return None;
    }
    let secs = days_from_civil(y, mo, d) * 86_400 + hh * 3600 + mi * 60 + ss - off * 60;
    let (days, sod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y2, m2, d2) = civil_from_days(days);
    if !(1..=9999).contains(&y2) {
        return None;
    }
    let frac = frac.trim_end_matches('0');
    let mut out = format!(
        "{y2:04}-{m2:02}-{d2:02}T{:02}:{:02}:{:02}",
        sod / 3600,
        sod / 60 % 60,
        sod % 60
    );
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    out.push('Z');
    Some(out)
}

// ------------------------------------------------------------------ validation

fn ptr(parts: &[&dyn fmt::Display]) -> String {
    parts
        .iter()
        .map(|p| format!("/{}", p.to_string().replace('~', "~0").replace('/', "~1")))
        .collect()
}

fn join(path: &str, key: &str) -> String {
    format!("{path}{}", ptr(&[&key]))
}

fn has_float(v: &Value) -> bool {
    match v {
        Value::Number(n) => is_float(n),
        Value::Array(a) => a.iter().any(has_float),
        Value::Object(m) => m.values().any(has_float),
        _ => false,
    }
}

fn big_int(v: &Value) -> bool {
    match v {
        Value::Number(n) => {
            n.as_i64()
                .map(|i| i.unsigned_abs() > MAX_SAFE_INT as u64)
                .unwrap_or(false)
                || n.as_u64().map(|u| u > MAX_SAFE_INT as u64).unwrap_or(false)
        }
        Value::Array(a) => a.iter().any(big_int),
        Value::Object(m) => m.values().any(big_int),
        _ => false,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Str,
    Int,
    Num,
    Obj,
    List,
    Any,
}

#[derive(Default)]
struct Ctx {
    errors: BTreeSet<Issue>,
}

type Obj = Map<String, Value>;

impl Ctx {
    fn err(&mut self, code: &str, path: String) {
        self.errors.insert(Issue::new(path, code));
    }

    /// Missing, then type, then float, then range. `Err(())` = reported; `Ok(None)` =
    /// absent or null where allowed.
    fn field<'a>(
        &mut self,
        obj: &'a Obj,
        key: &str,
        path: &str,
        kind: Kind,
        required: bool,
        nullable: bool,
    ) -> Result<Option<&'a Value>, ()> {
        let p = join(path, key);
        let Some(v) = obj.get(key) else {
            if required {
                self.err("E_MISSING", p);
                return Err(());
            }
            return Ok(None);
        };
        if v.is_null() && nullable {
            return Ok(None);
        }
        let bad = |c: &mut Ctx, code| {
            c.err(code, p.clone());
            Err(())
        };
        match kind {
            Kind::Str if !v.is_string() => bad(self, "E_TYPE"),
            Kind::Int => match v {
                Value::Number(n) if is_float(n) => bad(self, "E_FLOAT"),
                Value::Number(_) if big_int(v) => bad(self, "E_RANGE"),
                Value::Number(_) => Ok(Some(v)),
                _ => bad(self, "E_TYPE"),
            },
            Kind::Num if !v.is_number() => bad(self, "E_TYPE"),
            Kind::Obj if !v.is_object() => bad(self, "E_TYPE"),
            Kind::List if !v.is_array() => bad(self, "E_TYPE"),
            Kind::Any if has_float(v) => bad(self, "E_FLOAT"),
            Kind::Any if big_int(v) => bad(self, "E_RANGE"),
            _ => Ok(Some(v)),
        }
    }

    fn string(&mut self, obj: &Obj, key: &str, path: &str) -> Option<String> {
        self.field(obj, key, path, Kind::Str, true, false)
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(str::to_string))
    }

    fn unknown(&mut self, obj: &Obj, allowed: &[&str], path: &str) {
        for k in obj.keys() {
            if !allowed.contains(&k.as_str()) {
                self.err("E_UNKNOWN_FIELD", join(path, k));
            }
        }
    }

    fn time(&mut self, obj: &Obj, key: &str, path: &str) -> Option<String> {
        let s = self.string(obj, key, path)?;
        let t = canonical_ts(&s);
        if t.is_none() {
            self.err("E_TIME", join(path, key));
        }
        t
    }

    fn one_of(
        &mut self,
        obj: &Obj,
        key: &str,
        path: &str,
        allowed: &[&str],
        code: &str,
    ) -> Option<String> {
        let s = self.string(obj, key, path)?;
        if !allowed.contains(&s.as_str()) {
            self.err(code, join(path, key));
            return None;
        }
        Some(s)
    }
}

fn opt_str(v: Option<String>) -> Value {
    v.map(Value::String).unwrap_or(Value::Null)
}

fn python_trace(c: &mut Ctx, t: &Value, path: &str) -> Option<Obj> {
    let Some(t) = t.as_object() else {
        c.err("E_TYPE", path.to_string());
        return None;
    };
    let mut allowed = TRACE_FIELDS.to_vec();
    allowed.push("ext");
    c.unknown(t, &allowed, path);
    let mut out = Obj::new();
    let key = c.string(t, "nat_key", path);
    if key.as_deref() == Some("") {
        c.err("E_RANGE", join(path, "nat_key"));
    }
    out.insert("nat_key".into(), opt_str(key));
    for k in ["snippet", "source"] {
        let v = c.string(t, k, path);
        out.insert(k.into(), opt_str(v));
    }
    for k in ["time", "first_seen", "last_seen"] {
        let v = c.time(t, k, path);
        out.insert(k.into(), opt_str(v));
    }
    let v = c.one_of(t, "status", path, &STATUSES, "E_STATUS");
    out.insert("status".into(), opt_str(v));
    let v = c.one_of(t, "kind", path, &KINDS, "E_KIND");
    out.insert("kind".into(), opt_str(v));
    let v = c.one_of(t, "lock", path, &LOCKS, "E_LOCK");
    out.insert("lock".into(), opt_str(v));
    for k in NULLABLE {
        let v = c
            .field(t, k, path, Kind::Str, false, true)
            .ok()
            .flatten()
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        out.insert(k.into(), opt_str(v));
    }
    let seen = c.field(t, "seen_count", path, Kind::Int, true, false);
    if let Ok(Some(v)) = seen {
        if v.as_i64().is_none_or(|n| n < 1) {
            c.err("E_RANGE", join(path, "seen_count"));
        }
        out.insert("seen_count".into(), v.clone());
    } else {
        out.insert("seen_count".into(), Value::Null);
    }
    if t.contains_key("ext") {
        if let Ok(Some(ext)) = c.field(t, "ext", path, Kind::Obj, true, false) {
            let ext = ext.as_object().expect("object");
            let p = join(path, "ext");
            let e = trace_ext(c, ext, &p);
            if !e.is_empty() {
                out.insert("ext".into(), Value::Object(e));
            }
        }
    }
    Some(out)
}

fn trace_ext(c: &mut Ctx, ext: &Obj, path: &str) -> Obj {
    c.unknown(
        ext,
        &["coherence_milli", "content", "kind", "metadata"],
        path,
    );
    let mut out = Obj::new();
    if let Ok(Some(v)) = c.field(ext, "coherence_milli", path, Kind::Int, false, false) {
        if v.as_i64().is_none_or(|n| !(0..=1000).contains(&n)) {
            c.err("E_RANGE", join(path, "coherence_milli"));
        }
        out.insert("coherence_milli".into(), v.clone());
    }
    if ext.contains_key("content") {
        if let Ok(Some(v)) = c.field(ext, "content", path, Kind::Any, true, false) {
            out.insert("content".into(), v.clone());
        }
    }
    if let Ok(Some(v)) = c.field(ext, "kind", path, Kind::Str, false, false) {
        out.insert("kind".into(), v.clone());
    }
    if let Ok(Some(v)) = c.field(ext, "metadata", path, Kind::Obj, false, false) {
        if c.field(ext, "metadata", path, Kind::Any, true, false)
            .is_ok()
        {
            out.insert("metadata".into(), v.clone());
        }
    }
    out
}

fn python_event(c: &mut Ctx, e: &Value, path: &str) -> Option<Obj> {
    let Some(e) = e.as_object() else {
        c.err("E_TYPE", path.to_string());
        return None;
    };
    c.unknown(
        e,
        &["correlation_id", "payload", "source", "ts", "ext"],
        path,
    );
    let mut out = Obj::new();
    let corr = c
        .field(e, "correlation_id", path, Kind::Str, false, true)
        .ok()
        .flatten()
        .cloned()
        .unwrap_or(Value::Null);
    out.insert("correlation_id".into(), corr);
    let v = c.string(e, "payload", path);
    out.insert("payload".into(), opt_str(v));
    let v = c.string(e, "source", path);
    out.insert("source".into(), opt_str(v));
    let v = c.time(e, "ts", path);
    out.insert("ts".into(), opt_str(v));
    if e.contains_key("ext") {
        if let Ok(Some(ext)) = c.field(e, "ext", path, Kind::Obj, true, false) {
            let ext = ext.as_object().expect("object");
            let p = join(path, "ext");
            c.unknown(ext, &["kind"], &p);
            if let Ok(Some(k)) = c.field(ext, "kind", &p, Kind::Str, false, false) {
                let mut m = Obj::new();
                m.insert("kind".into(), k.clone());
                out.insert("ext".into(), Value::Object(m));
            }
        }
    }
    Some(out)
}

fn rust_trace(c: &mut Ctx, t: &Value, path: &str) -> Option<Obj> {
    let Some(t) = t.as_object() else {
        c.err("E_TYPE", path.to_string());
        return None;
    };
    c.unknown(
        t,
        &[
            "nat_key",
            "ts",
            "kind",
            "content",
            "coherence",
            "agent_id",
            "metadata",
            "admitted_at",
        ],
        path,
    );
    let key = c.string(t, "nat_key", path);
    if key.as_deref() == Some("") {
        c.err("E_RANGE", join(path, "nat_key"));
    }
    let time = c.time(t, "ts", path);
    let admitted = c.time(t, "admitted_at", path);
    let kind = c.string(t, "kind", path);
    let source = c.string(t, "agent_id", path);
    let mut ext = Obj::new();
    ext.insert("kind".into(), opt_str(kind));
    let mut snippet = None;
    if let Ok(Some(content)) = c.field(t, "content", path, Kind::Any, true, false) {
        match content {
            Value::String(s) => snippet = Some(s.clone()),
            other => {
                snippet = canonical_json(other).ok();
                ext.insert("content".into(), other.clone());
            }
        }
    }
    if let Ok(Some(coh)) = c.field(t, "coherence", path, Kind::Num, true, false) {
        let x = coh.as_f64().unwrap_or(f64::NAN);
        let milli = (x * 1000.0 + 0.5).floor();
        if !(0.0..=1000.0).contains(&milli) {
            c.err("E_RANGE", join(path, "coherence"));
        } else {
            ext.insert("coherence_milli".into(), Value::from(milli as i64));
        }
    }
    if t.contains_key("metadata") {
        if let Ok(Some(meta)) = c.field(t, "metadata", path, Kind::Obj, true, false) {
            if c.field(t, "metadata", path, Kind::Any, true, false).is_ok()
                && !meta.as_object().expect("object").is_empty()
            {
                ext.insert("metadata".into(), meta.clone());
            }
        }
    }
    let admitted = opt_str(admitted);
    let mut out = Obj::new();
    out.insert("nat_key".into(), opt_str(key));
    out.insert("snippet".into(), opt_str(snippet));
    out.insert("source".into(), opt_str(source));
    out.insert("time".into(), opt_str(time));
    out.insert("status".into(), "unknown".into());
    out.insert("kind".into(), "memory".into());
    out.insert("lock".into(), "open".into());
    for k in NULLABLE {
        out.insert(k.into(), Value::Null);
    }
    out.insert("seen_count".into(), 1.into());
    out.insert("first_seen".into(), admitted.clone());
    out.insert("last_seen".into(), admitted);
    out.insert("ext".into(), Value::Object(ext));
    Some(out)
}

fn rust_event(c: &mut Ctx, e: &Value, path: &str) -> Option<Obj> {
    let Some(e) = e.as_object() else {
        c.err("E_TYPE", path.to_string());
        return None;
    };
    c.unknown(e, &["nat_key", "ts", "kind", "payload"], path);
    let key = c.string(e, "nat_key", path);
    let ts = c.time(e, "ts", path);
    let kind = c.string(e, "kind", path);
    let payload = match c.field(e, "payload", path, Kind::Any, true, false) {
        Ok(Some(p)) => canonical_json(p).ok(),
        _ => None,
    };
    let mut ext = Obj::new();
    ext.insert("kind".into(), opt_str(kind));
    let mut out = Obj::new();
    out.insert("correlation_id".into(), opt_str(key));
    out.insert("payload".into(), opt_str(payload));
    out.insert("source".into(), "ensembly".into());
    out.insert("ts".into(), opt_str(ts));
    out.insert("ext".into(), Value::Object(ext));
    Some(out)
}

struct Analysis {
    doc: Option<Value>,
    errors: Vec<Issue>,
    warnings: Vec<Issue>,
    traces: usize,
    archive: usize,
    source_format: Option<String>,
}

fn analyze(v: &Value) -> Analysis {
    let mut res = Analysis {
        doc: None,
        errors: Vec::new(),
        warnings: Vec::new(),
        traces: 0,
        archive: 0,
        source_format: None,
    };
    let Some(top) = v.as_object() else {
        res.errors = vec![Issue::new("/format", "E_FORMAT")];
        return res;
    };
    let fmt = top.get("format").and_then(Value::as_str);
    res.source_format = fmt.map(str::to_string);
    let rust = fmt.is_some_and(|f| RUST_FORMATS.contains(&f));
    if fmt != Some(CANON_FORMAT) && !rust {
        res.errors = vec![Issue::new("/format", "E_FORMAT")];
        return res;
    }
    let (tkey, akey, hkey) = if rust {
        ("memory_traces", "archive_events", "source_host")
    } else {
        ("traces", "archive", "host")
    };
    res.traces = top.get(tkey).and_then(Value::as_array).map_or(0, Vec::len);
    res.archive = top.get(akey).and_then(Value::as_array).map_or(0, Vec::len);

    let mut c = Ctx::default();
    let mut doc = Obj::new();
    doc.insert("format".into(), CANON_FORMAT.into());
    if rust {
        c.unknown(
            top,
            &[
                "format",
                "exported_at",
                "source_host",
                "pack_hash",
                "memory_hash",
                "memory_traces",
                "archive_events",
                "memory_crdt",
            ],
            "",
        );
        let ph = c.string(top, "pack_hash", "");
        let mh = c
            .field(top, "memory_hash", "", Kind::Str, false, true)
            .ok()
            .flatten()
            .cloned()
            .unwrap_or(Value::Null);
        let mut ext = Obj::new();
        ext.insert("memory_hash".into(), mh);
        ext.insert("source_format".into(), opt_str(res.source_format.clone()));
        ext.insert("source_pack_hash".into(), opt_str(ph));
        doc.insert("ext".into(), Value::Object(ext));
    } else {
        c.unknown(
            top,
            &["format", "exported_at", "host", "traces", "archive", "ext"],
            "",
        );
        if top.contains_key("ext") {
            if let Ok(Some(ext)) = c.field(top, "ext", "", Kind::Obj, true, false) {
                let ext = ext.as_object().expect("object");
                c.unknown(
                    ext,
                    &["memory_hash", "source_format", "source_pack_hash"],
                    "/ext",
                );
                let mut e = Obj::new();
                if let Ok(x) = c.field(ext, "memory_hash", "/ext", Kind::Str, false, true) {
                    if ext.contains_key("memory_hash") {
                        e.insert("memory_hash".into(), x.cloned().unwrap_or(Value::Null));
                    }
                }
                for k in ["source_format", "source_pack_hash"] {
                    if let Ok(Some(x)) = c.field(ext, k, "/ext", Kind::Str, false, false) {
                        e.insert(k.into(), x.clone());
                    }
                }
                if !e.is_empty() {
                    doc.insert("ext".into(), Value::Object(e));
                }
            }
        }
    }
    let exported = c.time(top, "exported_at", "");
    doc.insert("exported_at".into(), opt_str(exported));
    let host = c.string(top, hkey, "");
    doc.insert("host".into(), opt_str(host.clone()));

    let mut traces: Vec<Option<Obj>> = Vec::new();
    if let Ok(Some(list)) = c.field(top, tkey, "", Kind::List, true, false) {
        for (i, t) in list.as_array().expect("array").iter().enumerate() {
            let p = ptr(&[&tkey, &i]);
            traces.push(if rust {
                rust_trace(&mut c, t, &p)
            } else {
                python_trace(&mut c, t, &p)
            });
        }
    }
    let mut events: Vec<Option<Obj>> = Vec::new();
    if top.contains_key(akey) {
        if let Ok(Some(list)) = c.field(top, akey, "", Kind::List, true, false) {
            for (i, e) in list.as_array().expect("array").iter().enumerate() {
                let p = ptr(&[&akey, &i]);
                events.push(if rust {
                    rust_event(&mut c, e, &p)
                } else {
                    python_event(&mut c, e, &p)
                });
            }
        }
    }

    let mut seen = BTreeSet::new();
    for (i, t) in traces.iter().enumerate() {
        let Some(t) = t else { continue };
        if let Some(k) = t["nat_key"].as_str().filter(|k| !k.is_empty()) {
            if !seen.insert(k.to_string()) {
                c.err("E_DUP_KEY", ptr(&[&tkey, &i, &"nat_key"]));
            }
        }
        if t["status"] == "tool-verified" {
            if let Some(s) = t["snippet"].as_str() {
                if t["verify_hash"].as_str() != Some(content_hash(s).as_str()) {
                    c.err("E_EVIDENCE", ptr(&[&tkey, &i, &"verify_hash"]));
                }
            }
        }
    }
    if !c.errors.is_empty() {
        res.errors = c.errors.into_iter().collect();
        return res;
    }

    let mut warnings = BTreeSet::new();
    if !host.as_deref().is_some_and(|h| HOST_ALIASES.contains(&h)) {
        warnings.insert(Issue::new(ptr(&[&hkey]), "W_HOST_ALIAS"));
    }
    let mut traces: Vec<Obj> = traces.into_iter().flatten().collect();
    for (i, t) in traces.iter().enumerate() {
        if let Some(by) = t["superseded_by"].as_str() {
            if !seen.contains(by) {
                warnings.insert(Issue::new(
                    ptr(&[&tkey, &i, &"superseded_by"]),
                    "W_DANGLING_SUPERSEDED",
                ));
            }
        }
        if let (Some(h), Some(s)) = (t["verify_hash"].as_str(), t["snippet"].as_str()) {
            if t["status"] != "tool-verified" && h != content_hash(s) {
                warnings.insert(Issue::new(
                    ptr(&[&tkey, &i, &"verify_hash"]),
                    "W_VERIFY_MISMATCH",
                ));
            }
        }
    }
    traces.sort_by(|a, b| {
        let k = |t: &Obj| {
            t["nat_key"]
                .as_str()
                .unwrap_or_default()
                .as_bytes()
                .to_vec()
        };
        k(a).cmp(&k(b))
    });
    let mut events: Vec<(String, Obj)> = events
        .into_iter()
        .flatten()
        .map(|e| {
            let key = canonical_json(&Value::Object(e.clone())).expect("no floats");
            (key, e)
        })
        .collect();
    events.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    doc.insert(
        "traces".into(),
        Value::Array(traces.into_iter().map(Value::Object).collect()),
    );
    doc.insert(
        "archive".into(),
        Value::Array(events.into_iter().map(|(_, e)| Value::Object(e)).collect()),
    );
    res.doc = Some(Value::Object(doc));
    res.warnings = warnings.into_iter().collect();
    res
}

fn verdict_of(res: Analysis) -> (Option<Value>, Verdict) {
    let ok = res.errors.is_empty();
    let warn = ok && !res.warnings.is_empty();
    let doc = if ok { res.doc } else { None };
    let v = Verdict {
        archive: res.archive,
        canon: CANON_VERSION,
        content_sha256: doc.as_ref().map(content_sha256),
        errors: res.errors,
        exit: if !ok {
            2
        } else if warn {
            1
        } else {
            0
        },
        format: ok.then(|| CANON_FORMAT.to_string()),
        pack_sha256: doc.as_ref().map(pack_sha256),
        source_format: res.source_format,
        traces: res.traces,
        verdict: if !ok {
            "fail"
        } else if warn {
            "warn"
        } else {
            "ok"
        }
        .to_string(),
        warnings: if ok { res.warnings } else { Vec::new() },
    };
    (doc, v)
}

fn json_error_verdict() -> Verdict {
    Verdict {
        archive: 0,
        canon: CANON_VERSION,
        content_sha256: None,
        errors: vec![Issue::new("", "E_JSON")],
        exit: 2,
        format: None,
        pack_sha256: None,
        source_format: None,
        traces: 0,
        verdict: "fail".into(),
        warnings: Vec::new(),
    }
}

/// Validate and canonicalize a value already parsed (or built from a store).
pub fn canonicalize_value(v: &Value) -> (Option<Value>, Verdict) {
    verdict_of(analyze(v))
}

/// Parse strictly, validate and canonicalize. Returns the canonical doc (on ok or warn)
/// and the verdict.
pub fn check_bytes(input: &[u8]) -> (Option<Value>, Verdict) {
    match parse_strict(input) {
        Ok(v) => canonicalize_value(&v),
        Err(_) => (None, json_error_verdict()),
    }
}

/// The verdict alone.
pub fn check_pack(input: &[u8]) -> Verdict {
    check_bytes(input).1
}

/// The canonical document, or the errors.
pub fn canonicalize(input: &[u8]) -> Result<Value, Vec<Issue>> {
    let (doc, v) = check_bytes(input);
    doc.ok_or(v.errors)
}

fn issues(xs: &[Issue]) -> Value {
    Value::Array(
        xs.iter()
            .map(|i| serde_json::json!({"code": i.code, "path": i.path}))
            .collect(),
    )
}

/// Verdict as canonical bytes (the golden `check.json`).
pub fn verdict_bytes(v: &Verdict) -> Vec<u8> {
    let mut m = Obj::new();
    m.insert("archive".into(), v.archive.into());
    m.insert("canon".into(), v.canon.into());
    m.insert("content_sha256".into(), opt_str(v.content_sha256.clone()));
    m.insert("errors".into(), issues(&v.errors));
    m.insert("exit".into(), v.exit.into());
    m.insert("format".into(), opt_str(v.format.clone()));
    m.insert("pack_sha256".into(), opt_str(v.pack_sha256.clone()));
    m.insert("source_format".into(), opt_str(v.source_format.clone()));
    m.insert("traces".into(), v.traces.into());
    m.insert("verdict".into(), v.verdict.clone().into());
    m.insert("warnings".into(), issues(&v.warnings));
    canonical_bytes(&Value::Object(m)).expect("verdict has no floats")
}

// ------------------------------------------------------------------ store

/// Build the Python-shape document from a store opened with
/// [`crate::pulse_core::open_ro`]. Run it through [`canonicalize_value`].
pub fn from_db(
    conn: &rusqlite::Connection,
    exported_at: &str,
    host: &str,
) -> rusqlite::Result<Value> {
    let sql = format!(
        "SELECT {} FROM memory_traces ORDER BY id",
        TRACE_FIELDS.join(", ")
    );
    let mut st = conn.prepare(&sql)?;
    let mut traces = Vec::new();
    let mut rows = st.query([])?;
    while let Some(r) = rows.next()? {
        let mut t = Obj::new();
        for (i, k) in TRACE_FIELDS.iter().enumerate() {
            let v = sql_value(r.get_ref(i)?);
            if v.is_null() && NULLABLE.contains(k) {
                continue;
            }
            t.insert((*k).into(), v);
        }
        traces.push(Value::Object(t));
    }
    let has_archive: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='archive_events'",
        [],
        |r| r.get(0),
    )?;
    let mut archive = Vec::new();
    if has_archive > 0 {
        let mut st = conn.prepare(
            "SELECT ts, source, payload, correlation_id FROM archive_events ORDER BY id",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let mut e = Obj::new();
            for (i, k) in ["ts", "source", "payload", "correlation_id"]
                .iter()
                .enumerate()
            {
                e.insert((*k).into(), sql_value(r.get_ref(i)?));
            }
            archive.push(Value::Object(e));
        }
    }
    let mut doc = Obj::new();
    doc.insert("format".into(), CANON_FORMAT.into());
    doc.insert("exported_at".into(), exported_at.into());
    doc.insert("host".into(), host.into());
    doc.insert("traces".into(), Value::Array(traces));
    doc.insert("archive".into(), Value::Array(archive));
    Ok(Value::Object(doc))
}

fn sql_value(v: rusqlite::types::ValueRef<'_>) -> Value {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => Number::from_f64(f).map_or(Value::Null, Value::Number),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        // A blob is never a valid field; a list makes it an E_TYPE.
        ValueRef::Blob(b) => Value::Array(b.iter().map(|x| Value::from(*x)).collect()),
    }
}

/// Canonical doc from a store path, opened read-only (`mode=ro` + `query_only`).
pub fn canonical_from_db_path(
    path: &Path,
    exported_at: &str,
    host: &str,
) -> anyhow::Result<(Option<Value>, Verdict)> {
    let conn = crate::pulse_core::open_ro(path)?;
    let doc = from_db(&conn, exported_at, host)?;
    Ok(canonicalize_value(&doc))
}
