//! The application state and the helpers that read or reset it.

use crate::message::{Filter, Message};
use crate::theme::UiVariation;
use crate::view::strength_level;
use colony_ui::preferences::StandardPreferences;
use iced::{Subscription, Task};
use lilypad_core::i18n::{t, tf};
use lilypad_core::{
    App as Service, EntryType, EntryView, OpenOptions, PasswordOptions, RevealedSecret, Session,
};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

/// The modifier of [`shortcut`] as the sidebar hint names it.
pub const SHORTCUT_MODIFIER: &str = if cfg!(target_os = "macos") {
    "Cmd"
} else {
    "Ctrl"
};

/// Ctrl+, (Cmd+, on macOS) opens and closes Preferences, like the Lilypad
/// name. Matched on the character, so it is the key that types a comma on
/// any layout.
pub fn shortcut(event: iced::keyboard::Event) -> Option<Message> {
    match event {
        iced::keyboard::Event::KeyPressed { key, modifiers, .. }
            if modifiers.command() && key.as_ref() == iced::keyboard::Key::Character(",") =>
        {
            Some(Message::TogglePreferences)
        }
        _ => None,
    }
}

#[derive(PartialEq)]
pub enum Screen {
    Unlock,
    Create,
    Vault,
}

pub enum Overlay {
    None,
    Generator,
    ConfirmDelete(String),
    AddForm,
}

#[derive(Default)]
pub struct FormBuffers {
    pub editing: Option<String>,
    pub label: String,
    pub username: String,
    pub url: String,
    pub password: String,
    pub notes: String,
    pub totp: String,
    /// Comma-separated tag list, as typed.
    pub tags: String,
    pub folder: String,
    pub entry_type: Option<EntryType>,
    /// Password expiry choice: None = keep as-is, Some(0) = never,
    /// Some(n) = n days from now.
    pub expiry_days: Option<u32>,
}

impl Drop for FormBuffers {
    fn drop(&mut self) {
        self.password.zeroize();
        self.totp.zeroize();
        // Notes are EntrySecret material too (encrypted at rest).
        self.notes.zeroize();
    }
}

pub struct LilypadApp {
    pub service: Service,
    pub data_dir: Option<PathBuf>,
    pub session: Option<Session>,
    /// Theme, accent, text sizes, motion: the Colony standard preferences.
    pub prefs: StandardPreferences,
    /// The chosen interface language tag; `None` follows the system.
    pub language: Option<String>,
    pub density: UiVariation,
    pub clipboard_secs: u64,
    pub auto_lock_minutes: u64,
    pub default_filter: u8,
    pub restore_last_vault: bool,

    pub screen: Screen,
    pub overlay: Overlay,

    /// Preferences, orthogonal to `screen` (reachable while locked or
    /// unlocked). While open it replaces the content area; the sidebar stays.
    pub preferences_open: bool,
    pub preferences_category: usize,
    /// Keys of the collapsible Preferences sections currently open.
    pub expanded: HashSet<String>,
    /// A dismissible notice over the content ("Theme applied.").
    pub notice: Option<&'static str>,
    /// Change-master-password buffers (wiped on drop).
    pub cm_new: String,
    pub cm_confirm: String,

    /// File paths typed into the Vault settings' import/export section.
    pub import_path: String,
    pub export_path: String,

    /// Labels whose password an explicit HIBP check found in breaches. Feeds
    /// the health report's Compromised category until the next lock.
    pub breached: HashSet<String>,
    pub breach_busy: bool,

    /// A sync network operation is in flight (buttons disabled meanwhile).
    pub sync_busy: bool,
    /// Pending device-flow login: code + URL shown while GitHub waits.
    pub sync_login: Option<lilypad_core::sync::net::DeviceLogin>,
    /// Last fetched sync status for the active vault.
    pub sync_status: Option<lilypad_core::SyncStatusView>,
    /// Master password for a validated pull (wiped after use and on drop).
    pub sync_pull_password: String,

    pub vaults: Vec<String>,
    pub selected_vault: Option<String>,
    pub pending_vault: String,
    pub password: String,
    pub unlocking: bool,

    pub new_name: String,
    pub new_password: String,

    pub entries: Vec<EntryView>,
    /// Per-entry derived data (strength level 1..=5, has_totp), computed once
    /// at unlock/refresh with the secrets dropped immediately - so the list can
    /// show strength dots and TOTP affordances without holding any plaintext.
    pub derived: HashMap<String, (u8, bool)>,
    pub trashed: Vec<EntryView>,
    pub health: Option<lilypad_core::HealthReport>,
    pub authed: Option<String>,
    pub search: String,
    pub filter: Filter,
    pub hovered: Option<String>,

    pub selected: Option<String>,
    pub selected_secret: Option<RevealedSecret>,
    pub reveal: bool,
    pub reveal_hold: bool,
    pub editing: bool,
    pub totp_code: String,
    pub totp_left: u64,

    pub form: FormBuffers,

    pub gen_length: usize,
    pub gen_upper: bool,
    pub gen_lower: bool,
    pub gen_digits: bool,
    pub gen_symbols: bool,
    pub gen_result: String,

    pub status: Option<String>,
    pub status_at: Option<Instant>,
    pub clipboard_clear_at: Option<Instant>,
}

impl Drop for LilypadApp {
    fn drop(&mut self) {
        self.password.zeroize();
        self.new_password.zeroize();
        self.gen_result.zeroize();
        self.cm_new.zeroize();
        self.cm_confirm.zeroize();
        self.sync_pull_password.zeroize();
    }
}

impl LilypadApp {
    pub fn new() -> (Self, Task<Message>) {
        Self::with_data_dir(None)
    }

    pub fn with_data_dir(data_dir: Option<PathBuf>) -> (Self, Task<Message>) {
        let bootstrap = Service::open(OpenOptions {
            data_dir: data_dir.clone(),
            auto_lock_after: None,
        })
        .expect("failed to open Lilypad data directory");
        let settings = bootstrap.load_settings();
        // Theme, accent, fonts, sizes and motion are process-wide in
        // colony-ui, and so is the locale: set them before the first view.
        settings.standard.apply();
        lilypad_core::i18n::set_locale(settings.locale());
        let auto_lock = settings.auto_lock_duration();
        let service = Service::open(OpenOptions {
            data_dir: data_dir.clone(),
            auto_lock_after: auto_lock,
        })
        .expect("failed to open Lilypad data directory");

        let vaults = service.list_vaults().unwrap_or_default();
        let selected_vault = if settings.restore_last_vault {
            settings
                .active_vault
                .clone()
                .filter(|v| vaults.contains(v))
                .or_else(|| vaults.first().cloned())
        } else {
            vaults.first().cloned()
        };
        let screen = if vaults.is_empty() {
            Screen::Create
        } else {
            Screen::Unlock
        };
        let app = Self {
            service,
            data_dir,
            session: None,
            prefs: settings.standard.clone(),
            language: settings.language.clone(),
            density: UiVariation::from_index(settings.density as usize),
            clipboard_secs: settings.clipboard_clear_secs,
            auto_lock_minutes: settings.auto_lock_minutes,
            default_filter: settings.default_filter,
            restore_last_vault: settings.restore_last_vault,
            screen,
            overlay: Overlay::None,
            preferences_open: false,
            preferences_category: 0,
            expanded: HashSet::new(),
            notice: None,
            cm_new: String::new(),
            cm_confirm: String::new(),
            import_path: String::new(),
            export_path: String::new(),
            breached: HashSet::new(),
            breach_busy: false,
            sync_busy: false,
            sync_login: None,
            sync_status: None,
            sync_pull_password: String::new(),
            vaults,
            selected_vault,
            pending_vault: String::new(),
            password: String::new(),
            unlocking: false,
            new_name: String::new(),
            new_password: String::new(),
            entries: Vec::new(),
            derived: HashMap::new(),
            trashed: Vec::new(),
            health: None,
            authed: None,
            search: String::new(),
            filter: Filter::All,
            hovered: None,
            selected: None,
            selected_secret: None,
            reveal: false,
            reveal_hold: false,
            editing: false,
            totp_code: String::new(),
            totp_left: 0,
            form: FormBuffers::default(),
            gen_length: 20,
            gen_upper: true,
            gen_lower: true,
            gen_digits: true,
            gen_symbols: true,
            gen_result: String::new(),
            status: None,
            status_at: None,
            clipboard_clear_at: None,
        };
        (app, Task::none())
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
            iced::keyboard::listen().filter_map(shortcut),
        ])
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some(msg.into());
        self.status_at = Some(Instant::now());
    }

    pub fn refresh_entries(&mut self) {
        match &self.session {
            Some(s) => {
                self.entries = lilypad_core::list_entries(s);
                self.trashed = lilypad_core::list_trash(s);
                self.health = Some(lilypad_core::vault_health_with_breaches(s, &self.breached));
            }
            None => {
                self.entries = Vec::new();
                self.trashed = Vec::new();
                self.health = None;
            }
        }
        self.compute_derived();
    }

    /// Decrypts each entry once to record its strength + has-TOTP, then drops the
    /// secret. Only the non-secret derived values are kept.
    pub fn compute_derived(&mut self) {
        let mut map = HashMap::new();
        if let Some(session) = self.session.as_ref() {
            for e in &self.entries {
                if let Ok(secret) = lilypad_core::reveal_secret(session, &e.label) {
                    map.insert(
                        e.label.clone(),
                        (
                            strength_level(&secret.password),
                            secret.totp_secret.is_some(),
                        ),
                    );
                }
            }
        }
        self.derived = map;
    }

    /// Recomputes the selected entry's live TOTP code and seconds remaining.
    pub fn refresh_totp(&mut self) {
        self.totp_code.clear();
        self.totp_left = 0;
        let ts = self
            .selected_secret
            .as_ref()
            .and_then(|s| s.totp_secret.clone());
        if let Some(ts) = ts {
            if let Ok(code) = lilypad_core::totp::code_for_secret(&ts) {
                self.totp_code = (*code).clone();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                self.totp_left = 30 - (now % 30);
            }
        }
    }

    pub fn revealed(&self) -> bool {
        self.reveal || self.reveal_hold
    }

    /// Writes every setting this frontend owns back to disk, preserving the
    /// fields it does not manage (window ratios). Called after any preference
    /// change so a change survives a restart.
    pub fn persist_settings(&self) {
        let mut s = self.service.load_settings();
        s.standard = self.prefs.clone();
        s.language = self.language.clone();
        s.density = self.density.to_index() as u32;
        s.active_vault = self.selected_vault.clone();
        s.auto_lock_minutes = self.auto_lock_minutes;
        s.clipboard_clear_secs = self.clipboard_secs;
        s.default_filter = self.default_filter;
        s.restore_last_vault = self.restore_last_vault;
        let _ = self.service.save_settings(&s);
    }

    /// The current auto-lock policy as a `Duration` (None when disabled).
    pub fn auto_lock_duration(&self) -> Option<Duration> {
        if self.auto_lock_minutes == 0 {
            None
        } else {
            Some(Duration::from_secs(self.auto_lock_minutes * 60))
        }
    }

    /// The vault sync operations act on: the unlocked session's vault, or the
    /// one selected on the unlock screen (sync works on the on-disk state, so a
    /// session is not required).
    pub fn active_vault_name(&self) -> Option<String> {
        self.session
            .as_ref()
            .map(|s| s.name().to_string())
            .or_else(|| self.selected_vault.clone())
    }

    pub fn lock(&mut self) {
        self.session = None;
        self.selected = None;
        self.selected_secret = None;
        self.editing = false;
        self.entries.clear();
        self.password.zeroize();
        self.password.clear();
        self.sync_pull_password.zeroize();
        self.sync_pull_password.clear();
        // A half-typed new master password must not survive an auto-lock.
        self.cm_new.zeroize();
        self.cm_new.clear();
        self.cm_confirm.zeroize();
        self.cm_confirm.clear();
        // Breach results describe the locked session's vault.
        self.breached.clear();
        self.breach_busy = false;
        self.search.clear();
        self.filter = Filter::All;
        self.overlay = Overlay::None;
        self.screen = Screen::Unlock;
        self.vaults = self.service.list_vaults().unwrap_or_default();
        if self.selected_vault.is_none() {
            self.selected_vault = self.vaults.first().cloned();
        }
    }

    /// Copies `value`; `what_key` is the i18n key naming what was copied.
    pub fn copy(&mut self, value: &str, what_key: &str) {
        match lilypad_core::clipboard::copy_to_clipboard(value) {
            Ok(()) => {
                self.clipboard_clear_at =
                    Some(Instant::now() + Duration::from_secs(self.clipboard_secs.max(1)));
                self.set_status(tf(
                    "status_copied",
                    &[("what", &t(what_key)), ("secs", &self.clipboard_secs)],
                ));
            }
            Err(_) => self.set_status(t("status_clipboard_unavailable")),
        }
    }

    pub fn regen(&mut self) {
        match lilypad_core::generate_password(&PasswordOptions {
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

    pub fn reveal_selected(&mut self) {
        self.selected_secret = None;
        if let (Some(label), Some(session)) = (self.selected.clone(), self.session.as_ref()) {
            match lilypad_core::reveal_secret(session, &label) {
                Ok(secret) => self.selected_secret = Some(secret),
                Err(e) => self.set_status(e.localized()),
            }
        }
    }

    /// Entries matching the current sidebar filter + search.
    pub fn visible(&self) -> Vec<&EntryView> {
        let q = self.search.trim().to_lowercase();
        let source = if self.filter == Filter::Trash {
            &self.trashed
        } else {
            &self.entries
        };
        source
            .iter()
            .filter(|e| match &self.filter {
                Filter::All | Filter::Trash => true,
                Filter::Favorites => e.is_favorite,
                Filter::Type(t) => &e.entry_type == t,
                Filter::Folder(f) => e
                    .folder
                    .as_deref()
                    .is_some_and(|ef| ef == f || ef.starts_with(&format!("{f}/"))),
                Filter::Tag(tag) => e.tags.contains(tag),
            })
            .filter(|e| {
                q.is_empty()
                    || e.label.to_lowercase().contains(&q)
                    || e.username
                        .as_deref()
                        .is_some_and(|u| u.to_lowercase().contains(&q))
                    || e.url
                        .as_deref()
                        .is_some_and(|u| u.to_lowercase().contains(&q))
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect()
    }
}
