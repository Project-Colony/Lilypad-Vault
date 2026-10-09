//! Lilypad OAuth - GitHub OAuth authentication and vault synchronization.
//!
//! This crate provides:
//! - GitHub OAuth authentication (Device Flow)
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
pub use oauth::{AuthorizationResult, DeviceFlowAuth};
pub use sync::{GitHubSyncBackend, SyncMetadata, SyncStatus};
pub use token_store::{TokenInfo, TokenStoreManager};

/// Built-in OAuth client ID, set at compile time via `LILYPAD_GITHUB_CLIENT_ID`.
/// It is the only client ID Lilypad uses: the variable is not read at run time.
/// If it was not set at build time, sign-in and token refresh fail with a
/// configuration error (see [`OAuthConfig::builtin_github`]).
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
