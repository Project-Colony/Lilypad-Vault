use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use lilypad_core::{
    decrypt, default_config, derive_key, encrypt, Entry, KeyDerivationParams, KeyMaterial,
    KeyMetadata, Vault,
};
use lilypad_storage::LocalStore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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
    #[arg(
        long,
        value_name = "PASSWORD",
        env = "LILYPAD_MASTER_PASSWORD",
        global = true
    )]
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
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum KeyFile {
    Raw { key_hex: String },
    Kdf { params: KeyDerivationParams },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
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
            cli.master_password.as_deref(),
        ),
        Commands::Add {
            vault,
            label,
            value,
        } => add_entry(
            &store,
            &config,
            &vault,
            &label,
            &value,
            cli.master_password.as_deref(),
        ),
        Commands::List { vault } => {
            list_entries(&store, &config, &vault, cli.master_password.as_deref())
        }
        Commands::Get { vault, label } => get_entry(
            &store,
            &config,
            &vault,
            &label,
            cli.master_password.as_deref(),
        ),
        Commands::Update {
            vault,
            label,
            value,
        } => update_entry(
            &store,
            &config,
            &vault,
            &label,
            &value,
            cli.master_password.as_deref(),
        ),
        Commands::Remove { vault, label } => remove_entry(
            &store,
            &config,
            &vault,
            &label,
            cli.master_password.as_deref(),
        ),
        Commands::Search { vault, query } => search_entries(
            &store,
            &config,
            &vault,
            &query,
            cli.master_password.as_deref(),
        ),
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
        (
            key,
            KeyFile::Raw {
                key_hex: encode_hex(key.as_bytes()),
            },
        )
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

    println!(
        "Vault '{}' initialized in {}.",
        vault.name, config.data_dir
    );
    Ok(())
}

fn add_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    value: &str,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_key(&key_path(config), master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let ciphertext = encrypt(&key, value.as_bytes())?;
    vault.add_entry(Entry::new(label, ciphertext))?;
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
        println!("- {} (updated: {})", entry.label, entry.updated_at);
    }
    Ok(())
}

fn get_entry(
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
    let plaintext = decrypt(&key, &entry.ciphertext)?;
    println!("{}", String::from_utf8_lossy(&plaintext));
    Ok(())
}

fn update_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    value: &str,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_key(&key_path(config), master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let ciphertext = encrypt(&key, value.as_bytes())?;
    vault
        .update_entry(label, ciphertext)
        .with_context(|| format!("entry '{label}' not found"))?;
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

fn key_path(config: &lilypad_core::AppConfig) -> PathBuf {
    PathBuf::from(&config.data_dir).join("key.json")
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
    let payload = fs::read(path).with_context(|| {
        format!(
            "key not found: {} (run `lilypad init`)",
            path.display()
        )
    })?;
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
        let byte =
            u8::from_str_radix(chunk_str, 16).map_err(|_| anyhow!("invalid hex key"))?;
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
