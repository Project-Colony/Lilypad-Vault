//! Lilypad TUI - Terminal User Interface for the Lilypad password manager.
//!
//! A keyboard-driven interface for managing your vault in the terminal.
//! Supports multi-vault, search, password generation, editing, TOTP, and health dashboard.

use anyhow::{anyhow, Result};
use arboard::Clipboard;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use lilypad_common::{
    analyze_vault_health,
    keyfile::{load_key, save_key, KeyFile},
    time::format_timestamp_relative,
    validation::validate_password_strength,
    EntryHealthData, HealthReport, PasswordStrength,
};
use lilypad_core::{
    decrypt, default_config, derive_key, encrypt, AppConfig, CryptoAlgorithm, CustomField, Entry,
    EntryMetadata, EntrySecret, KeyDerivationParams, KeyMaterial, KeyMetadata, Vault,
};
use lilypad_storage::LocalStore;
use rand::RngExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::cmp::Reverse;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use totp_rs::{Algorithm, Secret, TOTP};
use zeroize::Zeroize;

/// Decrypted vault entry for display
struct TuiEntry {
    label: String,
    username: String,
    password: String,
    url: String,
    notes: String,
    email: String,
    phone: String,
    totp_secret: Option<String>,
    custom_fields: Vec<CustomField>,
    tags: Vec<String>,
    folder: Option<String>,
    updated_at: u64,
    is_favorite: bool,
    password_strength: PasswordStrength,
    entry_type: String,
}

impl Drop for TuiEntry {
    fn drop(&mut self) {
        self.password.zeroize();
        if let Some(ref mut totp) = self.totp_secret {
            totp.zeroize();
        }
    }
}

/// Application state
enum AppState {
    Locked,
    Unlocked,
    AddEntry,
    EditEntry,
    ViewEntry,
    ConfirmDelete,
    Help,
    Search,
    GeneratePassword,
    SelectVault,
    HealthDashboard,
    FilterByFolder,
    ExportVault,
}

/// Input field focus for add/edit forms
#[derive(Clone, Copy, PartialEq)]
enum InputField {
    Password,
    Label,
    Username,
    EntryPassword,
    Url,
    Notes,
    Email,
    Tags,
}

/// Main application structure
struct App {
    state: AppState,
    config: AppConfig,
    store: LocalStore,

    // Vault data
    active_vault: String,
    available_vaults: Vec<String>,
    vault: Option<Vault>,
    vault_key: Option<KeyMaterial>,
    entries: Vec<TuiEntry>,
    filtered_indices: Vec<usize>,
    entry_list_state: ListState,

    // Input fields
    master_password: String,
    input_label: String,
    input_username: String,
    input_password: String,
    input_url: String,
    input_notes: String,
    input_email: String,
    input_tags: String,
    current_field: InputField,

    // Edit mode
    edit_index: Option<usize>,

    // Search
    search_query: String,
    search_active: bool,

    // Password generator
    gen_length: usize,
    gen_lowercase: bool,
    gen_uppercase: bool,
    gen_digits: bool,
    gen_symbols: bool,
    gen_result: String,

    // Status
    status_message: Option<String>,
    status_time: Option<Instant>,

    // Clipboard timeout
    clipboard_timeout: Option<Instant>,
    clipboard_value: Option<String>,

    // Health
    health_report: Option<HealthReport>,

    // Folder filter
    folder_filter: Option<String>,
    available_folders: Vec<String>,
    folder_list_state: ListState,

    // Export
    export_path: String,

    // UI state
    show_password: bool,
    should_quit: bool,
    vault_list_state: ListState,
}

impl Drop for App {
    fn drop(&mut self) {
        self.master_password.zeroize();
        self.input_password.zeroize();
        self.gen_result.zeroize();
        if let Some(ref mut v) = self.clipboard_value {
            v.zeroize();
        }
    }
}

impl App {
    fn new() -> Result<Self> {
        let config = default_config();
        let store = LocalStore::new(&config)?;
        let available_vaults = store.list_vaults().unwrap_or_default();
        let active_vault = available_vaults
            .first()
            .cloned()
            .unwrap_or_else(|| "primary".to_string());

        Ok(Self {
            state: AppState::Locked,
            config,
            store,
            active_vault,
            available_vaults,
            vault: None,
            vault_key: None,
            entries: Vec::new(),
            filtered_indices: Vec::new(),
            entry_list_state: ListState::default(),
            master_password: String::new(),
            input_label: String::new(),
            input_username: String::new(),
            input_password: String::new(),
            input_url: String::new(),
            input_notes: String::new(),
            input_email: String::new(),
            input_tags: String::new(),
            current_field: InputField::Password,
            edit_index: None,
            search_query: String::new(),
            search_active: false,
            gen_length: 16,
            gen_lowercase: true,
            gen_uppercase: true,
            gen_digits: true,
            gen_symbols: true,
            gen_result: String::new(),
            status_message: None,
            status_time: None,
            clipboard_timeout: None,
            clipboard_value: None,
            health_report: None,
            folder_filter: None,
            available_folders: Vec::new(),
            folder_list_state: ListState::default(),
            export_path: String::new(),
            show_password: false,
            should_quit: false,
            vault_list_state: ListState::default(),
        })
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
        self.status_time = Some(Instant::now());
    }

    fn check_status_clear(&mut self) {
        if let Some(time) = self.status_time {
            if time.elapsed() > Duration::from_secs(5) {
                self.status_message = None;
                self.status_time = None;
            }
        }
    }

    fn check_clipboard_clear(&mut self) {
        if let (Some(timeout), Some(value)) = (&self.clipboard_timeout, &self.clipboard_value) {
            if Instant::now() >= *timeout {
                if let Ok(mut clipboard) = Clipboard::new() {
                    if clipboard.get_text().ok().as_deref() == Some(value) {
                        let _ = clipboard.set_text(String::new());
                    }
                }
                self.clipboard_timeout = None;
                self.clipboard_value = None;
            }
        }
    }

    fn copy_to_clipboard(&mut self, value: &str) {
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(value.to_string()).is_ok() {
                self.clipboard_value = Some(value.to_string());
                self.clipboard_timeout = Some(Instant::now() + Duration::from_secs(30));
                self.set_status("Copied to clipboard (clears in 30s)");
            }
        }
    }

    fn key_path(&self) -> PathBuf {
        PathBuf::from(&self.config.data_dir).join("key.json")
    }

    fn unlock_vault(&mut self) -> Result<()> {
        let password = self.master_password.trim();
        if password.is_empty() {
            return Err(anyhow!("master password is required"));
        }

        let path = self.key_path();
        let (key, key_file) = if path.exists() {
            load_key(&path, Some(password))?
        } else {
            let params = KeyDerivationParams::generate_adaptive();
            let key = derive_key(password, &params)?;
            let key_file = KeyFile::from_kdf(params);
            save_key(&path, &key_file)?;
            (key, key_file)
        };

        let vault = if let Ok(v) = self.store.load_vault(&self.active_vault, &key) {
            v
        } else {
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

        self.entries = Self::load_entries(&vault, &key)?;
        self.vault = Some(vault);
        self.vault_key = Some(key);
        self.state = AppState::Unlocked;
        self.update_filtered_indices();
        self.collect_folders();
        self.refresh_health();
        self.entry_list_state
            .select(if self.filtered_indices.is_empty() {
                None
            } else {
                Some(0)
            });

        // Refresh vault list
        self.available_vaults = self.store.list_vaults().unwrap_or_default();
        if self.available_vaults.is_empty() {
            self.available_vaults.push(self.active_vault.clone());
        }

        Ok(())
    }

    fn load_entries(vault: &Vault, key: &KeyMaterial) -> Result<Vec<TuiEntry>> {
        let mut entries = Vec::with_capacity(vault.entries.len());

        for entry in &vault.entries {
            let plaintext = decrypt(key, &entry.ciphertext)?;
            let secret: EntrySecret = serde_json::from_slice(&plaintext).unwrap_or_else(|_| {
                EntrySecret::new(String::from_utf8_lossy(&plaintext).into_owned())
            });

            let username = entry.metadata.username.clone().unwrap_or_default();
            let url = entry.metadata.url.clone().unwrap_or_default();
            let strength = validate_password_strength(&secret.password);

            let entry_type = format!("{:?}", entry.metadata.entry_type);

            entries.push(TuiEntry {
                label: entry.label.clone(),
                username,
                password: secret.password,
                url,
                notes: secret.notes.unwrap_or_default(),
                email: secret.email.unwrap_or_default(),
                phone: secret.phone.unwrap_or_default(),
                totp_secret: secret.totp_secret,
                custom_fields: secret.custom_fields,
                tags: entry.metadata.tags.clone(),
                folder: entry.metadata.folder.clone(),
                updated_at: entry.updated_at,
                is_favorite: entry.is_favorite,
                password_strength: strength,
                entry_type,
            });
        }

        entries.sort_by_key(|entry| Reverse(entry.updated_at));
        Ok(entries)
    }

    fn update_filtered_indices(&mut self) {
        self.filtered_indices = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                // Folder filter
                if let Some(ref folder) = self.folder_filter {
                    match &e.folder {
                        Some(f) if f == folder => {}
                        _ => return false,
                    }
                }
                // Search query
                if !self.search_query.is_empty() {
                    let query = self.search_query.to_lowercase();
                    if !(e.label.to_lowercase().contains(&query)
                        || e.username.to_lowercase().contains(&query)
                        || e.url.to_lowercase().contains(&query)
                        || e.tags.iter().any(|t| t.to_lowercase().contains(&query))
                        || e.custom_fields.iter().any(|cf| {
                            cf.name.to_lowercase().contains(&query)
                                || cf.value.to_lowercase().contains(&query)
                        }))
                    {
                        return false;
                    }
                }
                true
            })
            .map(|(i, _)| i)
            .collect();
    }

    fn collect_folders(&mut self) {
        let mut folders: Vec<String> = self
            .entries
            .iter()
            .filter_map(|e| e.folder.clone())
            .collect();
        folders.sort();
        folders.dedup();
        self.available_folders = folders;
    }

    fn generate_totp_code(secret: &str) -> Option<String> {
        let decoded = Secret::Encoded(secret.to_string()).to_bytes();
        if let Ok(bytes) = decoded {
            let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes);
            if let Ok(t) = totp {
                return t.generate_current().ok();
            }
        }
        None
    }

    fn export_vault_json(&self, path: &str) -> Result<()> {
        let vault = self
            .vault
            .as_ref()
            .ok_or_else(|| anyhow!("vault not loaded"))?;
        let key = self
            .vault_key
            .as_ref()
            .ok_or_else(|| anyhow!("vault not unlocked"))?;

        let mut export_entries = Vec::new();
        for entry in &vault.entries {
            let plaintext = decrypt(key, &entry.ciphertext)?;
            let secret: EntrySecret = serde_json::from_slice(&plaintext)?;
            export_entries.push(serde_json::json!({
                "label": entry.label,
                "username": entry.metadata.username,
                "password": secret.password,
                "url": entry.metadata.url,
                "notes": secret.notes,
                "email": secret.email,
                "phone": secret.phone,
                "tags": entry.metadata.tags,
                "folder": entry.metadata.folder,
                "entry_type": format!("{:?}", entry.metadata.entry_type),
                "custom_fields": secret.custom_fields.iter().map(|cf| {
                    serde_json::json!({"name": cf.name, "value": cf.value})
                }).collect::<Vec<_>>(),
            }));
        }

        let json = serde_json::to_string_pretty(&serde_json::json!({
            "vault_name": vault.name,
            "exported_at": lilypad_common::current_timestamp(),
            "entries": export_entries,
        }))?;

        std::fs::write(path, json)?;
        Ok(())
    }

    fn export_vault_csv(&self, path: &str) -> Result<()> {
        let mut wtr = csv::Writer::from_path(path)?;
        wtr.write_record([
            "label", "username", "password", "url", "notes", "email", "tags", "folder",
        ])?;

        for entry in &self.entries {
            wtr.write_record([
                &entry.label,
                &entry.username,
                &entry.password,
                &entry.url,
                &entry.notes,
                &entry.email,
                &entry.tags.join(";"),
                entry.folder.as_deref().unwrap_or(""),
            ])?;
        }

        wtr.flush()?;
        Ok(())
    }

    fn selected_entry_index(&self) -> Option<usize> {
        let list_idx = self.entry_list_state.selected()?;
        self.filtered_indices.get(list_idx).copied()
    }

    fn save_entry(&mut self) -> Result<()> {
        let label = self.input_label.trim();
        if label.is_empty() {
            return Err(anyhow!("label is required"));
        }
        let password = self.input_password.trim();
        if password.is_empty() {
            return Err(anyhow!("password is required"));
        }

        let key = self
            .vault_key
            .as_ref()
            .ok_or_else(|| anyhow!("vault not unlocked"))?
            .clone();

        let mut secret = if let Some(edit_idx) = self.edit_index {
            // When editing, preserve fields not exposed in the form
            self.vault
                .as_ref()
                .and_then(|v| v.entries.get(edit_idx))
                .and_then(|entry| decrypt(&key, &entry.ciphertext).ok())
                .and_then(|bytes| serde_json::from_slice::<EntrySecret>(&bytes).ok())
                .unwrap_or_else(|| EntrySecret::new(password))
        } else {
            EntrySecret::new(password)
        };

        secret.password = password.to_string();
        secret.notes = if self.input_notes.trim().is_empty() {
            None
        } else {
            Some(self.input_notes.trim().to_string())
        };
        secret.email = if self.input_email.trim().is_empty() {
            None
        } else {
            Some(self.input_email.trim().to_string())
        };

        let serialized = serde_json::to_vec(&secret)?;
        let ciphertext = encrypt(&key, &serialized)?;

        let tags: Vec<String> = self
            .input_tags
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();

        let metadata = EntryMetadata {
            username: if self.input_username.trim().is_empty() {
                None
            } else {
                Some(self.input_username.trim().to_string())
            },
            url: if self.input_url.trim().is_empty() {
                None
            } else {
                Some(self.input_url.trim().to_string())
            },
            tags,
            ..Default::default()
        };

        {
            let vault = self
                .vault
                .as_mut()
                .ok_or_else(|| anyhow!("vault not loaded"))?;

            if let Some(edit_idx) = self.edit_index {
                // Update existing entry
                if let Some(entry) = vault.entries.get(edit_idx) {
                    let old_label = entry.label.clone();
                    vault.update_entry(&old_label, ciphertext)?;
                    let _ = vault.update_entry_metadata(&old_label, metadata);
                }
            } else if vault.find_entry(label).is_some() {
                vault.update_entry(label, ciphertext)?;
                let _ = vault.update_entry_metadata(label, metadata);
            } else {
                vault.add_entry(Entry::new_with_metadata(label, metadata, ciphertext))?;
            }

            self.store.save_vault(vault, &key)?;
        }

        // Reload entries
        let vault = self
            .vault
            .as_ref()
            .ok_or_else(|| anyhow!("vault not loaded"))?;
        self.entries = Self::load_entries(vault, &key)?;
        self.update_filtered_indices();

        self.clear_form();
        let msg = if self.edit_index.is_some() {
            "Entry updated"
        } else {
            "Entry saved"
        };
        self.edit_index = None;
        self.state = AppState::Unlocked;
        self.set_status(msg);
        self.refresh_health();
        Ok(())
    }

    fn delete_selected_entry(&mut self) -> Result<()> {
        let index = match self.selected_entry_index() {
            Some(i) => i,
            None => return Ok(()),
        };

        let label = self.entries.get(index).map(|e| e.label.clone());
        if let Some(label) = label {
            let key = self
                .vault_key
                .as_ref()
                .ok_or_else(|| anyhow!("vault not unlocked"))?
                .clone();

            {
                let vault = self
                    .vault
                    .as_mut()
                    .ok_or_else(|| anyhow!("vault not loaded"))?;

                vault.remove_entry(&label)?;
                self.store.save_vault(vault, &key)?;
            }

            let vault = self
                .vault
                .as_ref()
                .ok_or_else(|| anyhow!("vault not loaded"))?;
            self.entries = Self::load_entries(vault, &key)?;
            self.update_filtered_indices();

            if self.filtered_indices.is_empty() {
                self.entry_list_state.select(None);
            } else {
                let sel = self.entry_list_state.selected().unwrap_or(0);
                if sel >= self.filtered_indices.len() {
                    self.entry_list_state
                        .select(Some(self.filtered_indices.len() - 1));
                }
            }

            self.set_status(format!("Deleted '{}'", label));
            self.refresh_health();
        }

        Ok(())
    }

    fn clear_form(&mut self) {
        self.input_label.clear();
        self.input_username.clear();
        self.input_password.clear();
        self.input_url.clear();
        self.input_notes.clear();
        self.input_email.clear();
        self.input_tags.clear();
    }

    fn start_edit(&mut self) {
        if let Some(idx) = self.selected_entry_index() {
            if let Some(entry) = self.entries.get(idx) {
                self.input_label = entry.label.clone();
                self.input_username = entry.username.clone();
                self.input_password = entry.password.clone();
                self.input_url = entry.url.clone();
                self.input_notes = entry.notes.clone();
                self.input_email = entry.email.clone();
                self.input_tags = entry.tags.join(", ");
                self.edit_index = Some(idx);
                self.current_field = InputField::Label;
                self.state = AppState::EditEntry;
            }
        }
    }

    fn generate_password(&mut self) {
        let mut charset = String::new();
        if self.gen_lowercase {
            charset.push_str("abcdefghijklmnopqrstuvwxyz");
        }
        if self.gen_uppercase {
            charset.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
        }
        if self.gen_digits {
            charset.push_str("0123456789");
        }
        if self.gen_symbols {
            charset.push_str("!@#$%^&*()_+-=[]{}|;:,.<>?");
        }
        if charset.is_empty() {
            charset.push_str("abcdefghijklmnopqrstuvwxyz");
        }

        let chars: Vec<char> = charset.chars().collect();
        let mut rng = rand::rng();
        self.gen_result = (0..self.gen_length)
            .map(|_| chars[rng.random_range(0..chars.len())])
            .collect();
    }

    fn refresh_health(&mut self) {
        let health_data: Vec<EntryHealthData> = self
            .entries
            .iter()
            .map(|e| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let password_age_days = (now.saturating_sub(e.updated_at)) / (24 * 60 * 60);

                EntryHealthData {
                    label: e.label.clone(),
                    password: e.password.clone(),
                    has_username: !e.username.is_empty(),
                    has_url: !e.url.is_empty(),
                    has_totp: e.totp_secret.is_some(),
                    password_age_days,
                    days_until_expiry: None,
                    is_expired: false,
                    is_compromised: false,
                }
            })
            .collect();

        self.health_report = Some(analyze_vault_health(&health_data));
    }

    fn switch_vault(&mut self, name: &str) {
        self.active_vault = name.to_string();
        self.vault = None;
        self.vault_key = None;
        self.entries.clear();
        self.filtered_indices.clear();
        self.health_report = None;
        self.search_query.clear();
        // Re-unlock with same password
        if let Err(e) = self.unlock_vault() {
            self.set_status(format!("Failed to switch vault: {}", e));
            self.state = AppState::Locked;
        }
    }

    // ========================================================================
    // Input Handling
    // ========================================================================

    fn handle_key_event(&mut self, key: event::KeyEvent) {
        match self.state {
            AppState::Locked => self.handle_locked_input(key),
            AppState::Unlocked => self.handle_unlocked_input(key),
            AppState::AddEntry | AppState::EditEntry => self.handle_form_input(key),
            AppState::ViewEntry => self.handle_view_entry_input(key),
            AppState::ConfirmDelete => self.handle_confirm_delete_input(key),
            AppState::Help => self.handle_help_input(key),
            AppState::Search => self.handle_search_input(key),
            AppState::GeneratePassword => self.handle_generator_input(key),
            AppState::SelectVault => self.handle_vault_select_input(key),
            AppState::HealthDashboard => self.handle_health_input(key),
            AppState::FilterByFolder => self.handle_folder_filter_input(key),
            AppState::ExportVault => self.handle_export_input(key),
        }
    }

    fn handle_locked_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Enter => {
                if let Err(e) = self.unlock_vault() {
                    self.set_status(format!("Unlock failed: {}", e));
                    self.master_password.clear();
                }
            }
            KeyCode::Char(c) => {
                self.master_password.push(c);
            }
            KeyCode::Backspace => {
                self.master_password.pop();
            }
            _ => {}
        }
    }

    fn handle_unlocked_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('a') => {
                self.clear_form();
                self.edit_index = None;
                self.state = AppState::AddEntry;
                self.current_field = InputField::Label;
            }
            KeyCode::Char('e') => {
                self.start_edit();
            }
            KeyCode::Char('d') => {
                if self.selected_entry_index().is_some() {
                    self.state = AppState::ConfirmDelete;
                }
            }
            KeyCode::Char('c') => {
                if let Some(idx) = self.selected_entry_index() {
                    let password = self.entries.get(idx).map(|e| e.password.clone());
                    if let Some(password) = password {
                        self.copy_to_clipboard(&password);
                    }
                }
            }
            KeyCode::Char('u') => {
                if let Some(idx) = self.selected_entry_index() {
                    let username = self.entries.get(idx).map(|e| e.username.clone());
                    if let Some(username) = username {
                        if !username.is_empty() {
                            self.copy_to_clipboard(&username);
                        } else {
                            self.set_status("No username to copy");
                        }
                    }
                }
            }
            KeyCode::Char('f') => {
                if let Some(idx) = self.selected_entry_index() {
                    let key = self.vault_key.clone();
                    let label = self.entries.get(idx).map(|e| e.label.clone());
                    if let (Some(key), Some(label)) = (key, label) {
                        if let Some(vault) = self.vault.as_mut() {
                            let new_fav = !self.entries[idx].is_favorite;
                            let _ = vault.set_entry_favorite(&label, new_fav);
                            let _ = self.store.save_vault(vault, &key);
                        }
                        if let Some(vault) = self.vault.as_ref() {
                            if let Ok(entries) = Self::load_entries(vault, &key) {
                                self.entries = entries;
                                self.update_filtered_indices();
                            }
                        }
                    }
                }
            }
            KeyCode::Enter => {
                if self.selected_entry_index().is_some() {
                    self.state = AppState::ViewEntry;
                    self.show_password = false;
                }
            }
            KeyCode::Char('/') => {
                self.state = AppState::Search;
                self.search_active = true;
            }
            KeyCode::Char('g') => {
                self.generate_password();
                self.state = AppState::GeneratePassword;
            }
            KeyCode::Char('?') => {
                self.state = AppState::Help;
            }
            KeyCode::Char('v') => {
                self.vault_list_state.select(Some(0));
                self.state = AppState::SelectVault;
            }
            KeyCode::Char('h') => {
                self.refresh_health();
                self.state = AppState::HealthDashboard;
            }
            KeyCode::Char('F') => {
                self.collect_folders();
                if !self.available_folders.is_empty() {
                    self.folder_list_state.select(Some(0));
                    self.state = AppState::FilterByFolder;
                } else {
                    self.set_status("No folders defined");
                }
            }
            KeyCode::Char('x') => {
                self.export_path.clear();
                self.state = AppState::ExportVault;
            }
            KeyCode::Char('t') => {
                // Generate and copy TOTP code for selected entry
                if let Some(idx) = self.selected_entry_index() {
                    if let Some(entry) = self.entries.get(idx) {
                        if let Some(ref secret) = entry.totp_secret {
                            if let Some(code) = Self::generate_totp_code(secret) {
                                self.copy_to_clipboard(&code);
                                self.set_status(format!("TOTP code: {}", code));
                            } else {
                                self.set_status("Failed to generate TOTP code");
                            }
                        } else {
                            self.set_status("No TOTP configured for this entry");
                        }
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.select_previous();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.select_next();
            }
            _ => {}
        }
    }

    fn handle_view_entry_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state = AppState::Unlocked;
            }
            KeyCode::Char('p') => {
                self.show_password = !self.show_password;
            }
            KeyCode::Char('c') => {
                if let Some(idx) = self.selected_entry_index() {
                    let password = self.entries.get(idx).map(|e| e.password.clone());
                    if let Some(password) = password {
                        self.copy_to_clipboard(&password);
                    }
                }
            }
            KeyCode::Char('u') => {
                if let Some(idx) = self.selected_entry_index() {
                    let username = self.entries.get(idx).map(|e| e.username.clone());
                    if let Some(username) = username {
                        if !username.is_empty() {
                            self.copy_to_clipboard(&username);
                        }
                    }
                }
            }
            KeyCode::Char('t') => {
                if let Some(idx) = self.selected_entry_index() {
                    if let Some(entry) = self.entries.get(idx) {
                        if let Some(ref secret) = entry.totp_secret {
                            if let Some(code) = Self::generate_totp_code(secret) {
                                self.copy_to_clipboard(&code);
                                self.set_status(format!("TOTP code: {}", code));
                            }
                        }
                    }
                }
            }
            KeyCode::Char('e') => {
                self.state = AppState::Unlocked;
                self.start_edit();
            }
            _ => {}
        }
    }

    fn handle_confirm_delete_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Err(e) = self.delete_selected_entry() {
                    self.set_status(format!("Delete failed: {}", e));
                }
                self.state = AppState::Unlocked;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.state = AppState::Unlocked;
                self.set_status("Delete cancelled");
            }
            _ => {}
        }
    }

    fn handle_help_input(&mut self, key: event::KeyEvent) {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')
        ) {
            self.state = AppState::Unlocked;
        }
    }

    fn handle_search_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search_query.clear();
                self.update_filtered_indices();
                self.search_active = false;
                self.state = AppState::Unlocked;
                self.entry_list_state
                    .select(if self.filtered_indices.is_empty() {
                        None
                    } else {
                        Some(0)
                    });
            }
            KeyCode::Enter => {
                self.search_active = false;
                self.state = AppState::Unlocked;
                self.entry_list_state
                    .select(if self.filtered_indices.is_empty() {
                        None
                    } else {
                        Some(0)
                    });
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.update_filtered_indices();
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.update_filtered_indices();
            }
            _ => {}
        }
    }

    fn handle_generator_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state = AppState::Unlocked;
            }
            KeyCode::Char('r') | KeyCode::Enter => {
                self.generate_password();
            }
            KeyCode::Char('c') => {
                if !self.gen_result.is_empty() {
                    self.copy_to_clipboard(&self.gen_result.clone());
                }
            }
            KeyCode::Char('+') => {
                if self.gen_length < 128 {
                    self.gen_length += 1;
                    self.generate_password();
                }
            }
            KeyCode::Char('-') => {
                if self.gen_length > 4 {
                    self.gen_length -= 1;
                    self.generate_password();
                }
            }
            KeyCode::Char('l') => {
                self.gen_lowercase = !self.gen_lowercase;
                self.generate_password();
            }
            KeyCode::Char('U') => {
                self.gen_uppercase = !self.gen_uppercase;
                self.generate_password();
            }
            KeyCode::Char('d') => {
                self.gen_digits = !self.gen_digits;
                self.generate_password();
            }
            KeyCode::Char('s') => {
                self.gen_symbols = !self.gen_symbols;
                self.generate_password();
            }
            _ => {}
        }
    }

    fn handle_vault_select_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state = AppState::Unlocked;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.vault_list_state.selected().unwrap_or(0);
                if i > 0 {
                    self.vault_list_state.select(Some(i - 1));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.vault_list_state.selected().unwrap_or(0);
                if i < self.available_vaults.len().saturating_sub(1) {
                    self.vault_list_state.select(Some(i + 1));
                }
            }
            KeyCode::Enter => {
                if let Some(i) = self.vault_list_state.selected() {
                    if let Some(name) = self.available_vaults.get(i).cloned() {
                        if name != self.active_vault {
                            self.switch_vault(&name);
                        }
                        self.state = AppState::Unlocked;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_health_input(&mut self, key: event::KeyEvent) {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            self.state = AppState::Unlocked;
        }
    }

    fn handle_folder_filter_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state = AppState::Unlocked;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.folder_list_state.selected().unwrap_or(0);
                if i > 0 {
                    self.folder_list_state.select(Some(i - 1));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.folder_list_state.selected().unwrap_or(0);
                // +1 for the "All (no filter)" option
                if i < self.available_folders.len() {
                    self.folder_list_state.select(Some(i + 1));
                }
            }
            KeyCode::Enter => {
                if let Some(i) = self.folder_list_state.selected() {
                    if i == 0 {
                        // "All" option - remove filter
                        self.folder_filter = None;
                        self.set_status("Folder filter cleared");
                    } else if let Some(folder) = self.available_folders.get(i - 1).cloned() {
                        self.folder_filter = Some(folder.clone());
                        self.set_status(format!("Filtering by folder: {}", folder));
                    }
                    self.update_filtered_indices();
                    self.entry_list_state
                        .select(if self.filtered_indices.is_empty() {
                            None
                        } else {
                            Some(0)
                        });
                }
                self.state = AppState::Unlocked;
            }
            _ => {}
        }
    }

    fn handle_export_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.state = AppState::Unlocked;
            }
            KeyCode::Enter => {
                let path = self.export_path.trim().to_string();
                if path.is_empty() {
                    self.set_status("Export path cannot be empty");
                } else if path.ends_with(".csv") {
                    match self.export_vault_csv(&path) {
                        Ok(()) => self.set_status(format!(
                            "Exported {} entries to {}",
                            self.entries.len(),
                            path
                        )),
                        Err(e) => self.set_status(format!("Export failed: {}", e)),
                    }
                    self.state = AppState::Unlocked;
                } else {
                    let path = if path.ends_with(".json") {
                        path
                    } else {
                        format!("{}.json", path)
                    };
                    match self.export_vault_json(&path) {
                        Ok(()) => self.set_status(format!(
                            "Exported {} entries to {}",
                            self.entries.len(),
                            path
                        )),
                        Err(e) => self.set_status(format!("Export failed: {}", e)),
                    }
                    self.state = AppState::Unlocked;
                }
            }
            KeyCode::Char(c) => {
                self.export_path.push(c);
            }
            KeyCode::Backspace => {
                self.export_path.pop();
            }
            _ => {}
        }
    }

    fn handle_form_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.clear_form();
                self.edit_index = None;
                self.state = AppState::Unlocked;
            }
            KeyCode::Tab => {
                self.current_field = match self.current_field {
                    InputField::Label => InputField::Username,
                    InputField::Username => InputField::EntryPassword,
                    InputField::EntryPassword => InputField::Url,
                    InputField::Url => InputField::Email,
                    InputField::Email => InputField::Notes,
                    InputField::Notes => InputField::Tags,
                    InputField::Tags => InputField::Label,
                    _ => InputField::Label,
                };
            }
            KeyCode::BackTab => {
                self.current_field = match self.current_field {
                    InputField::Label => InputField::Tags,
                    InputField::Username => InputField::Label,
                    InputField::EntryPassword => InputField::Username,
                    InputField::Url => InputField::EntryPassword,
                    InputField::Email => InputField::Url,
                    InputField::Notes => InputField::Email,
                    InputField::Tags => InputField::Notes,
                    _ => InputField::Label,
                };
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Err(e) = self.save_entry() {
                    self.set_status(format!("Save failed: {}", e));
                }
            }
            KeyCode::Char(c) => {
                let field = self.current_field_mut();
                field.push(c);
            }
            KeyCode::Backspace => {
                let field = self.current_field_mut();
                field.pop();
            }
            _ => {}
        }
    }

    fn current_field_mut(&mut self) -> &mut String {
        match self.current_field {
            InputField::Label => &mut self.input_label,
            InputField::Username => &mut self.input_username,
            InputField::EntryPassword | InputField::Password => &mut self.input_password,
            InputField::Url => &mut self.input_url,
            InputField::Notes => &mut self.input_notes,
            InputField::Email => &mut self.input_email,
            InputField::Tags => &mut self.input_tags,
        }
    }

    fn select_next(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let i = match self.entry_list_state.selected() {
            Some(i) => {
                if i >= self.filtered_indices.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.entry_list_state.select(Some(i));
    }

    fn select_previous(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let i = match self.entry_list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.filtered_indices.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.entry_list_state.select(Some(i));
    }

    // ========================================================================
    // Drawing
    // ========================================================================

    fn draw(&mut self, frame: &mut Frame) {
        match self.state {
            AppState::Locked => self.draw_locked_screen(frame),
            AppState::Unlocked => self.draw_unlocked_screen(frame),
            AppState::AddEntry => self.draw_form_screen(frame, "Add New Entry"),
            AppState::EditEntry => self.draw_form_screen(frame, "Edit Entry"),
            AppState::ViewEntry => self.draw_view_entry_screen(frame),
            AppState::ConfirmDelete => {
                self.draw_unlocked_screen(frame);
                self.draw_confirm_delete_popup(frame);
            }
            AppState::Help => self.draw_help_screen(frame),
            AppState::Search => self.draw_search_screen(frame),
            AppState::GeneratePassword => self.draw_generator_screen(frame),
            AppState::SelectVault => {
                self.draw_unlocked_screen(frame);
                self.draw_vault_select_popup(frame);
            }
            AppState::HealthDashboard => self.draw_health_screen(frame),
            AppState::FilterByFolder => {
                self.draw_unlocked_screen(frame);
                self.draw_folder_filter_popup(frame);
            }
            AppState::ExportVault => {
                self.draw_unlocked_screen(frame);
                self.draw_export_popup(frame);
            }
        }
    }

    fn draw_locked_screen(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        let title = Paragraph::new(format!(
            "Lilypad Password Manager - Vault: {}",
            self.active_vault
        ))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default());
        frame.render_widget(title, chunks[0]);

        let password_display = "*".repeat(self.master_password.len());
        let password_input = Paragraph::new(password_display)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Master Password"),
            )
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(password_input, chunks[1]);

        let help =
            Paragraph::new("Enter: Unlock | Esc: Quit").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(help, chunks[2]);

        if let Some(ref msg) = self.status_message {
            let status = Paragraph::new(msg.as_str()).style(Style::default().fg(Color::Red));
            frame.render_widget(status, chunks[3]);
        }
    }

    fn draw_unlocked_screen(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);

        // Header with vault name, search, and filter indicators
        let folder_info = self
            .folder_filter
            .as_ref()
            .map(|f| format!(" [folder: {}]", f))
            .unwrap_or_default();
        let header_text = if self.search_query.is_empty() {
            format!(
                "Lilypad - {}{} ({} entries)",
                self.active_vault,
                folder_info,
                self.filtered_indices.len()
            )
        } else {
            format!(
                "Lilypad - {}{} (search: '{}', {} results)",
                self.active_vault,
                folder_info,
                self.search_query,
                self.filtered_indices.len()
            )
        };
        let title = Paragraph::new(header_text)
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::BOTTOM));
        frame.render_widget(title, chunks[0]);

        // Entry list
        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .filter_map(|&idx| self.entries.get(idx))
            .map(|entry| {
                let username = if entry.username.is_empty() {
                    "(no username)".to_string()
                } else {
                    entry.username.clone()
                };
                let updated = format_timestamp_relative(entry.updated_at);

                let mut spans = vec![
                    if entry.is_favorite {
                        Span::styled("* ", Style::default().fg(Color::Yellow))
                    } else {
                        Span::raw("  ")
                    },
                    Span::styled(&entry.label, Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" - "),
                    Span::styled(username, Style::default().fg(Color::Gray)),
                ];

                // Strength indicator
                let strength_color = match entry.password_strength {
                    PasswordStrength::VeryWeak => Color::Red,
                    PasswordStrength::Weak => Color::LightRed,
                    PasswordStrength::Fair => Color::Yellow,
                    PasswordStrength::Strong => Color::Green,
                    PasswordStrength::VeryStrong => Color::Cyan,
                };
                spans.push(Span::raw(" "));
                spans.push(Span::styled("●", Style::default().fg(strength_color)));

                if !entry.tags.is_empty() {
                    spans.push(Span::raw(" ["));
                    spans.push(Span::styled(
                        entry.tags.join(", "),
                        Style::default().fg(Color::Magenta),
                    ));
                    spans.push(Span::raw("]"));
                }

                spans.push(Span::raw(" ("));
                spans.push(Span::styled(updated, Style::default().fg(Color::DarkGray)));
                spans.push(Span::raw(")"));

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Entries"))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, chunks[1], &mut self.entry_list_state);

        // Footer
        let help_text =
            "a:Add e:Edit d:Del c:Copy u:User f:Fav t:TOTP /:Search g:Gen v:Vault F:Folder x:Export h:Health ?:Help";
        let status_text = self.status_message.as_deref().unwrap_or(help_text);
        let help = Paragraph::new(status_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::TOP));
        frame.render_widget(help, chunks[2]);
    }

    fn draw_form_screen(&self, frame: &mut Frame, title: &str) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        let header = Paragraph::new(title).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(header, chunks[0]);

        let fields: Vec<(&str, &str, InputField)> = vec![
            ("Label", &self.input_label, InputField::Label),
            ("Username", &self.input_username, InputField::Username),
            ("Password", &self.input_password, InputField::EntryPassword),
            ("URL", &self.input_url, InputField::Url),
            ("Email", &self.input_email, InputField::Email),
            ("Notes", &self.input_notes, InputField::Notes),
            ("Tags (comma-separated)", &self.input_tags, InputField::Tags),
        ];

        for (i, (name, value, field)) in fields.iter().enumerate() {
            let style = if *field == self.current_field {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let display_value = if *field == InputField::EntryPassword {
                "*".repeat(value.len())
            } else {
                value.to_string()
            };

            let input = Paragraph::new(display_value)
                .block(Block::default().borders(Borders::ALL).title(*name))
                .style(style);
            frame.render_widget(input, chunks[i + 1]);
        }

        let help = Paragraph::new("Tab/Shift+Tab: Navigate | Ctrl+Enter: Save | Esc: Cancel")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(help, chunks[8]);
    }

    fn draw_view_entry_screen(&self, frame: &mut Frame) {
        let area = frame.area();

        let entry = match self.selected_entry_index() {
            Some(idx) => match self.entries.get(idx) {
                Some(e) => e,
                None => return,
            },
            None => return,
        };

        let popup_area = centered_rect(70, 70, area);
        frame.render_widget(Clear, popup_area);

        let fav_marker = if entry.is_favorite { " *" } else { "" };
        let type_marker = if entry.entry_type != "Login" {
            format!(" [{}]", entry.entry_type)
        } else {
            String::new()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {}{}{} ", entry.label, fav_marker, type_marker))
            .style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Username: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(if entry.username.is_empty() {
                    "(none)"
                } else {
                    &entry.username
                }),
            ]),
            Line::from(vec![
                Span::styled("Password: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(if self.show_password {
                    &entry.password
                } else {
                    "********"
                }),
            ]),
            Line::from(vec![
                Span::styled("Strength: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:?}", entry.password_strength),
                    Style::default().fg(match entry.password_strength {
                        PasswordStrength::VeryWeak | PasswordStrength::Weak => Color::Red,
                        PasswordStrength::Fair => Color::Yellow,
                        PasswordStrength::Strong | PasswordStrength::VeryStrong => Color::Green,
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("URL: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(if entry.url.is_empty() {
                    "(none)"
                } else {
                    &entry.url
                }),
            ]),
        ];

        if !entry.email.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Email: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&entry.email),
            ]));
        }

        if !entry.phone.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Phone: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&entry.phone),
            ]));
        }

        // TOTP with live code
        if let Some(ref secret) = entry.totp_secret {
            let code_display =
                Self::generate_totp_code(secret).unwrap_or_else(|| "Error".to_string());
            lines.push(Line::from(vec![
                Span::styled(
                    "TOTP: ".to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    code_display,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " (t to copy)".to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    "TOTP: ".to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Not configured".to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        // Custom fields
        if !entry.custom_fields.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Custom Fields:",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            )));
            for cf in &entry.custom_fields {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {}: ", cf.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(&cf.value),
                ]));
            }
        }

        if !entry.notes.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Notes: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&entry.notes),
            ]));
        }

        if !entry.tags.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Tags: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(entry.tags.join(", "), Style::default().fg(Color::Magenta)),
            ]));
        }

        if let Some(ref folder) = entry.folder {
            lines.push(Line::from(vec![
                Span::styled("Folder: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(folder.as_str()),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Updated: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format_timestamp_relative(entry.updated_at)),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "p:Toggle Password | c:Copy Password | u:Copy Username | t:Copy TOTP | e:Edit | Esc:Close",
            Style::default().fg(Color::DarkGray),
        )));

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, inner);
    }

    fn draw_confirm_delete_popup(&self, frame: &mut Frame) {
        let area = frame.area();
        let popup_area = centered_rect(50, 20, area);
        frame.render_widget(Clear, popup_area);

        let entry_name = self
            .selected_entry_index()
            .and_then(|idx| self.entries.get(idx))
            .map(|e| e.label.as_str())
            .unwrap_or("this entry");

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Confirm Delete ")
            .style(Style::default().fg(Color::Red));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let text = Paragraph::new(vec![
            Line::from(""),
            Line::from(format!("Delete '{}'?", entry_name)),
            Line::from(""),
            Line::from(Span::styled(
                "y: Yes, delete | n/Esc: Cancel",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .style(Style::default().fg(Color::Yellow));
        frame.render_widget(text, inner);
    }

    fn draw_vault_select_popup(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let popup_area = centered_rect(40, 40, area);
        frame.render_widget(Clear, popup_area);

        let items: Vec<ListItem> = self
            .available_vaults
            .iter()
            .map(|name| {
                let marker = if *name == self.active_vault {
                    " (active)"
                } else {
                    ""
                };
                ListItem::new(format!("{}{}", name, marker))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Select Vault ")
                    .style(Style::default().fg(Color::Cyan)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, popup_area, &mut self.vault_list_state);
    }

    fn draw_help_screen(&self, frame: &mut Frame) {
        let area = frame.area();
        let popup_area = centered_rect(70, 80, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Help - Keyboard Shortcuts ")
            .style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let lines = vec![
            Line::from(Span::styled(
                "Vault (Unlocked)",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            )),
            Line::from("  a       Add new entry"),
            Line::from("  e       Edit selected entry"),
            Line::from("  d       Delete selected entry (with confirmation)"),
            Line::from("  c       Copy password to clipboard"),
            Line::from("  u       Copy username to clipboard"),
            Line::from("  f       Toggle favorite"),
            Line::from("  Enter   View entry details"),
            Line::from("  t       Copy TOTP code"),
            Line::from("  /       Search entries"),
            Line::from("  g       Password generator"),
            Line::from("  v       Switch vault"),
            Line::from("  F       Filter by folder"),
            Line::from("  x       Export vault (JSON/CSV)"),
            Line::from("  h       Health dashboard"),
            Line::from("  ?       This help screen"),
            Line::from("  j/k     Navigate up/down"),
            Line::from("  q/Esc   Quit"),
            Line::from(""),
            Line::from(Span::styled(
                "Entry View",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            )),
            Line::from("  p       Toggle password visibility"),
            Line::from("  c       Copy password"),
            Line::from("  u       Copy username"),
            Line::from("  t       Copy TOTP code"),
            Line::from("  e       Edit entry"),
            Line::from("  Esc     Close"),
            Line::from(""),
            Line::from(Span::styled(
                "Add/Edit Form",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            )),
            Line::from("  Tab         Next field"),
            Line::from("  Shift+Tab   Previous field"),
            Line::from("  Ctrl+Enter  Save"),
            Line::from("  Esc         Cancel"),
            Line::from(""),
            Line::from(Span::styled(
                "Password Generator",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            )),
            Line::from("  r/Enter  Regenerate"),
            Line::from("  c        Copy to clipboard"),
            Line::from("  +/-      Adjust length"),
            Line::from("  l/U/d/s  Toggle lower/upper/digits/symbols"),
            Line::from(""),
            Line::from(Span::styled(
                "Press Esc or ? to close",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn draw_search_screen(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);

        // Search input
        let search_input = Paragraph::new(self.search_query.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Search (Enter to confirm, Esc to cancel)")
                    .style(Style::default().fg(Color::Yellow)),
            )
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(search_input, chunks[0]);

        // Filtered results
        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .filter_map(|&idx| self.entries.get(idx))
            .map(|entry| {
                let username = if entry.username.is_empty() {
                    "(no username)"
                } else {
                    &entry.username
                };
                ListItem::new(Line::from(vec![
                    Span::styled(&entry.label, Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" - "),
                    Span::styled(username, Style::default().fg(Color::Gray)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("{} results", self.filtered_indices.len())),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, chunks[1], &mut self.entry_list_state);

        let help = Paragraph::new("Type to search | Enter: Confirm | Esc: Cancel & clear")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::TOP));
        frame.render_widget(help, chunks[2]);
    }

    fn draw_generator_screen(&self, frame: &mut Frame) {
        let area = frame.area();
        let popup_area = centered_rect(60, 50, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Password Generator ")
            .style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let strength = validate_password_strength(&self.gen_result);
        let strength_color = match strength {
            PasswordStrength::VeryWeak | PasswordStrength::Weak => Color::Red,
            PasswordStrength::Fair => Color::Yellow,
            PasswordStrength::Strong | PasswordStrength::VeryStrong => Color::Green,
        };

        let on = Style::default().fg(Color::Green);
        let off = Style::default().fg(Color::DarkGray);

        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Generated: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    &self.gen_result,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Strength:  ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:?}", strength),
                    Style::default().fg(strength_color),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Length:    ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{} (+/- to adjust)", self.gen_length)),
            ]),
            Line::from(vec![
                Span::styled("[l] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(
                        "Lowercase: {}",
                        if self.gen_lowercase { "ON" } else { "OFF" }
                    ),
                    if self.gen_lowercase { on } else { off },
                ),
            ]),
            Line::from(vec![
                Span::styled("[U] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(
                        "Uppercase: {}",
                        if self.gen_uppercase { "ON" } else { "OFF" }
                    ),
                    if self.gen_uppercase { on } else { off },
                ),
            ]),
            Line::from(vec![
                Span::styled("[d] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("Digits:    {}", if self.gen_digits { "ON" } else { "OFF" }),
                    if self.gen_digits { on } else { off },
                ),
            ]),
            Line::from(vec![
                Span::styled("[s] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("Symbols:   {}", if self.gen_symbols { "ON" } else { "OFF" }),
                    if self.gen_symbols { on } else { off },
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "r/Enter: Regenerate | c: Copy | Esc: Close",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn draw_health_screen(&self, frame: &mut Frame) {
        let area = frame.area();
        let popup_area = centered_rect(70, 80, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Health Dashboard ")
            .style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let report = match &self.health_report {
            Some(r) => r,
            None => {
                let msg = Paragraph::new("No health report available. Unlock a vault first.");
                frame.render_widget(msg, inner);
                return;
            }
        };

        let grade_color = match report.score.grade {
            lilypad_common::HealthGrade::A => Color::Green,
            lilypad_common::HealthGrade::B => Color::LightGreen,
            lilypad_common::HealthGrade::C => Color::Yellow,
            lilypad_common::HealthGrade::D => Color::LightRed,
            lilypad_common::HealthGrade::F => Color::Red,
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Score: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(
                        "{}/100 (Grade: {:?})",
                        report.score.score, report.score.grade
                    ),
                    Style::default()
                        .fg(grade_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![Span::styled(
                report.score.grade.description(),
                Style::default().fg(grade_color),
            )]),
            Line::from(""),
            Line::from(Span::styled(
                "Statistics",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            )),
            Line::from(format!(
                "  Total entries:      {}",
                report.stats.total_entries
            )),
            Line::from(format!(
                "  Strong passwords:   {}",
                report.stats.strong_passwords
            )),
            Line::from(format!(
                "  Weak passwords:     {}",
                report.stats.weak_passwords
            )),
            Line::from(format!(
                "  Reused passwords:   {}",
                report.stats.reused_passwords
            )),
            Line::from(format!("  With 2FA:           {}", report.stats.with_2fa)),
            Line::from(format!(
                "  Unique passwords:   {}",
                report.stats.unique_passwords
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Score Breakdown",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            )),
            Line::from(format!(
                "  Password strength:  {}/25",
                report.score.breakdown.password_strength
            )),
            Line::from(format!(
                "  Uniqueness:         {}/25",
                report.score.breakdown.uniqueness
            )),
            Line::from(format!(
                "  Freshness:          {}/25",
                report.score.breakdown.freshness
            )),
            Line::from(format!(
                "  Two-factor:         {}/25",
                report.score.breakdown.two_factor
            )),
        ];

        if !report.issues.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("Issues ({})", report.issues.len()),
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            )));

            for issue in report.issues.iter().take(10) {
                let severity_color = match issue.severity {
                    lilypad_common::IssueSeverity::Critical => Color::Red,
                    lilypad_common::IssueSeverity::Warning => Color::Yellow,
                    lilypad_common::IssueSeverity::Info => Color::Blue,
                };
                let severity_label = match issue.severity {
                    lilypad_common::IssueSeverity::Critical => "CRIT",
                    lilypad_common::IssueSeverity::Warning => "WARN",
                    lilypad_common::IssueSeverity::Info => "INFO",
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  [{}] ", severity_label),
                        Style::default().fg(severity_color),
                    ),
                    Span::raw(&issue.title),
                    Span::styled(
                        format!(" ({} entries)", issue.affected_entries.len()),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press Esc to close",
            Style::default().fg(Color::DarkGray),
        )));

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, inner);
    }

    fn draw_folder_filter_popup(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let popup_area = centered_rect(40, 40, area);
        frame.render_widget(Clear, popup_area);

        let mut items: Vec<ListItem> = vec![ListItem::new(Line::from(vec![
            Span::styled("All", Style::default().add_modifier(Modifier::BOLD)),
            if self.folder_filter.is_none() {
                Span::styled(" (active)", Style::default().fg(Color::Green))
            } else {
                Span::raw("")
            },
        ]))];

        for folder in &self.available_folders {
            let is_active = self.folder_filter.as_deref() == Some(folder);
            items.push(ListItem::new(Line::from(vec![
                Span::raw(format!("  {}", folder)),
                if is_active {
                    Span::styled(" (active)", Style::default().fg(Color::Green))
                } else {
                    Span::raw("")
                },
            ])));
        }

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Filter by Folder ")
                    .style(Style::default().fg(Color::Cyan)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, popup_area, &mut self.folder_list_state);
    }

    fn draw_export_popup(&self, frame: &mut Frame) {
        let area = frame.area();
        let popup_area = centered_rect(60, 25, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Export Vault ")
            .style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Enter file path (.json or .csv):",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                &self.export_path,
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Enter: Export | Esc: Cancel",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
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
        .split(popup_layout[1])[1]
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new()?;

    // Main loop
    loop {
        // Check timers
        app.check_status_clear();
        app.check_clipboard_clear();

        // Draw
        terminal.draw(|f| app.draw(f))?;

        // Handle events with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key_event(key);
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
