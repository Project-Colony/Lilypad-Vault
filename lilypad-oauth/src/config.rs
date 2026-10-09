//! OAuth configuration and provider settings.

use crate::error::{OAuthError, Result};
use serde::{Deserialize, Serialize};

/// OAuth provider enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    /// GitHub OAuth provider.
    #[default]
    GitHub,
}

impl std::fmt::Display for OAuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuthProvider::GitHub => write!(f, "GitHub"),
        }
    }
}

/// OAuth configuration for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// OAuth provider.
    pub provider: OAuthProvider,

    /// OAuth client ID (from GitHub OAuth App settings).
    pub client_id: String,

    /// OAuth scopes to request.
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,

    /// OAuth token URL.
    #[serde(default = "default_token_url")]
    pub token_url: String,

    /// Device authorization URL (for Device Flow).
    #[serde(default = "default_device_auth_url")]
    pub device_auth_url: String,

    /// API base URL.
    #[serde(default = "default_api_url")]
    pub api_url: String,
}

fn default_scopes() -> Vec<String> {
    crate::DEFAULT_GITHUB_SCOPES
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_token_url() -> String {
    "https://github.com/login/oauth/access_token".to_string()
}

fn default_device_auth_url() -> String {
    "https://github.com/login/device/code".to_string()
}

fn default_api_url() -> String {
    "https://api.github.com".to_string()
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            provider: OAuthProvider::GitHub,
            client_id: String::new(),
            scopes: default_scopes(),
            token_url: default_token_url(),
            device_auth_url: default_device_auth_url(),
            api_url: default_api_url(),
        }
    }
}

impl OAuthConfig {
    /// Creates a new OAuth configuration for GitHub.
    pub fn github(client_id: impl Into<String>) -> Self {
        Self {
            provider: OAuthProvider::GitHub,
            client_id: client_id.into(),
            ..Default::default()
        }
    }

    /// The GitHub configuration Lilypad signs in and refreshes tokens with:
    /// the client ID compiled in through `LILYPAD_GITHUB_CLIENT_ID` at build
    /// time ([`BUILTIN_GITHUB_CLIENT_ID`](crate::BUILTIN_GITHUB_CLIENT_ID)) and
    /// the default scopes. Nothing is read from the environment at run time,
    /// so a variable set by another program cannot redirect sign-in or token
    /// refresh to a different OAuth App.
    pub fn builtin_github() -> Result<Self> {
        if crate::BUILTIN_GITHUB_CLIENT_ID.is_empty() {
            return Err(OAuthError::ConfigError(
                "no GitHub OAuth client id was compiled in (build with LILYPAD_GITHUB_CLIENT_ID)"
                    .to_string(),
            ));
        }
        Ok(Self::github(crate::BUILTIN_GITHUB_CLIENT_ID))
    }

    /// Sets custom OAuth scopes.
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.client_id.is_empty() {
            return Err(OAuthError::ConfigError(
                "OAuth client_id is required".to_string(),
            ));
        }

        if self.scopes.is_empty() {
            return Err(OAuthError::ConfigError(
                "at least one OAuth scope is required".to_string(),
            ));
        }

        Ok(())
    }

    /// Returns the scopes as a space-separated string.
    pub fn scopes_string(&self) -> String {
        self.scopes.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OAuthConfig::default();
        assert_eq!(config.provider, OAuthProvider::GitHub);
        assert!(config.client_id.is_empty());
        assert_eq!(config.scopes, crate::DEFAULT_GITHUB_SCOPES);
    }

    #[test]
    fn test_github_config() {
        let config = OAuthConfig::github("test-client-id");
        assert_eq!(config.client_id, "test-client-id");
        assert_eq!(
            config.token_url,
            "https://github.com/login/oauth/access_token"
        );
    }

    #[test]
    fn test_builtin_github_uses_the_compiled_in_client_id() {
        match OAuthConfig::builtin_github() {
            Ok(config) => {
                assert!(!crate::BUILTIN_GITHUB_CLIENT_ID.is_empty());
                assert_eq!(config.client_id, crate::BUILTIN_GITHUB_CLIENT_ID);
                assert_eq!(config.scopes, crate::DEFAULT_GITHUB_SCOPES);
            }
            Err(OAuthError::ConfigError(msg)) => {
                assert!(crate::BUILTIN_GITHUB_CLIENT_ID.is_empty());
                assert!(msg.contains("no GitHub OAuth client id was compiled in"));
            }
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_validation() {
        let empty_config = OAuthConfig::default();
        assert!(empty_config.validate().is_err());

        let valid_config = OAuthConfig::github("client-id");
        assert!(valid_config.validate().is_ok());
    }

    #[test]
    fn test_config_scopes_string() {
        let config = OAuthConfig::github("id123").with_scopes(vec![
            "repo".to_string(),
            "read:user".to_string(),
            "gist".to_string(),
        ]);

        assert_eq!(config.scopes_string(), "repo read:user gist");

        // Single scope
        let single = OAuthConfig::github("id123").with_scopes(vec!["repo".to_string()]);
        assert_eq!(single.scopes_string(), "repo");

        // Default scopes
        let default_cfg = OAuthConfig::github("id123");
        assert_eq!(default_cfg.scopes_string(), "repo read:user");
    }

    #[test]
    fn test_config_validation_missing_client_id() {
        // Empty client_id should fail validation
        let config = OAuthConfig::default();
        assert!(config.client_id.is_empty());

        let result = config.validate();
        assert!(result.is_err());

        let err = result.unwrap_err();
        let msg = format!("{}", err);
        assert!(
            msg.contains("client_id"),
            "Error message should mention client_id, got: {}",
            msg
        );
    }

    #[test]
    fn test_provider_display() {
        let provider = OAuthProvider::GitHub;
        assert_eq!(format!("{}", provider), "GitHub");

        // Verify it also works via to_string (which uses Display)
        assert_eq!(provider.to_string(), "GitHub");
    }
}
