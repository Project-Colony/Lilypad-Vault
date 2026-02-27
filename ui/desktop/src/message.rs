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
    /// Confirm password input changed (for vault creation)
    ConfirmPasswordChanged(String),
    /// Attempt to unlock the vault
    UnlockVault,
    /// Create a new vault with the entered password
    CreateVaultWithPassword,
    /// Lock the vault (manual or auto-lock)
    LockVault,
    /// Acknowledge the welcome screen
    AcknowledgeWelcome,

    // ========================================================================
    // Onboarding
    // ========================================================================
    /// Move to the next onboarding step
    OnboardingNext,
    /// Move to the previous onboarding step
    OnboardingPrev,
    /// Skip onboarding entirely
    OnboardingSkip,
    /// Jump to a specific onboarding step (dot navigation)
    OnboardingGoTo(usize),
    /// Tutorial: generate a demo password
    OnboardingTutorialGenerate,
    /// Tutorial: copy the demo password to clipboard
    OnboardingTutorialCopy,
    /// Tutorial: advance to next tutorial phase
    OnboardingTutorialNext,
    /// Onboarding: create GitHub repo after successful auth
    OnboardingEnsureRepo,
    /// Onboarding: result of repo creation
    OnboardingRepoResult(std::result::Result<String, String>),

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
    /// Entry email changed
    EntryEmailChanged(String),
    /// Entry phone changed
    EntryPhoneChanged(String),
    /// Entry folder changed
    EntryFolderChanged(String),
    /// New tag input changed
    EntryNewTagChanged(String),
    /// Add a tag to the entry being edited
    AddEntryTag,
    /// Remove a tag from the entry being edited
    RemoveEntryTag(usize),
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
    /// View entry history
    ViewEntryHistory(usize),
    /// Close entry history view
    CloseEntryHistory,
    /// Entry TOTP secret changed
    EntryTotpSecretChanged(String),
    /// Add a custom field
    AddCustomField,
    /// Remove a custom field by index
    RemoveCustomField(usize),
    /// Custom field name changed
    CustomFieldNameChanged(usize, String),
    /// Custom field value changed
    CustomFieldValueChanged(usize, String),
    /// Add attachment to entry
    AddAttachment,
    /// Remove attachment by index
    RemoveAttachment(usize),
    /// Attachment file selected from dialog
    AttachmentFileSelected(Option<std::path::PathBuf>),
    /// Filter by folder
    FilterByFolder(Option<String>),
    /// Copy TOTP code to clipboard
    CopyTotpCode(usize),
    /// Resolve sync conflict: keep local version
    SyncResolveKeepLocal,
    /// Resolve sync conflict: keep remote version
    SyncResolveKeepRemote,
    /// Change entry type
    EntryTypeChanged(String),
    /// Toggle advanced fields section in entry form
    ToggleAdvancedFields,
    /// Show/hide audit log
    ShowAuditLog,
    /// Close audit log
    CloseAuditLog,

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
    /// Change master password (key rotation)
    ChangeMasterPassword,
    /// New master password input
    NewMasterPasswordChanged(String),
    /// Confirm master password change
    ConfirmChangeMasterPassword,
    /// Cancel master password change
    CancelChangeMasterPassword,

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
    /// Check passwords against HIBP breach database
    CheckBreaches,
    /// Breach check completed
    BreachCheckCompleted(Vec<String>),

    // ========================================================================
    // System
    // ========================================================================
    /// Tick for timer-based updates (auto-lock, clipboard clear)
    Tick,
    /// Clear status message
    ClearStatus,
    /// Set a status message
    SetStatus(String),
    /// Copy arbitrary text to clipboard
    CopyToClipboard(String),
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
    /// Export vault as CSV
    ExportVaultCsv,
    /// Import from browser CSV (Chrome, Firefox, Bitwarden, LastPass, 1Password, KeePass)
    ImportBrowserCsv,
    /// Browser CSV file selected
    BrowserCsvSelected(Option<std::path::PathBuf>),
    /// File selected for import
    FileSelected(Option<std::path::PathBuf>),
    /// Create vault backup
    BackupVault,
    /// Restore vault from backup
    RestoreVault,
    /// Backup file selected for restore
    BackupFileSelected(Option<std::path::PathBuf>),

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
    /// Device flow: show user code for manual entry and start polling
    DeviceFlowCode {
        user_code: String,
        verification_uri: String,
        device_code: String,
        interval: u64,
    },

    // ========================================================================
    // No-op
    // ========================================================================
    /// Do nothing (used for subscriptions)
    None,
}
