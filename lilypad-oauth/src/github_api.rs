//! GitHub API client for repository and file operations.
//!
//! This module provides a typed client for interacting with the GitHub API,
//! specifically for managing vault repositories and their contents.

use crate::error::{OAuthError, Result};
use crate::{SYNC_META_FILENAME, VAULT_DATA_FILENAME, VAULT_REPO_PREFIX};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// GitHub API base URL.
const GITHUB_API_URL: &str = "https://api.github.com";

/// GitHub user information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    /// GitHub username/login.
    pub login: String,
    /// User ID.
    pub id: u64,
    /// Display name.
    pub name: Option<String>,
    /// Email (may be private).
    pub email: Option<String>,
    /// Avatar URL.
    pub avatar_url: Option<String>,
}

/// GitHub repository information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRepo {
    /// Repository ID.
    pub id: u64,
    /// Repository name.
    pub name: String,
    /// Full repository name (owner/repo).
    pub full_name: String,
    /// Whether the repo is private.
    pub private: bool,
    /// Repository description.
    pub description: Option<String>,
    /// Default branch name.
    pub default_branch: String,
    /// Clone URL (HTTPS).
    pub clone_url: String,
    /// SSH URL.
    pub ssh_url: String,
    /// Created timestamp.
    pub created_at: String,
    /// Last updated timestamp.
    pub updated_at: String,
}

/// GitHub file content response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubFileContent {
    /// File name.
    pub name: String,
    /// File path in repository.
    pub path: String,
    /// SHA of the file (used for updates).
    pub sha: String,
    /// File size in bytes.
    pub size: u64,
    /// Content encoding (usually "base64").
    pub encoding: Option<String>,
    /// Base64-encoded content.
    pub content: Option<String>,
}

/// Request to create or update a file.
#[derive(Debug, Serialize)]
struct CreateOrUpdateFileRequest<'a> {
    message: &'a str,
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<&'a str>,
}

/// Request to create a repository.
#[derive(Debug, Serialize)]
struct CreateRepoRequest<'a> {
    name: &'a str,
    description: &'a str,
    private: bool,
    auto_init: bool,
}

/// GitHub API client.
pub struct GitHubClient {
    client: reqwest::blocking::Client,
    access_token: String,
    api_url: String,
}

impl GitHubClient {
    /// Creates a new GitHub API client.
    pub fn new(access_token: impl Into<String>) -> Result<Self> {
        Self::with_api_url(access_token, GITHUB_API_URL)
    }

    /// Creates a new GitHub API client with a custom API URL.
    pub fn with_api_url(access_token: impl Into<String>, api_url: impl Into<String>) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Lilypad-Vault/1.0")
            .build()?;

        Ok(Self {
            client,
            access_token: access_token.into(),
            api_url: api_url.into(),
        })
    }

    /// Gets the authenticated user's information.
    pub fn get_user(&self) -> Result<GitHubUser> {
        let url = format!("{}/user", self.api_url);
        self.get(&url)
    }

    /// Lists repositories for the authenticated user.
    pub fn list_repos(&self) -> Result<Vec<GitHubRepo>> {
        let url = format!("{}/user/repos?type=owner&per_page=100", self.api_url);
        self.get(&url)
    }

    /// Gets a specific repository.
    pub fn get_repo(&self, owner: &str, repo: &str) -> Result<GitHubRepo> {
        let url = format!("{}/repos/{}/{}", self.api_url, owner, repo);
        self.get(&url)
    }

    /// Creates a new private repository.
    pub fn create_repo(&self, name: &str, description: &str) -> Result<GitHubRepo> {
        let url = format!("{}/user/repos", self.api_url);
        let request = CreateRepoRequest {
            name,
            description,
            private: true,
            auto_init: true,
        };
        self.post(&url, &request)
    }

    /// Deletes a repository.
    pub fn delete_repo(&self, owner: &str, repo: &str) -> Result<()> {
        let url = format!("{}/repos/{}/{}", self.api_url, owner, repo);
        self.delete(&url)
    }

    /// Gets file content from a repository.
    pub fn get_file(&self, owner: &str, repo: &str, path: &str) -> Result<GitHubFileContent> {
        let url = format!("{}/repos/{}/{}/contents/{}", self.api_url, owner, repo, path);
        self.get(&url)
    }

    /// Creates or updates a file in a repository.
    pub fn put_file(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        content: &[u8],
        message: &str,
        sha: Option<&str>,
    ) -> Result<GitHubFileContent> {
        let url = format!("{}/repos/{}/{}/contents/{}", self.api_url, owner, repo, path);

        // Base64 encode the content
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, content);

        let request = CreateOrUpdateFileRequest {
            message,
            content: &encoded,
            sha,
            branch: None,
        };

        #[derive(Deserialize)]
        struct PutFileResponse {
            content: GitHubFileContent,
        }

        let response: PutFileResponse = self.put(&url, &request)?;
        Ok(response.content)
    }

    /// Deletes a file from a repository.
    pub fn delete_file(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        sha: &str,
        message: &str,
    ) -> Result<()> {
        let url = format!("{}/repos/{}/{}/contents/{}", self.api_url, owner, repo, path);

        #[derive(Serialize)]
        struct DeleteFileRequest<'a> {
            message: &'a str,
            sha: &'a str,
        }

        let request = DeleteFileRequest { message, sha };

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&request)
            .send()?;

        self.handle_response_status(&response)?;
        Ok(())
    }

    /// Checks if a vault repository exists for the user.
    pub fn vault_repo_exists(&self, username: &str) -> Result<bool> {
        let repo_name = format!("{}-{}", VAULT_REPO_PREFIX, username);
        match self.get_repo(username, &repo_name) {
            Ok(_) => Ok(true),
            Err(OAuthError::RepoNotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Gets or creates the vault repository for a user.
    pub fn get_or_create_vault_repo(&self, username: &str) -> Result<GitHubRepo> {
        let repo_name = format!("{}-{}", VAULT_REPO_PREFIX, username);

        match self.get_repo(username, &repo_name) {
            Ok(repo) => Ok(repo),
            Err(OAuthError::RepoNotFound(_)) => {
                let description = "Lilypad encrypted vault storage (do not edit manually)";
                self.create_repo(&repo_name, description)
            }
            Err(e) => Err(e),
        }
    }

    /// Gets the vault data file from the vault repository.
    pub fn get_vault_data(&self, username: &str) -> Result<Option<(Vec<u8>, String)>> {
        let repo_name = format!("{}-{}", VAULT_REPO_PREFIX, username);

        match self.get_file(username, &repo_name, VAULT_DATA_FILENAME) {
            Ok(file) => {
                let content = file.content.ok_or_else(|| {
                    OAuthError::ParseError("file content is empty".to_string())
                })?;

                // Decode base64 content (GitHub returns content with newlines)
                let cleaned: String = content.chars().filter(|c| !c.is_whitespace()).collect();
                let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &cleaned)
                    .map_err(|e| OAuthError::ParseError(format!("invalid base64: {}", e)))?;

                Ok(Some((decoded, file.sha)))
            }
            Err(OAuthError::FileNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Saves vault data to the vault repository.
    pub fn save_vault_data(
        &self,
        username: &str,
        data: &[u8],
        sha: Option<&str>,
    ) -> Result<String> {
        let repo_name = format!("{}-{}", VAULT_REPO_PREFIX, username);

        // Ensure repo exists
        self.get_or_create_vault_repo(username)?;

        let message = if sha.is_some() {
            "Update encrypted vault"
        } else {
            "Initial vault upload"
        };

        let file = self.put_file(username, &repo_name, VAULT_DATA_FILENAME, data, message, sha)?;
        Ok(file.sha)
    }

    /// Gets the sync metadata from the vault repository.
    pub fn get_sync_metadata(&self, username: &str) -> Result<Option<(String, String)>> {
        let repo_name = format!("{}-{}", VAULT_REPO_PREFIX, username);

        match self.get_file(username, &repo_name, SYNC_META_FILENAME) {
            Ok(file) => {
                let content = file.content.ok_or_else(|| {
                    OAuthError::ParseError("file content is empty".to_string())
                })?;

                let cleaned: String = content.chars().filter(|c| !c.is_whitespace()).collect();
                let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &cleaned)
                    .map_err(|e| OAuthError::ParseError(format!("invalid base64: {}", e)))?;

                let meta = String::from_utf8(decoded)
                    .map_err(|e| OAuthError::ParseError(format!("invalid UTF-8: {}", e)))?;

                Ok(Some((meta, file.sha)))
            }
            Err(OAuthError::FileNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Saves sync metadata to the vault repository.
    pub fn save_sync_metadata(
        &self,
        username: &str,
        metadata: &str,
        sha: Option<&str>,
    ) -> Result<String> {
        let repo_name = format!("{}-{}", VAULT_REPO_PREFIX, username);

        let message = "Update sync metadata";
        let file = self.put_file(
            username,
            &repo_name,
            SYNC_META_FILENAME,
            metadata.as_bytes(),
            message,
            sha,
        )?;
        Ok(file.sha)
    }

    /// Makes a GET request to the GitHub API.
    fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()?;

        self.handle_response(response)
    }

    /// Makes a POST request to the GitHub API.
    fn post<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()?;

        self.handle_response(response)
    }

    /// Makes a PUT request to the GitHub API.
    fn put<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        let response = self
            .client
            .put(url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()?;

        self.handle_response(response)
    }

    /// Makes a DELETE request to the GitHub API.
    fn delete(&self, url: &str) -> Result<()> {
        let response = self
            .client
            .delete(url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()?;

        self.handle_response_status(&response)?;
        Ok(())
    }

    /// Handles the API response and parses JSON.
    fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::blocking::Response,
    ) -> Result<T> {
        let status = response.status();
        let url = response.url().to_string();

        // Check rate limiting
        if status == reqwest::StatusCode::FORBIDDEN {
            if let Some(remaining) = response.headers().get("x-ratelimit-remaining") {
                if remaining.to_str().unwrap_or("1") == "0" {
                    let reset = response
                        .headers()
                        .get("x-ratelimit-reset")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse().ok());
                    return Err(OAuthError::RateLimitExceeded { reset_at: reset });
                }
            }
        }

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(OAuthError::InvalidToken);
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            // Determine if it's a repo or file not found
            if url.contains("/contents/") {
                let parts: Vec<&str> = url.split('/').collect();
                let repo = parts.get(5).unwrap_or(&"unknown").to_string();
                let path = parts.get(7..).map(|p| p.join("/")).unwrap_or_default();
                return Err(OAuthError::FileNotFound { repo, path });
            } else {
                let repo = url.split('/').next_back().unwrap_or("unknown").to_string();
                return Err(OAuthError::RepoNotFound(repo));
            }
        }

        if !status.is_success() {
            #[derive(Deserialize)]
            struct ErrorResponse {
                message: Option<String>,
            }

            let error_msg = response
                .json::<ErrorResponse>()
                .ok()
                .and_then(|e| e.message)
                .unwrap_or_else(|| format!("HTTP {}", status));

            return Err(OAuthError::GitHubApiError {
                status: Some(status.as_u16()),
                message: error_msg,
            });
        }

        response.json().map_err(|e| OAuthError::ParseError(e.to_string()))
    }

    /// Handles response status without parsing body.
    fn handle_response_status(&self, response: &reqwest::blocking::Response) -> Result<()> {
        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(OAuthError::InvalidToken);
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(OAuthError::RepoNotFound("unknown".to_string()));
        }

        if !status.is_success() {
            return Err(OAuthError::GitHubApiError {
                status: Some(status.as_u16()),
                message: format!("HTTP {}", status),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_repo_name() {
        let username = "testuser";
        let expected = format!("{}-{}", VAULT_REPO_PREFIX, username);
        assert_eq!(expected, "lilypad-vault-testuser");
    }
}
