use chacha20poly1305::aead::{Aead, AeadCore, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::{CoreError, Result};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMaterial {
    key: [u8; KEY_LEN],
}

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
    use super::{decrypt, encrypt, KeyMaterial};

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
}
