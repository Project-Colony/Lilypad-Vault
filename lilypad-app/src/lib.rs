//! # lilypad-app
//!
//! The shared application/service layer for Lilypad. It owns every vault
//! *decision* - unlocking, encrypting, locking, syncing, clipboard policy - so
//! the CLI, TUI and desktop frontends can be thin clients that only render and
//! collect input. Centralizing these operations here is what makes the two
//! audited data-loss bugs (the TUI overwriting a vault on a wrong password, and
//! sync overwriting the local vault before validating the remote) impossible to
//! reintroduce: there is exactly one place that knows how to write a vault.
//!
//! The backend crates (`lilypad-core`, `lilypad-storage`, `lilypad-oauth`,
//! `lilypad-common`) are unchanged; this crate composes them behind a single,
//! typed [`AppError`] surface.

pub mod breach;
pub mod crypto_ops;
pub mod entries;
pub mod error;
pub mod generator;
pub mod health;
pub mod import;
pub mod locking;
pub mod management;
pub mod password;
pub mod paths;
pub mod secret;
pub mod session;
pub mod settings;
pub mod sync;
pub mod totp;
pub mod vault;

pub use breach::{breach_check, collect_hashes, query_hashes, BreachReport, BreachedEntry};
pub use crypto_ops::change_master_password;
pub use entries::{
    add_entry, add_tag, audit_log, delete_entry, edit_entry, entry_history, generate_backup_codes,
    list_entries, list_trash, remove_tag, rename_entry, restore, reveal_secret, search_entries,
    set_color, set_expiry_days, set_favorite, set_folder, soft_delete, update_metadata,
    update_secret, use_backup_code, EntryEdit, EntryView, HistoryEvent,
};
pub use error::{AppError, Result};
pub use generator::{generate_password, PasswordOptions};
pub use health::{vault_health, vault_health_with_breaches};
pub use import::{
    detect_format, export_csv, import_entries, parse_import, ImportFormat, ImportReport,
    ImportedEntry, ParsedImport,
};
pub use lilypad_common::{HealthGrade, HealthIssue, HealthReport, HealthScore, IssueSeverity};
pub use lilypad_core::EntryColor;
pub use management::BackupEntry;
pub use secret::RevealedSecret;
pub use session::Session;
pub use settings::Settings;
pub use sync::{SyncState, SyncStatusView};
pub use totp::{code_for_secret, TotpAlgorithm, TotpConfig};
pub use vault::{App, OpenOptions};

// Re-export the core domain types the frontends need, so a frontend depends on
// `lilypad-app` alone rather than reaching into the backend crates directly.
pub use lilypad_core::{
    Attachment, AuditEvent, CustomField, CustomFieldType, EntryMetadata, EntrySecret, EntryType,
    KeyMaterial,
};

#[cfg(test)]
mod tests {
    use super::*;
    use lilypad_core::EntrySecret;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn test_app(dir: &TempDir) -> App {
        App::open(OpenOptions {
            data_dir: Some(PathBuf::from(dir.path())),
            auto_lock_after: None,
        })
        .expect("open app")
    }

    #[test]
    fn create_then_unlock_roundtrips() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let session = app
            .create_vault("personal", "correct horse battery")
            .unwrap();
        assert_eq!(session.name(), "personal");
        drop(session);

        let session = app.unlock("personal", "correct horse battery").unwrap();
        assert_eq!(session.vault().entries.len(), 0);
    }

    #[test]
    fn unlock_wrong_password_does_not_touch_the_vault() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);

        // Create a vault with one entry, then relock.
        let mut session = app.create_vault("work", "s3cret-pass").unwrap();
        add_entry(
            &app,
            &mut session,
            "github",
            EntryMetadata::default(),
            &EntrySecret::new("hunter2"),
        )
        .unwrap();
        drop(session);

        // Wrong password must be rejected as WrongMasterPassword...
        let err = app.unlock("work", "not-the-password").unwrap_err();
        assert!(matches!(err, AppError::WrongMasterPassword), "got {err:?}");

        // ...and must NOT have overwritten the vault: the entry survives and the
        // real password still opens it. This is the regression guard for the
        // TUI "empty vault overwrite" data-loss bug.
        let session = app.unlock("work", "s3cret-pass").unwrap();
        assert_eq!(session.vault().entries.len(), 1);
        assert_eq!(session.vault().entries[0].label, "github");
        let secret = reveal_secret(&session, "github").unwrap();
        assert_eq!(secret.password, "hunter2");
    }

    #[test]
    fn unlock_missing_vault_errors_without_creating() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let err = app.unlock("ghost", "whatever").unwrap_err();
        assert!(matches!(err, AppError::VaultNotFound(_)), "got {err:?}");
        assert!(!app.vault_exists("ghost").unwrap());
    }

    #[test]
    fn create_refuses_to_clobber_existing_vault() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        app.create_vault("dup", "pw-one").unwrap();
        let err = app.create_vault("dup", "pw-two").unwrap_err();
        assert!(
            matches!(err, AppError::VaultAlreadyExists(_)),
            "got {err:?}"
        );
        // The original password still opens it (it was not recreated).
        assert!(app.unlock("dup", "pw-one").is_ok());
    }

    #[test]
    fn apply_remote_validated_rejects_undecryptable_payload() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("sync", "master-pw").unwrap();
        add_entry(
            &app,
            &mut session,
            "keep",
            EntryMetadata::default(),
            &EntrySecret::new("do-not-lose-me"),
        )
        .unwrap();
        let key = session.key().clone();
        drop(session);

        // Garbage remote bytes must be rejected as corruption (not silently as a
        // wrong password) and must NOT overwrite local.
        let err = app
            .apply_remote_validated("sync", &key, b"not a real vault file")
            .unwrap_err();
        assert!(
            matches!(
                err,
                AppError::IntegrityCheckFailed | AppError::Validation(_)
            ),
            "got {err:?}"
        );

        // The local entry is still there: the pull-before-validate data-loss bug
        // cannot happen through this API.
        let session = app.unlock("sync", "master-pw").unwrap();
        assert_eq!(session.vault().entries.len(), 1);
        assert_eq!(session.vault().entries[0].label, "keep");
    }

    #[test]
    fn generator_respects_charset_and_length() {
        let pw = generate_password(&PasswordOptions {
            length: 32,
            uppercase: false,
            lowercase: true,
            digits: false,
            symbols: false,
        })
        .unwrap();
        assert_eq!(pw.len(), 32);
        assert!(pw.chars().all(|c| c.is_ascii_lowercase()));

        // No charset enabled is rejected.
        assert!(generate_password(&PasswordOptions {
            length: 8,
            uppercase: false,
            lowercase: false,
            digits: false,
            symbols: false,
        })
        .is_err());
    }

    #[test]
    fn totp_parses_otpauth_and_generates_six_digits() {
        // JBSWY3DPEHPK3PXP is the canonical base32 test secret.
        let cfg = TotpConfig::from_otpauth(
            "otpauth://totp/ACME:alice?secret=JBSWY3DPEHPK3PXP&issuer=ACME&digits=6&period=30",
        )
        .unwrap();
        assert_eq!(cfg.digits, 6);
        assert_eq!(cfg.period, 30);
        let code = cfg.current_code().unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn search_matches_cleartext_metadata() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("s", "pw").unwrap();
        let meta = EntryMetadata {
            username: Some("alice@example.com".to_string()),
            ..EntryMetadata::default()
        };
        add_entry(&app, &mut session, "GitHub", meta, &EntrySecret::new("p")).unwrap();
        add_entry(
            &app,
            &mut session,
            "GitLab",
            EntryMetadata::default(),
            &EntrySecret::new("p"),
        )
        .unwrap();

        let hits = search_entries(&session, "hub");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].label, "GitHub");
    }

    #[test]
    fn change_master_password_rekeys_and_preserves_secrets() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("v", "old-password").unwrap();
        add_entry(
            &app,
            &mut session,
            "bank",
            EntryMetadata::default(),
            &EntrySecret::new("s3cr3t"),
        )
        .unwrap();

        change_master_password(&app, &mut session, "brand-new-password").unwrap();
        // Session is transparently re-keyed: the secret is still readable.
        let secret = reveal_secret(&session, "bank").unwrap();
        assert_eq!(secret.password, "s3cr3t");
        drop(session);

        // Old password no longer opens the vault; new one does, secret intact.
        assert!(matches!(
            app.unlock("v", "old-password").unwrap_err(),
            AppError::WrongMasterPassword
        ));
        let session = app.unlock("v", "brand-new-password").unwrap();
        assert_eq!(session.vault().entries.len(), 1);
        let secret = reveal_secret(&session, "bank").unwrap();
        assert_eq!(secret.password, "s3cr3t");
    }

    #[test]
    fn rename_and_delete_vault() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        app.create_vault("old", "pw").unwrap();
        app.rename_vault("old", "new").unwrap();
        assert!(!app.vault_exists("old").unwrap());
        assert!(app.vault_exists("new").unwrap());
        assert!(app.unlock("new", "pw").is_ok());

        app.delete_vault("new").unwrap();
        assert!(!app.vault_exists("new").unwrap());
    }

    #[test]
    fn locking_rejects_path_traversal_names() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        // A hostile vault name must be rejected before any file is touched.
        assert!(app.unlock("../evil", "pw").is_err());
        assert!(app.create_vault("../evil", "pw").is_err());
    }

    #[test]
    fn attachments_and_backup_codes_survive_a_reveal_roundtrip() {
        use lilypad_core::Attachment;
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("v", "pw").unwrap();

        let mut secret = EntrySecret::new("pw");
        secret
            .attachments
            .push(Attachment::new("key.pem", "ZmlsZS1ib2R5"));
        add_entry(&app, &mut session, "srv", EntryMetadata::default(), &secret).unwrap();
        drop(session);

        // The attachment round-trips through seal/reveal (the zeroize-on-drop
        // must not corrupt the stored copy, only wipe the revealed one).
        let session = app.unlock("v", "pw").unwrap();
        let revealed = reveal_secret(&session, "srv").unwrap();
        assert_eq!(revealed.attachments.len(), 1);
        assert_eq!(revealed.attachments[0].filename, "key.pem");
    }

    #[test]
    fn change_master_never_weakens_kdf() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("v", "old").unwrap();
        let before = app.vault_kdf_memory("v");
        change_master_password(&app, &mut session, "new").unwrap();
        let after = app.vault_kdf_memory("v");
        assert!(
            after >= before,
            "KDF memory must not decrease on password change: {before} -> {after}"
        );
    }

    #[test]
    fn sync_status_distinguishes_ahead_from_conflict() {
        use crate::sync::{status, SyncState, SyncStatusView};
        let mut state = SyncState {
            device_id: "dev".to_string(),
            last_sync_sha: Some("sha-1".to_string()),
            last_local_checksum: Some("chk-1".to_string()),
        };
        // Nothing changed since baseline.
        assert_eq!(
            status(&state, Some("chk-1"), Some("sha-1")),
            SyncStatusView::InSync
        );
        // Only local moved.
        assert_eq!(
            status(&state, Some("chk-2"), Some("sha-1")),
            SyncStatusView::LocalAhead
        );
        // Only remote moved.
        assert_eq!(
            status(&state, Some("chk-1"), Some("sha-2")),
            SyncStatusView::RemoteAhead
        );
        // Both moved.
        assert_eq!(
            status(&state, Some("chk-2"), Some("sha-2")),
            SyncStatusView::Conflict
        );
        // No remote at all.
        assert_eq!(
            status(&state, Some("chk-1"), None),
            SyncStatusView::NoRemote
        );
        // Local vault deleted but a remote exists: recovery is to pull.
        assert_eq!(
            status(&state, None, Some("sha-1")),
            SyncStatusView::RemoteAhead
        );
        // No baseline yet.
        state.last_local_checksum = None;
        assert_eq!(
            status(&state, Some("chk-1"), Some("sha-1")),
            SyncStatusView::NoBaseline
        );
    }

    #[test]
    fn sync_state_persists_stable_device_id() {
        let dir = TempDir::new().unwrap();
        let sync_dir = dir.path().join("sync");
        let s1 = SyncState::load_or_init(&sync_dir, "v", "device-abc").unwrap();
        assert_eq!(s1.device_id, "device-abc");
        // A second load ignores the new seed and keeps the persisted id.
        let s2 = SyncState::load_or_init(&sync_dir, "v", "device-xyz").unwrap();
        assert_eq!(s2.device_id, "device-abc");
    }

    #[test]
    fn apply_remote_validated_pull_replaces_only_after_validation() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);

        // Two independent vaults, each with distinct content and its own master
        // password, both self-contained (V2).
        let mut a = app.create_vault("device_a", "pw-A").unwrap();
        add_entry(
            &app,
            &mut a,
            "from-a",
            EntryMetadata::default(),
            &EntrySecret::new("secretA"),
        )
        .unwrap();

        // Build a legitimate payload for "device_a" (as if pulled from remote).
        let good_payload = app.store().sync_payload("device_a").unwrap();
        drop(a);

        // A separate local vault we will pull INTO.
        let mut b = app.create_vault("target", "pw-B").unwrap();
        add_entry(
            &app,
            &mut b,
            "keep-b",
            EntryMetadata::default(),
            &EntrySecret::new("secretB"),
        )
        .unwrap();
        drop(b);

        // Garbage remote is rejected; local "target" is untouched.
        assert!(crate::sync::apply_remote(&app, "target", "pw-B", b"garbage").is_err());
        assert!(app.unlock("target", "pw-B").is_ok());

        // A real payload sealed under pw-A, applied to "target" with the correct
        // password for THAT payload (pw-A), is accepted and openable afterwards -
        // because the key is resolved from the candidate's own embedded params.
        crate::sync::apply_remote(&app, "target", "pw-A", &good_payload).unwrap();
        let session = app.unlock("target", "pw-A").unwrap();
        let labels: Vec<_> = session
            .vault()
            .entries
            .iter()
            .map(|e| e.label.clone())
            .collect();
        assert_eq!(labels, vec!["from-a".to_string()]);
    }

    #[test]
    fn favorite_color_tags_expiry_persist() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("v", "pw").unwrap();
        add_entry(
            &app,
            &mut session,
            "gh",
            EntryMetadata::default(),
            &EntrySecret::new("p"),
        )
        .unwrap();

        set_favorite(&app, &mut session, "gh", true).unwrap();
        set_color(&app, &mut session, "gh", Some(EntryColor::Blue)).unwrap();
        add_tag(&app, &mut session, "gh", "work").unwrap();
        add_tag(&app, &mut session, "gh", "dev").unwrap();
        remove_tag(&app, &mut session, "gh", "dev").unwrap();
        set_expiry_days(&app, &mut session, "gh", 30).unwrap();
        drop(session);

        // All of it survives a relock (persisted through the locked RMW).
        let session = app.unlock("v", "pw").unwrap();
        let views = list_entries(&session);
        assert_eq!(views.len(), 1);
        assert!(views[0].is_favorite);
        assert_eq!(views[0].tags, vec!["work".to_string()]);
        let entry = session.vault().find_entry("gh").unwrap();
        assert_eq!(entry.color, Some(EntryColor::Blue));
        assert!(entry.password_expires_at > 0);
    }

    #[test]
    fn settings_round_trip_with_defaults() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);

        // Defaults when nothing is saved.
        let s = app.load_settings();
        assert_eq!(s.auto_lock_minutes, 5);
        assert_eq!(s.clipboard_clear_secs, 20);

        let mut s = s;
        s.theme = 7;
        s.density = 2;
        s.auto_lock_minutes = 15;
        s.active_vault = Some("v".to_string());
        app.save_settings(&s).unwrap();

        let loaded = app.load_settings();
        assert_eq!(loaded.theme, 7);
        assert_eq!(loaded.density, 2);
        assert_eq!(loaded.auto_lock_minutes, 15);
        assert_eq!(loaded.active_vault.as_deref(), Some("v"));
    }

    #[test]
    fn edit_entry_preserves_untouched_fields_and_records_history() {
        use lilypad_core::{Attachment, CustomField, TotpBackupCode};
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("v", "pw").unwrap();

        // A rich entry: fields the edit form never renders.
        let mut secret = EntrySecret::new("original-pw");
        secret.email = Some("me@example.com".to_string());
        secret.phone = Some("+15551234567".to_string());
        secret
            .attachments
            .push(Attachment::new("key.pem", "ZmlsZQ=="));
        secret.custom_fields.push(CustomField::new("PIN", "1234"));
        secret.totp_backup_codes = vec![TotpBackupCode::new("ABCD2345")];
        let meta = EntryMetadata {
            username: Some("old-user".to_string()),
            tags: vec!["work".to_string()],
            folder: Some("Personal/Bank".to_string()),
            entry_type: EntryType::Login,
            ..EntryMetadata::default()
        };
        add_entry(&app, &mut session, "acct", meta, &secret).unwrap();

        // Edit only the form-visible fields.
        edit_entry(
            &app,
            &mut session,
            "acct",
            EntryEdit {
                new_label: "acct".to_string(),
                username: Some("new-user".to_string()),
                url: Some("https://example.com".to_string()),
                entry_type: EntryType::Card,
                password: "new-pw".to_string(),
                notes: Some("a note".to_string()),
                totp_secret: None,
                tags: None,
                folder: None,
            },
        )
        .unwrap();
        drop(session);

        // Reopen and assert nothing was lost.
        let session = app.unlock("v", "pw").unwrap();
        let revealed = reveal_secret(&session, "acct").unwrap();
        assert_eq!(revealed.password, "new-pw");
        assert_eq!(revealed.email.as_deref(), Some("me@example.com"));
        assert_eq!(revealed.phone.as_deref(), Some("+15551234567"));
        assert_eq!(revealed.attachments.len(), 1);
        assert_eq!(revealed.attachments[0].filename, "key.pem");
        assert_eq!(revealed.custom_fields.len(), 1);
        assert_eq!(revealed.totp_backup_codes.len(), 1);

        let entry = session.vault().find_entry("acct").unwrap();
        assert_eq!(entry.metadata.username.as_deref(), Some("new-user"));
        assert_eq!(entry.metadata.tags, vec!["work".to_string()]);
        assert_eq!(entry.metadata.folder.as_deref(), Some("Personal/Bank"));
        assert_eq!(entry.metadata.entry_type, EntryType::Card);
        // The password change was recorded in history with the old ciphertext.
        assert!(entry.history.iter().any(|h| matches!(
            h.change_type,
            lilypad_core::EntryChangeType::PasswordChanged
        ) && h.previous_ciphertext.is_some()));
    }

    #[test]
    fn edit_entry_tag_and_folder_overrides() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("v", "pw").unwrap();
        let meta = EntryMetadata {
            tags: vec!["old".to_string()],
            folder: Some("OldFolder".to_string()),
            ..EntryMetadata::default()
        };
        add_entry(&app, &mut session, "gh", meta, &EntrySecret::new("p")).unwrap();

        let base_edit = |tags, folder| EntryEdit {
            new_label: "gh".to_string(),
            username: None,
            url: None,
            entry_type: EntryType::Login,
            password: "p".to_string(),
            notes: None,
            totp_secret: None,
            tags,
            folder,
        };

        // None preserves.
        edit_entry(&app, &mut session, "gh", base_edit(None, None)).unwrap();
        let e = session.vault().find_entry("gh").unwrap();
        assert_eq!(e.metadata.tags, vec!["old".to_string()]);
        assert_eq!(e.metadata.folder.as_deref(), Some("OldFolder"));

        // Some replaces; Some(None) clears the folder.
        edit_entry(
            &app,
            &mut session,
            "gh",
            base_edit(Some(vec!["new".to_string()]), Some(None)),
        )
        .unwrap();
        let e = session.vault().find_entry("gh").unwrap();
        assert_eq!(e.metadata.tags, vec!["new".to_string()]);
        assert!(e.metadata.folder.is_none());
    }

    #[test]
    fn trash_lifecycle_records_audit_events_and_enforces_state() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("v", "pw").unwrap();
        add_entry(
            &app,
            &mut session,
            "gh",
            EntryMetadata::default(),
            &EntrySecret::new("p"),
        )
        .unwrap();

        // Restoring a live entry is an error, not a false success.
        assert!(restore(&app, &mut session, "gh").is_err());

        soft_delete(&app, &mut session, "gh").unwrap();
        // Double-trash is rejected.
        assert!(soft_delete(&app, &mut session, "gh").is_err());
        restore(&app, &mut session, "gh").unwrap();

        // Edits are audited too.
        edit_entry(
            &app,
            &mut session,
            "gh",
            EntryEdit {
                new_label: "gh".to_string(),
                username: None,
                url: None,
                entry_type: EntryType::Login,
                password: "p2".to_string(),
                notes: None,
                totp_secret: None,
                tags: None,
                folder: None,
            },
        )
        .unwrap();

        let actions: Vec<String> = audit_log(&session).into_iter().map(|e| e.action).collect();
        assert!(actions.iter().any(|a| a == "entry_trashed"));
        assert!(actions.iter().any(|a| a == "entry_restored"));
        assert!(actions.iter().any(|a| a == "entry_updated"));
    }

    #[test]
    fn health_report_ignores_trashed_entries() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("v", "pw").unwrap();
        // Two entries sharing a weak password -> reuse + weak findings.
        add_entry(
            &app,
            &mut session,
            "a",
            EntryMetadata::default(),
            &EntrySecret::new("weak"),
        )
        .unwrap();
        add_entry(
            &app,
            &mut session,
            "b",
            EntryMetadata::default(),
            &EntrySecret::new("weak"),
        )
        .unwrap();
        let before = vault_health(&session);
        assert!(!before.issues.is_empty());

        // Trashing one of them must clear the reuse finding (soft delete is the
        // default delete; findings must be dismissable by deleting).
        soft_delete(&app, &mut session, "b").unwrap();
        let after = vault_health(&session);
        let reuse_after = after
            .issues
            .iter()
            .filter(|i| i.affected_entries.iter().any(|e| e == "b"))
            .count();
        assert_eq!(
            reuse_after, 0,
            "trashed entries must not appear in findings"
        );
    }

    #[test]
    fn add_and_delete_entry_persist() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("v", "pw").unwrap();
        add_entry(
            &app,
            &mut session,
            "one",
            EntryMetadata::default(),
            &EntrySecret::new("p1"),
        )
        .unwrap();
        add_entry(
            &app,
            &mut session,
            "two",
            EntryMetadata::default(),
            &EntrySecret::new("p2"),
        )
        .unwrap();
        assert_eq!(list_entries(&session).len(), 2);

        delete_entry(&app, &mut session, "one").unwrap();
        assert_eq!(list_entries(&session).len(), 1);
        drop(session);

        // Persisted across relock.
        let session = app.unlock("v", "pw").unwrap();
        let views = list_entries(&session);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].label, "two");
    }

    // ==================== entry-level sync merge ====================

    /// Shifts an entry's `updated_at` on disk, to build deterministic
    /// merge scenarios (the real clock has 1-second granularity, so two test
    /// steps otherwise collide on the same timestamp).
    fn shift_entry_timestamp(app: &App, vault: &str, password: &str, label: &str, delta: i64) {
        let key = app.derive_unlock_key(vault, password).unwrap();
        let mut v = app.store().load_vault(vault, &key).unwrap();
        let e = v.entries.iter_mut().find(|e| e.label == label).unwrap();
        e.updated_at = (e.updated_at as i64 + delta) as u64;
        app.store().save_vault(&v, &key).unwrap();
    }

    /// Two replicas of the same vault (same master password, different KDF
    /// salts), each holding one extra entry.
    fn two_replicas(dir_a: &TempDir, dir_b: &TempDir, pw: &str) -> (App, App) {
        let app_a = test_app(dir_a);
        let app_b = test_app(dir_b);
        let mut sa = app_a.create_vault("v", pw).unwrap();
        let mut sb = app_b.create_vault("v", pw).unwrap();
        add_entry(
            &app_a,
            &mut sa,
            "only-a",
            EntryMetadata::default(),
            &EntrySecret::new("a-secret"),
        )
        .unwrap();
        add_entry(
            &app_b,
            &mut sb,
            "only-b",
            EntryMetadata::default(),
            &EntrySecret::new("b-secret"),
        )
        .unwrap();
        (app_a, app_b)
    }

    #[test]
    fn merge_remote_adds_and_reseals_cross_key_entries() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        let pw = "same master password";
        let (app_a, app_b) = two_replicas(&dir_a, &dir_b, pw);

        let bytes = app_b.store().sync_payload("v").unwrap();
        let (report, _) = sync::merge_remote(&app_a, "v", pw, &bytes).unwrap();

        assert_eq!(report.added_from_remote, vec!["only-b".to_string()]);
        assert_eq!(report.local_only, vec!["only-a".to_string()]);
        assert!(report.remote_is_stale());

        // The merged entry must decrypt under A's key even though B sealed it
        // under a different KDF salt: this is the cross-key reseal working.
        let session = app_a.unlock("v", pw).unwrap();
        assert_eq!(
            reveal_secret(&session, "only-b").unwrap().password,
            "b-secret"
        );
        assert_eq!(
            reveal_secret(&session, "only-a").unwrap().password,
            "a-secret"
        );
    }

    #[test]
    fn merge_remote_rejects_wrong_password_without_touching_the_vault() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        let pw = "same master password";
        let (app_a, app_b) = two_replicas(&dir_a, &dir_b, pw);

        let bytes = app_b.store().sync_payload("v").unwrap();
        assert!(sync::merge_remote(&app_a, "v", "wrong password", &bytes).is_err());

        let session = app_a.unlock("v", pw).unwrap();
        let views = list_entries(&session);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].label, "only-a");
    }

    #[test]
    fn merge_remote_propagates_permanent_deletion() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        let pw = "same master password";
        let (app_a, app_b) = two_replicas(&dir_a, &dir_b, pw);

        // B creates "doomed"; a first merge replicates it to A (same entry
        // id on both sides - tombstones are id-pinned, so deletion only
        // propagates between true replicas of the entry).
        let mut sb = app_b.unlock("v", pw).unwrap();
        add_entry(
            &app_b,
            &mut sb,
            "doomed",
            EntryMetadata::default(),
            &EntrySecret::new("x"),
        )
        .unwrap();
        let bytes = app_b.store().sync_payload("v").unwrap();
        sync::merge_remote(&app_a, "v", pw, &bytes).unwrap();

        // B then permanently deletes it; A's replica is strictly older.
        delete_entry(&app_b, &mut sb, "doomed").unwrap();
        drop(sb);
        shift_entry_timestamp(&app_a, "v", pw, "doomed", -10);

        let bytes = app_b.store().sync_payload("v").unwrap();
        let (report, _) = sync::merge_remote(&app_a, "v", pw, &bytes).unwrap();

        assert_eq!(report.removed_by_tombstone, vec!["doomed".to_string()]);
        let session = app_a.unlock("v", pw).unwrap();
        assert!(list_entries(&session).iter().all(|e| e.label != "doomed"));
        assert!(list_trash(&session).is_empty());
    }

    #[test]
    fn merge_remote_does_not_resurrect_locally_deleted_entry() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        let pw = "same master password";
        let (app_a, app_b) = two_replicas(&dir_a, &dir_b, pw);

        // B creates "zombie"; a first merge replicates it to A (shared id).
        let mut sb = app_b.unlock("v", pw).unwrap();
        add_entry(
            &app_b,
            &mut sb,
            "zombie",
            EntryMetadata::default(),
            &EntrySecret::new("x"),
        )
        .unwrap();
        drop(sb);
        let bytes = app_b.store().sync_payload("v").unwrap();
        sync::merge_remote(&app_a, "v", pw, &bytes).unwrap();

        // B's live copy becomes strictly older than A's deletion of it.
        shift_entry_timestamp(&app_b, "v", pw, "zombie", -10);
        let mut sa = app_a.unlock("v", pw).unwrap();
        delete_entry(&app_a, &mut sa, "zombie").unwrap();
        drop(sa);

        let bytes = app_b.store().sync_payload("v").unwrap();
        let (report, _) = sync::merge_remote(&app_a, "v", pw, &bytes).unwrap();

        assert_eq!(report.suppressed_by_tombstone, vec!["zombie".to_string()]);
        // The remote still holds the live copy: the deletion must be pushed.
        assert!(report.remote_is_stale());
        let session = app_a.unlock("v", pw).unwrap();
        assert!(list_entries(&session).iter().all(|e| e.label != "zombie"));
    }

    #[test]
    fn merge_remote_resolves_same_second_conflicting_edits_deterministically() {
        // Both replicas hold the SAME entry (replicated by a first merge),
        // then each side's copy ends up with the same updated_at but a
        // DIFFERENT secret - the divergence plain LWW can never see. The
        // merge must detect it, pick a deterministic winner, and converge.
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        let pw = "same master password";
        let (app_a, app_b) = two_replicas(&dir_a, &dir_b, pw);

        let mut sb = app_b.unlock("v", pw).unwrap();
        add_entry(
            &app_b,
            &mut sb,
            "contested",
            EntryMetadata::default(),
            &EntrySecret::new("version-B"),
        )
        .unwrap();
        drop(sb);
        let bytes = app_b.store().sync_payload("v").unwrap();
        sync::merge_remote(&app_a, "v", pw, &bytes).unwrap();

        // A edits its replica, then both copies are pinned to the same
        // updated_at: a same-second concurrent edit.
        let mut sa = app_a.unlock("v", pw).unwrap();
        update_secret(&app_a, &mut sa, "contested", &EntrySecret::new("version-A")).unwrap();
        drop(sa);
        let tied = {
            let key = app_a.derive_unlock_key("v", pw).unwrap();
            let v = app_a.store().load_vault("v", &key).unwrap();
            v.entries
                .iter()
                .find(|e| e.label == "contested")
                .unwrap()
                .updated_at
        };
        let set_ts = |app: &App, ts: u64| {
            let key = app.derive_unlock_key("v", pw).unwrap();
            let mut v = app.store().load_vault("v", &key).unwrap();
            v.entries
                .iter_mut()
                .find(|e| e.label == "contested")
                .unwrap()
                .updated_at = ts;
            app.store().save_vault(&v, &key).unwrap();
        };
        set_ts(&app_a, tied);
        set_ts(&app_b, tied);

        let bytes_b = app_b.store().sync_payload("v").unwrap();
        let (report_a, _) = sync::merge_remote(&app_a, "v", pw, &bytes_b).unwrap();

        // The conflict must be DETECTED (one side wins), not silently ignored.
        let a_won = !report_a.tie_conflicts_local_won.is_empty();
        let b_won = !report_a.tie_conflicts_remote_won.is_empty();
        assert!(a_won ^ b_won, "tie conflict must be resolved exactly once");
        // A merge in the OTHER direction must pick the SAME winner (that is
        // what makes the fleet converge instead of ping-ponging).
        let bytes_a = app_a.store().sync_payload("v").unwrap();
        let winner_a = reveal_secret(&app_a.unlock("v", pw).unwrap(), "contested")
            .unwrap()
            .password
            .clone();
        sync::merge_remote(&app_b, "v", pw, &bytes_a).unwrap();
        let winner_b = reveal_secret(&app_b.unlock("v", pw).unwrap(), "contested")
            .unwrap()
            .password
            .clone();
        assert_eq!(winner_a, winner_b, "replicas must converge on one version");
    }

    #[test]
    fn merge_remote_survives_undecryptable_history_blob() {
        // A stale password-history blob (sealed under a key that no longer
        // exists, residue of an old change_master_password) must not brick
        // the merge: the entry itself merges, the blob is carried unchanged.
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        let pw = "same master password";
        let (app_a, app_b) = two_replicas(&dir_a, &dir_b, pw);

        // Corrupt B's "only-b" history: give it a previous_ciphertext that
        // decrypts with no known key.
        {
            let key = app_b.derive_unlock_key("v", pw).unwrap();
            let mut v = app_b.store().load_vault("v", &key).unwrap();
            let entry = v.entries.iter_mut().find(|e| e.label == "only-b").unwrap();
            let alien = lilypad_core::KeyMaterial::generate();
            let blob = lilypad_core::encrypt(&alien, b"old-password").unwrap();
            entry.history.push(lilypad_core::EntryHistoryRecord {
                timestamp: 1,
                change_type: lilypad_core::EntryChangeType::PasswordChanged,
                description: None,
                previous_ciphertext: Some(blob),
            });
            app_b.store().save_vault(&v, &key).unwrap();
        }

        let bytes = app_b.store().sync_payload("v").unwrap();
        let (report, _) = sync::merge_remote(&app_a, "v", pw, &bytes).unwrap();
        assert_eq!(report.added_from_remote, vec!["only-b".to_string()]);

        // The live secret still decrypts on A.
        let session = app_a.unlock("v", pw).unwrap();
        assert_eq!(
            reveal_secret(&session, "only-b").unwrap().password,
            "b-secret"
        );
    }

    #[test]
    fn merge_remote_keeps_newer_local_version() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        let pw = "same master password";
        let (app_a, app_b) = two_replicas(&dir_a, &dir_b, pw);

        let mut sa = app_a.unlock("v", pw).unwrap();
        let mut sb = app_b.unlock("v", pw).unwrap();
        add_entry(
            &app_a,
            &mut sa,
            "shared",
            EntryMetadata::default(),
            &EntrySecret::new("newer-local"),
        )
        .unwrap();
        add_entry(
            &app_b,
            &mut sb,
            "shared",
            EntryMetadata::default(),
            &EntrySecret::new("older-remote"),
        )
        .unwrap();
        drop(sa);
        drop(sb);
        shift_entry_timestamp(&app_b, "v", pw, "shared", -10);

        let bytes = app_b.store().sync_payload("v").unwrap();
        let (report, _) = sync::merge_remote(&app_a, "v", pw, &bytes).unwrap();

        assert_eq!(report.kept_local, vec!["shared".to_string()]);
        assert!(report.remote_is_stale());
        let session = app_a.unlock("v", pw).unwrap();
        assert_eq!(
            reveal_secret(&session, "shared").unwrap().password,
            "newer-local"
        );
    }
}
