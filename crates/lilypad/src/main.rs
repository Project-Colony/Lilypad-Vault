//! Lilypad, the desktop app: a password manager built with iced on the
//! Colony design system.

// A GUI program: on Windows, without this, launching it from Colony or
// Explorer also opens an empty console window beside the app.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod icons;
mod message;
mod preferences;
mod state;
#[cfg(test)]
mod tests;
mod theme;
mod update;
mod view;

use iced::Size;
use state::LilypadApp;

fn main() -> iced::Result {
    // Answered before any window or GPU setup, so release CI can smoke-test
    // the binary on a runner with no display.
    if std::env::args_os().nth(1).is_some_and(|a| a == "--version") {
        println!("lilypad {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Logging: quiet the GPU and font subsystems.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into())
                .add_directive("wgpu_hal=error".parse().unwrap())
                .add_directive("wgpu_core=error".parse().unwrap())
                .add_directive("fontdb=error".parse().unwrap())
                .add_directive("naga=error".parse().unwrap()),
        )
        .init();

    // The Colony fonts (JetBrainsMono Nerd Font, OpenDyslexic, Font Awesome)
    // ship inside colony-ui. iced fixes its default font here, before the
    // preferences are read, so every text names its font through
    // `ui_font()` and friends, which follow the dyslexia-font toggle live.
    colony_ui::fonts::BYTES
        .into_iter()
        .fold(
            iced::application(LilypadApp::new, LilypadApp::update, LilypadApp::view),
            |app, font| app.font(font),
        )
        .title("Lilypad")
        .subscription(LilypadApp::subscription)
        .theme(|_: &LilypadApp| theme::iced_theme())
        .window_size(Size::new(1200.0, 800.0))
        .default_font(colony_ui::ui_font())
        .run()
}
