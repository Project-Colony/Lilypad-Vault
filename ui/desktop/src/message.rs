//! Lilypad Message System
//!
//! Defines all the messages (events) that can occur in the application.
//! Iced uses a message-based architecture for state management.

use crate::state::VaultViewMode;
use crate::theme::LilypadTheme;

/// Main application message enum
/// Some variants are defined for future features
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    // ========================================================================
    // Navigation
    // ========================================================================
    /// Change the selected navigation category (0-4)
    SelectCategory(usize),
    /// Change vault view mode filter
    SetViewMode(VaultViewMode),

    // ========================================================================
    // Authentication
    // ========================================================================
    /// Master password input changed
    MasterPasswordChanged(String),
    /// Attempt to unlock the vault
    UnlockVault,
    /// Lock the vault (manual or auto-lock)
    LockVault,
    /// Acknowledge the welcome screen
    AcknowledgeWelcome,

    // ========================================================================
    // Search
    // ========================================================================
    /// Search query changed
    SearchChanged(String),
    /// Clear search
    ClearSearch,

    // ========================================================================
    // Entry Management
    // ========================================================================
    /// Show add/edit entry form
    ShowAddEntry,
    /// Hide add/edit entry form
    HideAddEntry,
    /// Entry title changed
    EntryTitleChanged(String),
    /// Entry username changed
    EntryUsernameChanged(String),
    /// Entry password changed
    EntryPasswordChanged(String),
    /// Entry URL changed
    EntryUrlChanged(String),
    /// Entry notes changed
    EntryNotesChanged(String),
    /// Save the current entry (add or update)
    SaveEntry,
    /// Start editing an entry by index
    EditEntry(usize),
    /// Delete an entry by index
    DeleteEntry(usize),
    /// Confirm deletion
    ConfirmDelete,
    /// Cancel deletion
    CancelDelete,
    /// Toggle entry as favorite
    ToggleFavorite(usize),
    /// Copy username to clipboard
    CopyUsername(usize),
    /// Copy password to clipboard (may require re-auth)
    CopyPassword(usize),
    /// Open URL in browser
    OpenUrl(usize),

    // ========================================================================
    // Re-authentication Modal
    // ========================================================================
    /// Show re-auth modal for password copy
    ShowReauthModal(String),
    /// Re-auth password input changed
    ReauthPasswordChanged(String),
    /// Confirm re-authentication
    ConfirmReauth,
    /// Cancel re-authentication
    CancelReauth,

    // ========================================================================
    // Password Generator
    // ========================================================================
    /// Generate a new password
    GeneratePassword,
    /// Copy generated password to clipboard
    CopyGeneratedPassword,
    /// Use generated password in entry form
    UseGeneratedPassword,
    /// Generator length changed
    GeneratorLengthChanged(usize),
    /// Toggle lowercase letters
    ToggleLowercase(bool),
    /// Toggle uppercase letters
    ToggleUppercase(bool),
    /// Toggle digits
    ToggleDigits(bool),
    /// Toggle symbols
    ToggleSymbols(bool),
    /// Toggle exclude ambiguous characters
    ToggleExcludeAmbiguous(bool),

    // ========================================================================
    // Vault Management
    // ========================================================================
    /// Show vault selector dropdown
    ShowVaultSelector,
    /// Hide vault selector dropdown
    HideVaultSelector,
    /// Select a vault by name
    SelectVault(String),
    /// Show new vault modal
    ShowNewVaultModal,
    /// Hide new vault modal
    HideNewVaultModal,
    /// New vault name changed
    NewVaultNameChanged(String),
    /// Create a new vault
    CreateVault,

    // ========================================================================
    // Settings
    // ========================================================================
    /// Show settings modal
    ShowSettings,
    /// Hide settings modal
    HideSettings,
    /// Change theme
    ChangeTheme(LilypadTheme),
    /// Change auto-lock minutes
    ChangeAutoLock(u32),
    /// Change clipboard timeout
    ChangeClipboardTimeout(u32),
    /// Toggle security alerts
    ToggleSecurityAlerts(bool),
    /// Toggle require master password on copy
    ToggleRequireMasterOnCopy(bool),

    // ========================================================================
    // Account Settings (Demo)
    // ========================================================================
    /// Display name changed
    DisplayNameChanged(String),
    /// Email changed
    EmailChanged(String),
    /// Timezone changed
    TimezoneChanged(String),
    /// Toggle two-factor authentication
    ToggleTwoFactor(bool),
    /// Toggle marketing opt-in
    ToggleMarketingOptIn(bool),
    /// Recovery email changed
    RecoveryEmailChanged(String),
    /// Remove trusted device
    RemoveTrustedDevice(usize),

    // ========================================================================
    // Health Dashboard
    // ========================================================================
    /// Refresh health report
    RefreshHealthReport,

    // ========================================================================
    // System
    // ========================================================================
    /// Tick for timer-based updates (auto-lock, clipboard clear)
    Tick,
    /// Clear status message
    ClearStatus,
    /// Set a status message
    SetStatus(String),
    /// Open external link
    OpenExternalLink(String),

    // ========================================================================
    // File Operations
    // ========================================================================
    /// Import vault from file
    ImportVault,
    /// Export vault to file (encrypted .lily format)
    ExportVault,
    /// Export vault as plaintext JSON
    ExportVaultJson,
    /// File selected for import
    FileSelected(Option<std::path::PathBuf>),

    // ========================================================================
    // GitHub OAuth & Sync
    // ========================================================================
    /// Initiate GitHub OAuth login (Device Flow)
    GitHubLogin,
    /// Logout from GitHub
    GitHubLogout,
    /// OAuth login completed (result)
    GitHubLoginResult(std::result::Result<String, String>),
    /// Check sync status
    SyncCheckStatus,
    /// Push vault to GitHub
    SyncPush,
    /// Pull vault from GitHub
    SyncPull,
    /// Sync operation completed (result message)
    SyncCompleted(std::result::Result<String, String>),
    /// Device flow: show user code for manual entry
    DeviceFlowCode {
        user_code: String,
        verification_uri: String,
    },

    // ========================================================================
    // No-op
    // ========================================================================
    /// Do nothing (used for subscriptions)
    None,
}
