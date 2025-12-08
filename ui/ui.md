# UI Interfaces

This folder collects Lilypad's user-facing interfaces. Each subfolder focuses on a specific presentation layer so that desktop, terminal, and future mobile builds can evolve independently while sharing the same security and domain guarantees.

- `desktop/` hosts the desktop GUI built with `egui`/`eframe`. Keep the layout modular and ensure that user-facing strings remain in English until localization is added.
- Future interfaces (CLI, TUI, mobile) should live in their own folders with matching documentation.

Follow the documentation guidance in `doc/structure.md` by keeping interface-specific notes, theming conventions, and accessibility reminders alongside the code in each subfolder.
