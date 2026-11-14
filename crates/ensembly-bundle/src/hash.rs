use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

pub struct HashResult {
    pub sha256: String,
    pub bytes: u64,
}

/// Stream file contents through SHA-256 without loading whole file into memory.
pub fn hash_file(path: &Path) -> Result<HashResult, std::io::Error> {
    let mut file = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    Ok(HashResult {
        sha256: hex::encode(hasher.finalize()),
        bytes: total,
    })
}

pub fn hash_bytes(data: &[u8]) -> HashResult {
    let mut hasher = Sha256::new();
    hasher.update(data);
    HashResult {
        sha256: hex::encode(hasher.finalize()),
        bytes: data.len() as u64,
    }
}
