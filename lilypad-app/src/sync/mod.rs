//! Validated sync orchestration.
//!
//! This module owns the *safety* of sync; the network transport is a thin
//! wrapper over `lilypad-oauth` (see `net`). The two audited sync data-loss
//! bugs are fixed here:
//!
//! * **Validate before overwrite.** [`apply_remote`] resolves the key exactly
//!   the way `unlock` will *after* the write (derived from the candidate
//!   payload's own embedded KDF params + the master password), proves the
//!   candidate decrypts with that key, refuses a format downgrade, takes a
//!   safety backup, and only then commits - so a pulled vault can never leave
//!   the local file unreadable, and a wrong-key/corrupt remote never destroys
//!   local data.
//! * **Persisted baseline.** [`status`] compares the current local checksum and
//!   remote SHA against the persisted [`SyncState`] baseline, so local-ahead,
//!   remote-ahead and genuine conflicts are actually distinguishable (the old
//!   in-memory state always degraded to "conflict").
//!
//! On top of the validated *replace* ([`apply_remote`], used by pull),
//! [`merge_remote`] offers an entry-level merge: per-entry last-write-wins with
//! deletion tombstones (see `Vault::merge_from` in `lilypad-core`), plus
//! cross-key re-encryption so entries sealed under the remote replica's key
//! (different KDF salt, same master password) become readable locally.

pub mod net;
pub mod state;

pub use state::SyncState;

use crate::error::{AppError, Result};
use crate::locking::with_vault_locked;
use crate::password::normalize_master_password;
use crate::vault::App;
use lilypad_common::keyfile::load_key;
use lilypad_core::{decrypt, derive_key, encrypt, KeyMaterial, MergeResult, Vault};
use lilypad_storage::LocalStore;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Where the local vault stands relative to the remote, given the persisted
/// baseline. Distinguishing these is the whole point of persisting sync state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatusView {
    /// No remote vault exists yet.
    NoRemote,
    /// No local baseline recorded yet (first sync): the user must choose.
    NoBaseline,
    /// Local and remote both match the baseline.
    InSync,
    /// Only local changed since the baseline (safe to push).
    LocalAhead,
    /// Only remote changed since the baseline (safe to pull).
    RemoteAhead,
    /// Both changed since the baseline (needs an explicit choice).
    Conflict,
}

/// SHA-256 hex of a byte payload (matches the backend's checksum convention).
fn checksum(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// The checksum of the current on-disk local vault payload, or `None` if the
/// vault does not exist locally.
pub fn local_checksum(app: &App, vault: &str) -> Result<Option<String>> {
    if !app.vault_exists(vault)? {
        return Ok(None);
    }
    let payload = app.store().sync_payload(vault).map_err(AppError::from)?;
    Ok(Some(checksum(&payload)))
}

/// Computes the sync status from the persisted baseline plus the current local
/// checksum and the current remote SHA. Pure function: no I/O, fully testable.
pub fn status(
    state: &SyncState,
    local_checksum: Option<&str>,
    remote_sha: Option<&str>,
) -> SyncStatusView {
    let Some(remote_sha) = remote_sha else {
        return SyncStatusView::NoRemote;
    };
    // The vault is gone locally but a remote exists: the recovery action is to
    // pull it back, so report RemoteAhead rather than LocalAhead.
    if local_checksum.is_none() {
        return SyncStatusView::RemoteAhead;
    }
    let (Some(base_local), Some(base_sha)) = (
        state.last_local_checksum.as_deref(),
        state.last_sync_sha.as_deref(),
    ) else {
        return SyncStatusView::NoBaseline;
    };
    let local_changed = local_checksum != Some(base_local);
    let remote_changed = remote_sha != base_sha;
    match (local_changed, remote_changed) {
        (false, false) => SyncStatusView::InSync,
        (true, false) => SyncStatusView::LocalAhead,
        (false, true) => SyncStatusView::RemoteAhead,
        (true, true) => SyncStatusView::Conflict,
    }
}

/// Validates a candidate remote payload and, if safe, replaces the local vault
/// with it (taking a safety backup first). Returns the checksum of the applied
/// payload so the caller can update the sync baseline.
///
/// The key is resolved the SAME way [`crate::App::unlock`] will resolve it after
/// the write - from the candidate's own embedded KDF params - so a "validated"
/// write is guaranteed to be openable by the user's master password.
pub fn apply_remote(
    app: &App,
    vault: &str,
    master_password: &str,
    remote_bytes: &[u8],
) -> Result<String> {
    let normalized = normalize_master_password(master_password);

    let candidate_params =
        LocalStore::kdf_params_from_bytes(remote_bytes).map_err(AppError::from)?;

    // Refuse a legacy (keyfile-dependent) payload landing on a self-contained
    // vault: after the write, unlock would look for embedded params, find none,
    // fall back to a key.json the user may not have, and lock them out. Read
    // errors here fail CLOSED (propagate) rather than being swallowed into a
    // "no local params" assumption that would let the downgrade through.
    if candidate_params.is_none() && app.vault_exists(vault)? {
        let local_params = app
            .store()
            .load_vault_kdf_params(vault)
            .map_err(AppError::from)?;
        if local_params.is_some() {
            return Err(AppError::Validation(
                "refusing to apply a legacy (keyfile) payload over a self-contained vault"
                    .to_string(),
            ));
        }
    }

    // Resolve the key exactly as unlock will resolve it for the *candidate*.
    let key = match &candidate_params {
        Some(params) => derive_key(&normalized, &params.to_kdf_params())?,
        None => {
            let key_path = app.data_dir().join("key.json");
            let (key, _) = load_key(&key_path, Some(&normalized)).map_err(AppError::from)?;
            key
        }
    };

    // Prove the candidate decrypts with the key the next unlock will use.
    app.store()
        .load_vault_from_bytes(remote_bytes, &key)
        .map_err(AppError::from)?;

    // Commit under lock, safety-backup first.
    with_vault_locked(&app.lock_dir(), vault, || {
        if app.vault_exists(vault)? {
            app.store().create_backup(vault).map_err(AppError::from)?;
        }
        app.store()
            .apply_sync_payload(vault, remote_bytes)
            .map_err(AppError::from)
    })?;

    Ok(checksum(remote_bytes))
}

/// Re-seals every remote entry ciphertext (and encrypted history) under the
/// local key, so the merged vault stays fully readable with the local KDF
/// params. A no-op when both replicas already derive the same key (identical
/// salt). Fails closed: one undecryptable remote blob aborts the whole merge
/// rather than silently importing an unreadable entry.
fn reseal_under_local_key(
    mut remote: Vault,
    remote_key: &KeyMaterial,
    local_key: &KeyMaterial,
) -> Result<Vault> {
    if remote_key.as_bytes() == local_key.as_bytes() {
        return Ok(remote);
    }
    for entry in &mut remote.entries {
        let plaintext = Zeroizing::new(decrypt(remote_key, &entry.ciphertext).map_err(|_| {
            AppError::Validation(format!(
                "remote entry '{}' does not decrypt with the remote vault's own key; refusing to merge",
                entry.label
            ))
        })?);
        entry.ciphertext = encrypt(local_key, &plaintext).map_err(AppError::from)?;
        for record in &mut entry.history {
            if let Some(prev) = &record.previous_ciphertext {
                // History blobs left behind by a pre-fix change_master_password
                // may be sealed under a key that no longer exists. They are
                // carried unchanged rather than failing the merge: the blob is
                // equally unreadable on the remote, and aborting here would
                // permanently brick syncing for that vault.
                if let Ok(prev_plain) = decrypt(remote_key, prev) {
                    let prev_plain = Zeroizing::new(prev_plain);
                    record.previous_ciphertext =
                        Some(encrypt(local_key, &prev_plain).map_err(AppError::from)?);
                }
            }
        }
    }
    Ok(remote)
}

/// For entries present on both sides with IDENTICAL `updated_at` but different
/// content: plain last-write-wins sees a timestamp tie, assumes the entries are
/// the same, and lets the replicas silently diverge forever (1-second clocks
/// make same-second concurrent edits entirely realistic). This detects the
/// divergence by comparing content digests (decrypted secret + the cleartext
/// fields that matter) and resolves it deterministically - the larger digest
/// wins, so EVERY replica picks the same winner and the fleet converges. The
/// losing version survives in the pre-merge safety backup.
///
/// `rekeyed_remote` must already be sealed under `local_key`. Returns the
/// conflicting labels as (local_won, remote_won); remote-won entries are
/// written into `local` here (the timestamps stay tied, so `merge_from` will
/// classify them as unchanged).
fn resolve_tie_conflicts(
    local: &mut Vault,
    rekeyed_remote: &Vault,
    local_key: &KeyMaterial,
) -> Result<(Vec<String>, Vec<String>)> {
    let digest = |e: &lilypad_core::Entry| -> Result<[u8; 32]> {
        let plain = Zeroizing::new(decrypt(local_key, &e.ciphertext).map_err(|_| {
            AppError::Validation(format!(
                "entry '{}' does not decrypt during the conflict check",
                e.label
            ))
        })?);
        let mut hasher = Sha256::new();
        hasher.update(&*plain);
        hasher.update(serde_json::to_vec(&(
            &e.metadata,
            e.deleted_at,
            e.is_favorite,
            &e.color,
            e.password_expires_at,
            &e.icon,
        ))?);
        Ok(hasher.finalize().into())
    };

    let mut local_won = Vec::new();
    let mut remote_won = Vec::new();
    for remote_entry in &rekeyed_remote.entries {
        let Some(local_entry) = local
            .entries
            .iter_mut()
            .find(|e| e.label == remote_entry.label)
        else {
            continue;
        };
        if local_entry.updated_at != remote_entry.updated_at {
            continue; // a real newer side exists: ordinary LWW handles it
        }
        let (dl, dr) = (digest(local_entry)?, digest(remote_entry)?);
        if dl == dr {
            continue; // genuinely identical
        }
        if dl >= dr {
            local_won.push(local_entry.label.clone());
        } else {
            *local_entry = remote_entry.clone();
            remote_won.push(local_entry.label.clone());
        }
    }
    Ok((local_won, remote_won))
}

/// Merges a remote vault payload into the local vault entry-by-entry
/// (last-write-wins with deletion tombstones - see `Vault::merge_from`),
/// re-encrypting remote entries under the local key when the replicas use
/// different KDF salts. Never replaces local-only or locally-newer data.
///
/// Both keys are resolved the way `unlock` resolves them (the remote's from its
/// own embedded KDF params, the local's including the NFC-legacy fallback), and
/// the remote payload is proven to decrypt BEFORE the vault lock is taken. If
/// the merge changes the local vault, a safety backup is taken first and the
/// vault is written atomically.
///
/// Returns the merge report plus the resulting on-disk payload, read while the
/// vault lock is still held - callers must checksum/push THOSE bytes, never a
/// re-read: a concurrent local write landing after the lock is released would
/// otherwise be folded into the sync baseline as if it had been synced.
pub fn merge_remote(
    app: &App,
    vault: &str,
    master_password: &str,
    remote_bytes: &[u8],
) -> Result<(MergeResult, Vec<u8>)> {
    let normalized = normalize_master_password(master_password);

    // The remote payload's key comes from its OWN embedded KDF params: two
    // replicas of the same vault share the master password but not the salt.
    let candidate_params =
        LocalStore::kdf_params_from_bytes(remote_bytes).map_err(AppError::from)?;
    let remote_key = match &candidate_params {
        Some(params) => derive_key(&normalized, &params.to_kdf_params())?,
        None => {
            let key_path = app.data_dir().join("key.json");
            let (key, _) = load_key(&key_path, Some(&normalized)).map_err(AppError::from)?;
            key
        }
    };
    let remote_vault = app
        .store()
        .load_vault_from_bytes(remote_bytes, &remote_key)
        .map_err(AppError::from)?;

    // Resolved exactly as the next unlock will resolve it, so the merged vault
    // is guaranteed to reopen with the user's master password.
    let local_key = app.derive_unlock_key(vault, master_password)?;

    with_vault_locked(&app.lock_dir(), vault, || {
        let mut local = app
            .store()
            .load_vault(vault, &local_key)
            .map_err(AppError::from)?;
        let rekeyed = reseal_under_local_key(remote_vault, &remote_key, &local_key)?;
        let (ties_local, ties_remote) = resolve_tie_conflicts(&mut local, &rekeyed, &local_key)?;
        let mut report = local.merge_from(&rekeyed);
        report.tie_conflicts_local_won = ties_local;
        report.tie_conflicts_remote_won = ties_remote;
        if report.has_changes() {
            app.store().create_backup(vault).map_err(AppError::from)?;
            app.store()
                .save_vault(&local, &local_key)
                .map_err(AppError::from)?;
        }
        let payload = app.store().sync_payload(vault).map_err(AppError::from)?;
        Ok((report, payload))
    })
}
