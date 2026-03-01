//! Lilypad OAuth - GitHub OAuth authentication and vault synchronization.
//!
//! This crate provides:
//! - GitHub OAuth authentication flow (Device Flow and Authorization Code Flow)
//! - GitHub API client for repository management
//! - Vault synchronization backend for storing encrypted vaults in GitHub repos
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     OAuth      ┌─────────────┐
//! │   Lilypad   │◄──────────────►│   GitHub    │
//! │   Client    │                │   OAuth     │
//! └──────┬──────┘                └─────────────┘
//!        │
//!        │ GitHub API (with OAuth token)
//!        ▼
//! ┌─────────────────────────────────────────────┐
//! │  Private Repo: lilypad-vault-{user}         │
//! │  ├── vault.lily (encrypted data)             │
//! │  ├── .lilypad-meta (sync metadata)          │
//! │  └── README.md (optional)                   │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! # Security Model
//!
//! - All vault data is encrypted locally BEFORE being sent to GitHub
//! - GitHub only stores ciphertext - even GitHub cannot read your passwords
//! - OAuth tokens are stored securely with restricted file permissions
//! - Tokens can be revoked at any time from GitHub settings

mod config;
mod error;
mod github_api;
mod oauth;
mod sync;
mod token_store;

pub use config::{OAuthConfig, OAuthProvider};
pub use error::{OAuthError, Result};
pub use github_api::{GitHubClient, GitHubRepo, GitHubUser};
pub use oauth::{AuthorizationResult, DeviceFlowAuth, OAuthFlow};
pub use sync::{GitHubSyncBackend, SyncMetadata, SyncStatus};
pub use token_store::{TokenInfo, TokenStoreManager};

/// Built-in OAuth client ID, set at compile time via `LILYPAD_GITHUB_CLIENT_ID`.
/// At runtime, the environment variable `LILYPAD_GITHUB_CLIENT_ID` takes precedence.
/// If neither is set, OAuth operations will fail with a clear configuration error.
///
/// ```
/// // When built without the env var, the constant is an empty string.
/// let id = lilypad_oauth::BUILTIN_GITHUB_CLIENT_ID;
/// assert!(id.is_empty() || !id.is_empty()); // value depends on build env
/// ```
pub const BUILTIN_GITHUB_CLIENT_ID: &str = match option_env!("LILYPAD_GITHUB_CLIENT_ID") {
    Some(id) => id,
    None => "",
};

/// Default OAuth scopes required for vault sync.
pub const DEFAULT_GITHUB_SCOPES: &[&str] = &["repo", "read:user"];

/// Vault repository name prefix.
pub const VAULT_REPO_PREFIX: &str = "lilypad-vault";

/// Vault data filename in the repository.
pub const VAULT_DATA_FILENAME: &str = "vault.lily";

/// Sync metadata filename in the repository.
pub const SYNC_META_FILENAME: &str = ".lilypad-meta";
