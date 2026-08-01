//! OAuth authentication flows for GitHub.
//!
//! Supports two authentication methods:
//! 1. Device Flow (recommended for CLI) - User visits URL and enters code
//! 2. Authorization Code Flow - Opens browser with local callback server

use crate::config::{AuthFlow, OAuthConfig};
use crate::error::{OAuthError, Result};
use crate::token_store::TokenInfo;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

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

/// Main OAuth flow handler.
pub struct OAuthFlow {
    config: OAuthConfig,
    client: reqwest::blocking::Client,
}

impl OAuthFlow {
    /// Creates a new OAuth flow handler.
    pub fn new(config: OAuthConfig) -> Result<Self> {
        config.validate()?;

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Lilypad-OAuth/1.0")
            .build()?;

        Ok(Self { config, client })
    }

    /// Performs authentication using the configured flow.
    pub fn authenticate(&self) -> Result<AuthorizationResult> {
        match self.config.preferred_flow {
            AuthFlow::DeviceFlow => self.device_flow_authenticate(),
            AuthFlow::AuthorizationCode => self.authorization_code_flow(),
        }
    }

    /// Performs Device Flow authentication.
    fn device_flow_authenticate(&self) -> Result<AuthorizationResult> {
        let device_auth = DeviceFlowAuth::new(self.config.clone())?;
        device_auth.authenticate()
    }

    /// Performs Authorization Code Flow authentication.
    fn authorization_code_flow(&self) -> Result<AuthorizationResult> {
        let state = generate_state();
        let auth_url = self.build_authorization_url(&state)?;

        // Start local server to receive callback
        let server = tiny_http::Server::http(format!("127.0.0.1:{}", self.config.callback_port))
            .map_err(|e| OAuthError::LocalServerError(e.to_string()))?;

        // Open browser
        println!("Opening browser for GitHub authorization...");
        println!("If browser doesn't open, visit: {}", auth_url);

        if webbrowser::open(&auth_url).is_err() {
            println!("Failed to open browser automatically.");
        }

        // Wait for callback (with timeout)
        let timeout = Duration::from_secs(300); // 5 minutes
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(OAuthError::AuthorizationTimeout);
            }

            // Non-blocking receive with short timeout
            if let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(500)) {
                let url = request.url().to_string();

                // Parse callback URL
                if url.starts_with("/callback") {
                    let (code, received_state) = self.parse_callback_url(&url)?;

                    // Verify state
                    if received_state != state {
                        let response = tiny_http::Response::from_string(
                            "Error: State mismatch. Please try again.",
                        );
                        let _ = request.respond(response);
                        return Err(OAuthError::AuthenticationFailed(
                            "state parameter mismatch".to_string(),
                        ));
                    }

                    // Send success response to browser
                    let html = r#"
                        <!DOCTYPE html>
                        <html>
                        <head><title>Lilypad - Authorization Successful</title></head>
                        <body style="font-family: system-ui; text-align: center; padding: 50px;">
                            <h1>Authorization Successful!</h1>
                            <p>You can close this window and return to Lilypad.</p>
                        </body>
                        </html>
                    "#;
                    let response = tiny_http::Response::from_string(html).with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..])
                            .expect("valid static Content-Type header"),
                    );
                    let _ = request.respond(response);

                    // Exchange code for token
                    return self.exchange_code_for_token(&code);
                }
            }
        }
    }

    /// Builds the authorization URL for browser redirect.
    fn build_authorization_url(&self, state: &str) -> Result<String> {
        let mut url = url::Url::parse(&self.config.auth_url)?;
        url.query_pairs_mut()
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", &self.config.callback_url())
            .append_pair("scope", &self.config.scopes_string())
            .append_pair("state", state)
            .append_pair("response_type", "code");

        Ok(url.to_string())
    }

    /// Parses the callback URL to extract code and state.
    fn parse_callback_url(&self, url: &str) -> Result<(String, String)> {
        let full_url = format!("http://localhost{}", url);
        let parsed = url::Url::parse(&full_url)?;

        let mut code = None;
        let mut state = None;
        let mut error = None;

        for (key, value) in parsed.query_pairs() {
            match key.as_ref() {
                "code" => code = Some(value.to_string()),
                "state" => state = Some(value.to_string()),
                "error" => error = Some(value.to_string()),
                _ => {}
            }
        }

        if let Some(err) = error {
            if err == "access_denied" {
                return Err(OAuthError::AuthorizationDenied);
            }
            return Err(OAuthError::AuthenticationFailed(err));
        }

        match (code, state) {
            (Some(c), Some(s)) => Ok((c, s)),
            _ => Err(OAuthError::AuthenticationFailed(
                "missing code or state in callback".to_string(),
            )),
        }
    }

    /// Exchanges authorization code for access token.
    fn exchange_code_for_token(&self, code: &str) -> Result<AuthorizationResult> {
        #[derive(Serialize)]
        struct TokenRequest<'a> {
            client_id: &'a str,
            client_secret: &'a str,
            code: &'a str,
            redirect_uri: &'a str,
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: Option<String>,
            token_type: Option<String>,
            scope: Option<String>,
            refresh_token: Option<String>,
            expires_in: Option<u64>,
            error: Option<String>,
            error_description: Option<String>,
        }

        let client_secret = self.config.client_secret.as_ref().ok_or_else(|| {
            OAuthError::ConfigError("client_secret required for token exchange".to_string())
        })?;

        let request = TokenRequest {
            client_id: &self.config.client_id,
            client_secret,
            code,
            redirect_uri: &self.config.callback_url(),
        };

        let response = self
            .client
            .post(&self.config.token_url)
            .header("Accept", "application/json")
            .form(&request)
            .send()?;

        let token_response: TokenResponse = response.json()?;

        if let Some(error) = token_response.error {
            let msg = token_response
                .error_description
                .unwrap_or_else(|| error.clone());
            return Err(OAuthError::AuthenticationFailed(msg));
        }

        let access_token = token_response.access_token.ok_or_else(|| {
            OAuthError::AuthenticationFailed("no access_token in response".to_string())
        })?;

        Ok(AuthorizationResult {
            access_token,
            token_type: token_response
                .token_type
                .unwrap_or_else(|| "bearer".to_string()),
            scope: token_response.scope.unwrap_or_default(),
            refresh_token: token_response.refresh_token,
            expires_in: token_response.expires_in,
        })
    }
}

/// Device Flow authentication handler.
///
/// This is the recommended flow for CLI applications.
/// The user visits a URL and enters a code to authorize.
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

    /// Performs the complete Device Flow authentication.
    ///
    /// This method:
    /// 1. Initiates the device authorization
    /// 2. Displays the user code and verification URL
    /// 3. Optionally opens the browser
    /// 4. Polls for the access token
    pub fn authenticate(&self) -> Result<AuthorizationResult> {
        // Step 1: Initiate device authorization
        let auth = self.initiate()?;

        // Step 2: Display instructions to user
        println!();
        println!("=== GitHub Authorization Required ===");
        println!();
        println!("To authorize Lilypad, please:");
        println!("1. Visit: {}", auth.verification_uri);
        println!("2. Enter code: {}", auth.user_code);
        println!();
        println!(
            "Waiting for authorization (expires in {} seconds)...",
            auth.expires_in
        );
        println!();

        // Optionally open browser
        let _ = webbrowser::open(&auth.verification_uri);

        // Step 3: Poll for token
        self.poll_for_token(&auth.device_code, auth.interval)
    }
}

/// Generates a random state parameter for CSRF protection.
fn generate_state() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 32] = rng.random();
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_state() {
        let state1 = generate_state();
        let state2 = generate_state();

        // States should be unique
        assert_ne!(state1, state2);

        // States should be URL-safe base64
        assert!(state1
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
    }
}
