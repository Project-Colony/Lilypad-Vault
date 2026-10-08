# Lilypad Project Structure and Documentation Conventions

Lilypad must be organized as a modular Rust codebase rather than a single monolithic file. The layout below keeps components isolated, testable, and easy to extend while the project grows.

## Structural Requirements
- **No single-file implementation**: Core logic, storage, configuration, interfaces, and cryptography must not be hardcoded into one Rust source file.
- **Module-first organization**: Group related code into clearly named modules and folders (e.g., `config/`, `storage/`, `crypto/`, `ui/`). Avoid mixing unrelated concerns in the same module.
- **Concern separation**: Each major concern should live in its own folder or crate so teams can evolve features independently without creating tight coupling.

## Current Layout

The workspace contains eight crates:

- `lilypad-core/`: Cryptographic workflows (XChaCha20-Poly1305, Argon2id key derivation), data models (Vault, Entry, EntryMetadata, EntrySecret), domain services, and error types.
- `lilypad-storage/`: Encrypted local persistence with versioned `.lily` file format, atomic writes, backup/restore, and the storage interface consumed by sync backends.
- `lilypad-common/`: Shared utilities — clipboard management, password health analysis (strength scoring, reuse/expiry/2FA detection), advanced search engine, input validation, timestamp formatting, and key file management.
- `lilypad-oauth/`: GitHub OAuth (Device Flow), token management, and the encrypted vault sync backend.
- `lilypad-app/`: The shared application/service layer every frontend is a thin client of. Owns unlocking (never writes), the master-password normalization choke point, locked read-modify-write entry mutations (including the field-preserving `edit_entry`), Trash (soft delete/restore), health reports, settings, the audit log, entry history, the auto-detecting importer (LastPass, Bitwarden CSV/JSON, KeePassXC, 1Password, Safari, Chrome/Edge, Firefox, Proton Pass, Dashlane) with native CSV export, and validated sync (remote payloads are proven to decrypt before they can replace the local vault).
- `lilypad-cli/`: Command-line interface over `lilypad-app`: vault management, entry CRUD, trash/restore, audit/health, audit-log, entry history, import/export, generator, TOTP, backups, and GitHub sync (login/push/pull/status).
- `ui/tui/`: Terminal UI built with `ratatui` over `lilypad-app`: keyboard-driven vault management, search, generator, TOTP copy, Trash screen, and the persisted auto-lock/clipboard policy.
- `ui/desktop/`: Desktop GUI built with `iced 0.14` over `lilypad-app`: 3-pane vault shell (sidebar filters, list, inline detail/edit), 15 themes + 5 density modes, categorized settings page (general/appearance/security/vault/sync/about), Watchtower health, Trash, async Argon2 unlock, and GitHub sync (device-flow login, push/pull, status).

## Folder Documentation Convention
Every folder must contain a short Markdown file named after the folder. Each of these files should:
1. **Describe the purpose** of the folder.
2. **List the typical files and logic** it contains.
3. **Capture maintenance notes** for future contributors (e.g., invariants, testing expectations, or dependency constraints).
4. **Interface-specific guidance**: For UI folders, include theming, accessibility, and shortcut conventions so that CLI/TUI/GUI builds remain consistent.

Examples:
- `config/config.md` documents configuration formats, loaders, and safety considerations.
- `storage/storage.md` explains storage backends, file layouts, and migration rules.
- `ui/ui.md` outlines interface layers, command routing, and UX guidelines.

## Updating This Guide
Keep this document in sync with the evolving architecture. When introducing a new folder or crate, add its Markdown summary alongside the code and update the suggested layout above to reflect the new structure.
