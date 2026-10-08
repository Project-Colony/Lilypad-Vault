//! Lilypad CLI - a secure password manager.
//!
//! A thin command-line frontend over `lilypad-app`: it parses arguments,
//! prompts for secrets, and prints results. All vault logic (crypto, storage,
//! validated sync, locking) lives in the service layer.

mod cmd;
mod io;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use lilypad_app::{App, OpenOptions};
use std::io as stdio;
use std::path::PathBuf;

/// Output format for commands that support structured output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "lilypad", version, about = "Secure password manager.", long_about = None)]
struct Cli {
    /// Data directory (default: platform data dir, or $LILYPAD_DATA_DIR)
    #[arg(long, value_name = "DIR", global = true)]
    data_dir: Option<String>,
    /// Master password (prefer interactive entry; this is visible in `ps` and
    /// shell history). Falls back to the LILYPAD_MASTER_PASSWORD env var.
    #[arg(long, value_name = "PASSWORD", global = true)]
    master_password: Option<String>,
    /// Output format for commands that support it.
    #[arg(long = "output-format", value_enum, default_value_t = OutputFormat::Text, global = true)]
    output_format: OutputFormat,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
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
enum SyncCommands {
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.master_password.is_some() {
        eprintln!("WARNING: --master-password is visible in `ps` and shell history; prefer interactive entry.");
    }

    let master_password = cli
        .master_password
        .clone()
        .or_else(|| std::env::var("LILYPAD_MASTER_PASSWORD").ok());
    let mp = master_password.as_deref();
    let fmt = cli.output_format;

    let app = App::open(OpenOptions {
        data_dir: cli.data_dir.clone().map(PathBuf::from),
        auto_lock_after: None,
    })?;

    // CLI builds before lilypad-app defaulted to `./.lilypad`; point at those
    // vaults instead of silently showing an empty store.
    if cli.data_dir.is_none()
        && std::env::var_os("LILYPAD_DATA_DIR").is_none()
        && app.list_vaults().is_ok_and(|v| v.is_empty())
        && std::fs::read_dir(".lilypad/vaults").is_ok_and(|mut d| {
            d.any(|e| e.is_ok_and(|e| e.path().extension().is_some_and(|x| x == "lily")))
        })
    {
        eprintln!("hint: ./.lilypad holds vaults from an older Lilypad CLI; pass --data-dir .lilypad (or set LILYPAD_DATA_DIR) to use them.");
    }

    match cli.command {
        Commands::Init { vault } => cmd::init(&app, &vault, mp),
        Commands::Vaults => cmd::vaults(&app, fmt),
        Commands::RenameVault { from, to } => cmd::rename_vault(&app, &from, &to),
        Commands::DeleteVault { vault, force } => cmd::delete_vault(&app, &vault, force),

        Commands::Add {
            vault,
            label,
            value,
            username,
            url,
            notes,
            tags,
            folder,
            entry_type,
            totp_secret,
            attachments,
        } => cmd::add(
            &app,
            &vault,
            &label,
            value,
            username,
            url,
            notes,
            tags,
            folder,
            entry_type,
            totp_secret,
            attachments,
            mp,
        ),
        Commands::List { vault } => cmd::list(&app, &vault, fmt, mp),
        Commands::Get {
            vault,
            label,
            copy,
            clipboard_timeout,
            show_password,
        } => cmd::get(
            &app,
            &vault,
            &label,
            copy,
            clipboard_timeout,
            show_password,
            mp,
        ),
        Commands::Update {
            vault,
            label,
            value,
            username,
            url,
            notes,
            tags,
            remove_tags,
            clear_tags,
            folder,
            entry_type,
            totp_secret,
        } => cmd::update(
            &app,
            &vault,
            &label,
            value,
            username,
            url,
            notes,
            tags,
            remove_tags,
            clear_tags,
            folder,
            entry_type,
            totp_secret,
            mp,
        ),
        Commands::Remove {
            vault,
            label,
            purge,
        } => cmd::remove(&app, &vault, &label, purge, mp),
        Commands::Trash { vault } => cmd::trash(&app, &vault, fmt, mp),
        Commands::Restore { vault, label } => cmd::restore(&app, &vault, &label, mp),
        Commands::Audit { vault } => cmd::audit(&app, &vault, fmt, mp),
        Commands::BreachCheck { vault } => cmd::breach_check(&app, &vault, fmt, mp),
        Commands::AuditLog { vault } => cmd::audit_log(&app, &vault, fmt, mp),
        Commands::History { vault, label } => cmd::history(&app, &vault, &label, fmt, mp),
        Commands::Import {
            vault,
            file,
            dry_run,
        } => cmd::import(&app, &vault, &file, dry_run, mp),
        Commands::Export { vault, file, force } => cmd::export(&app, &vault, &file, force, mp),
        Commands::RenameEntry {
            vault,
            label,
            new_label,
        } => cmd::rename_entry(&app, &vault, &label, &new_label, mp),
        Commands::Search { vault, query } => cmd::search(&app, &vault, &query, fmt, mp),
        Commands::Generate {
            length,
            uppercase,
            lowercase,
            digits,
            symbols,
            copy,
            clipboard_timeout,
        } => cmd::generate(
            length,
            uppercase,
            lowercase,
            digits,
            symbols,
            copy,
            clipboard_timeout,
        ),
        Commands::Totp { vault, label } => cmd::totp(&app, &vault, &label, mp),
        Commands::BackupCodes {
            vault,
            label,
            generate,
            verify,
        } => cmd::backup_codes(&app, &vault, &label, generate, verify.as_deref(), mp),
        Commands::ChangeMasterPassword { vault } => cmd::change_master_password(&app, &vault, mp),

        Commands::Backup { vault } => cmd::backup(&app, &vault),
        Commands::ListBackups { vault } => cmd::list_backups(&app, &vault),
        Commands::RestoreBackup { backup, force } => cmd::restore_backup(&app, &backup, force),
        Commands::PruneBackups { vault, keep } => cmd::prune_backups(&app, &vault, keep),

        Commands::Completions { shell } => {
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            generate(shell, &mut command, name, &mut stdio::stdout());
            Ok(())
        }

        Commands::Login => cmd::login(),
        Commands::Logout => cmd::logout(),
        Commands::AuthStatus => cmd::auth_status(),
        Commands::Sync(sync) => match sync {
            SyncCommands::Push { vault, force } => cmd::sync_push(&app, &vault, force),
            SyncCommands::Pull { vault } => cmd::sync_pull(&app, &vault, mp),
            SyncCommands::Merge { vault } => cmd::sync_merge(&app, &vault, mp),
            SyncCommands::Status { vault } => cmd::sync_status(&app, &vault),
        },
    }
}
