# Lilypad Development Guide

This document outlines how to develop Lilypad, a Rust-based password manager. It explains the planned architecture, recommended workflows, and steps to build, run, and test the project. All documentation and code must be written in English for now; additional languages will be introduced later.

## Development Steps
1. **Set up tooling**
   - Install Rust (stable) 1.78 or newer with `cargo` via [`rustup`](https://rustup.rs/).
   - Add the `rustfmt` and `clippy` components: `rustup component add rustfmt clippy`.
   - Install optional helpers such as `cargo-edit` for managing dependencies.
2. **Create the workspace layout**
   - Initialize a Cargo workspace with crates for `core` (cryptography and secrets domain), `storage` (local and synced persistence), and `interfaces` (CLI, TUI, or desktop frontends).
   - Add shared tooling configuration (e.g., `rustfmt.toml`, `clippy.toml`) at the workspace root.
3. **Implement core functionality**
   - Define domain models for vaults, entries, key material, and audit logs.
   - Integrate cryptographic primitives (key derivation, encryption/decryption, secure random generation) using vetted crates.
   - Implement secure storage backends (local file-based vaults first, optional remote sync later).
   - Provide validation, error handling, and logging instrumentation.
4. **Develop user interfaces**
   - Build an initial command-line interface for creating vaults, adding/retrieving secrets, and rotating keys.
   - Plan for additional interfaces (e.g., TUI/desktop) while ensuring consistent UX across platforms.
   - Choose frameworks that match deployment needs:
     - CLI: `clap` or `lexopt` for argument parsing; pair with `indicatif` for progress UI where helpful.
     - TUI: `ratatui` with `crossterm` offers portable terminal widgets and keyboard handling.
     - Desktop GUI: `Tauri` provides a lightweight shell with strong sandboxing; `egui` is a solid choice for a pure-Rust immediate mode UI.
5. **Quality and distribution**
   - Add automated tests for core logic and CLI behavior.
   - Wire up continuous integration for formatting, linting, testing, and security scanning.
   - Prepare release packaging and artifact signing once stability improves.

## Architecture Overview
- **Core crate**: Owns cryptographic workflows (key derivation, encryption, integrity checks), data models, and domain services.
- **Storage crate**: Handles persistence for vaults, including file-based storage with future support for encrypted synchronization providers.
- **Interface crates**: Provide user interactions (initially CLI), translating user intent into core operations while preserving security guarantees.
- **Shared utilities**: Common helpers for configuration loading, logging, telemetry, and error reporting.

This modular structure keeps cryptography and storage concerns isolated from UI layers, making it easier to audit, test, and swap components without risking regressions in sensitive areas.

## Development Workflow
- **Build**: `cargo build` to compile all crates in the workspace.
- **Format**: `cargo fmt --all` to ensure consistent style.
- **Lint**: `cargo clippy --all-targets --all-features -- -D warnings` to keep the codebase warning-free.
- **Test**: `cargo test` for unit and integration coverage; add feature flags to exercise optional components.
- **Run**: `cargo run --bin lilypad-cli -- <args>` once the CLI crate exists.
- **Security checks**: Periodically audit dependencies with `cargo audit` and review cryptographic usage against current best practices.

### Interface Quality Checklist
- **Accessibility**: Provide keyboard-first navigation in CLI/TUI flows and ensure GUI widgets have descriptive labels and shortcuts.
- **Themeability**: Keep colors and typography configurable; expose a small theming API for GUI/TUI builds instead of hardcoding styles.
- **State boundaries**: Route user intent through the interface crate and avoid leaking GUI/TUI state into core or storage crates.
- **Offline defaults**: Design flows to work without network access; prompt before enabling any sync-related features.

### Testing Guidance
- Add snapshot-style tests for CLI/TUI output where possible to keep UX stable.
- Mock storage backends and cryptographic primitives in integration tests to avoid leaking secrets and to keep tests deterministic.
- Include smoke tests for GUI builds that validate window creation, theming hooks, and menu actions without requiring a real backend.

Run these commands locally and in CI to keep changes safe and consistent. Update the workflow as new tooling or checks are introduced.

## Desktop GUI (initial version)
- **Framework**: The first desktop interface uses the `egui` immediate-mode toolkit via the `eframe` native shell. This keeps the codebase pure Rust, simplifies cross-platform builds (Linux, macOS, Windows), and matches Lilypad's preference for lightweight, auditable dependencies.
- **Modal onboarding**: On launch, a welcome modal introduces Lilypad as part of the Colony project, explains that it is free and in active development, and invites feedback via the Colony GitHub repository before the main window appears.
- **Layout**: The initial GUI mirrors modern password managers with a header for search and actions, a sidebar for categories, and a central panel for credential previews. It is intentionally skeletal so panels and widgets can be swapped or expanded without disrupting core logic.
- **Source location**: Desktop GUI sources live in `ui/desktop/src/`. Supporting folder documentation sits in `ui/ui.md`, `ui/desktop/desktop.md`, and `ui/desktop/src/src.md`.
- **Dependencies**: Use the latest stable versions of the GUI stack (`eframe`, `egui`, and supporting crates such as `webbrowser`). When updating, prefer `cargo update -p <crate>` for targeted bumps or `cargo install cargo-edit` followed by `cargo upgrade` to refresh all dependencies. Always review changelogs for breaking changes and re-run `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` after upgrading.
- **Running the GUI**: From the repository root, run `cargo run -p lilypad-desktop` to launch the interface during development. For release builds, prefer `cargo run -p lilypad-desktop --release` to match production settings.

## Extending This Document
As Lilypad evolves, expand this guide with:
- Detailed module overviews and diagrams for new components.
- Migration notes when making storage or cryptography changes.
- Platform-specific setup guides and troubleshooting tips.
- Localization and internationalization guidelines once additional languages are supported.

When introducing a new interface, add a short section describing how its theming, shortcut mapping, and security prompts map onto the shared core workflows.

Maintaining clear, English-language documentation now will reduce friction as the team grows and prepares for multilingual support later.
