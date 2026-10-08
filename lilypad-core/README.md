# lilypad-core

Shared foundations for Lilypad: the cryptography (Argon2id key derivation,
XChaCha20-Poly1305), the vault and entry models, the error types and the
default configuration that the other crates of the workspace build on.

## Usage

```rust
use lilypad_core::default_config;

let config = default_config();
```
