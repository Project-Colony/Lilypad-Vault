# lilypad-storage

Local and remote storage abstraction for Lilypad. This crate handles encrypted
vault persistence on disk and exposes the interface consumed by sync backends.

## Storage Format

Local storage uses an encrypted file format. Vault contents are serialized to
JSON, encrypted with XChaCha20-Poly1305, and written to disk. The format is
versioned to support future migrations.

Vault file location: `<data_dir>/vaults/<name>.lily`

The file starts with a `LILYPAD_VAULT_V1` magic header followed by a
human-readable marker indicating the file is an encrypted vault, then the
encrypted JSON payload. Legacy `.json` files are still supported for backward
compatibility, but new vaults are always written as `.lily`.

```json
{
  "version": 1,
  "key_metadata": {
    "key_id": "...",
    "algorithm": "XChaCha20-Poly1305"
  },
  "ciphertext": {
    "nonce": [/* 24 bytes */],
    "data": [/* encrypted bytes */]
  }
}
```

The encrypted payload contains the full vault (name, entries, metadata), so no
entry data is stored in plaintext.

## Remote Synchronization

Remote sync is fully implemented via the `lilypad-oauth` crate. The GitHub
backend pushes and pulls encrypted vault blobs to a private repository
(`lilypad-vault-<username>`), with SHA-based conflict detection, device
tracking, and `--force` resolution. See `lilypad-oauth` for OAuth
authentication details and conflict management.

## Usage

```rust
use lilypad_core::default_config;
use lilypad_storage::LocalStore;

let config = default_config();
let store = LocalStore::new(&config)?;
let status = store.status();
```
