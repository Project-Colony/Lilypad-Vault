# Desktop GUI Source

This directory contains the Rust source for the Lilypad desktop application.

- `main.rs` bootstraps the `eframe` application, renders the welcome modal, and lays out the initial GUI panels.
- Keep future components modular (e.g., move panels or widgets into separate modules) to preserve readability and testability.
- When adding new files, document their purpose and UI responsibilities to stay aligned with the guidance in `doc/structure.md`.
