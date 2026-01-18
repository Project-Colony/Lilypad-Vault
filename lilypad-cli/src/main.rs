use anyhow::{anyhow, Context, Result};
use arboard::Clipboard;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use clap::{Parser, Subcommand};
use csv::{ReaderBuilder, WriterBuilder};
use lilypad_core::{
    decrypt, default_config, derive_key, encrypt, Attachment, Entry, EntryMetadata, EntrySecret,
    EntryType, KeyDerivationParams, KeyMaterial, KeyMetadata, Vault,
};
use lilypad_storage::LocalStore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use totp_rs::{Algorithm, Secret, TOTP};

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
    /// Run a basic password health audit.
    Audit {
        /// Vault name to inspect
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum KeyFile {
    Raw { key_hex: String },
    Kdf { params: KeyDerivationParams },
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let master_password = cli
        .master_password
        .or_else(|| std::env::var("LILYPAD_MASTER_PASSWORD").ok());
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
            master_password.as_deref(),
        ),
        Commands::Vaults => list_vaults(&store),
        Commands::RenameVault { from, to } => rename_vault(&store, &from, &to),
        Commands::DeleteVault { vault } => delete_vault(&store, &vault),
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
            master_password.as_deref(),
        ),
        Commands::List { vault } => {
            list_entries(&store, &config, &vault, master_password.as_deref())
        }
        Commands::Get {
            vault,
            label,
            copy,
            clipboard_timeout,
        } => get_entry(
            &store,
            &config,
            &vault,
            &label,
            copy,
            clipboard_timeout,
            master_password.as_deref(),
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
            master_password.as_deref(),
        ),
        Commands::Remove { vault, label } => {
            remove_entry(&store, &config, &vault, &label, master_password.as_deref())
        }
        Commands::Search { vault, query } => {
            search_entries(&store, &config, &vault, &query, master_password.as_deref())
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
            master_password.as_deref(),
        ),
        Commands::Import {
            vault,
            input,
            format,
        } => import_vault(
            &store,
            &config,
            &vault,
            &input,
            &format,
            master_password.as_deref(),
        ),
        Commands::RotateKey {
            vault,
            use_master_password,
            new_master_password,
        } => rotate_key(
            &store,
            &config,
            &vault,
            use_master_password,
            new_master_password.as_deref(),
            master_password.as_deref(),
        ),
        Commands::Totp { vault, label } => {
            show_totp(&store, &config, &vault, &label, master_password.as_deref())
        }
        Commands::Audit { vault } => {
            audit_vault(&store, &config, &vault, master_password.as_deref())
        }
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
        let params = KeyDerivationParams::generate();
        let key = derive_key(password, &params)?;
        (key, KeyFile::Kdf { params })
    } else {
        let key = KeyMaterial::generate();
        let key_hex = encode_hex(key.as_bytes());
        (key, KeyFile::Raw { key_hex })
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

fn list_vaults(store: &LocalStore) -> Result<()> {
    let vaults = store.list_vaults()?;
    if vaults.is_empty() {
        println!("No vaults found.");
        return Ok(());
    }
    println!("Vaults:");
    for name in vaults {
        println!("- {name}");
    }
    Ok(())
}

fn rename_vault(store: &LocalStore, from: &str, to: &str) -> Result<()> {
    store.rename_vault(from, to)?;
    println!("Vault '{from}' renamed to '{to}'.");
    Ok(())
}

fn delete_vault(store: &LocalStore, vault: &str) -> Result<()> {
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
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_key(&key_path(config), master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let metadata = build_metadata(username, url, tags, folder, entry_type)?;
    let mut secret = EntrySecret::new(value);
    secret.notes = notes;
    secret.totp_secret = totp_secret;
    secret.attachments = load_attachments(attachment)?;
    let ciphertext = encrypt_entry_secret(&key, &secret)?;
    vault.add_entry(Entry::new_with_metadata(label, metadata, ciphertext))?;
    store.save_vault(&vault, &key)?;
    println!("Entry '{label}' added to vault '{vault_name}'.");
    Ok(())
}

fn list_entries(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
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
        println!(
            "- {} (type: {}, folder: {}, tags: {}, updated: {})",
            entry.label, entry_type, folder, tags, entry.updated_at
        );
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
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let entry = vault
        .find_entry(label)
        .ok_or_else(|| anyhow!("entry '{label}' not found"))?;
    let secret = decrypt_entry_secret(&key, entry)?;
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
    println!("Password: {}", secret.password);
    if let Some(notes) = &secret.notes {
        println!("Notes: {notes}");
    }
    if let Some(totp_secret) = &secret.totp_secret {
        println!("TOTP secret: {totp_secret}");
    }
    if !secret.attachments.is_empty() {
        println!("Attachments: {}", secret.attachments.len());
    }
    if copy {
        copy_to_clipboard(&secret.password, clipboard_timeout)?;
        println!("Password copied to clipboard for {clipboard_timeout} seconds.");
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
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_key(&key_path(config), master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let (mut metadata, mut secret) = {
        let existing = vault
            .find_entry(label)
            .ok_or_else(|| anyhow!("entry '{label}' not found"))?;
        (
            existing.metadata.clone(),
            decrypt_entry_secret(&key, existing)?,
        )
    };
    apply_metadata_updates(&mut metadata, username, url, tags, folder, entry_type)?;
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
    let ciphertext = encrypt_entry_secret(&key, &secret)?;
    vault
        .update_entry(label, ciphertext)
        .with_context(|| format!("entry '{label}' not found"))?;
    if let Some(entry) = vault.entries.iter_mut().find(|entry| entry.label == label) {
        entry.metadata = metadata;
    }
    store.save_vault(&vault, &key)?;
    println!("Entry '{label}' updated.");
    Ok(())
}

fn remove_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_key(&key_path(config), master_password)?;
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
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let matches = vault.search_entries(query);
    if matches.is_empty() {
        println!("No entries match '{query}'.");
        return Ok(());
    }
    println!("Results for '{query}':");
    for entry in matches {
        println!("- {}", entry.label);
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
        fs::write(output, payload)?;
        println!("Encrypted vault exported to {output}.");
        return Ok(());
    }

    if !allow_plaintext {
        return Err(anyhow!(
            "plaintext export requires --allow-plaintext (format: {format})"
        ));
    }

    let key = load_key(&key_path(config), master_password)?;
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
            fs::write(output, payload)?;
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
    master_password: Option<&str>,
) -> Result<()> {
    let format = format.to_lowercase();
    if format == "lily" {
        let payload = fs::read(input)?;
        store.apply_sync_payload(vault_name, &payload)?;
        println!("Encrypted vault imported from {input}.");
        return Ok(());
    }

    let key = load_key(&key_path(config), master_password)?;
    let mut vault = match store.load_vault(vault_name, &key) {
        Ok(vault) => vault,
        Err(_) => {
            let metadata = KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305);
            Vault::new(vault_name, metadata)
        }
    };

    match format.as_str() {
        "json" => {
            let payload = fs::read(input)?;
            let import: VaultExport = serde_json::from_slice(&payload)?;
            for entry in import.entries {
                upsert_entry(&mut vault, &key, &entry.label, entry.metadata, entry.secret)?;
            }
        }
        "csv" => {
            let mut reader = ReaderBuilder::new().from_path(input)?;
            for result in reader.deserialize() {
                let record: CsvEntry = result?;
                let metadata = build_metadata(
                    if record.username.is_empty() {
                        None
                    } else {
                        Some(record.username)
                    },
                    if record.url.is_empty() {
                        None
                    } else {
                        Some(record.url)
                    },
                    parse_tags(&record.tags),
                    if record.folder.is_empty() {
                        None
                    } else {
                        Some(record.folder)
                    },
                    if record.entry_type.is_empty() {
                        None
                    } else {
                        Some(record.entry_type)
                    },
                )?;
                let mut secret = EntrySecret::new(record.password);
                if !record.notes.is_empty() {
                    secret.notes = Some(record.notes);
                }
                if !record.totp_secret.is_empty() {
                    secret.totp_secret = Some(record.totp_secret);
                }
                upsert_entry(&mut vault, &key, &record.label, metadata, secret)?;
            }
        }
        _ => {
            return Err(anyhow!("unsupported import format: {format}"));
        }
    }

    store.save_vault(&vault, &key)?;
    println!("Vault '{vault_name}' updated from {input}.");
    Ok(())
}

fn rotate_key(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    use_master_password: bool,
    new_master_password: Option<&str>,
    master_password: Option<&str>,
) -> Result<()> {
    let old_key = load_key(&key_path(config), master_password)?;
    let mut vault = store
        .load_vault(vault_name, &old_key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    let (new_key, key_file, metadata) = if use_master_password {
        let password = new_master_password
            .ok_or_else(|| anyhow!("new master password required (--new-master-password)"))?;
        let params = KeyDerivationParams::generate();
        let key = derive_key(password, &params)?;
        let metadata = KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305)
            .with_kdf("argon2id");
        (key, KeyFile::Kdf { params }, metadata)
    } else {
        let key = KeyMaterial::generate();
        let metadata = KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305);
        let key_hex = encode_hex(key.as_bytes());
        (key, KeyFile::Raw { key_hex }, metadata)
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
    let key = load_key(&key_path(config), master_password)?;
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

fn audit_vault(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_key(&key_path(config), master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    let mut weak = Vec::new();
    let mut duplicates: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for entry in &vault.entries {
        let secret = decrypt_entry_secret(&key, entry)?;
        if secret.password.len() < 12 {
            weak.push(entry.label.clone());
        }
        duplicates
            .entry(secret.password)
            .or_default()
            .push(entry.label.clone());
    }

    println!("Audit results for '{vault_name}':");
    if weak.is_empty() {
        println!("- No weak passwords detected.");
    } else {
        println!("- Weak passwords (<12 chars): {}", weak.join(", "));
    }
    let reused: Vec<String> = duplicates
        .into_iter()
        .filter(|(_, labels)| labels.len() > 1)
        .map(|(_, labels)| labels.join(", "))
        .collect();
    if reused.is_empty() {
        println!("- No reused passwords detected.");
    } else {
        println!("- Reused passwords: {}", reused.join(" | "));
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
    if let Ok(secret) = serde_json::from_slice::<EntrySecret>(&plaintext) {
        return Ok(secret);
    }
    let password = String::from_utf8_lossy(&plaintext).to_string();
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

fn copy_to_clipboard(value: &str, timeout: u64) -> Result<()> {
    let mut clipboard = Clipboard::new().map_err(|err| anyhow!("{err}"))?;
    clipboard
        .set_text(value.to_string())
        .map_err(|err| anyhow!("{err}"))?;
    if timeout > 0 {
        let value = value.to_string();
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(timeout));
            if let Ok(mut clipboard) = Clipboard::new() {
                if clipboard.get_text().ok().as_deref() == Some(&value) {
                    let _ = clipboard.set_text(String::new());
                }
            }
        });
    }
    Ok(())
}

fn save_key(path: &Path, key_file: &KeyFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(&key_file)?;
    fs::write(path, payload)?;
    Ok(())
}

fn load_key(path: &Path, master_password: Option<&str>) -> Result<KeyMaterial> {
    let payload = fs::read(path)
        .with_context(|| format!("key not found: {} (run `lilypad init`)", path.display()))?;
    let key_file: KeyFile = serde_json::from_slice(&payload)?;
    match key_file {
        KeyFile::Raw { key_hex } => {
            let bytes = decode_hex(&key_hex)?;
            KeyMaterial::from_bytes(&bytes).context("invalid key")
        }
        KeyFile::Kdf { params } => {
            let password = master_password.ok_or_else(|| {
                anyhow!("master password required (--master-password or LILYPAD_MASTER_PASSWORD)")
            })?;
            derive_key(password, &params).context("invalid kdf")
        }
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode_hex(hex: &str) -> Result<Vec<u8>> {
    let value = hex.trim();
    if value.is_empty() {
        return Err(anyhow!("empty key"));
    }
    if value.len() % 2 != 0 {
        return Err(anyhow!("invalid hex key"));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for chunk in value.as_bytes().chunks(2) {
        let chunk_str = std::str::from_utf8(chunk)?;
        let byte = u8::from_str_radix(chunk_str, 16).map_err(|_| anyhow!("invalid hex key"))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn non_empty_value(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err("value cannot be empty".to_string());
    }
    Ok(value.to_string())
}
