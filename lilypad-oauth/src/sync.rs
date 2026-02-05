//! GitHub-based vault synchronization backend.
//!
//! This module implements the `SyncBackend` trait for synchronizing
//! encrypted vaults with a private GitHub repository.

use crate::config::{OAuthConfig, OAuthProvider};
use crate::error::{OAuthError, Result};
use crate::github_api::GitHubClient;
use crate::oauth::OAuthFlow;
use crate::token_store::TokenStoreManager;
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
    /// Token store retained for future token refresh support.
    #[allow(dead_code)]
    token_store: TokenStoreManager,
    /// Cached sync metadata.
    metadata: Option<SyncMetadata>,
    /// Cached file SHA for updates.
    cached_sha: Option<String>,
}

impl GitHubSyncBackend {
    /// Creates a new GitHub sync backend from stored credentials.
    pub fn from_stored_token() -> Result<Self> {
        let token_store = TokenStoreManager::new()?;
        let token = token_store
            .load_token(OAuthProvider::GitHub)?
            .ok_or(OAuthError::InvalidToken)?;

        if token.is_expired() {
            return Err(OAuthError::TokenExpired);
        }

        let client = GitHubClient::new(token.access_token())?;
        let user = client.get_user()?;

        Ok(Self {
            client,
            username: user.login,
            token_store,
            metadata: None,
            cached_sha: None,
        })
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
        })
    }

    /// Returns the authenticated GitHub username.
    pub fn username(&self) -> &str {
        &self.username
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

                // No local metadata, remote has data
                Ok(SyncStatus::RemoteAhead)
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
        let new_sha = self.client.save_vault_data(
            &self.username,
            payload,
            sha.as_deref(),
        )?;

        // Calculate checksum
        let checksum = calculate_checksum(payload);

        // Update sync metadata
        let meta = SyncMetadata::new(self.username.clone(), new_sha.clone(), checksum.clone());
        let meta_json = serde_json::to_string_pretty(&meta)?;

        // Get metadata SHA if exists
        let meta_sha = self
            .client
            .get_sync_metadata(&self.username)?
            .map(|(_, sha)| sha);

        self.client.save_sync_metadata(&self.username, &meta_json, meta_sha.as_deref())?;

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
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 8] = rng.gen();
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
}
