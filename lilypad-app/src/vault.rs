//! The application handle and the vault lifecycle: open store, list, create,
//! unlock, and safely apply a remote payload.
//!
//! Two audited data-loss bugs are fixed here by construction:
//!
//! * **Unlock never writes.** [`App::unlock`] only ever *reads*. Creating a
//!   vault is a separate, explicit call ([`App::create_vault`]) that refuses if
//!   the vault already exists. The TUI bug - "load failed, so save a fresh empty
//!   vault over it" - cannot be expressed against this API.
//! * **Remote is validated before it can overwrite local.**
//!   [`App::apply_remote_validated`] decrypts the candidate bytes in memory and
//!   takes a safety backup before the live file is touched.

use crate::error::{AppError, Result};
use crate::locking::with_vault_locked;
use crate::password::normalize_master_password;
use crate::session::Session;
use lilypad_common::keyfile::load_key;
use lilypad_core::{
    default_config, derive_key, AppConfig, CryptoAlgorithm, KeyDerivationParams, KeyMaterial,
    KeyMetadata, Vault,
};
use lilypad_storage::LocalStore;
use std::path::PathBuf;
use std::time::Duration;

/// Options for opening the application.
#[derive(Debug, Default, Clone)]
pub struct OpenOptions {
    /// Explicit data-directory override (e.g. from a `--data-dir` flag).
    pub data_dir: Option<PathBuf>,
    /// Idle auto-lock policy applied to sessions this app hands out.
    pub auto_lock_after: Option<Duration>,
}

/// The top-level application handle shared by every frontend.
pub struct App {
    store: LocalStore,
    data_dir: PathBuf,
    auto_lock_after: Option<Duration>,
}

impl App {
    /// Opens the local store rooted at the canonical data directory.
    pub fn open(opts: OpenOptions) -> Result<Self> {
        let data_dir = crate::paths::resolve_data_dir(opts.data_dir)?;
        let config = AppConfig {
            data_dir: data_dir.to_string_lossy().into_owned(),
            ..default_config()
        };
        let store = LocalStore::new(&config).map_err(AppError::from)?;
        Ok(Self {
            store,
            data_dir,
            auto_lock_after: opts.auto_lock_after,
        })
    }

    /// The underlying store, for modules in this crate that orchestrate it.
    pub(crate) fn store(&self) -> &LocalStore {
        &self.store
    }

    pub(crate) fn lock_dir(&self) -> PathBuf {
        self.data_dir.join("locks")
    }

    pub(crate) fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    pub(crate) fn sync_dir(&self) -> PathBuf {
        self.data_dir.join("sync")
    }

    /// Loads persisted user settings (defaults if none/unreadable).
    pub fn load_settings(&self) -> crate::settings::Settings {
        crate::settings::Settings::load(&self.data_dir)
    }

    /// Persists user settings.
    pub fn save_settings(&self, settings: &crate::settings::Settings) -> Result<()> {
        settings.save(&self.data_dir)
    }

    /// Lists the names of every vault in the store.
    pub fn list_vaults(&self) -> Result<Vec<String>> {
        self.store.list_vaults().map_err(AppError::from)
    }

    /// Whether a vault with this name exists on disk.
    pub fn vault_exists(&self, name: &str) -> Result<bool> {
        self.store.vault_exists(name).map_err(AppError::from)
    }

    /// The embedded Argon2 memory cost (KiB) of a self-contained vault, or 0 if
    /// unknown (V1 vault or missing). Useful for a "vault security details" view.
    pub fn vault_kdf_memory(&self, name: &str) -> u32 {
        self.store
            .load_vault_kdf_params(name)
            .ok()
            .flatten()
            .map(|p| p.memory_kib)
            .unwrap_or(0)
    }

    /// Creates a new, empty, self-contained (V2) vault and returns an unlocked
    /// session for it. Errors if a vault of that name already exists.
    pub fn create_vault(&self, name: &str, password: &str) -> Result<Session> {
        // Fast, non-authoritative pre-check for a friendly early error; the
        // binding check happens under the lock below.
        if self.vault_exists(name)? {
            return Err(AppError::VaultAlreadyExists(name.to_string()));
        }
        let normalized = normalize_master_password(password);
        if normalized.is_empty() {
            return Err(AppError::Validation(
                "master password is required".to_string(),
            ));
        }
        let params = KeyDerivationParams::generate_adaptive();
        let key = derive_key(&normalized, &params)?;
        let metadata =
            KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305).with_embedded_kdf(&params);
        let vault = Vault::new(name, metadata);
        with_vault_locked(&self.lock_dir(), name, || {
            // Re-check existence INSIDE the lock so check-and-write is atomic:
            // otherwise two concurrent creates could both pass the pre-check and
            // the second would clobber the first's populated vault.
            if self.store.vault_exists(name).map_err(AppError::from)? {
                return Err(AppError::VaultAlreadyExists(name.to_string()));
            }
            self.store.save_vault(&vault, &key).map_err(AppError::from)
        })?;
        Ok(Session::new(
            name.to_string(),
            vault,
            key,
            self.auto_lock_after,
        ))
    }

    /// Unlocks an existing self-contained vault. This method NEVER writes: on a
    /// wrong password or a corrupt file it returns an error and leaves the vault
    /// untouched.
    pub fn unlock(&self, name: &str, password: &str) -> Result<Session> {
        let key = self.derive_unlock_key(name, password)?;
        self.open_with_key(name, &key)
    }

    /// Derives the decryption key for a vault from the master password. This is
    /// the CPU-heavy step (Argon2); a GUI should run it off the UI thread and
    /// then call [`Self::open_with_key`] with the result. Never writes.
    pub fn derive_unlock_key(&self, name: &str, password: &str) -> Result<KeyMaterial> {
        if !self.vault_exists(name)? {
            return Err(AppError::VaultNotFound(name.to_string()));
        }
        let normalized = normalize_master_password(password);
        if normalized.is_empty() {
            return Err(AppError::WrongMasterPassword);
        }
        let key = self.resolve_key(name, &normalized)?;

        // Compatibility fallback: keys derived before NFC normalization landed
        // used the trim-only form. When the two forms differ (non-NFC input,
        // e.g. decomposed accents from macOS) and the NFC key does not open the
        // vault, retry with the legacy form so pre-NFC vaults stay openable.
        // Verification only reads; this path never writes. The common ASCII
        // case costs nothing (both forms are byte-identical).
        let legacy = crate::password::normalize_master_password_legacy(password);
        if *legacy != *normalized && self.store.load_vault(name, &key).is_err() {
            let legacy_key = self.resolve_key(name, &legacy)?;
            if self.store.load_vault(name, &legacy_key).is_ok() {
                return Ok(legacy_key);
            }
        }
        Ok(key)
    }

    /// Opens a vault with an already-derived key (the cheap step: load + verify).
    /// A wrong key surfaces as [`AppError::WrongMasterPassword`]; the file is
    /// never modified.
    pub fn open_with_key(&self, name: &str, key: &KeyMaterial) -> Result<Session> {
        let vault = self.store.load_vault(name, key).map_err(AppError::from)?;
        Ok(Session::new(
            name.to_string(),
            vault,
            key.clone(),
            self.auto_lock_after,
        ))
    }

    /// Derives/loads the decryption key for a vault.
    ///
    /// Prefers V2 self-contained vaults (KDF params embedded in the file, so the
    /// master password alone suffices on any device). Falls back to the legacy
    /// V1 model where the key lives in a standalone `key.json` keyfile beside the
    /// vault, so vaults written by earlier CLI/TUI builds still open.
    fn resolve_key(&self, name: &str, normalized_pw: &str) -> Result<KeyMaterial> {
        if let Some(embedded) = self
            .store
            .load_vault_kdf_params(name)
            .map_err(AppError::from)?
        {
            let params = embedded.to_kdf_params();
            return derive_key(normalized_pw, &params).map_err(AppError::from);
        }
        let key_path = self.data_dir.join("key.json");
        let (key, _) = load_key(&key_path, Some(normalized_pw)).map_err(AppError::from)?;
        Ok(key)
    }

    /// Applies a candidate vault payload (e.g. freshly pulled from a remote) to
    /// the local file, but only after proving it decrypts with `key`. A safety
    /// backup of the current vault is taken first. If the payload does not
    /// decrypt, the live vault is never touched.
    pub fn apply_remote_validated(
        &self,
        name: &str,
        key: &KeyMaterial,
        bytes: &[u8],
    ) -> Result<()> {
        // 1. Prove it decrypts, in memory, before anything on disk changes.
        //    Preserve the typed failure signal (UnsupportedVersion /
        //    IntegrityCheckFailed / WrongMasterPassword): a caller must be able
        //    to tell a merely-newer or corrupt remote from a real auth failure,
        //    so it never force-overwrites a good remote in response.
        let candidate = self
            .store
            .load_vault_from_bytes(bytes, key)
            .map_err(AppError::from)?;

        // 2. Refuse a format downgrade that would strip self-containment: writing
        //    a V1 (keyfile-dependent) candidate over an existing V2 vault would
        //    make the next unlock resolve the key from a key.json the user may
        //    not have, locking them out. If we already have a V2 vault, the
        //    candidate must also be self-contained.
        let candidate_is_v2 = candidate.key_metadata.kdf_params.is_some();
        if !candidate_is_v2 {
            if let Ok(Some(_)) = self.store.load_vault_kdf_params(name) {
                return Err(AppError::Validation(
                    "refusing to apply a legacy (keyfile) payload over a self-contained vault"
                        .to_string(),
                ));
            }
        }

        // 3. Under lock: safety-backup the current vault, then commit.
        with_vault_locked(&self.lock_dir(), name, || {
            if self.vault_exists(name)? {
                self.store.create_backup(name).map_err(AppError::from)?;
            }
            self.store
                .apply_sync_payload(name, bytes)
                .map_err(AppError::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A vault created BEFORE NFC normalization (key derived from the trim-only
    /// byte form of an NFD password) must still open when the same keystrokes
    /// are typed today - via the legacy fallback in `derive_unlock_key`.
    #[test]
    fn pre_nfc_vault_still_unlocks_via_legacy_fallback() {
        let dir = TempDir::new().unwrap();
        let app = App::open(OpenOptions {
            data_dir: Some(dir.path().to_path_buf()),
            auto_lock_after: None,
        })
        .unwrap();

        // Simulate the pre-NFC world: derive the key directly from the
        // decomposed (NFD) bytes, bypassing today's NFC choke point.
        let nfd_password = "cafe\u{0301}"; // "café" with a combining accent
        let params = KeyDerivationParams::generate_adaptive();
        let key = derive_key(nfd_password, &params).unwrap();
        let metadata =
            KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305).with_embedded_kdf(&params);
        let vault = Vault::new("legacy", metadata);
        app.store.save_vault(&vault, &key).unwrap();

        // Typing the identical keystrokes today normalizes to NFC ("caf\u{e9}"),
        // whose key differs - the fallback must recover the legacy key from the
        // trim-only form of the same input.
        let session = app.unlock("legacy", nfd_password).unwrap();
        assert_eq!(session.name(), "legacy");

        // A genuinely wrong password is still rejected (both forms fail).
        assert!(matches!(
            app.unlock("legacy", "wrong"),
            Err(AppError::WrongMasterPassword)
        ));
    }
}
