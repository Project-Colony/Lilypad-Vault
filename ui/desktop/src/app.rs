//! Lilypad Application
//!
//! Main application struct implementing Iced's Application trait.

use std::fs;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use directories::ProjectDirs;
use iced::widget::{column, container, Space};
use iced::{Element, Length, Subscription, Task};
use rand::Rng;

use lilypad_common::{
    analyze_vault_health,
    keyfile::{load_key, save_key, KeyFile},
    time::format_timestamp_relative,
    validation::validate_password_strength,
    EntryHealthData,
};
use lilypad_core::{
    decrypt, default_config, derive_key, encrypt, CryptoAlgorithm, Entry, EntryMetadata,
    KeyDerivationParams, KeyMaterial, KeyMetadata, Vault,
};
use lilypad_storage::LocalStore;
use zeroize::Zeroize;

use crate::message::Message;
use crate::state::{
    AppSettings, Category, DesktopEntryPayload, LockoutState, VaultEntry, VaultViewMode,
    DEFAULT_VAULT_NAME, SETTINGS_VERSION,
};
use crate::theme::{self, LilypadTheme};
use crate::views;

/// Main Lilypad application
pub struct LilypadApp {
    // Application mode
    pub show_welcome: bool,
    pub vault_unlocked: bool,
    pub error_message: Option<String>,

    // Search
    pub search_query: String,

    // Navigation
    pub selected_category: usize,
    pub vault_view_mode: VaultViewMode,

    // Status messages
    pub status_message: Option<String>,
    pub status_message_time: Option<Instant>,

    // Paths
    pub welcome_ack_path: Option<std::path::PathBuf>,
    pub settings_path: Option<std::path::PathBuf>,
    pub lockout_path: Option<std::path::PathBuf>,

    // Sensitive data
    pub master_password: String,
    pub generated_password: String,

    // Core
    pub config: lilypad_core::AppConfig,
    pub store: LocalStore,
    pub active_vault: String,
    pub vault: Option<Vault>,
    pub vault_key: Option<KeyMaterial>,

    // Password generator settings
    pub generator_length: usize,
    pub generator_lowercase: bool,
    pub generator_uppercase: bool,
    pub generator_digits: bool,
    pub generator_symbols: bool,

    // Entry form
    pub show_add_entry: bool,
    pub vault_entries: Vec<VaultEntry>,
    pub entry_title: String,
    pub entry_username: String,
    pub entry_password: String,
    pub entry_url: String,
    pub entry_notes: String,

    // Settings
    pub settings: AppSettings,
    pub show_settings: bool,
    pub theme: LilypadTheme,

    // Security
    pub lockout_state: LockoutState,
    pub last_activity: Instant,

    // Clipboard management
    pub clipboard_clear_time: Option<Instant>,
    pub clipboard_value: Option<String>,

    // Re-authentication modal
    pub show_reauth_modal: bool,
    pub reauth_password: String,
    pub pending_copy_password: Option<String>,

    // Delete confirmation
    pub show_delete_confirm: bool,
    pub pending_delete_index: Option<usize>,

    // Edit mode
    pub edit_mode: bool,
    pub edit_index: Option<usize>,

    // Multi-vault
    pub available_vaults: Vec<String>,
    pub show_vault_selector: bool,
    pub show_new_vault_modal: bool,
    pub new_vault_name: String,

    // Account settings (demo)
    pub account_display_name: String,
    pub account_email: String,
    pub account_timezone: String,
    pub account_two_factor_enabled: bool,
    pub account_marketing_opt_in: bool,
    pub security_recovery_email: String,
    pub security_trusted_devices: Vec<String>,

    // Health dashboard
    pub health_report: Option<lilypad_common::HealthReport>,
}

impl Drop for LilypadApp {
    fn drop(&mut self) {
        self.master_password.zeroize();
        self.generated_password.zeroize();
        self.entry_password.zeroize();
        self.reauth_password.zeroize();
        if let Some(ref mut value) = self.clipboard_value {
            value.zeroize();
        }
        if let Some(ref mut value) = self.pending_copy_password {
            value.zeroize();
        }
        for entry in &mut self.vault_entries {
            entry.password.zeroize();
        }
    }
}

impl LilypadApp {
    /// Create a new application instance
    pub fn new() -> (Self, Task<Message>) {
        let config = default_config();
        let store = LocalStore::new(&config).expect("store init");
        let now = Instant::now();

        let mut app = Self {
            show_welcome: true,
            vault_unlocked: false,
            error_message: None,
            search_query: String::new(),
            selected_category: 0,
            vault_view_mode: VaultViewMode::All,
            status_message: None,
            status_message_time: None,
            welcome_ack_path: None,
            settings_path: None,
            lockout_path: None,
            master_password: String::new(),
            generated_password: String::new(),
            config,
            store,
            active_vault: DEFAULT_VAULT_NAME.to_string(),
            vault: None,
            vault_key: None,
            generator_length: 16,
            generator_lowercase: true,
            generator_uppercase: true,
            generator_digits: true,
            generator_symbols: true,
            show_add_entry: false,
            vault_entries: Vec::new(),
            entry_title: String::new(),
            entry_username: String::new(),
            entry_password: String::new(),
            entry_url: String::new(),
            entry_notes: String::new(),
            settings: AppSettings::default(),
            show_settings: false,
            theme: LilypadTheme::ClassicGreen,
            lockout_state: LockoutState::default(),
            last_activity: now,
            clipboard_clear_time: None,
            clipboard_value: None,
            show_reauth_modal: false,
            reauth_password: String::new(),
            pending_copy_password: None,
            show_delete_confirm: false,
            pending_delete_index: None,
            edit_mode: false,
            edit_index: None,
            available_vaults: Vec::new(),
            show_vault_selector: false,
            show_new_vault_modal: false,
            new_vault_name: String::new(),
            account_display_name: "Avery Quinn".to_string(),
            account_email: "avery@lilypad.app".to_string(),
            account_timezone: "Europe/Paris".to_string(),
            account_two_factor_enabled: true,
            account_marketing_opt_in: false,
            security_recovery_email: "recovery@lilypad.app".to_string(),
            security_trusted_devices: vec![
                "MacBook Pro - Paris".to_string(),
                "iPhone 15 - Bordeaux".to_string(),
            ],
            health_report: None,
        };

        // Load persisted state
        if let Some(project_dirs) = ProjectDirs::from("", "", "Lilypad") {
            let config_dir = project_dirs.config_dir();
            let welcome_ack_path = config_dir.join("welcome_ack");
            let settings_path = config_dir.join("settings.json");
            let lockout_path = config_dir.join("lockout.json");

            app.welcome_ack_path = Some(welcome_ack_path.clone());
            app.settings_path = Some(settings_path.clone());
            app.lockout_path = Some(lockout_path.clone());

            // Load welcome acknowledgement
            if let Ok(contents) = fs::read_to_string(&welcome_ack_path) {
                if contents.trim() == "acknowledged=true" {
                    app.show_welcome = false;
                }
            }

            // Load persisted settings
            if let Ok(contents) = fs::read_to_string(&settings_path) {
                if let Ok(mut settings) = serde_json::from_str::<AppSettings>(&contents) {
                    if settings.version < SETTINGS_VERSION {
                        settings.version = SETTINGS_VERSION;
                    }
                    app.theme = LilypadTheme::from_index(settings.theme_index);
                    app.settings = settings;
                }
            }

            // Load persisted lockout state
            if let Ok(contents) = fs::read_to_string(&lockout_path) {
                if let Ok(lockout) = serde_json::from_str::<LockoutState>(&contents) {
                    app.lockout_state = lockout;
                }
            }
        }

        // Load available vaults
        app.load_available_vaults();

        (app, Task::none())
    }

    /// Get the title for the application window (may be used by Iced for window title)
    #[allow(dead_code)]
    pub fn title(&self) -> String {
        if self.vault_unlocked {
            format!("Lilypad - {}", self.active_vault)
        } else {
            "Lilypad".to_string()
        }
    }

    /// Handle incoming messages
    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Record activity on most messages
        match &message {
            Message::Tick | Message::None => {}
            _ => self.record_activity(),
        }

        match message {
            // Navigation
            Message::SelectCategory(cat) => {
                self.selected_category = cat;
            }
            Message::SetViewMode(mode) => {
                self.vault_view_mode = mode;
            }

            // Authentication
            Message::MasterPasswordChanged(pwd) => {
                self.master_password = pwd;
                self.error_message = None;
            }
            Message::UnlockVault => {
                return self.try_unlock_vault();
            }
            Message::LockVault => {
                self.lock_vault();
            }
            Message::AcknowledgeWelcome => {
                self.show_welcome = false;
                self.save_welcome_ack();
            }

            // Search
            Message::SearchChanged(query) => {
                self.search_query = query;
            }
            Message::ClearSearch => {
                self.search_query.clear();
            }

            // Entry Management
            Message::ShowAddEntry => {
                self.show_add_entry = true;
                self.edit_mode = false;
                self.clear_entry_form();
            }
            Message::HideAddEntry => {
                self.show_add_entry = false;
                self.edit_mode = false;
                self.clear_entry_form();
            }
            Message::EntryTitleChanged(title) => {
                self.entry_title = title;
            }
            Message::EntryUsernameChanged(username) => {
                self.entry_username = username;
            }
            Message::EntryPasswordChanged(password) => {
                self.entry_password = password;
            }
            Message::EntryUrlChanged(url) => {
                self.entry_url = url;
            }
            Message::EntryNotesChanged(notes) => {
                self.entry_notes = notes;
            }
            Message::SaveEntry => {
                return self.save_entry();
            }
            Message::EditEntry(index) => {
                self.start_edit(index);
            }
            Message::DeleteEntry(index) => {
                self.pending_delete_index = Some(index);
                self.show_delete_confirm = true;
            }
            Message::ConfirmDelete => {
                return self.confirm_delete();
            }
            Message::CancelDelete => {
                self.show_delete_confirm = false;
                self.pending_delete_index = None;
            }
            Message::ToggleFavorite(index) => {
                return self.toggle_favorite(index);
            }
            Message::CopyUsername(index) => {
                if let Some(entry) = self.vault_entries.get(index) {
                    self.copy_to_clipboard(&entry.username.clone());
                    self.set_status("Username copied to clipboard");
                }
            }
            Message::CopyPassword(index) => {
                if self.settings.require_master_on_copy {
                    if let Some(entry) = self.vault_entries.get(index) {
                        self.pending_copy_password = Some(entry.password.clone());
                        self.show_reauth_modal = true;
                    }
                } else if let Some(entry) = self.vault_entries.get(index) {
                    self.copy_to_clipboard(&entry.password.clone());
                    self.set_status("Password copied to clipboard");
                }
            }
            Message::OpenUrl(index) => {
                if let Some(entry) = self.vault_entries.get(index) {
                    if !entry.url.is_empty() {
                        let _ = webbrowser::open(&entry.url);
                    }
                }
            }

            // Re-auth modal
            Message::ShowReauthModal(password) => {
                self.pending_copy_password = Some(password);
                self.show_reauth_modal = true;
            }
            Message::ReauthPasswordChanged(pwd) => {
                self.reauth_password = pwd;
            }
            Message::ConfirmReauth => {
                if self.verify_master_password(&self.reauth_password.clone()) {
                    if let Some(pwd) = self.pending_copy_password.take() {
                        self.copy_to_clipboard(&pwd);
                        self.set_status("Password copied to clipboard");
                    }
                    self.show_reauth_modal = false;
                    self.reauth_password.clear();
                } else {
                    self.set_status("Invalid password");
                }
            }
            Message::CancelReauth => {
                self.show_reauth_modal = false;
                self.reauth_password.clear();
                self.pending_copy_password = None;
            }

            // Password Generator
            Message::GeneratePassword => {
                self.generate_password();
            }
            Message::CopyGeneratedPassword => {
                if !self.generated_password.is_empty() {
                    self.copy_to_clipboard(&self.generated_password.clone());
                    self.set_status("Password copied to clipboard");
                }
            }
            Message::UseGeneratedPassword => {
                self.generate_password();
                self.entry_password = self.generated_password.clone();
            }
            Message::GeneratorLengthChanged(len) => {
                self.generator_length = len;
            }
            Message::ToggleLowercase(v) => {
                self.generator_lowercase = v;
            }
            Message::ToggleUppercase(v) => {
                self.generator_uppercase = v;
            }
            Message::ToggleDigits(v) => {
                self.generator_digits = v;
            }
            Message::ToggleSymbols(v) => {
                self.generator_symbols = v;
            }
            Message::ToggleExcludeAmbiguous(v) => {
                self.settings.exclude_ambiguous_chars = v;
                self.save_settings();
            }

            // Vault Management
            Message::ShowVaultSelector => {
                self.show_vault_selector = true;
            }
            Message::HideVaultSelector => {
                self.show_vault_selector = false;
            }
            Message::SelectVault(name) => {
                self.show_vault_selector = false;
                if name != self.active_vault {
                    self.switch_vault(&name);
                }
            }
            Message::ShowNewVaultModal => {
                self.show_new_vault_modal = true;
                self.show_vault_selector = false;
                self.new_vault_name.clear();
            }
            Message::HideNewVaultModal => {
                self.show_new_vault_modal = false;
                self.new_vault_name.clear();
            }
            Message::NewVaultNameChanged(name) => {
                self.new_vault_name = name;
            }
            Message::CreateVault => {
                return self.create_new_vault();
            }

            // Settings
            Message::ShowSettings => {
                self.show_settings = true;
            }
            Message::HideSettings => {
                self.show_settings = false;
            }
            Message::ChangeTheme(theme) => {
                self.theme = theme;
                self.settings.theme_index = theme.to_index();
                self.save_settings();
            }
            Message::ChangeAutoLock(minutes) => {
                self.settings.auto_lock_minutes = minutes;
                self.save_settings();
            }
            Message::ChangeClipboardTimeout(seconds) => {
                self.settings.clipboard_timeout_seconds = seconds;
                self.save_settings();
            }
            Message::ToggleSecurityAlerts(v) => {
                self.settings.send_security_alerts = v;
                self.save_settings();
            }
            Message::ToggleRequireMasterOnCopy(v) => {
                self.settings.require_master_on_copy = v;
                self.save_settings();
            }

            // Account Settings
            Message::DisplayNameChanged(name) => {
                self.account_display_name = name;
            }
            Message::EmailChanged(email) => {
                self.account_email = email;
            }
            Message::TimezoneChanged(tz) => {
                self.account_timezone = tz;
            }
            Message::ToggleTwoFactor(v) => {
                self.account_two_factor_enabled = v;
            }
            Message::ToggleMarketingOptIn(v) => {
                self.account_marketing_opt_in = v;
            }
            Message::RecoveryEmailChanged(email) => {
                self.security_recovery_email = email;
            }
            Message::RemoveTrustedDevice(index) => {
                if index < self.security_trusted_devices.len() {
                    self.security_trusted_devices.remove(index);
                }
            }

            // Health Dashboard
            Message::RefreshHealthReport => {
                self.refresh_health_report();
            }

            // System
            Message::Tick => {
                self.check_auto_lock();
                self.check_clipboard_clear();
                self.check_status_clear();
            }
            Message::ClearStatus => {
                self.status_message = None;
                self.status_message_time = None;
            }
            Message::SetStatus(msg) => {
                self.set_status(msg);
            }
            Message::OpenExternalLink(url) => {
                let _ = webbrowser::open(&url);
            }

            // File Operations
            Message::ImportVault | Message::ExportVault | Message::FileSelected(_) => {
                // TODO: Implement file operations
            }

            Message::None => {}
        }

        Task::none()
    }

    /// Create the view
    pub fn view(&self) -> Element<'_, Message> {
        if self.show_welcome {
            return views::welcome::view(self.theme);
        }

        if !self.vault_unlocked {
            return views::unlock::view(
                self.theme,
                &self.master_password,
                &self.lockout_state,
                self.error_message.as_deref(),
            );
        }

        // Main application layout
        let header = views::header::view(
            self.theme,
            &self.active_vault,
            &self.available_vaults,
            &self.search_query,
            self.show_vault_selector,
        );

        let main_content: Element<Message> = match Category::from_index(self.selected_category) {
            Category::Credentials => views::vault::view(
                self.theme,
                &self.vault_entries,
                &self.search_query,
                self.vault_view_mode,
                self.show_add_entry,
                self.edit_mode,
                &self.entry_title,
                &self.entry_username,
                &self.entry_password,
                &self.entry_url,
                &self.entry_notes,
            ),
            Category::Health => views::health::view(self.theme, self.health_report.as_ref()),
            Category::Generator => views::generator::view(
                self.theme,
                &self.generated_password,
                self.generator_length,
                self.generator_lowercase,
                self.generator_uppercase,
                self.generator_digits,
                self.generator_symbols,
                self.settings.exclude_ambiguous_chars,
            ),
            Category::Account => views::settings::account_view(
                self.theme,
                &self.account_display_name,
                &self.account_email,
                &self.account_timezone,
                self.account_two_factor_enabled,
                self.account_marketing_opt_in,
            ),
            Category::Security => views::settings::security_view(
                self.theme,
                &self.security_recovery_email,
                &self.security_trusted_devices,
                self.settings.auto_lock_minutes,
                self.settings.clipboard_timeout_seconds,
                self.settings.require_master_on_copy,
            ),
        };

        let navigation = views::navigation::view(self.theme, self.selected_category);

        // Status bar
        let status_bar: Element<Message> = if let Some(ref msg) = self.status_message {
            let palette = self.theme.palette();
            container(
                iced::widget::text(msg)
                    .size(13)
                    .color(palette.text_secondary),
            )
            .width(Length::Fill)
            .padding([8, 20])
            .style(move |_| theme::nav_container(self.theme))
            .into()
        } else {
            Space::new(0, 0).into()
        };

        let layout = column![header, main_content, navigation, status_bar,];

        // Layer modals on top
        let content: Element<Message> = container(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| theme::app_container(self.theme))
            .into();

        // Add modal overlays
        if self.show_settings {
            return iced::widget::stack![
                content,
                views::modals::settings_modal(self.theme, self.theme),
            ]
            .into();
        }

        if self.show_new_vault_modal {
            return iced::widget::stack![
                content,
                views::modals::new_vault_modal(self.theme, &self.new_vault_name),
            ]
            .into();
        }

        if self.show_delete_confirm {
            let title = self
                .pending_delete_index
                .and_then(|i| self.vault_entries.get(i))
                .map(|e| e.title.as_str())
                .unwrap_or("this entry");

            return iced::widget::stack![
                content,
                views::modals::delete_confirm_modal(self.theme, title),
            ]
            .into();
        }

        if self.show_reauth_modal {
            return iced::widget::stack![
                content,
                views::modals::reauth_modal(self.theme, &self.reauth_password),
            ]
            .into();
        }

        content
    }

    /// Create subscriptions
    pub fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    fn record_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    fn check_auto_lock(&mut self) {
        if !self.vault_unlocked {
            return;
        }
        let timeout_secs = (self.settings.auto_lock_minutes as u64) * 60;
        if timeout_secs == 0 {
            return;
        }
        if self.last_activity.elapsed() > Duration::from_secs(timeout_secs) {
            self.lock_vault();
            self.set_status("Vault auto-locked due to inactivity");
        }
    }

    fn lock_vault(&mut self) {
        self.vault_unlocked = false;
        self.vault = None;
        self.vault_key = None;
        self.vault_entries.clear();
        self.master_password.clear();
        self.show_add_entry = false;
        self.show_settings = false;
    }

    fn check_clipboard_clear(&mut self) {
        if let (Some(clear_time), Some(value)) = (&self.clipboard_clear_time, &self.clipboard_value)
        {
            if Instant::now() >= *clear_time {
                if let Ok(mut clipboard) = Clipboard::new() {
                    if clipboard.get_text().ok().as_deref() == Some(value) {
                        let _ = clipboard.set_text(String::new());
                    }
                }
                self.clipboard_clear_time = None;
                self.clipboard_value = None;
            }
        }
    }

    fn copy_to_clipboard(&mut self, value: &str) {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(value.to_string());
            if self.settings.clipboard_timeout_seconds > 0 {
                self.clipboard_value = Some(value.to_string());
                self.clipboard_clear_time = Some(
                    Instant::now()
                        + Duration::from_secs(self.settings.clipboard_timeout_seconds as u64),
                );
            }
        }
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
        self.status_message_time = Some(Instant::now());
    }

    fn check_status_clear(&mut self) {
        if let Some(time) = self.status_message_time {
            if time.elapsed() > Duration::from_secs(5) {
                self.status_message = None;
                self.status_message_time = None;
            }
        }
    }

    fn save_settings(&self) {
        if let Some(path) = &self.settings_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&self.settings) {
                let _ = fs::write(path, json);
            }
        }
    }

    fn save_welcome_ack(&self) {
        if let Some(path) = &self.welcome_ack_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(path, "acknowledged=true");
        }
    }

    fn save_lockout(&self) {
        if let Some(path) = &self.lockout_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(&self.lockout_state) {
                let _ = fs::write(path, json);
            }
        }
    }

    fn verify_master_password(&self, password: &str) -> bool {
        password == self.master_password
    }

    fn try_unlock_vault(&mut self) -> Task<Message> {
        if self.lockout_state.is_locked_out() {
            self.error_message = Some(format!(
                "Locked out. Try again in {} seconds.",
                self.lockout_state.remaining_secs()
            ));
            return Task::none();
        }

        // Try to load and decrypt vault
        match self.unlock_vault_internal() {
            Ok(()) => {
                self.vault_unlocked = true;
                self.lockout_state.reset();
                self.save_lockout();
                self.error_message = None;
                self.refresh_health_report();
            }
            Err(e) => {
                self.lockout_state.record_failure();
                self.save_lockout();
                self.error_message = Some(e.to_string());
            }
        }

        Task::none()
    }

    fn key_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.config.data_dir).join("key.json")
    }

    fn unlock_vault_internal(&mut self) -> anyhow::Result<()> {
        let password = self.master_password.trim();
        if password.is_empty() {
            return Err(anyhow::anyhow!("Master password is required"));
        }

        let path = self.key_path();
        let (key, key_file) = if path.exists() {
            // Load existing key
            load_key(&path, Some(password))?
        } else {
            // Create new key from password
            let params = KeyDerivationParams::generate();
            let key = derive_key(password, &params)?;
            let key_file = KeyFile::from_kdf(params);
            save_key(&path, &key_file)?;
            (key, key_file)
        };

        // Load or create vault
        let vault = if let Ok(v) = self.store.load_vault(&self.active_vault, &key) {
            v
        } else {
            // Create new vault with key metadata
            let metadata = match &key_file {
                KeyFile::Kdf { .. } => {
                    KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305).with_kdf("argon2id")
                }
                KeyFile::Raw { .. } => KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305),
            };
            let v = Vault::new(&self.active_vault, metadata);
            self.store.save_vault(&v, &key)?;
            v
        };

        // Decrypt entries and build UI representation
        self.vault_entries = vault
            .entries
            .iter()
            .filter_map(|entry| {
                let decrypted = decrypt(&key, &entry.ciphertext).ok()?;
                let payload: DesktopEntryPayload = serde_json::from_slice(&decrypted).ok()?;

                let strength = validate_password_strength(&payload.password);

                // Check if password is expired (older than 90 days)
                let password_age_days = entry.password_age_days();
                let is_expired = password_age_days > 90;

                Some(VaultEntry {
                    title: entry.label.clone(),
                    username: payload.username,
                    password: payload.password,
                    url: payload.url,
                    notes: payload.notes,
                    last_updated: format_timestamp_relative(entry.updated_at),
                    updated_at: entry.updated_at,
                    is_favorite: entry.is_favorite,
                    last_accessed_at: entry.last_accessed_at,
                    access_count: entry.access_count,
                    password_strength: strength,
                    is_expired,
                    color: entry.color,
                })
            })
            .collect();

        self.vault = Some(vault);
        self.vault_key = Some(key);

        Ok(())
    }

    fn load_available_vaults(&mut self) {
        self.available_vaults = self.store.list_vaults().unwrap_or_default();
        if self.available_vaults.is_empty() {
            self.available_vaults.push(DEFAULT_VAULT_NAME.to_string());
        }
    }

    fn switch_vault(&mut self, name: &str) {
        self.active_vault = name.to_string();
        self.vault = None;
        self.vault_key = None;
        self.vault_entries.clear();
        self.vault_unlocked = false;
    }

    fn create_new_vault(&mut self) -> Task<Message> {
        let name = self.new_vault_name.trim().to_string();
        if name.is_empty() {
            return Task::none();
        }

        // Just register the vault name - actual vault file will be created on first unlock
        // This is because we need a key to encrypt the vault, and we don't have one yet
        if !self.available_vaults.contains(&name) {
            self.available_vaults.push(name.clone());
            self.show_new_vault_modal = false;
            self.new_vault_name.clear();
            self.set_status(format!("Vault '{}' created. Switch to it and enter your password.", name));
        } else {
            self.set_status("A vault with this name already exists");
        }

        Task::none()
    }

    fn clear_entry_form(&mut self) {
        self.entry_title.clear();
        self.entry_username.clear();
        self.entry_password.clear();
        self.entry_url.clear();
        self.entry_notes.clear();
        self.edit_index = None;
    }

    fn start_edit(&mut self, index: usize) {
        if let Some(entry) = self.vault_entries.get(index) {
            self.entry_title = entry.title.clone();
            self.entry_username = entry.username.clone();
            self.entry_password = entry.password.clone();
            self.entry_url = entry.url.clone();
            self.entry_notes = entry.notes.clone();
            self.edit_mode = true;
            self.edit_index = Some(index);
            self.show_add_entry = true;
        }
    }

    fn save_entry(&mut self) -> Task<Message> {
        let Some(ref key) = self.vault_key else {
            return Task::none();
        };
        let Some(ref mut vault) = self.vault else {
            return Task::none();
        };

        let payload = DesktopEntryPayload {
            username: self.entry_username.clone(),
            password: self.entry_password.clone(),
            url: self.entry_url.clone(),
            notes: self.entry_notes.clone(),
        };

        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
        let ciphertext = match encrypt(key, &payload_bytes) {
            Ok(c) => c,
            Err(_) => return Task::none(),
        };

        let was_edit_mode = self.edit_mode;

        if was_edit_mode {
            if let Some(index) = self.edit_index {
                if let Some(entry) = vault.entries.get(index) {
                    let label = entry.label.clone();
                    // Update entry ciphertext
                    let _ = vault.update_entry(&label, ciphertext);
                }
            }
        } else {
            // Create new entry with metadata
            let metadata = EntryMetadata {
                username: Some(self.entry_username.clone()),
                url: if self.entry_url.is_empty() {
                    None
                } else {
                    Some(self.entry_url.clone())
                },
                ..Default::default()
            };
            let entry = Entry::new_with_metadata(&self.entry_title, metadata, ciphertext);
            let _ = vault.add_entry(entry);
        }

        let _ = self.store.save_vault(vault, key);

        // Refresh entries
        let _ = self.unlock_vault_internal();

        self.show_add_entry = false;
        self.edit_mode = false;
        self.clear_entry_form();
        self.set_status(if was_edit_mode {
            "Entry updated"
        } else {
            "Entry added"
        });

        Task::none()
    }

    fn confirm_delete(&mut self) -> Task<Message> {
        let Some(index) = self.pending_delete_index else {
            return Task::none();
        };
        let Some(ref mut vault) = self.vault else {
            return Task::none();
        };
        let Some(ref key) = self.vault_key else {
            return Task::none();
        };

        if index < vault.entries.len() {
            let label = vault.entries[index].label.clone();
            let _ = vault.remove_entry(&label);
            let _ = self.store.save_vault(vault, key);
            let _ = self.unlock_vault_internal();
            self.set_status("Entry deleted");
        }

        self.show_delete_confirm = false;
        self.pending_delete_index = None;

        Task::none()
    }

    fn toggle_favorite(&mut self, index: usize) -> Task<Message> {
        let Some(ref mut vault) = self.vault else {
            return Task::none();
        };
        let Some(ref key) = self.vault_key else {
            return Task::none();
        };

        if let Some(entry) = vault.entries.get(index) {
            let label = entry.label.clone();
            let new_favorite = !entry.is_favorite;
            let _ = vault.set_entry_favorite(&label, new_favorite);
            let _ = self.store.save_vault(vault, key);
            let _ = self.unlock_vault_internal();
        }

        Task::none()
    }

    fn generate_password(&mut self) {
        let mut charset = String::new();

        if self.generator_lowercase {
            if self.settings.exclude_ambiguous_chars {
                charset.push_str("abcdefghjkmnpqrstuvwxyz");
            } else {
                charset.push_str("abcdefghijklmnopqrstuvwxyz");
            }
        }

        if self.generator_uppercase {
            if self.settings.exclude_ambiguous_chars {
                charset.push_str("ABCDEFGHJKMNPQRSTUVWXYZ");
            } else {
                charset.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
            }
        }

        if self.generator_digits {
            if self.settings.exclude_ambiguous_chars {
                charset.push_str("23456789");
            } else {
                charset.push_str("0123456789");
            }
        }

        if self.generator_symbols {
            charset.push_str("!@#$%^&*()_+-=[]{}|;:,.<>?");
        }

        if charset.is_empty() {
            charset.push_str("abcdefghijklmnopqrstuvwxyz");
        }

        let chars: Vec<char> = charset.chars().collect();
        let mut rng = rand::thread_rng();
        self.generated_password = (0..self.generator_length)
            .map(|_| chars[rng.gen_range(0..chars.len())])
            .collect();
    }

    fn refresh_health_report(&mut self) {
        let health_data: Vec<EntryHealthData> = self
            .vault_entries
            .iter()
            .map(|e| {
                // Calculate password age in days
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let password_age_days = (now.saturating_sub(e.updated_at)) / (24 * 60 * 60);

                EntryHealthData {
                    label: e.title.clone(),
                    password: e.password.clone(),
                    has_username: !e.username.is_empty(),
                    has_url: !e.url.is_empty(),
                    has_totp: false, // TODO: Check if entry has TOTP
                    password_age_days,
                    days_until_expiry: None, // TODO: Get from entry if set
                    is_expired: e.is_expired,
                }
            })
            .collect();

        self.health_report = Some(analyze_vault_health(&health_data));
    }
}
