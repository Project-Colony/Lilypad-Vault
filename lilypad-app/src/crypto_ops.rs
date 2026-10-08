//! Whole-vault crypto operations: changing the master password.
//!
//! Changing the master password re-derives a fresh key and re-encrypts every
//! entry's secret under it, then rewrites the vault. It runs as a locked
//! read-modify-write against a fresh on-disk copy and always writes a
//! self-contained (V2) vault, which additionally migrates any legacy V1 vault
//! away from its standalone `key.json` in the process. A safety backup is taken
//! before the rewrite so an interrupted change is recoverable.

use crate::error::{AppError, Result};
use crate::locking::with_vault_locked;
use crate::password::normalize_master_password;
use crate::secret::{open_secret, seal_secret};
use crate::session::Session;
use crate::vault::App;
use lilypad_core::{derive_key, CryptoAlgorithm, KeyDerivationParams, KeyMetadata};

/// Changes the master password of the currently unlocked vault.
///
/// The caller must hold an unlocked [`Session`] (which proves they knew the old
/// password); its key is used to decrypt existing secrets. On success the
/// session is transparently re-keyed to the new password.
pub fn change_master_password(app: &App, session: &mut Session, new_password: &str) -> Result<()> {
    let normalized = normalize_master_password(new_password);
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "new master password is required".to_string(),
        ));
    }

    let name = session.name().to_string();
    let old_key = session.key().clone();

    // Ratchet the KDF cost up, never down: a password change must not silently
    // weaken the vault below whatever parameters it currently uses (e.g. this
    // device having less RAM than the one that created the vault).
    let mut new_params = KeyDerivationParams::generate_adaptive();
    if let Ok(Some(existing)) = app.store().load_vault_kdf_params(&name) {
        let existing = existing.to_kdf_params();
        new_params.memory_kib = new_params.memory_kib.max(existing.memory_kib);
        new_params.iterations = new_params.iterations.max(existing.iterations);
        new_params.parallelism = new_params.parallelism.max(existing.parallelism);
    }
    let new_key = derive_key(&normalized, &new_params)?;

    let lock_dir = app.lock_dir();
    let rekeyed = with_vault_locked(&lock_dir, &name, || {
        // Safety backup before we rewrite anything.
        if app.vault_exists(&name)? {
            app.create_backup(&name)?;
        }
        // Reload the authoritative copy under the lock, using the OLD key.
        let mut vault = app
            .store()
            .load_vault(&name, &old_key)
            .map_err(AppError::from)?;
        // Re-encrypt each entry's secret: decrypt with old key, seal with new.
        for entry in vault.entries.iter_mut() {
            let secret = open_secret(&old_key, entry)?;
            entry.ciphertext = seal_secret(&new_key, secret.get())?;
            // Password-history blobs are sealed under the vault key too:
            // leaving them under the old key would make them unreadable and
            // permanently block a cross-key sync merge. A blob that does not
            // decrypt with the old key (stale residue of an even earlier
            // rekey) is carried unchanged - it is already unreadable and
            // dropping it would destroy the only remaining copy.
            for record in entry.history.iter_mut() {
                if let Some(prev) = &record.previous_ciphertext {
                    if let Ok(plain) = lilypad_core::decrypt(&old_key, prev) {
                        let plain = zeroize::Zeroizing::new(plain);
                        record.previous_ciphertext =
                            Some(lilypad_core::encrypt(&new_key, &plain).map_err(AppError::from)?);
                    }
                }
            }
        }
        // Rewrite metadata as a self-contained V2 vault under the new key.
        vault.key_metadata = KeyMetadata::new(&new_key, CryptoAlgorithm::XChaCha20Poly1305)
            .with_embedded_kdf(&new_params);
        app.store()
            .save_vault(&vault, &new_key)
            .map_err(AppError::from)?;
        Ok(vault)
    })?;

    session.rekey(new_key, rekeyed);
    Ok(())
}
