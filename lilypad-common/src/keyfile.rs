//! Key file management for Lilypad vaults.
//!
//! This module handles loading and saving encryption keys, supporting both
//! raw keys (hex-encoded) and password-derived keys (using Argon2id KDF).

use anyhow::{anyhow, Context, Result};
use lilypad_core::{derive_key, KeyDerivationParams, KeyMaterial};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Represents the different types of key storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum KeyFile {
    /// A raw key stored as hexadecimal string.
    Raw { key_hex: String },
    /// A key derived from a master password using KDF parameters.
    Kdf { params: KeyDerivationParams },
}

impl KeyFile {
    /// Creates a new raw key file from key material.
    pub fn from_raw(key: &KeyMaterial) -> Self {
        KeyFile::Raw {
            key_hex: encode_hex(key.as_bytes()),
        }
    }

    /// Creates a new KDF-based key file.
    pub fn from_kdf(params: KeyDerivationParams) -> Self {
        KeyFile::Kdf { params }
    }

    /// Returns true if this key file requires a master password.
    pub fn requires_password(&self) -> bool {
        matches!(self, KeyFile::Kdf { .. })
    }
}

/// Saves a key file to disk with secure permissions.
///
/// On Unix systems, the file is created with mode 0o600 (read/write owner only).
pub fn save_key(path: &Path, key_file: &KeyFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(key_file)?;
    fs::write(path, &payload)?;

    // Set secure permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// Loads a key from a key file, optionally using a master password.
///
/// # Arguments
/// * `path` - Path to the key file
/// * `master_password` - Optional master password for KDF-based keys
///
/// # Returns
/// A tuple of (KeyMaterial, KeyFile) on success.
pub fn load_key(path: &Path, master_password: Option<&str>) -> Result<(KeyMaterial, KeyFile)> {
    let payload = fs::read(path).with_context(|| {
        format!(
            "key file not found: {} (run `lilypad init` first)",
            path.display()
        )
    })?;
    let key_file: KeyFile = serde_json::from_slice(&payload)?;
    let key = match &key_file {
        KeyFile::Raw { key_hex } => {
            let bytes = decode_hex(key_hex)?;
            KeyMaterial::from_bytes(&bytes).context("invalid key material")?
        }
        KeyFile::Kdf { params } => {
            let password = master_password
                .ok_or_else(|| anyhow!("master password required for this vault"))?;
            if password.trim().is_empty() {
                return Err(anyhow!("master password cannot be empty"));
            }
            derive_key(password, params).context("key derivation failed")?
        }
    };
    Ok((key, key_file))
}

/// Encodes bytes as a hexadecimal string.
pub fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Decodes a hexadecimal string into bytes.
pub fn decode_hex(hex: &str) -> Result<Vec<u8>> {
    let value = hex.trim();
    if value.is_empty() {
        return Err(anyhow!("key cannot be empty"));
    }
    if !value.len().is_multiple_of(2) {
        return Err(anyhow!("invalid hex string length"));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for chunk in value.as_bytes().chunks(2) {
        let chunk_str = std::str::from_utf8(chunk)?;
        let byte =
            u8::from_str_radix(chunk_str, 16).map_err(|_| anyhow!("invalid hex character"))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_encode_decode_hex() {
        let original = vec![0xde, 0xad, 0xbe, 0xef];
        let hex = encode_hex(&original);
        assert_eq!(hex, "deadbeef");
        let decoded = decode_hex(&hex).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_decode_hex_invalid() {
        assert!(decode_hex("").is_err());
        assert!(decode_hex("abc").is_err()); // odd length
        assert!(decode_hex("gg").is_err()); // invalid chars
    }

    #[test]
    fn test_save_load_raw_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("key.json");
        let key = KeyMaterial::generate();
        let key_file = KeyFile::from_raw(&key);

        save_key(&path, &key_file).unwrap();
        let (loaded_key, loaded_file) = load_key(&path, None).unwrap();

        assert_eq!(loaded_key.key_id(), key.key_id());
        assert!(!loaded_file.requires_password());
    }

    #[test]
    fn test_save_load_kdf_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("key.json");
        let params = KeyDerivationParams::generate();
        let password = "test-master-password-123!";
        let key = derive_key(password, &params).unwrap();
        let key_file = KeyFile::from_kdf(params);

        save_key(&path, &key_file).unwrap();
        let (loaded_key, loaded_file) = load_key(&path, Some(password)).unwrap();

        assert_eq!(loaded_key.key_id(), key.key_id());
        assert!(loaded_file.requires_password());
    }

    #[test]
    fn test_kdf_key_requires_password() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("key.json");
        let params = KeyDerivationParams::generate();
        let key_file = KeyFile::from_kdf(params);

        save_key(&path, &key_file).unwrap();
        assert!(load_key(&path, None).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_key_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let path = dir.path().join("key.json");
        let key = KeyMaterial::generate();
        let key_file = KeyFile::from_raw(&key);

        save_key(&path, &key_file).unwrap();

        let metadata = fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
