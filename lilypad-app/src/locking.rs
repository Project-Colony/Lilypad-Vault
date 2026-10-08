//! Cross-process vault locking.
//!
//! `lilypad-storage`'s file locking is decorative: its atomic writer locks the
//! freshly-created temp file (which no other process can see) and then renames
//! it over the destination, so two processes doing read-modify-write on the same
//! vault can interleave and silently lose each other's changes. `lilypad-app`
//! serializes the WHOLE read-modify-write by holding an exclusive lock on a
//! stable per-vault lockfile for the duration of the closure.

use crate::error::{AppError, Result};
use fs2::FileExt;
use std::fs::OpenOptions;
use std::path::Path;

/// Runs `f` while holding an exclusive advisory lock on `<lock_dir>/<name>.lock`.
///
/// The lock is released when the guard drops, whether `f` returns `Ok` or `Err`
/// or panics. Locking a stable sidecar file (rather than the vault file that
/// gets renamed underneath it) is what makes the lock actually serialize writers.
pub fn with_vault_locked<T>(
    lock_dir: &Path,
    name: &str,
    f: impl FnOnce() -> Result<T>,
) -> Result<T> {
    // Validate the vault name BEFORE deriving a lockfile path from it, so a
    // hostile name like "../evil" cannot create/lock a file outside lock_dir.
    lilypad_common::validation::validate_vault_name(name)
        .map_err(|e| AppError::Validation(format!("invalid vault name: {e}")))?;
    std::fs::create_dir_all(lock_dir)
        .map_err(|e| AppError::Io(format!("failed to create lock dir: {e}")))?;
    let lock_path = lock_dir.join(format!("{name}.lock"));
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| AppError::Io(format!("failed to open lockfile: {e}")))?;
    file.lock_exclusive()
        .map_err(|e| AppError::Io(format!("failed to acquire vault lock: {e}")))?;

    let result = f();

    // Best-effort unlock; the guard also releases on drop.
    let _ = FileExt::unlock(&file);
    result
}
