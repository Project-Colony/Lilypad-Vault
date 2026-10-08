//! Vault-level management: rename, delete, and the backup subsystem.
//!
//! These are thin, typed wrappers over `lilypad-storage` that always pass the
//! vault identity explicitly, so callers never reconstruct backup filenames by
//! hand (the source of the audited wrong-vault restore bug). Destructive
//! operations funnel through the app so a safety backup can be taken first.

use crate::error::{AppError, Result};
use crate::vault::App;

/// Metadata about a single stored backup.
#[derive(Debug, Clone)]
pub struct BackupEntry {
    pub filename: String,
    pub vault_name: String,
    pub created_at: u64,
    pub size_bytes: u64,
}

impl App {
    /// Renames a vault on disk.
    pub fn rename_vault(&self, from: &str, to: &str) -> Result<()> {
        self.store().rename_vault(from, to).map_err(AppError::from)
    }

    /// Permanently deletes a vault, taking a safety backup first so an
    /// accidental delete is recoverable from the backup directory.
    pub fn delete_vault(&self, name: &str) -> Result<()> {
        if self.vault_exists(name)? {
            // Best-effort safety net; do not block deletion if backup fails.
            let _ = self.store().create_backup(name);
        }
        self.store().delete_vault(name).map_err(AppError::from)
    }

    /// Creates a timestamped backup of a vault and returns its filename.
    pub fn create_backup(&self, name: &str) -> Result<String> {
        self.store().create_backup(name).map_err(AppError::from)
    }

    /// Lists a vault's backups, most recent first.
    pub fn list_backups(&self, name: &str) -> Result<Vec<BackupEntry>> {
        let backups = self.store().list_backups(name).map_err(AppError::from)?;
        Ok(backups
            .into_iter()
            .map(|b| BackupEntry {
                filename: b.filename,
                vault_name: b.vault_name,
                created_at: b.created_at,
                size_bytes: b.size_bytes,
            })
            .collect())
    }

    /// Restores a vault from a named backup. Returns the safety-backup filename
    /// created for the pre-restore state, if the vault already existed.
    ///
    /// Runs under the same per-vault lock as every entry mutation, so a
    /// concurrent locked read-modify-write cannot interleave with the restore
    /// (one of them would silently win otherwise).
    pub fn restore_backup(&self, backup_filename: &str) -> Result<Option<String>> {
        let vault = lilypad_storage::backup_vault_name(backup_filename).ok_or_else(|| {
            AppError::Other(format!("invalid backup filename: {backup_filename}"))
        })?;
        crate::locking::with_vault_locked(&self.lock_dir(), &vault, || {
            self.store()
                .restore_backup(backup_filename)
                .map_err(AppError::from)
        })
    }

    /// Deletes all but the most recent `keep` backups of a vault. Returns the
    /// number of backups removed.
    pub fn prune_backups(&self, name: &str, keep: usize) -> Result<usize> {
        self.store()
            .prune_backups(name, keep)
            .map_err(AppError::from)
    }
}
