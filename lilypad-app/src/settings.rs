//! Persisted user settings, shared by all frontends.
//!
//! Stored at `<data_dir>/settings.json`. Holds app-level policy (auto-lock and
//! clipboard timeouts, the last active vault) plus a couple of opaque UI indices
//! (theme, density) that a frontend interprets - lilypad-app stays UI-agnostic
//! and just round-trips the numbers. Nothing here is a secret, so it is stored
//! in cleartext with the same 0600 discipline as everything else.

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_auto_lock() -> u64 {
    5
}
fn default_clipboard() -> u64 {
    20
}
fn default_sidebar_ratio() -> f32 {
    0.2
}
fn default_list_ratio() -> f32 {
    0.34
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Idle minutes before a session auto-locks.
    #[serde(default = "default_auto_lock")]
    pub auto_lock_minutes: u64,
    /// Seconds a copied secret stays on the clipboard before it is cleared.
    #[serde(default = "default_clipboard")]
    pub clipboard_clear_secs: u64,
    /// The vault to preselect on the unlock screen.
    #[serde(default)]
    pub active_vault: Option<String>,
    /// Opaque theme index (interpreted by the frontend).
    #[serde(default)]
    pub theme: u32,
    /// Opaque density/variation index (interpreted by the frontend).
    #[serde(default)]
    pub density: u32,
    /// Sidebar width as a fraction of the window (0.0..1.0).
    #[serde(default = "default_sidebar_ratio")]
    pub sidebar_ratio: f32,
    /// Item-list width as a fraction of the window (0.0..1.0).
    #[serde(default = "default_list_ratio")]
    pub list_ratio: f32,
    /// Reopen the last-used vault on the unlock screen.
    #[serde(default = "default_true")]
    pub restore_last_vault: bool,
    /// Default sidebar filter applied on unlock (0 = All, 1 = Favorites).
    #[serde(default)]
    pub default_filter: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: default_auto_lock(),
            clipboard_clear_secs: default_clipboard(),
            active_vault: None,
            theme: 0,
            density: 0,
            sidebar_ratio: default_sidebar_ratio(),
            list_ratio: default_list_ratio(),
            restore_last_vault: true,
            default_filter: 0,
        }
    }
}

impl Settings {
    fn path(data_dir: &Path) -> std::path::PathBuf {
        data_dir.join("settings.json")
    }

    /// Loads settings, falling back to defaults on a missing or unreadable file
    /// (settings are convenience, never a reason to block the app).
    pub fn load(data_dir: &Path) -> Self {
        let path = Self::path(data_dir);
        std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
            .unwrap_or_default()
    }

    /// Persists settings atomically (temp file + rename).
    pub fn save(&self, data_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| AppError::Io(format!("failed to create data dir: {e}")))?;
        let json = serde_json::to_vec_pretty(self)?;
        let mut tmp = tempfile::NamedTempFile::new_in(data_dir)
            .map_err(|e| AppError::Io(format!("failed to create temp settings: {e}")))?;
        {
            use std::io::Write as _;
            tmp.write_all(&json)
                .map_err(|e| AppError::Io(format!("failed to write settings: {e}")))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o600));
        }
        tmp.persist(Self::path(data_dir))
            .map_err(|e| AppError::Io(format!("failed to persist settings: {e}")))?;
        Ok(())
    }

    /// Auto-lock duration, or `None` if disabled (0).
    pub fn auto_lock_duration(&self) -> Option<std::time::Duration> {
        if self.auto_lock_minutes == 0 {
            None
        } else {
            Some(std::time::Duration::from_secs(self.auto_lock_minutes * 60))
        }
    }
}
