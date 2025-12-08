# Desktop GUI

The desktop GUI is implemented with the `egui` ecosystem using `eframe` for native windowing. The initial layout mirrors modern password managers with a sidebar for vault navigation, a top header for global actions, and a central area for credential previews.

Key notes:
- Keep the UI modular so panels and dialogs can evolve without reshaping core logic.
- Favor immediate-mode patterns from `egui` for predictable rendering and input handling.
- Maintain cross-platform compatibility (Linux, macOS, Windows) by avoiding platform-specific APIs unless gated.
- Use the welcome modal as the first interaction to introduce Lilypad's relationship to the Colony project and to gather goodwill for feedback.

See `src/src.md` for source organization details.
