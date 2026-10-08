//! The single, typed error surface of the application layer.
//!
//! The backend crates are inconsistent: `lilypad-core` returns [`CoreError`],
//! `lilypad-storage` leaks `anyhow::Error` from its public API, and validation
//! returns its own error. The frontends previously string-matched those to tell
//! "wrong password" apart from "vault missing" apart from "corrupted" - which is
//! exactly how the TUI's data-loss bug slipped in. `lilypad-app` collapses all
//! of that into one enum so callers branch on variants, never on message text.

use lilypad_core::CoreError;

/// Every fallible `lilypad-app` operation returns `Result<T, AppError>`.
pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("vault '{0}' does not exist")]
    VaultNotFound(String),

    #[error("a vault named '{0}' already exists")]
    VaultAlreadyExists(String),

    #[error("incorrect master password")]
    WrongMasterPassword,

    #[error("vault integrity check failed: the file may be corrupted or tampered with")]
    IntegrityCheckFailed,

    #[error("vault '{0}' is not a self-contained (V2) vault; no embedded key parameters")]
    NotSelfContained(String),

    #[error("vault format is newer than this build supports; please update Lilypad")]
    UnsupportedVersion,

    #[error("the vault is locked")]
    Locked,

    #[error("not authenticated with the sync provider")]
    NotAuthenticated,

    #[error("network error: {0}")]
    Network(String),

    #[error("sync error: {0}")]
    Sync(String),

    #[error("invalid input: {0}")]
    Validation(String),

    #[error("cryptography error: {0}")]
    Crypto(String),

    #[error("key derivation failed: {0}")]
    Kdf(String),

    #[error("i/o error: {0}")]
    Io(String),

    /// A storage/other error that did not match a more specific classification.
    #[error("{0}")]
    Other(String),
}

/// Maps a borrowed [`CoreError`] into an [`AppError`]. `CoreError` is not
/// `Clone`, so the owned and borrowed conversions both route through here.
fn from_core_ref(e: &CoreError) -> AppError {
    match e {
        CoreError::Crypto(m) => AppError::Crypto(m.clone()),
        CoreError::Kdf(m) => AppError::Kdf(m.clone()),
        CoreError::NotFound(m) => AppError::Other(format!("not found: {m}")),
        CoreError::AlreadyExists(m) => AppError::VaultAlreadyExists(m.clone()),
        CoreError::InvalidInput(m) | CoreError::Serialization(m) => AppError::Validation(m.clone()),
        CoreError::InvalidKeyLength { expected, actual } => AppError::Crypto(format!(
            "invalid key length: expected {expected}, got {actual}"
        )),
    }
}

impl From<CoreError> for AppError {
    fn from(e: CoreError) -> Self {
        from_core_ref(&e)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Validation(format!("serialization error: {e}"))
    }
}

/// Classifies an `anyhow::Error` (as leaked by `lilypad-storage`) into a typed
/// [`AppError`] by inspecting its message. This is deliberately the *only* place
/// in the codebase that string-matches storage errors; every caller above this
/// boundary works with typed variants.
impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        // Prefer a structured downcast when the source is a CoreError.
        if let Some(core) = e.downcast_ref::<CoreError>() {
            return from_core_ref(core);
        }
        let msg = e.to_string();
        let lower = msg.to_lowercase();
        // Order matters: match specific auth/version/corruption signals before
        // the generic catch-alls, so a present-but-unreadable vault is reported
        // as corruption (prompting a restore) rather than a wrong password or a
        // missing vault.
        if lower.contains("incorrect master password") || lower.contains("key id mismatch") {
            AppError::WrongMasterPassword
        } else if lower.contains("not supported") || lower.contains("upgrade lilypad") {
            AppError::UnsupportedVersion
        } else if lower.contains("integrity check failed")
            || lower.contains("checksum")
            || lower.contains("invalid header")
            || lower.contains("expected value")
            || lower.contains("eof while parsing")
            || lower.contains("trailing characters")
            || lower.contains("invalid utf-8")
            || lower.contains("corrupt")
        {
            // Present-but-unreadable file = corruption, not a wrong password.
            AppError::IntegrityCheckFailed
        } else if lower.contains("invalid vault name") {
            AppError::Validation(msg)
        } else if lower.contains("does not exist") || lower.contains("not found") {
            // Storage phrases missing vaults/backups as "does not exist".
            AppError::VaultNotFound(msg)
        } else if lower.contains("invalid") {
            AppError::Validation(msg)
        } else {
            AppError::Other(msg)
        }
    }
}
