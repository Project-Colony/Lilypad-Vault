use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, Generate, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
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
    /// Generates a new random 256-bit encryption key.
    ///
    /// Uses the operating system's cryptographically secure random
    /// number generator.
    ///
    /// # Examples
    ///
    /// ```
    /// use lilypad_core::KeyMaterial;
    ///
    /// let key = KeyMaterial::generate();
    /// assert_eq!(key.as_bytes().len(), 32);
    ///
    /// // Each generated key is unique.
    /// let key2 = KeyMaterial::generate();
    /// assert_ne!(key.as_bytes(), key2.as_bytes());
    /// ```
    pub fn generate() -> Self {
        Self {
            key: Key::generate().into(),
        }
    }

    /// Creates a [`KeyMaterial`] from an existing 32-byte slice.
    ///
    /// Returns an error if `bytes` is not exactly 32 bytes long.
    ///
    /// # Examples
    ///
    /// ```
    /// use lilypad_core::KeyMaterial;
    ///
    /// let key = KeyMaterial::from_bytes(&[0xAB; 32]).unwrap();
    /// assert_eq!(key.as_bytes()[0], 0xAB);
    ///
    /// // Wrong length is rejected.
    /// assert!(KeyMaterial::from_bytes(&[0u8; 16]).is_err());
    /// ```
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
        hasher.update(self.key);
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
    /// Generates new key derivation parameters with default settings.
    ///
    /// Uses 64 MiB memory, 3 iterations, and single-threaded processing.
    /// For systems with limited resources, use `generate_adaptive()` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use lilypad_core::KeyDerivationParams;
    ///
    /// let params = KeyDerivationParams::generate();
    /// assert_eq!(params.memory_kib, 64 * 1024);
    /// assert_eq!(params.iterations, 3);
    /// assert_eq!(params.parallelism, 1);
    /// // Salt is 16 bytes of random data.
    /// assert_eq!(params.salt.len(), 16);
    /// ```
    pub fn generate() -> Self {
        Self {
            salt: Generate::generate(),
            memory_kib: 64 * 1024, // 64 MiB
            iterations: 3,
            parallelism: 1,
        }
    }

    /// Generates new key derivation parameters adapted to system capabilities.
    ///
    /// Adjusts memory usage based on available system RAM and parallelism
    /// based on CPU core count. This ensures the KDF is as strong as possible
    /// while remaining usable on the target system.
    pub fn generate_adaptive() -> Self {
        let salt = Generate::generate();

        // Detect system capabilities
        let (memory_kib, iterations, parallelism) = Self::detect_optimal_params();

        Self {
            salt,
            memory_kib,
            iterations,
            parallelism,
        }
    }

    /// Detects optimal Argon2 parameters based on system capabilities.
    ///
    /// Returns (memory_kib, iterations, parallelism).
    fn detect_optimal_params() -> (u32, u32, u32) {
        // Get number of CPU cores (use 1 as fallback)
        let num_cpus = std::thread::available_parallelism()
            .map(|p| p.get() as u32)
            .unwrap_or(1);

        // Use at most 4 threads to avoid excessive resource usage
        let parallelism = num_cpus.min(4);

        // Try to detect available memory using sys-info or fallback to conservative defaults
        // We aim to use about 1/16th of available RAM, capped between 16 MiB and 256 MiB
        let memory_kib = Self::detect_available_memory_kib()
            .map(|total| {
                // Use 1/16th of total memory, but at least 16 MiB and at most 256 MiB
                let target = total / 16;
                target.clamp(16 * 1024, 256 * 1024) as u32
            })
            .unwrap_or(64 * 1024); // Default to 64 MiB if detection fails

        // Adjust iterations based on memory: less memory = more iterations
        // This maintains security even on low-memory systems
        let iterations = if memory_kib >= 128 * 1024 {
            2 // High memory: fewer iterations needed
        } else if memory_kib >= 64 * 1024 {
            3 // Medium memory: standard iterations
        } else if memory_kib >= 32 * 1024 {
            4 // Low memory: more iterations
        } else {
            6 // Very low memory: many iterations to compensate
        };

        (memory_kib, iterations, parallelism)
    }

    /// Attempts to detect available system memory in KiB.
    ///
    /// Returns None if detection fails.
    #[cfg(target_os = "linux")]
    fn detect_available_memory_kib() -> Option<u64> {
        // Read from /proc/meminfo
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|line| line.starts_with("MemTotal:"))
                    .and_then(|line| {
                        line.split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse::<u64>().ok())
                    })
            })
    }

    #[cfg(target_os = "macos")]
    fn detect_available_memory_kib() -> Option<u64> {
        // Use sysctl on macOS
        use std::process::Command;
        Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .map(|bytes| bytes / 1024) // Convert to KiB
            })
    }

    #[cfg(target_os = "windows")]
    fn detect_available_memory_kib() -> Option<u64> {
        // On Windows, we'd need to use Windows API
        // For simplicity, return None to use defaults
        None
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    fn detect_available_memory_kib() -> Option<u64> {
        None
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

/// Derives a 256-bit encryption key from a password using Argon2id.
///
/// # Examples
///
/// ```
/// use lilypad_core::{derive_key, KeyDerivationParams};
///
/// let params = KeyDerivationParams::generate();
/// let key = derive_key("my strong passphrase", &params).unwrap();
/// assert_eq!(key.as_bytes().len(), 32);
///
/// // Empty passwords are rejected
/// assert!(derive_key("", &params).is_err());
/// ```
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

/// Encrypts data using XChaCha20-Poly1305.
///
/// # Examples
///
/// ```
/// use lilypad_core::{encrypt, decrypt, KeyMaterial};
///
/// let key = KeyMaterial::generate();
/// let ciphertext = encrypt(&key, b"secret data").unwrap();
/// let decrypted = decrypt(&key, &ciphertext).unwrap();
/// assert_eq!(decrypted, b"secret data");
/// ```
pub fn encrypt(key: &KeyMaterial, plaintext: &[u8]) -> Result<Ciphertext> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let nonce = XNonce::generate();
    let data = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|err| CoreError::Crypto(format!("{err}")))?;
    Ok(Ciphertext {
        nonce: nonce.into(),
        data,
    })
}

/// Decrypts data previously encrypted with [`encrypt`].
///
/// Returns an error if the key is wrong or the ciphertext is corrupted.
///
/// # Examples
///
/// ```
/// use lilypad_core::{encrypt, decrypt, KeyMaterial};
///
/// let key = KeyMaterial::generate();
/// let ct = encrypt(&key, b"hello").unwrap();
/// assert_eq!(decrypt(&key, &ct).unwrap(), b"hello");
///
/// // Wrong key fails
/// let other = KeyMaterial::generate();
/// assert!(decrypt(&other, &ct).is_err());
/// ```
pub fn decrypt(key: &KeyMaterial, ciphertext: &Ciphertext) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    cipher
        .decrypt((&ciphertext.nonce).into(), ciphertext.data.as_ref())
        .map_err(|err| CoreError::Crypto(format!("{err}")))
}

#[cfg(test)]
mod tests {
    use super::{decrypt, derive_key, encrypt, Ciphertext, KeyDerivationParams, KeyMaterial};

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

    // Negative tests

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let key1 = KeyMaterial::generate();
        let key2 = KeyMaterial::generate();
        let message = b"secret-note";
        let ciphertext = encrypt(&key1, message).expect("encrypt");

        // Decrypting with a different key should fail
        let result = decrypt(&key2, &ciphertext);
        assert!(result.is_err(), "decryption with wrong key should fail");
    }

    #[test]
    fn decrypt_with_corrupted_ciphertext_fails() {
        let key = KeyMaterial::generate();
        let message = b"secret-note";
        let mut ciphertext = encrypt(&key, message).expect("encrypt");

        // Corrupt the ciphertext data
        if !ciphertext.data.is_empty() {
            ciphertext.data[0] ^= 0xFF;
        }

        let result = decrypt(&key, &ciphertext);
        assert!(
            result.is_err(),
            "decryption with corrupted ciphertext should fail"
        );
    }

    #[test]
    fn decrypt_with_corrupted_nonce_fails() {
        let key = KeyMaterial::generate();
        let message = b"secret-note";
        let mut ciphertext = encrypt(&key, message).expect("encrypt");

        // Corrupt the nonce
        ciphertext.nonce[0] ^= 0xFF;

        let result = decrypt(&key, &ciphertext);
        assert!(
            result.is_err(),
            "decryption with corrupted nonce should fail"
        );
    }

    #[test]
    fn decrypt_with_truncated_ciphertext_fails() {
        let key = KeyMaterial::generate();
        let message = b"secret-note-that-is-longer-for-truncation";
        let ciphertext = encrypt(&key, message).expect("encrypt");

        // Truncate the ciphertext
        let truncated = Ciphertext {
            nonce: ciphertext.nonce,
            data: ciphertext.data[..ciphertext.data.len() / 2].to_vec(),
        };

        let result = decrypt(&key, &truncated);
        assert!(
            result.is_err(),
            "decryption with truncated ciphertext should fail"
        );
    }

    #[test]
    fn derive_key_with_empty_password_fails() {
        let params = KeyDerivationParams::generate();
        let result = derive_key("", &params);
        assert!(
            result.is_err(),
            "key derivation with empty password should fail"
        );
    }

    #[test]
    fn derive_key_with_whitespace_password_fails() {
        let params = KeyDerivationParams::generate();
        let result = derive_key("   ", &params);
        assert!(
            result.is_err(),
            "key derivation with whitespace-only password should fail"
        );
    }

    #[test]
    fn key_from_wrong_length_bytes_fails() {
        // Too short
        let result = KeyMaterial::from_bytes(&[0u8; 16]);
        assert!(result.is_err(), "key from 16 bytes should fail");

        // Too long
        let result = KeyMaterial::from_bytes(&[0u8; 64]);
        assert!(result.is_err(), "key from 64 bytes should fail");
    }

    #[test]
    fn different_passwords_derive_different_keys() {
        let params = KeyDerivationParams::generate();
        let key1 = derive_key("password1", &params).expect("derive key1");
        let key2 = derive_key("password2", &params).expect("derive key2");

        assert_ne!(
            key1.as_bytes(),
            key2.as_bytes(),
            "different passwords should derive different keys"
        );
    }

    #[test]
    fn same_password_different_salts_derive_different_keys() {
        let params1 = KeyDerivationParams::generate();
        let params2 = KeyDerivationParams::generate();

        let key1 = derive_key("same-password", &params1).expect("derive key1");
        let key2 = derive_key("same-password", &params2).expect("derive key2");

        assert_ne!(
            key1.as_bytes(),
            key2.as_bytes(),
            "same password with different salts should derive different keys"
        );
    }

    #[test]
    fn key_equality_is_constant_time() {
        // This test ensures the PartialEq implementation using constant-time comparison
        // We can't directly test timing, but we verify the comparison works correctly
        let key1 = KeyMaterial::from_bytes(&[42u8; 32]).expect("key1");
        let key2 = KeyMaterial::from_bytes(&[42u8; 32]).expect("key2");
        let key3 = KeyMaterial::from_bytes(&[43u8; 32]).expect("key3");

        assert_eq!(key1, key2, "equal keys should be equal");
        assert_ne!(key1, key3, "different keys should not be equal");
    }
}
