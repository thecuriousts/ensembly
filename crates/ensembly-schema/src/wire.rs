use crate::{Checked, SCHEMA_VERSION};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
pub struct Decoded<T> {
    pub version: u32,
    pub value: T,
}

#[derive(Serialize)]
struct Envelope<'a, T: Serialize> {
    #[serde(flatten)]
    body: &'a T,
    #[serde(skip_serializing_if = "Option::is_none")]
    v: Option<u32>,
}

/// Decode one JSON object line.
///
/// A missing `v` is version 0. The only accepted present value is [`SCHEMA_VERSION`].
/// An explicit `v` of 0 is refused, as is any other value. Duplicate keys are refused.
pub fn decode_line<T: DeserializeOwned + Checked>(line: &str) -> Result<Decoded<T>, String> {
    reject_duplicate_keys(line)?;
    let mut value: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    let version = take_version(value.as_object_mut())?;
    let value: T = serde_json::from_value(value).map_err(|e| e.to_string())?;
    value.check()?;
    Ok(Decoded { version, value })
}

fn take_version(
    obj: Option<&mut serde_json::Map<String, serde_json::Value>>,
) -> Result<u32, String> {
    let Some(obj) = obj else {
        return Err("line is not an object".into());
    };
    match obj.remove("v") {
        None => Ok(0),
        Some(serde_json::Value::Number(n)) if n.as_u64() == Some(u64::from(SCHEMA_VERSION)) => {
            Ok(SCHEMA_VERSION)
        }
        Some(seen) => Err(unsupported_version(&seen)),
    }
}

fn unsupported_version(seen: &serde_json::Value) -> String {
    format!(
        "unsupported v {seen}; supported versions: omitted or {SCHEMA_VERSION}; explicit v 0 is refused"
    )
}

pub fn encode_line<T: Serialize>(version: u32, value: &T) -> Result<String, String> {
    let v = match version {
        0 => None,
        SCHEMA_VERSION => Some(SCHEMA_VERSION),
        other => {
            return Err(format!(
                "unsupported v {other}; supported versions: omitted or {SCHEMA_VERSION}; explicit v 0 is refused"
            ));
        }
    };
    serde_json::to_string(&Envelope { body: value, v }).map_err(|e| e.to_string())
}

fn reject_duplicate_keys(line: &str) -> Result<(), String> {
    let mut scan = JsonScan { s: line, i: 0 };
    scan.value()?;
    scan.skip_ws();
    if scan.i != scan.s.len() {
        return Err("trailing data".into());
    }
    Ok(())
}

struct JsonScan<'a> {
    s: &'a str,
    i: usize,
}

impl JsonScan<'_> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s.as_bytes()[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn bump(&mut self) -> Result<u8, String> {
        let b = self
            .s
            .as_bytes()
            .get(self.i)
            .copied()
            .ok_or("truncated json")?;
        self.i += 1;
        Ok(b)
    }

    fn eat(&mut self, want: u8) -> bool {
        if self.s.as_bytes().get(self.i).copied() == Some(want) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, want: u8) -> Result<(), String> {
        let got = self.bump()?;
        if got == want {
            Ok(())
        } else {
            Err(format!("expected {}", want as char))
        }
    }

    fn value(&mut self) -> Result<(), String> {
        self.skip_ws();
        if self
            .s
            .as_bytes()
            .get(self.i..)
            .is_some_and(|b| b.starts_with(b"\xEF\xBB\xBF"))
        {
            return Err("byte-order mark".into());
        }
        let b = self.bump()?;
        self.value_byte(b)
    }

    fn value_byte(&mut self, b: u8) -> Result<(), String> {
        if b == b'-' || b.is_ascii_digit() {
            return self.number_rest();
        }
        match b {
            b'{' => self.object_after_brace(),
            b'[' => self.array_after_bracket(),
            b'"' => self.string_after_quote().map(|_| ()),
            b't' => self.literal("rue"),
            b'f' => self.literal("alse"),
            b'n' => self.literal("ull"),
            _ => Err("bad json value".into()),
        }
    }

    fn object_after_brace(&mut self) -> Result<(), String> {
        self.skip_ws();
        if self.eat(b'}') {
            return Ok(());
        }
        let mut seen = BTreeSet::new();
        loop {
            self.skip_ws();
            let key = self.string()?;
            if !seen.insert(key.clone()) {
                return Err(format!("duplicate key {key}"));
            }
            self.skip_ws();
            self.expect(b':')?;
            self.value()?;
            self.skip_ws();
            if self.eat(b'}') {
                return Ok(());
            }
            self.expect(b',')?;
        }
    }

    fn array_after_bracket(&mut self) -> Result<(), String> {
        self.skip_ws();
        if self.eat(b']') {
            return Ok(());
        }
        loop {
            self.value()?;
            self.skip_ws();
            if self.eat(b']') {
                return Ok(());
            }
            self.expect(b',')?;
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        self.string_after_quote()
    }

    fn string_after_quote(&mut self) -> Result<String, String> {
        let start = self.i - 1;
        loop {
            match self.bump()? {
                b'\\' => {
                    self.bump()?;
                }
                b'"' => break,
                _ => {}
            }
        }
        serde_json::from_str(&self.s[start..self.i]).map_err(|e| e.to_string())
    }

    fn literal(&mut self, rest: &str) -> Result<(), String> {
        if self.s[self.i..].starts_with(rest) {
            self.i += rest.len();
            Ok(())
        } else {
            Err("bad literal".into())
        }
    }

    fn number_rest(&mut self) -> Result<(), String> {
        while self.i < self.s.len() && is_number_byte(self.s.as_bytes()[self.i]) {
            self.i += 1;
        }
        Ok(())
    }
}

fn is_number_byte(b: u8) -> bool {
    b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-')
}
