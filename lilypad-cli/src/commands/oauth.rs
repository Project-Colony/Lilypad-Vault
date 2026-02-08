//! OAuth-related CLI commands for GitHub authentication and vault sync.

use anyhow::{anyhow, Result};
use lilypad_common::keyfile::load_key;
use lilypad_oauth::{
    GitHubSyncBackend, OAuthConfig, OAuthProvider, SyncStatus, TokenStoreManager,
};

use super::utils::{key_path, OutputFormat};

/// Logs in to GitHub using OAuth.
pub fn login(output_format: OutputFormat) -> Result<()> {
    // Check if already logged in
    if GitHubSyncBackend::has_valid_token()? {
        let token_store = TokenStoreManager::new()?;
        if let Some(token) = token_store.load_token(OAuthProvider::GitHub)? {
            if let Some(ref username) = token.username {
                if output_format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "already_authenticated",
                            "username": username
                        })
                    );
                } else {
                    println!("Already logged in as: {}", username);
                    println!("Use 'lilypad logout' to sign out first.");
                }
                return Ok(());
            }
        }
    }

    // Load OAuth config from environment
    let config = OAuthConfig::from_env().map_err(|e| {
        anyhow!(
            "OAuth not configured. Please set environment variables:\n\
             - LILYPAD_GITHUB_CLIENT_ID (required)\n\
             - LILYPAD_GITHUB_CLIENT_SECRET (optional, for auth code flow)\n\n\
             Error: {}",
            e
        )
    })?;

    // Perform authentication
    let backend = GitHubSyncBackend::authenticate(config)?;
    let username = backend.username().to_string();

    if output_format == OutputFormat::Json {
        println!(
            "{}",
            serde_json::json!({
                "status": "authenticated",
                "username": username
            })
        );
    } else {
        println!();
        println!("Successfully authenticated as: {}", username);
        println!();
        println!("Your vault repository will be created at:");
        println!("  https://github.com/{}/lilypad-vault-{}", username, username);
        println!();
        println!("Use 'lilypad sync push <vault>' to sync your vault to GitHub.");
    }

    Ok(())
}

/// Logs out from GitHub OAuth.
pub fn logout(output_format: OutputFormat) -> Result<()> {
    if !GitHubSyncBackend::has_valid_token()? {
        if output_format == OutputFormat::Json {
            println!(
                "{}",
                serde_json::json!({
                    "status": "not_authenticated"
                })
            );
        } else {
            println!("Not currently logged in.");
        }
        return Ok(());
    }

    GitHubSyncBackend::logout()?;

    if output_format == OutputFormat::Json {
        println!(
            "{}",
            serde_json::json!({
                "status": "logged_out"
            })
        );
    } else {
        println!("Successfully logged out from GitHub.");
        println!();
        println!("Note: Your tokens have been removed locally.");
        println!("To fully revoke access, visit:");
        println!("  https://github.com/settings/applications");
    }

    Ok(())
}

/// Shows the current OAuth status.
pub fn status(output_format: OutputFormat) -> Result<()> {
    let token_store = TokenStoreManager::new()?;

    if !GitHubSyncBackend::has_valid_token()? {
        if output_format == OutputFormat::Json {
            println!(
                "{}",
                serde_json::json!({
                    "authenticated": false
                })
            );
        } else {
            println!("Status: Not authenticated");
            println!();
            println!("Use 'lilypad login' to authenticate with GitHub.");
        }
        return Ok(());
    }

    let token = token_store.load_token(OAuthProvider::GitHub)?;

    if let Some(token_info) = token {
        let backend = GitHubSyncBackend::from_stored_token()?;
        let repo_info = backend.get_repo_info()?;

        if output_format == OutputFormat::Json {
            let repo_data = repo_info.as_ref().map(|r| {
                serde_json::json!({
                    "name": r.full_name,
                    "url": format!("https://github.com/{}", r.full_name),
                    "private": r.private
                })
            });

            println!(
                "{}",
                serde_json::json!({
                    "authenticated": true,
                    "username": token_info.username,
                    "scopes": token_info.scope,
                    "repository": repo_data
                })
            );
        } else {
            println!("Status: Authenticated");
            println!();
            if let Some(username) = &token_info.username {
                println!("  GitHub User: {}", username);
            }
            println!("  Scopes: {}", token_info.scope);

            if let Some(repo) = repo_info {
                println!();
                println!("Vault Repository:");
                println!("  Name: {}", repo.full_name);
                println!("  URL: https://github.com/{}", repo.full_name);
                println!("  Private: {}", if repo.private { "Yes" } else { "No" });
            } else {
                println!();
                println!("Vault Repository: Not created yet");
                println!("  Use 'lilypad sync push <vault>' to create and sync.");
            }
        }
    }

    Ok(())
}

/// Pushes a vault to GitHub.
pub fn sync_push(
    store: &lilypad_storage::LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    force: bool,
    master_password: Option<&str>,
    output_format: OutputFormat,
) -> Result<()> {
    // Check authentication
    if !GitHubSyncBackend::has_valid_token()? {
        return Err(anyhow!(
            "Not authenticated. Use 'lilypad login' first."
        ));
    }

    // Load vault to get the encrypted payload
    let (key, _) = load_key(&key_path(config), master_password)?;
    let _vault = store.load_vault(vault_name, &key)?;

    // Get the raw encrypted payload
    let payload = store.sync_payload(vault_name)?;

    // Create sync backend and push
    let mut backend = GitHubSyncBackend::from_stored_token()?;
    backend.ensure_repo()?;

    // Check for conflicts unless --force is used
    if !force {
        let local_checksum = calculate_checksum(&payload);
        let status = backend.get_status(&local_checksum)?;
        if status == SyncStatus::Conflict {
            return Err(anyhow!(
                "Conflict detected: both local and remote have changed.\n\
                 Use 'lilypad sync push {} --force' to overwrite remote changes.",
                vault_name
            ));
        }
    }

    backend.push(vault_name, &payload)?;

    if output_format == OutputFormat::Json {
        println!(
            "{}",
            serde_json::json!({
                "status": "synced",
                "vault": vault_name,
                "remote": format!("lilypad-vault-{}", backend.username())
            })
        );
    } else {
        println!();
        println!("Vault '{}' pushed to GitHub successfully!", vault_name);
        println!();
        println!("View at: https://github.com/{}/lilypad-vault-{}",
            backend.username(), backend.username());
    }

    Ok(())
}

/// Pulls a vault from GitHub.
pub fn sync_pull(
    store: &lilypad_storage::LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    force: bool,
    master_password: Option<&str>,
    output_format: OutputFormat,
) -> Result<()> {
    // Check authentication
    if !GitHubSyncBackend::has_valid_token()? {
        return Err(anyhow!(
            "Not authenticated. Use 'lilypad login' first."
        ));
    }

    let mut backend = GitHubSyncBackend::from_stored_token()?;

    // Check for conflicts unless --force is used
    if !force {
        let local_payload = store.sync_payload(vault_name);
        if let Ok(local_data) = local_payload {
            let local_checksum = calculate_checksum(&local_data);
            let status = backend.get_status(&local_checksum)?;
            if status == SyncStatus::Conflict {
                return Err(anyhow!(
                    "Conflict detected: both local and remote have changed.\n\
                     Use 'lilypad sync pull {} --force' to overwrite local changes.",
                    vault_name
                ));
            }
        }
    }

    // Pull data from GitHub
    let payload = backend.pull(vault_name)?;

    match payload {
        Some(data) => {
            // Verify we can decrypt it with the provided key
            let (key, _) = load_key(&key_path(config), master_password)?;

            // Apply the payload (this will overwrite local)
            store.apply_sync_payload(vault_name, &data)?;

            // Verify decryption works
            let _vault = store.load_vault(vault_name, &key)?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "pulled",
                        "vault": vault_name,
                        "size_bytes": data.len()
                    })
                );
            } else {
                println!("Vault '{}' pulled from GitHub successfully!", vault_name);
            }
        }
        None => {
            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_found",
                        "vault": vault_name
                    })
                );
            } else {
                println!("No vault data found on GitHub.");
                println!("Use 'lilypad sync push {}' to upload your vault.", vault_name);
            }
        }
    }

    Ok(())
}

/// Shows the sync status for a vault.
pub fn sync_status(
    store: &lilypad_storage::LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    master_password: Option<&str>,
    output_format: OutputFormat,
) -> Result<()> {
    // Check authentication
    if !GitHubSyncBackend::has_valid_token()? {
        if output_format == OutputFormat::Json {
            println!(
                "{}",
                serde_json::json!({
                    "status": "not_authenticated"
                })
            );
        } else {
            println!("Status: Not authenticated");
            println!("Use 'lilypad login' to authenticate with GitHub.");
        }
        return Ok(());
    }

    // Load vault and calculate checksum
    let (key, _) = load_key(&key_path(config), master_password)?;
    let _vault = store.load_vault(vault_name, &key)?;
    let payload = store.sync_payload(vault_name)?;
    let local_checksum = calculate_checksum(&payload);

    let mut backend = GitHubSyncBackend::from_stored_token()?;
    let status = backend.get_status(&local_checksum)?;

    let status_str = match &status {
        SyncStatus::NotAuthenticated => "not_authenticated",
        SyncStatus::NoRemoteVault => "no_remote_vault",
        SyncStatus::InSync => "in_sync",
        SyncStatus::LocalAhead => "local_ahead",
        SyncStatus::RemoteAhead => "remote_ahead",
        SyncStatus::Conflict => "conflict",
        SyncStatus::Unknown(_) => "unknown",
    };

    if output_format == OutputFormat::Json {
        println!(
            "{}",
            serde_json::json!({
                "vault": vault_name,
                "status": status_str,
                "local_checksum": local_checksum
            })
        );
    } else {
        println!("Sync Status for '{}':", vault_name);
        println!();
        match status {
            SyncStatus::NotAuthenticated => {
                println!("  Status: Not authenticated");
            }
            SyncStatus::NoRemoteVault => {
                println!("  Status: No remote vault");
                println!("  Action: Use 'lilypad sync push {}' to upload", vault_name);
            }
            SyncStatus::InSync => {
                println!("  Status: In sync");
                println!("  Local and remote vaults are identical.");
            }
            SyncStatus::LocalAhead => {
                println!("  Status: Local changes pending");
                println!("  Action: Use 'lilypad sync push {}' to upload", vault_name);
            }
            SyncStatus::RemoteAhead => {
                println!("  Status: Remote changes available");
                println!("  Action: Use 'lilypad sync pull {}' to download", vault_name);
            }
            SyncStatus::Conflict => {
                println!("  Status: Conflict detected!");
                println!("  Both local and remote have changes.");
                println!("  Action: Use --force with push or pull to resolve.");
            }
            SyncStatus::Unknown(msg) => {
                println!("  Status: Unknown ({})", msg);
            }
        }
    }

    Ok(())
}

/// Calculates SHA-256 checksum.
fn calculate_checksum(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let result = Sha256::digest(data);
    hex::encode(result)
}

// Re-export hex for checksum
mod hex {
    pub fn encode(data: impl AsRef<[u8]>) -> String {
        data.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
    }
}
