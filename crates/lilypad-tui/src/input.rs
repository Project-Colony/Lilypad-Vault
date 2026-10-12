//! Keyboard handling for every screen.

use crate::app::{App, FormData, FormField, Screen};
use crossterm::event::{self, KeyCode, KeyModifiers};
use lilypad_core::i18n::{t, tf};
use lilypad_core::{EntryMetadata, EntrySecret};
use zeroize::Zeroize;

impl App {
    pub fn on_key(&mut self, key: event::KeyEvent) {
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

    pub fn key_locked(&mut self, key: event::KeyEvent) {
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

    pub fn try_unlock(&mut self) {
        let Some(vault) = self.selected_vault() else {
            self.set_status(t("status_no_vault"));
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
                self.set_status(tf("tui_unlocked", &[("vault", &vault)]));
            }
            Err(e) => {
                self.master_password.zeroize();
                self.master_password.clear();
                // The vault is NEVER modified on a failed unlock.
                self.set_status(e.localized());
            }
        }
    }

    pub fn key_create(&mut self, key: event::KeyEvent) {
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
                    self.set_status(t("status_vault_name_needed"));
                    return;
                }
                if self.new_vault_password.is_empty() {
                    self.set_status(t("status_master_needed"));
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
                        self.set_status(tf("tui_created", &[("vault", &name)]));
                    }
                    Err(e) => self.set_status(e.localized()),
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
    pub fn new_vault_name_focused(&self) -> bool {
        self.create_focus_name
    }

    pub fn key_unlocked(&mut self, key: event::KeyEvent) {
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
                self.set_status(t("status_locked"));
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

    pub fn key_trash(&mut self, key: event::KeyEvent) {
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
                        .map(|s| lilypad_core::restore(&self.service, s, &label));
                    match result {
                        Some(Ok(())) => {
                            self.refresh_entries();
                            self.set_status(tf("status_restored", &[("label", &label)]));
                            if self.trashed.is_empty() {
                                self.screen = Screen::Unlocked;
                            }
                        }
                        Some(Err(e)) => self.set_status(e.localized()),
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

    pub fn key_confirm_purge(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(label) = self.selected_trash_label() {
                    // Permanently erase this trashed entry.
                    let result = self
                        .session
                        .as_mut()
                        .map(|s| lilypad_core::delete_entry(&self.service, s, &label));
                    match result {
                        Some(Ok(())) => {
                            self.refresh_entries();
                            self.set_status(tf("status_deleted", &[("label", &label)]));
                        }
                        Some(Err(e)) => self.set_status(e.localized()),
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

    pub fn open_view(&mut self) {
        let Some(label) = self.selected_entry_label() else {
            return;
        };
        match self
            .session
            .as_ref()
            .map(|s| lilypad_core::reveal_secret(s, &label))
        {
            Some(Ok(revealed)) => {
                self.view_label = label;
                self.view_reveal = Some(revealed);
                self.view_show_password = false;
                self.screen = Screen::View;
            }
            Some(Err(e)) => self.set_status(e.localized()),
            None => {}
        }
    }

    pub fn key_view(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.view_reveal = None;
                self.screen = Screen::Unlocked;
            }
            KeyCode::Char('p') => self.view_show_password = !self.view_show_password,
            KeyCode::Char('c') => {
                if let Some(pw) = self.view_reveal.as_ref().map(|r| r.password.clone()) {
                    self.copy_to_clipboard(&pw, "copied_password");
                }
            }
            KeyCode::Char('u') => {
                let user = self
                    .entries
                    .iter()
                    .find(|e| e.label == self.view_label)
                    .and_then(|e| e.username.clone());
                if let Some(u) = user {
                    self.copy_to_clipboard(&u, "copied_username");
                }
            }
            KeyCode::Char('t') => self.copy_view_totp(),
            _ => {}
        }
    }

    pub fn key_confirm_delete(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(label) = self.selected_entry_label() {
                    // Soft-delete: recoverable from Trash, not an irreversible erase.
                    let result = self
                        .session
                        .as_mut()
                        .map(|s| lilypad_core::soft_delete(&self.service, s, &label));
                    match result {
                        Some(Ok(())) => {
                            self.refresh_entries();
                            self.set_status(tf("status_trashed", &[("label", &label)]));
                        }
                        Some(Err(e)) => self.set_status(e.localized()),
                        None => {}
                    }
                }
                self.screen = Screen::Unlocked;
            }
            _ => self.screen = Screen::Unlocked,
        }
    }

    pub fn key_generate(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.screen = Screen::Unlocked,
            KeyCode::Char('r') | KeyCode::Enter => {
                match lilypad_core::generate_password(&lilypad_core::PasswordOptions {
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
                    Err(e) => self.set_status(e.localized()),
                }
            }
            KeyCode::Char('c') => {
                let r = self.gen_result.clone();
                if !r.is_empty() {
                    self.copy_to_clipboard(&r, "copied_generated");
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
    pub fn open_form(&mut self, edit_label: Option<String>) {
        let mut form = FormData::default();
        if let Some(label) = &edit_label {
            if let (Some(session), Some(view)) = (
                self.session.as_ref(),
                self.entries.iter().find(|e| &e.label == label).cloned(),
            ) {
                if let Ok(secret) = lilypad_core::reveal_secret(session, label) {
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

    pub fn form_focus(&self) -> FormField {
        FormField::ORDER[self.form.focus_index % FormField::ORDER.len()]
    }

    pub fn key_form(&mut self, key: event::KeyEvent) {
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

    pub fn form_field_mut(&mut self, field: FormField) -> &mut String {
        match field {
            FormField::Label => &mut self.form.label,
            FormField::Username => &mut self.form.username,
            FormField::Url => &mut self.form.url,
            FormField::Password => &mut self.form.password,
            FormField::Notes => &mut self.form.notes,
            FormField::Totp => &mut self.form.totp,
        }
    }

    pub fn save_form(&mut self) {
        if self.form.label.trim().is_empty() {
            self.set_status(t("status_label_needed"));
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
                lilypad_core::edit_entry(
                    &self.service,
                    session,
                    original,
                    lilypad_core::EntryEdit {
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
                lilypad_core::add_entry(&self.service, session, &label, metadata, &secret)
            }
        };
        match result {
            Ok(()) => {
                self.screen = Screen::Unlocked;
                self.refresh_entries();
                self.set_status(tf("status_saved", &[("label", &label)]));
            }
            Err(e) => self.set_status(e.localized()),
        }
    }

    // Copy helpers -------------------------------------------------------
    pub fn copy_selected_password(&mut self) {
        if let Some(label) = self.selected_entry_label() {
            let pw = self
                .session
                .as_ref()
                .and_then(|s| lilypad_core::reveal_secret(s, &label).ok())
                .map(|r| r.password.clone());
            if let Some(pw) = pw {
                self.copy_to_clipboard(&pw, "copied_password");
            }
        }
    }

    pub fn copy_selected_username(&mut self) {
        let user = self
            .entry_list
            .selected()
            .and_then(|i| self.entries.get(i))
            .and_then(|e| e.username.clone());
        if let Some(u) = user {
            self.copy_to_clipboard(&u, "copied_username");
        } else {
            self.set_status(t("status_no_username"));
        }
    }

    pub fn copy_selected_totp(&mut self) {
        if let Some(label) = self.selected_entry_label() {
            let secret = self
                .session
                .as_ref()
                .and_then(|s| lilypad_core::reveal_secret(s, &label).ok());
            self.copy_totp_from(secret);
        }
    }

    pub fn copy_view_totp(&mut self) {
        let label = self.view_label.clone();
        let secret = self
            .session
            .as_ref()
            .and_then(|s| lilypad_core::reveal_secret(s, &label).ok());
        self.copy_totp_from(secret);
    }

    pub fn copy_totp_from(&mut self, secret: Option<lilypad_core::RevealedSecret>) {
        let ts = secret.as_ref().and_then(|r| r.totp_secret.clone());
        match ts {
            Some(ts) => match lilypad_core::totp::code_for_secret(&ts) {
                Ok(code) => self.copy_to_clipboard(&code, "copied_totp"),
                Err(e) => self.set_status(e.localized()),
            },
            None => self.set_status(t("status_no_totp")),
        }
    }

    pub fn move_selection(&mut self, target: &mut SelectionTarget, delta: i32) {
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

pub enum SelectionTarget {
    Entry,
    Vault,
    Trash,
}

pub fn opt(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}
