# Lilypad

Lilypad is a Rust-based password manager focused on security, reliability, and a clean developer experience. The project will grow into a fully featured tool for generating, storing, and syncing secrets across devices while keeping cryptography and safety front and center.

## Prerequisites
- **Rust toolchain**: Rust (stable) 1.78 or newer with `cargo` installed (use [`rustup`](https://rustup.rs/) to manage toolchains).
- **Build tools**: Standard C toolchain for your platform (e.g., `build-essential` on Debian/Ubuntu, Xcode Command Line Tools on macOS).
- **Optional developer tools**: `rustfmt` and `clippy` components for formatting and linting, and `cargo-edit` for dependency management quality-of-life commands.
- **Git**: Required for source control and fetching dependencies.

## Language Policy
The project language is **English** for now. All code comments, documentation, commit messages, and user-facing text must be written in English to avoid confusion while the foundations are being built. Additional languages will be added later as Lilypad matures.

## Getting Started
This repository is currently focused on documentation and planning. Development will follow the steps outlined in [`doc/doc.md`](doc/doc.md) as the codebase takes shape. Future milestones will include setting up the Rust workspace, defining the core crates (cryptography, storage, and interfaces), and wiring up continuous integration.
