use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use sha1::{Digest, Sha1};
use csv::{ReaderBuilder, WriterBuilder};
use lilypad_common::{
    clipboard::copy_to_clipboard_with_timeout,
    keyfile::{load_key, save_key, KeyFile},
    time::format_timestamp_relative,
    validation::{validate_password_strength, PasswordStrength},
};
use lilypad_core::{
    decrypt, default_config, derive_key, encrypt, Attachment, Entry, EntryChangeType,
    EntryMetadata, EntrySecret, EntryType, KeyDerivationParams, KeyMaterial, KeyMetadata, Vault,
    MAX_NOTES_SIZE, MAX_PASSWORD_SIZE,
};
use lilypad_storage::LocalStore;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs::{self, Permissions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tempfile::NamedTempFile;
use totp_rs::{Algorithm, Secret, TOTP};
use zeroize::Zeroize;

/// A String wrapper that zeroizes its contents on drop.
#[derive(Clone)]
struct SecureString(String);

impl SecureString {
    fn new(s: String) -> Self {
        Self(s)
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl Drop for SecureString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Writes data to a file atomically using a temporary file and rename.
/// This ensures the file is never in a partially-written state.
fn atomic_write(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    let parent = path.parent().unwrap_or(Path::new("."));

    // Create temp file in the same directory to ensure same filesystem for rename
    let mut temp_file = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temp file in {}", parent.display()))?;

    temp_file
        .write_all(data.as_ref())
        .with_context(|| "failed to write to temp file")?;

    temp_file
        .flush()
        .with_context(|| "failed to flush temp file")?;

    // Persist the temp file by renaming it to the target path
    temp_file
        .persist(path)
        .with_context(|| format!("failed to persist file to {}", path.display()))?;

    Ok(())
}

/// Output format for CLI commands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
enum OutputFormat {
    #[default]
    Text,
    Json,
}

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
    /// Remove an entry from a vault.
    Remove {
        /// Vault name to update
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
    },
    /// Search entries by label.
    Search {
        /// Vault name to inspect
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Keyword
        #[arg(value_parser = non_empty_value)]
        query: String,
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
        /// Skip automatic backup before rotation
        #[arg(long)]
        skip_backup: bool,
    },
    /// Show the current TOTP code for an entry.
    Totp {
        /// Vault name to inspect
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
    },
    /// Generate or view TOTP backup codes for an entry.
    BackupCodes {
        /// Vault name
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
        /// Generate new backup codes (replaces existing ones)
        #[arg(long)]
        generate: bool,
        /// Verify a backup code (marks it as used if valid)
        #[arg(long)]
        verify: Option<String>,
    },
    /// Run a basic password health audit.
    Audit {
        /// Vault name to inspect
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
    /// Generate a random password.
    Generate {
        /// Password length
        #[arg(long, short, default_value_t = 20)]
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
        /// Include special characters
        #[arg(long, default_value_t = true)]
        symbols: bool,
        /// Copy to clipboard
        #[arg(long)]
        copy: bool,
        /// Clipboard timeout in seconds
        #[arg(long, value_name = "SECONDS", default_value_t = 15)]
        clipboard_timeout: u64,
    },
    /// Export audit logs from a vault.
    AuditLog {
        /// Vault name to inspect
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Output file path (prints to stdout if not specified)
        #[arg(long)]
        output: Option<String>,
        /// Output format: json, csv, text
        #[arg(long, default_value = "json")]
        format: String,
        /// Filter by action type (e.g., "entry_added", "entry_removed", "vault_accessed")
        #[arg(long)]
        action: Option<String>,
        /// Filter by entry label
        #[arg(long)]
        entry: Option<String>,
        /// Show only events after this date (YYYY-MM-DD or Unix timestamp)
        #[arg(long)]
        after: Option<String>,
        /// Show only events before this date (YYYY-MM-DD or Unix timestamp)
        #[arg(long)]
        before: Option<String>,
        /// Maximum number of events to show
        #[arg(long, short)]
        limit: Option<usize>,
    },
    /// Generate shell completions for bash, zsh, fish, or powershell.
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Change the master password without full key rotation.
    ChangeMasterPassword {
        /// Vault name to update
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
    /// Check if passwords have been exposed in known data breaches (HaveIBeenPwned).
    BreachCheck {
        /// Vault name to check
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Check only a specific entry
        #[arg(long)]
        entry: Option<String>,
    },
    /// View the change history of an entry.
    History {
        /// Vault name to inspect
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Entry label
        #[arg(value_parser = non_empty_value)]
        label: String,
        /// Number of history records to show (default: 10)
        #[arg(long, short, default_value_t = 10)]
        limit: usize,
    },
    /// Create a backup of a vault.
    Backup {
        /// Vault name to backup
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
    /// List all backups for a vault.
    ListBackups {
        /// Vault name (optional, lists all backups if not specified)
        vault: Option<String>,
    },
    /// Restore a vault from a backup.
    RestoreBackup {
        /// Backup filename to restore
        #[arg(value_parser = non_empty_value)]
        backup: String,
        /// Force restore without confirmation
        #[arg(long, short)]
        force: bool,
    },
    /// Delete old backups, keeping only the most recent N.
    PruneBackups {
        /// Vault name to prune backups for
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Number of backups to keep (default: 5)
        #[arg(long, short, default_value_t = 5)]
        keep: usize,
    },
    /// Verify vault integrity without decrypting.
    VerifyVault {
        /// Vault name to verify
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultExport {
    name: String,
    entries: Vec<EntryExport>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EntryExport {
    label: String,
    metadata: EntryMetadata,
    secret: EntrySecret,
    created_at: u64,
    updated_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct CsvEntry {
    label: String,
    entry_type: String,
    username: String,
    url: String,
    folder: String,
    tags: String,
    password: String,
    notes: String,
    totp_secret: String,
}

// LastPass CSV format: url,username,password,totp,extra,name,grouping,fav
#[derive(Debug, Deserialize)]
struct LastPassEntry {
    url: String,
    username: String,
    password: String,
    totp: String,
    extra: String,
    name: String,
    grouping: String,
    #[serde(default)]
    fav: String,
}

// Bitwarden CSV format: folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp
#[derive(Debug, Deserialize)]
struct BitwardenEntry {
    folder: String,
    #[serde(default)]
    favorite: String,
    #[serde(rename = "type")]
    entry_type: String,
    name: String,
    notes: String,
    #[serde(default)]
    fields: String,
    #[serde(default)]
    reprompt: String,
    #[serde(default)]
    login_uri: String,
    #[serde(default)]
    login_username: String,
    #[serde(default)]
    login_password: String,
    #[serde(default)]
    login_totp: String,
}

// 1Password CSV format: Title,Url,Username,Password,Notes,OTPAuth
#[derive(Debug, Deserialize)]
struct OnePasswordEntry {
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Url", default)]
    url: String,
    #[serde(rename = "Username", default)]
    username: String,
    #[serde(rename = "Password", default)]
    password: String,
    #[serde(rename = "Notes", default)]
    notes: String,
    #[serde(rename = "OTPAuth", default)]
    otp_auth: String,
}

// Chrome CSV format: name,url,username,password,note
#[derive(Debug, Deserialize)]
struct ChromeEntry {
    name: String,
    url: String,
    username: String,
    password: String,
    #[serde(default)]
    note: String,
}

// Firefox CSV format: url,username,password,httpRealm,formActionOrigin,guid,timeCreated,timeLastUsed,timePasswordChanged
#[derive(Debug, Deserialize)]
struct FirefoxEntry {
    url: String,
    username: String,
    password: String,
    #[serde(rename = "httpRealm", default)]
    http_realm: String,
    #[serde(rename = "formActionOrigin", default)]
    form_action_origin: String,
    #[serde(default)]
    guid: String,
    #[serde(rename = "timeCreated", default)]
    time_created: String,
    #[serde(rename = "timeLastUsed", default)]
    time_last_used: String,
    #[serde(rename = "timePasswordChanged", default)]
    time_password_changed: String,
}

// Dashlane CSV format: username,username2,username3,title,password,note,url,category,otpSecret
#[derive(Debug, Deserialize)]
struct DashlaneEntry {
    username: String,
    #[serde(default)]
    username2: String,
    #[serde(default)]
    username3: String,
    title: String,
    password: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    category: String,
    #[serde(rename = "otpSecret", default)]
    otp_secret: String,
}

// KeePass CSV format: Group,Title,Username,Password,URL,Notes,TOTP
#[derive(Debug, Deserialize)]
struct KeePassEntry {
    #[serde(rename = "Group", default)]
    group: String,
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Username", default)]
    username: String,
    #[serde(rename = "Password", default)]
    password: String,
    #[serde(rename = "URL", default)]
    url: String,
    #[serde(rename = "Notes", default)]
    notes: String,
    #[serde(rename = "TOTP", default)]
    totp: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // Wrap master password in SecureString for automatic zeroization on drop
    let master_password = cli
        .master_password
        .or_else(|| std::env::var("LILYPAD_MASTER_PASSWORD").ok())
        .map(SecureString::new);
    let output_format = cli.output_format;
    let mut config = default_config();
    if let Some(data_dir) = cli.data_dir {
        if data_dir.trim().is_empty() {
            return Err(anyhow!("data_dir cannot be empty"));
        }
        config.data_dir = data_dir;
    }

    let store = LocalStore::new(&config)?;
    match cli.command {
        Commands::Init {
            vault,
            use_master_password,
        } => init_vault(
            &store,
            &config,
            &vault,
            use_master_password,
            master_password.as_ref().map(|s| s.as_str()),
        ),
        Commands::Vaults => list_vaults(&store, output_format),
        Commands::RenameVault { from, to } => rename_vault(&store, &from, &to),
        Commands::DeleteVault { vault, force } => delete_vault(&store, &vault, force),
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
            attachment,
            require_strong,
            expires_in,
        } => add_entry(
            &store,
            &config,
            &vault,
            &label,
            &value,
            username,
            url,
            notes,
            tags,
            folder,
            entry_type,
            totp_secret,
            attachment,
            require_strong,
            expires_in,
            master_password.as_ref().map(|s| s.as_str()),
        ),
        Commands::List { vault } => {
            list_entries(&store, &config, &vault, output_format, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Get {
            vault,
            label,
            copy,
            clipboard_timeout,
            show_password,
        } => get_entry(
            &store,
            &config,
            &vault,
            &label,
            copy,
            clipboard_timeout,
            show_password,
            output_format,
            master_password.as_ref().map(|s| s.as_str()),
        ),
        Commands::Update {
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
            attachment,
            require_strong,
            expires_in,
        } => update_entry(
            &store,
            &config,
            &vault,
            &label,
            &value,
            username,
            url,
            notes,
            tags,
            folder,
            entry_type,
            totp_secret,
            attachment,
            require_strong,
            expires_in,
            master_password.as_ref().map(|s| s.as_str()),
        ),
        Commands::Remove { vault, label } => {
            remove_entry(&store, &config, &vault, &label, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Search { vault, query } => {
            search_entries(&store, &config, &vault, &query, output_format, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Export {
            vault,
            output,
            format,
            allow_plaintext,
        } => export_vault(
            &store,
            &config,
            &vault,
            &output,
            &format,
            allow_plaintext,
            master_password.as_ref().map(|s| s.as_str()),
        ),
        Commands::Import {
            vault,
            input,
            format,
            source,
        } => import_vault(
            &store,
            &config,
            &vault,
            &input,
            &format,
            &source,
            master_password.as_ref().map(|s| s.as_str()),
        ),
        Commands::RotateKey {
            vault,
            use_master_password,
            new_master_password,
            skip_backup,
        } => rotate_key(
            &store,
            &config,
            &vault,
            use_master_password,
            new_master_password.as_ref().map(|s| s.as_str()),
            skip_backup,
            master_password.as_ref().map(|s| s.as_str()),
        ),
        Commands::Totp { vault, label } => {
            show_totp(&store, &config, &vault, &label, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::BackupCodes { vault, label, generate, verify } => {
            backup_codes(&store, &config, &vault, &label, generate, verify.as_deref(), master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Audit { vault } => {
            audit_vault(&store, &config, &vault, output_format, master_password.as_ref().map(|s| s.as_str()))
        }
        Commands::Generate {
            length,
            uppercase,
            lowercase,
            digits,
            symbols,
            copy,
            clipboard_timeout,
        } => generate_password(length, uppercase, lowercase, digits, symbols, copy, clipboard_timeout),
        Commands::AuditLog {
            vault,
            output,
            format,
            action,
            entry,
            after,
            before,
            limit,
        } => export_audit_log(
            &store,
            &config,
            &vault,
            output.as_deref(),
            &format,
            action.as_deref(),
            entry.as_deref(),
            after.as_deref(),
            before.as_deref(),
            limit,
            master_password.as_ref().map(|s| s.as_str()),
        ),
        Commands::Completions { shell } => {
            generate_completions(shell);
            Ok(())
        }
        Commands::ChangeMasterPassword { vault } => change_master_password(
            &store,
            &config,
            &vault,
            master_password.as_ref().map(|s| s.as_str()),
        ),
        Commands::BreachCheck { vault, entry } => breach_check(
            &store,
            &config,
            &vault,
            entry.as_deref(),
            master_password.as_ref().map(|s| s.as_str()),
        ),
        Commands::History { vault, label, limit } => show_entry_history(
            &store,
            &config,
            &vault,
            &label,
            limit,
            master_password.as_ref().map(|s| s.as_str()),
        ),
        Commands::Backup { vault } => create_backup(&store, &vault),
        Commands::ListBackups { vault } => list_backups(&store, vault.as_deref()),
        Commands::RestoreBackup { backup, force } => restore_backup(&store, &backup, force),
        Commands::PruneBackups { vault, keep } => prune_backups(&store, &vault, keep),
        Commands::VerifyVault { vault } => verify_vault(&store, &vault),
    }
}

fn init_vault(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault: &str,
    use_master_password: bool,
    master_password: Option<&str>,
) -> Result<()> {
    let key_path = key_path(config);
    if key_path.exists() {
        return Err(anyhow!("key already initialized at {}", key_path.display()));
    }

    let (key, key_file) = if use_master_password {
        let password = master_password.ok_or_else(|| {
            anyhow!("master password required (--master-password or LILYPAD_MASTER_PASSWORD)")
        })?;
        // Warn if password is weak
        let strength = validate_password_strength(password);
        if !strength.is_acceptable() {
            eprintln!("Warning: {}", strength.feedback());
        }
        let params = KeyDerivationParams::generate();
        let key = derive_key(password, &params)?;
        (key, KeyFile::from_kdf(params))
    } else {
        let key = KeyMaterial::generate();
        let key_file = KeyFile::from_raw(&key);
        (key, key_file)
    };
    save_key(&key_path, &key_file)?;

    let metadata = if use_master_password {
        KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305)
            .with_kdf("argon2id")
    } else {
        KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305)
    };
    let vault = Vault::new(vault, metadata);
    store.save_vault(&vault, &key)?;

    println!("Vault '{}' initialized in {}.", vault.name, config.data_dir);
    Ok(())
}

fn list_vaults(store: &LocalStore, output_format: OutputFormat) -> Result<()> {
    let vaults = store.list_vaults()?;

    match output_format {
        OutputFormat::Json => {
            let json = serde_json::json!({ "vaults": vaults });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            if vaults.is_empty() {
                println!("No vaults found.");
                return Ok(());
            }
            println!("Vaults:");
            for name in vaults {
                println!("- {name}");
            }
        }
    }
    Ok(())
}

fn rename_vault(store: &LocalStore, from: &str, to: &str) -> Result<()> {
    store.rename_vault(from, to)?;
    println!("Vault '{from}' renamed to '{to}'.");
    Ok(())
}

fn delete_vault(store: &LocalStore, vault: &str, force: bool) -> Result<()> {
    if !force {
        print!("Are you sure you want to delete vault '{vault}'? This cannot be undone. [y/N] ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("Deletion cancelled.");
            return Ok(());
        }
    }
    store.delete_vault(vault)?;
    println!("Vault '{vault}' deleted.");
    Ok(())
}

fn add_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    value: &str,
    username: Option<String>,
    url: Option<String>,
    notes: Option<String>,
    tags: Vec<String>,
    folder: Option<String>,
    entry_type: Option<String>,
    totp_secret: Option<String>,
    attachment: Vec<PathBuf>,
    require_strong: bool,
    expires_in: u32,
    master_password: Option<&str>,
) -> Result<()> {
    // Check password strength
    let strength = validate_password_strength(value);
    if require_strong && !strength.is_acceptable() {
        return Err(anyhow!(
            "password is too weak: {}. Use a stronger password or remove --require-strong.",
            strength.feedback()
        ));
    } else if !strength.is_acceptable() {
        eprintln!("Warning: {}", strength.feedback());
    }

    let (key, _) = load_key(&key_path(config), master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let metadata = build_metadata(username, url, tags, folder, entry_type)?;
    // Validate metadata
    metadata.validate()?;

    let mut secret = EntrySecret::new(value);
    secret.notes = notes;
    secret.totp_secret = totp_secret;
    secret.attachments = load_attachments(attachment)?;
    // Validate secret sizes
    secret.validate()?;

    let ciphertext = encrypt_entry_secret(&key, &secret)?;
    let mut entry = Entry::new_with_metadata(label, metadata, ciphertext);
    if expires_in > 0 {
        entry.set_password_expiry_days(expires_in);
    }
    vault.add_entry(entry)?;
    store.save_vault(&vault, &key)?;
    println!("Entry '{label}' added to vault '{vault_name}'.");
    if expires_in > 0 {
        println!("Password will expire in {expires_in} days.");
    }
    Ok(())
}

fn list_entries(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    output_format: OutputFormat,
    master_password: Option<&str>,
) -> Result<()> {
    let (key, _) = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    match output_format {
        OutputFormat::Json => {
            let entries: Vec<_> = vault.entries.iter().map(|e| {
                serde_json::json!({
                    "label": e.label,
                    "type": entry_type_label(&e.metadata.entry_type),
                    "username": e.metadata.username,
                    "url": e.metadata.url,
                    "folder": e.metadata.folder,
                    "tags": e.metadata.tags,
                    "created_at": e.created_at,
                    "updated_at": e.updated_at,
                })
            }).collect();
            let json = serde_json::json!({
                "vault": vault_name,
                "entries": entries,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            if vault.entries.is_empty() {
                println!("No entries in '{vault_name}'.");
                return Ok(());
            }
            println!("Entries in '{vault_name}':");
            for entry in &vault.entries {
                let tags = if entry.metadata.tags.is_empty() {
                    "none".to_string()
                } else {
                    entry.metadata.tags.join(", ")
                };
                let entry_type = entry_type_label(&entry.metadata.entry_type);
                let folder = entry.metadata.folder.as_deref().unwrap_or("none");
                let updated = format_timestamp_relative(entry.updated_at);
                println!(
                    "- {} (type: {}, folder: {}, tags: {}, updated: {})",
                    entry.label, entry_type, folder, tags, updated
                );
            }
        }
    }
    Ok(())
}

fn get_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    copy: bool,
    clipboard_timeout: u64,
    show_password: bool,
    output_format: OutputFormat,
    master_password: Option<&str>,
) -> Result<()> {
    let (key, _) = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let entry = vault
        .find_entry(label)
        .ok_or_else(|| anyhow!("entry '{label}' not found"))?;
    let secret = decrypt_entry_secret(&key, entry)?;

    match output_format {
        OutputFormat::Json => {
            let json = serde_json::json!({
                "label": entry.label,
                "type": entry_type_label(&entry.metadata.entry_type),
                "username": entry.metadata.username,
                "url": entry.metadata.url,
                "folder": entry.metadata.folder,
                "tags": entry.metadata.tags,
                "password": if show_password { Some(&secret.password) } else { None },
                "notes": secret.notes,
                "totp_secret": if show_password { secret.totp_secret.as_ref() } else { None },
                "attachments_count": secret.attachments.len(),
                "created_at": entry.created_at,
                "updated_at": entry.updated_at,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            println!("Label: {}", entry.label);
            println!("Type: {}", entry_type_label(&entry.metadata.entry_type));
            if let Some(username) = &entry.metadata.username {
                println!("Username: {username}");
            }
            if let Some(url) = &entry.metadata.url {
                println!("URL: {url}");
            }
            if let Some(folder) = &entry.metadata.folder {
                println!("Folder: {folder}");
            }
            if !entry.metadata.tags.is_empty() {
                println!("Tags: {}", entry.metadata.tags.join(", "));
            }
            // Mask password by default for security (visible in terminal history/logs)
            if show_password {
                println!("Password: {}", secret.password);
            } else {
                println!("Password: ******** (use --show-password to reveal)");
            }
            if let Some(notes) = &secret.notes {
                println!("Notes: {notes}");
            }
            if let Some(totp_secret) = &secret.totp_secret {
                // Also mask TOTP secret by default
                if show_password {
                    println!("TOTP secret: {totp_secret}");
                } else {
                    println!("TOTP secret: ******** (use --show-password to reveal)");
                }
            }
            if !secret.attachments.is_empty() {
                println!("Attachments: {}", secret.attachments.len());
            }
            println!("Updated: {}", format_timestamp_relative(entry.updated_at));
        }
    }

    if copy {
        copy_to_clipboard_with_timeout(&secret.password, clipboard_timeout)?;
        if output_format == OutputFormat::Text {
            println!("Password copied to clipboard for {clipboard_timeout} seconds.");
        }
    }
    Ok(())
}

fn update_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    value: &str,
    username: Option<String>,
    url: Option<String>,
    notes: Option<String>,
    tags: Vec<String>,
    folder: Option<String>,
    entry_type: Option<String>,
    totp_secret: Option<String>,
    attachment: Vec<PathBuf>,
    require_strong: bool,
    expires_in: u32,
    master_password: Option<&str>,
) -> Result<()> {
    // Check password strength
    let strength = validate_password_strength(value);
    if require_strong && !strength.is_acceptable() {
        return Err(anyhow!(
            "password is too weak: {}. Use a stronger password or remove --require-strong.",
            strength.feedback()
        ));
    } else if !strength.is_acceptable() {
        eprintln!("Warning: {}", strength.feedback());
    }

    let (key, _) = load_key(&key_path(config), master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let (mut metadata, mut secret, old_password) = {
        let existing = vault
            .find_entry(label)
            .ok_or_else(|| anyhow!("entry '{label}' not found"))?;
        let decrypted = decrypt_entry_secret(&key, existing)?;
        let old_pw = decrypted.password.clone();
        (existing.metadata.clone(), decrypted, old_pw)
    };
    apply_metadata_updates(&mut metadata, username, url, tags, folder, entry_type)?;
    // Validate metadata
    metadata.validate()?;

    let password_changed = value != old_password;
    secret.password = value.to_string();
    if notes.is_some() {
        secret.notes = notes;
    }
    if totp_secret.is_some() {
        secret.totp_secret = totp_secret;
    }
    let new_attachments = load_attachments(attachment)?;
    if !new_attachments.is_empty() {
        secret.attachments = new_attachments;
    }
    // Validate secret sizes
    secret.validate()?;

    let ciphertext = encrypt_entry_secret(&key, &secret)?;
    vault
        .update_entry(label, ciphertext)
        .with_context(|| format!("entry '{label}' not found"))?;
    if let Some(entry) = vault.entries.iter_mut().find(|entry| entry.label == label) {
        entry.metadata = metadata;
        // Update password change tracking
        if password_changed {
            entry.record_password_change();
        }
        // Update expiry if specified
        if expires_in > 0 {
            entry.set_password_expiry_days(expires_in);
        }
    }
    store.save_vault(&vault, &key)?;
    println!("Entry '{label}' updated.");
    if password_changed && expires_in > 0 {
        println!("Password will expire in {expires_in} days.");
    }
    Ok(())
}

fn remove_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    master_password: Option<&str>,
) -> Result<()> {
    let (key, _) = load_key(&key_path(config), master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    vault
        .remove_entry(label)
        .with_context(|| format!("entry '{label}' not found"))?;
    store.save_vault(&vault, &key)?;
    println!("Entry '{label}' removed.");
    Ok(())
}

fn search_entries(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    query: &str,
    output_format: OutputFormat,
    master_password: Option<&str>,
) -> Result<()> {
    let (key, _) = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let matches = vault.search_entries(query);

    match output_format {
        OutputFormat::Json => {
            let results: Vec<_> = matches.iter().map(|e| {
                serde_json::json!({
                    "label": e.label,
                    "type": entry_type_label(&e.metadata.entry_type),
                    "username": e.metadata.username,
                    "url": e.metadata.url,
                    "folder": e.metadata.folder,
                    "tags": e.metadata.tags,
                    "updated_at": e.updated_at,
                })
            }).collect();
            let json = serde_json::json!({
                "query": query,
                "results": results,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            if matches.is_empty() {
                println!("No entries match '{query}'.");
                return Ok(());
            }
            println!("Results for '{query}':");
            for entry in matches {
                let updated = format_timestamp_relative(entry.updated_at);
                println!("- {} (updated: {})", entry.label, updated);
            }
        }
    }
    Ok(())
}

fn export_vault(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    output: &str,
    format: &str,
    allow_plaintext: bool,
    master_password: Option<&str>,
) -> Result<()> {
    let format = format.to_lowercase();
    if format == "lily" {
        let payload = store.sync_payload(vault_name)?;
        atomic_write(output, payload)?;
        println!("Encrypted vault exported to {output}.");
        return Ok(());
    }

    if !allow_plaintext {
        return Err(anyhow!(
            "plaintext export requires --allow-plaintext (format: {format})"
        ));
    }

    // Security warning for plaintext export
    eprintln!("⚠️  WARNING: You are about to export credentials in PLAINTEXT format.");
    eprintln!("   All passwords, TOTP secrets, and notes will be visible in the output file.");
    eprintln!("   Ensure you store or transmit this file securely, then delete it when done.");
    eprintln!();

    let (key, _) = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    match format.as_str() {
        "json" => {
            let entries = vault
                .entries
                .iter()
                .map(|entry| {
                    let secret = decrypt_entry_secret(&key, entry)?;
                    Ok(EntryExport {
                        label: entry.label.clone(),
                        metadata: entry.metadata.clone(),
                        secret,
                        created_at: entry.created_at,
                        updated_at: entry.updated_at,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let export = VaultExport {
                name: vault.name.clone(),
                entries,
            };
            let payload = serde_json::to_vec_pretty(&export)?;
            atomic_write(output, payload)?;
            println!("Vault exported to {output}.");
        }
        "csv" => {
            let mut writer = WriterBuilder::new().from_path(output)?;
            for entry in &vault.entries {
                let secret = decrypt_entry_secret(&key, entry)?;
                let record = CsvEntry {
                    label: entry.label.clone(),
                    entry_type: entry_type_label(&entry.metadata.entry_type).to_string(),
                    username: entry.metadata.username.clone().unwrap_or_default(),
                    url: entry.metadata.url.clone().unwrap_or_default(),
                    folder: entry.metadata.folder.clone().unwrap_or_default(),
                    tags: entry.metadata.tags.join(";"),
                    password: secret.password,
                    notes: secret.notes.unwrap_or_default(),
                    totp_secret: secret.totp_secret.unwrap_or_default(),
                };
                writer.serialize(record)?;
            }
            writer.flush()?;
            println!("Vault exported to {output}.");
        }
        _ => {
            return Err(anyhow!("unsupported export format: {format}"));
        }
    }
    Ok(())
}

fn import_vault(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    input: &str,
    format: &str,
    source: &str,
    master_password: Option<&str>,
) -> Result<()> {
    let format = format.to_lowercase();
    let source = source.to_lowercase();

    if format == "lily" {
        let payload = fs::read(input)?;
        store.apply_sync_payload(vault_name, &payload)?;
        println!("Encrypted vault imported from {input}.");
        return Ok(());
    }

    let (key, _) = load_key(&key_path(config), master_password)?;
    let mut vault = match store.load_vault(vault_name, &key) {
        Ok(vault) => vault,
        Err(_) => {
            let metadata = KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305);
            Vault::new(vault_name, metadata)
        }
    };

    let mut imported_count = 0;

    match format.as_str() {
        "json" => {
            let payload = fs::read(input)?;
            let import: VaultExport = serde_json::from_slice(&payload)?;
            for entry in import.entries {
                validate_import_entry(&entry.label, &entry.secret, &entry.metadata)?;
                upsert_entry(&mut vault, &key, &entry.label, entry.metadata, entry.secret)?;
                imported_count += 1;
            }
        }
        "csv" => {
            match source.as_str() {
                "lilypad" => {
                    imported_count = import_lilypad_csv(&mut vault, &key, input)?;
                }
                "lastpass" => {
                    imported_count = import_lastpass_csv(&mut vault, &key, input)?;
                }
                "bitwarden" => {
                    imported_count = import_bitwarden_csv(&mut vault, &key, input)?;
                }
                "1password" | "onepassword" => {
                    imported_count = import_1password_csv(&mut vault, &key, input)?;
                }
                "chrome" | "google" => {
                    imported_count = import_chrome_csv(&mut vault, &key, input)?;
                }
                "firefox" => {
                    imported_count = import_firefox_csv(&mut vault, &key, input)?;
                }
                "dashlane" => {
                    imported_count = import_dashlane_csv(&mut vault, &key, input)?;
                }
                "keepass" => {
                    imported_count = import_keepass_csv(&mut vault, &key, input)?;
                }
                _ => {
                    return Err(anyhow!(
                        "Unsupported source: {source}. Supported sources: lilypad, lastpass, bitwarden, 1password, chrome, firefox, dashlane, keepass"
                    ));
                }
            }
        }
        _ => {
            return Err(anyhow!("unsupported import format: {format}"));
        }
    }

    store.save_vault(&vault, &key)?;
    println!("✓ Imported {imported_count} entries from {source} into vault '{vault_name}'.");
    Ok(())
}

/// Import from Lilypad's native CSV format
fn import_lilypad_csv(vault: &mut Vault, key: &KeyMaterial, input: &str) -> Result<usize> {
    let mut reader = ReaderBuilder::new().from_path(input)?;
    let mut count = 0;
    for result in reader.deserialize() {
        let record: CsvEntry = result?;
        let metadata = build_metadata(
            if record.username.is_empty() { None } else { Some(record.username) },
            if record.url.is_empty() { None } else { Some(record.url) },
            parse_tags(&record.tags),
            if record.folder.is_empty() { None } else { Some(record.folder) },
            if record.entry_type.is_empty() { None } else { Some(record.entry_type) },
        )?;
        let mut secret = EntrySecret::new(record.password);
        if !record.notes.is_empty() {
            secret.notes = Some(record.notes);
        }
        if !record.totp_secret.is_empty() {
            secret.totp_secret = Some(record.totp_secret);
        }
        validate_import_entry(&record.label, &secret, &metadata)?;
        upsert_entry(vault, key, &record.label, metadata, secret)?;
        count += 1;
    }
    Ok(count)
}

/// Import from LastPass CSV export
/// Format: url,username,password,totp,extra,name,grouping,fav
fn import_lastpass_csv(vault: &mut Vault, key: &KeyMaterial, input: &str) -> Result<usize> {
    let mut reader = ReaderBuilder::new().from_path(input)?;
    let mut count = 0;
    for result in reader.deserialize() {
        let record: LastPassEntry = result?;

        // Use 'name' as the label, fallback to URL if empty
        let label = if record.name.is_empty() {
            extract_domain_from_url(&record.url).unwrap_or_else(|| "Unnamed Entry".to_string())
        } else {
            record.name
        };

        let metadata = build_metadata(
            if record.username.is_empty() { None } else { Some(record.username) },
            if record.url.is_empty() { None } else { Some(record.url) },
            Vec::new(), // LastPass uses grouping, not tags
            if record.grouping.is_empty() { None } else { Some(record.grouping) },
            None, // No explicit type in LastPass export
        )?;

        let mut secret = EntrySecret::new(record.password);
        if !record.extra.is_empty() {
            secret.notes = Some(record.extra);
        }
        if !record.totp.is_empty() {
            // LastPass stores TOTP as otpauth:// URL, extract the secret
            secret.totp_secret = Some(extract_totp_secret(&record.totp));
        }

        validate_import_entry(&label, &secret, &metadata)?;
        upsert_entry(vault, key, &label, metadata, secret)?;
        count += 1;
    }
    Ok(count)
}

/// Import from Bitwarden CSV export
/// Format: folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp
fn import_bitwarden_csv(vault: &mut Vault, key: &KeyMaterial, input: &str) -> Result<usize> {
    let mut reader = ReaderBuilder::new().from_path(input)?;
    let mut count = 0;
    for result in reader.deserialize() {
        let record: BitwardenEntry = result?;

        // Skip non-login entries (cards, identities, secure notes without passwords)
        if record.entry_type != "login" && record.login_password.is_empty() {
            continue;
        }

        let metadata = build_metadata(
            if record.login_username.is_empty() { None } else { Some(record.login_username) },
            if record.login_uri.is_empty() { None } else { Some(record.login_uri) },
            Vec::new(),
            if record.folder.is_empty() { None } else { Some(record.folder) },
            Some(record.entry_type),
        )?;

        let mut secret = EntrySecret::new(record.login_password);
        if !record.notes.is_empty() {
            secret.notes = Some(record.notes);
        }
        if !record.login_totp.is_empty() {
            secret.totp_secret = Some(extract_totp_secret(&record.login_totp));
        }

        validate_import_entry(&record.name, &secret, &metadata)?;
        upsert_entry(vault, key, &record.name, metadata, secret)?;
        count += 1;
    }
    Ok(count)
}

/// Import from 1Password CSV export
/// Format: Title,Url,Username,Password,Notes,OTPAuth
fn import_1password_csv(vault: &mut Vault, key: &KeyMaterial, input: &str) -> Result<usize> {
    let mut reader = ReaderBuilder::new().from_path(input)?;
    let mut count = 0;
    for result in reader.deserialize() {
        let record: OnePasswordEntry = result?;

        let metadata = build_metadata(
            if record.username.is_empty() { None } else { Some(record.username) },
            if record.url.is_empty() { None } else { Some(record.url) },
            Vec::new(),
            None,
            None,
        )?;

        let mut secret = EntrySecret::new(record.password);
        if !record.notes.is_empty() {
            secret.notes = Some(record.notes);
        }
        if !record.otp_auth.is_empty() {
            secret.totp_secret = Some(extract_totp_secret(&record.otp_auth));
        }

        validate_import_entry(&record.title, &secret, &metadata)?;
        upsert_entry(vault, key, &record.title, metadata, secret)?;
        count += 1;
    }
    Ok(count)
}

/// Import from Chrome/Google Password Manager CSV export
/// Format: name,url,username,password,note
fn import_chrome_csv(vault: &mut Vault, key: &KeyMaterial, input: &str) -> Result<usize> {
    let mut reader = ReaderBuilder::new().from_path(input)?;
    let mut count = 0;
    for result in reader.deserialize() {
        let record: ChromeEntry = result?;

        // Use 'name' as label, fallback to domain from URL
        let label = if record.name.is_empty() {
            extract_domain_from_url(&record.url).unwrap_or_else(|| "Unnamed Entry".to_string())
        } else {
            record.name
        };

        let metadata = build_metadata(
            if record.username.is_empty() { None } else { Some(record.username) },
            if record.url.is_empty() { None } else { Some(record.url) },
            Vec::new(),
            None,
            None,
        )?;

        let mut secret = EntrySecret::new(record.password);
        if !record.note.is_empty() {
            secret.notes = Some(record.note);
        }

        validate_import_entry(&label, &secret, &metadata)?;
        upsert_entry(vault, key, &label, metadata, secret)?;
        count += 1;
    }
    Ok(count)
}

/// Import from Firefox CSV export
/// Format: url,username,password,httpRealm,formActionOrigin,guid,timeCreated,timeLastUsed,timePasswordChanged
fn import_firefox_csv(vault: &mut Vault, key: &KeyMaterial, input: &str) -> Result<usize> {
    let mut reader = ReaderBuilder::new().from_path(input)?;
    let mut count = 0;
    for result in reader.deserialize() {
        let record: FirefoxEntry = result?;

        // Use domain from URL as label
        let label = extract_domain_from_url(&record.url).unwrap_or_else(|| "Unnamed Entry".to_string());

        let metadata = build_metadata(
            if record.username.is_empty() { None } else { Some(record.username) },
            if record.url.is_empty() { None } else { Some(record.url) },
            Vec::new(),
            None,
            None,
        )?;

        let secret = EntrySecret::new(record.password);

        validate_import_entry(&label, &secret, &metadata)?;
        upsert_entry(vault, key, &label, metadata, secret)?;
        count += 1;
    }
    Ok(count)
}

/// Import from Dashlane CSV export
/// Format: username,username2,username3,title,password,note,url,category,otpSecret
fn import_dashlane_csv(vault: &mut Vault, key: &KeyMaterial, input: &str) -> Result<usize> {
    let mut reader = ReaderBuilder::new().from_path(input)?;
    let mut count = 0;
    for result in reader.deserialize() {
        let record: DashlaneEntry = result?;

        let metadata = build_metadata(
            if record.username.is_empty() { None } else { Some(record.username) },
            if record.url.is_empty() { None } else { Some(record.url) },
            Vec::new(),
            if record.category.is_empty() { None } else { Some(record.category) },
            None,
        )?;

        let mut secret = EntrySecret::new(record.password);
        if !record.note.is_empty() {
            secret.notes = Some(record.note);
        }
        if !record.otp_secret.is_empty() {
            secret.totp_secret = Some(record.otp_secret);
        }

        validate_import_entry(&record.title, &secret, &metadata)?;
        upsert_entry(vault, key, &record.title, metadata, secret)?;
        count += 1;
    }
    Ok(count)
}

/// Import from KeePass CSV export
/// Format: Group,Title,Username,Password,URL,Notes,TOTP
fn import_keepass_csv(vault: &mut Vault, key: &KeyMaterial, input: &str) -> Result<usize> {
    let mut reader = ReaderBuilder::new().from_path(input)?;
    let mut count = 0;
    for result in reader.deserialize() {
        let record: KeePassEntry = result?;

        let metadata = build_metadata(
            if record.username.is_empty() { None } else { Some(record.username) },
            if record.url.is_empty() { None } else { Some(record.url) },
            Vec::new(),
            if record.group.is_empty() { None } else { Some(record.group) },
            None,
        )?;

        let mut secret = EntrySecret::new(record.password);
        if !record.notes.is_empty() {
            secret.notes = Some(record.notes);
        }
        if !record.totp.is_empty() {
            secret.totp_secret = Some(extract_totp_secret(&record.totp));
        }

        validate_import_entry(&record.title, &secret, &metadata)?;
        upsert_entry(vault, key, &record.title, metadata, secret)?;
        count += 1;
    }
    Ok(count)
}

/// Extract domain from URL for use as label
fn extract_domain_from_url(url: &str) -> Option<String> {
    if url.is_empty() {
        return None;
    }
    // Try to extract domain from URL
    let url = url.trim();
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);

    let domain = without_scheme.split('/').next()?;
    let domain = domain.split(':').next()?; // Remove port if present

    if domain.is_empty() {
        None
    } else {
        Some(domain.to_string())
    }
}

/// Extract TOTP secret from various formats (otpauth:// URL or raw secret)
fn extract_totp_secret(totp_value: &str) -> String {
    let totp_value = totp_value.trim();

    // If it's an otpauth:// URL, extract the secret parameter
    if totp_value.starts_with("otpauth://") {
        if let Some(secret_start) = totp_value.find("secret=") {
            let secret_part = &totp_value[secret_start + 7..];
            let secret_end = secret_part.find('&').unwrap_or(secret_part.len());
            return secret_part[..secret_end].to_string();
        }
    }

    // Otherwise, return as-is (it's already a raw secret)
    totp_value.to_string()
}

fn rotate_key(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    use_master_password: bool,
    new_master_password: Option<&str>,
    skip_backup: bool,
    master_password: Option<&str>,
) -> Result<()> {
    let (old_key, old_key_file) = load_key(&key_path(config), master_password)?;
    let mut vault = store
        .load_vault(vault_name, &old_key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    // Create backup before rotation (unless --skip-backup is specified)
    if !skip_backup {
        let backup_dir = PathBuf::from(&config.data_dir).join("backups");
        fs::create_dir_all(&backup_dir)?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Backup vault file
        let vault_backup_path = backup_dir.join(format!("{vault_name}.{timestamp}.lily.bak"));
        let vault_payload = store.sync_payload(vault_name)?;
        fs::write(&vault_backup_path, &vault_payload)?;
        #[cfg(unix)]
        fs::set_permissions(&vault_backup_path, Permissions::from_mode(0o600))?;

        // Backup key file
        let key_backup_path = backup_dir.join(format!("key.{timestamp}.json.bak"));
        save_key(&key_backup_path, &old_key_file)?;
        #[cfg(unix)]
        fs::set_permissions(&key_backup_path, Permissions::from_mode(0o600))?;

        eprintln!(
            "⚠️  Backup created (contains encrypted vault and key file):\n  - {}\n  - {}",
            vault_backup_path.display(),
            key_backup_path.display()
        );
        eprintln!("   Store these backups securely or delete them after rotation is verified.");
    }

    let (new_key, key_file, metadata) = if use_master_password {
        let password = new_master_password
            .ok_or_else(|| anyhow!("new master password required (--new-master-password)"))?;
        // Warn if new password is weak
        let strength = validate_password_strength(password);
        if !strength.is_acceptable() {
            eprintln!("Warning: {}", strength.feedback());
        }
        let params = KeyDerivationParams::generate();
        let key = derive_key(password, &params)?;
        let metadata = KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305)
            .with_kdf("argon2id");
        (key, KeyFile::from_kdf(params), metadata)
    } else {
        let key = KeyMaterial::generate();
        let metadata = KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305);
        let key_file = KeyFile::from_raw(&key);
        (key, key_file, metadata)
    };

    for entry in &mut vault.entries {
        let secret = decrypt_entry_secret(&old_key, entry)?;
        entry.ciphertext = encrypt_entry_secret(&new_key, &secret)?;
    }
    vault.key_metadata = metadata;
    store.save_vault(&vault, &new_key)?;
    save_key(&key_path(config), &key_file)?;
    println!("Vault '{vault_name}' re-encrypted with a new key.");
    Ok(())
}

fn show_totp(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    master_password: Option<&str>,
) -> Result<()> {
    let (key, _) = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let entry = vault
        .find_entry(label)
        .ok_or_else(|| anyhow!("entry '{label}' not found"))?;
    let secret = decrypt_entry_secret(&key, entry)?;
    let totp_secret = secret
        .totp_secret
        .ok_or_else(|| anyhow!("entry '{label}' does not have a TOTP secret"))?;
    let secret = Secret::Encoded(totp_secret);
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().map_err(|err| anyhow!("{err}"))?,
    )?;
    let code = totp.generate_current()?;
    println!("{code}");
    Ok(())
}

fn backup_codes(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    generate: bool,
    verify: Option<&str>,
    master_password: Option<&str>,
) -> Result<()> {
    let (key, _) = load_key(&key_path(config), master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    let entry = vault
        .find_entry(label)
        .ok_or_else(|| anyhow!("entry '{label}' not found"))?;

    let mut secret = decrypt_entry_secret(&key, entry)?;

    // Check that TOTP is enabled
    if secret.totp_secret.is_none() {
        return Err(anyhow!("entry '{label}' does not have TOTP enabled. Backup codes require TOTP."));
    }

    if generate {
        // Generate new backup codes
        eprintln!("⚠️  WARNING: This will replace any existing backup codes!");
        eprintln!("   Store these codes in a safe place. They can only be shown once.");
        eprintln!();

        let codes = secret.generate_backup_codes();

        println!("═══════════════════════════════════════════");
        println!("   TOTP Backup Codes for '{label}'");
        println!("═══════════════════════════════════════════");
        println!();
        for (i, code) in codes.iter().enumerate() {
            // Format as XXXX-XXXX for readability
            let formatted = format!("{}-{}", &code[0..4], &code[4..8]);
            println!("   {:2}. {}", i + 1, formatted);
        }
        println!();
        println!("═══════════════════════════════════════════");
        println!("   Each code can only be used ONCE.");
        println!("   Store these codes securely offline.");
        println!("═══════════════════════════════════════════");

        // Save the updated entry
        let payload = serde_json::to_vec(&secret)?;
        let ciphertext = encrypt(&key, &payload)?;
        vault.update_entry(label, ciphertext)?;
        store.save_vault(&vault, &key)?;

        println!();
        println!("✓ {} backup codes generated and saved.", codes.len());
    } else if let Some(code) = verify {
        // Verify a backup code
        if secret.use_backup_code(code) {
            println!("✓ Backup code verified and consumed.");
            println!("  Remaining unused codes: {}", secret.unused_backup_codes_count());

            // Save the updated entry (code marked as used)
            let payload = serde_json::to_vec(&secret)?;
            let ciphertext = encrypt(&key, &payload)?;
            vault.update_entry(label, ciphertext)?;
            store.save_vault(&vault, &key)?;

            if secret.backup_codes_low() {
                eprintln!();
                eprintln!("⚠️  WARNING: Only {} backup codes remaining. Consider generating new codes.",
                         secret.unused_backup_codes_count());
            }
        } else {
            return Err(anyhow!("Invalid or already used backup code."));
        }
    } else {
        // Show status of backup codes
        let total = secret.totp_backup_codes.len();
        let unused = secret.unused_backup_codes_count();
        let used = total - unused;

        if total == 0 {
            println!("No backup codes generated for '{label}'.");
            println!("Use --generate to create backup codes.");
        } else {
            println!("Backup codes for '{label}':");
            println!("  Total:  {}", total);
            println!("  Used:   {}", used);
            println!("  Unused: {}", unused);

            if secret.backup_codes_low() {
                eprintln!();
                eprintln!("⚠️  WARNING: Backup codes running low! Consider generating new codes.");
            }
        }
    }

    Ok(())
}

fn audit_vault(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    output_format: OutputFormat,
    master_password: Option<&str>,
) -> Result<()> {
    let (key, _) = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    let mut weak = Vec::new();
    let mut very_weak = Vec::new();
    let mut expired = Vec::new();
    let mut expiring_soon = Vec::new(); // Expires within 7 days
    let mut old_passwords = Vec::new(); // Older than 90 days without expiry set
    let mut duplicates: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for entry in &vault.entries {
        let secret = decrypt_entry_secret(&key, entry)?;
        let strength = validate_password_strength(&secret.password);
        match strength {
            PasswordStrength::VeryWeak => very_weak.push(entry.label.clone()),
            PasswordStrength::Weak => weak.push(entry.label.clone()),
            _ => {}
        }
        duplicates
            .entry(secret.password)
            .or_default()
            .push(entry.label.clone());

        // Check password expiry
        if entry.is_password_expired() {
            expired.push(entry.label.clone());
        } else if let Some(days) = entry.days_until_password_expires() {
            if days <= 7 && days >= 0 {
                expiring_soon.push((entry.label.clone(), days));
            }
        }

        // Check for old passwords without expiry set
        if entry.password_expires_at == 0 && entry.password_age_days() > 90 {
            old_passwords.push((entry.label.clone(), entry.password_age_days()));
        }
    }

    let reused: Vec<Vec<String>> = duplicates
        .into_iter()
        .filter(|(_, labels)| labels.len() > 1)
        .map(|(_, labels)| labels)
        .collect();

    match output_format {
        OutputFormat::Json => {
            let json = serde_json::json!({
                "vault": vault_name,
                "total_entries": vault.entries.len(),
                "very_weak_passwords": very_weak,
                "weak_passwords": weak,
                "expired_passwords": expired,
                "expiring_soon": expiring_soon.iter().map(|(label, days)| {
                    serde_json::json!({ "label": label, "days_remaining": days })
                }).collect::<Vec<_>>(),
                "old_passwords": old_passwords.iter().map(|(label, days)| {
                    serde_json::json!({ "label": label, "age_days": days })
                }).collect::<Vec<_>>(),
                "reused_passwords": reused,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            println!("Audit results for '{vault_name}':");
            println!("Total entries: {}", vault.entries.len());
            println!();

            if very_weak.is_empty() && weak.is_empty() {
                println!("Password strength: All passwords meet minimum requirements.");
            } else {
                if !very_weak.is_empty() {
                    println!(
                        "Very weak passwords (<8 chars): {}",
                        very_weak.join(", ")
                    );
                }
                if !weak.is_empty() {
                    println!(
                        "Weak passwords (8-11 chars, missing diversity): {}",
                        weak.join(", ")
                    );
                }
            }

            // Password expiry status
            if !expired.is_empty() {
                println!("\n⚠️  EXPIRED passwords: {}", expired.join(", "));
            }
            if !expiring_soon.is_empty() {
                println!("\nExpiring within 7 days:");
                for (label, days) in &expiring_soon {
                    println!("  - {} ({} days remaining)", label, days);
                }
            }
            if !old_passwords.is_empty() {
                println!("\nOld passwords (>90 days, no expiry set):");
                for (label, days) in &old_passwords {
                    println!("  - {} ({} days old)", label, days);
                }
            }

            if reused.is_empty() {
                println!("\nPassword reuse: No reused passwords detected.");
            } else {
                println!("\nPassword reuse detected:");
                for group in &reused {
                    println!("  - {}", group.join(", "));
                }
            }
        }
    }
    Ok(())
}

fn key_path(config: &lilypad_core::AppConfig) -> PathBuf {
    PathBuf::from(&config.data_dir).join("key.json")
}

fn build_metadata(
    username: Option<String>,
    url: Option<String>,
    tags: Vec<String>,
    folder: Option<String>,
    entry_type: Option<String>,
) -> Result<EntryMetadata> {
    let mut metadata = EntryMetadata::default();
    apply_metadata_updates(&mut metadata, username, url, tags, folder, entry_type)?;
    Ok(metadata)
}

fn apply_metadata_updates(
    metadata: &mut EntryMetadata,
    username: Option<String>,
    url: Option<String>,
    tags: Vec<String>,
    folder: Option<String>,
    entry_type: Option<String>,
) -> Result<()> {
    if username.is_some() {
        metadata.username = username;
    }
    if url.is_some() {
        metadata.url = url;
    }
    if !tags.is_empty() {
        metadata.tags = tags;
    }
    if folder.is_some() {
        metadata.folder = folder;
    }
    if let Some(entry_type) = entry_type {
        metadata.entry_type = parse_entry_type(&entry_type)?;
    }
    Ok(())
}

fn parse_entry_type(value: &str) -> Result<EntryType> {
    let normalized = value.to_lowercase().replace('_', "-");
    match normalized.as_str() {
        "login" => Ok(EntryType::Login),
        "card" => Ok(EntryType::Card),
        "identity" => Ok(EntryType::Identity),
        "secure-note" => Ok(EntryType::SecureNote),
        "software-license" => Ok(EntryType::SoftwareLicense),
        "wifi" => Ok(EntryType::Wifi),
        "server" => Ok(EntryType::Server),
        "custom" => Ok(EntryType::Custom),
        _ => Err(anyhow!("unsupported entry type: {value}")),
    }
}

fn entry_type_label(entry_type: &EntryType) -> &'static str {
    match entry_type {
        EntryType::Login => "login",
        EntryType::Card => "card",
        EntryType::Identity => "identity",
        EntryType::SecureNote => "secure-note",
        EntryType::SoftwareLicense => "software-license",
        EntryType::Wifi => "wifi",
        EntryType::Server => "server",
        EntryType::Custom => "custom",
    }
}

fn parse_tags(tags: &str) -> Vec<String> {
    tags.split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
}

fn load_attachments(paths: Vec<PathBuf>) -> Result<Vec<Attachment>> {
    let mut attachments = Vec::new();
    for path in paths {
        let data = fs::read(&path)
            .with_context(|| format!("failed to read attachment {}", path.display()))?;
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| anyhow!("attachment has invalid filename"))?
            .to_string();
        let data_base64 = STANDARD.encode(data);
        attachments.push(Attachment {
            filename,
            mime_type: None,
            data_base64,
        });
    }
    Ok(attachments)
}

fn encrypt_entry_secret(
    key: &KeyMaterial,
    secret: &EntrySecret,
) -> Result<lilypad_core::Ciphertext> {
    let payload = serde_json::to_vec(secret)?;
    Ok(encrypt(key, &payload)?)
}

fn decrypt_entry_secret(key: &KeyMaterial, entry: &Entry) -> Result<EntrySecret> {
    let plaintext = decrypt(key, &entry.ciphertext)?;
    // Try to deserialize as structured JSON first
    if let Ok(secret) = serde_json::from_slice::<EntrySecret>(&plaintext) {
        return Ok(secret);
    }
    // Legacy format: plaintext is just the password as UTF-8 string
    // Use proper UTF-8 conversion with error handling instead of lossy
    let password = String::from_utf8(plaintext.clone()).map_err(|_| {
        anyhow!(
            "entry '{}' has corrupted data: invalid UTF-8 in legacy format",
            entry.label
        )
    })?;
    // Warn about legacy format so user knows to re-save the entry
    eprintln!(
        "Warning: entry '{}' is in legacy format. Consider updating it to migrate to new format.",
        entry.label
    );
    Ok(EntrySecret::new(password))
}

fn upsert_entry(
    vault: &mut Vault,
    key: &KeyMaterial,
    label: &str,
    metadata: EntryMetadata,
    secret: EntrySecret,
) -> Result<()> {
    let ciphertext = encrypt_entry_secret(key, &secret)?;
    if vault.find_entry(label).is_some() {
        vault.update_entry(label, ciphertext)?;
        if let Some(entry) = vault.entries.iter_mut().find(|entry| entry.label == label) {
            entry.metadata = metadata;
        }
    } else {
        vault.add_entry(Entry::new_with_metadata(label, metadata, ciphertext))?;
    }
    Ok(())
}

fn non_empty_value(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err("value cannot be empty".to_string());
    }
    Ok(value.to_string())
}

fn generate_password(
    length: usize,
    uppercase: bool,
    lowercase: bool,
    digits: bool,
    symbols: bool,
    copy: bool,
    clipboard_timeout: u64,
) -> Result<()> {
    if length == 0 {
        return Err(anyhow!("password length must be greater than 0"));
    }
    if length > 1024 {
        return Err(anyhow!("password length must be at most 1024"));
    }
    if !uppercase && !lowercase && !digits && !symbols {
        return Err(anyhow!(
            "at least one character set must be enabled (uppercase, lowercase, digits, symbols)"
        ));
    }

    let mut charset = String::new();
    if uppercase {
        charset.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
    }
    if lowercase {
        charset.push_str("abcdefghijklmnopqrstuvwxyz");
    }
    if digits {
        charset.push_str("0123456789");
    }
    if symbols {
        charset.push_str("!@#$%^&*()-_=+[]{}|;:,.<>?");
    }

    let charset_bytes = charset.as_bytes();
    let mut rng = rand::thread_rng();
    let password: String = (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..charset_bytes.len());
            charset_bytes[idx] as char
        })
        .collect();

    // Display password strength
    let strength = validate_password_strength(&password);
    println!("Generated password: {password}");
    println!("Strength: {}", strength.feedback());

    if copy {
        copy_to_clipboard_with_timeout(&password, clipboard_timeout)?;
        println!("Password copied to clipboard for {clipboard_timeout} seconds.");
    }

    Ok(())
}

fn export_audit_log(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    output: Option<&str>,
    format: &str,
    action_filter: Option<&str>,
    entry_filter: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
    limit: Option<usize>,
    master_password: Option<&str>,
) -> Result<()> {
    let (key, _) = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    // Parse date filters
    let after_ts = after.map(|s| parse_date_to_timestamp(s)).transpose()?;
    let before_ts = before.map(|s| parse_date_to_timestamp(s)).transpose()?;

    // Filter events
    let mut events: Vec<_> = vault
        .audit_log
        .iter()
        .filter(|event| {
            // Action filter
            if let Some(action) = action_filter {
                if !event.action.contains(action) {
                    return false;
                }
            }
            // Entry filter
            if let Some(entry) = entry_filter {
                match &event.entry_label {
                    Some(label) if label.contains(entry) => {}
                    Some(_) => return false,
                    None => return false,
                }
            }
            // Date filters
            if let Some(after) = after_ts {
                if event.timestamp < after {
                    return false;
                }
            }
            if let Some(before) = before_ts {
                if event.timestamp > before {
                    return false;
                }
            }
            true
        })
        .collect();

    // Sort by timestamp (most recent first for display)
    events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Apply limit
    if let Some(limit) = limit {
        events.truncate(limit);
    }

    let format = format.to_lowercase();
    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&events)?,
        "csv" => {
            let mut writer = WriterBuilder::new().from_writer(Vec::new());
            writer.serialize(("timestamp", "action", "entry_label", "date"))?;
            for event in &events {
                let date = format_timestamp_relative(event.timestamp);
                writer.serialize((
                    event.timestamp,
                    &event.action,
                    event.entry_label.as_deref().unwrap_or(""),
                    date,
                ))?;
            }
            writer.flush()?;
            String::from_utf8(writer.into_inner()?)?
        }
        "text" => {
            let mut output = String::new();
            output.push_str(&format!("Audit Log for '{}'\n", vault_name));
            output.push_str(&format!("{}\n\n", "=".repeat(40)));

            if events.is_empty() {
                output.push_str("No events found matching the filters.\n");
            } else {
                for event in &events {
                    let date = format_timestamp_relative(event.timestamp);
                    let entry = event.entry_label.as_deref().unwrap_or("-");
                    output.push_str(&format!("[{}] {} (entry: {})\n", date, event.action, entry));
                }
                output.push_str(&format!("\nTotal: {} events\n", events.len()));
            }
            output
        }
        _ => return Err(anyhow!("unsupported format: {format} (use json, csv, or text)")),
    };

    if let Some(path) = output {
        atomic_write(path, &content)?;
        println!("Exported {} audit events to {path}.", events.len());
    } else {
        println!("{content}");
    }

    Ok(())
}

/// Parse a date string (YYYY-MM-DD) or Unix timestamp to a Unix timestamp.
fn parse_date_to_timestamp(date_str: &str) -> Result<u64> {
    // Try parsing as Unix timestamp first
    if let Ok(ts) = date_str.parse::<u64>() {
        return Ok(ts);
    }

    // Try parsing as date (YYYY-MM-DD)
    if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        let datetime = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(datetime.and_utc().timestamp() as u64);
    }

    Err(anyhow!(
        "invalid date format: '{}'. Use YYYY-MM-DD or Unix timestamp.",
        date_str
    ))
}

/// Validates imported entry data against size limits and format requirements.
fn validate_import_entry(label: &str, secret: &EntrySecret, metadata: &EntryMetadata) -> Result<()> {
    // Validate password size
    if secret.password.len() > MAX_PASSWORD_SIZE {
        return Err(anyhow!(
            "entry '{}': password exceeds maximum size ({} bytes, max {} bytes)",
            label,
            secret.password.len(),
            MAX_PASSWORD_SIZE
        ));
    }

    // Validate notes size
    if let Some(notes) = &secret.notes {
        if notes.len() > MAX_NOTES_SIZE {
            return Err(anyhow!(
                "entry '{}': notes exceed maximum size ({} bytes, max {} bytes)",
                label,
                notes.len(),
                MAX_NOTES_SIZE
            ));
        }
    }

    // Validate URL format if present
    if let Some(url) = &metadata.url {
        if !url.is_empty() {
            validate_url_format(url)
                .with_context(|| format!("entry '{}' has invalid URL", label))?;
        }
    }

    // Validate TOTP secret format if present
    if let Some(totp_secret) = &secret.totp_secret {
        if !totp_secret.is_empty() {
            validate_totp_secret(totp_secret)
                .with_context(|| format!("entry '{}' has invalid TOTP secret", label))?;
        }
    }

    // Validate metadata
    metadata.validate().with_context(|| format!("entry '{}' has invalid metadata", label))?;

    // Validate secret (including attachments)
    secret.validate().with_context(|| format!("entry '{}' has invalid secret data", label))?;

    Ok(())
}

/// Validates URL format (basic validation).
fn validate_url_format(url: &str) -> Result<()> {
    // Check for valid URL schemes
    let valid_schemes = ["http://", "https://", "ftp://", "ftps://", "ssh://", "file://"];
    let has_valid_scheme = valid_schemes.iter().any(|scheme| url.to_lowercase().starts_with(scheme));

    if !has_valid_scheme && !url.contains("://") {
        // Allow URLs without scheme (will be treated as https)
        // But must have at least a domain-like structure
        if !url.contains('.') && !url.starts_with("localhost") {
            return Err(anyhow!("URL '{}' appears malformed (no domain)", url));
        }
    }

    // Check for dangerous URL schemes
    let dangerous_schemes = ["javascript:", "data:", "vbscript:"];
    for scheme in dangerous_schemes {
        if url.to_lowercase().starts_with(scheme) {
            return Err(anyhow!("URL '{}' uses a potentially dangerous scheme", url));
        }
    }

    // Check for control characters
    if url.chars().any(|c| c.is_control()) {
        return Err(anyhow!("URL contains control characters"));
    }

    Ok(())
}

/// Validates TOTP secret format (should be valid base32).
fn validate_totp_secret(secret: &str) -> Result<()> {
    // TOTP secrets should be base32 encoded
    // Valid base32 characters: A-Z and 2-7
    let clean_secret = secret.to_uppercase().replace([' ', '-'], "");

    if clean_secret.is_empty() {
        return Err(anyhow!("TOTP secret is empty"));
    }

    // Check for valid base32 characters
    for c in clean_secret.chars() {
        if !matches!(c, 'A'..='Z' | '2'..='7' | '=') {
            return Err(anyhow!(
                "TOTP secret contains invalid character '{}' (must be base32: A-Z, 2-7)",
                c
            ));
        }
    }

    // Try to decode to verify it's valid base32
    let secret_obj = Secret::Encoded(clean_secret);
    secret_obj.to_bytes().map_err(|e| anyhow!("invalid TOTP secret: {}", e))?;

    Ok(())
}

/// Generates shell completion scripts to stdout.
fn generate_completions(shell: Shell) {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, &mut io::stdout());
}

/// Changes the master password without re-encrypting vault entries.
fn change_master_password(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    current_master_password: Option<&str>,
) -> Result<()> {
    let current_password = current_master_password.ok_or_else(|| {
        anyhow!("current master password required (--master-password or LILYPAD_MASTER_PASSWORD)")
    })?;

    // Load and verify current key
    let key_path = key_path(config);
    let (key, _) = load_key(&key_path, Some(current_password))?;

    // Verify vault exists and can be loaded
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    // Prompt for new password
    print!("Enter new master password: ");
    io::stdout().flush()?;
    let mut new_password = String::new();
    io::stdin().read_line(&mut new_password)?;
    let new_password = new_password.trim();

    if new_password.is_empty() {
        return Err(anyhow!("new master password cannot be empty"));
    }

    // Confirm new password
    print!("Confirm new master password: ");
    io::stdout().flush()?;
    let mut confirm_password = String::new();
    io::stdin().read_line(&mut confirm_password)?;
    let confirm_password = confirm_password.trim();

    if new_password != confirm_password {
        return Err(anyhow!("passwords do not match"));
    }

    // Check password strength
    let strength = validate_password_strength(new_password);
    if !strength.is_acceptable() {
        eprintln!("Warning: {}", strength.feedback());
    }

    // Generate new KDF parameters and derive key
    let new_params = KeyDerivationParams::generate();
    let new_key = derive_key(new_password, &new_params)?;

    // Verify the new key produces the same key_id (it won't - keys are different)
    // Actually for change-password we need to re-encrypt with the new key
    // This is essentially a key rotation but with explicit password change

    // Re-encrypt all entries with the new key
    let mut updated_vault = vault.clone();
    for entry in &mut updated_vault.entries {
        let plaintext = decrypt(&key, &entry.ciphertext)?;
        entry.ciphertext = encrypt(&new_key, &plaintext)?;
    }

    // Update vault metadata
    let new_metadata = KeyMetadata::new(&new_key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305)
        .with_kdf("argon2id");
    updated_vault.key_metadata = new_metadata;

    // Save the updated key file and vault
    let new_key_file = KeyFile::from_kdf(new_params);
    save_key(&key_path, &new_key_file)?;
    store.save_vault(&updated_vault, &new_key)?;

    println!("Master password changed successfully for vault '{vault_name}'.");
    Ok(())
}

/// Checks passwords against the HaveIBeenPwned database using k-anonymity.
///
/// This uses the HIBP range API which only sends the first 5 characters of the
/// SHA-1 hash, preserving privacy while checking for breached passwords.
fn breach_check(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    entry_label: Option<&str>,
    master_password: Option<&str>,
) -> Result<()> {
    let (key, _) = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    let entries_to_check: Vec<_> = if let Some(label) = entry_label {
        let entry = vault
            .find_entry(label)
            .ok_or_else(|| anyhow!("entry '{label}' not found"))?;
        vec![entry]
    } else {
        vault.entries.iter().collect()
    };

    if entries_to_check.is_empty() {
        println!("No entries to check.");
        return Ok(());
    }

    println!("Checking {} entries against HaveIBeenPwned database...", entries_to_check.len());
    println!("(This uses k-anonymity - only partial hashes are sent)\n");

    let client = reqwest::blocking::Client::builder()
        .user_agent("Lilypad-Password-Manager/0.1")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let mut breached_entries = Vec::new();
    let mut checked = 0;
    let total = entries_to_check.len();

    for entry in &entries_to_check {
        let secret = decrypt_entry_secret(&key, entry)?;

        match check_password_hibp(&client, &secret.password) {
            Ok(Some(count)) => {
                breached_entries.push((entry.label.clone(), count));
            }
            Ok(None) => {
                // Password not found in breaches
            }
            Err(e) => {
                eprintln!("Warning: Could not check '{}': {}", entry.label, e);
            }
        }

        checked += 1;
        if checked % 10 == 0 {
            eprint!("\rChecked {}/{} entries...", checked, total);
        }
    }

    if checked >= 10 {
        eprintln!(); // Clear progress line
    }

    println!("\nBreach check results for '{vault_name}':");
    println!("Entries checked: {}", checked);

    if breached_entries.is_empty() {
        println!("\n✓ No breached passwords found!");
    } else {
        println!("\n⚠️  BREACHED PASSWORDS DETECTED:");
        println!("The following entries use passwords found in known data breaches:\n");
        for (label, count) in &breached_entries {
            println!("  - {} (seen {} times in breaches)", label, count);
        }
        println!("\nThese passwords should be changed immediately.");
    }

    Ok(())
}

/// Checks a single password against HaveIBeenPwned using k-anonymity.
/// Returns Ok(Some(count)) if breached, Ok(None) if not found, Err on network error.
fn check_password_hibp(client: &reqwest::blocking::Client, password: &str) -> Result<Option<u64>> {
    // Hash the password with SHA-1
    let mut hasher = Sha1::new();
    hasher.update(password.as_bytes());
    let hash = hasher.finalize();
    let hash_hex = format!("{:X}", hash);

    // Split into prefix (first 5 chars) and suffix (rest)
    let prefix = &hash_hex[..5];
    let suffix = &hash_hex[5..];

    // Query the HIBP API with the prefix
    let url = format!("https://api.pwnedpasswords.com/range/{}", prefix);
    let response = client.get(&url).send()?;

    if !response.status().is_success() {
        return Err(anyhow!("HIBP API returned status {}", response.status()));
    }

    let body = response.text()?;

    // Search for our suffix in the response
    for line in body.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() == 2 && parts[0].eq_ignore_ascii_case(suffix) {
            let count: u64 = parts[1].parse().unwrap_or(0);
            return Ok(Some(count));
        }
    }

    Ok(None)
}

/// Shows the change history of an entry.
fn show_entry_history(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    limit: usize,
    master_password: Option<&str>,
) -> Result<()> {
    let (key, _) = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    let entry = vault
        .find_entry(label)
        .ok_or_else(|| anyhow!("entry '{label}' not found"))?;

    println!("History for '{}' (showing up to {} records):\n", label, limit);

    if entry.history.is_empty() {
        println!("No history records found.");
        return Ok(());
    }

    for (i, record) in entry.get_history().take(limit).enumerate() {
        let timestamp = format_timestamp_relative(record.timestamp);
        let change_type = match &record.change_type {
            EntryChangeType::Created => "Created",
            EntryChangeType::PasswordChanged => "Password changed",
            EntryChangeType::MetadataUpdated => "Metadata updated",
            EntryChangeType::NotesUpdated => "Notes updated",
            EntryChangeType::Renamed => "Renamed",
            EntryChangeType::TotpUpdated => "TOTP updated",
            EntryChangeType::AttachmentsUpdated => "Attachments updated",
        };

        print!("{}. {} - {}", i + 1, timestamp, change_type);
        if let Some(desc) = &record.description {
            print!(" ({})", desc);
        }
        if record.previous_ciphertext.is_some() {
            print!(" [previous password stored]");
        }
        println!();
    }

    let total = entry.history.len();
    if total > limit {
        println!("\n... and {} more records (use --limit to see more)", total - limit);
    }

    println!("\nTotal history records: {}", total);
    println!("Password changes: {}", entry.password_change_count());

    Ok(())
}

fn create_backup(store: &LocalStore, vault_name: &str) -> Result<()> {
    let backup_name = store.create_backup(vault_name)?;
    println!("✓ Backup created: {}", backup_name);
    println!("  Location: {}", store.backup_dir().join(&backup_name).display());
    Ok(())
}

fn list_backups(store: &LocalStore, vault_name: Option<&str>) -> Result<()> {
    let backup_dir = store.backup_dir();
    if !backup_dir.exists() {
        println!("No backups found.");
        return Ok(());
    }

    // If vault name specified, list only its backups
    if let Some(name) = vault_name {
        let backups = store.list_backups(name)?;
        if backups.is_empty() {
            println!("No backups found for vault '{}'.", name);
            return Ok(());
        }

        println!("Backups for vault '{}':\n", name);
        for backup in backups {
            let timestamp = format_timestamp_relative(backup.created_at);
            let size = format_size(backup.size_bytes);
            println!("  {} ({}, {})", backup.filename, timestamp, size);
        }
        return Ok(());
    }

    // List all backups grouped by vault
    let mut all_backups: std::collections::HashMap<String, Vec<lilypad_storage::BackupInfo>> =
        std::collections::HashMap::new();

    for entry in fs::read_dir(&backup_dir)? {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().to_string();
        if filename.ends_with(".backup") {
            if let Some(vault) = filename.split('_').next() {
                let backups = store.list_backups(vault)?;
                all_backups.entry(vault.to_string()).or_default();
                for b in backups {
                    all_backups.get_mut(vault).unwrap().push(b);
                }
            }
        }
    }

    if all_backups.is_empty() {
        println!("No backups found.");
        return Ok(());
    }

    println!("All backups:\n");
    for (vault, backups) in all_backups {
        println!("Vault '{}':", vault);
        // Dedup backups (in case they were added multiple times)
        let mut seen = std::collections::HashSet::new();
        for backup in backups {
            if seen.insert(backup.filename.clone()) {
                let timestamp = format_timestamp_relative(backup.created_at);
                let size = format_size(backup.size_bytes);
                println!("  {} ({}, {})", backup.filename, timestamp, size);
            }
        }
        println!();
    }

    Ok(())
}

fn restore_backup(store: &LocalStore, backup_name: &str, force: bool) -> Result<()> {
    if !force {
        eprintln!("⚠️  WARNING: This will replace the current vault with the backup.");
        eprintln!("   The current vault will be backed up first.");
        eprint!("   Continue? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Restore cancelled.");
            return Ok(());
        }
    }

    match store.restore_backup(backup_name)? {
        Some(old_backup) => {
            println!("✓ Backup restored successfully.");
            println!("  Previous vault backed up as: {}", old_backup);
        }
        None => {
            println!("✓ Backup restored successfully.");
        }
    }

    Ok(())
}

fn prune_backups(store: &LocalStore, vault_name: &str, keep: usize) -> Result<()> {
    let deleted = store.prune_backups(vault_name, keep)?;
    if deleted == 0 {
        println!("No old backups to delete (keeping {} most recent).", keep);
    } else {
        println!("✓ Deleted {} old backup(s), kept {} most recent.", deleted, keep);
    }
    Ok(())
}

fn verify_vault(store: &LocalStore, vault_name: &str) -> Result<()> {
    let vaults_dir = store.backup_dir().parent().unwrap().join("vaults");
    let vault_path = vaults_dir.join(format!("{}.lily", vault_name));

    if !vault_path.exists() {
        // Try legacy extension
        let legacy_path = vaults_dir.join(format!("{}.json", vault_name));
        if legacy_path.exists() {
            lilypad_storage::verify_vault_integrity(&legacy_path)?;
        } else {
            return Err(anyhow!("vault '{}' not found", vault_name));
        }
    } else {
        lilypad_storage::verify_vault_integrity(&vault_path)?;
    }

    println!("✓ Vault '{}' integrity verified successfully.", vault_name);
    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
