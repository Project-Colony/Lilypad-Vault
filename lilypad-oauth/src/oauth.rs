//! GitHub OAuth device flow: the user visits a URL and enters a code.

use crate::config::OAuthConfig;
use crate::error::{OAuthError, Result};
use crate::token_store::TokenInfo;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Result of a successful OAuth authorization.
#[derive(Debug, Clone)]
pub struct AuthorizationResult {
    /// Access token for API requests.
    pub access_token: String,
    /// Token type (usually "bearer").
    pub token_type: String,
    /// OAuth scopes granted.
    pub scope: String,
    /// Refresh token (if provided).
    pub refresh_token: Option<String>,
    /// Token expiration time in seconds (if provided).
    pub expires_in: Option<u64>,
}

impl AuthorizationResult {
    /// Converts to TokenInfo for storage.
    pub fn to_token_info(&self) -> TokenInfo {
        TokenInfo::new(
            self.access_token.clone(),
            self.token_type.clone(),
            self.scope.clone(),
        )
        .with_refresh_token(self.refresh_token.clone())
        .with_expires_in(self.expires_in)
    }
}

/// Device Flow authentication handler.
///
/// [`initiate`](Self::initiate) returns the code and URL to show the user,
/// then [`poll_for_token`](Self::poll_for_token) waits until they authorize.
pub struct DeviceFlowAuth {
    config: OAuthConfig,
    client: reqwest::blocking::Client,
}

/// Device authorization response from GitHub.
#[derive(Debug, Deserialize)]
pub struct DeviceAuthResponse {
    /// Code to display to user.
    pub user_code: String,
    /// URL for user to visit.
    pub verification_uri: String,
    /// Device code for polling.
    pub device_code: String,
    /// Polling interval in seconds.
    pub interval: u64,
    /// Code expiration in seconds.
    pub expires_in: u64,
}

impl DeviceFlowAuth {
    /// Creates a new Device Flow authentication handler.
    pub fn new(config: OAuthConfig) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Lilypad-OAuth/1.0")
            .build()?;

        Ok(Self { config, client })
    }

    /// Initiates the device authorization flow.
    ///
    /// Returns the device authorization response containing the user code
    /// and verification URL to display to the user.
    pub fn initiate(&self) -> Result<DeviceAuthResponse> {
        #[derive(Serialize)]
        struct DeviceAuthRequest<'a> {
            client_id: &'a str,
            scope: &'a str,
        }

        #[derive(Deserialize)]
        struct Response {
            device_code: Option<String>,
            user_code: Option<String>,
            verification_uri: Option<String>,
            interval: Option<u64>,
            expires_in: Option<u64>,
            error: Option<String>,
            error_description: Option<String>,
        }

        let request = DeviceAuthRequest {
            client_id: &self.config.client_id,
            scope: &self.config.scopes_string(),
        };

        let response = self
            .client
            .post(&self.config.device_auth_url)
            .header("Accept", "application/json")
            .form(&request)
            .send()?;

        let resp: Response = response.json()?;

        if let Some(error) = resp.error {
            let msg = resp.error_description.unwrap_or(error);
            return Err(OAuthError::AuthenticationFailed(msg));
        }

        Ok(DeviceAuthResponse {
            device_code: resp.device_code.ok_or_else(|| {
                OAuthError::AuthenticationFailed("missing device_code".to_string())
            })?,
            user_code: resp
                .user_code
                .ok_or_else(|| OAuthError::AuthenticationFailed("missing user_code".to_string()))?,
            verification_uri: resp.verification_uri.ok_or_else(|| {
                OAuthError::AuthenticationFailed("missing verification_uri".to_string())
            })?,
            interval: resp.interval.unwrap_or(5),
            expires_in: resp.expires_in.unwrap_or(900),
        })
    }

    /// Polls for the access token after user authorization.
    pub fn poll_for_token(&self, device_code: &str, interval: u64) -> Result<AuthorizationResult> {
        #[derive(Serialize)]
        struct PollRequest<'a> {
            client_id: &'a str,
            device_code: &'a str,
            grant_type: &'a str,
        }

        #[derive(Deserialize)]
        struct PollResponse {
            access_token: Option<String>,
            token_type: Option<String>,
            scope: Option<String>,
            refresh_token: Option<String>,
            expires_in: Option<u64>,
            error: Option<String>,
            error_description: Option<String>,
        }

        let request = PollRequest {
            client_id: &self.config.client_id,
            device_code,
            grant_type: "urn:ietf:params:oauth:grant-type:device_code",
        };

        let poll_interval = Duration::from_secs(interval.max(5));

        loop {
            std::thread::sleep(poll_interval);

            let response = self
                .client
                .post(&self.config.token_url)
                .header("Accept", "application/json")
                .form(&request)
                .send()?;

            let resp: PollResponse = response.json()?;

            // Check for success
            if let Some(access_token) = resp.access_token {
                return Ok(AuthorizationResult {
                    access_token,
                    token_type: resp.token_type.unwrap_or_else(|| "bearer".to_string()),
                    scope: resp.scope.unwrap_or_default(),
                    refresh_token: resp.refresh_token,
                    expires_in: resp.expires_in,
                });
            }

            // Check for errors
            if let Some(error) = resp.error {
                match error.as_str() {
                    "authorization_pending" => {
                        // User hasn't authorized yet, keep polling
                        continue;
                    }
                    "slow_down" => {
                        // We're polling too fast, add extra delay
                        std::thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                    "expired_token" => {
                        return Err(OAuthError::AuthorizationTimeout);
                    }
                    "access_denied" => {
                        return Err(OAuthError::AuthorizationDenied);
                    }
                    _ => {
                        let msg = resp.error_description.unwrap_or(error);
                        return Err(OAuthError::AuthenticationFailed(msg));
                    }
                }
            }
        }
    }
}
