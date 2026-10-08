//! Lilypad TUI - a keyboard-driven terminal frontend.
//!
//! A thin client over `lilypad-app`: unlocking, entry access, and every write
//! go through the service layer. In particular unlocking uses `App::unlock`,
//! which only ever reads - so the original data-loss bug (a wrong master
//! password overwriting the vault with an empty one) is impossible here by
//! construction. The session carries an idle auto-lock policy.

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use lilypad_app::{App as Service, EntryMetadata, EntrySecret, EntryView, OpenOptions, Session};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

#[derive(PartialEq)]
enum Screen {
    Locked,
    CreateVault,
    Unlocked,
    View,
    Form,
    ConfirmDelete,
    Generate,
    Help,
    Trash,
    ConfirmPurge,
}

#[derive(Clone, Copy, PartialEq)]
enum FormField {
    Label,
    Username,
    Url,
    Password,
    Notes,
    Totp,
}

impl FormField {
    const ORDER: [FormField; 6] = [
        FormField::Label,
        FormField::Username,
        FormField::Url,
        FormField::Password,
        FormField::Notes,
        FormField::Totp,
    ];
    fn next(self) -> FormField {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        Self::ORDER[(i + 1) % Self::ORDER.len()]
    }
    fn prev(self) -> FormField {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        Self::ORDER[(i + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
    fn label(self) -> &'static str {
        match self {
            FormField::Label => "Label",
            FormField::Username => "Username",
            FormField::Url => "URL",
            FormField::Password => "Password",
            FormField::Notes => "Notes",
            FormField::Totp => "TOTP secret",
        }
    }
}

#[derive(Default)]
struct FormData {
    editing_original: Option<String>,
    label: String,
    username: String,
    url: String,
    password: String,
    notes: String,
    totp: String,
    focus_index: usize,
}

impl Drop for FormData {
    fn drop(&mut self) {
        self.password.zeroize();
        self.totp.zeroize();
        // Notes are EntrySecret material too (encrypted at rest).
        self.notes.zeroize();
    }
}

struct App {
    service: Service,
    session: Option<Session>,
    screen: Screen,

    vaults: Vec<String>,
    vault_list: ListState,
    master_password: String,

    // Create-vault form
    new_vault_name: String,
    new_vault_password: String,
    create_focus_name: bool,

    entries: Vec<EntryView>,
    entry_list: ListState,
    search_query: String,
    searching: bool,

    // Trash (soft-deleted entries)
    trashed: Vec<EntryView>,
    trash_list: ListState,

    // Idle/clipboard policy (from persisted settings)
    clipboard_clear_secs: u64,

    // View screen
    view_label: String,
    view_reveal: Option<lilypad_app::RevealedSecret>,
    view_show_password: bool,

    form: FormData,

    // Generator
    gen_length: usize,
    gen_upper: bool,
    gen_lower: bool,
    gen_digits: bool,
    gen_symbols: bool,
    gen_result: String,

    status: Option<String>,
    status_at: Option<Instant>,
    clipboard_clear_at: Option<Instant>,

    should_quit: bool,
}

impl Drop for App {
    fn drop(&mut self) {
        self.master_password.zeroize();
        self.new_vault_password.zeroize();
        self.gen_result.zeroize();
    }
}

impl App {
    fn new(data_dir: Option<std::path::PathBuf>) -> Result<Self> {
        // Load persisted settings so the TUI honors the same auto-lock and
        // clipboard policy as the other frontends (bootstrap open just reads).
        let bootstrap = Service::open(OpenOptions {
            data_dir: data_dir.clone(),
            auto_lock_after: None,
        })?;
        let settings = bootstrap.load_settings();
        let auto_lock = settings.auto_lock_duration();
        let clipboard_clear_secs = settings.clipboard_clear_secs.max(1);
        let service = Service::open(OpenOptions {
            data_dir,
            auto_lock_after: auto_lock,
        })?;
        let vaults = service.list_vaults().unwrap_or_default();
        let mut vault_list = ListState::default();
        if !vaults.is_empty() {
            vault_list.select(Some(0));
        }
        let screen = if vaults.is_empty() {
            Screen::CreateVault
        } else {
            Screen::Locked
        };
        Ok(Self {
            service,
            session: None,
            screen,
            vaults,
            vault_list,
            master_password: String::new(),
            new_vault_name: String::new(),
            new_vault_password: String::new(),
            create_focus_name: true,
            entries: Vec::new(),
            entry_list: ListState::default(),
            search_query: String::new(),
            searching: false,
            trashed: Vec::new(),
            trash_list: ListState::default(),
            clipboard_clear_secs,
            view_label: String::new(),
            view_reveal: None,
            view_show_password: false,
            form: FormData::default(),
            gen_length: 20,
            gen_upper: true,
            gen_lower: true,
            gen_digits: true,
            gen_symbols: true,
            gen_result: String::new(),
            status: None,
            status_at: None,
            clipboard_clear_at: None,
            should_quit: false,
        })
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some(msg.into());
        self.status_at = Some(Instant::now());
    }

    fn selected_vault(&self) -> Option<String> {
        self.vault_list
            .selected()
            .and_then(|i| self.vaults.get(i).cloned())
    }

    fn refresh_entries(&mut self) {
        self.entries = match &self.session {
            Some(s) => {
                if self.search_query.is_empty() {
                    lilypad_app::list_entries(s)
                } else {
                    lilypad_app::search_entries(s, &self.search_query)
                }
            }
            None => Vec::new(),
        };
        self.trashed = match &self.session {
            Some(s) => lilypad_app::list_trash(s),
            None => Vec::new(),
        };
        let sel = if self.entries.is_empty() {
            None
        } else {
            Some(
                self.entry_list
                    .selected()
                    .unwrap_or(0)
                    .min(self.entries.len() - 1),
            )
        };
        self.entry_list.select(sel);
        let tsel = if self.trashed.is_empty() {
            None
        } else {
            Some(
                self.trash_list
                    .selected()
                    .unwrap_or(0)
                    .min(self.trashed.len() - 1),
            )
        };
        self.trash_list.select(tsel);
    }

    fn selected_trash_label(&self) -> Option<String> {
        self.trash_list
            .selected()
            .and_then(|i| self.trashed.get(i))
            .map(|e| e.label.clone())
    }

    fn selected_entry_label(&self) -> Option<String> {
        self.entry_list
            .selected()
            .and_then(|i| self.entries.get(i))
            .map(|e| e.label.clone())
    }

    fn lock(&mut self) {
        self.session = None;
        self.view_reveal = None;
        self.entries.clear();
        self.master_password.zeroize();
        self.master_password.clear();
        self.search_query.clear();
        self.screen = Screen::Locked;
        self.vaults = self.service.list_vaults().unwrap_or_default();
        if self.vault_list.selected().is_none() && !self.vaults.is_empty() {
            self.vault_list.select(Some(0));
        }
    }

    fn copy_to_clipboard(&mut self, value: &str, what: &str) {
        match lilypad_common::clipboard::copy_to_clipboard(value) {
            Ok(()) => {
                self.clipboard_clear_at =
                    Some(Instant::now() + Duration::from_secs(self.clipboard_clear_secs));
                self.set_status(format!(
                    "Copied {what} (auto-clears in {}s)",
                    self.clipboard_clear_secs
                ));
            }
            Err(_) => self.set_status("Clipboard unavailable (install wl-copy or xclip)"),
        }
    }

    fn tick(&mut self) {
        if let Some(at) = self.status_at {
            if at.elapsed() > Duration::from_secs(4) {
                self.status = None;
                self.status_at = None;
            }
        }
        if let Some(at) = self.clipboard_clear_at {
            if Instant::now() >= at {
                let _ = lilypad_common::clipboard::copy_to_clipboard("");
                self.clipboard_clear_at = None;
            }
        }
        if self.session.as_ref().is_some_and(|s| s.is_expired()) {
            self.lock();
            self.set_status("Locked (idle timeout)");
        }
    }
}

// ---------------------------------------------------------------------------
// Input handling
// ---------------------------------------------------------------------------

impl App {
    fn on_key(&mut self, key: event::KeyEvent) {
        if let Some(s) = self.session.as_mut() {
            s.touch();
        }
        match self.screen {
            Screen::Locked => self.key_locked(key),
            Screen::CreateVault => self.key_create(key),
            Screen::Unlocked => self.key_unlocked(key),
            Screen::View => self.key_view(key),
            Screen::Form => self.key_form(key),
            Screen::ConfirmDelete => self.key_confirm_delete(key),
            Screen::Generate => self.key_generate(key),
            Screen::Trash => self.key_trash(key),
            Screen::ConfirmPurge => self.key_confirm_purge(key),
            Screen::Help => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')
                ) {
                    self.screen = Screen::Unlocked;
                }
            }
        }
    }

    fn key_locked(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Up => self.move_selection(&mut SelectionTarget::Vault, -1),
            KeyCode::Down => self.move_selection(&mut SelectionTarget::Vault, 1),
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.screen = Screen::CreateVault;
            }
            KeyCode::Char(c) => self.master_password.push(c),
            KeyCode::Backspace => {
                self.master_password.pop();
            }
            KeyCode::Enter => self.try_unlock(),
            _ => {}
        }
    }

    fn try_unlock(&mut self) {
        let Some(vault) = self.selected_vault() else {
            self.set_status("No vault selected");
            return;
        };
        match self.service.unlock(&vault, &self.master_password) {
            Ok(session) => {
                self.session = Some(session);
                self.master_password.zeroize();
                self.master_password.clear();
                self.search_query.clear();
                self.screen = Screen::Unlocked;
                self.refresh_entries();
                self.set_status(format!("Unlocked '{vault}'"));
            }
            Err(e) => {
                self.master_password.zeroize();
                self.master_password.clear();
                // The vault is NEVER modified on a failed unlock.
                self.set_status(format!("{e}"));
            }
        }
    }

    fn key_create(&mut self, key: event::KeyEvent) {
        // Two fields: name then password. Tab/Enter advances; on the password
        // field Enter creates.
        match key.code {
            KeyCode::Esc => {
                if self.vaults.is_empty() {
                    self.should_quit = true;
                } else {
                    self.screen = Screen::Locked;
                }
            }
            KeyCode::Tab => self.create_focus_name = !self.create_focus_name,
            KeyCode::Enter => {
                if self.new_vault_name.trim().is_empty() {
                    self.set_status("Vault name required");
                    return;
                }
                if self.new_vault_password.is_empty() {
                    self.set_status("Master password required");
                    return;
                }
                match self
                    .service
                    .create_vault(self.new_vault_name.trim(), &self.new_vault_password)
                {
                    Ok(session) => {
                        let name = self.new_vault_name.trim().to_string();
                        self.new_vault_password.zeroize();
                        self.new_vault_password.clear();
                        self.new_vault_name.clear();
                        self.session = Some(session);
                        self.screen = Screen::Unlocked;
                        self.refresh_entries();
                        self.set_status(format!("Created and unlocked '{name}'"));
                    }
                    Err(e) => self.set_status(format!("{e}")),
                }
            }
            KeyCode::Char(c) => {
                if self.new_vault_name_focused() {
                    self.new_vault_name.push(c);
                } else {
                    self.new_vault_password.push(c);
                }
            }
            KeyCode::Backspace => {
                if self.new_vault_name_focused() {
                    self.new_vault_name.pop();
                } else {
                    self.new_vault_password.pop();
                }
            }
            _ => {}
        }
    }

    // Simple heuristic: name focused until it is non-empty and the user pressed
    // Tab. We track focus with a bool via Tab toggling.
    fn new_vault_name_focused(&self) -> bool {
        self.create_focus_name
    }

    fn key_unlocked(&mut self, key: event::KeyEvent) {
        if self.searching {
            match key.code {
                KeyCode::Esc => {
                    self.searching = false;
                    self.search_query.clear();
                    self.refresh_entries();
                }
                KeyCode::Enter => self.searching = false,
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.refresh_entries();
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.refresh_entries();
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('l') => {
                self.lock();
                self.set_status("Locked");
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection(&mut SelectionTarget::Entry, -1)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection(&mut SelectionTarget::Entry, 1)
            }
            KeyCode::Char('/') => {
                self.searching = true;
                self.search_query.clear();
            }
            KeyCode::Enter | KeyCode::Char('v') => self.open_view(),
            KeyCode::Char('c') => self.copy_selected_password(),
            KeyCode::Char('u') => self.copy_selected_username(),
            KeyCode::Char('t') => self.copy_selected_totp(),
            KeyCode::Char('a') => self.open_form(None),
            KeyCode::Char('e') => {
                if let Some(label) = self.selected_entry_label() {
                    self.open_form(Some(label));
                }
            }
            KeyCode::Char('d') => {
                if self.selected_entry_label().is_some() {
                    self.screen = Screen::ConfirmDelete;
                }
            }
            KeyCode::Char('g') => {
                self.gen_result.clear();
                self.screen = Screen::Generate;
            }
            KeyCode::Char('T') => {
                self.trash_list
                    .select((!self.trashed.is_empty()).then_some(0));
                self.screen = Screen::Trash;
            }
            KeyCode::Char('?') => self.screen = Screen::Help,
            _ => {}
        }
    }

    fn key_trash(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.screen = Screen::Unlocked,
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection(&mut SelectionTarget::Trash, -1)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection(&mut SelectionTarget::Trash, 1)
            }
            KeyCode::Char('r') => {
                if let Some(label) = self.selected_trash_label() {
                    let result = self
                        .session
                        .as_mut()
                        .map(|s| lilypad_app::restore(&self.service, s, &label));
                    match result {
                        Some(Ok(())) => {
                            self.refresh_entries();
                            self.set_status(format!("Restored '{label}'"));
                            if self.trashed.is_empty() {
                                self.screen = Screen::Unlocked;
                            }
                        }
                        Some(Err(e)) => self.set_status(format!("{e}")),
                        None => {}
                    }
                }
            }
            // Purging is unrecoverable: require an explicit confirmation,
            // like the (recoverable!) delete from the live list already does.
            KeyCode::Char('x') if self.selected_trash_label().is_some() => {
                self.screen = Screen::ConfirmPurge;
            }
            _ => {}
        }
    }

    fn key_confirm_purge(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(label) = self.selected_trash_label() {
                    // Permanently erase this trashed entry.
                    let result = self
                        .session
                        .as_mut()
                        .map(|s| lilypad_app::delete_entry(&self.service, s, &label));
                    match result {
                        Some(Ok(())) => {
                            self.refresh_entries();
                            self.set_status(format!("Permanently deleted '{label}'"));
                        }
                        Some(Err(e)) => self.set_status(format!("{e}")),
                        None => {}
                    }
                }
                self.screen = if self.trashed.is_empty() {
                    Screen::Unlocked
                } else {
                    Screen::Trash
                };
            }
            _ => self.screen = Screen::Trash,
        }
    }

    fn open_view(&mut self) {
        let Some(label) = self.selected_entry_label() else {
            return;
        };
        match self
            .session
            .as_ref()
            .map(|s| lilypad_app::reveal_secret(s, &label))
        {
            Some(Ok(revealed)) => {
                self.view_label = label;
                self.view_reveal = Some(revealed);
                self.view_show_password = false;
                self.screen = Screen::View;
            }
            Some(Err(e)) => self.set_status(format!("{e}")),
            None => {}
        }
    }

    fn key_view(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.view_reveal = None;
                self.screen = Screen::Unlocked;
            }
            KeyCode::Char('p') => self.view_show_password = !self.view_show_password,
            KeyCode::Char('c') => {
                if let Some(pw) = self.view_reveal.as_ref().map(|r| r.password.clone()) {
                    self.copy_to_clipboard(&pw, "password");
                }
            }
            KeyCode::Char('u') => {
                let user = self
                    .entries
                    .iter()
                    .find(|e| e.label == self.view_label)
                    .and_then(|e| e.username.clone());
                if let Some(u) = user {
                    self.copy_to_clipboard(&u, "username");
                }
            }
            KeyCode::Char('t') => self.copy_view_totp(),
            _ => {}
        }
    }

    fn key_confirm_delete(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(label) = self.selected_entry_label() {
                    // Soft-delete: recoverable from Trash, not an irreversible erase.
                    let result = self
                        .session
                        .as_mut()
                        .map(|s| lilypad_app::soft_delete(&self.service, s, &label));
                    match result {
                        Some(Ok(())) => {
                            self.refresh_entries();
                            self.set_status(format!("Moved '{label}' to Trash"));
                        }
                        Some(Err(e)) => self.set_status(format!("{e}")),
                        None => {}
                    }
                }
                self.screen = Screen::Unlocked;
            }
            _ => self.screen = Screen::Unlocked,
        }
    }

    fn key_generate(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.screen = Screen::Unlocked,
            KeyCode::Char('r') | KeyCode::Enter => {
                match lilypad_app::generate_password(&lilypad_app::PasswordOptions {
                    length: self.gen_length,
                    uppercase: self.gen_upper,
                    lowercase: self.gen_lower,
                    digits: self.gen_digits,
                    symbols: self.gen_symbols,
                }) {
                    Ok(p) => {
                        self.gen_result.zeroize();
                        self.gen_result = (*p).clone();
                    }
                    Err(e) => self.set_status(format!("{e}")),
                }
            }
            KeyCode::Char('c') => {
                let r = self.gen_result.clone();
                if !r.is_empty() {
                    self.copy_to_clipboard(&r, "generated password");
                }
            }
            KeyCode::Char('+') | KeyCode::Right => self.gen_length = (self.gen_length + 1).min(128),
            KeyCode::Char('-') | KeyCode::Left => {
                self.gen_length = self.gen_length.saturating_sub(1).max(4)
            }
            KeyCode::Char('U') => self.gen_upper = !self.gen_upper,
            KeyCode::Char('L') => self.gen_lower = !self.gen_lower,
            KeyCode::Char('D') => self.gen_digits = !self.gen_digits,
            KeyCode::Char('S') => self.gen_symbols = !self.gen_symbols,
            _ => {}
        }
    }

    // Form ---------------------------------------------------------------
    fn open_form(&mut self, edit_label: Option<String>) {
        let mut form = FormData::default();
        if let Some(label) = &edit_label {
            if let (Some(session), Some(view)) = (
                self.session.as_ref(),
                self.entries.iter().find(|e| &e.label == label).cloned(),
            ) {
                if let Ok(secret) = lilypad_app::reveal_secret(session, label) {
                    form.editing_original = Some(label.clone());
                    form.label = view.label.clone();
                    form.username = view.username.clone().unwrap_or_default();
                    form.url = view.url.clone().unwrap_or_default();
                    form.password = secret.password.clone();
                    form.notes = secret.notes.clone().unwrap_or_default();
                    form.totp = secret.totp_secret.clone().unwrap_or_default();
                }
            }
        }
        self.form = form;
        self.screen = Screen::Form;
    }

    fn form_focus(&self) -> FormField {
        FormField::ORDER[self.form.focus_index % FormField::ORDER.len()]
    }

    fn key_form(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc => self.screen = Screen::Unlocked,
            KeyCode::Tab | KeyCode::Down => {
                let next = self.form_focus().next();
                self.form.focus_index = FormField::ORDER
                    .iter()
                    .position(|f| *f == next)
                    .unwrap_or(0);
            }
            KeyCode::BackTab | KeyCode::Up => {
                let prev = self.form_focus().prev();
                self.form.focus_index = FormField::ORDER
                    .iter()
                    .position(|f| *f == prev)
                    .unwrap_or(0);
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => self.save_form(),
            KeyCode::Char(c) => self.form_field_mut(self.form_focus()).push(c),
            KeyCode::Backspace => {
                self.form_field_mut(self.form_focus()).pop();
            }
            _ => {}
        }
    }

    fn form_field_mut(&mut self, field: FormField) -> &mut String {
        match field {
            FormField::Label => &mut self.form.label,
            FormField::Username => &mut self.form.username,
            FormField::Url => &mut self.form.url,
            FormField::Password => &mut self.form.password,
            FormField::Notes => &mut self.form.notes,
            FormField::Totp => &mut self.form.totp,
        }
    }

    fn save_form(&mut self) {
        if self.form.label.trim().is_empty() {
            self.set_status("Label is required");
            return;
        }
        let label = self.form.label.trim().to_string();
        let notes = opt(&self.form.notes);
        let totp = opt(&self.form.totp);
        let username = opt(&self.form.username);
        let url = opt(&self.form.url);

        let editing = self.form.editing_original.clone();
        // Preserve the entry type on edit (the TUI form does not expose it).
        let entry_type = editing
            .as_ref()
            .and_then(|orig| self.entries.iter().find(|e| &e.label == orig))
            .map(|e| e.entry_type.clone())
            .unwrap_or_default();

        let Some(session) = self.session.as_mut() else {
            return;
        };
        let result = match &editing {
            Some(original) => {
                // Field-preserving edit: attachments, backup codes, email, phone,
                // custom fields, tags and folder are kept; only the form fields
                // are overwritten, and a password change is recorded in history.
                lilypad_app::edit_entry(
                    &self.service,
                    session,
                    original,
                    lilypad_app::EntryEdit {
                        new_label: label.clone(),
                        username,
                        url,
                        entry_type,
                        password: self.form.password.clone(),
                        notes,
                        totp_secret: totp,
                        tags: None,
                        folder: None,
                    },
                )
            }
            None => {
                let mut secret = EntrySecret::new(self.form.password.clone());
                secret.notes = notes;
                secret.totp_secret = totp;
                let metadata = EntryMetadata {
                    username,
                    url,
                    entry_type,
                    ..EntryMetadata::default()
                };
                lilypad_app::add_entry(&self.service, session, &label, metadata, &secret)
            }
        };
        match result {
            Ok(()) => {
                self.screen = Screen::Unlocked;
                self.refresh_entries();
                self.set_status(format!("Saved '{label}'"));
            }
            Err(e) => self.set_status(format!("{e}")),
        }
    }

    // Copy helpers -------------------------------------------------------
    fn copy_selected_password(&mut self) {
        if let Some(label) = self.selected_entry_label() {
            let pw = self
                .session
                .as_ref()
                .and_then(|s| lilypad_app::reveal_secret(s, &label).ok())
                .map(|r| r.password.clone());
            if let Some(pw) = pw {
                self.copy_to_clipboard(&pw, "password");
            }
        }
    }

    fn copy_selected_username(&mut self) {
        let user = self
            .entry_list
            .selected()
            .and_then(|i| self.entries.get(i))
            .and_then(|e| e.username.clone());
        if let Some(u) = user {
            self.copy_to_clipboard(&u, "username");
        } else {
            self.set_status("No username on this entry");
        }
    }

    fn copy_selected_totp(&mut self) {
        if let Some(label) = self.selected_entry_label() {
            let secret = self
                .session
                .as_ref()
                .and_then(|s| lilypad_app::reveal_secret(s, &label).ok());
            self.copy_totp_from(secret);
        }
    }

    fn copy_view_totp(&mut self) {
        let label = self.view_label.clone();
        let secret = self
            .session
            .as_ref()
            .and_then(|s| lilypad_app::reveal_secret(s, &label).ok());
        self.copy_totp_from(secret);
    }

    fn copy_totp_from(&mut self, secret: Option<lilypad_app::RevealedSecret>) {
        let ts = secret.as_ref().and_then(|r| r.totp_secret.clone());
        match ts {
            Some(ts) => match lilypad_app::totp::code_for_secret(&ts) {
                Ok(code) => self.copy_to_clipboard(&code, "TOTP code"),
                Err(e) => self.set_status(format!("{e}")),
            },
            None => self.set_status("No TOTP secret on this entry"),
        }
    }

    fn move_selection(&mut self, target: &mut SelectionTarget, delta: i32) {
        let (state, len) = match target {
            SelectionTarget::Entry => (&mut self.entry_list, self.entries.len()),
            SelectionTarget::Vault => (&mut self.vault_list, self.vaults.len()),
            SelectionTarget::Trash => (&mut self.trash_list, self.trashed.len()),
        };
        if len == 0 {
            return;
        }
        let current = state.selected().unwrap_or(0) as i32;
        let next = (current + delta).rem_euclid(len as i32) as usize;
        state.select(Some(next));
    }
}

enum SelectionTarget {
    Entry,
    Vault,
    Trash,
}

fn opt(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl App {
    fn draw(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(frame.area());
        self.draw_title(frame, chunks[0]);
        match self.screen {
            Screen::Locked => self.draw_locked(frame, chunks[1]),
            Screen::CreateVault => self.draw_create(frame, chunks[1]),
            Screen::Trash | Screen::ConfirmPurge => self.draw_trash(frame, chunks[1]),
            Screen::Unlocked
            | Screen::ConfirmDelete
            | Screen::View
            | Screen::Form
            | Screen::Generate
            | Screen::Help => self.draw_list(frame, chunks[1]),
        }
        self.draw_status(frame, chunks[2]);

        // Overlays
        match self.screen {
            Screen::View => self.draw_view(frame),
            Screen::Form => self.draw_form(frame),
            Screen::Generate => self.draw_generate(frame),
            Screen::Help => self.draw_help(frame),
            Screen::ConfirmDelete => self.draw_confirm(frame),
            Screen::ConfirmPurge => self.draw_confirm_purge(frame),
            _ => {}
        }
    }

    fn draw_title(&self, frame: &mut Frame, area: Rect) {
        let title = match &self.session {
            Some(s) => format!(" Lilypad - {} ", s.name()),
            None => " Lilypad ".to_string(),
        };
        frame.render_widget(
            Paragraph::new(title).style(Style::default().fg(Color::Black).bg(Color::Green)),
            area,
        );
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let hint = match self.screen {
            Screen::Locked => "type password · ↑↓ vault · Enter unlock · ^N new vault · Esc quit",
            Screen::CreateVault => "Tab switch field · Enter create · Esc back",
            Screen::Unlocked if self.searching => "type to filter · Enter done · Esc clear",
            Screen::Unlocked => "a add · e edit · d del · Enter view · c/u/t copy · / search · g gen · T trash · l lock · ? help · q quit",
            Screen::View => "p show/hide · c copy pw · u copy user · t copy totp · Esc back",
            Screen::Form => "Tab next · Ctrl+Enter save · Esc cancel",
            Screen::Generate => "r regen · +/- length · U/L/D/S toggle sets · c copy · Esc back",
            Screen::Help => "Esc close",
            Screen::ConfirmDelete => "y confirm · any other key cancel",
            Screen::Trash => "r restore · x delete forever · ↑↓ move · Esc back",
            Screen::ConfirmPurge => "y delete FOREVER · any other key cancel",
        };
        let text = self.status.clone().unwrap_or_else(|| hint.to_string());
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(Color::DarkGray)),
            area,
        );
    }

    fn draw_locked(&mut self, frame: &mut Frame, area: Rect) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        let items: Vec<ListItem> = self
            .vaults
            .iter()
            .map(|v| ListItem::new(v.clone()))
            .collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Vaults "))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, cols[0], &mut self.vault_list);

        let masked: String = "•".repeat(self.master_password.chars().count());
        let pw = Paragraph::new(format!("\n  Master password:\n  {masked}"))
            .block(Block::default().borders(Borders::ALL).title(" Unlock "));
        frame.render_widget(pw, cols[1]);
    }

    fn draw_create(&mut self, frame: &mut Frame, area: Rect) {
        let masked: String = "•".repeat(self.new_vault_password.chars().count());
        let name_marker = if self.create_focus_name { "> " } else { "  " };
        let pw_marker = if self.create_focus_name { "  " } else { "> " };
        let body = format!(
            "\n {name_marker}Vault name:      {}\n\n {pw_marker}Master password: {masked}\n\n  Tab to switch field, Enter to create.",
            self.new_vault_name
        );
        frame.render_widget(
            Paragraph::new(body)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Create vault "),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn draw_list(&mut self, frame: &mut Frame, area: Rect) {
        let title = if self.search_query.is_empty() {
            format!(" Entries ({}) ", self.entries.len())
        } else {
            format!(" Search: {} ({}) ", self.search_query, self.entries.len())
        };
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|e| {
                let mut spans = vec![Span::raw(e.label.clone())];
                if let Some(u) = &e.username {
                    spans.push(Span::styled(
                        format!("  {u}"),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                ListItem::new(Line::from(spans))
            })
            .collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut self.entry_list);
    }

    fn draw_trash(&mut self, frame: &mut Frame, area: Rect) {
        let title = format!(" Trash ({}) ", self.trashed.len());
        let items: Vec<ListItem> = if self.trashed.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                "  (empty)",
                Style::default().fg(Color::DarkGray),
            )))]
        } else {
            self.trashed
                .iter()
                .map(|e| {
                    let mut spans = vec![Span::raw(e.label.clone())];
                    if let Some(u) = &e.username {
                        spans.push(Span::styled(
                            format!("  {u}"),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect()
        };
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut self.trash_list);
    }

    fn draw_view(&self, frame: &mut Frame) {
        let area = centered_rect(70, 60, frame.area());
        frame.render_widget(Clear, area);
        let view = self.entries.iter().find(|e| e.label == self.view_label);
        let mut lines = vec![Line::from(Span::styled(
            self.view_label.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ))];
        lines.push(Line::from(""));
        if let Some(v) = view {
            if let Some(u) = &v.username {
                lines.push(Line::from(format!("Username: {u}")));
            }
            if let Some(u) = &v.url {
                lines.push(Line::from(format!("URL:      {u}")));
            }
            if !v.tags.is_empty() {
                lines.push(Line::from(format!("Tags:     {}", v.tags.join(", "))));
            }
        }
        if let Some(secret) = &self.view_reveal {
            let pw = if self.view_show_password {
                secret.password.clone()
            } else {
                "•".repeat(secret.password.chars().count().max(8))
            };
            lines.push(Line::from(format!("Password: {pw}")));
            if let Some(n) = &secret.notes {
                lines.push(Line::from(format!("Notes:    {n}")));
            }
            if secret.totp_secret.is_some() {
                lines.push(Line::from("TOTP:     configured (t to copy)"));
            }
            if !secret.attachments.is_empty() {
                lines.push(Line::from(format!(
                    "Attachments: {}",
                    secret.attachments.len()
                )));
            }
        }
        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title(" Entry "))
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn draw_form(&self, frame: &mut Frame) {
        let area = centered_rect(70, 70, frame.area());
        frame.render_widget(Clear, area);
        let focus = self.form_focus();
        let mut lines = Vec::new();
        for field in FormField::ORDER {
            let value = match field {
                FormField::Label => self.form.label.clone(),
                FormField::Username => self.form.username.clone(),
                FormField::Url => self.form.url.clone(),
                FormField::Password => "•".repeat(self.form.password.chars().count()),
                FormField::Notes => self.form.notes.clone(),
                FormField::Totp => self.form.totp.clone(),
            };
            let marker = if field == focus { "> " } else { "  " };
            let style = if field == focus {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("{marker}{:<13} {value}", format!("{}:", field.label())),
                style,
            )));
        }
        let title = if self.form.editing_original.is_some() {
            " Edit entry "
        } else {
            " Add entry "
        };
        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title(title))
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn draw_generate(&self, frame: &mut Frame) {
        let area = centered_rect(60, 50, frame.area());
        frame.render_widget(Clear, area);
        let toggle = |b: bool| if b { "[x]" } else { "[ ]" };
        let body = format!(
            "\n Length: {}\n\n {} Upper (U)   {} Lower (L)\n {} Digits (D)  {} Symbols (S)\n\n Result:\n  {}\n",
            self.gen_length,
            toggle(self.gen_upper),
            toggle(self.gen_lower),
            toggle(self.gen_digits),
            toggle(self.gen_symbols),
            if self.gen_result.is_empty() { "(press r)" } else { &self.gen_result },
        );
        frame.render_widget(
            Paragraph::new(body).block(Block::default().borders(Borders::ALL).title(" Generate ")),
            area,
        );
    }

    fn draw_confirm(&self, frame: &mut Frame) {
        let area = centered_rect(50, 20, frame.area());
        frame.render_widget(Clear, area);
        let label = self.selected_entry_label().unwrap_or_default();
        frame.render_widget(
            Paragraph::new(format!(
                "\n Delete entry '{label}'?\n\n y = yes, any other key = no"
            ))
            .block(Block::default().borders(Borders::ALL).title(" Confirm ")),
            area,
        );
    }

    fn draw_confirm_purge(&self, frame: &mut Frame) {
        let area = centered_rect(55, 22, frame.area());
        frame.render_widget(Clear, area);
        let label = self.selected_trash_label().unwrap_or_default();
        frame.render_widget(
            Paragraph::new(format!(
                "\n Permanently delete '{label}'?\n This cannot be undone.\n\n y = yes, any other key = no"
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Delete forever "),
            ),
            area,
        );
    }

    fn draw_help(&self, frame: &mut Frame) {
        let area = centered_rect(60, 70, frame.area());
        frame.render_widget(Clear, area);
        let body = "\n Navigation\n  j/k or ↑/↓   move\n  Enter or v   view entry\n  /            search\n\n Actions\n  a  add entry     e  edit entry\n  d  to Trash      T  open Trash\n  g  generate      l  lock vault\n  c  copy password u  copy username\n  t  copy TOTP\n\n Trash\n  r  restore       x  delete forever\n\n  q / Esc  quit\n";
        frame.render_widget(
            Paragraph::new(body).block(Block::default().borders(Borders::ALL).title(" Help ")),
            area,
        );
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn main() -> Result<()> {
    let mut app = App::new(None)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run<B>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B: ratatui::backend::Backend,
    B::Error: Send + Sync + 'static,
{
    loop {
        app.tick();
        terminal.draw(|f| app.draw(f))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    app.on_key(key);
                }
            }
        }
        if app.should_quit {
            // Best-effort clipboard clear on exit.
            if app.clipboard_clear_at.is_some() {
                let _ = lilypad_common::clipboard::copy_to_clipboard("");
            }
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! These exercise the TUI's logic (unlock, entry refresh, add) directly,
    //! bypassing crossterm/the terminal, so the data-loss-critical unlock path
    //! is verified deterministically.
    use super::*;
    use tempfile::TempDir;

    fn app_in(dir: &TempDir) -> App {
        App::new(Some(dir.path().to_path_buf())).expect("open app")
    }

    fn seed_vault(app: &mut App, name: &str, password: &str, entry: &str) {
        let mut session = app.service.create_vault(name, password).unwrap();
        let secret = EntrySecret::new(entry.to_string());
        lilypad_app::add_entry(
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
            .push(lilypad_app::Attachment::new("key.pem", "ZmlsZQ=="));
        lilypad_app::add_entry(
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

        let revealed = lilypad_app::reveal_secret(app.session.as_ref().unwrap(), "gh").unwrap();
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
}
