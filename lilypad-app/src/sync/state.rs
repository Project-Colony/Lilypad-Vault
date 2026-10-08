//! Persisted per-vault sync state.
//!
//! The old backend kept sync metadata in memory only and generated a fresh
//! device id on every process start, so status detection never had a baseline
//! and effectively always reported "Conflict". `lilypad-app` persists the state
//! under `<data_dir>/sync/<vault>.json`: a stable device id (generated once and
//! reused) plus the last remote commit SHA and the local checksum at the last
//! successful sync, which together let `status` tell local-ahead from
//! remote-ahead from a real conflict.

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What we remember about the last successful sync of a single vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    /// Stable identifier for this install, generated once and reused.
    pub device_id: String,
    /// Remote git blob/commit SHA observed at the last successful sync.
    #[serde(default)]
    pub last_sync_sha: Option<String>,
    /// Local vault checksum at the last successful sync.
    #[serde(default)]
    pub last_local_checksum: Option<String>,
}

impl SyncState {
    fn new(device_id: String) -> Self {
        Self {
            device_id,
            last_sync_sha: None,
            last_local_checksum: None,
        }
    }

    fn path(sync_dir: &Path, vault: &str) -> PathBuf {
        sync_dir.join(format!("{vault}.json"))
    }

    /// Loads the persisted state for a vault, creating (and persisting) a fresh
    /// one with a new stable device id on first use.
    ///
    /// A file that exists but fails to parse is NOT silently discarded (that
    /// would erase the sync baseline and could turn a real conflict into a blind
    /// overwrite): it is moved aside to `<vault>.json.corrupt` for possible
    /// manual recovery before a fresh state is initialized.
    pub fn load_or_init(sync_dir: &Path, vault: &str, new_device_id: &str) -> Result<Self> {
        let path = Self::path(sync_dir, vault);
        if path.exists() {
            let bytes = std::fs::read(&path)
                .map_err(|e| AppError::Io(format!("failed to read sync state: {e}")))?;
            match serde_json::from_slice::<SyncState>(&bytes) {
                Ok(state) => return Ok(state),
                Err(_) => {
                    let aside = path.with_extension("json.corrupt");
                    let _ = std::fs::rename(&path, &aside);
                }
            }
        }
        let state = SyncState::new(new_device_id.to_string());
        state.save(sync_dir, vault)?;
        Ok(state)
    }

    /// Persists the state to `<sync_dir>/<vault>.json` atomically (temp file +
    /// fsync + rename), so a crash mid-write can never leave a torn file that a
    /// later load would mistake for corruption.
    pub fn save(&self, sync_dir: &Path, vault: &str) -> Result<()> {
        std::fs::create_dir_all(sync_dir)
            .map_err(|e| AppError::Io(format!("failed to create sync dir: {e}")))?;
        let path = Self::path(sync_dir, vault);
        let json = serde_json::to_vec_pretty(self)?;

        let mut tmp = tempfile::NamedTempFile::new_in(sync_dir)
            .map_err(|e| AppError::Io(format!("failed to create temp sync state: {e}")))?;
        {
            use std::io::Write as _;
            tmp.write_all(&json)
                .map_err(|e| AppError::Io(format!("failed to write sync state: {e}")))?;
            tmp.as_file()
                .sync_all()
                .map_err(|e| AppError::Io(format!("failed to sync sync state: {e}")))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o600));
        }
        tmp.persist(&path)
            .map_err(|e| AppError::Io(format!("failed to persist sync state: {e}")))?;
        Ok(())
    }

    /// Records a completed sync at the given remote SHA and local checksum.
    pub fn record_sync(&mut self, remote_sha: Option<String>, local_checksum: String) {
        self.last_sync_sha = remote_sha;
        self.last_local_checksum = Some(local_checksum);
    }
}
