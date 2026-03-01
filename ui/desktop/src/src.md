# Desktop GUI Source

This directory contains the Rust source for the Lilypad desktop application built with the **Iced** framework (v0.14).

- `main.rs` bootstraps the Iced application, loads fonts, and runs the main event loop.
- `app.rs` contains the `LilypadApp` struct implementing Iced's Application pattern (state, update, view, subscription).
- `message.rs` defines the `Message` enum used for all UI events and state transitions.
- `state.rs` contains application state structures (`VaultEntry`, `AppSettings`, `Category`, etc.).
- `theme.rs` provides 15 themes (Classic Green, Night Bloom, Pond Light, Nord, Dracula, Solarized, etc.) with reusable style functions and UI variation support (Default, Compact, Spacious).
- `fonts.rs` embeds JetBrainsMono Nerd Font and defines a Nerd Font icon palette for the UI.
- `views/` contains modular view functions for each screen (vault, health, generator, sync, settings, unlock, welcome/onboarding, modals, etc.).
- Keep future components modular (e.g., move panels or widgets into separate modules) to preserve readability and testability.
- When adding new files, document their purpose and UI responsibilities to stay aligned with the guidance in `doc/structure.md`.
