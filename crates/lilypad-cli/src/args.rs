//! Command-line arguments: every flag and subcommand of `lilypad-cli`.

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

/// Output format for commands that support structured output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "lilypad", version, about = "Secure password manager.", long_about = None)]
pub struct Cli {
    /// Data directory (default: platform data dir, or $LILYPAD_DATA_DIR)
    #[arg(long, value_name = "DIR", global = true)]
    pub data_dir: Option<String>,
    /// Master password (prefer interactive entry; this is visible in `ps` and
    /// shell history). Falls back to the LILYPAD_MASTER_PASSWORD env var.
    #[arg(long, value_name = "PASSWORD", global = true)]
    pub master_password: Option<String>,
    /// Output format for commands that support it.
    #[arg(long = "output-format", value_enum, default_value_t = OutputFormat::Text, global = true)]
    pub output_format: OutputFormat,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Create a new vault (prompts for a master password).
    Init { vault: String },
    /// List available vaults.
    Vaults,
    /// Rename a vault.
    RenameVault { from: String, to: String },
    /// Delete a vault (keeps a safety backup).
    DeleteVault {
        vault: String,
        #[arg(long, short)]
        force: bool,
    },
    /// Add an entry to a vault.
    Add {
        vault: String,
        label: String,
        /// Secret value (omit to enter it interactively or pipe it via stdin).
        value: Option<String>,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long)]
        folder: Option<String>,
        /// login, card, identity, secure-note, software-license, wifi, server, custom
        #[arg(long)]
        entry_type: Option<String>,
        #[arg(long)]
        totp_secret: Option<String>,
        #[arg(long = "attachment", value_name = "FILE")]
        attachments: Vec<PathBuf>,
    },
    /// List entries in a vault.
    List { vault: String },
    /// Show an entry.
    Get {
        vault: String,
        label: String,
        #[arg(long)]
        copy: bool,
        #[arg(long, value_name = "SECONDS", default_value_t = 15)]
        clipboard_timeout: u64,
        #[arg(long)]
        show_password: bool,
    },
    /// Update an entry (only the fields you pass change).
    Update {
        vault: String,
        label: String,
        /// New secret value (omit to keep the current one).
        value: Option<String>,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long = "remove-tag")]
        remove_tags: Vec<String>,
        #[arg(long)]
        clear_tags: bool,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        entry_type: Option<String>,
        #[arg(long)]
        totp_secret: Option<String>,
    },
    /// Remove an entry (moves it to Trash unless --purge).
    Remove {
        vault: String,
        label: String,
        /// Permanently erase instead of moving to Trash (not recoverable).
        #[arg(long)]
        purge: bool,
    },
    /// List a vault's Trash (soft-deleted entries).
    Trash { vault: String },
    /// Restore a soft-deleted entry from Trash.
    Restore { vault: String, label: String },
    /// Report a vault's password health (Watchtower).
    Audit { vault: String },
    /// Check passwords against Have-I-Been-Pwned (k-anonymity: only the first
    /// 5 characters of each SHA-1 hash are sent; passwords never leave this
    /// machine). Explicit network operation.
    BreachCheck { vault: String },
    /// Show a vault's audit log (record of mutations).
    AuditLog { vault: String },
    /// Show an entry's change history (kinds and dates; secrets stay sealed).
    History { vault: String, label: String },
    /// Import entries from another password manager (format auto-detected:
    /// Lilypad, LastPass, Bitwarden CSV/JSON, KeePassXC, 1Password, Safari,
    /// Chrome/Edge, Firefox, Proton Pass, Dashlane).
    Import {
        vault: String,
        /// Path to the exported file.
        file: PathBuf,
        /// Parse and report what would be imported without writing anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Export a vault as PLAINTEXT CSV (Lilypad format; every secret revealed).
    Export {
        vault: String,
        /// Destination path for the CSV file.
        file: PathBuf,
        /// Skip the plaintext warning prompt.
        #[arg(long, short)]
        force: bool,
    },
    /// Rename an entry.
    RenameEntry {
        vault: String,
        label: String,
        new_label: String,
    },
    /// Search entries by their (unencrypted) metadata.
    Search { vault: String, query: String },
    /// Generate a random password.
    Generate {
        #[arg(long, default_value_t = 20)]
        length: usize,
        #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
        uppercase: bool,
        #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
        lowercase: bool,
        #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
        digits: bool,
        #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
        symbols: bool,
        #[arg(long)]
        copy: bool,
        #[arg(long, value_name = "SECONDS", default_value_t = 15)]
        clipboard_timeout: u64,
    },
    /// Show a TOTP code for an entry.
    Totp { vault: String, label: String },
    /// Show how many TOTP backup codes an entry has left, generate a new set,
    /// or check a code and mark it used.
    BackupCodes {
        vault: String,
        label: String,
        /// Replace the codes with a fresh set and print them once.
        #[arg(long, conflicts_with = "verify")]
        generate: bool,
        /// Check a backup code and mark it used.
        #[arg(long, value_name = "CODE")]
        verify: Option<String>,
    },
    /// Change a vault's master password.
    ChangeMasterPassword { vault: String },
    /// Create a backup of a vault.
    Backup { vault: String },
    /// List a vault's backups.
    ListBackups { vault: String },
    /// Restore a vault from a backup file.
    RestoreBackup {
        backup: String,
        #[arg(long)]
        force: bool,
    },
    /// Delete old backups, keeping the most recent N.
    PruneBackups {
        vault: String,
        #[arg(long, default_value_t = 5)]
        keep: usize,
    },
    /// Generate shell completions.
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Log in to GitHub for sync.
    Login,
    /// Log out of GitHub.
    Logout,
    /// Show GitHub authentication status.
    AuthStatus,
    /// Vault synchronization with GitHub.
    #[command(subcommand)]
    Sync(SyncCommands),
}

#[derive(Debug, Subcommand)]
pub enum SyncCommands {
    /// Push a vault to GitHub.
    Push {
        vault: String,
        #[arg(long)]
        force: bool,
    },
    /// Pull a vault from GitHub (validated; a safety backup is taken).
    Pull { vault: String },
    /// Bidirectional sync: merge the remote vault entry-by-entry (deletions
    /// propagate, newest version of each entry wins), pushing back if needed.
    Merge { vault: String },
    /// Show sync status for a vault.
    Status { vault: String },
}
