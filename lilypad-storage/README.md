# lilypad-storage

Abstraction de stockage pour Lilypad. Cette crate prépare les primitives pour
brancher des backends (local, distant, etc.).

## Usage

```rust
use lilypad_core::default_config;
use lilypad_storage::LocalStore;

let config = default_config();
let store = LocalStore::new(&config)?;
let status = store.status();
```
