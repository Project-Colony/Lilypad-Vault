//! Vault health orchestration ("Watchtower").
//!
//! `lilypad-common` has the analysis engine (reuse/weak/expiry/2FA detection and
//! an A-F grade) but it needs every entry's plaintext password at once. This
//! decrypts the whole vault, feeds the engine, and returns the report; the
//! transient plaintext lives only inside the local `EntryHealthData` vec, which
//! is dropped when this function returns.

use crate::secret::open_secret;
use crate::session::Session;
use lilypad_common::{analyze_vault_health, EntryHealthData, HealthReport};
use std::collections::HashSet;
use zeroize::Zeroize;

/// Computes the security health report for an unlocked vault (offline; the
/// compromised category stays empty - see [`vault_health_with_breaches`]).
pub fn vault_health(session: &Session) -> HealthReport {
    vault_health_with_breaches(session, &HashSet::new())
}

/// Computes the health report, marking the given labels as breach-compromised
/// (as returned by an explicit `breach::breach_check` run - checking is a
/// network operation and never happens implicitly here).
///
/// Trashed entries are excluded: with soft-delete as the default, a deleted
/// weak/reused password must clear its finding, not haunt the report until the
/// Trash is purged.
pub fn vault_health_with_breaches(
    session: &Session,
    compromised: &HashSet<String>,
) -> HealthReport {
    let vault = session.vault();
    let mut data = Vec::with_capacity(vault.entries.len());
    for entry in vault.entries.iter().filter(|e| e.deleted_at == 0) {
        let (password, has_totp) = match open_secret(session.key(), entry) {
            Ok(secret) => (secret.password.clone(), secret.totp_secret.is_some()),
            Err(_) => (String::new(), false),
        };
        data.push(EntryHealthData {
            label: entry.label.clone(),
            password,
            has_username: entry.metadata.username.is_some(),
            has_url: entry.metadata.url.is_some(),
            has_totp,
            password_age_days: entry.password_age_days(),
            days_until_expiry: entry.days_until_password_expires(),
            is_expired: entry.is_password_expired(),
            is_compromised: compromised.contains(&entry.label),
        });
    }
    let report = analyze_vault_health(&data);
    // Wipe our plaintext copies before dropping the vec (the analyzer's own
    // transient copies are its concern; this closes the copies we made).
    for d in data.iter_mut() {
        d.password.zeroize();
    }
    report
}
