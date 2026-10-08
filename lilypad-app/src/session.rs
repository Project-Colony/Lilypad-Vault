//! An unlocked vault session.
//!
//! A [`Session`] owns the decrypted [`Vault`] aggregate plus the [`KeyMaterial`]
//! needed to reveal individual secrets on demand. Crucially it does NOT hold the
//! decrypted passwords: entry secrets are decrypted one at a time via the
//! `entries` module and wiped immediately, so a long-lived session keeps only
//! the key resident, not every password.
//!
//! The session also owns auto-lock timing so all three frontends share one idle
//! policy instead of the desktop having a timer and the TUI having none.

use lilypad_core::{KeyMaterial, Vault};
use std::time::{Duration, Instant};

/// A live, unlocked vault.
///
/// `Debug` is safe to derive: `KeyMaterial`'s own `Debug` redacts the key bytes,
/// so a `{:?}` of a `Session` never prints secret material.
#[derive(Debug)]
pub struct Session {
    name: String,
    vault: Vault,
    key: KeyMaterial,
    last_activity: Instant,
    auto_lock_after: Option<Duration>,
}

impl Session {
    pub(crate) fn new(
        name: String,
        vault: Vault,
        key: KeyMaterial,
        auto_lock_after: Option<Duration>,
    ) -> Self {
        Self {
            name,
            vault,
            key,
            last_activity: Instant::now(),
            auto_lock_after,
        }
    }

    /// The name of the unlocked vault.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The decrypted vault aggregate (metadata + entry ciphertexts).
    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    pub(crate) fn vault_mut(&mut self) -> &mut Vault {
        &mut self.vault
    }

    /// Replaces the session's key and vault after a re-key (e.g. master-password
    /// change). The old [`KeyMaterial`] is dropped here, zeroizing it.
    pub(crate) fn rekey(&mut self, key: KeyMaterial, vault: Vault) {
        self.key = key;
        self.vault = vault;
        self.last_activity = Instant::now();
    }

    /// The in-memory decryption key, for revealing secrets on demand.
    pub fn key(&self) -> &KeyMaterial {
        &self.key
    }

    /// Records user activity, resetting the idle timer.
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    /// How long the session has been idle.
    pub fn idle_for(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// Whether the auto-lock deadline has passed.
    pub fn is_expired(&self) -> bool {
        match self.auto_lock_after {
            Some(limit) => self.last_activity.elapsed() >= limit,
            None => false,
        }
    }

    /// Time left before auto-lock, if a policy is set and not yet expired.
    pub fn time_until_lock(&self) -> Option<Duration> {
        self.auto_lock_after
            .map(|limit| limit.saturating_sub(self.last_activity.elapsed()))
    }

    /// Changes the idle auto-lock policy on the live session (e.g. after the user
    /// edits it in settings) and resets the idle timer so the new limit is
    /// measured from now, not from the last activity under the old policy.
    pub fn set_auto_lock(&mut self, after: Option<Duration>) {
        self.auto_lock_after = after;
        self.last_activity = Instant::now();
    }
}

// `KeyMaterial` is `ZeroizeOnDrop`, so dropping the `Session` (on lock/quit)
// wipes the key automatically; no manual `Drop` is required here.
