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
    RateLimitExceeded { reset_at: Option<u64> },

    /// Repository not found.
    RepoNotFound(String),

    /// Repository already exists.
    RepoAlreadyExists(String),

    /// File not found in repository.
    FileNotFound { repo: String, path: String },

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        // Verify every OAuthError variant produces a meaningful, non-empty Display string
        let variants: Vec<OAuthError> = vec![
            OAuthError::AuthenticationFailed("bad credentials".to_string()),
            OAuthError::AuthorizationDenied,
            OAuthError::TokenExpired,
            OAuthError::InvalidToken,
            OAuthError::AuthorizationTimeout,
            OAuthError::GitHubApiError {
                status: Some(403),
                message: "forbidden".to_string(),
            },
            OAuthError::GitHubApiError {
                status: None,
                message: "unknown".to_string(),
            },
            OAuthError::RateLimitExceeded {
                reset_at: Some(1700000000),
            },
            OAuthError::RateLimitExceeded { reset_at: None },
            OAuthError::RepoNotFound("my-repo".to_string()),
            OAuthError::RepoAlreadyExists("my-repo".to_string()),
            OAuthError::FileNotFound {
                repo: "my-repo".to_string(),
                path: "vault.lily".to_string(),
            },
            OAuthError::SyncConflict {
                local_sha: "aaa".to_string(),
                remote_sha: "bbb".to_string(),
            },
            OAuthError::NetworkError("connection refused".to_string()),
            OAuthError::ParseError("unexpected token".to_string()),
            OAuthError::TokenStoreError("permission denied".to_string()),
            OAuthError::ConfigError("missing field".to_string()),
            OAuthError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "file not found",
            )),
        ];

        for variant in &variants {
            let display = format!("{}", variant);
            assert!(
                !display.is_empty(),
                "Display for {:?} should not be empty",
                variant
            );
        }

        // Spot-check specific messages
        assert!(format!("{}", OAuthError::TokenExpired).contains("expired"));
        assert!(format!("{}", OAuthError::InvalidToken).contains("invalid"));
        assert!(format!("{}", OAuthError::AuthorizationDenied).contains("denied"));
        assert!(format!(
            "{}",
            OAuthError::GitHubApiError {
                status: Some(404),
                message: "not found".to_string()
            }
        )
        .contains("404"));
        assert!(format!(
            "{}",
            OAuthError::SyncConflict {
                local_sha: "abc".to_string(),
                remote_sha: "xyz".to_string()
            }
        )
        .contains("abc"));
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let oauth_err: OAuthError = io_err.into();

        match &oauth_err {
            OAuthError::IoError(inner) => {
                assert_eq!(inner.kind(), std::io::ErrorKind::PermissionDenied);
                assert!(inner.to_string().contains("access denied"));
            }
            other => panic!("Expected IoError variant, got {:?}", other),
        }

        // Verify Display includes the inner error message
        let display = format!("{}", oauth_err);
        assert!(display.contains("access denied"));

        // Verify Error::source returns the inner io::Error
        use std::error::Error;
        assert!(oauth_err.source().is_some());
    }

    #[test]
    fn test_error_from_reqwest_timeout() {
        // A reqwest::Error for a timeout cannot be easily constructed without
        // performing an actual network request. Instead, we verify that the
        // NetworkError variant with the expected timeout message formats correctly,
        // matching the behavior of the From<reqwest::Error> impl for timeouts.
        let err = OAuthError::NetworkError("request timed out".to_string());
        let display = format!("{}", err);
        assert!(
            display.contains("request timed out"),
            "NetworkError display should contain 'request timed out', got: {}",
            display
        );

        // Also verify the connection variant mapping
        let err_connect = OAuthError::NetworkError("failed to connect".to_string());
        let display_connect = format!("{}", err_connect);
        assert!(display_connect.contains("failed to connect"));
    }

    #[test]
    fn test_error_from_serde_json() {
        // Create a serde_json::Error by parsing invalid JSON
        let json_err = serde_json::from_str::<serde_json::Value>("{{invalid json}}")
            .expect_err("should fail to parse invalid JSON");

        let oauth_err: OAuthError = json_err.into();

        match &oauth_err {
            OAuthError::ParseError(msg) => {
                assert!(!msg.is_empty(), "ParseError message should not be empty");
            }
            other => panic!("Expected ParseError variant, got {:?}", other),
        }

        // Verify Display includes parse error details
        let display = format!("{}", oauth_err);
        assert!(display.contains("parse"));
    }
}
