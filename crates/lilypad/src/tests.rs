//! State-machine tests of the desktop app.

use crate::message::{Filter, FormField, Message};
use crate::state::LilypadApp;
use crate::theme::{LilypadTheme, UiVariation};
use lilypad_core::{EntryMetadata, EntrySecret, EntryType};
use tempfile::TempDir;

fn app_in(dir: &TempDir) -> LilypadApp {
    LilypadApp::with_data_dir(Some(dir.path().to_path_buf())).0
}

fn seed(app: &LilypadApp, name: &str, password: &str, entry: &str) {
    let mut session = app.service.create_vault(name, password).unwrap();
    let secret = EntrySecret::new(entry.to_string());
    lilypad_core::add_entry(
        &app.service,
        &mut session,
        entry,
        EntryMetadata::default(),
        &secret,
    )
    .unwrap();
}

#[test]
fn key_derived_opens_vault_and_lists_entries() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed(&app, "v", "pw", "gh");
    let key = app.service.derive_unlock_key("v", "pw").unwrap();
    app.pending_vault = "v".to_string();
    let _ = app.update(Message::KeyDerived(Ok(key)));
    assert!(app.session.is_some());
    assert_eq!(app.entries.len(), 1);
}

#[test]
fn wrong_key_does_not_open_or_destroy_vault() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed(&app, "v", "right", "gh");
    let wrong = app.service.derive_unlock_key("v", "wrong").unwrap();
    app.pending_vault = "v".to_string();
    let _ = app.update(Message::KeyDerived(Ok(wrong)));
    assert!(app.session.is_none());
    let right = app.service.derive_unlock_key("v", "right").unwrap();
    let _ = app.update(Message::KeyDerived(Ok(right)));
    assert!(app.session.is_some());
    assert_eq!(app.entries.len(), 1);
}

#[test]
fn favorite_toggle_and_filter() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed(&app, "v", "pw", "gh");
    let key = app.service.derive_unlock_key("v", "pw").unwrap();
    app.pending_vault = "v".to_string();
    let _ = app.update(Message::KeyDerived(Ok(key)));

    let _ = app.update(Message::ToggleFavorite("gh".to_string()));
    assert!(
        app.entries
            .iter()
            .find(|e| e.label == "gh")
            .unwrap()
            .is_favorite
    );

    app.filter = Filter::Favorites;
    assert_eq!(app.visible().len(), 1);
    app.filter = Filter::Type(EntryType::Card);
    assert_eq!(app.visible().len(), 0);
}

#[test]
fn theme_and_density_persist() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    let _ = app.update(Message::ThemeSelected(LilypadTheme::Dracula));
    let _ = app.update(Message::DensitySelected(UiVariation::Compact));
    // A fresh app reads the persisted settings.
    let app2 = app_in(&dir);
    assert_eq!(app2.theme, LilypadTheme::Dracula);
    assert_eq!(app2.density, UiVariation::Compact);
}

#[test]
fn settings_changes_persist_across_restart() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    let _ = app.update(Message::SetAutoLock(15));
    let _ = app.update(Message::SetClipboardSecs(45));
    let _ = app.update(Message::SetDefaultFilter(1));
    let _ = app.update(Message::ToggleRestoreLastVault); // true -> false
    let app2 = app_in(&dir);
    assert_eq!(app2.auto_lock_minutes, 15);
    assert_eq!(app2.clipboard_secs, 45);
    assert_eq!(app2.default_filter, 1);
    assert!(!app2.restore_last_vault);
}

#[test]
fn settings_page_open_and_section_toggle() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    assert!(!app.settings_open);
    let _ = app.update(Message::OpenSettings);
    assert!(app.settings_open);
    let _ = app.update(Message::SettingsCategory(2));
    assert_eq!(app.settings_category, 2);
    // A section that is collapsed by default toggles on, then off.
    assert!(!app.settings_sections.contains("master"));
    let _ = app.update(Message::SettingsToggleSection("master".into()));
    assert!(app.settings_sections.contains("master"));
    let _ = app.update(Message::SettingsToggleSection("master".into()));
    assert!(!app.settings_sections.contains("master"));
    let _ = app.update(Message::CloseSettings);
    assert!(!app.settings_open);
}

#[test]
fn form_saves_tags_folder_type_and_expiry() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed(&app, "v", "pw", "gh");
    let key = app.service.derive_unlock_key("v", "pw").unwrap();
    app.pending_vault = "v".to_string();
    let _ = app.update(Message::KeyDerived(Ok(key)));

    // New entry with every form field set.
    let _ = app.update(Message::ShowAddForm);
    let _ = app.update(Message::FormChanged(FormField::Label, "bank".into()));
    let _ = app.update(Message::FormChanged(FormField::Password, "pw2".into()));
    let _ = app.update(Message::FormChanged(FormField::Tags, "money, perso".into()));
    let _ = app.update(Message::FormChanged(FormField::Folder, "Finance".into()));
    let _ = app.update(Message::FormTypeSelected("Cards".into()));
    let _ = app.update(Message::FormExpirySelected("90 days".into()));
    let _ = app.update(Message::FormSave);

    let view = app.entries.iter().find(|e| e.label == "bank").unwrap();
    assert_eq!(view.tags, vec!["money".to_string(), "perso".to_string()]);
    assert_eq!(view.folder.as_deref(), Some("Finance"));
    assert_eq!(view.entry_type, EntryType::Card);
    assert!(view.password_expires_at > 0, "expiry must be set");

    // Editing with cleared tags persists the clearing.
    app.selected = Some("bank".to_string());
    app.reveal_selected();
    let _ = app.update(Message::StartEdit);
    assert_eq!(app.form.tags, "money, perso");
    let _ = app.update(Message::FormChanged(FormField::Tags, "".into()));
    let _ = app.update(Message::FormSave);
    let view = app.entries.iter().find(|e| e.label == "bank").unwrap();
    assert!(view.tags.is_empty());
    assert_eq!(view.folder.as_deref(), Some("Finance"), "folder untouched");
}

#[test]
fn sync_login_done_records_the_username() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    app.sync_busy = true;
    let _ = app.update(Message::SyncLoginDone(Ok("lin".to_string())));
    assert_eq!(app.authed.as_deref(), Some("lin"));
    assert!(!app.sync_busy);

    let _ = app.update(Message::SyncLoginDone(Err("denied".to_string())));
    // A failed login clears the pending flow but keeps prior auth state.
    assert!(app.sync_login.is_none());
}

#[test]
fn pull_locks_the_session_before_the_transfer_starts() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed(&app, "v", "pw", "gh");
    let key = app.service.derive_unlock_key("v", "pw").unwrap();
    app.pending_vault = "v".to_string();
    let _ = app.update(Message::KeyDerived(Ok(key)));
    assert!(app.session.is_some());

    // Dispatching SyncPull must lock IMMEDIATELY (before the network task
    // runs), closing the window where an edit could race the file swap.
    app.sync_pull_password = "pw".to_string();
    let _task = app.update(Message::SyncPull);
    assert!(app.session.is_none(), "must lock before the pull transfer");
    assert!(app.sync_busy);
    assert!(app.sync_pull_password.is_empty());
}

#[test]
fn successful_pull_locks_the_session_and_wipes_the_password() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed(&app, "v", "pw", "gh");
    let key = app.service.derive_unlock_key("v", "pw").unwrap();
    app.pending_vault = "v".to_string();
    let _ = app.update(Message::KeyDerived(Ok(key)));
    assert!(app.session.is_some());

    app.sync_pull_password = "pw".to_string();
    let _ = app.update(Message::SyncPullDone(Ok(
        lilypad_core::sync::net::PullOutcome::Applied,
    )));
    // The on-disk vault may have been re-keyed: the session must be locked
    // and the pull password wiped.
    assert!(app.session.is_none());
    assert!(app.sync_pull_password.is_empty());
    assert_eq!(app.sync_status, Some(lilypad_core::SyncStatusView::InSync));
}

#[test]
fn sync_merge_locks_before_running_and_lands_in_sync() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed(&app, "v", "pw", "gh");
    let key = app.service.derive_unlock_key("v", "pw").unwrap();
    app.pending_vault = "v".to_string();
    let _ = app.update(Message::KeyDerived(Ok(key)));
    assert!(app.session.is_some());

    // Dispatching SyncMerge must lock IMMEDIATELY (before the network task
    // runs): the merge rewrites the vault file, and an edit landing in
    // that window would be silently clobbered.
    app.sync_pull_password = "pw".to_string();
    let _task = app.update(Message::SyncMerge);
    assert!(app.session.is_none(), "must lock before the merge runs");
    assert!(app.sync_busy);
    assert!(app.sync_pull_password.is_empty());

    // A successful merge for the active vault lands on InSync.
    let _ = app.update(Message::SyncMergeDone(Ok((
        "v".to_string(),
        Some(("1 added from remote".to_string(), true)),
    ))));
    assert!(!app.sync_busy);
    assert_eq!(app.sync_status, Some(lilypad_core::SyncStatusView::InSync));

    // A stale completion for a DIFFERENT vault must not touch the status.
    app.sync_status = None;
    let _ = app.update(Message::SyncMergeDone(Ok((
        "other".to_string(),
        Some(("1 added from remote".to_string(), false)),
    ))));
    assert_eq!(app.sync_status, None);
}

#[test]
fn unlock_is_blocked_while_a_sync_runs() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed(&app, "v", "pw", "gh");
    app.selected_vault = Some("v".to_string());
    app.sync_busy = true;

    // The unlock button is inert during a sync: the file is being
    // rewritten and a fresh session would race it.
    app.password = "pw".to_string();
    let _ = app.update(Message::UnlockPressed);
    assert!(!app.unlocking, "unlock must not start during a sync");
    assert!(app.session.is_none());

    // A key whose derivation was already in flight when the sync started
    // is discarded rather than opening a stale session.
    let key = app.service.derive_unlock_key("v", "pw").unwrap();
    app.pending_vault = "v".to_string();
    let _ = app.update(Message::KeyDerived(Ok(key)));
    assert!(app.session.is_none());
    assert!(app.password.is_empty());
}

#[test]
fn merge_completion_for_another_vault_leaves_session_alone() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed(&app, "w", "pw", "gh");
    let key = app.service.derive_unlock_key("w", "pw").unwrap();
    app.pending_vault = "w".to_string();
    let _ = app.update(Message::KeyDerived(Ok(key)));
    assert!(app.session.is_some());

    // A merge that finished for vault "v" must not lock the session that
    // is open on vault "w" - the merge never touched that file.
    let _ = app.update(Message::SyncMergeDone(Ok((
        "v".to_string(),
        Some(("1 added from remote".to_string(), false)),
    ))));
    assert!(
        app.session.is_some(),
        "a session on an untouched vault must not be locked"
    );
}

#[test]
fn auto_lock_setting_applies_to_live_session() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed(&app, "v", "pw", "gh");
    let key = app.service.derive_unlock_key("v", "pw").unwrap();
    app.pending_vault = "v".to_string();
    let _ = app.update(Message::KeyDerived(Ok(key)));
    assert!(app.session.is_some());
    // Disabling auto-lock clears the live session's deadline immediately.
    let _ = app.update(Message::SetAutoLock(0));
    assert!(app.session.as_ref().unwrap().time_until_lock().is_none());
    // A positive value re-arms it.
    let _ = app.update(Message::SetAutoLock(5));
    assert!(app.session.as_ref().unwrap().time_until_lock().is_some());
}
