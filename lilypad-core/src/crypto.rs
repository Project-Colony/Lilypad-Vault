use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::errors::{CoreError, Result};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const SALT_LEN: usize = 16;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CryptoAlgorithm {
    XChaCha20Poly1305,
}

impl std::fmt::Display for CryptoAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptoAlgorithm::XChaCha20Poly1305 => write!(f, "XChaCha20-Poly1305"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ciphertext {
    pub nonce: [u8; NONCE_LEN],
    pub data: Vec<u8>,
}

/// Cryptographic key material with secure memory handling.
///
/// The key bytes are automatically zeroed when the struct is dropped,
/// preventing sensitive data from lingering in memory.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct KeyMaterial {
    key: [u8; KEY_LEN],
}

// Manual Debug impl to avoid leaking key bytes
impl std::fmt::Debug for KeyMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyMaterial")
            .field("key", &"[REDACTED]")
            .finish()
    }
}

// Manual PartialEq to allow comparisons using constant-time comparison
impl PartialEq for KeyMaterial {
    fn eq(&self, other: &Self) -> bool {
        self.key.ct_eq(&other.key).into()
    }
}

impl Eq for KeyMaterial {}

impl KeyMaterial {
    pub fn generate() -> Self {
        let key = XChaCha20Poly1305::generate_key(&mut OsRng);
        Self { key: key.into() }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != KEY_LEN {
            return Err(CoreError::InvalidKeyLength {
                expected: KEY_LEN,
                actual: bytes.len(),
            });
        }
        let mut key = [0u8; KEY_LEN];
        key.copy_from_slice(bytes);
        Ok(Self { key })
    }

    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.key
    }

    pub fn key_id(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.key);
        let digest = hasher.finalize();
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyDerivationParams {
    pub salt: [u8; SALT_LEN],
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl KeyDerivationParams {
    pub fn generate() -> Self {
        let mut salt = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        Self {
            salt,
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        }
    }

    pub fn argon2(&self) -> Result<Argon2<'static>> {
        let params = Params::new(
            self.memory_kib,
            self.iterations,
            self.parallelism,
            Some(KEY_LEN),
        )
        .map_err(|err| CoreError::Kdf(err.to_string()))?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

pub fn derive_key(password: &str, params: &KeyDerivationParams) -> Result<KeyMaterial> {
    if password.trim().is_empty() {
        return Err(CoreError::InvalidInput(
            "password cannot be empty".to_string(),
        ));
    }
    let mut key = [0u8; KEY_LEN];
    let argon2 = params.argon2()?;
    argon2
        .hash_password_into(password.as_bytes(), &params.salt, &mut key)
        .map_err(|err| CoreError::Kdf(err.to_string()))?;
    Ok(KeyMaterial { key })
}

pub fn encrypt(key: &KeyMaterial, plaintext: &[u8]) -> Result<Ciphertext> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.as_bytes()));
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let data = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|err| CoreError::Crypto(format!("{err}")))?;
    Ok(Ciphertext {
        nonce: nonce.into(),
        data,
    })
}

pub fn decrypt(key: &KeyMaterial, ciphertext: &Ciphertext) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.as_bytes()));
    let nonce = XNonce::from_slice(&ciphertext.nonce);
    cipher
        .decrypt(nonce, ciphertext.data.as_ref())
        .map_err(|err| CoreError::Crypto(format!("{err}")))
}

#[cfg(test)]
mod tests {
    use super::{decrypt, derive_key, encrypt, KeyDerivationParams, KeyMaterial};

    #[test]
    fn encrypts_and_decrypts() {
        let key = KeyMaterial::generate();
        let message = b"secret-note";
        let ciphertext = encrypt(&key, message).expect("encrypt");
        let decrypted = decrypt(&key, &ciphertext).expect("decrypt");
        assert_eq!(decrypted, message);
    }

    #[test]
    fn key_ids_are_stable() {
        let key = KeyMaterial::from_bytes(&[42u8; 32]).expect("key");
        let key_id = key.key_id();
        assert_eq!(key_id.len(), 64);
        assert!(key_id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn derives_keys_from_password() {
        let params = KeyDerivationParams::generate();
        let key = derive_key("correct horse battery staple", &params).expect("derive");
        assert_eq!(key.as_bytes().len(), 32);
    }
}
