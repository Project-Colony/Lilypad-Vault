# Desktop GUI Source

This directory contains the Rust source for the Lilypad desktop application, an
`iced` 0.14 frontend that is a thin client over the `lilypad-app` service layer
(all vault decisions - unlocking, mutation, sync - live there, not here).

- `main.rs` bootstraps the iced application, loads the embedded fonts, and runs
  the event loop.
- `app.rs` holds the `LilypadApp` state and the whole `update` logic: unlock
  with async Argon2 (`derive_unlock_key` off-thread, then `open_with_key`),
  entry selection/edit via the field-preserving `edit_entry`, Trash, settings
  persistence, and the GitHub sync handlers (device-flow login, push/pull,
  status - all network runs via `Task` + `spawn_blocking`). Unit tests for the
  state machine live at the bottom.
- `view.rs` (included into `app.rs`) renders the shell: unlock/create cards,
  the 3-pane vault (sidebar filters | list | inline detail/edit), overlays
  (generator, add form, delete confirm), and the copy toast.
- `settings.rs` (included into `app.rs`) renders the full-page categorized
  settings: General, Appearance (theme cards + density), Security (auto-lock,
  clipboard, change master password), Vault (contents + Watchtower health),
  Sync (GitHub account, status, push/pull), About.
- `message.rs` defines the `Message` enum for all UI events.
- `theme.rs` provides the 15 `LilypadTheme` palettes, the `UiVariation`
  density system, and reusable style functions.
- `fonts.rs` embeds JetBrainsMono Nerd Font, the icon palette, and the
  side-bearing-compensated `centered_icon` helpers.

Maintenance notes:
- Secret-bearing buffers (form password/notes/TOTP, master-password fields,
  the sync pull password) are zeroized on drop, on lock, and when settings
  close - keep that invariant when adding fields.
- Never call blocking crypto or network on the UI thread; follow the
  `derive_key_task` / sync-handler pattern.
- Keep this file in sync when adding modules (see `doc/structure.md`).
