//! Secure storage for OAuth tokens.
//!
//! Tokens are stored in a JSON file with restricted file permissions.
//! The file location follows platform conventions:
//! - Linux: ~/.config/Colony/Lilypad/oauth_tokens.json
//! - macOS: ~/Library/Application Support/Colony/Lilypad/oauth_tokens.json
//! - Windows: %APPDATA%\Colony\Lilypad\oauth_tokens.json

use crate::config::OAuthProvider;
use crate::error::{OAuthError, Result};
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngExt;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroize;

/// Token storage filename.
const TOKEN_FILENAME: &str = "oauth_tokens.json";

/// Magic header for encrypted token stores (8 bytes).
const ENCRYPTED_HEADER: &[u8; 8] = b"LPTOK\x00\x01\x00";

/// Application-specific salt for machine key derivation.
const MACHINE_KEY_SALT: &[u8] = b"lilypad-oauth-token-store-v1";

/// Nonce length for XChaCha20-Poly1305.
const NONCE_LEN: usize = 24;

/// Information about a stored OAuth token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    /// Access token.
    #[serde(default)]
    access_token: String,

    /// Token type (usually "bearer").
    #[serde(default)]
    pub token_type: String,

    /// OAuth scopes granted.
    #[serde(default)]
    pub scope: String,

    /// Refresh token (if available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,

    /// Unix timestamp when the token was obtained.
    #[serde(default)]
    pub obtained_at: u64,

    /// Token lifetime in seconds (if known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,

    /// GitHub username (cached for convenience).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

impl TokenInfo {
    /// Creates a new TokenInfo.
    ///
    /// ```
    /// let token = lilypad_oauth::TokenInfo::new(
    ///     "ghp_abc123".to_string(),
    ///     "bearer".to_string(),
    ///     "repo read:user".to_string(),
    /// );
    /// assert_eq!(token.token_type, "bearer");
    /// assert!(!token.is_expired());
    /// ```
    pub fn new(access_token: String, token_type: String, scope: String) -> Self {
        Self {
            access_token,
            token_type,
            scope,
            refresh_token: None,
            obtained_at: current_timestamp(),
            expires_in: None,
            username: None,
        }
    }

    /// Sets the refresh token.
    pub fn with_refresh_token(mut self, token: Option<String>) -> Self {
        self.refresh_token = token;
        self
    }

    /// Sets the expiration time.
    pub fn with_expires_in(mut self, seconds: Option<u64>) -> Self {
        self.expires_in = seconds;
        self
    }

    /// Sets the cached username.
    pub fn with_username(mut self, username: String) -> Self {
        self.username = Some(username);
        self
    }

    /// Returns the access token.
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Returns the refresh token if available.
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    /// Checks if the token has expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_in) = self.expires_in {
            let now = current_timestamp();
            let expiry = self.obtained_at + expires_in;
            now >= expiry
        } else {
            // No expiry info, assume valid (GitHub tokens don't expire)
            false
        }
    }

    /// Returns seconds until expiration, or None if no expiry.
    pub fn seconds_until_expiry(&self) -> Option<i64> {
        self.expires_in.map(|expires_in| {
            let now = current_timestamp();
            let expiry = self.obtained_at + expires_in;
            expiry as i64 - now as i64
        })
    }
}

impl Drop for TokenInfo {
    fn drop(&mut self) {
        // Securely erase sensitive data
        self.access_token.zeroize();
        if let Some(ref mut token) = self.refresh_token {
            token.zeroize();
        }
    }
}

/// Stored tokens for multiple providers.
#[derive(Debug, Default, Serialize, Deserialize)]
struct TokenStore {
    /// Tokens indexed by provider name.
    tokens: HashMap<String, TokenInfo>,
}

/// Manager for OAuth token storage.
pub struct TokenStoreManager {
    path: PathBuf,
}

impl TokenStoreManager {
    /// Creates a new TokenStoreManager with the default path.
    pub fn new() -> Result<Self> {
        let path = Self::default_path()?;
        Ok(Self { path })
    }

    /// Creates a TokenStoreManager with a custom path.
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Returns the default token store path.
    pub fn default_path() -> Result<PathBuf> {
        let project_dirs = directories::ProjectDirs::from_path(std::path::PathBuf::from("Colony/Lilypad"))
            .ok_or_else(|| {
                OAuthError::TokenStoreError("could not determine config directory".to_string())
            })?;

        Ok(project_dirs.config_dir().join(TOKEN_FILENAME))
    }

    /// Saves a token for a provider.
    pub fn save_token(&self, provider: OAuthProvider, token: TokenInfo) -> Result<()> {
        let mut store = self.load_store()?;
        store.tokens.insert(provider.to_string().to_lowercase(), token);
        self.save_store(&store)
    }

    /// Loads a token for a provider.
    pub fn load_token(&self, provider: OAuthProvider) -> Result<Option<TokenInfo>> {
        let store = self.load_store()?;
        Ok(store.tokens.get(&provider.to_string().to_lowercase()).cloned())
    }

    /// Removes a token for a provider.
    pub fn remove_token(&self, provider: OAuthProvider) -> Result<()> {
        let mut store = self.load_store()?;
        store.tokens.remove(&provider.to_string().to_lowercase());
        self.save_store(&store)
    }

    /// Checks if a valid token exists for a provider.
    pub fn has_valid_token(&self, provider: OAuthProvider) -> Result<bool> {
        if let Some(token) = self.load_token(provider)? {
            Ok(!token.is_expired())
        } else {
            Ok(false)
        }
    }

    /// Lists all stored providers.
    pub fn list_providers(&self) -> Result<Vec<String>> {
        let store = self.load_store()?;
        Ok(store.tokens.keys().cloned().collect())
    }

    /// Loads the token store from disk.
    ///
    /// Supports both encrypted (current) and legacy plaintext formats.
    /// If a plaintext file is detected, it is re-saved encrypted automatically.
    fn load_store(&self) -> Result<TokenStore> {
        if !self.path.exists() {
            return Ok(TokenStore::default());
        }

        let raw = fs::read(&self.path).map_err(|e| {
            OAuthError::TokenStoreError(format!("failed to read token store: {}", e))
        })?;

        if raw.starts_with(ENCRYPTED_HEADER) {
            // Encrypted format
            let plaintext = decrypt_token_data(&raw)?;
            let json = String::from_utf8(plaintext).map_err(|e| {
                OAuthError::TokenStoreError(format!("decrypted data is not UTF-8: {}", e))
            })?;
            serde_json::from_str(&json).map_err(|e| {
                OAuthError::TokenStoreError(format!("failed to parse token store: {}", e))
            })
        } else {
            // Legacy plaintext JSON — parse and auto-migrate to encrypted format
            let json = String::from_utf8(raw).map_err(|e| {
                OAuthError::TokenStoreError(format!("token file is not UTF-8: {}", e))
            })?;
            let store: TokenStore = serde_json::from_str(&json).map_err(|e| {
                OAuthError::TokenStoreError(format!("failed to parse token store: {}", e))
            })?;

            // Auto-migrate: re-save as encrypted
            let _ = self.save_store(&store);

            Ok(store)
        }
    }

    /// Saves the token store to disk, encrypted with a machine-local key.
    fn save_store(&self, store: &TokenStore) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                OAuthError::TokenStoreError(format!("failed to create config directory: {}", e))
            })?;
        }

        // Serialize to JSON and encrypt
        let json = serde_json::to_string_pretty(store)?;
        let encrypted = encrypt_token_data(json.as_bytes())?;

        // Write atomically using a temp file
        let temp_path = self.path.with_extension("tmp");
        fs::write(&temp_path, &encrypted).map_err(|e| {
            OAuthError::TokenStoreError(format!("failed to write token store: {}", e))
        })?;

        // Set secure permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600)).map_err(|e| {
                OAuthError::TokenStoreError(format!("failed to set file permissions: {}", e))
            })?;
        }

        // Rename to final path
        fs::rename(&temp_path, &self.path).map_err(|e| {
            OAuthError::TokenStoreError(format!("failed to finalize token store: {}", e))
        })?;

        Ok(())
    }

    /// Securely deletes the token store by overwriting with random data
    /// across multiple passes before removing the file.
    pub fn destroy(&self) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }

        let file_size = fs::metadata(&self.path)
            .map_err(|e| {
                OAuthError::TokenStoreError(format!("failed to read token store metadata: {}", e))
            })?
            .len() as usize;

        if file_size > 0 {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = fs::OpenOptions::new()
                .write(true)
                .open(&self.path)
                .map_err(|e| {
                    OAuthError::TokenStoreError(format!(
                        "failed to open token store for secure deletion: {}",
                        e
                    ))
                })?;

            let mut rng = rand::rng();
            // 3 passes of random data to match lilypad-storage's secure_delete
            for _ in 0..3 {
                file.seek(SeekFrom::Start(0)).map_err(|e| {
                    OAuthError::TokenStoreError(format!("seek failed during secure delete: {}", e))
                })?;
                let random_data: Vec<u8> = (0..file_size).map(|_| rng.random::<u8>()).collect();
                file.write_all(&random_data).map_err(|e| {
                    OAuthError::TokenStoreError(format!("write failed during secure delete: {}", e))
                })?;
                file.sync_all().map_err(|e| {
                    OAuthError::TokenStoreError(format!("sync failed during secure delete: {}", e))
                })?;
            }

            // Final: truncate to zero
            file.set_len(0).map_err(|e| {
                OAuthError::TokenStoreError(format!(
                    "truncate failed during secure delete: {}",
                    e
                ))
            })?;
            file.sync_all().map_err(|e| {
                OAuthError::TokenStoreError(format!("final sync failed during secure delete: {}", e))
            })?;
        }

        fs::remove_file(&self.path).map_err(|e| {
            OAuthError::TokenStoreError(format!("failed to delete token store: {}", e))
        })?;

        Ok(())
    }
}

/// Derives a machine-local encryption key from stable system identifiers.
///
/// The key is deterministic on the same machine so we can decrypt without
/// user interaction, but different on other machines so a copied file is
/// useless without the same (hostname, username) combination.
fn derive_machine_key() -> [u8; 32] {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();
    let username = whoami::username();

    let mut hasher = Sha256::new();
    hasher.update(MACHINE_KEY_SALT);
    hasher.update(hostname.as_bytes());
    hasher.update(b":");
    hasher.update(username.as_bytes());
    hasher.finalize().into()
}

/// Encrypts plaintext with a machine-derived key.
///
/// Output format: ENCRYPTED_HEADER (8 bytes) || nonce (24 bytes) || ciphertext
fn encrypt_token_data(plaintext: &[u8]) -> Result<Vec<u8>> {
    let key_bytes = derive_machine_key();
    let key = Key::from_slice(&key_bytes);
    let cipher = XChaCha20Poly1305::new(key);
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| OAuthError::TokenStoreError(format!("encryption failed: {}", e)))?;

    let mut output = Vec::with_capacity(ENCRYPTED_HEADER.len() + NONCE_LEN + ciphertext.len());
    output.extend_from_slice(ENCRYPTED_HEADER);
    output.extend_from_slice(nonce.as_slice());
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypts data that was encrypted with `encrypt_token_data`.
fn decrypt_token_data(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < ENCRYPTED_HEADER.len() + NONCE_LEN + 1 {
        return Err(OAuthError::TokenStoreError(
            "encrypted token data is too short".to_string(),
        ));
    }

    let nonce_start = ENCRYPTED_HEADER.len();
    let ct_start = nonce_start + NONCE_LEN;

    let nonce = XNonce::from_slice(&data[nonce_start..ct_start]);

    let key_bytes = derive_machine_key();
    let key = Key::from_slice(&key_bytes);
    let cipher = XChaCha20Poly1305::new(key);

    cipher
        .decrypt(nonce, &data[ct_start..])
        .map_err(|_| OAuthError::TokenStoreError(
            "failed to decrypt token store (machine key mismatch or corrupted file)".to_string(),
        ))
}

/// Returns the current Unix timestamp.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_token_info() {
        let token = TokenInfo::new(
            "access123".to_string(),
            "bearer".to_string(),
            "repo read:user".to_string(),
        )
        .with_refresh_token(Some("refresh456".to_string()))
        .with_username("testuser".to_string());

        assert_eq!(token.access_token(), "access123");
        assert_eq!(token.refresh_token(), Some("refresh456"));
        assert_eq!(token.username, Some("testuser".to_string()));
        assert!(!token.is_expired());
    }

    #[test]
    fn test_token_expiry() {
        let mut token = TokenInfo::new(
            "access123".to_string(),
            "bearer".to_string(),
            "repo".to_string(),
        );

        // Token with no expiry should not be expired
        assert!(!token.is_expired());

        // Token that expires in 1 hour should not be expired
        token.expires_in = Some(3600);
        assert!(!token.is_expired());

        // Token that expired 1 hour ago should be expired
        token.obtained_at = current_timestamp() - 7200; // 2 hours ago
        token.expires_in = Some(3600); // 1 hour lifetime
        assert!(token.is_expired());
    }

    #[test]
    fn test_token_store() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tokens.json");
        let store = TokenStoreManager::with_path(path);

        // Save a token
        let token = TokenInfo::new(
            "access123".to_string(),
            "bearer".to_string(),
            "repo".to_string(),
        );
        store.save_token(OAuthProvider::GitHub, token).unwrap();

        // Load it back
        let loaded = store.load_token(OAuthProvider::GitHub).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().access_token(), "access123");

        // Check has_valid_token
        assert!(store.has_valid_token(OAuthProvider::GitHub).unwrap());

        // Remove token
        store.remove_token(OAuthProvider::GitHub).unwrap();
        assert!(store.load_token(OAuthProvider::GitHub).unwrap().is_none());
    }

    #[test]
    fn test_token_seconds_until_expiry() {
        let token = TokenInfo::new(
            "tok_abc".to_string(),
            "bearer".to_string(),
            "repo".to_string(),
        )
        .with_expires_in(Some(3600)); // expires in 1 hour

        let remaining = token.seconds_until_expiry();
        assert!(remaining.is_some());

        let secs = remaining.unwrap();
        // The token was just created, so remaining should be close to 3600
        // Allow a small margin for test execution time
        assert!(
            secs > 3500 && secs <= 3600,
            "Expected ~3600 seconds remaining, got {}",
            secs
        );
    }

    #[test]
    fn test_token_no_expiry_seconds() {
        let token = TokenInfo::new(
            "tok_no_exp".to_string(),
            "bearer".to_string(),
            "repo read:user".to_string(),
        );

        // No expires_in set, so seconds_until_expiry should return None
        assert!(
            token.seconds_until_expiry().is_none(),
            "Token without expiry should return None for seconds_until_expiry"
        );

        // Also verify it is not considered expired
        assert!(!token.is_expired());
    }

    #[test]
    fn test_token_with_username() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tokens_username.json");
        let store = TokenStoreManager::with_path(path);

        let token = TokenInfo::new(
            "tok_user".to_string(),
            "bearer".to_string(),
            "repo".to_string(),
        )
        .with_username("octocat".to_string());

        assert_eq!(token.username, Some("octocat".to_string()));

        // Save and reload to verify persistence
        store.save_token(OAuthProvider::GitHub, token).unwrap();
        let loaded = store.load_token(OAuthProvider::GitHub).unwrap().unwrap();
        assert_eq!(
            loaded.username,
            Some("octocat".to_string()),
            "Username should persist through save/load"
        );
    }

    #[test]
    fn test_token_store_list_providers() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tokens_list.json");
        let store = TokenStoreManager::with_path(path);

        // Initially no providers
        let providers = store.list_providers().unwrap();
        assert!(providers.is_empty(), "No providers initially");

        // Save a GitHub token
        let token = TokenInfo::new(
            "tok_gh".to_string(),
            "bearer".to_string(),
            "repo".to_string(),
        );
        store.save_token(OAuthProvider::GitHub, token).unwrap();

        let providers = store.list_providers().unwrap();
        assert_eq!(providers.len(), 1);
        assert!(
            providers.contains(&"github".to_string()),
            "Should list 'github' as a stored provider"
        );
    }

    #[test]
    fn test_token_store_destroy() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tokens_destroy.json");
        let store = TokenStoreManager::with_path(path.clone());

        // Save a token so the file exists
        let token = TokenInfo::new(
            "tok_destroy".to_string(),
            "bearer".to_string(),
            "repo".to_string(),
        );
        store.save_token(OAuthProvider::GitHub, token).unwrap();
        assert!(path.exists(), "Token file should exist after save");

        // Destroy the store
        store.destroy().unwrap();
        assert!(
            !path.exists(),
            "Token file should be deleted after destroy"
        );
    }

    #[test]
    fn test_token_store_overwrite() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tokens_overwrite.json");
        let store = TokenStoreManager::with_path(path);

        // Save initial token
        let token1 = TokenInfo::new(
            "first_token".to_string(),
            "bearer".to_string(),
            "repo".to_string(),
        )
        .with_username("user_one".to_string());
        store.save_token(OAuthProvider::GitHub, token1).unwrap();

        // Verify first token
        let loaded1 = store.load_token(OAuthProvider::GitHub).unwrap().unwrap();
        assert_eq!(loaded1.access_token(), "first_token");
        assert_eq!(loaded1.username, Some("user_one".to_string()));

        // Overwrite with a different token for the same provider
        let token2 = TokenInfo::new(
            "second_token".to_string(),
            "bearer".to_string(),
            "repo read:user".to_string(),
        )
        .with_username("user_two".to_string());
        store.save_token(OAuthProvider::GitHub, token2).unwrap();

        // Verify the new token replaced the old one
        let loaded2 = store.load_token(OAuthProvider::GitHub).unwrap().unwrap();
        assert_eq!(loaded2.access_token(), "second_token");
        assert_eq!(loaded2.username, Some("user_two".to_string()));
        assert_eq!(loaded2.scope, "repo read:user");

        // There should still be only one provider
        let providers = store.list_providers().unwrap();
        assert_eq!(providers.len(), 1);
    }
}
