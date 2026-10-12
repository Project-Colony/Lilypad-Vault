//! These exercise the TUI's logic (unlock, entry refresh, add) directly,
//! bypassing crossterm/the terminal, so the data-loss-critical unlock path
//! is verified deterministically.
use crate::app::{App, FormData, Screen};
use crossterm::event::{self, KeyCode, KeyModifiers};
use lilypad_core::{EntryMetadata, EntrySecret};
use tempfile::TempDir;

fn app_in(dir: &TempDir) -> App {
    App::new(Some(dir.path().to_path_buf())).expect("open app")
}

fn seed_vault(app: &mut App, name: &str, password: &str, entry: &str) {
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
    app.vaults = app.service.list_vaults().unwrap();
    app.vault_list.select(Some(0));
}

#[test]
fn wrong_password_does_not_unlock_or_destroy_the_vault() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed_vault(&mut app, "v", "right", "gh");

    app.master_password = "wrong".into();
    app.try_unlock();
    assert!(app.session.is_none(), "wrong password must not unlock");

    // The vault is intact: the correct password still opens it with its entry.
    app.master_password = "right".into();
    app.try_unlock();
    assert!(app.session.is_some(), "correct password must unlock");
    app.refresh_entries();
    assert_eq!(app.entries.len(), 1);
    assert_eq!(app.entries[0].label, "gh");
}

#[test]
fn unlock_populates_entries_and_lock_clears_session() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed_vault(&mut app, "v", "pw", "gh");

    app.master_password = "pw".into();
    app.try_unlock();
    assert!(app.screen == Screen::Unlocked);
    assert_eq!(app.entries.len(), 1);

    app.lock();
    assert!(app.session.is_none());
    assert!(app.entries.is_empty());
    assert!(app.screen == Screen::Locked);
}

#[test]
fn save_form_adds_a_new_entry() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed_vault(&mut app, "v", "pw", "gh");
    app.master_password = "pw".into();
    app.try_unlock();

    let mut form = FormData::default();
    form.label = "email".into();
    form.username = "me@example.com".into();
    form.password = "p2".into();
    app.form = form;
    app.save_form();
    assert_eq!(app.entries.len(), 2);
    assert!(app.entries.iter().any(|e| e.label == "email"));
}

#[test]
fn editing_an_entry_preserves_its_attachment() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    // Seed a vault with an entry that carries an attachment (a field the
    // form never renders).
    let mut session = app.service.create_vault("v", "pw").unwrap();
    let mut secret = EntrySecret::new("orig");
    secret
        .attachments
        .push(lilypad_core::Attachment::new("key.pem", "ZmlsZQ=="));
    lilypad_core::add_entry(
        &app.service,
        &mut session,
        "gh",
        EntryMetadata::default(),
        &secret,
    )
    .unwrap();
    drop(session);
    app.vaults = app.service.list_vaults().unwrap();
    app.vault_list.select(Some(0));
    app.master_password = "pw".into();
    app.try_unlock();

    // Edit the entry, changing only the password through the form.
    app.open_form(Some("gh".to_string()));
    app.form.password = "changed".into();
    app.save_form();

    let revealed = lilypad_core::reveal_secret(app.session.as_ref().unwrap(), "gh").unwrap();
    assert_eq!(revealed.password, "changed");
    assert_eq!(
        revealed.attachments.len(),
        1,
        "attachment must survive edit"
    );
}

#[test]
fn delete_moves_to_trash_then_restores() {
    let dir = TempDir::new().unwrap();
    let mut app = app_in(&dir);
    seed_vault(&mut app, "v", "pw", "gh");
    app.master_password = "pw".into();
    app.try_unlock();
    app.entry_list.select(Some(0));

    // 'y' in the confirm dialog soft-deletes (recoverable).
    app.key_confirm_delete(event::KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    assert_eq!(app.entries.len(), 0);
    assert_eq!(app.trashed.len(), 1);

    // Restore from Trash brings it back.
    app.trash_list.select(Some(0));
    app.key_trash(event::KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    assert_eq!(app.entries.len(), 1);
    assert_eq!(app.trashed.len(), 0);
}
