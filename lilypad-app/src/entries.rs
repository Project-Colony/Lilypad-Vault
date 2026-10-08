//! Entry operations: cleartext list views, on-demand secret reveal, and
//! locked read-modify-write mutations.
//!
//! Two policies live here so all frontends inherit them:
//!
//! * **On-demand decryption.** Lists are built from [`EntryView`]s, which carry
//!   only the cleartext metadata already stored unencrypted in the vault. A
//!   secret is decrypted one entry at a time via [`reveal_secret`] and wiped as
//!   soon as the returned guard drops - not eagerly decrypted en masse and held
//!   for the whole session.
//! * **Serialized read-modify-write.** Every mutation reloads the vault from
//!   disk under an exclusive lock, applies the change, and saves - so a
//!   concurrent process cannot silently lose an update.

use crate::error::{AppError, Result};
use crate::secret::{open_secret, seal_secret, RevealedSecret};
use crate::session::Session;
use crate::vault::App;
use lilypad_core::{Entry, EntryMetadata, EntrySecret, EntryType, Vault};
use zeroize::{Zeroize, Zeroizing};

/// A read-only, secret-free projection of an entry for list/detail views.
#[derive(Debug, Clone)]
pub struct EntryView {
    pub label: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub folder: Option<String>,
    pub entry_type: EntryType,
    pub is_favorite: bool,
    pub color: Option<lilypad_core::EntryColor>,
    /// Unix timestamp the password expires at (0 = no expiry).
    pub password_expires_at: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

impl EntryView {
    fn from_entry(entry: &Entry) -> Self {
        Self {
            label: entry.label.clone(),
            username: entry.metadata.username.clone(),
            url: entry.metadata.url.clone(),
            tags: entry.metadata.tags.clone(),
            folder: entry.metadata.folder.clone(),
            entry_type: entry.metadata.entry_type.clone(),
            is_favorite: entry.is_favorite,
            color: entry.color,
            password_expires_at: entry.password_expires_at,
            created_at: entry.created_at,
            updated_at: entry.updated_at,
        }
    }
}

/// Lists the live (non-trashed) entries as secret-free views.
pub fn list_entries(session: &Session) -> Vec<EntryView> {
    session
        .vault()
        .entries
        .iter()
        .filter(|e| e.deleted_at == 0)
        .map(EntryView::from_entry)
        .collect()
}

/// Lists the soft-deleted (trashed) entries.
pub fn list_trash(session: &Session) -> Vec<EntryView> {
    session
        .vault()
        .entries
        .iter()
        .filter(|e| e.deleted_at != 0)
        .map(EntryView::from_entry)
        .collect()
}

/// Searches live entries by their cleartext metadata: label, username, url,
/// tags and folder. Secrets are never touched.
pub fn search_entries(session: &Session, query: &str) -> Vec<EntryView> {
    session
        .vault()
        .search_entries(query)
        .into_iter()
        .filter(|e| e.deleted_at == 0)
        .map(EntryView::from_entry)
        .collect()
}

/// Returns the vault's audit log (append-only record of mutations), oldest
/// first. Entries carry only a cleartext action name and optional entry label,
/// never a secret.
pub fn audit_log(session: &Session) -> Vec<lilypad_core::AuditEvent> {
    session.vault().audit_log.clone()
}

/// Decrypts a single entry's secret on demand, returning a zeroizing guard.
pub fn reveal_secret(session: &Session, label: &str) -> Result<RevealedSecret> {
    let entry = session
        .vault()
        .find_entry(label)
        .ok_or_else(|| AppError::Other(format!("entry '{label}' not found")))?;
    open_secret(session.key(), entry)
}

/// Runs a mutation as a locked read-modify-write, then refreshes the session's
/// in-memory vault from the saved result.
fn mutate(
    app: &App,
    session: &mut Session,
    f: impl FnOnce(&mut Vault) -> Result<()>,
) -> Result<()> {
    let key = session.key().clone();
    let name = session.name().to_string();
    let lock_dir = app.lock_dir();
    let updated = with_vault_locked(&lock_dir, &name, || {
        let mut vault = app
            .store()
            .load_vault(&name, &key)
            .map_err(AppError::from)?;
        f(&mut vault)?;
        app.store()
            .save_vault(&vault, &key)
            .map_err(AppError::from)?;
        Ok(vault)
    })?;
    *session.vault_mut() = updated;
    session.touch();
    Ok(())
}

/// Adds a new entry (metadata + secret) to the vault.
pub fn add_entry(
    app: &App,
    session: &mut Session,
    label: &str,
    metadata: EntryMetadata,
    secret: &EntrySecret,
) -> Result<()> {
    let ciphertext = seal_secret(session.key(), secret)?;
    let entry = Entry::new_with_metadata(label, metadata, ciphertext);
    mutate(app, session, |vault| {
        vault.add_entry(entry).map_err(AppError::from)
    })
}

/// Replaces an entry's secret (its encrypted payload).
pub fn update_secret(
    app: &App,
    session: &mut Session,
    label: &str,
    secret: &EntrySecret,
) -> Result<()> {
    let ciphertext = seal_secret(session.key(), secret)?;
    let label = label.to_string();
    mutate(app, session, move |vault| {
        vault
            .update_entry(&label, ciphertext)
            .map_err(AppError::from)
    })
}

/// Applies `f` to an entry's decrypted secret under the vault lock and saves
/// the re-sealed result; `f` returning an error leaves the vault untouched.
fn mutate_secret<T>(
    app: &App,
    session: &mut Session,
    label: &str,
    f: impl FnOnce(&mut EntrySecret) -> Result<T>,
) -> Result<T> {
    let key = session.key().clone();
    let mut out = None;
    mutate(app, session, |vault| {
        let entry = vault
            .find_entry(label)
            .ok_or_else(|| AppError::Other(format!("entry '{label}' not found")))?;
        let mut secret = open_secret(&key, entry)?.get().clone();
        let sealed = f(&mut secret).and_then(|v| Ok((v, seal_secret(&key, &secret)?)));
        wipe_secret(&mut secret);
        let (v, ciphertext) = sealed?;
        out = Some(v);
        vault
            .update_entry(label, ciphertext)
            .map_err(AppError::from)
    })?;
    out.ok_or_else(|| AppError::Other("secret mutation produced no result".to_string()))
}

/// Replaces a TOTP entry's backup codes with a fresh set and returns them, to
/// be shown to the user once.
pub fn generate_backup_codes(
    app: &App,
    session: &mut Session,
    label: &str,
) -> Result<Zeroizing<Vec<String>>> {
    mutate_secret(app, session, label, |secret| {
        if secret.totp_secret.is_none() {
            return Err(AppError::Validation(format!(
                "entry '{label}' has no TOTP secret; backup codes require TOTP"
            )));
        }
        Ok(Zeroizing::new(secret.generate_backup_codes()))
    })
}

/// Marks a backup code as used and returns how many unused codes remain.
pub fn use_backup_code(app: &App, session: &mut Session, label: &str, code: &str) -> Result<usize> {
    mutate_secret(app, session, label, |secret| {
        if secret.use_backup_code(code) {
            Ok(secret.unused_backup_codes_count())
        } else {
            Err(AppError::Validation(
                "invalid or already used backup code".to_string(),
            ))
        }
    })
}

/// Replaces an entry's cleartext metadata.
pub fn update_metadata(
    app: &App,
    session: &mut Session,
    label: &str,
    metadata: EntryMetadata,
) -> Result<()> {
    let label = label.to_string();
    mutate(app, session, move |vault| {
        vault
            .update_entry_metadata(&label, metadata)
            .map_err(AppError::from)
    })
}

/// The form-visible fields of an entry, for a field-preserving edit.
///
/// Everything an entry can hold that is *not* in this struct - attachments,
/// TOTP backup codes, email, phone, custom fields (in the secret) and tags,
/// folder (in the metadata) - is preserved across the edit. This is the fix
/// for the data-loss bug where a frontend rebuilt the secret/metadata from
/// scratch and silently dropped every field it did not render.
#[derive(Debug, Clone)]
pub struct EntryEdit {
    /// The (possibly renamed) label.
    pub new_label: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub entry_type: EntryType,
    pub password: String,
    pub notes: Option<String>,
    pub totp_secret: Option<String>,
    /// `None` preserves the entry's current tags; `Some` replaces them.
    pub tags: Option<Vec<String>>,
    /// `None` preserves the current folder; `Some(None)` clears it.
    pub folder: Option<Option<String>>,
}

// The edit carries plaintext secrets from the caller's form; wipe them when the
// edit value is dropped (edit_entry consumes it by value).
impl Drop for EntryEdit {
    fn drop(&mut self) {
        self.password.zeroize();
        if let Some(n) = self.notes.as_mut() {
            n.zeroize();
        }
        if let Some(t) = self.totp_secret.as_mut() {
            t.zeroize();
        }
    }
}

/// One recorded change of an entry, without any secret material. The previous
/// password ciphertexts stay sealed inside the vault; this view only says that
/// a change happened, when, and of what kind.
#[derive(Debug, Clone)]
pub struct HistoryEvent {
    pub timestamp: u64,
    /// Kebab-case change kind: created, password-changed, metadata-updated,
    /// notes-updated, renamed, totp-updated, attachments-updated.
    pub change: String,
    /// Whether the record retains the previous password (encrypted).
    pub stores_previous_password: bool,
}

/// Returns an entry's change history, most recent first (metadata only; no
/// secrets are decrypted).
pub fn entry_history(session: &Session, label: &str) -> Result<Vec<HistoryEvent>> {
    use lilypad_core::EntryChangeType as C;
    let entry = session
        .vault()
        .find_entry(label)
        .ok_or_else(|| AppError::Other(format!("entry '{label}' not found")))?;
    Ok(entry
        .get_history()
        .map(|h| HistoryEvent {
            timestamp: h.timestamp,
            change: match h.change_type {
                C::Created => "created",
                C::PasswordChanged => "password-changed",
                C::MetadataUpdated => "metadata-updated",
                C::NotesUpdated => "notes-updated",
                C::Renamed => "renamed",
                C::TotpUpdated => "totp-updated",
                C::AttachmentsUpdated => "attachments-updated",
            }
            .to_string(),
            stores_previous_password: h.previous_ciphertext.is_some(),
        })
        .collect())
}

/// Edits an entry while preserving every field the caller did not touch.
///
/// Runs as a single locked read-modify-write: it reloads the vault, decrypts
/// the entry's current secret, overwrites only the fields in [`EntryEdit`],
/// preserves the rest (attachments, backup codes, email, phone, custom fields,
/// tags, folder), records password history when the password actually changed,
/// re-encrypts, renames if the label changed, and saves - all atomically.
pub fn edit_entry(
    app: &App,
    session: &mut Session,
    original_label: &str,
    edit: EntryEdit,
) -> Result<()> {
    let key = session.key().clone();
    let name = session.name().to_string();
    let lock_dir = app.lock_dir();
    let original_label = original_label.to_string();

    let updated = with_vault_locked(&lock_dir, &name, || {
        let mut vault = app
            .store()
            .load_vault(&name, &key)
            .map_err(AppError::from)?;

        // Read the current secret + metadata under the lock so preserved fields
        // come from the freshest on-disk state, and capture the previous
        // ciphertext for the password-history record.
        let (mut new_secret, prev_ciphertext, new_metadata, password_changed) = {
            let entry = vault
                .entries
                .iter()
                .find(|e| e.label == original_label)
                .ok_or_else(|| AppError::Other(format!("entry '{original_label}' not found")))?;
            let current = open_secret(&key, entry)?;
            let password_changed = current.password != edit.password;
            let new_secret = EntrySecret {
                password: edit.password.clone(),
                notes: edit.notes.clone(),
                totp_secret: edit.totp_secret.clone(),
                attachments: current.attachments.clone(),
                totp_backup_codes: current.totp_backup_codes.clone(),
                email: current.email.clone(),
                phone: current.phone.clone(),
                custom_fields: current.custom_fields.clone(),
            };
            let new_metadata = EntryMetadata {
                username: edit.username.clone(),
                url: edit.url.clone(),
                tags: edit
                    .tags
                    .clone()
                    .unwrap_or_else(|| entry.metadata.tags.clone()),
                folder: edit
                    .folder
                    .clone()
                    .unwrap_or_else(|| entry.metadata.folder.clone()),
                entry_type: edit.entry_type.clone(),
            };
            (
                new_secret,
                entry.ciphertext.clone(),
                new_metadata,
                password_changed,
            )
            // `current` (RevealedSecret) drops here, wiping the decrypted copy.
        };

        // Validate and seal, wiping the plaintext copies in `new_secret` on
        // every path (including validation/seal errors) before returning.
        let sealed = new_secret
            .validate()
            .map_err(AppError::from)
            .and_then(|_| new_metadata.validate().map_err(AppError::from))
            .and_then(|_| seal_secret(&key, &new_secret));
        wipe_secret(&mut new_secret);
        let ciphertext = sealed?;

        {
            let entry = vault
                .entries
                .iter_mut()
                .find(|e| e.label == original_label)
                .ok_or_else(|| AppError::Other(format!("entry '{original_label}' not found")))?;
            entry.metadata = new_metadata;
            if password_changed {
                // Records password_changed_at, pushes a history record carrying
                // the previous ciphertext, and bumps updated_at.
                entry.record_password_change_with_history(Some(prev_ciphertext));
            } else {
                entry.updated_at = lilypad_common::current_timestamp();
            }
            entry.ciphertext = ciphertext;
        }
        vault.record_mutation("entry_updated", Some(&original_label));

        if original_label != edit.new_label {
            vault
                .rename_entry(&original_label, edit.new_label.clone())
                .map_err(AppError::from)?;
        }

        app.store()
            .save_vault(&vault, &key)
            .map_err(AppError::from)?;
        Ok(vault)
    })?;

    *session.vault_mut() = updated;
    session.touch();
    Ok(())
}

/// Wipes the sensitive plaintext fields of an [`EntrySecret`] we built in memory
/// (mirrors `RevealedSecret`'s drop, which `EntrySecret` itself does not have).
fn wipe_secret(secret: &mut EntrySecret) {
    secret.password.zeroize();
    if let Some(n) = secret.notes.as_mut() {
        n.zeroize();
    }
    if let Some(t) = secret.totp_secret.as_mut() {
        t.zeroize();
    }
    if let Some(e) = secret.email.as_mut() {
        e.zeroize();
    }
    if let Some(p) = secret.phone.as_mut() {
        p.zeroize();
    }
    for f in secret.custom_fields.iter_mut() {
        f.value.zeroize();
    }
    for a in secret.attachments.iter_mut() {
        a.data_base64.zeroize();
    }
    for c in secret.totp_backup_codes.iter_mut() {
        c.code.zeroize();
    }
}

/// Removes an entry from the vault.
pub fn delete_entry(app: &App, session: &mut Session, label: &str) -> Result<()> {
    let label = label.to_string();
    mutate(app, session, move |vault| {
        vault
            .remove_entry(&label)
            .map(|_| ())
            .map_err(AppError::from)
    })
}

/// Soft-deletes an entry: moves it to Trash (recoverable) instead of erasing it.
/// Errors if the entry is already in Trash, so a repeated delete cannot silently
/// re-stamp the deletion time (and a typo surfaces instead of "succeeding").
pub fn soft_delete(app: &App, session: &mut Session, label: &str) -> Result<()> {
    let label = label.to_string();
    mutate(app, session, move |vault| {
        {
            let entry = vault
                .entries
                .iter_mut()
                .find(|e| e.label == label)
                .ok_or_else(|| AppError::Other(format!("entry '{label}' not found")))?;
            if entry.deleted_at != 0 {
                return Err(AppError::Other(format!(
                    "entry '{label}' is already in Trash"
                )));
            }
            let now = lilypad_common::current_timestamp();
            entry.deleted_at = now;
            // Trashing IS a modification: without this bump, a sync merge's
            // last-write-wins would let an older live copy from another
            // replica override the (newer) move to Trash.
            entry.updated_at = now;
        }
        vault.record_mutation("entry_trashed", Some(&label));
        Ok(())
    })
}

/// Restores a soft-deleted entry from Trash. Errors if the entry is not in
/// Trash, so a typo yields an error instead of a false "Restored" success.
pub fn restore(app: &App, session: &mut Session, label: &str) -> Result<()> {
    let label = label.to_string();
    mutate(app, session, move |vault| {
        {
            let entry = vault
                .entries
                .iter_mut()
                .find(|e| e.label == label)
                .ok_or_else(|| AppError::Other(format!("entry '{label}' not found")))?;
            if entry.deleted_at == 0 {
                return Err(AppError::Other(format!("entry '{label}' is not in Trash")));
            }
            entry.deleted_at = 0;
            // Same reasoning as soft_delete: the restore must win a merge
            // against the still-trashed copy on another replica.
            entry.updated_at = lilypad_common::current_timestamp();
        }
        vault.record_mutation("entry_restored", Some(&label));
        Ok(())
    })
}

/// Renames an entry's label.
pub fn rename_entry(app: &App, session: &mut Session, label: &str, new_label: &str) -> Result<()> {
    let label = label.to_string();
    let new_label = new_label.to_string();
    mutate(app, session, move |vault| {
        vault
            .rename_entry(&label, new_label)
            .map_err(AppError::from)
    })
}

/// Sets or clears an entry's favorite flag (persisted).
pub fn set_favorite(app: &App, session: &mut Session, label: &str, favorite: bool) -> Result<()> {
    let label = label.to_string();
    mutate(app, session, move |vault| {
        vault
            .set_entry_favorite(&label, favorite)
            .map_err(AppError::from)
    })
}

/// Sets or clears an entry's color label (persisted).
pub fn set_color(
    app: &App,
    session: &mut Session,
    label: &str,
    color: Option<lilypad_core::EntryColor>,
) -> Result<()> {
    let label = label.to_string();
    mutate(app, session, move |vault| {
        vault.set_entry_color(&label, color).map_err(AppError::from)
    })
}

/// Moves an entry to a folder (or clears it with `None`).
pub fn set_folder(
    app: &App,
    session: &mut Session,
    label: &str,
    folder: Option<String>,
) -> Result<()> {
    let label = label.to_string();
    mutate(app, session, move |vault| {
        vault
            .set_entry_folder(&label, folder)
            .map_err(AppError::from)
    })
}

/// Adds a tag to an entry (persisted).
pub fn add_tag(app: &App, session: &mut Session, label: &str, tag: &str) -> Result<()> {
    let label = label.to_string();
    let tag = tag.to_string();
    mutate(app, session, move |vault| {
        vault.add_entry_tag(&label, tag).map_err(AppError::from)
    })
}

/// Removes a tag from an entry (persisted).
pub fn remove_tag(app: &App, session: &mut Session, label: &str, tag: &str) -> Result<()> {
    let label = label.to_string();
    let tag = tag.to_string();
    mutate(app, session, move |vault| {
        vault.remove_entry_tag(&label, &tag).map_err(AppError::from)
    })
}

/// Sets an entry's password expiry (in days from now), or clears it with 0.
pub fn set_expiry_days(app: &App, session: &mut Session, label: &str, days: u32) -> Result<()> {
    let label = label.to_string();
    mutate(app, session, move |vault| {
        {
            let entry = vault
                .entries
                .iter_mut()
                .find(|e| e.label == label)
                .ok_or_else(|| AppError::Other(format!("entry '{label}' not found")))?;
            if days == 0 {
                entry.password_expires_at = 0;
            } else {
                entry.set_password_expiry_days(days);
            }
        }
        vault.record_mutation("entry_expiry_updated", Some(&label));
        Ok(())
    })
}

// Bring the locking helper into scope for `mutate`.
use crate::locking::with_vault_locked;
