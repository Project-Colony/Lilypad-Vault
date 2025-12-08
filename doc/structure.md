# Lilypad Project Structure and Documentation Conventions

Lilypad must be organized as a modular Rust codebase rather than a single monolithic file. The layout below keeps components isolated, testable, and easy to extend while the project grows.

## Structural Requirements
- **No single-file implementation**: Core logic, storage, configuration, interfaces, and cryptography must not be hardcoded into one Rust source file.
- **Module-first organization**: Group related code into clearly named modules and folders (e.g., `config/`, `storage/`, `crypto/`, `ui/`). Avoid mixing unrelated concerns in the same module.
- **Concern separation**: Each major concern should live in its own folder or crate so teams can evolve features independently without creating tight coupling.

## Suggested Layout
- `config/`: Configuration loading, validation, and environment integration.
- `storage/`: Persistence layers for vaults (file-based first, remote or sync backends later).
- `crypto/`: Key derivation, encryption/decryption routines, secure random utilities, and integrity checks.
- `ui/`: User-facing interfaces such as CLI, TUI, or desktop frontends that orchestrate core operations.
- `core/` or `domain/`: Data models, vault domain logic, and shared services used across interfaces and storage.
- `shared/` or `utils/`: Cross-cutting utilities (logging, telemetry, error handling) that should stay minimal to avoid dependency sprawl.

Within the `ui/` area, create subfolders per interface type (`cli/`, `tui/`, `desktop/`) so that platform-specific assets, theming hooks, and interaction models can evolve independently.

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
