//! Lilypad Application
//!
//! Main application struct implementing Iced's Application trait.

use std::fs;
use std::time::{Duration, Instant};

use directories::ProjectDirs;
use iced::widget::{column, container};
use iced::alignment::{Horizontal, Vertical};
use iced::{Color, Element, Length, Subscription, Task};
use rand::RngExt;

use lilypad_common::{
    analyze_vault_health,
    keyfile::{load_key, save_key, KeyFile},
    time::format_timestamp_relative,
    validation::validate_password_strength,
    EntryHealthData,
};
use lilypad_core::{
    decrypt, default_config, derive_key, encrypt, AppConfig, CryptoAlgorithm, Entry,
    EntryMetadata, EntrySecret, KeyDerivationParams, KeyMaterial, KeyMetadata, Vault,
};
use lilypad_storage::LocalStore;
use zeroize::Zeroize;

use crate::message::Message;
use crate::state::{
    AppSettings, Category, LockoutState, OnboardingStep, TutorialPhase, UnlockMode, VaultEntry,
    VaultViewMode, DEFAULT_VAULT_NAME, SETTINGS_VERSION,
};
use crate::theme::{self, LilypadTheme, UiVariation};
use crate::views;

/// Convert a string to `Some(string)` if non-empty, or `None` otherwise.
fn non_empty(s: impl Into<String> + AsRef<str>) -> Option<String> {
    if s.as_ref().is_empty() { None } else { Some(s.into()) }
}

/// Main Lilypad application
pub struct LilypadApp {
    // Application mode
    pub show_welcome: bool,
    pub onboarding_step: OnboardingStep,
    pub tutorial_phase: TutorialPhase,
    pub tutorial_generated_pw: String,
    pub tutorial_pw_copied: bool,
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

    // Unlock mode
    pub unlock_mode: UnlockMode,

    // Sensitive data
    pub master_password: String,
    pub confirm_password: String,
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
    pub entry_email: String,
    pub entry_phone: String,
    pub entry_folder: String,
    pub entry_tags: Vec<String>,
    pub entry_new_tag: String,
    pub entry_totp_secret: String,
    pub entry_custom_fields: Vec<(String, String)>,
    pub entry_attachments: Vec<(String, String)>, // (filename, base64_data)

    // Settings
    pub settings: AppSettings,
    pub show_settings: bool,
    pub theme: LilypadTheme,
    pub ui_variation: UiVariation,
    pub theme_sort_by_color: bool,

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
    pub renaming_vault: Option<String>,
    pub rename_vault_input: String,

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

    // GitHub OAuth & Sync
    pub github_username: Option<String>,
    pub github_authenticated: bool,
    pub sync_status_text: Option<String>,
    pub sync_in_progress: bool,
    pub sync_started_at: Option<Instant>,
    pub device_flow_code: Option<String>,
    pub device_flow_uri: Option<String>,
    pub device_flow_device_code: Option<String>,
    pub device_flow_interval: Option<u64>,

    // Key rotation
    pub show_change_password: bool,
    pub new_master_password: String,

    // Entry history view
    pub show_entry_history: bool,
    pub history_entries: Vec<(u64, String)>,

    // Breach check
    pub breached_entries: Vec<String>,

    // Folder filter
    pub folder_filter: Option<String>,

    // Sync conflict
    pub sync_conflict: bool,

    // Audit log
    pub show_audit_log: bool,
    pub audit_events: Vec<(u64, String, Option<String>)>,

    // Entry type for form
    pub entry_type: String,
    // Whether the advanced fields section is expanded in the entry form
    pub show_advanced_fields: bool,

    // UI interaction state
    pub hovered_entry_index: Option<usize>,
}

impl Drop for LilypadApp {
    fn drop(&mut self) {
        self.master_password.zeroize();
        self.confirm_password.zeroize();
        self.generated_password.zeroize();
        self.tutorial_generated_pw.zeroize();
        self.entry_password.zeroize();
        self.reauth_password.zeroize();
        self.new_master_password.zeroize();
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
        // Use platform-specific config directory for all data:
        //   Linux:   ~/.config/Colony/Lilypad/
        //   Windows: %LOCALAPPDATA%\Colony\Lilypad\
        //   macOS:   ~/Library/Application Support/Colony/Lilypad/
        let data_dir = ProjectDirs::from_path(std::path::PathBuf::from("Colony/Lilypad"))
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from(".lilypad"));
        if let Err(e) = fs::create_dir_all(&data_dir) {
            eprintln!("Warning: failed to create data directory: {e}");
        }
        let config = AppConfig {
            data_dir: data_dir.to_string_lossy().to_string(),
            ..default_config()
        };
        let store = match LocalStore::new(&config) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to initialize store: {e}");
                match LocalStore::new(&default_config()) {
                    Ok(s) => s,
                    Err(e2) => {
                        eprintln!("Fatal: fallback store also failed: {e2}");
                        std::process::exit(1);
                    }
                }
            }
        };
        let now = Instant::now();

        let mut app = Self {
            show_welcome: true,
            onboarding_step: OnboardingStep::Welcome,
            tutorial_phase: TutorialPhase::GeneratePassword,
            tutorial_generated_pw: String::new(),
            tutorial_pw_copied: false,
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
            unlock_mode: UnlockMode::Unlock,
            master_password: String::new(),
            confirm_password: String::new(),
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
            entry_email: String::new(),
            entry_phone: String::new(),
            entry_folder: String::new(),
            entry_tags: Vec::new(),
            entry_new_tag: String::new(),
            entry_totp_secret: String::new(),
            entry_custom_fields: Vec::new(),
            entry_attachments: Vec::new(),
            settings: AppSettings::default(),
            show_settings: false,
            theme: LilypadTheme::ClassicGreen,
            ui_variation: UiVariation::Default,
            theme_sort_by_color: false,
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
            renaming_vault: None,
            rename_vault_input: String::new(),
            account_display_name: String::new(),
            account_email: String::new(),
            account_timezone: String::new(),
            account_two_factor_enabled: false,
            account_marketing_opt_in: false,
            security_recovery_email: String::new(),
            security_trusted_devices: Vec::new(),
            health_report: None,
            github_username: None,
            github_authenticated: false,
            sync_status_text: None,
            sync_in_progress: false,
            sync_started_at: None,
            device_flow_code: None,
            device_flow_uri: None,
            device_flow_device_code: None,
            device_flow_interval: None,
            show_change_password: false,
            new_master_password: String::new(),
            show_entry_history: false,
            history_entries: Vec::new(),
            breached_entries: Vec::new(),
            folder_filter: None,
            sync_conflict: false,
            show_audit_log: false,
            audit_events: Vec::new(),
            entry_type: "Login".to_string(),
            show_advanced_fields: false,
            hovered_entry_index: None,
        };

        // Check GitHub OAuth status on startup
        if let Ok(has_token) = lilypad_oauth::GitHubSyncBackend::has_valid_token() {
            if has_token {
                app.github_authenticated = true;
                let token_store = lilypad_oauth::TokenStoreManager::new().ok();
                if let Some(store) = token_store {
                    if let Ok(Some(token)) = store.load_token(lilypad_oauth::OAuthProvider::GitHub) {
                        app.github_username = token.username.clone();
                    }
                }
            }
        }

        // Load persisted state
        if let Some(project_dirs) = ProjectDirs::from_path(std::path::PathBuf::from("Colony/Lilypad")) {
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
                    app.ui_variation = UiVariation::from_index(settings.ui_variation_index);
                    // Restore account settings from persisted state
                    app.account_display_name = settings.account_display_name.clone();
                    app.account_email = settings.account_email.clone();
                    app.account_timezone = settings.account_timezone.clone();
                    app.account_two_factor_enabled = settings.account_two_factor_enabled;
                    app.account_marketing_opt_in = settings.account_marketing_opt_in;
                    app.security_recovery_email = settings.security_recovery_email.clone();
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

        // Load available vaults and determine unlock mode
        app.load_available_vaults();
        app.determine_unlock_mode();

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
                // Close open forms/modals when switching tabs
                self.show_add_entry = false;
                self.show_vault_selector = false;
                self.edit_mode = false;
            }
            Message::SetViewMode(mode) => {
                self.vault_view_mode = mode;
            }

            // Authentication
            Message::MasterPasswordChanged(pwd) => {
                self.master_password = pwd;
                self.error_message = None;
            }
            Message::ConfirmPasswordChanged(pwd) => {
                self.confirm_password = pwd;
                self.error_message = None;
            }
            Message::UnlockVault => {
                return self.try_unlock_vault();
            }
            Message::CreateVaultWithPassword => {
                return self.create_vault_with_password();
            }
            Message::LockVault => {
                self.lock_vault();
            }
            Message::AcknowledgeWelcome | Message::OnboardingSkip => {
                self.show_welcome = false;
                self.save_welcome_ack();
            }
            Message::OnboardingNext => {
                if let Some(next) = self.onboarding_step.next() {
                    if next == OnboardingStep::Tutorial {
                        self.tutorial_phase = TutorialPhase::GeneratePassword;
                        self.tutorial_generated_pw.clear();
                        self.tutorial_pw_copied = false;
                    }
                    self.onboarding_step = next;
                }
            }
            Message::OnboardingPrev => {
                if let Some(prev) = self.onboarding_step.prev() {
                    self.onboarding_step = prev;
                }
            }
            Message::OnboardingGoTo(index) => {
                self.onboarding_step = OnboardingStep::from_index(index);
            }
            Message::OnboardingTutorialGenerate => {
                self.tutorial_generated_pw = self.generate_demo_password();
                self.tutorial_phase = TutorialPhase::ViewVault;
            }
            Message::OnboardingTutorialCopy => {
                self.copy_to_clipboard(&self.tutorial_generated_pw.clone());
                self.tutorial_pw_copied = true;
            }
            Message::OnboardingTutorialNext => {
                if let Some(next_phase) = self.tutorial_phase.next() {
                    self.tutorial_phase = next_phase;
                }
            }
            Message::OnboardingEnsureRepo => {
                return self.ensure_github_repo();
            }
            Message::OnboardingRepoResult(result) => {
                match result {
                    Ok(msg) => self.set_status(msg),
                    Err(e) => self.set_status(format!("Repository setup failed: {}", e)),
                }
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
            Message::EntryEmailChanged(email) => {
                self.entry_email = email;
            }
            Message::EntryPhoneChanged(phone) => {
                self.entry_phone = phone;
            }
            Message::EntryFolderChanged(folder) => {
                self.entry_folder = folder;
            }
            Message::EntryNewTagChanged(tag) => {
                self.entry_new_tag = tag;
            }
            Message::AddEntryTag => {
                let tag = self.entry_new_tag.trim().to_string();
                if !tag.is_empty() && !self.entry_tags.contains(&tag) {
                    self.entry_tags.push(tag);
                    self.entry_new_tag.clear();
                }
            }
            Message::RemoveEntryTag(index) => {
                if index < self.entry_tags.len() {
                    self.entry_tags.remove(index);
                }
            }
            Message::EntryTotpSecretChanged(secret) => {
                self.entry_totp_secret = secret;
            }
            Message::AddCustomField => {
                self.entry_custom_fields
                    .push((String::new(), String::new()));
            }
            Message::RemoveCustomField(index) => {
                if index < self.entry_custom_fields.len() {
                    self.entry_custom_fields.remove(index);
                }
            }
            Message::CustomFieldNameChanged(index, name) => {
                if let Some(field) = self.entry_custom_fields.get_mut(index) {
                    field.0 = name;
                }
            }
            Message::CustomFieldValueChanged(index, value) => {
                if let Some(field) = self.entry_custom_fields.get_mut(index) {
                    field.1 = value;
                }
            }

            // Attachments
            Message::AddAttachment => {
                let file = rfd::FileDialog::new()
                    .set_title("Select Attachment")
                    .pick_file();
                if let Some(path) = file {
                    if let Ok(data) = fs::read(&path) {
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                        let filename = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "attachment".to_string());
                        self.entry_attachments.push((filename.clone(), b64));
                        self.set_status(format!("Attached: {}", filename));
                    } else {
                        self.set_status("Failed to read file");
                    }
                }
            }
            Message::RemoveAttachment(index) => {
                if index < self.entry_attachments.len() {
                    self.entry_attachments.remove(index);
                }
            }
            Message::AttachmentFileSelected(_) => {
                // Handled synchronously in AddAttachment
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
            Message::FilterByFolder(folder) => {
                self.folder_filter = folder;
            }
            Message::CopyTotpCode(index) => {
                let code = self
                    .vault_entries
                    .get(index)
                    .and_then(|e| e.totp_code.clone());
                if let Some(code) = code {
                    self.copy_to_clipboard(&code);
                    self.set_status("TOTP code copied to clipboard");
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
                    self.reauth_password.zeroize();
                } else {
                    self.set_status("Invalid password");
                }
            }
            Message::CancelReauth => {
                self.show_reauth_modal = false;
                self.reauth_password.zeroize();
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
            Message::DeleteVault(name) => {
                self.show_vault_selector = false;
                // Don't allow deleting the currently active vault if it's the only one
                if self.available_vaults.len() <= 1 {
                    self.set_status("Cannot delete the only vault");
                } else {
                    match self.store.delete_vault(&name) {
                        Ok(()) => {
                            self.available_vaults.retain(|v| v != &name);
                            // If we deleted the active vault, switch to the first available
                            if self.active_vault == name {
                                let next = self.available_vaults[0].clone();
                                self.switch_vault(&next);
                            }
                            self.set_status(format!("Vault '{}' deleted", name));
                        }
                        Err(e) => {
                            self.set_status(format!("Failed to delete vault: {e}"));
                        }
                    }
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
            Message::StartRenameVault(name) => {
                self.rename_vault_input = name.clone();
                self.renaming_vault = Some(name);
                self.show_vault_selector = false;
            }
            Message::RenameVaultNameChanged(name) => {
                self.rename_vault_input = name;
            }
            Message::ConfirmRenameVault => {
                if let Some(old_name) = self.renaming_vault.take() {
                    let new_name = self.rename_vault_input.trim().to_string();
                    if !new_name.is_empty() && new_name != old_name {
                        match self.store.rename_vault(&old_name, &new_name) {
                            Ok(()) => {
                                if let Some(v) = self.available_vaults.iter_mut().find(|v| *v == &old_name) {
                                    *v = new_name.clone();
                                }
                                if self.active_vault == old_name {
                                    self.active_vault = new_name.clone();
                                }
                                self.settings.active_vault = self.active_vault.clone();
                                self.save_settings();
                                self.set_status(format!("Vault renamed to '{}'", new_name));
                            }
                            Err(e) => {
                                self.set_status(format!("Failed to rename vault: {}", e));
                            }
                        }
                    }
                    self.rename_vault_input.clear();
                }
            }
            Message::CancelRenameVault => {
                self.renaming_vault = None;
                self.rename_vault_input.clear();
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
            Message::ChangeUiVariation(variation) => {
                self.ui_variation = variation;
                self.settings.ui_variation_index = variation.to_index();
                self.save_settings();
            }
            Message::ToggleThemeSort => {
                self.theme_sort_by_color = !self.theme_sort_by_color;
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
                self.account_display_name = name.clone();
                self.settings.account_display_name = name;
                self.save_settings();
            }
            Message::EmailChanged(email) => {
                self.account_email = email.clone();
                self.settings.account_email = email;
                self.save_settings();
            }
            Message::TimezoneChanged(tz) => {
                self.account_timezone = tz.clone();
                self.settings.account_timezone = tz;
                self.save_settings();
            }
            Message::ToggleTwoFactor(v) => {
                self.account_two_factor_enabled = v;
                self.settings.account_two_factor_enabled = v;
                self.save_settings();
            }
            Message::ToggleMarketingOptIn(v) => {
                self.account_marketing_opt_in = v;
                self.settings.account_marketing_opt_in = v;
                self.save_settings();
            }
            Message::RecoveryEmailChanged(email) => {
                self.security_recovery_email = email.clone();
                self.settings.security_recovery_email = email;
                self.save_settings();
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
                self.refresh_totp_codes();
                // Timeout sync operations after 10 seconds
                if self.sync_in_progress {
                    if let Some(started) = self.sync_started_at {
                        if started.elapsed() > Duration::from_secs(10) {
                            self.sync_in_progress = false;
                            self.sync_started_at = None;
                            self.set_status("Sync operation timed out");
                        }
                    }
                }
            }
            Message::EntryCardHovered(index) => {
                self.hovered_entry_index = Some(index);
            }
            Message::EntryCardUnhovered => {
                self.hovered_entry_index = None;
            }
            Message::ClearStatus => {
                self.status_message = None;
                self.status_message_time = None;
            }
            Message::SetStatus(msg) => {
                self.set_status(msg);
            }
            Message::CopyToClipboard(value) => {
                self.copy_to_clipboard(&value);
                self.set_status("Copied to clipboard");
            }
            Message::OpenExternalLink(url) => {
                let _ = webbrowser::open(&url);
            }

            // File Operations
            Message::ExportVault => {
                return self.export_vault_encrypted();
            }
            Message::ExportVaultJson => {
                return self.export_vault_json();
            }
            Message::ImportVault => {
                return self.import_vault_dialog();
            }
            Message::FileSelected(path) => {
                if let Some(path) = path {
                    return self.import_vault_file(path);
                }
            }

            // GitHub OAuth & Sync
            Message::GitHubLogin => {
                return self.github_login();
            }
            Message::GitHubLogout => {
                return self.github_logout();
            }
            Message::GitHubLoginResult(result) => {
                self.sync_in_progress = false;
                self.sync_started_at = None;
                match result {
                    Ok(username) => {
                        self.github_authenticated = true;
                        self.github_username = Some(username.clone());
                        self.device_flow_code = None;
                        self.device_flow_uri = None;
                        self.device_flow_device_code = None;
                        self.device_flow_interval = None;
                        self.set_status(format!("Logged in as {}", username));
                        // Auto-create repo during onboarding
                        if self.show_welcome {
                            return self.ensure_github_repo();
                        }
                    }
                    Err(e) => {
                        self.device_flow_code = None;
                        self.device_flow_uri = None;
                        self.device_flow_device_code = None;
                        self.device_flow_interval = None;
                        self.set_status(format!("Login failed: {}", e));
                    }
                }
            }
            Message::DeviceFlowCode {
                user_code,
                verification_uri,
                device_code,
                interval,
            } => {
                self.device_flow_code = Some(user_code);
                self.device_flow_uri = Some(verification_uri.clone());
                self.device_flow_device_code = Some(device_code.clone());
                self.device_flow_interval = Some(interval);
                // Open browser
                let _ = webbrowser::open(&verification_uri);
                // Start polling in background
                return self.github_poll_for_token(device_code, interval);
            }
            Message::SyncCheckStatus => {
                return self.sync_check_status();
            }
            Message::SyncPush => {
                return self.sync_push();
            }
            Message::SyncPull => {
                return self.sync_pull();
            }
            Message::SyncPullToUnlock => {
                return self.sync_pull_to_unlock();
            }
            Message::SyncCompleted(result) => {
                self.sync_in_progress = false;
                self.sync_started_at = None;
                match result {
                    Ok(msg) => {
                        self.set_status(&msg);
                        if self.vault_unlocked {
                            // Refresh entries after pull
                            self.reload_vault_from_disk();
                        } else {
                            // Vault was pulled but not unlocked yet
                            // If on welcome screen, skip to unlock
                            if self.show_welcome {
                                self.show_welcome = false;
                                self.save_welcome_ack();
                                self.load_available_vaults();
                            }
                            self.determine_unlock_mode();
                        }
                    }
                    Err(e) => {
                        self.set_status(format!("Sync error: {}", e));
                    }
                }
            }

            // CSV Export
            Message::ExportVaultCsv => {
                return self.export_vault_csv();
            }

            // Browser CSV Import
            Message::ImportBrowserCsv => {
                return self.import_browser_csv_dialog();
            }
            Message::BrowserCsvSelected(path) => {
                if let Some(path) = path {
                    return self.import_browser_csv_file(path);
                }
            }

            // Breach Check
            Message::CheckBreaches => {
                return self.check_breaches();
            }
            Message::BreachCheckCompleted(breached) => {
                self.breached_entries = breached.clone();
                if breached.is_empty() {
                    self.set_status("No breached passwords found!");
                } else {
                    self.set_status(format!("{} potentially breached passwords found", breached.len()));
                }
            }

            // Key Rotation
            Message::ChangeMasterPassword => {
                self.show_change_password = true;
                self.new_master_password = String::new();
            }
            Message::NewMasterPasswordChanged(p) => {
                self.new_master_password = p;
            }
            Message::ConfirmChangeMasterPassword => {
                return self.change_master_password();
            }
            Message::CancelChangeMasterPassword => {
                self.show_change_password = false;
                self.new_master_password.zeroize();
            }

            // Entry History
            Message::ViewEntryHistory(idx) => {
                self.view_entry_history(idx);
            }
            Message::CloseEntryHistory => {
                self.show_entry_history = false;
                self.history_entries.clear();
            }

            // Sync conflict resolution
            Message::SyncResolveKeepLocal => {
                self.sync_conflict = false;
                return self.sync_push();
            }
            Message::SyncResolveKeepRemote => {
                self.sync_conflict = false;
                return self.sync_pull();
            }

            // Entry type
            Message::EntryTypeChanged(t) => {
                self.entry_type = t;
            }
            Message::ToggleAdvancedFields => {
                self.show_advanced_fields = !self.show_advanced_fields;
            }

            // Audit log
            Message::ShowAuditLog => {
                // Load audit events from vault
                self.audit_events = if let Some(ref vault) = self.vault {
                    vault
                        .audit_log
                        .iter()
                        .rev()
                        .map(|e| (e.timestamp, e.action.clone(), e.entry_label.clone()))
                        .collect()
                } else {
                    Vec::new()
                };
                self.show_audit_log = true;
            }
            Message::CloseAuditLog => {
                self.show_audit_log = false;
            }
            Message::ExportAuditLogJson => {
                return self.export_audit_log_json();
            }
            Message::ExportAuditLogCsv => {
                return self.export_audit_log_csv();
            }
            Message::ExportAuditLogText => {
                return self.export_audit_log_text();
            }

            // Backup
            Message::BackupVault => {
                return self.backup_vault();
            }
            Message::RestoreVault => {
                return self.restore_vault_dialog();
            }
            Message::BackupFileSelected(path) => {
                if let Some(path) = path {
                    return self.restore_vault_file(path);
                }
            }
            Message::PruneBackups => {
                return self.prune_backups();
            }

            Message::None => {}
        }

        Task::none()
    }

    /// Create the view
    pub fn view(&self) -> Element<'_, Message> {
        let v = self.ui_variation;

        if self.show_welcome {
            return views::welcome::view(views::welcome::OnboardingParams {
                theme: self.theme,
                variation: v,
                step: self.onboarding_step,
                tutorial_phase: self.tutorial_phase,
                tutorial_generated_pw: &self.tutorial_generated_pw,
                tutorial_pw_copied: self.tutorial_pw_copied,
                github_authenticated: self.github_authenticated,
                github_username: self.github_username.as_deref(),
                sync_in_progress: self.sync_in_progress,
                device_flow_code: self.device_flow_code.as_deref(),
                device_flow_uri: self.device_flow_uri.as_deref(),
                master_password: &self.master_password,
                confirm_password: &self.confirm_password,
                error_message: self.error_message.as_deref(),
            });
        }

        if !self.vault_unlocked {
            return views::unlock::view(
                self.theme,
                v,
                &self.master_password,
                &self.confirm_password,
                &self.lockout_state,
                self.error_message.as_deref(),
                self.unlock_mode,
                &self.active_vault,
                &self.available_vaults,
            );
        }

        // Main application layout
        let header = views::header::view(
            self.theme,
            v,
            &self.active_vault,
            &self.available_vaults,
            &self.search_query,
            self.show_vault_selector,
            self.github_authenticated,
        );

        let main_content: Element<Message> = match Category::from_index(self.selected_category) {
            Category::Credentials => views::vault::view(views::vault::VaultViewParams {
                theme: self.theme,
                variation: v,
                entries: &self.vault_entries,
                search_query: &self.search_query,
                view_mode: self.vault_view_mode,
                show_add_entry: self.show_add_entry,
                edit_mode: self.edit_mode,
                entry_title: &self.entry_title,
                entry_username: &self.entry_username,
                entry_password: &self.entry_password,
                entry_url: &self.entry_url,
                entry_notes: &self.entry_notes,
                entry_email: &self.entry_email,
                entry_phone: &self.entry_phone,
                entry_folder: &self.entry_folder,
                entry_tags: &self.entry_tags,
                entry_new_tag: &self.entry_new_tag,
                entry_totp_secret: &self.entry_totp_secret,
                entry_custom_fields: &self.entry_custom_fields,
                folder_filter: self.folder_filter.as_deref(),
                entry_type: &self.entry_type,
                entry_attachments: &self.entry_attachments,
                show_advanced_fields: self.show_advanced_fields,
                hovered_entry_index: self.hovered_entry_index,
            }),
            Category::Health => views::health::view(self.theme, v, self.health_report.as_ref(), &self.breached_entries),
            Category::Generator => views::generator::view(views::generator::GeneratorViewParams {
                theme: self.theme,
                variation: v,
                generated_password: &self.generated_password,
                generator_length: self.generator_length,
                generator_lowercase: self.generator_lowercase,
                generator_uppercase: self.generator_uppercase,
                generator_digits: self.generator_digits,
                generator_symbols: self.generator_symbols,
                exclude_ambiguous: self.settings.exclude_ambiguous_chars,
            }),
            Category::Sync => views::sync::view(
                self.theme,
                v,
                self.github_authenticated,
                self.sync_in_progress,
                self.sync_conflict,
            ),
            Category::Account => views::settings::account_view(
                self.theme,
                v,
                self.github_authenticated,
                self.github_username.as_deref(),
                self.sync_in_progress,
                self.device_flow_code.as_deref(),
                self.device_flow_uri.as_deref(),
                self.account_two_factor_enabled,
                self.account_marketing_opt_in,
            ),
            Category::Security => views::settings::security_view(
                self.theme,
                v,
                &self.security_recovery_email,
                &self.security_trusted_devices,
                self.settings.auto_lock_minutes,
                self.settings.clipboard_timeout_seconds,
                self.settings.require_master_on_copy,
            ),
        };

        let navigation = views::navigation::view(self.theme, v, self.selected_category);

        // Status bar
        let layout = column![header, main_content, navigation];

        // Layer modals on top
        let content: Element<Message> = container(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| theme::app_container(self.theme, v))
            .into();

        // Add vault selector dropdown overlay
        let content: Element<Message> = if self.show_vault_selector {
            iced::widget::stack![
                content,
                views::header::vault_dropdown_overlay(
                    self.theme,
                    v,
                    &self.active_vault,
                    &self.available_vaults,
                ),
            ]
            .into()
        } else if self.show_settings {
            iced::widget::stack![
                content,
                views::modals::settings_modal(self.theme, v, self.theme_sort_by_color),
            ]
            .into()
        } else if self.show_new_vault_modal {
            iced::widget::stack![
                content,
                views::modals::new_vault_modal(self.theme, v, &self.new_vault_name),
            ]
            .into()
        } else if self.renaming_vault.is_some() {
            iced::widget::stack![
                content,
                views::modals::rename_vault_modal(self.theme, v, &self.rename_vault_input),
            ]
            .into()
        } else if self.show_delete_confirm {
            let title = self
                .pending_delete_index
                .and_then(|i| self.vault_entries.get(i))
                .map(|e| e.title.as_str())
                .unwrap_or("this entry");
            iced::widget::stack![
                content,
                views::modals::delete_confirm_modal(self.theme, v, title),
            ]
            .into()
        } else if self.show_reauth_modal {
            iced::widget::stack![
                content,
                views::modals::reauth_modal(self.theme, v, &self.reauth_password),
            ]
            .into()
        } else if self.show_change_password {
            iced::widget::stack![
                content,
                views::modals::change_password_modal(self.theme, v, &self.new_master_password),
            ]
            .into()
        } else if self.show_entry_history {
            iced::widget::stack![
                content,
                views::modals::entry_history_modal(self.theme, v, &self.history_entries),
            ]
            .into()
        } else if self.show_audit_log {
            iced::widget::stack![
                content,
                views::modals::audit_log_modal(self.theme, v, &self.audit_events),
            ]
            .into()
        } else {
            content
        };

        // Toast notification overlay
        if let Some(ref msg) = self.status_message {
            let toast_pill: Element<Message> = container(
                iced::widget::text(msg)
                    .size(13)
                    .color(Color::WHITE),
            )
            .padding([10, 20])
            .style(move |_| theme::toast_container(self.theme, v))
            .into();

            let toast_overlay: Element<Message> = container(toast_pill)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Bottom)
                .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 20.0, left: 0.0 })
                .into();

            iced::widget::stack![content, toast_overlay]
                .into()
        } else {
            content
        }
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
        // Zeroize sensitive entry data before clearing
        for entry in &mut self.vault_entries {
            entry.password.zeroize();
        }
        self.vault_entries.clear();
        self.master_password.zeroize();
        self.confirm_password.zeroize();
        self.entry_password.zeroize();
        self.generated_password.zeroize();
        self.show_add_entry = false;
        self.show_settings = false;
        // Re-determine unlock mode in case vault state changed
        self.determine_unlock_mode();
    }

    fn check_clipboard_clear(&mut self) {
        if let (Some(clear_time), Some(ref value)) =
            (&self.clipboard_clear_time, &self.clipboard_value)
        {
            if Instant::now() >= *clear_time {
                // Use common clipboard module (arboard + Linux shell fallback)
                if let Err(e) = lilypad_common::clipboard::copy_to_clipboard("") {
                    eprintln!("[clipboard] failed to clear: {e}");
                }
                let _ = value; // suppress unused warning
                self.clipboard_clear_time = None;
                self.clipboard_value = None;
            }
        }
    }

    fn copy_to_clipboard(&mut self, value: &str) {
        match lilypad_common::clipboard::copy_to_clipboard(value) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("[clipboard] copy failed: {e}");
                self.set_status("Clipboard error — install wl-copy or xclip");
                return;
            }
        }
        if self.settings.clipboard_timeout_seconds > 0 {
            self.clipboard_value = Some(value.to_string());
            self.clipboard_clear_time = Some(
                Instant::now()
                    + Duration::from_secs(self.settings.clipboard_timeout_seconds as u64),
            );
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

    fn generate_demo_password(&self) -> String {
        let charset = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!@#$%&*";
        let mut rng = rand::rng();
        (0..16)
            .map(|_| {
                let idx = rng.random_range(0..charset.len());
                charset[idx] as char
            })
            .collect()
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
        use subtle::ConstantTimeEq;
        password.as_bytes().ct_eq(self.master_password.as_bytes()).into()
    }

    fn try_unlock_vault(&mut self) -> Task<Message> {
        if self.lockout_state.is_locked_out() {
            self.error_message = Some(format!(
                "Locked out. Try again in {} seconds.",
                self.lockout_state.remaining_secs()
            ));
            return Task::none();
        }

        match self.unlock_existing_vault() {
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

    /// Determines whether to show the Create or Unlock screen.
    fn determine_unlock_mode(&mut self) {
        match self.store.vault_exists(&self.active_vault) {
            Ok(true) => self.unlock_mode = UnlockMode::Unlock,
            _ => {
                // Only check key.json for the default vault (V1 backward compat).
                // For other vaults, if the .lily file doesn't exist, show Create.
                if self.active_vault == DEFAULT_VAULT_NAME && self.key_path().exists() {
                    self.unlock_mode = UnlockMode::Unlock;
                } else {
                    self.unlock_mode = UnlockMode::Create;
                }
            }
        }
    }

    /// Unlock an existing vault (V1 or V2).
    fn unlock_existing_vault(&mut self) -> anyhow::Result<()> {
        let password = self.master_password.trim();
        if password.is_empty() {
            return Err(anyhow::anyhow!("Master password is required"));
        }

        // First, try V2 path: read embedded KDF params from vault file
        if let Ok(Some(embedded)) = self.store.load_vault_kdf_params(&self.active_vault) {
            let kdf_params = embedded.to_kdf_params();
            let key = derive_key(password, &kdf_params)?;
            let mut vault = self.store.load_vault(&self.active_vault, &key)?;

            // Auto-upgrade: ensure vault metadata has embedded KDF
            if vault.key_metadata.kdf_params.is_none() {
                vault.key_metadata = vault.key_metadata.clone().with_embedded_kdf(&kdf_params);
                // Best-effort upgrade; failure is non-fatal (will retry on next unlock)
                let _upgrade = self.store.save_vault(&vault, &key);
            }

            // Ensure vault name matches active vault (may differ if file was copied/restored)
            vault.name = self.active_vault.clone();
            self.populate_vault_entries(&vault, &key);
            self.vault = Some(vault);
            self.vault_key = Some(key);
            return Ok(());
        }

        // Fall back to V1 path: load key.json
        let key_path = self.key_path();
        if !key_path.exists() {
            return Err(anyhow::anyhow!(
                "No vault found. Please create a vault first."
            ));
        }
        let (key, key_file) = load_key(&key_path, Some(password))?;
        let mut vault = self.store.load_vault(&self.active_vault, &key)?;

        // Auto-upgrade V1 vault to V2 if key.json has KDF params
        if vault.key_metadata.kdf_params.is_none() {
            if let KeyFile::Kdf { ref params } = key_file {
                vault.key_metadata = vault.key_metadata.clone().with_embedded_kdf(params);
                // Best-effort upgrade; failure is non-fatal (will retry on next unlock)
                let _upgrade = self.store.save_vault(&vault, &key);
            }
        }

        // Ensure vault name matches active vault
        vault.name = self.active_vault.clone();
        self.populate_vault_entries(&vault, &key);
        self.vault = Some(vault);
        self.vault_key = Some(key);
        Ok(())
    }

    /// Create a new vault with embedded V2 format.
    fn create_vault_with_password(&mut self) -> Task<Message> {
        let password = self.master_password.trim().to_string();
        if password.is_empty() {
            self.error_message = Some("Master password is required".to_string());
            return Task::none();
        }

        // Validate confirmation matches
        if password != self.confirm_password.trim() {
            self.error_message = Some("Passwords do not match".to_string());
            return Task::none();
        }

        // Validate password strength
        let strength = validate_password_strength(&password);
        if !strength.is_acceptable() {
            self.error_message = Some(format!("Password too weak: {}", strength.feedback()));
            return Task::none();
        }

        // Generate KDF params and derive key
        let kdf_params = KeyDerivationParams::generate_adaptive();
        let key = match derive_key(&password, &kdf_params) {
            Ok(k) => k,
            Err(e) => {
                self.error_message = Some(format!("Key derivation failed: {}", e));
                return Task::none();
            }
        };

        // Build V2 metadata with embedded KDF
        let metadata = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305)
            .with_embedded_kdf(&kdf_params);
        let vault = Vault::new(&self.active_vault, metadata);

        // Save as V2 vault (with password verifier)
        if let Err(e) = self.store.save_vault(&vault, &key) {
            self.error_message = Some(format!("Failed to create vault: {}", e));
            return Task::none();
        }

        // Also save key.json for CLI backward compat (non-fatal if it fails)
        let key_file = KeyFile::from_kdf(kdf_params);
        let _compat = save_key(&self.key_path(), &key_file);

        self.vault = Some(vault);
        self.vault_key = Some(key);
        self.vault_unlocked = true;
        self.error_message = None;
        self.confirm_password.zeroize();
        self.load_available_vaults();
        self.refresh_health_report();

        // Dismiss onboarding if active
        if self.show_welcome {
            self.show_welcome = false;
            self.save_welcome_ack();
        }

        Task::none()
    }

    /// Refresh vault_entries from the in-memory vault and key.
    /// Use after modifying vault in memory (add/edit/delete/favorite).
    fn refresh_vault_entries(&mut self) {
        if let (Some(vault), Some(key)) = (self.vault.take(), self.vault_key.take()) {
            self.populate_vault_entries(&vault, &key);
            self.vault = Some(vault);
            self.vault_key = Some(key);
        }
    }

    /// Reload vault from disk using the existing key, then refresh entries.
    /// Use after vault file is replaced on disk (sync pull, import, restore).
    fn reload_vault_from_disk(&mut self) {
        if let Some(key) = self.vault_key.take() {
            if let Ok(mut vault) = self.store.load_vault(&self.active_vault, &key) {
                vault.name = self.active_vault.clone();
                self.populate_vault_entries(&vault, &key);
                self.vault = Some(vault);
            }
            self.vault_key = Some(key);
        }
    }

    /// Populates the UI vault entries from a decrypted vault.
    fn populate_vault_entries(&mut self, vault: &Vault, key: &KeyMaterial) {
        self.vault_entries = vault
            .entries
            .iter()
            .filter_map(|entry| {
                let decrypted = decrypt(key, &entry.ciphertext).ok()?;
                let secret: EntrySecret = serde_json::from_slice(&decrypted).ok()?;

                let strength = validate_password_strength(&secret.password);
                let is_expired = entry.is_password_expired();

                let username = entry
                    .metadata
                    .username
                    .clone()
                    .unwrap_or_default();
                let url = entry.metadata.url.clone().unwrap_or_default();

                let totp_code = secret.totp_secret.as_ref().and_then(|s| {
                    generate_totp_code(s)
                });

                Some(VaultEntry {
                    title: entry.label.clone(),
                    username,
                    password: secret.password,
                    url,
                    notes: secret.notes.unwrap_or_default(),
                    email: secret.email.unwrap_or_default(),
                    phone: secret.phone.unwrap_or_default(),
                    totp_secret: secret.totp_secret,
                    custom_fields: secret.custom_fields,
                    attachments: secret
                        .attachments
                        .iter()
                        .map(|a| {
                            let size = base64::Engine::decode(
                                &base64::engine::general_purpose::STANDARD,
                                &a.data_base64,
                            )
                            .map(|d| d.len())
                            .unwrap_or(0);
                            (a.filename.clone(), size)
                        })
                        .collect(),
                    tags: entry.metadata.tags.clone(),
                    folder: entry.metadata.folder.clone(),
                    entry_type: entry.metadata.entry_type.clone(),
                    last_updated: format_timestamp_relative(entry.updated_at),
                    updated_at: entry.updated_at,
                    is_favorite: entry.is_favorite,
                    last_accessed_at: entry.last_accessed_at,
                    access_count: entry.access_count,
                    password_strength: strength,
                    is_expired,
                    color: entry.color,
                    totp_code,
                })
            })
            .collect();
    }

    fn load_available_vaults(&mut self) {
        self.available_vaults = self.store.list_vaults().unwrap_or_default();
        if self.available_vaults.is_empty() {
            self.available_vaults.push(DEFAULT_VAULT_NAME.to_string());
        }
        // Restore persisted active vault, or fall back to first available
        let persisted = &self.settings.active_vault;
        if !persisted.is_empty() && self.available_vaults.contains(persisted) {
            self.active_vault = persisted.clone();
        } else {
            self.active_vault = self.available_vaults[0].clone();
        }
    }

    fn switch_vault(&mut self, name: &str) {
        self.active_vault = name.to_string();
        self.settings.active_vault = name.to_string();
        self.save_settings();
        self.vault = None;
        self.vault_key = None;
        self.vault_entries.clear();
        self.vault_unlocked = false;
        // Clear password to force explicit re-entry for the new vault
        self.master_password.zeroize();
        self.confirm_password.zeroize();
        self.error_message = None;
        self.determine_unlock_mode();
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
        self.entry_email.clear();
        self.entry_phone.clear();
        self.entry_folder.clear();
        self.entry_tags.clear();
        self.entry_new_tag.clear();
        self.entry_totp_secret.clear();
        self.entry_custom_fields.clear();
        self.entry_attachments.clear();
        self.entry_type = "Login".to_string();
        self.show_advanced_fields = false;
        self.edit_index = None;
    }

    fn start_edit(&mut self, index: usize) {
        if let Some(entry) = self.vault_entries.get(index) {
            self.entry_title = entry.title.clone();
            self.entry_username = entry.username.clone();
            self.entry_password = entry.password.clone();
            self.entry_url = entry.url.clone();
            self.entry_notes = entry.notes.clone();
            self.entry_email = entry.email.clone();
            self.entry_phone = entry.phone.clone();
            self.entry_folder = entry.folder.clone().unwrap_or_default();
            self.entry_tags = entry.tags.clone();
            self.entry_new_tag.clear();
            self.entry_totp_secret = entry.totp_secret.clone().unwrap_or_default();
            self.entry_custom_fields = entry
                .custom_fields
                .iter()
                .map(|f| (f.name.clone(), f.value.clone()))
                .collect();
            self.entry_type = format!("{:?}", entry.entry_type);
            // Load raw attachment data from vault for editing
            self.entry_attachments = if let (Some(ref vault), Some(ref key)) =
                (&self.vault, &self.vault_key)
            {
                vault
                    .entries
                    .get(index)
                    .and_then(|e| decrypt(key, &e.ciphertext).ok())
                    .and_then(|bytes| serde_json::from_slice::<EntrySecret>(&bytes).ok())
                    .map(|s| {
                        s.attachments
                            .into_iter()
                            .map(|a| (a.filename, a.data_base64))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            self.edit_mode = true;
            self.edit_index = Some(index);
            self.show_add_entry = true;
        }
    }

    fn save_entry(&mut self) -> Task<Message> {
        // Safety check: ensure in-memory vault matches active vault
        if let Some(ref v) = self.vault {
            if v.name != self.active_vault {
                self.set_status("Vault mismatch error - please re-unlock");
                return Task::none();
            }
        }
        let Some(ref key) = self.vault_key else {
            return Task::none();
        };
        let Some(ref mut vault) = self.vault else {
            return Task::none();
        };

        let was_edit_mode = self.edit_mode;

        // When editing, load the existing EntrySecret to preserve fields we don't
        // expose in the UI (attachments, totp_backup_codes, etc.)
        let mut secret = if was_edit_mode {
            self.edit_index
                .and_then(|idx| vault.entries.get(idx))
                .and_then(|entry| decrypt(key, &entry.ciphertext).ok())
                .and_then(|bytes| serde_json::from_slice::<EntrySecret>(&bytes).ok())
                .unwrap_or_else(|| EntrySecret::new(&self.entry_password))
        } else {
            EntrySecret::new(&self.entry_password)
        };

        // Update the fields that the form exposes
        secret.password = self.entry_password.clone();
        secret.notes = non_empty(&self.entry_notes);
        secret.email = non_empty(&self.entry_email);
        secret.phone = non_empty(&self.entry_phone);
        secret.totp_secret = non_empty(&self.entry_totp_secret);
        secret.custom_fields = self
            .entry_custom_fields
            .iter()
            .filter(|(name, _)| !name.is_empty())
            .map(|(name, value)| lilypad_core::CustomField {
                name: name.clone(),
                value: value.clone(),
                field_type: lilypad_core::CustomFieldType::Text,
            })
            .collect();
        secret.attachments = self
            .entry_attachments
            .iter()
            .map(|(filename, data)| lilypad_core::Attachment::new(filename.clone(), data.clone()))
            .collect();

        let secret_bytes = match serde_json::to_vec(&secret) {
            Ok(b) => b,
            Err(e) => {
                self.set_status(format!("Failed to serialize entry: {}", e));
                return Task::none();
            }
        };
        let ciphertext = match encrypt(key, &secret_bytes) {
            Ok(c) => c,
            Err(e) => {
                self.set_status(format!("Encryption failed: {}", e));
                return Task::none();
            }
        };

        // Build metadata with tags and folder
        let metadata = EntryMetadata {
            username: non_empty(&self.entry_username),
            url: non_empty(&self.entry_url),
            tags: self.entry_tags.clone(),
            folder: non_empty(&self.entry_folder),
            entry_type: match self.entry_type.as_str() {
                "Card" => lilypad_core::EntryType::Card,
                "Identity" => lilypad_core::EntryType::Identity,
                "SecureNote" => lilypad_core::EntryType::SecureNote,
                "SoftwareLicense" => lilypad_core::EntryType::SoftwareLicense,
                "Wifi" => lilypad_core::EntryType::Wifi,
                "Server" => lilypad_core::EntryType::Server,
                "Custom" => lilypad_core::EntryType::Custom,
                _ => lilypad_core::EntryType::Login,
            },
        };

        if was_edit_mode {
            if let Some(index) = self.edit_index {
                if let Some(entry) = vault.entries.get(index) {
                    let label = entry.label.clone();
                    if let Err(e) = vault.update_entry(&label, ciphertext) {
                        self.set_status(format!("Failed to update entry: {}", e));
                        return Task::none();
                    }
                    if let Err(e) = vault.update_entry_metadata(&label, metadata) {
                        self.set_status(format!("Failed to update metadata: {}", e));
                        return Task::none();
                    }
                }
            }
        } else {
            let entry = Entry::new_with_metadata(&self.entry_title, metadata, ciphertext);
            if let Err(e) = vault.add_entry(entry) {
                self.set_status(format!("Failed to add entry: {}", e));
                return Task::none();
            }
        }

        if let Err(e) = self.store.save_vault(vault, key) {
            self.set_status(format!("Failed to save vault: {}", e));
            return Task::none();
        }

        // Refresh entries
        self.refresh_vault_entries();

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
            if let Err(e) = vault.remove_entry(&label) {
                self.set_status(format!("Failed to remove entry: {}", e));
                return Task::none();
            }
            if let Err(e) = self.store.save_vault(vault, key) {
                self.set_status(format!("Failed to save vault: {}", e));
                return Task::none();
            }
            self.refresh_vault_entries();
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
            if let Err(e) = vault.set_entry_favorite(&label, new_favorite) {
                self.set_status(format!("Failed to toggle favorite: {}", e));
                return Task::none();
            }
            if let Err(e) = self.store.save_vault(vault, key) {
                self.set_status(format!("Failed to save vault: {}", e));
                return Task::none();
            }
            // Update in place to avoid scroll reset
            if let Some(ui_entry) = self.vault_entries.get_mut(index) {
                ui_entry.is_favorite = new_favorite;
            }
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
        let mut rng = rand::rng();
        self.generated_password = (0..self.generator_length)
            .map(|_| chars[rng.random_range(0..chars.len())])
            .collect();
    }

    fn refresh_totp_codes(&mut self) {
        for entry in &mut self.vault_entries {
            if let Some(ref secret) = entry.totp_secret {
                entry.totp_code = generate_totp_code(secret);
            }
        }
    }

    fn refresh_health_report(&mut self) {
        let vault_entries_ref = self.vault.as_ref().map(|v| &v.entries);

        let health_data: Vec<EntryHealthData> = self
            .vault_entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let password_age_days = (now.saturating_sub(e.updated_at)) / (24 * 60 * 60);

                // Cross-reference with the core Entry for password expiry data
                let (days_until_expiry, is_expired_from_entry) =
                    if let Some(core_entry) = vault_entries_ref.and_then(|entries| entries.get(i)) {
                        (
                            core_entry.days_until_password_expires(),
                            core_entry.is_password_expired(),
                        )
                    } else {
                        (None, e.is_expired)
                    };

                EntryHealthData {
                    label: e.title.clone(),
                    password: e.password.clone(),
                    has_username: !e.username.is_empty(),
                    has_url: !e.url.is_empty(),
                    has_totp: e.totp_secret.is_some(),
                    password_age_days,
                    days_until_expiry,
                    is_expired: is_expired_from_entry,
                    is_compromised: false,
                }
            })
            .collect();

        self.health_report = Some(analyze_vault_health(&health_data));
    }

    // ========================================================================
    // Export / Import
    // ========================================================================

    fn export_vault_encrypted(&mut self) -> Task<Message> {
        let vault_name = self.active_vault.clone();

        // Get the raw encrypted payload (no key needed for encrypted export)
        let payload = match self.store.sync_payload(&vault_name) {
            Ok(p) => p,
            Err(e) => {
                self.set_status(format!("Export failed: {}", e));
                return Task::none();
            }
        };

        let default_filename = format!("{}.lily", vault_name);
        let file = rfd::FileDialog::new()
            .set_title("Export Vault (Encrypted)")
            .set_file_name(&default_filename)
            .add_filter("Lilypad Vault", &["lily"])
            .save_file();

        if let Some(path) = file {
            match std::fs::write(&path, &payload) {
                Ok(()) => self.set_status(format!("Vault exported to {}", path.display())),
                Err(e) => self.set_status(format!("Export failed: {}", e)),
            }
        }

        Task::none()
    }

    fn export_vault_json(&mut self) -> Task<Message> {
        let Some(ref key) = self.vault_key else {
            self.set_status("Vault must be unlocked to export as JSON");
            return Task::none();
        };
        let Some(ref vault) = self.vault else {
            return Task::none();
        };

        // Build plaintext export
        let mut entries_json: Vec<serde_json::Value> = Vec::new();
        let mut failed_count = 0u32;
        let total_count = vault.entries.len();
        for entry in &vault.entries {
            match decrypt(key, &entry.ciphertext) {
                Ok(decrypted) => {
                    if let Ok(secret) =
                        serde_json::from_slice::<EntrySecret>(&decrypted)
                    {
                        entries_json.push(serde_json::json!({
                            "label": entry.label,
                            "username": entry.metadata.username,
                            "password": secret.password,
                            "url": entry.metadata.url,
                            "notes": secret.notes,
                            "email": secret.email,
                            "phone": secret.phone,
                            "totp_secret": secret.totp_secret,
                            "tags": entry.metadata.tags,
                            "folder": entry.metadata.folder,
                            "is_favorite": entry.is_favorite,
                            "created_at": entry.created_at,
                            "updated_at": entry.updated_at,
                        }));
                    } else {
                        failed_count += 1;
                    }
                }
                Err(_) => {
                    failed_count += 1;
                }
            }
        }

        let export = serde_json::json!({
            "name": vault.name,
            "exported_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "entries": entries_json,
        });

        let json_str = match serde_json::to_string_pretty(&export) {
            Ok(s) => s,
            Err(e) => {
                self.set_status(format!("Export failed: {}", e));
                return Task::none();
            }
        };

        let default_filename = format!("{}.json", vault.name);
        let file = rfd::FileDialog::new()
            .set_title("Export Vault (JSON - PLAINTEXT)")
            .set_file_name(&default_filename)
            .add_filter("JSON", &["json"])
            .save_file();

        if let Some(path) = file {
            match std::fs::write(&path, json_str.as_bytes()) {
                Ok(()) => {
                    let msg = if failed_count > 0 {
                        format!(
                            "Exported {}/{} entries to {} ({} failed to decrypt)",
                            total_count - failed_count as usize,
                            total_count,
                            path.display(),
                            failed_count
                        )
                    } else {
                        format!("Exported {} entries to {}", total_count, path.display())
                    };
                    self.set_status(msg);
                }
                Err(e) => self.set_status(format!("Export failed: {}", e)),
            }
        }

        Task::none()
    }

    fn import_vault_dialog(&mut self) -> Task<Message> {
        let file = rfd::FileDialog::new()
            .set_title("Import Vault")
            .add_filter("Lilypad Vault", &["lily"])
            .add_filter("JSON", &["json"])
            .add_filter("All Files", &["*"])
            .pick_file();

        if let Some(path) = file {
            return Task::done(Message::FileSelected(Some(path)));
        }

        Task::none()
    }

    fn import_vault_file(&mut self, path: std::path::PathBuf) -> Task<Message> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "lily" => {
                // Import encrypted vault file
                match std::fs::read(&path) {
                    Ok(data) => {
                        let vault_name = &self.active_vault;
                        match self.store.apply_sync_payload(vault_name, &data) {
                            Ok(()) => {
                                // Re-decrypt to refresh entries
                                self.reload_vault_from_disk();
                                self.set_status("Vault imported successfully");
                            }
                            Err(e) => {
                                self.set_status(format!("Import failed: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        self.set_status(format!("Failed to read file: {}", e));
                    }
                }
            }
            "json" => {
                // Import JSON plaintext entries
                let Some(ref key) = self.vault_key else {
                    self.set_status("Vault must be unlocked to import JSON");
                    return Task::none();
                };
                let Some(ref mut vault) = self.vault else {
                    return Task::none();
                };

                match std::fs::read_to_string(&path) {
                    Ok(contents) => {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&contents) {
                            let entries = data["entries"].as_array();
                            let mut imported = 0u32;

                            if let Some(entries) = entries {
                                for entry_val in entries {
                                    let label = entry_val["label"]
                                        .as_str()
                                        .unwrap_or("Imported")
                                        .to_string();
                                    let username = entry_val["username"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string();
                                    let password = entry_val["password"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string();
                                    let url =
                                        entry_val["url"].as_str().unwrap_or("").to_string();
                                    let notes =
                                        entry_val["notes"].as_str().unwrap_or("").to_string();
                                    let email =
                                        entry_val["email"].as_str().unwrap_or("").to_string();
                                    let tags: Vec<String> = entry_val["tags"]
                                        .as_array()
                                        .map(|arr| {
                                            arr.iter()
                                                .filter_map(|v| v.as_str().map(String::from))
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    let folder =
                                        entry_val["folder"].as_str().map(String::from);

                                    let mut secret = EntrySecret::new(&password);
                                    secret.notes = non_empty(notes);
                                    secret.email = non_empty(email);

                                    let secret_bytes =
                                        serde_json::to_vec(&secret).unwrap_or_default();
                                    if let Ok(ciphertext) = encrypt(key, &secret_bytes) {
                                        let metadata = EntryMetadata {
                                            username: non_empty(username),
                                            url: non_empty(url),
                                            tags,
                                            folder,
                                            ..Default::default()
                                        };
                                        let entry = Entry::new_with_metadata(
                                            &label, metadata, ciphertext,
                                        );
                                        if vault.add_entry(entry).is_ok() {
                                            imported += 1;
                                        }
                                    }
                                }
                            }

                            if let Err(e) = self.store.save_vault(vault, key) {
                                self.set_status(format!("Failed to save vault: {}", e));
                                return Task::none();
                            }
                            self.refresh_vault_entries();
                            self.set_status(format!("{} entries imported", imported));
                        } else {
                            self.set_status("Invalid JSON format");
                        }
                    }
                    Err(e) => {
                        self.set_status(format!("Failed to read file: {}", e));
                    }
                }
            }
            _ => {
                self.set_status("Unsupported file format. Use .lily or .json");
            }
        }

        Task::none()
    }

    // ========================================================================
    // GitHub OAuth & Sync
    // ========================================================================

    fn github_login(&mut self) -> Task<Message> {
        if self.github_authenticated {
            self.set_status("Already logged in to GitHub");
            return Task::none();
        }

        self.sync_in_progress = true;
        self.sync_started_at = Some(Instant::now());
        self.set_status("Connecting to GitHub...");

        // Phase 1: Initiate device flow → get user code to show in UI
        Task::perform(
            async {
                tokio::task::spawn_blocking(|| {
                    let config = match lilypad_oauth::OAuthConfig::from_env() {
                        Ok(c) => c,
                        Err(e) => return Err(format!("OAuth not configured: {}", e)),
                    };

                    let device_auth = match lilypad_oauth::DeviceFlowAuth::new(config) {
                        Ok(d) => d,
                        Err(e) => return Err(format!("{}", e)),
                    };

                    match device_auth.initiate() {
                        Ok(r) => Ok((r.user_code, r.verification_uri, r.device_code, r.interval)),
                        Err(e) => Err(format!("{}", e)),
                    }
                })
                .await
                .unwrap_or_else(|e| Err(format!("{}", e)))
            },
            |result| match result {
                Ok((user_code, verification_uri, device_code, interval)) => {
                    Message::DeviceFlowCode {
                        user_code,
                        verification_uri,
                        device_code,
                        interval,
                    }
                }
                Err(e) => Message::GitHubLoginResult(Err(e)),
            },
        )
    }

    /// Phase 2: Poll for token in background after user sees the code
    fn github_poll_for_token(&mut self, device_code: String, interval: u64) -> Task<Message> {
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let config = match lilypad_oauth::OAuthConfig::from_env() {
                        Ok(c) => c,
                        Err(e) => return Err(format!("{}", e)),
                    };

                    let device_auth = match lilypad_oauth::DeviceFlowAuth::new(config) {
                        Ok(d) => d,
                        Err(e) => return Err(format!("{}", e)),
                    };

                    // This blocks until user authorizes or timeout
                    let result = match device_auth.poll_for_token(&device_code, interval) {
                        Ok(r) => r,
                        Err(e) => return Err(format!("{}", e)),
                    };

                    // Get user info and store token
                    let client = match lilypad_oauth::GitHubClient::new(&result.access_token) {
                        Ok(c) => c,
                        Err(e) => return Err(format!("Failed to get user info: {}", e)),
                    };
                    let user = match client.get_user() {
                        Ok(u) => u,
                        Err(e) => return Err(format!("{}", e)),
                    };

                    let token_store = match lilypad_oauth::TokenStoreManager::new() {
                        Ok(t) => t,
                        Err(e) => return Err(format!("{}", e)),
                    };
                    let token_info = result
                        .to_token_info()
                        .with_username(user.login.clone());
                    if let Err(e) = token_store.save_token(
                        lilypad_oauth::OAuthProvider::GitHub,
                        token_info,
                    ) {
                        return Err(format!("{}", e));
                    }

                    Ok(user.login)
                })
                .await
                .unwrap_or_else(|e| Err(format!("{}", e)))
            },
            Message::GitHubLoginResult,
        )
    }

    fn ensure_github_repo(&mut self) -> Task<Message> {
        Task::perform(
            async {
                tokio::task::spawn_blocking(|| {
                    let backend = match lilypad_oauth::GitHubSyncBackend::from_stored_token() {
                        Ok(b) => b,
                        Err(e) => return Err(format!("{}", e)),
                    };
                    match backend.ensure_repo() {
                        Ok(()) => Ok("Repository ready".to_string()),
                        Err(e) => Err(format!("{}", e)),
                    }
                })
                .await
                .unwrap_or_else(|e| Err(format!("{}", e)))
            },
            Message::OnboardingRepoResult,
        )
    }

    fn github_logout(&mut self) -> Task<Message> {
        if !self.github_authenticated {
            self.set_status("Not logged in to GitHub");
            return Task::none();
        }

        match lilypad_oauth::GitHubSyncBackend::logout() {
            Ok(()) => {
                self.github_authenticated = false;
                self.github_username = None;
                self.sync_status_text = None;
                self.set_status("Logged out from GitHub");
            }
            Err(e) => {
                self.set_status(format!("Logout failed: {}", e));
            }
        }

        Task::none()
    }

    fn sync_check_status(&mut self) -> Task<Message> {
        if !self.github_authenticated {
            self.sync_status_text = Some("Not authenticated".to_string());
            return Task::none();
        }

        let Some(ref key) = self.vault_key else {
            self.set_status("Vault must be unlocked to check sync status");
            return Task::none();
        };

        // Calculate local checksum
        let payload = match self.store.sync_payload(&self.active_vault) {
            Ok(p) => p,
            Err(e) => {
                self.set_status(format!("Failed to get vault payload: {}", e));
                return Task::none();
            }
        };

        let local_checksum = calculate_checksum(&payload);
        let _key_clone = key.clone();

        self.sync_in_progress = true;
        self.sync_started_at = Some(Instant::now());

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut backend = lilypad_oauth::GitHubSyncBackend::from_stored_token()
                        .map_err(|e| format!("{}", e))?;
                    let status = backend
                        .get_status(&local_checksum)
                        .map_err(|e| format!("{}", e))?;

                    let msg = match status {
                        lilypad_oauth::SyncStatus::InSync => "In sync with GitHub".to_string(),
                        lilypad_oauth::SyncStatus::LocalAhead => {
                            "Local changes pending push".to_string()
                        }
                        lilypad_oauth::SyncStatus::RemoteAhead => {
                            "Remote changes available".to_string()
                        }
                        lilypad_oauth::SyncStatus::Conflict => {
                            "Conflict: both local and remote changed".to_string()
                        }
                        lilypad_oauth::SyncStatus::NoRemoteVault => {
                            "No remote vault. Push to create.".to_string()
                        }
                        lilypad_oauth::SyncStatus::NotAuthenticated => {
                            "Not authenticated".to_string()
                        }
                        lilypad_oauth::SyncStatus::Unknown(msg) => format!("Unknown: {}", msg),
                    };

                    Ok(msg)
                })
                .await
                .unwrap_or_else(|e| Err(format!("Task failed: {}", e)))
            },
            |result: std::result::Result<String, String>| match result {
                Ok(msg) => Message::SyncCompleted(Ok(msg)),
                Err(e) => Message::SyncCompleted(Err(format!("Status check failed: {}", e))),
            },
        )
    }

    fn sync_push(&mut self) -> Task<Message> {
        if !self.github_authenticated {
            self.set_status("Login to GitHub first");
            return Task::none();
        }

        let vault_name = self.active_vault.clone();

        let payload = match self.store.sync_payload(&vault_name) {
            Ok(p) => p,
            Err(e) => {
                self.set_status(format!("Failed to prepare vault: {}", e));
                return Task::none();
            }
        };

        self.sync_in_progress = true;
        self.sync_started_at = Some(Instant::now());
        self.set_status("Pushing vault to GitHub...");

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut backend = lilypad_oauth::GitHubSyncBackend::from_stored_token()
                        .map_err(|e| format!("{}", e))?;
                    backend.ensure_repo().map_err(|e| format!("{}", e))?;
                    backend
                        .push(&vault_name, &payload)
                        .map_err(|e| format!("{}", e))?;
                    Ok(format!(
                        "Vault '{}' pushed to GitHub successfully",
                        vault_name
                    ))
                })
                .await
                .unwrap_or_else(|e| Err(format!("Task failed: {}", e)))
            },
            Message::SyncCompleted,
        )
    }

    fn sync_pull(&mut self) -> Task<Message> {
        if !self.github_authenticated {
            self.set_status("Login to GitHub first");
            return Task::none();
        }

        let Some(ref key) = self.vault_key else {
            self.set_status("Vault must be unlocked to pull");
            return Task::none();
        };

        let vault_name = self.active_vault.clone();
        let store_root = std::path::PathBuf::from(&self.config.data_dir);
        let key_clone = key.clone();

        self.sync_in_progress = true;
        self.sync_started_at = Some(Instant::now());
        self.set_status("Pulling vault from GitHub...");

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut backend = lilypad_oauth::GitHubSyncBackend::from_stored_token()
                        .map_err(|e| format!("{}", e))?;
                    let payload = backend
                        .pull(&vault_name)
                        .map_err(|e| format!("{}", e))?;

                    match payload {
                        Some(data) => {
                            // Create a temporary store to apply the payload
                            let config = lilypad_core::AppConfig {
                                data_dir: store_root.to_string_lossy().to_string(),
                                ..lilypad_core::default_config()
                            };
                            let store =
                                lilypad_storage::LocalStore::new(&config).map_err(|e| format!("{}", e))?;
                            store
                                .apply_sync_payload(&vault_name, &data)
                                .map_err(|e| format!("{}", e))?;
                            // Verify decryption works
                            let _vault = store
                                .load_vault(&vault_name, &key_clone)
                                .map_err(|e| format!("Decryption failed: {}", e))?;
                            Ok(format!(
                                "Vault '{}' pulled from GitHub ({} bytes)",
                                vault_name,
                                data.len()
                            ))
                        }
                        None => Ok("No remote vault data found on GitHub".to_string()),
                    }
                })
                .await
                .unwrap_or_else(|e| Err(format!("Task failed: {}", e)))
            },
            Message::SyncCompleted,
        )
    }

    /// Pull vault from GitHub without requiring unlock.
    /// Downloads the vault file and saves it to disk, then the user can unlock it.
    fn sync_pull_to_unlock(&mut self) -> Task<Message> {
        if !self.github_authenticated {
            self.set_status("Login to GitHub first");
            return Task::none();
        }

        let vault_name = self.active_vault.clone();
        let store_root = std::path::PathBuf::from(&self.config.data_dir);

        self.sync_in_progress = true;
        self.sync_started_at = Some(Instant::now());
        self.set_status("Pulling vault from GitHub...");

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut backend = lilypad_oauth::GitHubSyncBackend::from_stored_token()
                        .map_err(|e| format!("{}", e))?;
                    let payload = backend
                        .pull(&vault_name)
                        .map_err(|e| format!("{}", e))?;

                    match payload {
                        Some(data) => {
                            let config = lilypad_core::AppConfig {
                                data_dir: store_root.to_string_lossy().to_string(),
                                ..lilypad_core::default_config()
                            };
                            let store =
                                lilypad_storage::LocalStore::new(&config).map_err(|e| format!("{}", e))?;
                            store
                                .apply_sync_payload(&vault_name, &data)
                                .map_err(|e| format!("{}", e))?;
                            Ok(format!(
                                "Vault '{}' pulled from GitHub ({} bytes)",
                                vault_name,
                                data.len()
                            ))
                        }
                        None => {
                            Ok("No remote vault found on GitHub".to_string())
                        }
                    }
                })
                .await
                .unwrap_or_else(|e| Err(format!("Task failed: {}", e)))
            },
            Message::SyncCompleted,
        )
    }

    // ========================================================================
    // CSV Export
    // ========================================================================

    fn export_vault_csv(&mut self) -> Task<Message> {
        let Some(ref key) = self.vault_key else {
            self.set_status("Vault must be unlocked to export CSV");
            return Task::none();
        };
        let Some(ref vault) = self.vault else {
            return Task::none();
        };

        let mut csv_data = String::from("name,url,username,password,notes,folder,tags\n");
        let mut exported = 0u32;

        for entry in &vault.entries {
            if let Ok(decrypted) = decrypt(key, &entry.ciphertext) {
                if let Ok(secret) = serde_json::from_slice::<EntrySecret>(&decrypted) {
                    let escape_csv = |s: &str| -> String {
                        if s.contains(',') || s.contains('"') || s.contains('\n') {
                            format!("\"{}\"", s.replace('"', "\"\""))
                        } else {
                            s.to_string()
                        }
                    };

                    csv_data.push_str(&format!(
                        "{},{},{},{},{},{},{}\n",
                        escape_csv(&entry.label),
                        escape_csv(entry.metadata.url.as_deref().unwrap_or("")),
                        escape_csv(entry.metadata.username.as_deref().unwrap_or("")),
                        escape_csv(&secret.password),
                        escape_csv(secret.notes.as_deref().unwrap_or("")),
                        escape_csv(entry.metadata.folder.as_deref().unwrap_or("")),
                        escape_csv(&entry.metadata.tags.join(";")),
                    ));
                    exported += 1;
                }
            }
        }

        let default_filename = format!("{}.csv", vault.name);
        let file = rfd::FileDialog::new()
            .set_title("Export Vault as CSV (PLAINTEXT)")
            .set_file_name(&default_filename)
            .add_filter("CSV", &["csv"])
            .save_file();

        if let Some(path) = file {
            match std::fs::write(&path, csv_data.as_bytes()) {
                Ok(()) => self.set_status(format!("{} entries exported to {}", exported, path.display())),
                Err(e) => self.set_status(format!("CSV export failed: {}", e)),
            }
        }

        Task::none()
    }

    // ========================================================================
    // Browser CSV Import
    // ========================================================================

    fn import_browser_csv_dialog(&mut self) -> Task<Message> {
        let file = rfd::FileDialog::new()
            .set_title("Import from Browser CSV")
            .add_filter("CSV Files", &["csv"])
            .pick_file();

        if let Some(path) = file {
            return Task::done(Message::BrowserCsvSelected(Some(path)));
        }
        Task::none()
    }

    fn import_browser_csv_file(&mut self, path: std::path::PathBuf) -> Task<Message> {
        let Some(ref key) = self.vault_key else {
            self.set_status("Vault must be unlocked to import");
            return Task::none();
        };
        let Some(ref mut vault) = self.vault else {
            return Task::none();
        };

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                self.set_status(format!("Failed to read CSV: {}", e));
                return Task::none();
            }
        };

        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .has_headers(true)
            .from_reader(contents.as_bytes());

        let headers: Vec<String> = match reader.headers() {
            Ok(h) => h.iter().map(|s| s.to_lowercase().trim().to_string()).collect(),
            Err(e) => {
                self.set_status(format!("Invalid CSV headers: {}", e));
                return Task::none();
            }
        };

        // Detect format based on headers
        // Supports: Chrome, Firefox, Bitwarden, LastPass, 1Password, KeePass, Dashlane, NordPass
        let name_col = headers.iter().position(|h| matches!(h.as_str(), "name" | "title" | "login_name" | "login_label" | "entry"));
        let url_col = headers.iter().position(|h| matches!(h.as_str(), "url" | "login_uri" | "web site" | "website" | "urls"));
        let user_col = headers.iter().position(|h| matches!(h.as_str(), "username" | "login_username" | "user name" | "login" | "login_name"));
        let pass_col = headers.iter().position(|h| matches!(h.as_str(), "password" | "login_password" | "pass"));
        let notes_col = headers.iter().position(|h| matches!(h.as_str(), "notes" | "extra" | "comments" | "note"));
        let folder_col = headers.iter().position(|h| matches!(h.as_str(), "folder" | "group" | "grouping" | "collection_ids" | "category"));
        let totp_col = headers.iter().position(|h| matches!(h.as_str(), "totp" | "login_totp" | "otpauth" | "2fa"));
        let email_col = headers.iter().position(|h| matches!(h.as_str(), "email" | "e-mail"));

        let mut imported = 0u32;

        for result in reader.records() {
            let record = match result {
                Ok(r) => r,
                Err(_) => continue,
            };

            let get_field = |col: Option<usize>| -> String {
                col.and_then(|i| record.get(i))
                    .unwrap_or("")
                    .to_string()
            };

            let name = get_field(name_col);
            let url = get_field(url_col);
            let username = get_field(user_col);
            let password = get_field(pass_col);
            let notes = get_field(notes_col);
            let folder = get_field(folder_col);
            let totp = get_field(totp_col);
            let email = get_field(email_col);

            // Skip empty entries
            let label = if name.is_empty() {
                if url.is_empty() { continue; }
                url.clone()
            } else {
                name
            };

            let mut secret = EntrySecret::new(&password);
            secret.notes = non_empty(notes);
            secret.totp_secret = non_empty(totp);
            secret.email = non_empty(email);

            let secret_bytes = serde_json::to_vec(&secret).unwrap_or_default();
            if let Ok(ciphertext) = encrypt(key, &secret_bytes) {
                let metadata = EntryMetadata {
                    username: non_empty(username),
                    url: non_empty(url),
                    folder: non_empty(folder),
                    ..Default::default()
                };
                let entry = Entry::new_with_metadata(&label, metadata, ciphertext);
                if vault.add_entry(entry).is_ok() {
                    imported += 1;
                }
            }
        }

        if imported > 0 {
            if let Err(e) = self.store.save_vault(vault, key) {
                self.set_status(format!("Failed to save vault: {}", e));
                return Task::none();
            }
        }
        // `vault` and `key` borrows end here (NLL)

        if imported > 0 {
            self.refresh_vault_entries();
        }
        self.set_status(format!("{} entries imported from browser CSV", imported));

        Task::none()
    }

    // ========================================================================
    // Breach Check (HIBP k-anonymity)
    // ========================================================================

    fn check_breaches(&mut self) -> Task<Message> {
        let Some(ref key) = self.vault_key else {
            self.set_status("Vault must be unlocked to check breaches");
            return Task::none();
        };
        let Some(ref vault) = self.vault else {
            return Task::none();
        };

        // Collect passwords and labels for checking
        let mut entries_to_check: Vec<(String, String)> = Vec::new();
        for entry in &vault.entries {
            if let Ok(decrypted) = decrypt(key, &entry.ciphertext) {
                if let Ok(secret) = serde_json::from_slice::<EntrySecret>(&decrypted) {
                    if !secret.password.is_empty() {
                        entries_to_check.push((entry.label.clone(), secret.password.clone()));
                    }
                }
            }
        }

        self.sync_in_progress = true;
        self.sync_started_at = Some(Instant::now());
        self.set_status("Checking passwords against breach database...");

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    use sha1::{Sha1, Digest};
                    let client = reqwest::blocking::Client::builder()
                        .timeout(std::time::Duration::from_secs(10))
                        .user_agent("Lilypad-BreachCheck/1.0")
                        .build()
                        .map_err(|e| format!("{}", e))?;

                    let mut breached = Vec::new();

                    for (label, password) in &entries_to_check {
                        let mut hasher = Sha1::new();
                        hasher.update(password.as_bytes());
                        let hash = format!("{:X}", hasher.finalize());
                        let prefix = &hash[..5];
                        let suffix = &hash[5..];

                        let url = format!("https://api.pwnedpasswords.com/range/{}", prefix);
                        if let Ok(response) = client.get(&url).send() {
                            if let Ok(body) = response.text() {
                                for line in body.lines() {
                                    if let Some((hash_suffix, _count)) = line.split_once(':') {
                                        if hash_suffix.eq_ignore_ascii_case(suffix) {
                                            breached.push(label.clone());
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        // Rate limiting courtesy
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }

                    Ok(breached)
                })
                .await
                .unwrap_or_else(|e| Err(format!("{}", e)))
            },
            |result: std::result::Result<Vec<String>, String>| match result {
                Ok(breached) => Message::BreachCheckCompleted(breached),
                Err(e) => Message::SetStatus(format!("Breach check failed: {}", e)),
            },
        )
    }

    // ========================================================================
    // Key Rotation (Change Master Password)
    // ========================================================================

    fn change_master_password(&mut self) -> Task<Message> {
        let Some(ref old_key) = self.vault_key else {
            self.set_status("Vault must be unlocked to change password");
            return Task::none();
        };

        if self.new_master_password.is_empty() {
            self.set_status("New password cannot be empty");
            return Task::none();
        }

        let kdf_params = KeyDerivationParams::generate_adaptive();
        let new_key = match derive_key(&self.new_master_password, &kdf_params) {
            Ok(k) => k,
            Err(e) => {
                self.set_status(format!("Key derivation failed: {}", e));
                return Task::none();
            }
        };

        // Re-encrypt all entries in all vaults
        let vault_names = match self.store.list_vaults() {
            Ok(v) => v,
            Err(e) => {
                self.set_status(format!("Failed to list vaults: {}", e));
                return Task::none();
            }
        };

        let mut total_re_encrypted = 0u32;
        for vault_name in &vault_names {
            let mut vault = match self.store.load_vault(vault_name, old_key) {
                Ok(v) => v,
                Err(e) => {
                    self.set_status(format!("Failed to load vault '{}': {}", vault_name, e));
                    return Task::none();
                }
            };

            // Re-encrypt each entry
            for entry in &mut vault.entries {
                if let Ok(plaintext) = decrypt(old_key, &entry.ciphertext) {
                    if let Ok(new_ciphertext) = encrypt(&new_key, &plaintext) {
                        entry.ciphertext = new_ciphertext;
                        total_re_encrypted += 1;
                    }
                }
            }

            // Upgrade to V2 with embedded KDF params
            vault.key_metadata = KeyMetadata::new(&new_key, CryptoAlgorithm::XChaCha20Poly1305)
                .with_embedded_kdf(&kdf_params);

            if let Err(e) = self.store.save_vault(&vault, &new_key) {
                self.set_status(format!("Failed to save vault '{}': {}", vault_name, e));
                return Task::none();
            }
        }

        // Update key file with new KDF params (CLI backward compat, non-fatal)
        let key_file = KeyFile::from_kdf(kdf_params);
        let _compat = save_key(&self.key_path(), &key_file);

        // Update the active key and reload
        self.vault_key = Some(new_key.clone());
        self.master_password = self.new_master_password.clone();
        self.new_master_password.zeroize();
        self.show_change_password = false;

        // Reload vault entries with the new key
        if self.vault.is_some() {
            // Re-load fresh from disk since we just saved
            if let Ok(reloaded) = self.store.load_vault(&self.active_vault, &new_key) {
                self.populate_vault_entries(&reloaded, &new_key);
                self.vault = Some(reloaded);
            }
        }

        self.set_status(format!(
            "Master password changed. {} entries re-encrypted across {} vaults.",
            total_re_encrypted,
            vault_names.len()
        ));

        Task::none()
    }

    // ========================================================================
    // Entry History
    // ========================================================================

    fn view_entry_history(&mut self, idx: usize) {
        let Some(ref vault) = self.vault else { return };
        if let Some(entry) = vault.entries.get(idx) {
            self.history_entries = entry
                .get_history()
                .map(|h| {
                    let desc = h.description.clone().unwrap_or_else(|| {
                        format!("{:?}", h.change_type)
                    });
                    (h.timestamp, desc)
                })
                .collect();
            self.show_entry_history = true;
        }
    }

    // ========================================================================
    // Backup / Restore
    // ========================================================================

    fn backup_vault(&mut self) -> Task<Message> {
        let vault_name = self.active_vault.clone();

        let payload = match self.store.sync_payload(&vault_name) {
            Ok(p) => p,
            Err(e) => {
                self.set_status(format!("Backup failed: {}", e));
                return Task::none();
            }
        };

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let default_filename = format!("{}_backup_{}.lily", vault_name, timestamp);
        let file = rfd::FileDialog::new()
            .set_title("Backup Vault")
            .set_file_name(&default_filename)
            .add_filter("Lilypad Vault Backup", &["lily"])
            .save_file();

        if let Some(path) = file {
            match std::fs::write(&path, &payload) {
                Ok(()) => self.set_status(format!("Vault backed up to {}", path.display())),
                Err(e) => self.set_status(format!("Backup failed: {}", e)),
            }
        }

        Task::none()
    }

    fn restore_vault_dialog(&mut self) -> Task<Message> {
        let file = rfd::FileDialog::new()
            .set_title("Restore Vault from Backup")
            .add_filter("Lilypad Vault Backup", &["lily"])
            .pick_file();

        if let Some(path) = file {
            return Task::done(Message::BackupFileSelected(Some(path)));
        }
        Task::none()
    }

    fn restore_vault_file(&mut self, path: std::path::PathBuf) -> Task<Message> {
        match std::fs::read(&path) {
            Ok(data) => {
                let vault_name = &self.active_vault;
                match self.store.apply_sync_payload(vault_name, &data) {
                    Ok(()) => {
                        self.reload_vault_from_disk();
                        self.set_status(format!("Vault restored from {}", path.display()));
                    }
                    Err(e) => {
                        self.set_status(format!("Restore failed: {}", e));
                    }
                }
            }
            Err(e) => {
                self.set_status(format!("Failed to read backup file: {}", e));
            }
        }

        Task::none()
    }

    // ========================================================================
    // Audit Log Export
    // ========================================================================

    fn export_audit_log_json(&mut self) -> Task<Message> {
        if self.audit_events.is_empty() {
            self.set_status("No audit events to export");
            return Task::none();
        }

        let events: Vec<serde_json::Value> = self
            .audit_events
            .iter()
            .map(|(ts, action, label)| {
                serde_json::json!({
                    "timestamp": ts,
                    "action": action,
                    "entry_label": label,
                })
            })
            .collect();

        let export = serde_json::json!({
            "vault": self.active_vault,
            "exported_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "event_count": events.len(),
            "events": events,
        });

        let json_str = match serde_json::to_string_pretty(&export) {
            Ok(s) => s,
            Err(e) => {
                self.set_status(format!("Export failed: {}", e));
                return Task::none();
            }
        };

        let file = rfd::FileDialog::new()
            .set_title("Export Audit Log (JSON)")
            .set_file_name("audit_log.json")
            .add_filter("JSON", &["json"])
            .save_file();

        if let Some(path) = file {
            match fs::write(&path, json_str.as_bytes()) {
                Ok(()) => self.set_status(format!(
                    "{} events exported to {}",
                    self.audit_events.len(),
                    path.display()
                )),
                Err(e) => self.set_status(format!("Export failed: {}", e)),
            }
        }

        Task::none()
    }

    fn export_audit_log_csv(&mut self) -> Task<Message> {
        if self.audit_events.is_empty() {
            self.set_status("No audit events to export");
            return Task::none();
        }

        let mut csv_data = String::from("timestamp,date,action,entry_label\n");
        for (ts, action, label) in &self.audit_events {
            let date = lilypad_common::time::format_timestamp_relative(*ts);
            let escape = |s: &str| -> String {
                if s.contains(',') || s.contains('"') || s.contains('\n') {
                    format!("\"{}\"", s.replace('"', "\"\""))
                } else {
                    s.to_string()
                }
            };
            csv_data.push_str(&format!(
                "{},{},{},{}\n",
                ts,
                escape(&date),
                escape(action),
                escape(label.as_deref().unwrap_or("")),
            ));
        }

        let file = rfd::FileDialog::new()
            .set_title("Export Audit Log (CSV)")
            .set_file_name("audit_log.csv")
            .add_filter("CSV", &["csv"])
            .save_file();

        if let Some(path) = file {
            match fs::write(&path, csv_data.as_bytes()) {
                Ok(()) => self.set_status(format!(
                    "{} events exported to {}",
                    self.audit_events.len(),
                    path.display()
                )),
                Err(e) => self.set_status(format!("Export failed: {}", e)),
            }
        }

        Task::none()
    }

    fn export_audit_log_text(&mut self) -> Task<Message> {
        if self.audit_events.is_empty() {
            self.set_status("No audit events to export");
            return Task::none();
        }

        let mut text_data = format!(
            "Lilypad Audit Log - Vault: {}\n{}\n\n",
            self.active_vault,
            "=".repeat(50)
        );

        for (ts, action, label) in &self.audit_events {
            let date = lilypad_common::time::format_timestamp_relative(*ts);
            let label_str = label.as_deref().unwrap_or("-");
            text_data.push_str(&format!("[{}] {} | {}\n", date, action, label_str));
        }

        text_data.push_str(&format!(
            "\n{}\nTotal: {} events\n",
            "-".repeat(50),
            self.audit_events.len()
        ));

        let file = rfd::FileDialog::new()
            .set_title("Export Audit Log (Text)")
            .set_file_name("audit_log.txt")
            .add_filter("Text", &["txt"])
            .save_file();

        if let Some(path) = file {
            match fs::write(&path, text_data.as_bytes()) {
                Ok(()) => self.set_status(format!(
                    "{} events exported to {}",
                    self.audit_events.len(),
                    path.display()
                )),
                Err(e) => self.set_status(format!("Export failed: {}", e)),
            }
        }

        Task::none()
    }

    // ========================================================================
    // Backup Pruning
    // ========================================================================

    fn prune_backups(&mut self) -> Task<Message> {
        let dir = rfd::FileDialog::new()
            .set_title("Select Backup Directory to Prune")
            .pick_folder();

        let Some(dir) = dir else {
            return Task::none();
        };

        let vault_name = self.active_vault.clone();
        let prefix = format!("{}_backup_", vault_name);

        // Collect matching backup files
        let mut backups: Vec<std::path::PathBuf> = match fs::read_dir(&dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().and_then(|e| e.to_str()) == Some("lily")
                        && p.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.starts_with(&prefix))
                            .unwrap_or(false)
                })
                .collect(),
            Err(e) => {
                self.set_status(format!("Failed to read directory: {}", e));
                return Task::none();
            }
        };

        if backups.len() <= 5 {
            self.set_status(format!(
                "Only {} backups found (keeping all; prune removes when >5)",
                backups.len()
            ));
            return Task::none();
        }

        // Sort by modification time, newest first
        backups.sort_by(|a, b| {
            let ta = fs::metadata(a)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            let tb = fs::metadata(b)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            tb.cmp(&ta)
        });

        // Keep the 5 newest, delete the rest
        let to_delete = &backups[5..];
        let mut deleted = 0u32;
        for path in to_delete {
            if fs::remove_file(path).is_ok() {
                deleted += 1;
            }
        }

        self.set_status(format!(
            "Pruned {} old backups, kept {} most recent",
            deleted,
            backups.len().min(5)
        ));

        Task::none()
    }
}

/// Calculate SHA-256 checksum of data
fn calculate_checksum(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let result = Sha256::digest(data);
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Generate a TOTP code from a base32-encoded secret
fn generate_totp_code(secret: &str) -> Option<String> {
    use totp_rs::{Algorithm, TOTP};
    let decoded = totp_rs::Secret::Encoded(secret.to_string())
        .to_bytes()
        .ok()?;
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, decoded).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Some(totp.generate(now))
}
