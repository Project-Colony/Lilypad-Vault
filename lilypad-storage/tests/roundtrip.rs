use lilypad_core::crypto::{encrypt, CryptoAlgorithm, KeyMaterial};
use lilypad_core::{AppConfig, Entry, KeyMetadata, Vault};
use lilypad_storage::LocalStore;
use tempfile::tempdir;

const VAULT_HEADER: &[u8] = b"LILYPAD_VAULT_V1\n# Lilypad vault (encrypted)\n";

#[test]
fn roundtrip_save_and_load_vault() {
    let dir = tempdir().expect("tempdir");
    let config = AppConfig {
        environment: "test".to_string(),
        data_dir: dir.path().to_string_lossy().to_string(),
    };
    let store = LocalStore::new(&config).expect("store");

    let key = KeyMaterial::generate();
    let metadata = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
    let mut vault = Vault::new("primary", metadata);
    let ciphertext = encrypt(&key, b"bank-secret").expect("encrypt");
    vault
        .add_entry(Entry::new("bank", ciphertext))
        .expect("add entry");

    store.save_vault(&vault, &key).expect("save");
    let payload = store.sync_payload("primary").expect("payload");
    assert!(payload.starts_with(VAULT_HEADER));
    let loaded = store.load_vault("primary", &key).expect("load");

    assert_eq!(vault, loaded);
}
