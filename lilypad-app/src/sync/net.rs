//! Network/OAuth transport for sync.
//!
//! Thin wrapper over `lilypad-oauth`: GitHub device-flow auth and per-vault
//! push/pull. All *safety* (validate-before-write, safety backup) lives in the
//! parent module's [`apply_remote`](super::apply_remote); this layer only moves
//! bytes and updates the persisted [`SyncState`].
//!
//! NOTE: these functions perform live network I/O and GitHub OAuth, so they are
//! exercised end-to-end with real credentials (via the CLI/GUI), not by unit
//! tests. The parent module's validation logic - the part that protects vault
//! data - is what the unit tests cover.

use crate::error::{AppError, Result};
use crate::vault::App;
use lilypad_oauth::{
    DeviceFlowAuth, GitHubClient, GitHubSyncBackend, OAuthConfig, OAuthError, OAuthProvider,
    TokenStoreManager, BUILTIN_GITHUB_CLIENT_ID, DEFAULT_GITHUB_SCOPES,
};

use super::state::SyncState;
use super::SyncStatusView;

/// Authentication state with the sync provider.
#[derive(Debug, Clone)]
pub enum AuthState {
    Authenticated { username: String },
    NotAuthenticated,
}

/// A pending device-flow login for the frontend to display to the user.
#[derive(Debug, Clone)]
pub struct DeviceLogin {
    pub verification_uri: String,
    pub user_code: String,
    pub device_code: String,
    pub interval: u64,
}

/// Outcome of a pull.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullOutcome {
    Applied,
    NoRemote,
}

/// Outcome of a bidirectional sync ([`sync_merge`]).
#[derive(Debug, Clone)]
pub enum MergeOutcome {
    /// No remote vault exists yet (nothing to merge; push instead).
    NoRemote,
    /// The remote was merged into the local vault. `pushed` is true when the
    /// merged vault was pushed back because the remote was missing local data.
    Merged {
        report: Box<lilypad_core::MergeResult>,
        pushed: bool,
    },
}

fn map_oauth(e: OAuthError) -> AppError {
    match e {
        OAuthError::InvalidToken | OAuthError::TokenExpired => AppError::NotAuthenticated,
        OAuthError::NetworkError(m) => AppError::Network(m),
        other => AppError::Sync(other.to_string()),
    }
}

fn github_config() -> Result<OAuthConfig> {
    if BUILTIN_GITHUB_CLIENT_ID.is_empty() {
        return Err(AppError::Sync(
            "no GitHub OAuth client id was compiled in (build with LILYPAD_GITHUB_CLIENT_ID)"
                .to_string(),
        ));
    }
    Ok(OAuthConfig::github(BUILTIN_GITHUB_CLIENT_ID).with_scopes(
        DEFAULT_GITHUB_SCOPES
            .iter()
            .map(|s| s.to_string())
            .collect(),
    ))
}

/// Current authentication state (reads the local token store; the
/// not-authenticated case needs no network).
pub fn auth_status() -> Result<AuthState> {
    let store = TokenStoreManager::new().map_err(map_oauth)?;
    if !store
        .has_valid_token(OAuthProvider::GitHub)
        .map_err(map_oauth)?
    {
        return Ok(AuthState::NotAuthenticated);
    }
    let username = store
        .load_token(OAuthProvider::GitHub)
        .map_err(map_oauth)?
        .and_then(|t| t.username.clone())
        .unwrap_or_default();
    Ok(AuthState::Authenticated { username })
}

/// Removes stored credentials.
pub fn logout() -> Result<()> {
    GitHubSyncBackend::logout().map_err(map_oauth)
}

/// Begins a GitHub device-flow login; the returned code/URL are shown to the
/// user, then [`complete_login`] polls until they authorize.
pub fn begin_login() -> Result<DeviceLogin> {
    let flow = DeviceFlowAuth::new(github_config()?).map_err(map_oauth)?;
    let resp = flow.initiate().map_err(map_oauth)?;
    Ok(DeviceLogin {
        verification_uri: resp.verification_uri,
        user_code: resp.user_code,
        device_code: resp.device_code,
        interval: resp.interval,
    })
}

/// Completes a device-flow login by polling for the token, then persists it.
/// Returns the authenticated GitHub username.
pub fn complete_login(login: &DeviceLogin) -> Result<String> {
    let flow = DeviceFlowAuth::new(github_config()?).map_err(map_oauth)?;
    let result = flow
        .poll_for_token(&login.device_code, login.interval)
        .map_err(map_oauth)?;
    let client = GitHubClient::new(&result.access_token).map_err(map_oauth)?;
    let user = client.get_user().map_err(map_oauth)?;
    let token = result.to_token_info().with_username(user.login.clone());
    TokenStoreManager::new()
        .map_err(map_oauth)?
        .save_token(OAuthProvider::GitHub, token)
        .map_err(map_oauth)?;
    Ok(user.login)
}

/// Pushes the local vault to its per-vault remote path and records the baseline.
///
/// Refuses to overwrite a remote that has advanced past the last-synced baseline
/// (the caller must `pull` first) unless `force` is set. The optimistic-lock SHA
/// sent to GitHub is the persisted baseline, not the current remote SHA, so even
/// a race between the status read and the write is rejected server-side.
pub fn push(app: &App, vault: &str, force: bool) -> Result<()> {
    if !app.vault_exists(vault)? {
        return Err(AppError::VaultNotFound(vault.to_string()));
    }
    let mut backend = GitHubSyncBackend::from_stored_token().map_err(map_oauth)?;
    backend.ensure_repo().map_err(map_oauth)?;
    let payload = app.store().sync_payload(vault).map_err(AppError::from)?;

    let sync_dir = app.sync_dir();
    let mut state = SyncState::load_or_init(&sync_dir, vault, backend.device_id())?;

    let remote_sha = backend.remote_sha_named(vault).map_err(map_oauth)?;
    let local_checksum = super::checksum(&payload);

    if !force {
        match super::status(&state, Some(&local_checksum), remote_sha.as_deref()) {
            SyncStatusView::RemoteAhead | SyncStatusView::Conflict => {
                return Err(AppError::Sync(
                    "the remote vault has changed since the last sync; pull before pushing (or force)"
                        .to_string(),
                ));
            }
            SyncStatusView::NoBaseline if remote_sha.is_some() => {
                return Err(AppError::Sync(
                    "a remote vault already exists but this device has no sync baseline; pull first (or force)"
                        .to_string(),
                ));
            }
            _ => {}
        }
    }

    // Optimistic-lock token: the baseline SHA (server rejects with 409 if the
    // remote moved). When forcing, use the current remote SHA to intentionally
    // overwrite whatever is there.
    let prev = if force {
        remote_sha.clone()
    } else {
        state.last_sync_sha.clone()
    };
    let new_sha = backend
        .push_named(vault, &payload, prev.as_deref())
        .map_err(map_oauth)?;

    state.record_sync(Some(new_sha), local_checksum);
    state.save(&sync_dir, vault)?;
    Ok(())
}

/// Pulls the remote vault, validates it, and (if safe) replaces the local vault,
/// then records the baseline. Validation and the safety backup happen in
/// [`super::apply_remote`].
pub fn pull(app: &App, vault: &str, master_password: &str) -> Result<PullOutcome> {
    let mut backend = GitHubSyncBackend::from_stored_token().map_err(map_oauth)?;
    let sync_dir = app.sync_dir();
    match backend.pull_named(vault).map_err(map_oauth)? {
        Some((bytes, sha)) => {
            let applied_checksum = super::apply_remote(app, vault, master_password, &bytes)?;
            let mut state = SyncState::load_or_init(&sync_dir, vault, backend.device_id())?;
            state.record_sync(Some(sha), applied_checksum);
            state.save(&sync_dir, vault)?;
            Ok(PullOutcome::Applied)
        }
        None => Ok(PullOutcome::NoRemote),
    }
}

/// Bidirectional sync: pulls the remote vault, merges it into the local vault
/// entry-by-entry (see [`super::merge_remote`]), and - when the remote is
/// missing data the merged vault holds - pushes the merged result back. Ends
/// with the baseline recorded, so a successful call lands on `InSync` from ANY
/// starting state, including `Conflict` and `NoBaseline`.
pub fn sync_merge(app: &App, vault: &str, master_password: &str) -> Result<MergeOutcome> {
    if !app.vault_exists(vault)? {
        return Err(AppError::VaultNotFound(vault.to_string()));
    }
    let mut backend = GitHubSyncBackend::from_stored_token().map_err(map_oauth)?;
    let sync_dir = app.sync_dir();
    let Some((bytes, sha)) = backend.pull_named(vault).map_err(map_oauth)? else {
        return Ok(MergeOutcome::NoRemote);
    };

    // The payload comes back from merge_remote read under the vault lock:
    // checksumming or pushing a re-read here would let a concurrent local
    // write get recorded in the baseline as if it had been synced.
    let (report, payload) = super::merge_remote(app, vault, master_password, &bytes)?;

    let local_checksum = super::checksum(&payload);
    let mut state = SyncState::load_or_init(&sync_dir, vault, backend.device_id())?;

    if report.remote_is_stale() {
        // The remote lacks local data (local-only entries, locally-newer
        // versions, or deletions it has not applied): push the merged vault
        // back. The optimistic-lock SHA is the exact payload we merged from,
        // so a racing writer on another device becomes a server-side 409,
        // never a silent overwrite.
        let new_sha = backend
            .push_named(vault, &payload, Some(&sha))
            .map_err(map_oauth)?;
        state.record_sync(Some(new_sha), local_checksum);
        state.save(&sync_dir, vault)?;
        return Ok(MergeOutcome::Merged {
            report: Box::new(report),
            pushed: true,
        });
    }

    // The merged local vault holds nothing the remote lacks: adopt the fetched
    // remote as the baseline (content-wise the two are now equivalent).
    state.record_sync(Some(sha), local_checksum);
    state.save(&sync_dir, vault)?;
    Ok(MergeOutcome::Merged {
        report: Box::new(report),
        pushed: false,
    })
}

/// Computes the sync status against the remote, using the persisted baseline.
pub fn remote_status(app: &App, vault: &str) -> Result<SyncStatusView> {
    let mut backend = GitHubSyncBackend::from_stored_token().map_err(map_oauth)?;
    let remote_sha = backend.remote_sha_named(vault).map_err(map_oauth)?;
    let local = super::local_checksum(app, vault)?;
    let state = SyncState::load_or_init(&app.sync_dir(), vault, backend.device_id())?;
    Ok(super::status(
        &state,
        local.as_deref(),
        remote_sha.as_deref(),
    ))
}
