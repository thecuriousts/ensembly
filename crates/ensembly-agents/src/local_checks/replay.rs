//! Recorded synthetic outputs used by CI. Live model processes stay behind
//! `ENSEMBLY_LOCAL_MODELS=1`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::local_checks::runtime::{
    CheckFailure, CheckResult, CheckSource, CheckSuccess, FallbackReason, RuntimeKind,
};
use crate::local_checks::schema::{
    parse_conflict_class, parse_fact_class, ConflictClass, FactClass,
};

pub const FIXTURE_ROOT: &str = "fixtures/local-checks";
pub const MANIFEST_NAME: &str = "MANIFEST.sha256";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClassifyFixture {
    pub id: String,
    pub statement: String,
    pub recorded: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictFixture {
    pub id: String,
    pub left: String,
    pub right: String,
    pub recorded: Value,
}

#[derive(Debug, Clone)]
pub struct RecordedSet {
    pub root: PathBuf,
    pub classify: Vec<ClassifyFixture>,
    pub conflict: Vec<ConflictFixture>,
}

impl RecordedSet {
    pub fn load(root: &Path) -> Result<Self, String> {
        let classify = load_many::<ClassifyFixture>(&root.join("classify"))?;
        let conflict = load_many::<ConflictFixture>(&root.join("conflict"))?;
        if classify.is_empty() && conflict.is_empty() {
            return Err(format!("no recorded fixtures under {}", root.display()));
        }
        Ok(Self {
            root: root.to_path_buf(),
            classify,
            conflict,
        })
    }

    pub fn classify(&self, statement: &str) -> CheckResult<FactClass> {
        let started = Instant::now();
        let fixture = self
            .classify
            .iter()
            .find(|f| f.statement == statement)
            .ok_or_else(|| CheckFailure {
                reason: FallbackReason::ModelMissing,
                detail: "no recorded classify fixture for that statement".into(),
                latency: started.elapsed(),
                raw: None,
            })?;
        finish_fact(fixture, started)
    }

    pub fn classify_by_id(&self, id: &str) -> CheckResult<FactClass> {
        let started = Instant::now();
        let fixture = self
            .classify
            .iter()
            .find(|f| f.id == id)
            .ok_or_else(|| CheckFailure {
                reason: FallbackReason::ModelMissing,
                detail: format!("no recorded classify fixture `{id}`"),
                latency: started.elapsed(),
                raw: None,
            })?;
        finish_fact(fixture, started)
    }

    pub fn conflict(&self, left: &str, right: &str) -> CheckResult<ConflictClass> {
        let started = Instant::now();
        let fixture = self
            .conflict
            .iter()
            .find(|f| f.left == left && f.right == right)
            .ok_or_else(|| CheckFailure {
                reason: FallbackReason::ModelMissing,
                detail: "no recorded conflict fixture for those statements".into(),
                latency: started.elapsed(),
                raw: None,
            })?;
        finish_conflict(fixture, started)
    }

    pub fn conflict_by_id(&self, id: &str) -> CheckResult<ConflictClass> {
        let started = Instant::now();
        let fixture = self
            .conflict
            .iter()
            .find(|f| f.id == id)
            .ok_or_else(|| CheckFailure {
                reason: FallbackReason::ModelMissing,
                detail: format!("no recorded conflict fixture `{id}`"),
                latency: started.elapsed(),
                raw: None,
            })?;
        finish_conflict(fixture, started)
    }

    /// Canonical SHA-256 over every fixture file name and body, in path order.
    pub fn digest(&self) -> Result<String, String> {
        use std::collections::BTreeMap;
        let mut files = BTreeMap::new();
        collect_files(&self.root.join("classify"), &mut files)?;
        collect_files(&self.root.join("conflict"), &mut files)?;
        let mut hasher = Sha256::new();
        for (name, body) in files {
            hasher.update(name.as_bytes());
            hasher.update(&[0u8]);
            hasher.update(&body);
            hasher.update(&[0u8]);
        }
        Ok(hasher.hexdigest())
    }
}

fn finish_fact(fixture: &ClassifyFixture, started: Instant) -> CheckResult<FactClass> {
    let raw = serde_json::to_string(&fixture.recorded)
        .map_err(|e| fail(started, format!("recorded fact JSON: {e}")))?;
    let value = parse_fact_class(&raw).map_err(|e| fail(started, e))?;
    Ok(success(value, fixture.id.clone(), raw, started))
}

fn finish_conflict(fixture: &ConflictFixture, started: Instant) -> CheckResult<ConflictClass> {
    let raw = serde_json::to_string(&fixture.recorded)
        .map_err(|e| fail(started, format!("recorded conflict JSON: {e}")))?;
    let value = parse_conflict_class(&raw).map_err(|e| fail(started, e))?;
    Ok(success(value, fixture.id.clone(), raw, started))
}

fn success<T>(value: T, model: String, raw: String, started: Instant) -> CheckSuccess<T> {
    CheckSuccess {
        value,
        source: CheckSource {
            runtime: RuntimeKind::Replay,
            model,
            fallback: None,
        },
        latency: started.elapsed(),
        raw,
    }
}

fn fail(started: Instant, detail: String) -> CheckFailure {
    CheckFailure {
        reason: FallbackReason::InvalidJson,
        detail,
        latency: started.elapsed(),
        raw: None,
    }
}

fn load_many<T: for<'de> Deserialize<'de>>(dir: &Path) -> Result<Vec<T>, String> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    names.sort();
    let mut out = Vec::new();
    for path in names {
        let text =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let item: T =
            serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
        out.push(item);
    }
    Ok(out)
}

fn collect_files(
    dir: &Path,
    into: &mut std::collections::BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut names: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    names.sort();
    for path in names {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("bad fixture name {}", path.display()))?
            .to_string();
        let body = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        into.insert(name, body);
    }
    Ok(())
}

/// Tiny SHA-256 (no extra crate). Fixtures are small.
struct Sha256 {
    state: [u32; 8],
    buffer: Vec<u8>,
    len: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: Vec::new(),
            len: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.len += data.len() as u64;
        self.buffer.extend_from_slice(data);
        while self.buffer.len() >= 64 {
            let chunk: [u8; 64] = self.buffer[..64].try_into().unwrap();
            self.buffer.drain(..64);
            self.compress(chunk);
        }
    }

    fn hexdigest(mut self) -> String {
        let bit_len = self.len * 8;
        self.buffer.push(0x80);
        while (self.buffer.len() % 64) != 56 {
            self.buffer.push(0);
        }
        self.buffer.extend_from_slice(&bit_len.to_be_bytes());
        while !self.buffer.is_empty() {
            let chunk: [u8; 64] = self.buffer[..64].try_into().unwrap();
            self.buffer.drain(..64);
            self.compress(chunk);
        }
        let mut out = String::with_capacity(64);
        for word in self.state {
            out.push_str(&format!("{word:08x}"));
        }
        out
    }

    fn compress(&mut self, chunk: [u8; 64]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

pub fn fixture_root_from(cwd: &Path) -> PathBuf {
    if let Ok(explicit) = std::env::var("ENSEMBLY_LOCAL_CHECKS_FIXTURES") {
        return PathBuf::from(explicit);
    }
    let candidate = cwd.join(FIXTURE_ROOT);
    if candidate.is_dir() {
        return candidate;
    }
    cwd.join(FIXTURE_ROOT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vector() {
        let mut h = Sha256::new();
        h.update(b"abc");
        assert_eq!(
            h.hexdigest(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
