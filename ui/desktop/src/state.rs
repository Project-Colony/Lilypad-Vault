//! Lilypad Application State
//!
//! Contains all the state structures for the application.

use lilypad_common::PasswordStrength;
use lilypad_core::{CustomField, EntryColor, EntryType};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// Default vault name
pub const DEFAULT_VAULT_NAME: &str = "primary";

/// Maximum login attempts before lockout
pub const MAX_LOGIN_ATTEMPTS: u32 = 5;

/// Lockout duration in seconds (5 minutes)
pub const LOCKOUT_DURATION_SECS: u64 = 300;

/// Current settings format version for migration support
pub const SETTINGS_VERSION: u32 = 1;

/// Whether the user is creating a new vault or unlocking an existing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnlockMode {
    /// A vault exists; user is unlocking it.
    #[default]
    Unlock,
    /// No vault exists yet; user is creating one.
    Create,
}

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
    // Account settings (persisted)
    #[serde(default)]
    pub account_display_name: String,
    #[serde(default)]
    pub account_email: String,
    #[serde(default)]
    pub account_timezone: String,
    #[serde(default)]
    pub account_two_factor_enabled: bool,
    #[serde(default)]
    pub account_marketing_opt_in: bool,
    #[serde(default)]
    pub security_recovery_email: String,
    #[serde(default)]
    pub active_vault: String,
    #[serde(default)]
    pub ui_variation_index: usize,
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
            account_display_name: String::new(),
            account_email: String::new(),
            account_timezone: String::new(),
            account_two_factor_enabled: false,
            account_marketing_opt_in: false,
            security_recovery_email: String::new(),
            active_vault: String::new(),
            ui_variation_index: 0,
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

/// A decrypted vault entry with health metadata.
///
/// Contains data from both `EntryMetadata` (unencrypted) and `EntrySecret`
/// (decrypted), providing a complete view of the entry for the Desktop UI.
#[derive(Debug, Clone)]
pub struct VaultEntry {
    // Core fields
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,

    // Additional secret fields (from EntrySecret)
    pub email: String,
    pub phone: String,
    pub totp_secret: Option<String>,
    pub custom_fields: Vec<CustomField>,
    pub attachments: Vec<(String, usize)>, // (filename, data_size_bytes)

    // Metadata fields (from EntryMetadata, unencrypted)
    pub tags: Vec<String>,
    pub folder: Option<String>,
    pub entry_type: EntryType,

    // Display / health data
    pub last_updated: String,
    pub updated_at: u64,
    pub is_favorite: bool,
    pub last_accessed_at: Option<u64>,
    pub access_count: u64,
    pub password_strength: PasswordStrength,
    pub is_expired: bool,
    pub color: Option<EntryColor>,
    pub totp_code: Option<String>,
}

impl Drop for VaultEntry {
    fn drop(&mut self) {
        self.password.zeroize();
        if let Some(ref mut totp) = self.totp_secret {
            totp.zeroize();
        }
    }
}

/// Onboarding flow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnboardingStep {
    #[default]
    Welcome,
    Features,
    Colony,
    Tutorial,
    GitHubConnect,
    CreateVault,
}

impl OnboardingStep {
    pub const COUNT: usize = 6;

    pub fn index(&self) -> usize {
        match self {
            OnboardingStep::Welcome => 0,
            OnboardingStep::Features => 1,
            OnboardingStep::Colony => 2,
            OnboardingStep::Tutorial => 3,
            OnboardingStep::GitHubConnect => 4,
            OnboardingStep::CreateVault => 5,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => OnboardingStep::Welcome,
            1 => OnboardingStep::Features,
            2 => OnboardingStep::Colony,
            3 => OnboardingStep::Tutorial,
            4 => OnboardingStep::GitHubConnect,
            5 => OnboardingStep::CreateVault,
            _ => OnboardingStep::CreateVault,
        }
    }

    pub fn next(&self) -> Option<OnboardingStep> {
        let i = self.index();
        if i + 1 < Self::COUNT {
            Some(Self::from_index(i + 1))
        } else {
            None
        }
    }

    pub fn prev(&self) -> Option<OnboardingStep> {
        let i = self.index();
        if i > 0 {
            Some(Self::from_index(i - 1))
        } else {
            None
        }
    }
}

/// State within the mini-tutorial onboarding step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TutorialPhase {
    #[default]
    GeneratePassword,
    ViewVault,
    CopyPassword,
}

impl TutorialPhase {
    pub fn next(&self) -> Option<TutorialPhase> {
        match self {
            TutorialPhase::GeneratePassword => Some(TutorialPhase::ViewVault),
            TutorialPhase::ViewVault => Some(TutorialPhase::CopyPassword),
            TutorialPhase::CopyPassword => None,
        }
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

    pub fn to_index(self) -> usize {
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

