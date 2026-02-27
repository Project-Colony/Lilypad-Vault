# Lilypad Project Structure and Documentation Conventions

Lilypad must be organized as a modular Rust codebase rather than a single monolithic file. The layout below keeps components isolated, testable, and easy to extend while the project grows.

## Structural Requirements
- **No single-file implementation**: Core logic, storage, configuration, interfaces, and cryptography must not be hardcoded into one Rust source file.
- **Module-first organization**: Group related code into clearly named modules and folders (e.g., `config/`, `storage/`, `crypto/`, `ui/`). Avoid mixing unrelated concerns in the same module.
- **Concern separation**: Each major concern should live in its own folder or crate so teams can evolve features independently without creating tight coupling.

## Current Layout

The workspace contains seven crates:

- `lilypad-core/`: Cryptographic workflows (XChaCha20-Poly1305, Argon2id key derivation), data models (Vault, Entry, EntryMetadata, EntrySecret), domain services, and error types.
- `lilypad-storage/`: Encrypted local persistence with versioned `.lily` file format, backup/restore, and the storage interface consumed by sync backends.
- `lilypad-common/`: Shared utilities — clipboard management, password health analysis (HIBP breach check, strength scoring), advanced search engine, input validation, timestamp formatting, and key file management.
- `lilypad-oauth/`: GitHub OAuth (Device Flow and Authorization Code Flow), token management, and encrypted vault sync backend with conflict detection and multi-device support.
- `lilypad-cli/`: Full-featured command-line interface with 40+ commands covering vault management, entry CRUD, import/export (8 formats), backup, audit, and sync operations.
- `ui/tui/`: Terminal UI built with `ratatui` and `crossterm`, offering keyboard-driven vault management with multi-vault, search, password generation, health dashboard, and TOTP support.
- `ui/desktop/`: Desktop GUI built with `iced` (~10k lines), featuring multi-step onboarding, multi-vault support, theme switching (3 themes), health dashboard with breach detection, password generator, GitHub OAuth sync, import/export, backup/restore, master password rotation, and entry history.

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
