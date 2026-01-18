# lilypad-storage

Abstraction de stockage pour Lilypad. Cette crate prépare les primitives pour
brancher des backends (local, distant, etc.).

## Format de stockage

Le stockage local est un fichier JSON chiffré. Le contenu du coffre (vault) est
sérialisé en JSON, puis chiffré avec XChaCha20-Poly1305 avant d'être écrit sur
disque. Le format est versionné pour permettre des migrations futures.

Structure d'un fichier de coffre (`<data_dir>/vaults/<nom>.json`) :

```json
{
  "version": 1,
  "key_metadata": {
    "key_id": "...",
    "algorithm": "XChaCha20-Poly1305"
  },
  "ciphertext": {
    "nonce": [/* 24 octets */],
    "data": [/* octets chiffrés */]
  }
}
```

Le payload chiffré contient le vault complet (nom, entrées, etc.), donc aucune
entrée n'est stockée en clair.

## Synchronisation (prévue)

L'interface de synchronisation est exposée via le trait `SyncBackend`. Elle
permettra d'envoyer/récupérer les blobs de coffre chiffrés, mais aucun backend
distant n'est encore implémenté.

## Usage

```rust
use lilypad_core::default_config;
use lilypad_storage::LocalStore;

let config = default_config();
let store = LocalStore::new(&config)?;
let status = store.status();
```
