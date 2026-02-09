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

    /// OAuth client secret (optional, required for Authorization Code Flow).
    /// For Device Flow, this is not required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// OAuth scopes to request.
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,

    /// OAuth authorization URL.
    #[serde(default = "default_auth_url")]
    pub auth_url: String,

    /// OAuth token URL.
    #[serde(default = "default_token_url")]
    pub token_url: String,

    /// Device authorization URL (for Device Flow).
    #[serde(default = "default_device_auth_url")]
    pub device_auth_url: String,

    /// API base URL.
    #[serde(default = "default_api_url")]
    pub api_url: String,

    /// Local callback port for Authorization Code Flow.
    #[serde(default = "default_callback_port")]
    pub callback_port: u16,

    /// Preferred authentication flow.
    #[serde(default)]
    pub preferred_flow: AuthFlow,
}

/// Authentication flow preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthFlow {
    /// Device Flow (recommended for CLI applications).
    /// User visits a URL and enters a code.
    #[default]
    DeviceFlow,

    /// Authorization Code Flow with local callback server.
    /// Opens browser and receives callback on localhost.
    AuthorizationCode,
}

fn default_scopes() -> Vec<String> {
    vec!["repo".to_string(), "read:user".to_string()]
}

fn default_auth_url() -> String {
    "https://github.com/login/oauth/authorize".to_string()
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

fn default_callback_port() -> u16 {
    8585
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            provider: OAuthProvider::GitHub,
            client_id: String::new(),
            client_secret: None,
            scopes: default_scopes(),
            auth_url: default_auth_url(),
            token_url: default_token_url(),
            device_auth_url: default_device_auth_url(),
            api_url: default_api_url(),
            callback_port: default_callback_port(),
            preferred_flow: AuthFlow::default(),
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

    /// Sets the client secret (for Authorization Code Flow).
    pub fn with_client_secret(mut self, secret: impl Into<String>) -> Self {
        self.client_secret = Some(secret.into());
        self
    }

    /// Sets custom OAuth scopes.
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    /// Sets the preferred authentication flow.
    pub fn with_flow(mut self, flow: AuthFlow) -> Self {
        self.preferred_flow = flow;
        self
    }

    /// Sets the callback port for Authorization Code Flow.
    pub fn with_callback_port(mut self, port: u16) -> Self {
        self.callback_port = port;
        self
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.client_id.is_empty() {
            return Err(OAuthError::ConfigError(
                "OAuth client_id is required".to_string(),
            ));
        }

        if self.preferred_flow == AuthFlow::AuthorizationCode && self.client_secret.is_none() {
            return Err(OAuthError::ConfigError(
                "client_secret is required for Authorization Code Flow".to_string(),
            ));
        }

        if self.scopes.is_empty() {
            return Err(OAuthError::ConfigError(
                "at least one OAuth scope is required".to_string(),
            ));
        }

        Ok(())
    }

    /// Creates configuration from environment variables.
    ///
    /// Reads the following environment variables:
    /// - `LILYPAD_GITHUB_CLIENT_ID` - OAuth client ID (required)
    /// - `LILYPAD_GITHUB_CLIENT_SECRET` - OAuth client secret (optional)
    /// - `LILYPAD_GITHUB_SCOPES` - Comma-separated scopes (optional)
    /// - `LILYPAD_OAUTH_FLOW` - `device` or `authorization_code` (optional)
    /// - `LILYPAD_OAUTH_PORT` - Callback port (optional)
    pub fn from_env() -> Result<Self> {
        let client_id = std::env::var("LILYPAD_GITHUB_CLIENT_ID").map_err(|_| {
            OAuthError::ConfigError(
                "LILYPAD_GITHUB_CLIENT_ID environment variable is required".to_string(),
            )
        })?;

        let mut config = Self::github(client_id);

        if let Ok(secret) = std::env::var("LILYPAD_GITHUB_CLIENT_SECRET") {
            config.client_secret = Some(secret);
        }

        if let Ok(scopes) = std::env::var("LILYPAD_GITHUB_SCOPES") {
            config.scopes = scopes.split(',').map(|s| s.trim().to_string()).collect();
        }

        if let Ok(flow) = std::env::var("LILYPAD_OAUTH_FLOW") {
            config.preferred_flow = match flow.to_lowercase().as_str() {
                "device" | "device_flow" => AuthFlow::DeviceFlow,
                "authorization_code" | "auth_code" => AuthFlow::AuthorizationCode,
                _ => AuthFlow::DeviceFlow,
            };
        }

        if let Ok(port) = std::env::var("LILYPAD_OAUTH_PORT") {
            if let Ok(p) = port.parse() {
                config.callback_port = p;
            }
        }

        config.validate()?;
        Ok(config)
    }

    /// Returns the callback URL for Authorization Code Flow.
    pub fn callback_url(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.callback_port)
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
        assert!(config.client_secret.is_none());
        assert!(!config.scopes.is_empty());
    }

    #[test]
    fn test_github_config() {
        let config = OAuthConfig::github("test-client-id")
            .with_client_secret("test-secret")
            .with_flow(AuthFlow::AuthorizationCode);

        assert_eq!(config.client_id, "test-client-id");
        assert_eq!(config.client_secret, Some("test-secret".to_string()));
        assert_eq!(config.preferred_flow, AuthFlow::AuthorizationCode);
    }

    #[test]
    fn test_validation() {
        let empty_config = OAuthConfig::default();
        assert!(empty_config.validate().is_err());

        let valid_config = OAuthConfig::github("client-id");
        assert!(valid_config.validate().is_ok());

        let invalid_auth_code = OAuthConfig::github("client-id")
            .with_flow(AuthFlow::AuthorizationCode);
        assert!(invalid_auth_code.validate().is_err());
    }

    #[test]
    fn test_config_scopes_string() {
        let config = OAuthConfig::github("id123")
            .with_scopes(vec![
                "repo".to_string(),
                "read:user".to_string(),
                "gist".to_string(),
            ]);

        assert_eq!(config.scopes_string(), "repo read:user gist");

        // Single scope
        let single = OAuthConfig::github("id123")
            .with_scopes(vec!["repo".to_string()]);
        assert_eq!(single.scopes_string(), "repo");

        // Default scopes
        let default_cfg = OAuthConfig::github("id123");
        assert_eq!(default_cfg.scopes_string(), "repo read:user");
    }

    #[test]
    fn test_config_callback_url() {
        let config = OAuthConfig::github("id123");
        assert_eq!(
            config.callback_url(),
            "http://127.0.0.1:8585/callback",
            "Default callback URL should use port 8585"
        );

        let custom = OAuthConfig::github("id123").with_callback_port(9999);
        assert_eq!(
            custom.callback_url(),
            "http://127.0.0.1:9999/callback",
            "Custom port should be reflected in callback URL"
        );
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
