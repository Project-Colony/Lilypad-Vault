//! GitHub-based vault synchronization backend.
//!
//! This module implements the `SyncBackend` trait for synchronizing
//! encrypted vaults with a private GitHub repository.

use crate::config::{OAuthConfig, OAuthProvider};
use crate::error::{OAuthError, Result};
use crate::github_api::GitHubClient;
use crate::oauth::OAuthFlow;
use crate::token_store::{TokenInfo, TokenStoreManager};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Synchronization status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    /// Not authenticated with GitHub.
    NotAuthenticated,
    /// Authenticated but vault repo doesn't exist yet.
    NoRemoteVault,
    /// Local and remote are in sync.
    InSync,
    /// Local has changes not pushed to remote.
    LocalAhead,
    /// Remote has changes not pulled locally.
    RemoteAhead,
    /// Both local and remote have changes (conflict).
    Conflict,
    /// Unknown status (e.g., network error).
    Unknown(String),
}

/// Metadata for tracking sync state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMetadata {
    /// Last sync timestamp (Unix).
    pub last_sync_at: u64,
    /// SHA of the last synced vault file.
    pub last_sync_sha: String,
    /// Local vault checksum at last sync.
    pub local_checksum: String,
    /// GitHub username.
    pub github_username: String,
    /// Device identifier (for multi-device tracking).
    pub device_id: String,
    /// Sync version for format compatibility.
    pub version: u32,
}

impl SyncMetadata {
    /// Creates new sync metadata.
    ///
    /// ```
    /// let meta = lilypad_oauth::SyncMetadata::new(
    ///     "octocat".to_string(),
    ///     "abc123".to_string(),
    ///     "def456".to_string(),
    /// );
    /// assert_eq!(meta.github_username, "octocat");
    /// assert_eq!(meta.version, 1);
    /// assert!(meta.last_sync_at > 0);
    /// ```
    pub fn new(github_username: String, sha: String, local_checksum: String) -> Self {
        Self {
            last_sync_at: current_timestamp(),
            last_sync_sha: sha,
            local_checksum,
            github_username,
            device_id: generate_device_id(),
            version: 1,
        }
    }

    /// Creates metadata reusing an existing device ID.
    pub fn with_device_id(mut self, device_id: String) -> Self {
        self.device_id = device_id;
        self
    }

    /// Updates the metadata after a sync.
    pub fn update(&mut self, sha: String, local_checksum: String) {
        self.last_sync_at = current_timestamp();
        self.last_sync_sha = sha;
        self.local_checksum = local_checksum;
    }
}

/// GitHub-based sync backend for vault storage.
///
/// This backend stores encrypted vault data in a private GitHub repository.
/// All data is encrypted locally before being uploaded.
pub struct GitHubSyncBackend {
    client: GitHubClient,
    username: String,
    /// Token store used for token persistence and refresh.
    token_store: TokenStoreManager,
    /// Cached sync metadata.
    metadata: Option<SyncMetadata>,
    /// Cached file SHA for updates.
    cached_sha: Option<String>,
    /// Persistent device identifier.
    device_id: String,
}

impl GitHubSyncBackend {
    /// Creates a new GitHub sync backend from stored credentials.
    pub fn from_stored_token() -> Result<Self> {
        let token_store = TokenStoreManager::new()?;
        let mut token = token_store
            .load_token(OAuthProvider::GitHub)?
            .ok_or(OAuthError::InvalidToken)?;

        if token.is_expired() {
            // Attempt token refresh before failing
            token = Self::try_refresh_token(&token_store, &token)?;
        }

        let client = GitHubClient::new(token.access_token())?;
        let user = client.get_user()?;

        Ok(Self {
            client,
            username: user.login,
            token_store,
            metadata: None,
            cached_sha: None,
            device_id: generate_device_id(),
        })
    }

    /// Attempts to refresh an expired token using the refresh_token grant.
    fn try_refresh_token(token_store: &TokenStoreManager, token: &TokenInfo) -> Result<TokenInfo> {
        let refresh_token = token.refresh_token().ok_or(OAuthError::TokenExpired)?;

        let config = OAuthConfig::from_env().unwrap_or_default();

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Lilypad-OAuth/1.0")
            .build()
            .map_err(|e| OAuthError::NetworkError(e.to_string()))?;

        #[derive(serde::Serialize)]
        struct RefreshRequest<'a> {
            client_id: &'a str,
            grant_type: &'a str,
            refresh_token: &'a str,
        }

        #[derive(serde::Deserialize)]
        struct RefreshResponse {
            access_token: Option<String>,
            token_type: Option<String>,
            scope: Option<String>,
            refresh_token: Option<String>,
            expires_in: Option<u64>,
            error: Option<String>,
            error_description: Option<String>,
        }

        let request = RefreshRequest {
            client_id: &config.client_id,
            grant_type: "refresh_token",
            refresh_token,
        };

        let response = client
            .post(&config.token_url)
            .header("Accept", "application/json")
            .form(&request)
            .send()?;

        let resp: RefreshResponse = response.json()?;

        if let Some(error) = resp.error {
            let msg = resp.error_description.unwrap_or(error);
            return Err(OAuthError::AuthenticationFailed(format!(
                "Token refresh failed: {}",
                msg
            )));
        }

        let access_token = resp.access_token.ok_or_else(|| {
            OAuthError::AuthenticationFailed("no access_token in refresh response".to_string())
        })?;

        let new_token = TokenInfo::new(
            access_token,
            resp.token_type.unwrap_or_else(|| "bearer".to_string()),
            resp.scope.unwrap_or_else(|| token.scope.clone()),
        )
        .with_refresh_token(
            resp.refresh_token
                .or_else(|| token.refresh_token().map(String::from)),
        )
        .with_expires_in(resp.expires_in)
        .with_username(token.username.clone().unwrap_or_default());

        // Save the refreshed token
        token_store.save_token(OAuthProvider::GitHub, new_token.clone())?;

        Ok(new_token)
    }

    /// Creates a new GitHub sync backend with a fresh OAuth flow.
    pub fn authenticate(config: OAuthConfig) -> Result<Self> {
        let flow = OAuthFlow::new(config)?;
        let result = flow.authenticate()?;

        // Get user info
        let client = GitHubClient::new(&result.access_token)?;
        let user = client.get_user()?;

        // Store token
        let token_store = TokenStoreManager::new()?;
        let token_info = result.to_token_info().with_username(user.login.clone());
        token_store.save_token(OAuthProvider::GitHub, token_info)?;

        Ok(Self {
            client,
            username: user.login,
            token_store,
            metadata: None,
            cached_sha: None,
            device_id: generate_device_id(),
        })
    }

    /// Returns the authenticated GitHub username.
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Returns the device identifier.
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Ensures the stored token is still valid, refreshing if necessary.
    ///
    /// Call this before long-running operations to avoid mid-operation token expiry.
    pub fn ensure_token_valid(&self) -> Result<()> {
        if let Some(token) = self.token_store.load_token(OAuthProvider::GitHub)? {
            if token.is_expired() {
                Self::try_refresh_token(&self.token_store, &token)?;
            }
        }
        Ok(())
    }

    /// Checks if there is a stored valid token.
    pub fn has_valid_token() -> Result<bool> {
        let token_store = TokenStoreManager::new()?;
        token_store.has_valid_token(OAuthProvider::GitHub)
    }

    /// Logs out and removes stored credentials.
    pub fn logout() -> Result<()> {
        let token_store = TokenStoreManager::new()?;
        token_store.remove_token(OAuthProvider::GitHub)
    }

    /// Gets the sync status.
    pub fn get_status(&mut self, local_checksum: &str) -> Result<SyncStatus> {
        // Try to get remote data
        match self.client.get_vault_data(&self.username) {
            Ok(Some((_, remote_sha))) => {
                // Load sync metadata
                if let Some(ref meta) = self.metadata {
                    if meta.local_checksum == local_checksum && meta.last_sync_sha == remote_sha {
                        return Ok(SyncStatus::InSync);
                    } else if meta.local_checksum != local_checksum
                        && meta.last_sync_sha == remote_sha
                    {
                        return Ok(SyncStatus::LocalAhead);
                    } else if meta.local_checksum == local_checksum
                        && meta.last_sync_sha != remote_sha
                    {
                        return Ok(SyncStatus::RemoteAhead);
                    } else {
                        return Ok(SyncStatus::Conflict);
                    }
                }

                // No local metadata: if local vault has content, we cannot
                // determine whether local or remote is newer — treat as conflict
                // so the user explicitly chooses with --force.
                if local_checksum.is_empty() {
                    Ok(SyncStatus::RemoteAhead)
                } else {
                    Ok(SyncStatus::Conflict)
                }
            }
            Ok(None) => Ok(SyncStatus::NoRemoteVault),
            Err(OAuthError::InvalidToken) => Ok(SyncStatus::NotAuthenticated),
            Err(e) => Ok(SyncStatus::Unknown(e.to_string())),
        }
    }

    /// Pushes local vault data to GitHub.
    pub fn push(&mut self, vault_name: &str, payload: &[u8]) -> Result<()> {
        // Get current SHA if updating existing file
        let sha = self
            .client
            .get_vault_data(&self.username)?
            .map(|(_, sha)| sha);

        // Upload vault data
        let new_sha = self
            .client
            .save_vault_data(&self.username, payload, sha.as_deref())?;

        // Calculate checksum
        let checksum = calculate_checksum(payload);

        // Update sync metadata
        let meta = SyncMetadata::new(self.username.clone(), new_sha.clone(), checksum.clone())
            .with_device_id(self.device_id.clone());
        let meta_json = serde_json::to_string_pretty(&meta)?;

        // Get metadata SHA if exists
        let meta_sha = self
            .client
            .get_sync_metadata(&self.username)?
            .map(|(_, sha)| sha);

        self.client
            .save_sync_metadata(&self.username, &meta_json, meta_sha.as_deref())?;

        // Update cached state
        self.metadata = Some(meta);
        self.cached_sha = Some(new_sha);

        println!("Vault '{}' synced to GitHub successfully.", vault_name);
        Ok(())
    }

    /// Pulls vault data from GitHub.
    pub fn pull(&mut self, _vault_name: &str) -> Result<Option<Vec<u8>>> {
        match self.client.get_vault_data(&self.username)? {
            Some((data, sha)) => {
                self.cached_sha = Some(sha);
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }

    /// Remote path for a named vault, so multiple vaults do not collide on a
    /// single `vault.lily`. The name is caller-validated (alphanumeric, `_`,
    /// `-`) so it is safe as a single path segment.
    fn remote_path_for(vault_name: &str) -> String {
        format!("vaults/{vault_name}.lily")
    }

    /// Pulls a specific named vault's bytes and remote SHA (per-vault path).
    pub fn pull_named(&mut self, vault_name: &str) -> Result<Option<(Vec<u8>, String)>> {
        let path = Self::remote_path_for(vault_name);
        let result = self.client.get_vault_file(&self.username, &path)?;
        if let Some((_, ref sha)) = result {
            self.cached_sha = Some(sha.clone());
        }
        Ok(result)
    }

    /// Returns the current remote SHA for a named vault, if it exists.
    pub fn remote_sha_named(&mut self, vault_name: &str) -> Result<Option<String>> {
        let path = Self::remote_path_for(vault_name);
        Ok(self
            .client
            .get_vault_file(&self.username, &path)?
            .map(|(_, sha)| sha))
    }

    /// Pushes a named vault's payload to its per-vault remote path. Returns the
    /// new remote SHA. Pass the previously observed SHA to avoid clobbering a
    /// concurrent remote update (GitHub rejects a stale SHA).
    pub fn push_named(
        &mut self,
        vault_name: &str,
        payload: &[u8],
        prev_sha: Option<&str>,
    ) -> Result<String> {
        let path = Self::remote_path_for(vault_name);
        let new_sha = self
            .client
            .save_vault_file(&self.username, &path, payload, prev_sha)?;
        self.cached_sha = Some(new_sha.clone());
        Ok(new_sha)
    }

    /// Ensures the vault repository exists, creating it if needed.
    pub fn ensure_repo(&self) -> Result<()> {
        self.client.get_or_create_vault_repo(&self.username)?;
        println!("Vault repository ready: lilypad-vault-{}", self.username);
        Ok(())
    }

    /// Gets information about the vault repository.
    pub fn get_repo_info(&self) -> Result<Option<crate::github_api::GitHubRepo>> {
        let repo_name = format!("lilypad-vault-{}", self.username);
        match self.client.get_repo(&self.username, &repo_name) {
            Ok(repo) => Ok(Some(repo)),
            Err(OAuthError::RepoNotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Deletes a vault file (and its metadata) from the remote GitHub repository.
    pub fn delete(&mut self, vault_name: &str) -> Result<()> {
        let repo_name = format!("{}-{}", crate::VAULT_REPO_PREFIX, self.username);

        // Delete the vault data file
        match self
            .client
            .get_file(&self.username, &repo_name, crate::VAULT_DATA_FILENAME)
        {
            Ok(file) => {
                self.client.delete_file(
                    &self.username,
                    &repo_name,
                    crate::VAULT_DATA_FILENAME,
                    &file.sha,
                    &format!("Delete vault '{}'", vault_name),
                )?;
            }
            Err(OAuthError::FileNotFound { .. }) => {
                // Already gone — not an error
            }
            Err(e) => return Err(e),
        }

        // Also delete sync metadata if present
        match self
            .client
            .get_file(&self.username, &repo_name, crate::SYNC_META_FILENAME)
        {
            Ok(file) => {
                self.client.delete_file(
                    &self.username,
                    &repo_name,
                    crate::SYNC_META_FILENAME,
                    &file.sha,
                    "Delete sync metadata",
                )?;
            }
            Err(OAuthError::FileNotFound { .. }) => {}
            Err(e) => return Err(e),
        }

        self.metadata = None;
        self.cached_sha = None;

        Ok(())
    }

    /// Loads sync metadata from the remote repository.
    pub fn load_remote_metadata(&mut self) -> Result<Option<SyncMetadata>> {
        match self.client.get_sync_metadata(&self.username)? {
            Some((json, _sha)) => {
                let meta: SyncMetadata = serde_json::from_str(&json)?;
                self.metadata = Some(meta.clone());
                Ok(Some(meta))
            }
            None => Ok(None),
        }
    }
}

/// Calculates a SHA-256 checksum of data.
fn calculate_checksum(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let result = Sha256::digest(data);
    hex::encode(result)
}

/// Generates a unique device identifier.
fn generate_device_id() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let bytes: [u8; 8] = rng.random();
    hex::encode(bytes)
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

    #[test]
    fn test_sync_metadata() {
        let meta = SyncMetadata::new(
            "testuser".to_string(),
            "abc123".to_string(),
            "def456".to_string(),
        );

        assert_eq!(meta.github_username, "testuser");
        assert_eq!(meta.last_sync_sha, "abc123");
        assert_eq!(meta.local_checksum, "def456");
        assert!(meta.last_sync_at > 0);
    }

    #[test]
    fn test_calculate_checksum() {
        let data = b"test data";
        let checksum = calculate_checksum(data);
        assert!(!checksum.is_empty());
        assert_eq!(checksum.len(), 64); // SHA-256 produces 64 hex chars
    }

    #[test]
    fn test_device_id_persistence() {
        let meta = SyncMetadata::new(
            "testuser".to_string(),
            "abc123".to_string(),
            "def456".to_string(),
        )
        .with_device_id("my-device-001".to_string());

        assert_eq!(meta.device_id, "my-device-001");
        assert_eq!(meta.github_username, "testuser");
    }

    #[test]
    fn test_sync_metadata_update() {
        let mut meta = SyncMetadata::new(
            "testuser".to_string(),
            "abc123".to_string(),
            "def456".to_string(),
        );

        let original_timestamp = meta.last_sync_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        meta.update("xyz789".to_string(), "ghi012".to_string());

        assert_eq!(meta.last_sync_sha, "xyz789");
        assert_eq!(meta.local_checksum, "ghi012");
        assert!(meta.last_sync_at >= original_timestamp);
    }

    #[test]
    fn test_sync_status_display() {
        // Verify all SyncStatus variants can be formatted via Debug
        let variants: Vec<SyncStatus> = vec![
            SyncStatus::NotAuthenticated,
            SyncStatus::NoRemoteVault,
            SyncStatus::InSync,
            SyncStatus::LocalAhead,
            SyncStatus::RemoteAhead,
            SyncStatus::Conflict,
            SyncStatus::Unknown("test error".to_string()),
        ];

        for variant in &variants {
            let formatted = format!("{:?}", variant);
            assert!(
                !formatted.is_empty(),
                "SyncStatus variant should have non-empty Debug output"
            );
        }

        // Verify specific Debug representations
        assert_eq!(format!("{:?}", SyncStatus::InSync), "InSync");
        assert_eq!(format!("{:?}", SyncStatus::Conflict), "Conflict");
        assert!(format!("{:?}", SyncStatus::Unknown("net fail".to_string())).contains("net fail"));
    }

    #[test]
    fn test_sync_metadata_with_device_id() {
        let custom_device_id = "custom-device-abc123".to_string();
        let meta = SyncMetadata::new(
            "alice".to_string(),
            "sha_aaa".to_string(),
            "checksum_bbb".to_string(),
        )
        .with_device_id(custom_device_id.clone());

        assert_eq!(meta.device_id, custom_device_id);
        assert_eq!(meta.github_username, "alice");
        assert_eq!(meta.last_sync_sha, "sha_aaa");
        assert_eq!(meta.local_checksum, "checksum_bbb");
        assert_eq!(meta.version, 1);
    }

    #[test]
    fn test_sync_metadata_last_synced_updates() {
        let mut meta = SyncMetadata::new(
            "bob".to_string(),
            "sha_orig".to_string(),
            "cs_orig".to_string(),
        );

        let first_sync_at = meta.last_sync_at;
        assert!(first_sync_at > 0);

        // Sleep briefly to ensure timestamp advances
        std::thread::sleep(std::time::Duration::from_millis(1100));

        meta.update("sha_new".to_string(), "cs_new".to_string());

        // After update, last_sync_at should be >= the original
        assert!(
            meta.last_sync_at >= first_sync_at,
            "last_sync_at should not decrease after update"
        );
        assert_eq!(meta.last_sync_sha, "sha_new");
        assert_eq!(meta.local_checksum, "cs_new");
        // Username and device_id should be unchanged
        assert_eq!(meta.github_username, "bob");
    }

    #[test]
    fn test_checksum_different_data() {
        let checksum_a = calculate_checksum(b"hello world");
        let checksum_b = calculate_checksum(b"hello worl!");

        assert_ne!(
            checksum_a, checksum_b,
            "Different data must produce different checksums"
        );

        // Also test with empty vs non-empty
        let checksum_empty = calculate_checksum(b"");
        assert_ne!(checksum_empty, checksum_a);
    }

    #[test]
    fn test_checksum_same_data() {
        let data = b"identical payload bytes";
        let checksum1 = calculate_checksum(data);
        let checksum2 = calculate_checksum(data);

        assert_eq!(
            checksum1, checksum2,
            "Same data must produce the same checksum"
        );

        // Verify it's a valid SHA-256 hex string (64 hex chars)
        assert_eq!(checksum1.len(), 64);
        assert!(checksum1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_device_id_format() {
        let id = generate_device_id();

        // generate_device_id produces 8 random bytes hex-encoded => 16 hex characters
        assert_eq!(
            id.len(),
            16,
            "Device ID should be 16 hex characters (8 bytes)"
        );
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "Device ID should consist only of valid hex characters"
        );

        // Two generated IDs should (almost certainly) be different
        let id2 = generate_device_id();
        assert_ne!(id, id2, "Two generated device IDs should differ");
    }
}
