//! Lilypad Application State
//!
//! Contains all the state structures for the application.

use lilypad_common::{HealthReport, PasswordStrength};
use lilypad_core::{AppConfig, EntryColor, KeyMaterial, Vault};
use lilypad_storage::LocalStore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;
use zeroize::Zeroize;

use crate::theme::LilypadTheme;

/// Default vault name
pub const DEFAULT_VAULT_NAME: &str = "primary";

/// Maximum login attempts before lockout
pub const MAX_LOGIN_ATTEMPTS: u32 = 5;

/// Lockout duration in seconds (5 minutes)
pub const LOCKOUT_DURATION_SECS: u64 = 300;

/// Current settings format version for migration support
pub const SETTINGS_VERSION: u32 = 1;

/// View modes for the vault section
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VaultViewMode {
    #[default]
    All,
    Favorites,
    Recent,
    Weak,
    Expired,
}

impl VaultViewMode {
    /// Get display name for the view mode
    pub fn label(&self) -> &'static str {
        match self {
            VaultViewMode::All => "All",
            VaultViewMode::Favorites => "Favorites",
            VaultViewMode::Recent => "Recent",
            VaultViewMode::Weak => "Weak",
            VaultViewMode::Expired => "Expired",
        }
    }

    /// Get all view modes
    pub const ALL: [VaultViewMode; 5] = [
        VaultViewMode::All,
        VaultViewMode::Favorites,
        VaultViewMode::Recent,
        VaultViewMode::Weak,
        VaultViewMode::Expired,
    ];
}

/// Entry payload for desktop (decrypted data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopEntryPayload {
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
}

/// Persisted application settings with versioning for forward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub version: u32,
    pub theme_index: usize,
    pub auto_lock_minutes: u32,
    pub clipboard_timeout_seconds: u32,
    pub send_security_alerts: bool,
    pub require_master_on_copy: bool,
    #[serde(default)]
    pub exclude_ambiguous_chars: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            theme_index: 0,
            auto_lock_minutes: 10,
            clipboard_timeout_seconds: 30,
            send_security_alerts: true,
            require_master_on_copy: false,
            exclude_ambiguous_chars: false,
        }
    }
}

/// Persisted lockout state for brute-force protection
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LockoutState {
    pub failed_attempts: u32,
    /// Unix timestamp when lockout expires (0 if not locked out)
    pub lockout_until_timestamp: u64,
}

impl LockoutState {
    pub fn is_locked_out(&self) -> bool {
        if self.lockout_until_timestamp == 0 {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now < self.lockout_until_timestamp
    }

    pub fn remaining_secs(&self) -> u64 {
        if self.lockout_until_timestamp == 0 {
            return 0;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.lockout_until_timestamp.saturating_sub(now)
    }

    pub fn record_failure(&mut self) {
        self.failed_attempts += 1;
        if self.failed_attempts >= MAX_LOGIN_ATTEMPTS {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.lockout_until_timestamp = now + LOCKOUT_DURATION_SECS;
        }
    }

    pub fn reset(&mut self) {
        self.failed_attempts = 0;
        self.lockout_until_timestamp = 0;
    }
}

/// A decrypted vault entry with health metadata
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct VaultEntry {
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
    pub last_updated: String,
    pub updated_at: u64,
    pub is_favorite: bool,
    pub last_accessed_at: Option<u64>,
    pub access_count: u64,
    pub password_strength: PasswordStrength,
    pub is_expired: bool,
    pub color: Option<EntryColor>,
}

impl Drop for VaultEntry {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

/// Navigation category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Category {
    #[default]
    Credentials,
    Health,
    Generator,
    Sync,
    Account,
    Security,
}

#[allow(dead_code)]
impl Category {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Category::Credentials,
            1 => Category::Health,
            2 => Category::Generator,
            3 => Category::Sync,
            4 => Category::Account,
            5 => Category::Security,
            _ => Category::Credentials,
        }
    }

    pub fn to_index(&self) -> usize {
        match self {
            Category::Credentials => 0,
            Category::Health => 1,
            Category::Generator => 2,
            Category::Sync => 3,
            Category::Account => 4,
            Category::Security => 5,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Category::Credentials => "Credentials",
            Category::Health => "Health",
            Category::Generator => "Generator",
            Category::Sync => "Sync",
            Category::Account => "Account",
            Category::Security => "Security",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Category::Credentials => "🔐",
            Category::Health => "💚",
            Category::Generator => "🎲",
            Category::Sync => "🔄",
            Category::Account => "👤",
            Category::Security => "🛡",
        }
    }
}

/// Main application state (alternative struct for future refactoring)
#[allow(dead_code)]
pub struct LilypadState {
    // Application mode
    pub show_welcome: bool,
    pub vault_unlocked: bool,

    // Search
    pub search_query: String,

    // Navigation
    pub selected_category: usize,
    pub vault_view_mode: VaultViewMode,

    // Status messages
    pub status_message: Option<String>,
    pub status_message_time: Option<Instant>,

    // Paths
    pub welcome_ack_path: Option<PathBuf>,
    pub settings_path: Option<PathBuf>,
    pub lockout_path: Option<PathBuf>,

    // Sensitive data (zeroized on drop)
    pub master_password: String,
    pub generated_password: String,

    // Core
    pub config: AppConfig,
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
    pub health_report: Option<HealthReport>,
}

impl Drop for LilypadState {
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
