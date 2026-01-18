use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use lilypad_core::{decrypt, encrypt, default_config, Entry, KeyMaterial, KeyMetadata, Vault};
use lilypad_storage::LocalStore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(
    name = "lilypad",
    about = "CLI minimale pour gérer des coffres Lilypad.",
    long_about = None
)]
struct Cli {
    /// Dossier de stockage (par défaut: .lilypad)
    #[arg(long, value_name = "DIR")]
    data_dir: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Initialise un coffre et génère une clé locale.
    Init {
        /// Nom du coffre à créer
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
    /// Ajoute une entrée chiffrée à un coffre.
    Add {
        /// Nom du coffre à mettre à jour
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Libellé de l'entrée
        #[arg(value_parser = non_empty_value)]
        label: String,
        /// Valeur à chiffrer
        #[arg(value_parser = non_empty_value)]
        value: String,
    },
    /// Liste les entrées d'un coffre.
    List {
        /// Nom du coffre à inspecter
        #[arg(value_parser = non_empty_value)]
        vault: String,
    },
    /// Récupère une entrée d'un coffre.
    Get {
        /// Nom du coffre à lire
        #[arg(value_parser = non_empty_value)]
        vault: String,
        /// Libellé de l'entrée
        #[arg(value_parser = non_empty_value)]
        label: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct KeyFile {
    key_hex: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = default_config();
    if let Some(data_dir) = cli.data_dir {
        if data_dir.trim().is_empty() {
            return Err(anyhow!("data_dir ne peut pas être vide"));
        }
        config.data_dir = data_dir;
    }

    let store = LocalStore::new(&config)?;
    match cli.command {
        Commands::Init { vault } => init_vault(&store, &config, &vault),
        Commands::Add {
            vault,
            label,
            value,
        } => add_entry(&store, &config, &vault, &label, &value),
        Commands::List { vault } => list_entries(&store, &config, &vault),
        Commands::Get { vault, label } => get_entry(&store, &config, &vault, &label),
    }
}

fn init_vault(store: &LocalStore, config: &lilypad_core::AppConfig, vault: &str) -> Result<()> {
    let key_path = key_path(config);
    if key_path.exists() {
        return Err(anyhow!(
            "clé déjà initialisée dans {}",
            key_path.display()
        ));
    }

    let key = KeyMaterial::generate();
    save_key(&key_path, &key)?;

    let metadata = KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305);
    let vault = Vault::new(vault, metadata);
    store.save_vault(&vault, &key)?;

    println!(
        "Coffre '{}' initialisé dans {}.",
        vault.name,
        config.data_dir
    );
    Ok(())
}

fn add_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    value: &str,
) -> Result<()> {
    let key = load_key(&key_path(config))?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("coffre introuvable: {vault_name}"))?;
    let ciphertext = encrypt(&key, value.as_bytes())?;
    vault.add_entry(Entry::new(label, ciphertext));
    store.save_vault(&vault, &key)?;
    println!("Entrée '{label}' ajoutée au coffre '{vault_name}'.");
    Ok(())
}

fn list_entries(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
) -> Result<()> {
    let key = load_key(&key_path(config))?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("coffre introuvable: {vault_name}"))?;
    if vault.entries.is_empty() {
        println!("Aucune entrée dans '{vault_name}'.");
        return Ok(());
    }
    println!("Entrées dans '{vault_name}':");
    for entry in &vault.entries {
        println!("- {}", entry.label);
    }
    Ok(())
}

fn get_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
) -> Result<()> {
    let key = load_key(&key_path(config))?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("coffre introuvable: {vault_name}"))?;
    let entry = vault
        .entries
        .iter()
        .find(|entry| entry.label == label)
        .ok_or_else(|| anyhow!("entrée '{label}' introuvable"))?;
    let plaintext = decrypt(&key, &entry.ciphertext)?;
    println!("{}", String::from_utf8_lossy(&plaintext));
    Ok(())
}

fn key_path(config: &lilypad_core::AppConfig) -> PathBuf {
    PathBuf::from(&config.data_dir).join("key.json")
}

fn save_key(path: &Path, key: &KeyMaterial) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let key_file = KeyFile {
        key_hex: encode_hex(key.as_bytes()),
    };
    let payload = serde_json::to_vec_pretty(&key_file)?;
    fs::write(path, payload)?;
    Ok(())
}

fn load_key(path: &Path) -> Result<KeyMaterial> {
    let payload = fs::read(path).with_context(|| {
        format!(
            "clé introuvable: {} (exécutez `lilypad init`)",
            path.display()
        )
    })?;
    let key_file: KeyFile = serde_json::from_slice(&payload)?;
    let bytes = decode_hex(&key_file.key_hex)?;
    KeyMaterial::from_bytes(&bytes).context("clé invalide")
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode_hex(hex: &str) -> Result<Vec<u8>> {
    let value = hex.trim();
    if value.is_empty() {
        return Err(anyhow!("clé vide"));
    }
    if value.len() % 2 != 0 {
        return Err(anyhow!("clé hexadécimale invalide"));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for chunk in value.as_bytes().chunks(2) {
        let chunk_str = std::str::from_utf8(chunk)?;
        let byte = u8::from_str_radix(chunk_str, 16)
            .map_err(|_| anyhow!("clé hexadécimale invalide"))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn non_empty_value(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err("la valeur ne peut pas être vide".to_string());
    }
    Ok(value.to_string())
}
