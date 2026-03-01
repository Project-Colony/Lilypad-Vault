//! Lilypad CLI - A secure password manager.
//!
//! This is the command-line interface for Lilypad, providing commands for
//! managing vaults, entries, imports/exports, security features, and backups.

mod commands;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use std::io;
use std::path::PathBuf;

use commands::{
    backup, entries, export, import, oauth, security, vault,
    utils::{non_empty_value, OutputFormat, SecureString},
};
use lilypad_core::default_config;
use lilypad_storage::LocalStore;

#[derive(Debug, Parser)]
#[command(
    name = "lilypad",
    about = "Minimal CLI for managing Lilypad vaults.",
    long_about = None
)]
struct Cli {
    /// Storage directory (default: .lilypad)
    #[arg(long, value_name = "DIR")]
    data_dir: Option<String>,
    /// Master password (or LILYPAD_MASTER_PASSWORD environment variable)
    /// WARNING: Passing passwords via command line or environment variables
    /// may expose them in shell history or process listings.
    #[arg(long, value_name = "PASSWORD", global = true)]
    master_password: Option<String>,
    /// Output format for display (text or json)
    #[arg(long = "output-format", value_enum, default_value_t = OutputFormat::Text, global = true)]
    output_format: OutputFormat,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Initialize a vault and generate a local key.
    Init {
        /// Vault name to create
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Use a master password instead of a random key
        #[arg(long)]
        use_master_password: bool,
    },
    /// List available vaults.
    Vaults,
    /// Rename a vault.
    RenameVault {
        /// Existing vault name
        #[arg(value_parser = non_empty_value)]
        from: String,
        /// New vault name
        #[arg(value_parser = non_empty_value)]
        to: String,
    },
    /// Delete a vault.
    DeleteVault {
        /// Vault name to delete
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Skip confirmation prompt
        #[arg(long, short)]
        force: bool,
    },
    /// Add an encrypted entry to a vault.
    Add {
        /// Vault name to update
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
        /// Value to encrypt
        #[arg(value_parser = non_empty_value)]
        value: String,
        /// Username metadata
        #[arg(long)]
        username: Option<String>,
        /// URL metadata
        #[arg(long)]
        url: Option<String>,
        /// Notes stored inside the encrypted payload
        #[arg(long)]
        notes: Option<String>,
        /// Tags (repeatable)
        #[arg(long = "tag", value_parser = non_empty_value)]
        tags: Vec<String>,
        /// Folder metadata
        #[arg(long)]
        folder: Option<String>,
        /// Entry type (login, card, identity, secure-note, software-license, wifi, server, custom)
        #[arg(long, value_parser = non_empty_value)]
        entry_type: Option<String>,
        /// TOTP secret (base32) stored inside the encrypted payload
        #[arg(long, value_parser = non_empty_value)]
        totp_secret: Option<String>,
        /// Attachment file paths (repeatable)
        #[arg(long, value_name = "FILE")]
        attachment: Vec<PathBuf>,
        /// Require strong password (fail if password is weak)
        #[arg(long)]
        require_strong: bool,
        /// Password expires in N days (0 = no expiry)
        #[arg(long, value_name = "DAYS", default_value_t = 0)]
        expires_in: u32,
    },
    /// List entries in a vault.
    List {
        /// Vault name to inspect
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
    /// Fetch an entry from a vault.
    Get {
        /// Vault name to read
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
        /// Copy the password to the clipboard
        #[arg(long)]
        copy: bool,
        /// Clipboard timeout in seconds
        #[arg(long, value_name = "SECONDS", default_value_t = 15)]
        clipboard_timeout: u64,
        /// Show the password in plain text (hidden by default)
        #[arg(long)]
        show_password: bool,
    },
    /// Update the value of an existing entry.
    Update {
        /// Vault name to update
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
        /// New value
        #[arg(value_parser = non_empty_value)]
        value: String,
        /// Username metadata
        #[arg(long)]
        username: Option<String>,
        /// URL metadata
        #[arg(long)]
        url: Option<String>,
        /// Notes stored inside the encrypted payload
        #[arg(long)]
        notes: Option<String>,
        /// Tags to add (repeatable)
        #[arg(long = "tag", value_parser = non_empty_value)]
        tags: Vec<String>,
        /// Tags to remove (repeatable)
        #[arg(long = "remove-tag", value_parser = non_empty_value)]
        remove_tags: Vec<String>,
        /// Remove all existing tags
        #[arg(long)]
        clear_tags: bool,
        /// Folder metadata
        #[arg(long)]
        folder: Option<String>,
        /// Entry type (login, card, identity, secure-note, software-license, wifi, server, custom)
        #[arg(long, value_parser = non_empty_value)]
        entry_type: Option<String>,
        /// TOTP secret (base32) stored inside the encrypted payload
        #[arg(long, value_parser = non_empty_value)]
        totp_secret: Option<String>,
        /// Attachment file paths (repeatable)
        #[arg(long, value_name = "FILE")]
        attachment: Vec<PathBuf>,
        /// Require strong password (fail if password is weak)
        #[arg(long)]
        require_strong: bool,
        /// Password expires in N days (0 = no expiry)
        #[arg(long, value_name = "DAYS", default_value_t = 0)]
        expires_in: u32,
    },
    /// Remove an entry from a vault.
    Remove {
        /// Vault name to update
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
    },
    /// Rename an entry's label.
    RenameEntry {
        /// Vault name
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Current entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
        /// New entry label
        #[arg(value_parser = non_empty_value)]
        new_label: String,
    },
    /// Search entries by label.
    Search {
        /// Keyword
        #[arg(value_parser = non_empty_value)]
        query: String,
        /// Vault name to inspect (omit with --all-vaults)
        vault: Option<String>,
        /// Search across all vaults
        #[arg(long)]
        all_vaults: bool,
    },
    /// Export a vault to a file.
    Export {
        /// Vault name to export
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Output file path
        #[arg(long, value_parser = non_empty_value)]
        output: String,
        /// Export format: lily, json, csv
        #[arg(long, value_parser = non_empty_value)]
        format: String,
        /// Allow plaintext exports (json/csv)
        #[arg(long)]
        allow_plaintext: bool,
    },
    /// Import entries into a vault.
    Import {
        /// Vault name to import into
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Input file path
        #[arg(long, value_parser = non_empty_value)]
        input: String,
        /// Import format: lily, json, csv
        #[arg(long, value_parser = non_empty_value)]
        format: String,
        /// Source password manager: lilypad, lastpass, bitwarden, 1password, chrome, firefox, dashlane, keepass
        #[arg(long, default_value = "lilypad")]
        source: String,
    },
    /// Rotate the vault encryption key.
    RotateKey {
        /// Vault name to rotate
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Use a master password for the new key
        #[arg(long)]
        use_master_password: bool,
        /// New master password (if using a master password)
        #[arg(long, value_name = "PASSWORD")]
        new_master_password: Option<String>,
        /// Skip creating a backup before rotation
        #[arg(long)]
        skip_backup: bool,
    },
    /// Generate a TOTP code for an entry.
    Totp {
        /// Vault name
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
    },
    /// Manage TOTP backup codes.
    BackupCodes {
        /// Vault name
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
        /// Generate new backup codes
        #[arg(long)]
        generate: bool,
        /// Verify a backup code
        #[arg(long, value_name = "CODE")]
        verify: Option<String>,
    },
    /// Audit the vault for security issues.
    Audit {
        /// Vault name to audit
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
    /// Generate a random password.
    Generate {
        /// Password length
        #[arg(long, default_value_t = 16)]
        length: usize,
        /// Include uppercase letters
        #[arg(long, default_value_t = true)]
        uppercase: bool,
        /// Include lowercase letters
        #[arg(long, default_value_t = true)]
        lowercase: bool,
        /// Include digits
        #[arg(long, default_value_t = true)]
        digits: bool,
        /// Include symbols
        #[arg(long, default_value_t = true)]
        symbols: bool,
        /// Copy to clipboard
        #[arg(long)]
        copy: bool,
        /// Clipboard timeout in seconds
        #[arg(long, value_name = "SECONDS", default_value_t = 15)]
        clipboard_timeout: u64,
    },
    /// Export audit log.
    AuditLog {
        /// Vault name
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Output file (stdout if not specified)
        #[arg(long)]
        output: Option<String>,
        /// Output format: json, csv, text
        #[arg(long, default_value = "text")]
        format: String,
        /// Filter by action
        #[arg(long)]
        action: Option<String>,
        /// Filter by entry label
        #[arg(long)]
        entry: Option<String>,
        /// Filter events after this date (YYYY-MM-DD or Unix timestamp)
        #[arg(long)]
        after: Option<String>,
        /// Filter events before this date (YYYY-MM-DD or Unix timestamp)
        #[arg(long)]
        before: Option<String>,
        /// Limit number of events
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Generate shell completions.
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Change the master password for a vault.
    ChangeMasterPassword {
        /// Vault name
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
    /// Check passwords against the HaveIBeenPwned database.
    BreachCheck {
        /// Vault name
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Check only a specific entry
        #[arg(long)]
        entry: Option<String>,
    },
    /// View entry change history.
    History {
        /// Vault name
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
        /// Maximum number of history records to show
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Create a backup of a vault.
    Backup {
        /// Vault name to backup
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
    /// List backups.
    ListBackups {
        /// Filter by vault name
        vault: Option<String>,
    },
    /// Restore a vault from backup.
    RestoreBackup {
        /// Backup filename to restore
        #[arg(value_parser = non_empty_value)]
        backup: String,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Delete old backups, keeping the most recent.
    PruneBackups {
        /// Vault name
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Number of backups to keep
        #[arg(long, default_value_t = 5)]
        keep: usize,
    },
    /// Verify vault integrity.
    VerifyVault {
        /// Vault name to verify
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },

    // ============== GitHub OAuth & Sync Commands ==============

    /// Log in to GitHub for vault synchronization.
    Login,

    /// Log out from GitHub.
    Logout,

    /// Show GitHub authentication status.
    AuthStatus,

    /// Sync commands for GitHub vault storage.
    #[command(subcommand)]
    Sync(SyncCommands),
}

/// Subcommands for vault synchronization.
#[derive(Debug, Subcommand)]
enum SyncCommands {
    /// Push a vault to GitHub.
    Push {
        /// Vault name to push
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Force push, overwriting remote changes (resolves conflicts)
        #[arg(long)]
        force: bool,
    },
    /// Pull a vault from GitHub.
    Pull {
        /// Vault name to pull
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Force pull, overwriting local changes (resolves conflicts)
        #[arg(long)]
        force: bool,
    },
    /// Delete a vault from GitHub (local copy is kept).
    Delete {
        /// Vault name to delete from remote
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
    /// Show sync status for a vault.
    Status {
        /// Vault name to check
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Security warnings for password exposure via CLI args or env vars
    if cli.master_password.is_some() {
        eprintln!("WARNING: Master password passed via --master-password flag.");
        eprintln!("         This is visible in process listings (ps) and shell history.");
        eprintln!("         Prefer interactive entry or LILYPAD_MASTER_PASSWORD with caution.");
    }
    if std::env::var("LILYPAD_MASTER_PASSWORD").is_ok() {
        eprintln!("WARNING: Using master password from LILYPAD_MASTER_PASSWORD environment variable.");
        eprintln!("         This may be visible in process listings and shell history.");
        eprintln!("         Consider unsetting it after use: unset LILYPAD_MASTER_PASSWORD");
    }

    // Wrap master password in SecureString for automatic zeroization on drop
    let master_password = cli
        .master_password
        .or_else(|| std::env::var("LILYPAD_MASTER_PASSWORD").ok())
        .map(SecureString::new);

    let output_format = cli.output_format;
    let mut config = default_config();
    if let Some(data_dir) = cli.data_dir {
        if data_dir.trim().is_empty() {
            return Err(anyhow::anyhow!("data_dir cannot be empty"));
        }
        config.data_dir = data_dir;
    }

    let store = LocalStore::new(&config)?;

    match cli.command {
        // Vault commands
        Commands::Init { vault, use_master_password } => {
            vault::init_vault(&store, &config, &vault, use_master_password, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Vaults => vault::list_vaults(&store, output_format),
        Commands::RenameVault { from, to } => vault::rename_vault(&store, &from, &to),
        Commands::DeleteVault { vault, force } => vault::delete_vault(&store, &vault, force),
        Commands::VerifyVault { vault } => vault::verify_vault(&store, &vault),

        // Entry commands
        Commands::Add { vault, label, value, username, url, notes, tags, folder, entry_type, totp_secret, attachment, require_strong, expires_in } => {
            entries::add_entry(&store, &config, &vault, &label, &value, username, url, notes, tags, folder, entry_type, totp_secret, attachment, require_strong, expires_in, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::List { vault } => {
            entries::list_entries(&store, &config, &vault, output_format, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Get { vault, label, copy, clipboard_timeout, show_password } => {
            entries::get_entry(&store, &config, &vault, &label, copy, clipboard_timeout, show_password, output_format, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Update { vault, label, value, username, url, notes, tags, remove_tags, clear_tags, folder, entry_type, totp_secret, attachment, require_strong, expires_in } => {
            entries::update_entry(&store, &config, &vault, &label, &value, username, url, notes, tags, remove_tags, clear_tags, folder, entry_type, totp_secret, attachment, require_strong, expires_in, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Remove { vault, label } => {
            entries::remove_entry(&store, &config, &vault, &label, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::RenameEntry { vault, label, new_label } => {
            entries::rename_entry(&store, &config, &vault, &label, &new_label, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Search { vault, query, all_vaults } => {
            if all_vaults {
                entries::search_all_vaults(&store, &config, &query, output_format, master_password.as_ref().map(|s| s.as_str()))
            } else {
                let vault_name = vault.ok_or_else(|| anyhow::anyhow!("vault name is required (or use --all-vaults)"))?;
                entries::search_entries(&store, &config, &vault_name, &query, output_format, master_password.as_ref().map(|s| s.as_str()))
            }
        }
        Commands::Generate { length, uppercase, lowercase, digits, symbols, copy, clipboard_timeout } => {
            entries::generate_password(length, uppercase, lowercase, digits, symbols, copy, clipboard_timeout)
        }

        // Import/Export commands
        Commands::Export { vault, output, format, allow_plaintext } => {
            export::export_vault(&store, &config, &vault, &output, &format, allow_plaintext, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Import { vault, input, format, source } => {
            import::import_vault(&store, &config, &vault, &input, &format, &source, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::AuditLog { vault, output, format, action, entry, after, before, limit } => {
            export::export_audit_log(&store, &config, &vault, output.as_deref(), &format, action.as_deref(), entry.as_deref(), after.as_deref(), before.as_deref(), limit, master_password.as_ref().map(|s| s.as_str()))
        }

        // Security commands
        Commands::RotateKey { vault, use_master_password, new_master_password, skip_backup } => {
            security::rotate_key(&store, &config, &vault, use_master_password, new_master_password.as_deref(), skip_backup, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::ChangeMasterPassword { vault } => {
            security::change_master_password(&store, &config, &vault, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::BreachCheck { vault, entry } => {
            security::breach_check(&store, &config, &vault, entry.as_deref(), master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Totp { vault, label } => {
            security::show_totp(&store, &config, &vault, &label, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::BackupCodes { vault, label, generate, verify } => {
            security::backup_codes(&store, &config, &vault, &label, generate, verify.as_deref(), master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Audit { vault } => {
            security::audit_vault(&store, &config, &vault, output_format, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::History { vault, label, limit } => {
            security::show_entry_history(&store, &config, &vault, &label, limit, master_password.as_ref().map(|s| s.as_str()))
        }

        // Backup commands
        Commands::Backup { vault } => backup::create_backup(&store, &vault),
        Commands::ListBackups { vault } => backup::list_backups(&store, vault.as_deref()),
        Commands::RestoreBackup { backup, force } => backup::restore_backup(&store, &backup, force),
        Commands::PruneBackups { vault, keep } => backup::prune_backups(&store, &vault, keep),

        // Shell completions
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            generate(shell, &mut cmd, name, &mut io::stdout());
            Ok(())
        }

        // OAuth and Sync commands
        Commands::Login => oauth::login(output_format),
        Commands::Logout => oauth::logout(output_format),
        Commands::AuthStatus => oauth::status(output_format),
        Commands::Sync(sync_cmd) => match sync_cmd {
            SyncCommands::Push { vault, force } => {
                oauth::sync_push(&store, &config, &vault, force, master_password.as_ref().map(|s| s.as_str()), output_format)
            }
            SyncCommands::Pull { vault, force } => {
                oauth::sync_pull(&store, &config, &vault, force, master_password.as_ref().map(|s| s.as_str()), output_format)
            }
            SyncCommands::Delete { vault } => {
                oauth::sync_delete(&vault, output_format)
            }
            SyncCommands::Status { vault } => {
                oauth::sync_status(&store, &config, &vault, master_password.as_ref().map(|s| s.as_str()), output_format)
            }
        },
    }
}
