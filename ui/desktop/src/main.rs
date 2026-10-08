//! Lilypad Desktop Application
//!
//! A beautiful, secure password manager built with Iced.
//!
//! This is the main entry point for the Lilypad desktop application.

// The theme and font modules deliberately carry a full palette of styles,
// colors and icons; not every one is wired into the current lean UI.
#![allow(dead_code)]

mod app;
mod fonts;
mod message;
mod theme;

use iced::{Size, Task};

/// Application entry point
fn main() -> iced::Result {
    // Answered before any window or GPU setup, so release CI can smoke-test
    // the binary on a runner with no display.
    if std::env::args_os().nth(1).is_some_and(|a| a == "--version") {
        println!("lilypad-desktop {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Initialize logging: suppress noisy warnings from GPU and font subsystems
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

    // Run the application with custom fonts
    iced::application(LilypadApp::new, LilypadApp::update, LilypadApp::view)
        .title("Lilypad")
        .subscription(LilypadApp::subscription)
        .window_size(Size::new(1200.0, 800.0))
        .font(fonts::JETBRAINS_MONO_REGULAR)
        .font(fonts::JETBRAINS_MONO_BOLD)
        .font(fonts::JETBRAINS_MONO_SEMIBOLD)
        .font(fonts::JETBRAINS_MONO_MEDIUM)
        .font(fonts::JETBRAINS_MONO_LIGHT)
        .default_font(fonts::FONT_REGULAR)
        .run()
}

/// Wrapper struct for the Iced application
struct LilypadApp(app::LilypadApp);

impl LilypadApp {
    fn new() -> (Self, Task<message::Message>) {
        let (app, task) = app::LilypadApp::new();
        (Self(app), task)
    }

    fn update(&mut self, message: message::Message) -> Task<message::Message> {
        self.0.update(message)
    }

    fn view(&self) -> iced::Element<'_, message::Message> {
        self.0.view()
    }

    fn subscription(&self) -> iced::Subscription<message::Message> {
        self.0.subscription()
    }
}
