# Desktop GUI

The desktop GUI is built with the `iced` retained-mode toolkit (v0.13), providing a pure-Rust, cross-platform interface. The layout mirrors modern password managers with a bottom navigation bar for categories (Credentials, Health, Generator, Sync, Account, Security), a top header for search and vault selection, and a central panel for content.

Key notes:
- Uses Iced's message-based architecture for predictable state management.
- Supports multiple themes (Classic Green, Night Bloom, Pond Light) with configurable settings.
- Features include multi-vault support, password generator, health dashboard, GitHub OAuth sync, CSV/JSON import/export, HIBP breach checking, key rotation, entry history, and backup/restore.
- Maintain cross-platform compatibility (Linux, macOS, Windows) by avoiding platform-specific APIs unless gated.
- Dependencies: `iced 0.13`, `arboard` (clipboard), `rfd` (file dialogs), `tokio` (async), `chrono` (time), `csv`/`sha1` (import/breach check).

See `src/src.md` for source organization details.
