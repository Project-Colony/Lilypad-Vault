//! Error types for OAuth and GitHub operations.

use std::fmt;

/// Result type alias for OAuth operations.
pub type Result<T> = std::result::Result<T, OAuthError>;

/// Errors that can occur during OAuth and GitHub operations.
#[derive(Debug)]
pub enum OAuthError {
    /// OAuth authentication failed.
    AuthenticationFailed(String),

    /// OAuth authorization was denied by the user.
    AuthorizationDenied,

    /// OAuth token has expired.
    TokenExpired,

    /// OAuth token is invalid or revoked.
    InvalidToken,

    /// OAuth flow timed out waiting for user authorization.
    AuthorizationTimeout,

    /// GitHub API request failed.
    GitHubApiError {
        status: Option<u16>,
        message: String,
    },

    /// GitHub rate limit exceeded.
    RateLimitExceeded {
        reset_at: Option<u64>,
    },

    /// Repository not found.
    RepoNotFound(String),

    /// Repository already exists.
    RepoAlreadyExists(String),

    /// File not found in repository.
    FileNotFound {
        repo: String,
        path: String,
    },

    /// Conflict during sync (file was modified remotely).
    SyncConflict {
        local_sha: String,
        remote_sha: String,
    },

    /// Network error during request.
    NetworkError(String),

    /// Failed to parse response.
    ParseError(String),

    /// Token storage error.
    TokenStoreError(String),

    /// Configuration error.
    ConfigError(String),

    /// Local server error (for OAuth callback).
    LocalServerError(String),

    /// Generic IO error.
    IoError(std::io::Error),
}

impl fmt::Display for OAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OAuthError::AuthenticationFailed(msg) => {
                write!(f, "OAuth authentication failed: {}", msg)
            }
            OAuthError::AuthorizationDenied => {
                write!(f, "OAuth authorization was denied by the user")
            }
            OAuthError::TokenExpired => {
                write!(f, "OAuth token has expired, please re-authenticate")
            }
            OAuthError::InvalidToken => {
                write!(f, "OAuth token is invalid or has been revoked")
            }
            OAuthError::AuthorizationTimeout => {
                write!(f, "OAuth authorization timed out waiting for user")
            }
            OAuthError::GitHubApiError { status, message } => {
                if let Some(code) = status {
                    write!(f, "GitHub API error ({}): {}", code, message)
                } else {
                    write!(f, "GitHub API error: {}", message)
                }
            }
            OAuthError::RateLimitExceeded { reset_at } => {
                if let Some(reset) = reset_at {
                    write!(f, "GitHub API rate limit exceeded, resets at {}", reset)
                } else {
                    write!(f, "GitHub API rate limit exceeded")
                }
            }
            OAuthError::RepoNotFound(repo) => {
                write!(f, "Repository not found: {}", repo)
            }
            OAuthError::RepoAlreadyExists(repo) => {
                write!(f, "Repository already exists: {}", repo)
            }
            OAuthError::FileNotFound { repo, path } => {
                write!(f, "File not found in {}: {}", repo, path)
            }
            OAuthError::SyncConflict {
                local_sha,
                remote_sha,
            } => {
                write!(
                    f,
                    "Sync conflict: local SHA {} differs from remote SHA {}",
                    local_sha, remote_sha
                )
            }
            OAuthError::NetworkError(msg) => {
                write!(f, "Network error: {}", msg)
            }
            OAuthError::ParseError(msg) => {
                write!(f, "Failed to parse response: {}", msg)
            }
            OAuthError::TokenStoreError(msg) => {
                write!(f, "Token storage error: {}", msg)
            }
            OAuthError::ConfigError(msg) => {
                write!(f, "Configuration error: {}", msg)
            }
            OAuthError::LocalServerError(msg) => {
                write!(f, "Local OAuth server error: {}", msg)
            }
            OAuthError::IoError(err) => {
                write!(f, "IO error: {}", err)
            }
        }
    }
}

impl std::error::Error for OAuthError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            OAuthError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for OAuthError {
    fn from(err: std::io::Error) -> Self {
        OAuthError::IoError(err)
    }
}

impl From<reqwest::Error> for OAuthError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            OAuthError::NetworkError("request timed out".to_string())
        } else if err.is_connect() {
            OAuthError::NetworkError("failed to connect".to_string())
        } else {
            OAuthError::NetworkError(err.to_string())
        }
    }
}

impl From<serde_json::Error> for OAuthError {
    fn from(err: serde_json::Error) -> Self {
        OAuthError::ParseError(err.to_string())
    }
}

impl From<url::ParseError> for OAuthError {
    fn from(err: url::ParseError) -> Self {
        OAuthError::ConfigError(format!("invalid URL: {}", err))
    }
}
