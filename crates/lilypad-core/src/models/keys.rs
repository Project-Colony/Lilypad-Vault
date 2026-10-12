//! Key metadata and the key derivation parameters embedded in a vault.

use serde::{Deserialize, Serialize};

use crate::crypto::{CryptoAlgorithm, KeyMaterial};

/// Full KDF parameters embedded in the vault file (V2+).
/// This allows the vault to be self-contained: a new device can derive the
/// encryption key from the master password without needing a separate key.json file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbeddedKdfParams {
    pub algorithm: String,
    pub salt: [u8; 16],
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl EmbeddedKdfParams {
    /// Creates embedded params from the existing `KeyDerivationParams`.
    pub fn from_kdf_params(params: &crate::crypto::KeyDerivationParams) -> Self {
        Self {
            algorithm: "argon2id".to_string(),
            salt: params.salt,
            memory_kib: params.memory_kib,
            iterations: params.iterations,
            parallelism: params.parallelism,
        }
    }

    /// Converts back to `KeyDerivationParams` for key derivation.
    pub fn to_kdf_params(&self) -> crate::crypto::KeyDerivationParams {
        crate::crypto::KeyDerivationParams {
            salt: self.salt,
            memory_kib: self.memory_kib,
            iterations: self.iterations,
            parallelism: self.parallelism,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyMetadata {
    pub key_id: String,
    pub algorithm: CryptoAlgorithm,
    #[serde(default)]
    pub kdf: Option<String>,
    /// Full KDF parameters embedded in the vault (V2+).
    /// When present, the vault is self-contained and can be decrypted
    /// on any device with just the master password.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kdf_params: Option<EmbeddedKdfParams>,
}

impl KeyMetadata {
    pub fn new(key: &KeyMaterial, algorithm: CryptoAlgorithm) -> Self {
        Self {
            key_id: key.key_id(),
            algorithm,
            kdf: None,
            kdf_params: None,
        }
    }

    pub fn with_kdf(mut self, kdf: impl Into<String>) -> Self {
        self.kdf = Some(kdf.into());
        self
    }

    /// Embeds full KDF parameters in the metadata, making the vault self-contained (V2).
    pub fn with_embedded_kdf(mut self, params: &crate::crypto::KeyDerivationParams) -> Self {
        self.kdf = Some("argon2id".to_string());
        self.kdf_params = Some(EmbeddedKdfParams::from_kdf_params(params));
        self
    }
}
