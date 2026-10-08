# Changelog

All notable changes to Lilypad will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Core**: Compromised and IncompleteEntry health issue detection with severity levels
- **Core**: Entry history tracking with `EntryHistoryRecord` and `EntryChangeType`
- **Core**: Password expiration support with configurable expiry dates
- **Core**: Entry favorites, access tracking, and color labels
- **CLI**: `--force` flag for sync conflict resolution on push
- **CLI**: CSV audit log export and entry history commands
- **CLI**: `trash`/`restore` and `sync merge`; `import` detects 11 password-manager formats
- **CLI**: Shell completion generation (bash, zsh, fish, PowerShell)
- **CLI**: Password strength enforcement on vault init
- **Desktop**: CSV export for vault entries
- **Desktop**: Browser CSV import (Chrome, Firefox, Bitwarden, LastPass, 1Password, KeePass)
- **Desktop**: HIBP breach check using k-anonymity API
- **Desktop**: Master password rotation with full re-encryption across all vaults
- **Desktop**: Entry history view
- **Desktop**: Vault backup and restore with timestamped filenames
- **Desktop**: Account settings persistence to disk
- **Desktop**: Multi-vault support with vault selector
- **Desktop**: Theme switching (Classic Green, Night Bloom, Pond Light)
- **TUI**: Complete rewrite with edit entries, delete confirmation, multi-vault, search, password generator, health dashboard, help screen, tags display
- **OAuth**: Token refresh for expired access tokens
- **OAuth**: Persistent device_id for multi-device sync metadata
- **Sync**: GitHub-based encrypted vault synchronization
- **CI/CD**: GitHub Actions pipeline (check, build, test, clippy, fmt, deny, audit, coverage)
- **CI/CD**: Release workflow with cross-platform binary builds
- **Project**: GPL-3.0-or-later LICENSE file
- **Project**: `deny.toml` for cargo-deny license and vulnerability checking
- **Project**: Comprehensive `.gitignore`

### Changed
- **App**: New `lilypad-app` service layer shared by the CLI, TUI and desktop app, which are rewritten as thin clients over it
- **Storage**: All three frontends use one data directory: `--data-dir`, else `$LILYPAD_DATA_DIR`, else the platform `Colony/Lilypad` directory the desktop app already used (`~/.config/Colony/Lilypad` on Linux, `~/Library/Application Support/Colony/Lilypad` on macOS, `%APPDATA%\Colony\Lilypad\config` on Windows). The CLI and TUI no longer default to `./.lilypad`; the CLI prints a hint when it finds vaults there, and `--data-dir .lilypad` still opens them
- **Sync**: Each vault syncs to `vaults/<name>.lily` in the `lilypad-vault-<user>` repository instead of a single root `vault.lily`. A vault pushed only by an older build is not pulled automatically: move the remote file to `vaults/<name>.lily` first

### Removed
- **CLI**: `rotate-key`; `change-master-password` re-encrypts the vault instead
- **CLI**: `verify-vault`; unlocking a vault (for example `list`) authenticates every byte with the AEAD tag, a stronger check than the old checksum
- **CLI**: `sync delete`; delete `vaults/<name>.lily` from the `lilypad-vault-<user>` repository on GitHub
- **CLI**: `sync pull --force`; pull validates the remote vault and keeps a safety backup, and `sync merge` reconciles diverged copies
- **CLI**: `export --format lily|json` and `import --format/--source`; export writes Lilypad CSV and import detects the format (the `.lily` file itself is the native backup, see `backup`)

### Fixed
- Desktop TOTP detection now correctly reads `totp_secret.is_some()` instead of hardcoded `false`
- Production `.expect()` replaced with graceful `process::exit(1)` in desktop app initialization
- Environment variable warning prefix changed from "Note" to "WARNING"
- Documentation updated to reflect actual framework choices (iced, not egui/Tauri)
- MSRV standardized to 1.89.0 across all documentation

### Security
- XChaCha20-Poly1305 encryption with Argon2id key derivation
- Zeroize for all sensitive data in memory (passwords, keys, tokens)
- Constant-time comparison via `subtle` crate for authentication
- OAuth tokens stored with restricted file permissions (0o600 on Unix)
- No unsafe code, no panics in production, no unwrap/expect in non-test code
