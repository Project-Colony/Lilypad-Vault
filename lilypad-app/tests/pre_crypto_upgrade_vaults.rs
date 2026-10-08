//! Vaults written before the RustCrypto upgrade (argon2 0.5, sha2 0.10,
//! chacha20poly1305 0.10, rand_core 0.6) must keep opening, byte for byte.
//!
//! `fixtures/pre-crypto-upgrade/data` was written by commit b9b7b3d with
//! [`PASSWORD`]: `current` is a self-contained V2 vault (Argon2id parameters
//! embedded in the file), `legacy` is a V1 vault whose key comes from the
//! `key.json` keyfile beside it. `expected.json` is what that same commit read
//! back from them. Never regenerate these files with newer code: their whole
//! point is that they come from the old one.

use lilypad_app::{reveal_secret, App, OpenOptions};
use lilypad_common::KeyFile;
use lilypad_storage::{verify_vault_integrity, LocalStore};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Composed (NFC) accents, as the vaults were written. The test also unlocks
/// with the decomposed (NFD) spelling, which only works through NFC
/// normalization.
const PASSWORD: &str = "Lilypad fixture p\u{e4}ssw\u{f6}rd";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pre-crypto-upgrade")
}

/// Everything a user can read back from a vault: the derived key's id, the
/// decrypted vault (entries, metadata, audit log, key metadata) and every
/// entry's decrypted secret.
fn read_back(app: &App, name: &str) -> Value {
    let session = app.unlock(name, PASSWORD).expect("unlock");
    let secrets: BTreeMap<String, Value> = session
        .vault()
        .entries
        .iter()
        .map(|entry| {
            let secret = reveal_secret(&session, &entry.label).expect("reveal secret");
            (
                entry.label.clone(),
                serde_json::to_value(secret.get()).unwrap(),
            )
        })
        .collect();
    json!({
        "key_id": session.key().key_id(),
        "vault": session.vault(),
        "secrets": secrets,
    })
}

#[test]
fn vaults_from_before_the_crypto_upgrade_still_open() {
    // Work on a copy so nothing the app writes (lock files) lands in the tree.
    let dir = tempfile::tempdir().unwrap();
    for file in ["key.json", "vaults/current.lily", "vaults/legacy.lily"] {
        let target = dir.path().join(file);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(fixtures().join("data").join(file), &target).unwrap();
    }
    let app = App::open(OpenOptions {
        data_dir: Some(dir.path().to_path_buf()),
        auto_lock_after: None,
    })
    .unwrap();
    let expected: Value =
        serde_json::from_slice(&fs::read(fixtures().join("expected.json")).unwrap()).unwrap();

    for name in ["current", "legacy"] {
        let file = dir.path().join("vaults").join(format!("{name}.lily"));
        // The SHA-256 checksum stored in the file still matches its ciphertext.
        verify_vault_integrity(&file).expect("stored checksum");
        // Same key id (SHA-256 of the Argon2id key), same vault, same secrets.
        assert_eq!(read_back(&app, name), expected[name], "{name}");

        // Decomposed accents, as macOS input produces them, derive the same key.
        let nfd: String = PASSWORD.nfd().collect();
        assert_ne!(nfd, PASSWORD);
        let key = app.derive_unlock_key(name, &nfd).expect("NFD unlock");
        assert_eq!(json!(key.key_id()), expected[name]["key_id"], "{name} NFD");
    }

    let current = fs::read(dir.path().join("vaults/current.lily")).unwrap();
    let kdf = LocalStore::kdf_params_from_bytes(&current)
        .unwrap()
        .expect("a V2 vault embeds its KDF parameters");
    assert_eq!(
        (
            kdf.algorithm.as_str(),
            kdf.memory_kib,
            kdf.iterations,
            kdf.parallelism
        ),
        ("argon2id", 16 * 1024, 3, 4)
    );

    let key_file: KeyFile =
        serde_json::from_slice(&fs::read(dir.path().join("key.json")).unwrap()).unwrap();
    let KeyFile::Kdf { params } = key_file else {
        panic!("the legacy vault's keyfile derives its key from the password");
    };
    assert_eq!(
        (params.memory_kib, params.iterations, params.parallelism),
        (8 * 1024, 2, 1)
    );
}
