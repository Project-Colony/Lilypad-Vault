# lilypad-storage

Abstraction de stockage pour Lilypad. Cette crate prépare les primitives pour
brancher des backends (local, distant, etc.).

## Format de stockage

Le stockage local est un fichier JSON chiffré. Le contenu du coffre (vault) est
sérialisé en JSON, puis chiffré avec XChaCha20-Poly1305 avant d'être écrit sur
disque. Le format est versionné pour permettre des migrations futures.

Structure d'un fichier de coffre (`<data_dir>/vaults/<nom>.lily`) :

Le fichier commence par un en-tête magique `LILYPAD_VAULT_V1` suivi d'une ligne
lisible indiquant qu'il s'agit d'un coffre chiffré, puis du JSON chiffré. Les
fichiers `.json` existants restent pris en charge pour compatibilité, mais les
nouveaux coffres sont écrits en `.lily`.

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

## Synchronisation

L'interface de synchronisation est exposée via le trait `SyncBackend`. Le
backend GitHub est implémenté dans la crate `lilypad-oauth` et permet
d'envoyer/récupérer les blobs de coffre chiffrés vers un dépôt privé GitHub.
Voir `lilypad-oauth` pour les détails d'authentification OAuth et la gestion
des conflits.

## Usage

```rust
use lilypad_core::default_config;
use lilypad_storage::LocalStore;

let config = default_config();
let store = LocalStore::new(&config)?;
let status = store.status();
```
