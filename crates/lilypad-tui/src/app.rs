//! The TUI state and the operations every screen shares.

use anyhow::Result;
use lilypad_core::{App as Service, EntryView, OpenOptions, Session};
use ratatui::widgets::ListState;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

#[derive(PartialEq)]
pub enum Screen {
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
pub enum FormField {
    Label,
    Username,
    Url,
    Password,
    Notes,
    Totp,
}

impl FormField {
    pub const ORDER: [FormField; 6] = [
        FormField::Label,
        FormField::Username,
        FormField::Url,
        FormField::Password,
        FormField::Notes,
        FormField::Totp,
    ];
    pub fn next(self) -> FormField {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        Self::ORDER[(i + 1) % Self::ORDER.len()]
    }
    pub fn prev(self) -> FormField {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        Self::ORDER[(i + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
    pub fn label(self) -> &'static str {
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
pub struct FormData {
    pub editing_original: Option<String>,
    pub label: String,
    pub username: String,
    pub url: String,
    pub password: String,
    pub notes: String,
    pub totp: String,
    pub focus_index: usize,
}

impl Drop for FormData {
    fn drop(&mut self) {
        self.password.zeroize();
        self.totp.zeroize();
        // Notes are EntrySecret material too (encrypted at rest).
        self.notes.zeroize();
    }
}

pub struct App {
    pub service: Service,
    pub session: Option<Session>,
    pub screen: Screen,

    pub vaults: Vec<String>,
    pub vault_list: ListState,
    pub master_password: String,

    // Create-vault form
    pub new_vault_name: String,
    pub new_vault_password: String,
    pub create_focus_name: bool,

    pub entries: Vec<EntryView>,
    pub entry_list: ListState,
    pub search_query: String,
    pub searching: bool,

    // Trash (soft-deleted entries)
    pub trashed: Vec<EntryView>,
    pub trash_list: ListState,

    // Idle/clipboard policy (from persisted settings)
    pub clipboard_clear_secs: u64,

    // View screen
    pub view_label: String,
    pub view_reveal: Option<lilypad_core::RevealedSecret>,
    pub view_show_password: bool,

    pub form: FormData,

    // Generator
    pub gen_length: usize,
    pub gen_upper: bool,
    pub gen_lower: bool,
    pub gen_digits: bool,
    pub gen_symbols: bool,
    pub gen_result: String,

    pub status: Option<String>,
    pub status_at: Option<Instant>,
    pub clipboard_clear_at: Option<Instant>,

    pub should_quit: bool,
}

impl Drop for App {
    fn drop(&mut self) {
        self.master_password.zeroize();
        self.new_vault_password.zeroize();
        self.gen_result.zeroize();
    }
}

impl App {
    pub fn new(data_dir: Option<std::path::PathBuf>) -> Result<Self> {
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

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some(msg.into());
        self.status_at = Some(Instant::now());
    }

    pub fn selected_vault(&self) -> Option<String> {
        self.vault_list
            .selected()
            .and_then(|i| self.vaults.get(i).cloned())
    }

    pub fn refresh_entries(&mut self) {
        self.entries = match &self.session {
            Some(s) => {
                if self.search_query.is_empty() {
                    lilypad_core::list_entries(s)
                } else {
                    lilypad_core::search_entries(s, &self.search_query)
                }
            }
            None => Vec::new(),
        };
        self.trashed = match &self.session {
            Some(s) => lilypad_core::list_trash(s),
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

    pub fn selected_trash_label(&self) -> Option<String> {
        self.trash_list
            .selected()
            .and_then(|i| self.trashed.get(i))
            .map(|e| e.label.clone())
    }

    pub fn selected_entry_label(&self) -> Option<String> {
        self.entry_list
            .selected()
            .and_then(|i| self.entries.get(i))
            .map(|e| e.label.clone())
    }

    pub fn lock(&mut self) {
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

    pub fn copy_to_clipboard(&mut self, value: &str, what: &str) {
        match lilypad_core::clipboard::copy_to_clipboard(value) {
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

    pub fn tick(&mut self) {
        if let Some(at) = self.status_at {
            if at.elapsed() > Duration::from_secs(4) {
                self.status = None;
                self.status_at = None;
            }
        }
        if let Some(at) = self.clipboard_clear_at {
            if Instant::now() >= at {
                let _ = lilypad_core::clipboard::copy_to_clipboard("");
                self.clipboard_clear_at = None;
            }
        }
        if self.session.as_ref().is_some_and(|s| s.is_expired()) {
            self.lock();
            self.set_status("Locked (idle timeout)");
        }
    }
}
