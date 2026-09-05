//! T2 vault bridge — AES-256-GCM sealed blobs.
//!
//! Law (peram-vault): high-sens SoT is ciphertext; ML-KEM-768 for KEM wrapping
//! of content keys in full vault product. This crate provides the **application
//! seal/unseal** used by kernel backup packs and a deny-without-key gate.
//! Full ML-KEM ceremony lives in peram-vault; we do not re-home crypto.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Algorithm suite advertised in sealed packs (matches peram-vault direction).
pub const VAULT_SUITE: &str = "AES-256-GCM+SHA256-KDF; KEM-law=ML-KEM-768 (peram-vault)";

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("unlock denied: missing or wrong key")]
    UnlockDenied,
    #[error("crypto: {0}")]
    Crypto(String),
    #[error("malformed sealed blob")]
    Malformed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedBlob {
    pub suite: String,
    pub nonce_hex: String,
    pub ciphertext_hex: String,
    /// SHA-256 of unlock material fingerprint (not the key) for operators.
    pub key_fingerprint: String,
}

/// Write-side KDF prefix. Kept on the legacy string so existing sealed
/// backups stay valid. Do not change without a versioned re-seal path.
const VAULT_KDF_WRITE: &[u8] = b"peram-kernel-t2-v1:";
const VAULT_KDF_READ: &[&[u8]] = &[b"peram-kernel-t2-v1:", b"ensembly-kernel-t2-v1:"];
const VAULT_FP_WRITE: &[u8] = b"peram-kernel-fp-v1:";

/// Derive a 32-byte content key from unlock material (passphrase / keyring secret).
pub fn derive_key(unlock: &[u8]) -> [u8; 32] {
    derive_key_with(VAULT_KDF_WRITE, unlock)
}

fn derive_key_with(prefix: &[u8], unlock: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(unlock);
    let out = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&out);
    key
}

pub fn key_fingerprint(unlock: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(VAULT_FP_WRITE);
    hasher.update(unlock);
    hex::encode(hasher.finalize())[..16].to_string()
}

/// Seal plaintext with unlock material. Without correct unlock later, unseal fails.
pub fn seal(plaintext: &[u8], unlock: &[u8]) -> Result<SealedBlob, VaultError> {
    if unlock.is_empty() {
        return Err(VaultError::UnlockDenied);
    }
    let key = derive_key(unlock);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| VaultError::Crypto(e.to_string()))?;
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| VaultError::Crypto(e.to_string()))?;
    Ok(SealedBlob {
        suite: VAULT_SUITE.into(),
        nonce_hex: hex::encode(nonce_bytes),
        ciphertext_hex: hex::encode(ct),
        key_fingerprint: key_fingerprint(unlock),
    })
}

/// Unseal. Wrong or empty key → UnlockDenied (stolen disk without key stays sealed).
/// Dual-read: try write-side prefix first, then the ensembly-named prefix.
pub fn unseal(blob: &SealedBlob, unlock: &[u8]) -> Result<Vec<u8>, VaultError> {
    if unlock.is_empty() {
        return Err(VaultError::UnlockDenied);
    }
    let nonce_bytes = hex::decode(&blob.nonce_hex).map_err(|_| VaultError::Malformed)?;
    let ct = hex::decode(&blob.ciphertext_hex).map_err(|_| VaultError::Malformed)?;
    if nonce_bytes.len() != 12 {
        return Err(VaultError::Malformed);
    }
    let nonce = Nonce::from_slice(&nonce_bytes);
    let mut last_crypto: Option<VaultError> = None;
    for prefix in VAULT_KDF_READ {
        let key = derive_key_with(prefix, unlock);
        let cipher =
            Aes256Gcm::new_from_slice(&key).map_err(|e| VaultError::Crypto(e.to_string()))?;
        match cipher.decrypt(nonce, ct.as_ref()) {
            Ok(pt) => return Ok(pt),
            Err(_) => last_crypto = Some(VaultError::UnlockDenied),
        }
    }
    Err(last_crypto.unwrap_or(VaultError::UnlockDenied))
}

/// Export-deny classifier for finance/medical/identity class (share IR).
pub fn export_denied_for_class(class: &str) -> bool {
    matches!(
        class.to_ascii_lowercase().as_str(),
        "finance" | "medical" | "identity" | "family_health" | "vault"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_unseal_roundtrip() {
        let blob = seal(b"high-sens persona", b"correct-horse").unwrap();
        let pt = unseal(&blob, b"correct-horse").unwrap();
        assert_eq!(pt, b"high-sens persona");
    }

    #[test]
    fn wrong_key_denied() {
        let blob = seal(b"secret", b"right").unwrap();
        let err = unseal(&blob, b"wrong").unwrap_err();
        assert!(matches!(err, VaultError::UnlockDenied));
    }

    #[test]
    fn empty_key_denied() {
        assert!(matches!(seal(b"x", b"").unwrap_err(), VaultError::UnlockDenied));
        let blob = seal(b"x", b"k").unwrap();
        assert!(matches!(
            unseal(&blob, b"").unwrap_err(),
            VaultError::UnlockDenied
        ));
    }

    #[test]
    fn finance_export_denied() {
        assert!(export_denied_for_class("Finance"));
        assert!(!export_denied_for_class("Systems"));
    }

    fn seal_with_prefix(prefix: &[u8], plaintext: &[u8], unlock: &[u8]) -> SealedBlob {
        let key = derive_key_with(prefix, unlock);
        let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
        let mut nonce_bytes = [0u8; 12];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ct = cipher.encrypt(nonce, plaintext).unwrap();
        SealedBlob {
            suite: VAULT_SUITE.into(),
            nonce_hex: hex::encode(nonce_bytes),
            ciphertext_hex: hex::encode(ct),
            key_fingerprint: key_fingerprint(unlock),
        }
    }

    #[test]
    fn unseal_accepts_ensembly_named_kdf_prefix() {
        let blob = seal_with_prefix(b"ensembly-kernel-t2-v1:", b"alt-seal", b"k");
        assert_eq!(unseal(&blob, b"k").unwrap(), b"alt-seal");
    }
}
