//! Lilypad Desktop Application
//!
//! A beautiful, secure password manager built with Iced.
//!
//! This is the main entry point for the Lilypad desktop application.

mod app;
mod message;
mod state;
mod theme;
mod views;

use iced::{Settings, Size, Task};

/// Application entry point
fn main() -> iced::Result {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    // Run the application
    iced::application("Lilypad", LilypadApp::update, LilypadApp::view)
        .subscription(LilypadApp::subscription)
        .window_size(Size::new(1200.0, 800.0))
        .run_with(LilypadApp::new)
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

    fn view(&self) -> iced::Element<message::Message> {
        self.0.view()
    }

    fn subscription(&self) -> iced::Subscription<message::Message> {
        self.0.subscription()
    }
}
