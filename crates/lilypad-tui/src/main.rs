//! Lilypad TUI - a keyboard-driven terminal frontend.
//!
//! A thin client over `lilypad-core`: unlocking, entry access, and every write
//! go through the service layer. In particular unlocking uses `App::unlock`,
//! which only ever reads - so the original data-loss bug (a wrong master
//! password overwriting the vault with an empty one) is impossible here by
//! construction. The session carries an idle auto-lock policy.

mod app;
mod input;
#[cfg(test)]
mod tests;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

fn main() -> Result<()> {
    // Answered before the terminal is switched to raw mode, so release CI can
    // smoke-test the binary without a TTY.
    if std::env::args_os().nth(1).is_some_and(|a| a == "--version") {
        println!("lilypad-tui {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let mut app = App::new(None)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run<B>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B: ratatui::backend::Backend,
    B::Error: Send + Sync + 'static,
{
    loop {
        app.tick();
        terminal.draw(|f| app.draw(f))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    app.on_key(key);
                }
            }
        }
        if app.should_quit {
            // Best-effort clipboard clear on exit.
            if app.clipboard_clear_at.is_some() {
                let _ = lilypad_core::clipboard::copy_to_clipboard("");
            }
            break;
        }
    }
    Ok(())
}
