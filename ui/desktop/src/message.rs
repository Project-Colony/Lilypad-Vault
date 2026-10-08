//! Messages for the Lilypad desktop application.

use crate::theme::{LilypadTheme, UiVariation};
use lilypad_app::sync::net::{DeviceLogin, PullOutcome};
use lilypad_app::{EntryColor, EntryType, KeyMaterial, SyncStatusView};

/// Which field of the add/edit form changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Label,
    Username,
    Url,
    Password,
    Notes,
    Totp,
    /// Comma-separated tag list.
    Tags,
    Folder,
}

/// A generator character set toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharSet {
    Upper,
    Lower,
    Digits,
    Symbols,
}

/// What a quick-copy action on a list row copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickKind {
    Username,
    Password,
    Totp,
}

/// The active sidebar filter over the entry list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    All,
    Favorites,
    Type(EntryType),
    Folder(String),
    Tag(String),
    Trash,
}

#[derive(Debug, Clone)]
pub enum Message {
    // Appearance
    ThemeSelected(LilypadTheme),
    DensitySelected(UiVariation),

    // Settings page (categorized preferences)
    OpenSettings,
    CloseSettings,
    SettingsCategory(usize),
    SettingsToggleSection(String),
    SetAutoLock(u64),
    SetClipboardSecs(u64),
    SetDefaultFilter(u8),
    ToggleRestoreLastVault,
    CmNewChanged(String),
    CmConfirmChanged(String),
    ChangeMasterSubmit,

    // Import / export (Vault settings category)
    ImportPathChanged(String),
    ImportRun,
    ExportPathChanged(String),
    ExportRun,

    // HIBP breach check (explicit network operation)
    BreachCheckRun,
    /// Ok carries (unique passwords checked, breached (label, count) pairs).
    BreachChecked(Result<(usize, Vec<(String, u64)>), String>),

    // GitHub sync (device-flow login, push/pull, status)
    SyncLoginStart,
    SyncLoginStarted(Result<DeviceLogin, String>),
    SyncLoginCancel,
    SyncLoginDone(Result<String, String>),
    SyncLogout,
    SyncPush,
    /// Ok carries the vault the push was for, so a stale completion is not
    /// applied to a different active vault.
    SyncPushDone(Result<String, String>),
    SyncPullPasswordChanged(String),
    SyncPull,
    SyncPullDone(Result<PullOutcome, String>),
    SyncMerge,
    /// Ok carries (vault, outcome): None = no remote exists, Some((summary,
    /// pushed)) = merged, with `pushed` true when the merged vault was pushed
    /// back. Only applied if that vault is still active.
    SyncMergeDone(Result<(String, Option<(String, bool)>), String>),
    SyncRefreshStatus,
    /// Ok carries (vault, status); only applied if that vault is still active.
    SyncStatusLoaded(Result<(String, SyncStatusView), String>),

    // Unlock screen
    VaultSelected(String),
    PasswordChanged(String),
    UnlockPressed,
    /// The (CPU-heavy) key derivation finished off-thread.
    KeyDerived(Result<KeyMaterial, String>),

    // Create-vault screen
    ShowCreate,
    ShowUnlock,
    NewNameChanged(String),
    NewPasswordChanged(String),
    CreatePressed,

    // Sidebar / navigation
    FilterSelected(Filter),
    SearchChanged(String),

    // Entry selection / detail
    SelectEntry(String),
    ToggleReveal,
    RevealHold(bool),
    CopyPassword,
    CopyUsername,
    CopyTotp,
    OpenUrl(String),
    ToggleFavorite(String),
    SetColor(String, Option<EntryColor>),
    Lock,

    // List-row hover quick actions
    RowHovered(Option<String>),
    QuickCopy(String, QuickKind),
    ClipboardCancel,

    // Add / edit
    ShowAddForm,
    StartEdit,
    FormChanged(FormField, String),
    /// Entry-type picker (label from `type_label`).
    FormTypeSelected(String),
    /// Expiry picker choice (label from the EXPIRY_CHOICES list).
    FormExpirySelected(String),
    FormSave,
    FormCancel,

    // Delete / trash
    DeleteRequested(String),
    DeleteConfirmed,
    DeleteCancelled,
    RestoreEntry(String),

    // Generator
    ShowGenerator,
    GenLength(f32),
    GenToggle(CharSet),
    GenRegenerate,
    GenUse,
    CloseGenerator,

    // Timers / misc
    Tick,
    Dismiss,
    Noop,
}
