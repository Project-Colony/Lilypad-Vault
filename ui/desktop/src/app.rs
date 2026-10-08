//! The Lilypad desktop application - a 3-pane vault over `lilypad-app`.
//!
//! sidebar (filters) | item list | inline detail/edit. Unlocking goes through
//! `open_with_key` (only reads; a wrong password never overwrites the vault) and
//! Argon2 runs off-thread via `Task`, so the window never freezes. The whole
//! shell mounts only once a session exists; unlock/create are centered cards.

use crate::fonts;
use crate::message::{CharSet, Filter, FormField, Message, QuickKind};
use crate::theme::{self, LilypadTheme, UiVariation};
use iced::widget::overlay::menu as overlay_menu;
use iced::widget::{
    button, canvas, column, container, mouse_area, pick_list, row, scrollable, slider, stack, text,
    text_input, Canvas, Space,
};
use iced::{Alignment, Background, Color, Element, Length, Radians, Subscription, Task};
use lilypad_app::{
    App as Service, EntryMetadata, EntrySecret, EntryType, EntryView, KeyMaterial, OpenOptions,
    PasswordOptions, RevealedSecret, Session,
};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

#[derive(PartialEq)]
enum Screen {
    Unlock,
    Create,
    Vault,
}

enum Overlay {
    None,
    Generator,
    ConfirmDelete(String),
    AddForm,
}

#[derive(Default)]
struct FormBuffers {
    editing: Option<String>,
    label: String,
    username: String,
    url: String,
    password: String,
    notes: String,
    totp: String,
    /// Comma-separated tag list, as typed.
    tags: String,
    folder: String,
    entry_type: Option<EntryType>,
    /// Password expiry choice: None = keep as-is, Some(0) = never,
    /// Some(n) = n days from now.
    expiry_days: Option<u32>,
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
    service: Service,
    data_dir: Option<PathBuf>,
    session: Option<Session>,
    theme: LilypadTheme,
    density: UiVariation,
    clipboard_secs: u64,
    auto_lock_minutes: u64,
    default_filter: u8,
    restore_last_vault: bool,

    screen: Screen,
    overlay: Overlay,

    /// The full-page settings screen, orthogonal to `screen` (reachable while
    /// locked or unlocked). When set, it replaces whatever screen is underneath.
    settings_open: bool,
    settings_category: usize,
    /// Keys of the collapsible settings sections currently expanded.
    settings_sections: HashSet<String>,
    /// Change-master-password buffers (wiped on drop).
    cm_new: String,
    cm_confirm: String,

    /// File paths typed into the Vault settings' import/export section.
    import_path: String,
    export_path: String,

    /// Labels whose password an explicit HIBP check found in breaches. Feeds
    /// the health report's Compromised category until the next lock.
    breached: HashSet<String>,
    breach_busy: bool,

    /// A sync network operation is in flight (buttons disabled meanwhile).
    sync_busy: bool,
    /// Pending device-flow login: code + URL shown while GitHub waits.
    sync_login: Option<lilypad_app::sync::net::DeviceLogin>,
    /// Last fetched sync status for the active vault.
    sync_status: Option<lilypad_app::SyncStatusView>,
    /// Master password for a validated pull (wiped after use and on drop).
    sync_pull_password: String,

    vaults: Vec<String>,
    selected_vault: Option<String>,
    pending_vault: String,
    password: String,
    unlocking: bool,

    new_name: String,
    new_password: String,

    entries: Vec<EntryView>,
    /// Per-entry derived data (strength level 1..=5, has_totp), computed once
    /// at unlock/refresh with the secrets dropped immediately - so the list can
    /// show strength dots and TOTP affordances without holding any plaintext.
    derived: HashMap<String, (u8, bool)>,
    trashed: Vec<EntryView>,
    health: Option<lilypad_app::HealthReport>,
    authed: Option<String>,
    search: String,
    filter: Filter,
    hovered: Option<String>,

    selected: Option<String>,
    selected_secret: Option<RevealedSecret>,
    reveal: bool,
    reveal_hold: bool,
    editing: bool,
    totp_code: String,
    totp_left: u64,

    form: FormBuffers,

    gen_length: usize,
    gen_upper: bool,
    gen_lower: bool,
    gen_digits: bool,
    gen_symbols: bool,
    gen_result: String,

    status: Option<String>,
    status_at: Option<Instant>,
    clipboard_clear_at: Option<Instant>,
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

    fn with_data_dir(data_dir: Option<PathBuf>) -> (Self, Task<Message>) {
        let bootstrap = Service::open(OpenOptions {
            data_dir: data_dir.clone(),
            auto_lock_after: None,
        })
        .expect("failed to open Lilypad data directory");
        let settings = bootstrap.load_settings();
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
            theme: LilypadTheme::from_index(settings.theme as usize),
            density: UiVariation::from_index(settings.density as usize),
            clipboard_secs: settings.clipboard_clear_secs,
            auto_lock_minutes: settings.auto_lock_minutes,
            default_filter: settings.default_filter,
            restore_last_vault: settings.restore_last_vault,
            screen,
            overlay: Overlay::None,
            settings_open: false,
            settings_category: 0,
            settings_sections: default_settings_sections(),
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
        iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some(msg.into());
        self.status_at = Some(Instant::now());
    }

    fn refresh_entries(&mut self) {
        match &self.session {
            Some(s) => {
                self.entries = lilypad_app::list_entries(s);
                self.trashed = lilypad_app::list_trash(s);
                self.health = Some(lilypad_app::vault_health_with_breaches(s, &self.breached));
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
    fn compute_derived(&mut self) {
        let mut map = HashMap::new();
        if let Some(session) = self.session.as_ref() {
            for e in &self.entries {
                if let Ok(secret) = lilypad_app::reveal_secret(session, &e.label) {
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
    fn refresh_totp(&mut self) {
        self.totp_code.clear();
        self.totp_left = 0;
        let ts = self
            .selected_secret
            .as_ref()
            .and_then(|s| s.totp_secret.clone());
        if let Some(ts) = ts {
            if let Ok(code) = lilypad_app::totp::code_for_secret(&ts) {
                self.totp_code = (*code).clone();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                self.totp_left = 30 - (now % 30);
            }
        }
    }

    fn revealed(&self) -> bool {
        self.reveal || self.reveal_hold
    }

    /// Writes every setting this frontend owns back to disk, preserving the
    /// fields it does not manage (window ratios). Called after any preference
    /// change so a change survives a restart.
    fn persist_settings(&self) {
        let mut s = self.service.load_settings();
        s.theme = self.theme.to_index() as u32;
        s.density = self.density.to_index() as u32;
        s.active_vault = self.selected_vault.clone();
        s.auto_lock_minutes = self.auto_lock_minutes;
        s.clipboard_clear_secs = self.clipboard_secs;
        s.default_filter = self.default_filter;
        s.restore_last_vault = self.restore_last_vault;
        let _ = self.service.save_settings(&s);
    }

    /// The current auto-lock policy as a `Duration` (None when disabled).
    fn auto_lock_duration(&self) -> Option<Duration> {
        if self.auto_lock_minutes == 0 {
            None
        } else {
            Some(Duration::from_secs(self.auto_lock_minutes * 60))
        }
    }

    /// The vault sync operations act on: the unlocked session's vault, or the
    /// one selected on the unlock screen (sync works on the on-disk state, so a
    /// session is not required).
    fn active_vault_name(&self) -> Option<String> {
        self.session
            .as_ref()
            .map(|s| s.name().to_string())
            .or_else(|| self.selected_vault.clone())
    }

    fn lock(&mut self) {
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

    fn copy(&mut self, value: &str, what: &str) {
        match lilypad_common::clipboard::copy_to_clipboard(value) {
            Ok(()) => {
                self.clipboard_clear_at =
                    Some(Instant::now() + Duration::from_secs(self.clipboard_secs.max(1)));
                self.set_status(format!(
                    "Copied {what} · clears in {}s",
                    self.clipboard_secs
                ));
            }
            Err(_) => self.set_status("Clipboard unavailable (install wl-copy or xclip)"),
        }
    }

    fn regen(&mut self) {
        match lilypad_app::generate_password(&PasswordOptions {
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

    fn reveal_selected(&mut self) {
        self.selected_secret = None;
        if let (Some(label), Some(session)) = (self.selected.clone(), self.session.as_ref()) {
            match lilypad_app::reveal_secret(session, &label) {
                Ok(secret) => self.selected_secret = Some(secret),
                Err(e) => self.set_status(format!("{e}")),
            }
        }
    }

    /// Entries matching the current sidebar filter + search.
    fn visible(&self) -> Vec<&EntryView> {
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

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop => Task::none(),
            Message::Tick => {
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
                if self.selected_secret.is_some() {
                    self.refresh_totp();
                }
                Task::none()
            }
            Message::Dismiss => {
                self.status = None;
                Task::none()
            }

            Message::ThemeSelected(t) => {
                self.theme = t;
                self.persist_settings();
                Task::none()
            }
            Message::DensitySelected(d) => {
                self.density = d;
                self.persist_settings();
                Task::none()
            }

            Message::OpenSettings => {
                self.settings_open = true;
                self.overlay = Overlay::None;
                // Populate the GitHub auth state from the local token store
                // (fast disk read, no network) so the Sync tab is correct even
                // before the first unlock.
                if self.authed.is_none() {
                    self.authed = match lilypad_app::sync::net::auth_status() {
                        Ok(lilypad_app::sync::net::AuthState::Authenticated { username }) => {
                            Some(username)
                        }
                        _ => None,
                    };
                }
                Task::none()
            }
            Message::CloseSettings => {
                self.settings_open = false;
                self.cm_new.zeroize();
                self.cm_new.clear();
                self.cm_confirm.zeroize();
                self.cm_confirm.clear();
                self.sync_pull_password.zeroize();
                self.sync_pull_password.clear();
                Task::none()
            }
            Message::SettingsCategory(i) => {
                self.settings_category = i;
                Task::none()
            }
            Message::SettingsToggleSection(key) => {
                if !self.settings_sections.remove(&key) {
                    self.settings_sections.insert(key);
                }
                Task::none()
            }
            Message::SetAutoLock(minutes) => {
                self.auto_lock_minutes = minutes;
                // Apply to the live session so the change takes effect now, not
                // only on the next unlock.
                let dur = self.auto_lock_duration();
                if let Some(session) = self.session.as_mut() {
                    session.set_auto_lock(dur);
                }
                self.persist_settings();
                self.set_status(if minutes == 0 {
                    "Auto-lock disabled".to_string()
                } else {
                    format!("Auto-lock set to {minutes} min")
                });
                Task::none()
            }
            Message::SetClipboardSecs(secs) => {
                self.clipboard_secs = secs;
                self.persist_settings();
                self.set_status(format!("Clipboard clears after {secs}s"));
                Task::none()
            }
            Message::SetDefaultFilter(f) => {
                self.default_filter = f;
                self.persist_settings();
                Task::none()
            }
            Message::ToggleRestoreLastVault => {
                self.restore_last_vault = !self.restore_last_vault;
                self.persist_settings();
                Task::none()
            }
            Message::CmNewChanged(v) => {
                self.cm_new = v;
                Task::none()
            }
            Message::CmConfirmChanged(v) => {
                self.cm_confirm = v;
                Task::none()
            }
            Message::ChangeMasterSubmit => {
                if self.cm_new.len() < 8 {
                    self.set_status("Master password must be at least 8 characters");
                    return Task::none();
                }
                if self.cm_new != self.cm_confirm {
                    self.set_status("Passwords do not match");
                    return Task::none();
                }
                // Zeroizing: this working copy of the new master password is
                // wiped when it drops, on every path.
                let new = zeroize::Zeroizing::new(self.cm_new.clone());
                let result = match self.session.as_mut() {
                    Some(session) => {
                        lilypad_app::change_master_password(&self.service, session, &new)
                    }
                    None => {
                        self.set_status("Unlock a vault first");
                        return Task::none();
                    }
                };
                match result {
                    Ok(()) => {
                        self.cm_new.zeroize();
                        self.cm_new.clear();
                        self.cm_confirm.zeroize();
                        self.cm_confirm.clear();
                        self.set_status("Master password changed");
                    }
                    Err(e) => self.set_status(format!("{e}")),
                }
                Task::none()
            }

            Message::ImportPathChanged(p) => {
                self.import_path = p;
                Task::none()
            }
            Message::ImportRun => {
                let path = self.import_path.trim().to_string();
                if path.is_empty() {
                    self.set_status("Enter the path of the exported file to import");
                    return Task::none();
                }
                if self.session.is_none() {
                    self.set_status("Unlock a vault first");
                    return Task::none();
                }
                let result = std::fs::read(&path)
                    .map_err(|e| format!("failed to read {path}: {e}"))
                    .and_then(|bytes| {
                        let format =
                            lilypad_app::detect_format(&bytes).map_err(|e| e.to_string())?;
                        let parsed =
                            lilypad_app::parse_import(&bytes, format).map_err(|e| e.to_string())?;
                        Ok((format, parsed))
                    });
                match result {
                    Ok((format, parsed)) => {
                        let warnings = parsed.warnings.len();
                        let session = self.session.as_mut().expect("checked above");
                        match lilypad_app::import_entries(&self.service, session, parsed) {
                            Ok(report) => {
                                self.refresh_entries();
                                self.import_path.clear();
                                let mut msg = format!(
                                    "Imported {} entries from {}",
                                    report.added,
                                    format.name()
                                );
                                if !report.renamed.is_empty() {
                                    msg.push_str(&format!(
                                        " ({} renamed to avoid overwriting)",
                                        report.renamed.len()
                                    ));
                                }
                                if report.skipped > 0 || warnings > 0 {
                                    msg.push_str(&format!(
                                        " - {} skipped/warnings",
                                        report.skipped + warnings
                                    ));
                                }
                                msg.push_str(" - delete the source file once verified");
                                self.set_status(msg);
                            }
                            Err(e) => self.set_status(format!("{e}")),
                        }
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }
            Message::ExportPathChanged(p) => {
                self.export_path = p;
                Task::none()
            }
            Message::ExportRun => {
                let path = self.export_path.trim().to_string();
                if path.is_empty() {
                    self.set_status("Enter a destination path for the CSV export");
                    return Task::none();
                }
                let Some(session) = self.session.as_ref() else {
                    self.set_status("Unlock a vault first");
                    return Task::none();
                };
                match lilypad_app::export_csv(session) {
                    Ok(csv) => {
                        let write = std::fs::write(&path, csv.as_bytes());
                        #[cfg(unix)]
                        if write.is_ok() {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = std::fs::set_permissions(
                                &path,
                                std::fs::Permissions::from_mode(0o600),
                            );
                        }
                        match write {
                            Ok(()) => {
                                self.export_path.clear();
                                self.set_status(format!(
                                    "Exported PLAINTEXT CSV to {path} - store it safely, delete it when done"
                                ));
                            }
                            Err(e) => self.set_status(format!("failed to write {path}: {e}")),
                        }
                    }
                    Err(e) => self.set_status(format!("{e}")),
                }
                Task::none()
            }

            Message::BreachCheckRun => {
                if self.breach_busy {
                    return Task::none();
                }
                let Some(session) = self.session.as_ref() else {
                    self.set_status("Unlock a vault first");
                    return Task::none();
                };
                // Hashing is local and fast (UI thread); only the k-anonymity
                // range queries go to the background task. No plaintext and no
                // session crosses the thread boundary - only SHA-1 fragments.
                let hashes = lilypad_app::collect_hashes(session);
                self.breach_busy = true;
                self.set_status("Checking against Have-I-Been-Pwned...");
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            lilypad_app::query_hashes(hashes)
                                .map(|r| {
                                    (
                                        r.unique_checked,
                                        r.breached
                                            .into_iter()
                                            .map(|b| (b.label, b.count))
                                            .collect::<Vec<_>>(),
                                    )
                                })
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("background task failed: {e}")))
                    },
                    Message::BreachChecked,
                )
            }
            Message::BreachChecked(result) => {
                self.breach_busy = false;
                match result {
                    Ok((unique, breached)) => {
                        let n = breached.len();
                        self.breached = breached.into_iter().map(|(label, _)| label).collect();
                        // Recompute health so Compromised findings appear in the
                        // grade, the sidebar badge, and the per-entry banners.
                        self.refresh_entries();
                        self.set_status(if n == 0 {
                            format!(
                                "Breach check: {unique} unique passwords checked - none breached"
                            )
                        } else {
                            format!(
                                "Breach check: {n} entr{} with breached passwords - change them",
                                if n == 1 { "y" } else { "ies" }
                            )
                        });
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }

            Message::SyncLoginStart => {
                if self.sync_busy {
                    return Task::none();
                }
                self.sync_busy = true;
                Task::perform(
                    async {
                        tokio::task::spawn_blocking(|| {
                            lilypad_app::sync::net::begin_login().map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("background task failed: {e}")))
                    },
                    Message::SyncLoginStarted,
                )
            }
            Message::SyncLoginStarted(result) => match result {
                Ok(login) => {
                    // Show the code in the UI, open the verification page, and
                    // poll in the background until the user authorizes.
                    let _ = webbrowser::open(&login.verification_uri);
                    self.sync_login = Some(login.clone());
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                lilypad_app::sync::net::complete_login(&login)
                                    .map_err(|e| e.to_string())
                            })
                            .await
                            .unwrap_or_else(|e| Err(format!("background task failed: {e}")))
                        },
                        Message::SyncLoginDone,
                    )
                }
                Err(e) => {
                    self.sync_busy = false;
                    self.set_status(e);
                    Task::none()
                }
            },
            Message::SyncLoginCancel => {
                // Re-enables the sync UI; the abandoned poll either expires
                // server-side or completes and is still recorded harmlessly.
                self.sync_busy = false;
                self.sync_login = None;
                self.set_status("Sign-in cancelled");
                Task::none()
            }
            Message::SyncLoginDone(result) => {
                let was_cancelled = self.sync_login.is_none() && !self.sync_busy;
                self.sync_busy = false;
                self.sync_login = None;
                match result {
                    Ok(username) => {
                        self.authed = Some(username.clone());
                        self.set_status(format!("Signed in to GitHub as {username}"));
                    }
                    // A cancelled flow's eventual expiry error is not news.
                    Err(_) if was_cancelled => {}
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }
            Message::SyncLogout => {
                if self.sync_busy {
                    return Task::none();
                }
                // Local token removal only; no network round-trip.
                match lilypad_app::sync::net::logout() {
                    Ok(()) => {
                        self.authed = None;
                        self.sync_status = None;
                        self.set_status("Signed out of GitHub");
                    }
                    Err(e) => self.set_status(format!("{e}")),
                }
                Task::none()
            }
            Message::SyncPush => {
                let Some(vault) = self.active_vault_name() else {
                    self.set_status("No vault selected");
                    return Task::none();
                };
                if self.sync_busy {
                    return Task::none();
                }
                self.sync_busy = true;
                let data_dir = self.data_dir.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let app = Service::open(OpenOptions {
                                data_dir,
                                auto_lock_after: None,
                            })
                            .map_err(|e| e.to_string())?;
                            lilypad_app::sync::net::push(&app, &vault, false)
                                .map(|_| vault)
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("background task failed: {e}")))
                    },
                    Message::SyncPushDone,
                )
            }
            Message::SyncPushDone(result) => {
                self.sync_busy = false;
                match result {
                    Ok(vault) => {
                        // Only mark in-sync if that vault is still the active one.
                        if self.active_vault_name().as_deref() == Some(vault.as_str()) {
                            self.sync_status = Some(lilypad_app::SyncStatusView::InSync);
                        }
                        self.set_status(format!("Pushed '{vault}' to GitHub"));
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }
            Message::SyncPullPasswordChanged(p) => {
                self.sync_pull_password = p;
                Task::none()
            }
            Message::SyncPull => {
                if self.sync_busy {
                    return Task::none();
                }
                let Some(vault) = self.active_vault_name() else {
                    self.set_status("No vault selected");
                    return Task::none();
                };
                if self.sync_pull_password.is_empty() {
                    self.set_status("Enter the master password (Settings > Sync section) to validate the pulled vault");
                    return Task::none();
                }
                self.sync_busy = true;
                let data_dir = self.data_dir.clone();
                let pw = std::mem::take(&mut self.sync_pull_password);
                // Lock BEFORE the pull runs, not after: otherwise the vault
                // stays editable while the remote payload replaces the file,
                // and an edit landing in that window is silently clobbered.
                if self.session.is_some() {
                    self.lock();
                    self.set_status("Locked for pull...");
                }
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            // Owns the password for the duration of the pull,
                            // wiping it when the task ends.
                            let pw = zeroize::Zeroizing::new(pw);
                            let app = Service::open(OpenOptions {
                                data_dir,
                                auto_lock_after: None,
                            })
                            .map_err(|e| e.to_string())?;
                            lilypad_app::sync::net::pull(&app, &vault, &pw)
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("background task failed: {e}")))
                    },
                    Message::SyncPullDone,
                )
            }
            Message::SyncPullDone(result) => {
                self.sync_busy = false;
                match result {
                    Ok(lilypad_app::sync::net::PullOutcome::Applied) => {
                        // The on-disk vault was replaced (validated, safety
                        // backup taken) and may be keyed differently: lock so
                        // the next unlock reads the fresh state.
                        if self.session.is_some() {
                            self.lock();
                        }
                        self.sync_status = Some(lilypad_app::SyncStatusView::InSync);
                        self.set_status("Pulled from GitHub - unlock to open the updated vault");
                    }
                    Ok(lilypad_app::sync::net::PullOutcome::NoRemote) => {
                        self.set_status("No remote vault with this name")
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }
            Message::SyncMerge => {
                if self.sync_busy {
                    return Task::none();
                }
                let Some(vault) = self.active_vault_name() else {
                    self.set_status("No vault selected");
                    return Task::none();
                };
                if self.sync_pull_password.is_empty() {
                    self.set_status("Enter the master password (Settings > Sync section) to sync");
                    return Task::none();
                }
                self.sync_busy = true;
                let data_dir = self.data_dir.clone();
                let pw = std::mem::take(&mut self.sync_pull_password);
                // Same discipline as pull: the merge rewrites the vault file,
                // so lock BEFORE it runs - an edit landing mid-merge would
                // otherwise be silently clobbered.
                if self.session.is_some() {
                    self.lock();
                    self.set_status("Locked for sync...");
                }
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            // Owns the password for the duration of the merge,
                            // wiping it when the task ends.
                            let pw = zeroize::Zeroizing::new(pw);
                            let app = Service::open(OpenOptions {
                                data_dir,
                                auto_lock_after: None,
                            })
                            .map_err(|e| e.to_string())?;
                            match lilypad_app::sync::net::sync_merge(&app, &vault, &pw)
                                .map_err(|e| e.to_string())?
                            {
                                lilypad_app::sync::net::MergeOutcome::NoRemote => Ok((vault, None)),
                                lilypad_app::sync::net::MergeOutcome::Merged { report, pushed } => {
                                    Ok((vault, Some((report.summary(), pushed))))
                                }
                            }
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("background task failed: {e}")))
                    },
                    Message::SyncMergeDone,
                )
            }
            Message::SyncMergeDone(result) => {
                self.sync_busy = false;
                match result {
                    Ok((_, None)) => {
                        self.set_status("No remote vault with this name - push first to create it")
                    }
                    Ok((vault, Some((summary, pushed)))) => {
                        // The on-disk vault was merged (safety backup taken):
                        // lock so the next unlock reads the fresh state - but
                        // only a session on the MERGED vault; a session on a
                        // different vault was never touched by this merge.
                        if self
                            .session
                            .as_ref()
                            .is_some_and(|s| s.name() == vault.as_str())
                        {
                            self.lock();
                        }
                        if self.active_vault_name().as_deref() == Some(vault.as_str()) {
                            self.sync_status = Some(lilypad_app::SyncStatusView::InSync);
                        }
                        let suffix = if pushed { "; pushed back" } else { "" };
                        self.set_status(format!(
                            "Synced '{vault}': {summary}{suffix} - unlock to open the merged vault"
                        ));
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }
            Message::SyncRefreshStatus => {
                let Some(vault) = self.active_vault_name() else {
                    self.set_status("No vault selected");
                    return Task::none();
                };
                if self.sync_busy {
                    return Task::none();
                }
                self.sync_busy = true;
                let data_dir = self.data_dir.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let app = Service::open(OpenOptions {
                                data_dir,
                                auto_lock_after: None,
                            })
                            .map_err(|e| e.to_string())?;
                            lilypad_app::sync::net::remote_status(&app, &vault)
                                .map(|s| (vault, s))
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("background task failed: {e}")))
                    },
                    Message::SyncStatusLoaded,
                )
            }
            Message::SyncStatusLoaded(result) => {
                self.sync_busy = false;
                match result {
                    // Apply only if the status still describes the active vault
                    // (the user may have switched while the request ran).
                    Ok((vault, status)) => {
                        if self.active_vault_name().as_deref() == Some(vault.as_str()) {
                            self.sync_status = Some(status);
                        }
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }

            Message::VaultSelected(v) => {
                self.selected_vault = Some(v);
                // The last fetched sync status described the previous vault.
                self.sync_status = None;
                Task::none()
            }
            Message::PasswordChanged(p) => {
                self.password = p;
                Task::none()
            }
            Message::UnlockPressed => {
                // A sync (pull or merge) rewrites the vault file: opening a
                // session mid-flight would hand the user a stale vault whose
                // edits raced the rewrite.
                if self.sync_busy {
                    self.set_status("A sync is running - wait for it to finish before unlocking");
                    return Task::none();
                }
                let Some(vault) = self.selected_vault.clone() else {
                    self.set_status("No vault selected");
                    return Task::none();
                };
                self.pending_vault = vault.clone();
                self.unlocking = true;
                derive_key_task(self.data_dir.clone(), vault, self.password.clone())
            }
            Message::KeyDerived(result) => {
                self.unlocking = false;
                // Same guard as UnlockPressed, for a derive that was already
                // in flight when the sync started.
                if self.sync_busy {
                    self.password.zeroize();
                    self.password.clear();
                    self.set_status("A sync is running - unlock again when it finishes");
                    return Task::none();
                }
                match result {
                    Ok(key) => {
                        let vault = self.pending_vault.clone();
                        match self.service.open_with_key(&vault, &key) {
                            Ok(mut session) => {
                                // Apply the current auto-lock policy so it is
                                // correct regardless of what the service was
                                // opened with (settings may have changed since).
                                session.set_auto_lock(self.auto_lock_duration());
                                self.session = Some(session);
                                self.password.zeroize();
                                self.password.clear();
                                self.new_password.zeroize();
                                self.new_password.clear();
                                self.search.clear();
                                self.filter = if self.default_filter == 1 {
                                    Filter::Favorites
                                } else {
                                    Filter::All
                                };
                                self.selected = None;
                                self.selected_secret = None;
                                self.screen = Screen::Vault;
                                self.overlay = Overlay::None;
                                self.selected_vault = Some(vault.clone());
                                if !self.vaults.contains(&vault) {
                                    self.vaults.push(vault);
                                }
                                self.refresh_entries();
                                self.persist_settings();
                                self.authed = match lilypad_app::sync::net::auth_status() {
                                    Ok(lilypad_app::sync::net::AuthState::Authenticated {
                                        username,
                                    }) => Some(username),
                                    _ => None,
                                };
                            }
                            Err(e) => self.set_status(format!("{e}")),
                        }
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }

            Message::ShowCreate => {
                self.screen = Screen::Create;
                Task::none()
            }
            Message::ShowUnlock => {
                self.screen = Screen::Unlock;
                Task::none()
            }
            Message::NewNameChanged(n) => {
                self.new_name = n;
                Task::none()
            }
            Message::NewPasswordChanged(p) => {
                self.new_password = p;
                Task::none()
            }
            Message::CreatePressed => {
                let name = self.new_name.trim().to_string();
                if name.is_empty() {
                    self.set_status("Vault name is required");
                    return Task::none();
                }
                if self.new_password.is_empty() {
                    self.set_status("Master password is required");
                    return Task::none();
                }
                self.pending_vault = name.clone();
                self.unlocking = true;
                create_vault_task(self.data_dir.clone(), name, self.new_password.clone())
            }

            Message::FilterSelected(f) => {
                self.filter = f;
                self.selected = None;
                self.selected_secret = None;
                self.editing = false;
                Task::none()
            }
            Message::SearchChanged(q) => {
                self.search = q;
                Task::none()
            }

            Message::SelectEntry(label) => {
                self.selected = Some(label);
                self.editing = false;
                self.reveal = false;
                self.reveal_hold = false;
                self.reveal_selected();
                self.refresh_totp();
                Task::none()
            }
            Message::ToggleReveal => {
                self.reveal = !self.reveal;
                Task::none()
            }
            Message::RevealHold(held) => {
                self.reveal_hold = held;
                Task::none()
            }
            Message::RowHovered(label) => {
                self.hovered = label;
                Task::none()
            }
            Message::QuickCopy(label, kind) => {
                let value: Option<(String, &'static str)> =
                    self.session.as_ref().and_then(|session| match kind {
                        QuickKind::Username => self
                            .entries
                            .iter()
                            .find(|e| e.label == label)
                            .and_then(|e| e.username.clone())
                            .map(|u| (u, "username")),
                        QuickKind::Password => lilypad_app::reveal_secret(session, &label)
                            .ok()
                            .map(|s| (s.password.clone(), "password")),
                        QuickKind::Totp => lilypad_app::reveal_secret(session, &label)
                            .ok()
                            .and_then(|s| s.totp_secret.clone())
                            .and_then(|ts| lilypad_app::totp::code_for_secret(&ts).ok())
                            .map(|c| ((*c).clone(), "TOTP code")),
                    });
                match value {
                    Some((v, what)) => self.copy(&v, what),
                    None => self.set_status("Nothing to copy"),
                }
                Task::none()
            }
            Message::ClipboardCancel => {
                let _ = lilypad_common::clipboard::copy_to_clipboard("");
                self.clipboard_clear_at = None;
                self.set_status("Clipboard cleared");
                Task::none()
            }
            Message::CopyPassword => {
                if let Some(pw) = self.selected_secret.as_ref().map(|s| s.password.clone()) {
                    self.copy(&pw, "password");
                }
                Task::none()
            }
            Message::CopyUsername => {
                let user = self
                    .selected
                    .as_ref()
                    .and_then(|label| self.entries.iter().find(|e| &e.label == label))
                    .and_then(|e| e.username.clone());
                match user {
                    Some(u) => self.copy(&u, "username"),
                    None => self.set_status("No username on this entry"),
                }
                Task::none()
            }
            Message::CopyTotp => {
                let ts = self
                    .selected_secret
                    .as_ref()
                    .and_then(|s| s.totp_secret.clone());
                match ts {
                    Some(ts) => match lilypad_app::totp::code_for_secret(&ts) {
                        Ok(code) => self.copy(&code, "TOTP code"),
                        Err(e) => self.set_status(format!("{e}")),
                    },
                    None => self.set_status("No TOTP secret on this entry"),
                }
                Task::none()
            }
            Message::OpenUrl(url) => {
                let _ = webbrowser::open(&url);
                Task::none()
            }
            Message::ToggleFavorite(label) => {
                let new_state = !self
                    .entries
                    .iter()
                    .find(|e| e.label == label)
                    .map(|e| e.is_favorite)
                    .unwrap_or(false);
                if let Some(session) = self.session.as_mut() {
                    if let Err(e) =
                        lilypad_app::set_favorite(&self.service, session, &label, new_state)
                    {
                        self.set_status(format!("{e}"));
                    } else {
                        self.refresh_entries();
                    }
                }
                Task::none()
            }
            Message::SetColor(label, color) => {
                if let Some(session) = self.session.as_mut() {
                    let _ = lilypad_app::set_color(&self.service, session, &label, color);
                    self.refresh_entries();
                }
                Task::none()
            }
            Message::Lock => {
                self.lock();
                self.set_status("Locked");
                Task::none()
            }

            Message::ShowAddForm => {
                self.form = FormBuffers::default();
                self.overlay = Overlay::AddForm;
                Task::none()
            }
            Message::StartEdit => {
                if let (Some(label), Some(secret)) =
                    (self.selected.clone(), self.selected_secret.as_ref())
                {
                    let view = self.entries.iter().find(|e| e.label == label).cloned();
                    if let Some(view) = view {
                        self.form = FormBuffers {
                            editing: Some(label.clone()),
                            label,
                            username: view.username.unwrap_or_default(),
                            url: view.url.unwrap_or_default(),
                            password: secret.password.clone(),
                            notes: secret.notes.clone().unwrap_or_default(),
                            totp: secret.totp_secret.clone().unwrap_or_default(),
                            tags: view.tags.join(", "),
                            folder: view.folder.unwrap_or_default(),
                            entry_type: Some(view.entry_type),
                            expiry_days: None, // keep current unless changed
                        };
                        self.editing = true;
                    }
                }
                Task::none()
            }
            Message::FormChanged(field, value) => {
                match field {
                    FormField::Label => self.form.label = value,
                    FormField::Username => self.form.username = value,
                    FormField::Url => self.form.url = value,
                    FormField::Password => self.form.password = value,
                    FormField::Notes => self.form.notes = value,
                    FormField::Totp => self.form.totp = value,
                    FormField::Tags => self.form.tags = value,
                    FormField::Folder => self.form.folder = value,
                }
                Task::none()
            }
            Message::FormTypeSelected(label) => {
                self.form.entry_type = Some(entry_type_from_label(&label));
                Task::none()
            }
            Message::FormExpirySelected(choice) => {
                self.form.expiry_days = expiry_days_from_label(&choice);
                Task::none()
            }
            Message::FormCancel => {
                self.editing = false;
                self.overlay = Overlay::None;
                Task::none()
            }
            Message::FormSave => {
                self.save_form();
                Task::none()
            }

            Message::DeleteRequested(label) => {
                self.overlay = Overlay::ConfirmDelete(label);
                Task::none()
            }
            Message::DeleteConfirmed => {
                if let Overlay::ConfirmDelete(label) = &self.overlay {
                    let label = label.clone();
                    let is_trashed = self.trashed.iter().any(|e| e.label == label);
                    if let Some(session) = self.session.as_mut() {
                        let result = if is_trashed {
                            lilypad_app::delete_entry(&self.service, session, &label)
                        } else {
                            lilypad_app::soft_delete(&self.service, session, &label)
                        };
                        match result {
                            Ok(()) => {
                                self.refresh_entries();
                                if self.selected.as_deref() == Some(label.as_str()) {
                                    self.selected = None;
                                    self.selected_secret = None;
                                }
                                self.set_status(if is_trashed {
                                    format!("Permanently deleted '{label}'")
                                } else {
                                    format!("Moved '{label}' to Trash")
                                });
                            }
                            Err(e) => self.set_status(format!("{e}")),
                        }
                    }
                }
                self.overlay = Overlay::None;
                Task::none()
            }
            Message::DeleteCancelled => {
                self.overlay = Overlay::None;
                Task::none()
            }
            Message::RestoreEntry(label) => {
                if let Some(session) = self.session.as_mut() {
                    match lilypad_app::restore(&self.service, session, &label) {
                        Ok(()) => {
                            self.refresh_entries();
                            self.set_status(format!("Restored '{label}'"));
                        }
                        Err(e) => self.set_status(format!("{e}")),
                    }
                }
                Task::none()
            }

            Message::ShowGenerator => {
                if self.gen_result.is_empty() {
                    self.regen();
                }
                self.overlay = Overlay::Generator;
                Task::none()
            }
            Message::GenLength(v) => {
                self.gen_length = v as usize;
                self.regen();
                Task::none()
            }
            Message::GenToggle(set) => {
                match set {
                    CharSet::Upper => self.gen_upper = !self.gen_upper,
                    CharSet::Lower => self.gen_lower = !self.gen_lower,
                    CharSet::Digits => self.gen_digits = !self.gen_digits,
                    CharSet::Symbols => self.gen_symbols = !self.gen_symbols,
                }
                self.regen();
                Task::none()
            }
            Message::GenRegenerate => {
                self.regen();
                Task::none()
            }
            Message::GenUse => {
                let r = self.gen_result.clone();
                if !r.is_empty() {
                    self.copy(&r, "generated password");
                }
                Task::none()
            }
            Message::CloseGenerator => {
                self.overlay = Overlay::None;
                Task::none()
            }
        }
    }

    fn save_form(&mut self) {
        if self.form.label.trim().is_empty() {
            self.set_status("Label is required");
            return;
        }
        let label = self.form.label.trim().to_string();
        let editing = self.form.editing.clone();
        let username = opt(&self.form.username);
        let url = opt(&self.form.url);
        let notes = opt(&self.form.notes);
        let totp = opt(&self.form.totp);
        let entry_type = self.form.entry_type.clone().unwrap_or(EntryType::Login);
        let tags: Vec<String> = self
            .form
            .tags
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(String::from)
            .collect();
        let folder = opt(&self.form.folder);
        let expiry = self.form.expiry_days;

        let Some(session) = self.session.as_mut() else {
            return;
        };
        let result = match &editing {
            Some(original) => {
                // Field-preserving edit: attachments, backup codes, email,
                // phone and custom fields are kept; the form now owns tags and
                // folder too, and a password change enters history.
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
                        tags: Some(tags),
                        folder: Some(folder),
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
                    tags,
                    folder,
                };
                lilypad_app::add_entry(&self.service, session, &label, metadata, &secret)
            }
        };
        // Apply the expiry choice ("keep" = None skips this entirely).
        let result = result.and_then(|()| match expiry {
            Some(days) => {
                let session = self.session.as_mut().expect("checked above");
                lilypad_app::set_expiry_days(&self.service, session, &label, days)
            }
            None => Ok(()),
        });
        match result {
            Ok(()) => {
                self.editing = false;
                self.overlay = Overlay::None;
                self.refresh_entries();
                self.selected = Some(label.clone());
                self.reveal = false;
                self.reveal_selected();
                self.set_status(format!("Saved '{label}'"));
            }
            Err(e) => self.set_status(format!("{e}")),
        }
    }
}

fn opt(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// All entry types, in picker order (labels via `type_label`).
const ALL_TYPES: [EntryType; 8] = [
    EntryType::Login,
    EntryType::Card,
    EntryType::Identity,
    EntryType::SecureNote,
    EntryType::SoftwareLicense,
    EntryType::Wifi,
    EntryType::Server,
    EntryType::Custom,
];

fn entry_type_from_label(label: &str) -> EntryType {
    ALL_TYPES
        .iter()
        .find(|t| type_label(t) == label)
        .cloned()
        .unwrap_or(EntryType::Login)
}

/// Expiry picker choices (label, value). None = keep the current setting.
const EXPIRY_CHOICES: [(&str, Option<u32>); 7] = [
    ("Keep current", None),
    ("Never", Some(0)),
    ("30 days", Some(30)),
    ("60 days", Some(60)),
    ("90 days", Some(90)),
    ("180 days", Some(180)),
    ("1 year", Some(365)),
];

fn expiry_days_from_label(label: &str) -> Option<u32> {
    EXPIRY_CHOICES
        .iter()
        .find(|(l, _)| *l == label)
        .and_then(|(_, v)| *v)
}

fn expiry_label_for(days: Option<u32>) -> &'static str {
    EXPIRY_CHOICES
        .iter()
        .find(|(_, v)| *v == days)
        .map(|(l, _)| *l)
        .unwrap_or("Keep current")
}

/// The iced color for an entry's color label (mirrors EntryColor::to_hex).
fn entry_color(c: lilypad_app::EntryColor) -> Color {
    use lilypad_app::EntryColor as C;
    match c {
        C::Red => Color::from_rgb8(0xEF, 0x44, 0x44),
        C::Orange => Color::from_rgb8(0xF9, 0x73, 0x16),
        C::Yellow => Color::from_rgb8(0xEA, 0xB3, 0x08),
        C::Green => Color::from_rgb8(0x22, 0xC5, 0x5E),
        C::Blue => Color::from_rgb8(0x3B, 0x82, 0xF6),
        C::Purple => Color::from_rgb8(0xA8, 0x55, 0xF7),
        C::Pink => Color::from_rgb8(0xEC, 0x48, 0x99),
        C::Gray => Color::from_rgb8(0x6B, 0x72, 0x80),
    }
}

/// The settings sections expanded by default the first time the page opens, so
/// the most useful controls are visible without a click.
fn default_settings_sections() -> HashSet<String> {
    [
        "theme",
        "density",
        "autolock",
        "clipboard",
        "default_view",
        "stats",
        "sync_account",
        "sync_status",
        "sync_merge",
        "sync_transfer",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// A circular countdown ring (used for the TOTP timer).
struct Ring {
    fraction: f32,
    color: Color,
    track: Color,
}

impl canvas::Program<Message> for Ring {
    type State = ();
    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let radius = bounds.width.min(bounds.height) / 2.0 - 2.5;
        frame.stroke(
            &canvas::Path::circle(center, radius),
            canvas::Stroke::default()
                .with_width(3.0)
                .with_color(self.track),
        );
        let start = -std::f32::consts::FRAC_PI_2;
        let end = start + self.fraction.clamp(0.0, 1.0) * std::f32::consts::TAU;
        let arc = canvas::Path::new(|b| {
            b.arc(canvas::path::Arc {
                center,
                radius,
                start_angle: Radians(start),
                end_angle: Radians(end),
            });
        });
        frame.stroke(
            &arc,
            canvas::Stroke::default()
                .with_width(3.0)
                .with_color(self.color),
        );
        vec![frame.into_geometry()]
    }
}

fn grade_letter(g: lilypad_app::HealthGrade) -> &'static str {
    use lilypad_app::HealthGrade as G;
    match g {
        G::A => "A",
        G::B => "B",
        G::C => "C",
        G::D => "D",
        G::F => "F",
    }
}

/// Grade letter with a +/- refinement from where the score sits in its band
/// (e.g. score 88 in band B -> "B+"). F has no modifier.
fn grade_display(score: u8, g: lilypad_app::HealthGrade) -> String {
    use lilypad_app::HealthGrade as G;
    let letter = grade_letter(g);
    if matches!(g, G::F) {
        return letter.to_string();
    }
    let floor = match g {
        G::A => 90,
        G::B => 80,
        G::C => 70,
        G::D => 60,
        G::F => 0,
    };
    let within = score.saturating_sub(floor);
    let suffix = if within >= 7 {
        "+"
    } else if within <= 2 {
        "-"
    } else {
        ""
    };
    format!("{letter}{suffix}")
}

/// A translucent tint of a color, for banners.
fn tint(c: Color, alpha: f32) -> Color {
    Color { a: alpha, ..c }
}

/// Maps a 1..=5 strength level to a representative 0-100 score, because
/// `theme::strength_color` buckets on a 0-100 score (not the level).
fn strength_score(level: u8) -> u8 {
    match level {
        1 => 10,
        2 => 35,
        3 => 55,
        4 => 75,
        5 => 95,
        _ => 0,
    }
}

/// Password strength as a 1..=5 level (higher = stronger).
fn strength_level(pw: &str) -> u8 {
    use lilypad_common::PasswordStrength as S;
    match lilypad_common::validation::validate_password_strength(pw) {
        S::VeryWeak => 1,
        S::Weak => 2,
        S::Fair => 3,
        S::Strong => 4,
        S::VeryStrong => 5,
    }
}

fn type_icon(t: &EntryType) -> &'static str {
    use fonts::icons::*;
    match t {
        EntryType::Login => KEY,
        EntryType::Card => CREDIT_CARD,
        EntryType::Identity => ID_CARD,
        EntryType::SecureNote => STICKY_NOTE,
        EntryType::SoftwareLicense => CERTIFICATE,
        EntryType::Wifi => WIFI,
        EntryType::Server => SERVER,
        EntryType::Custom => LEAF,
    }
}

fn type_label(t: &EntryType) -> &'static str {
    match t {
        EntryType::Login => "Logins",
        EntryType::Card => "Cards",
        EntryType::Identity => "Identities",
        EntryType::SecureNote => "Notes",
        EntryType::SoftwareLicense => "Licenses",
        EntryType::Wifi => "Wifi",
        EntryType::Server => "Servers",
        EntryType::Custom => "Custom",
    }
}

fn vgap(px: f32) -> Space {
    Space::new().height(Length::Fixed(px))
}
fn hgap(px: f32) -> Space {
    Space::new().width(Length::Fixed(px))
}
fn hfill() -> Space {
    Space::new().width(Length::Fill)
}

type BtnStyle = Box<dyn Fn(&iced::Theme, button::Status) -> button::Style>;
fn boxed_btn(f: impl Fn(&iced::Theme, button::Status) -> button::Style + 'static) -> BtnStyle {
    Box::new(f)
}

// Async key derivation -------------------------------------------------------

fn derive_key_task(data_dir: Option<PathBuf>, vault: String, password: String) -> Task<Message> {
    Task::perform(
        async move {
            match tokio::task::spawn_blocking(move || {
                let app = Service::open(OpenOptions {
                    data_dir,
                    auto_lock_after: None,
                })
                .map_err(|e| e.to_string())?;
                app.derive_unlock_key(&vault, &password)
                    .map_err(|e| e.to_string())
            })
            .await
            {
                Ok(res) => res,
                Err(e) => Err(format!("background task failed: {e}")),
            }
        },
        Message::KeyDerived,
    )
}

fn create_vault_task(data_dir: Option<PathBuf>, vault: String, password: String) -> Task<Message> {
    Task::perform(
        async move {
            match tokio::task::spawn_blocking(move || {
                let app = Service::open(OpenOptions {
                    data_dir,
                    auto_lock_after: None,
                })
                .map_err(|e| e.to_string())?;
                let session = app
                    .create_vault(&vault, &password)
                    .map_err(|e| e.to_string())?;
                Ok::<KeyMaterial, String>(session.key().clone())
            })
            .await
            {
                Ok(res) => res,
                Err(e) => Err(format!("background task failed: {e}")),
            }
        },
        Message::KeyDerived,
    )
}

// The view layer lives in a second impl block for readability.
include!("view.rs");
// The categorized settings page lives in its own impl block.
include!("settings.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn app_in(dir: &TempDir) -> LilypadApp {
        LilypadApp::with_data_dir(Some(dir.path().to_path_buf())).0
    }

    fn seed(app: &LilypadApp, name: &str, password: &str, entry: &str) {
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
            lilypad_app::sync::net::PullOutcome::Applied,
        )));
        // The on-disk vault may have been re-keyed: the session must be locked
        // and the pull password wiped.
        assert!(app.session.is_none());
        assert!(app.sync_pull_password.is_empty());
        assert_eq!(app.sync_status, Some(lilypad_app::SyncStatusView::InSync));
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
        assert_eq!(app.sync_status, Some(lilypad_app::SyncStatusView::InSync));

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
}
