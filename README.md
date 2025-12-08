# Lilypad

Lilypad is a Rust-based password manager focused on security, reliability, and a clean developer experience. The project will grow into a fully featured tool for generating, storing, and syncing secrets across devices while keeping cryptography and safety front and center. The roadmap prioritizes:

- **Reproducible security**: Documented cryptographic primitives, predictable key management, and transparent storage formats.
- **Maintainable modules**: Clearly separated crates for core logic, storage, and user interfaces to keep the codebase testable and auditable.
- **Flexible frontends**: CLI, TUI, and GUI experiences that can be iterated independently while sharing a common core.

## Prerequisites
- **Rust toolchain**: Rust (stable) 1.78 or newer with `cargo` installed (use [`rustup`](https://rustup.rs/) to manage toolchains).
- **Build tools**: Standard C toolchain for your platform (e.g., `build-essential` on Debian/Ubuntu, Xcode Command Line Tools on macOS).
- **Optional developer tools**: `rustfmt` and `clippy` components for formatting and linting, and `cargo-edit` for dependency management quality-of-life commands.
- **Git**: Required for source control and fetching dependencies.

## Language Policy
The project language is **English** for now. All code comments, documentation, commit messages, and user-facing text must be written in English to avoid confusion while the foundations are being built. Additional languages will be added later as Lilypad matures.

## Getting Started
This repository is currently focused on documentation and planning. Development will follow the steps outlined in [`doc/doc.md`](doc/doc.md) as the codebase takes shape. Future milestones will include setting up the Rust workspace, defining the core crates (cryptography, storage, and interfaces), and wiring up continuous integration.

## Framework Guidance
You have flexibility in selecting frameworks for each layer as long as security and maintainability stay front and center:

- **CLI**: Prefer `clap` or `lexopt` for argument parsing, combined with `indicatif` for progress output when needed.
- **TUI**: `ratatui` or `crossterm` enable a responsive, keyboard-first terminal experience.
- **Desktop GUI**: `Tauri` and `egui` are both viable; pick based on deployment targets and how much native integration you need.
- **Crypto and storage**: Use vetted crates such as `ring`, `age`, or `orion` for cryptography, and `serde` + `serde_json`/`toml` for structured storage metadata.

Feel free to prototype multiple interface layers in parallel, but keep the cryptographic and storage guarantees consistent across them.

## Documentation Expectation
- Keep new folders accompanied by a short Markdown explainer as described in [`doc/structure.md`](doc/structure.md).
- Expand [`doc/doc.md`](doc/doc.md) with implementation notes, diagrams, and troubleshooting steps as new crates or interfaces are added.
- Ensure all contributor-facing text remains in English until the localization plan is introduced.
