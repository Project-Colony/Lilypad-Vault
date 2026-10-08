//! Canonical data-directory resolution.
//!
//! The old frontends disagreed on where vaults live: `lilypad-core`'s default
//! config used a CWD-relative `.lilypad`, so the CLI would read/write vaults in
//! whatever directory it happened to be launched from, while the desktop used an
//! XDG project dir. That means a vault "saved" from the shell could be invisible
//! to the GUI. `lilypad-app` resolves ONE directory for everyone, in this order:
//!
//! 1. an explicit override passed by the caller (`--data-dir`),
//! 2. the `LILYPAD_DATA_DIR` environment variable,
//! 3. the platform `Colony/Lilypad` directory: the one the desktop app and the
//!    OAuth token store have always used (`~/.config/Colony/Lilypad` on Linux,
//!    `~/Library/Application Support/Colony/Lilypad` on macOS,
//!    `%APPDATA%\Colony\Lilypad\config` on Windows), so vaults made by
//!    earlier desktop builds are still found after an upgrade.

use crate::error::{AppError, Result};
use directories::ProjectDirs;
use std::path::PathBuf;

/// The default directory. `ProjectDirs::from("com", "Colony", "Lilypad")` would
/// differ per platform (`~/.local/share/lilypad` on Linux, `com.Colony.Lilypad`
/// on macOS) and orphan existing vaults, so this keeps the explicit path.
fn platform_dir() -> Option<PathBuf> {
    ProjectDirs::from_path(PathBuf::from("Colony/Lilypad")).map(|p| p.config_dir().to_path_buf())
}

/// Resolves the canonical data directory, creating it if necessary.
pub fn resolve_data_dir(override_dir: Option<PathBuf>) -> Result<PathBuf> {
    let dir = if let Some(dir) = override_dir {
        dir
    } else if let Some(env_dir) = std::env::var_os("LILYPAD_DATA_DIR") {
        PathBuf::from(env_dir)
    } else {
        platform_dir().ok_or_else(|| {
            AppError::Io("could not determine a platform data directory".to_string())
        })?
    };
    // Reject an empty path (an unset-looking `LILYPAD_DATA_DIR=""` would resolve
    // to the current working directory, reintroducing the CWD-relative-vault bug
    // this module exists to kill) and a non-UTF8 path (AppConfig.data_dir is a
    // String the store round-trips, so a lossy conversion could silently point
    // at a different directory).
    if dir.as_os_str().is_empty() {
        return Err(AppError::Validation(
            "data directory path is empty".to_string(),
        ));
    }
    if dir.to_str().is_none() {
        return Err(AppError::Validation(
            "data directory path is not valid UTF-8".to_string(),
        ));
    }
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Io(format!("failed to create data dir {}: {e}", dir.display())))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dir_is_the_legacy_desktop_one_next_to_the_oauth_tokens() {
        let dir = platform_dir().expect("platform dir");
        let suffix = if cfg!(windows) {
            "Colony/Lilypad/config"
        } else {
            "Colony/Lilypad"
        };
        assert!(dir.ends_with(suffix), "{}", dir.display());
        let tokens = lilypad_oauth::TokenStoreManager::default_path().unwrap();
        assert_eq!(tokens.parent(), Some(dir.as_path()));
    }
}
