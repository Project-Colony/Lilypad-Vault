//! Lilypad TUI - Terminal User Interface for the Lilypad password manager.
//!
//! A keyboard-driven interface for managing your vault in the terminal.

use anyhow::{anyhow, Result};
use arboard::Clipboard;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use lilypad_common::{
    keyfile::{load_key, save_key, KeyFile},
    time::format_timestamp_relative,
};
use lilypad_core::{
    decrypt, default_config, derive_key, encrypt, AppConfig, CryptoAlgorithm, KeyDerivationParams,
    KeyMaterial, KeyMetadata, Vault,
};
use lilypad_storage::LocalStore;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

/// Entry payload structure for the TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntryPayload {
    password: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    notes: String,
}

/// Decrypted vault entry for display
struct VaultEntry {
    label: String,
    username: String,
    password: String,
    url: String,
    notes: String,
    updated_at: u64,
}

impl Drop for VaultEntry {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

/// Application state
enum AppState {
    Locked,
    Unlocked,
    AddEntry,
    ViewEntry,
}

/// Input field focus
#[derive(Clone, Copy, PartialEq)]
enum InputField {
    Password,
    Label,
    Username,
    EntryPassword,
    Url,
    Notes,
}

/// Main application structure
struct App {
    state: AppState,
    config: AppConfig,
    store: LocalStore,

    // Vault data
    vault: Option<Vault>,
    vault_key: Option<KeyMaterial>,
    entries: Vec<VaultEntry>,
    entry_list_state: ListState,

    // Input fields
    master_password: String,
    input_label: String,
    input_username: String,
    input_password: String,
    input_url: String,
    input_notes: String,
    current_field: InputField,

    // Status
    status_message: Option<String>,
    status_time: Option<Instant>,

    // Clipboard timeout
    clipboard_timeout: Option<Instant>,
    clipboard_value: Option<String>,

    // UI state
    show_password: bool,
    should_quit: bool,
}

impl Drop for App {
    fn drop(&mut self) {
        self.master_password.zeroize();
        self.input_password.zeroize();
        if let Some(ref mut v) = self.clipboard_value {
            v.zeroize();
        }
    }
}

impl App {
    fn new() -> Result<Self> {
        let config = default_config();
        let store = LocalStore::new(&config)?;

        Ok(Self {
            state: AppState::Locked,
            config,
            store,
            vault: None,
            vault_key: None,
            entries: Vec::new(),
            entry_list_state: ListState::default(),
            master_password: String::new(),
            input_label: String::new(),
            input_username: String::new(),
            input_password: String::new(),
            input_url: String::new(),
            input_notes: String::new(),
            current_field: InputField::Password,
            status_message: None,
            status_time: None,
            clipboard_timeout: None,
            clipboard_value: None,
            show_password: false,
            should_quit: false,
        })
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
        self.status_time = Some(Instant::now());
    }

    fn check_status_clear(&mut self) {
        if let Some(time) = self.status_time {
            if time.elapsed() > Duration::from_secs(5) {
                self.status_message = None;
                self.status_time = None;
            }
        }
    }

    fn check_clipboard_clear(&mut self) {
        if let (Some(timeout), Some(value)) = (&self.clipboard_timeout, &self.clipboard_value) {
            if Instant::now() >= *timeout {
                if let Ok(mut clipboard) = Clipboard::new() {
                    if clipboard.get_text().ok().as_deref() == Some(value) {
                        let _ = clipboard.set_text(String::new());
                    }
                }
                self.clipboard_timeout = None;
                self.clipboard_value = None;
            }
        }
    }

    fn copy_to_clipboard(&mut self, value: &str) {
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(value.to_string()).is_ok() {
                self.clipboard_value = Some(value.to_string());
                self.clipboard_timeout = Some(Instant::now() + Duration::from_secs(30));
                self.set_status("Password copied to clipboard (clears in 30s)");
            }
        }
    }

    fn key_path(&self) -> PathBuf {
        PathBuf::from(&self.config.data_dir).join("key.json")
    }

    fn unlock_vault(&mut self) -> Result<()> {
        let password = self.master_password.trim();
        if password.is_empty() {
            return Err(anyhow!("master password is required"));
        }

        let path = self.key_path();
        let (key, key_file) = if path.exists() {
            load_key(&path, Some(password))?
        } else {
            // Create new key from password
            let params = KeyDerivationParams::generate();
            let key = derive_key(password, &params)?;
            let key_file = KeyFile::from_kdf(params);
            save_key(&path, &key_file)?;
            (key, key_file)
        };

        // Load or create vault
        let vault = if let Ok(v) = self.store.load_vault("primary", &key) {
            v
        } else {
            let metadata = match &key_file {
                KeyFile::Kdf { .. } => {
                    KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305).with_kdf("argon2id")
                }
                KeyFile::Raw { .. } => KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305),
            };
            let v = Vault::new("primary", metadata);
            self.store.save_vault(&v, &key)?;
            v
        };

        self.entries = self.load_entries(&vault, &key)?;
        self.vault = Some(vault);
        self.vault_key = Some(key);
        self.state = AppState::Unlocked;
        self.entry_list_state.select(if self.entries.is_empty() {
            None
        } else {
            Some(0)
        });

        Ok(())
    }

    fn load_entries(&self, vault: &Vault, key: &KeyMaterial) -> Result<Vec<VaultEntry>> {
        let mut entries = Vec::with_capacity(vault.entries.len());

        for entry in &vault.entries {
            let plaintext = decrypt(key, &entry.ciphertext)?;
            let payload: EntryPayload = serde_json::from_slice(&plaintext).unwrap_or_else(|_| {
                EntryPayload {
                    password: String::from_utf8_lossy(&plaintext).to_string(),
                    username: String::new(),
                    url: String::new(),
                    notes: String::new(),
                }
            });

            entries.push(VaultEntry {
                label: entry.label.clone(),
                username: payload.username,
                password: payload.password,
                url: payload.url,
                notes: payload.notes,
                updated_at: entry.updated_at,
            });
        }

        entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(entries)
    }

    fn add_entry(&mut self) -> Result<()> {
        let label = self.input_label.trim();
        if label.is_empty() {
            return Err(anyhow!("label is required"));
        }
        let password = self.input_password.trim();
        if password.is_empty() {
            return Err(anyhow!("password is required"));
        }

        let key = self
            .vault_key
            .as_ref()
            .ok_or_else(|| anyhow!("vault not unlocked"))?
            .clone();

        let payload = EntryPayload {
            password: password.to_string(),
            username: self.input_username.trim().to_string(),
            url: self.input_url.trim().to_string(),
            notes: self.input_notes.trim().to_string(),
        };

        let serialized = serde_json::to_vec(&payload)?;
        let ciphertext = encrypt(&key, &serialized)?;

        {
            let vault = self
                .vault
                .as_mut()
                .ok_or_else(|| anyhow!("vault not loaded"))?;

            if vault.find_entry(label).is_some() {
                vault.update_entry(label, ciphertext)?;
            } else {
                vault.add_entry(lilypad_core::Entry::new(label, ciphertext))?;
            }

            self.store.save_vault(vault, &key)?;
        }

        // Reload entries after the mutable borrow is released
        let vault = self
            .vault
            .as_ref()
            .ok_or_else(|| anyhow!("vault not loaded"))?;
        self.entries = self.load_entries(vault, &key)?;

        // Clear input fields
        self.input_label.clear();
        self.input_username.clear();
        self.input_password.clear();
        self.input_url.clear();
        self.input_notes.clear();

        self.state = AppState::Unlocked;
        self.set_status("Entry saved");
        Ok(())
    }

    fn delete_selected_entry(&mut self) -> Result<()> {
        let index = match self.entry_list_state.selected() {
            Some(i) => i,
            None => return Ok(()),
        };

        let label = self.entries.get(index).map(|e| e.label.clone());
        if let Some(label) = label {
            let key = self
                .vault_key
                .as_ref()
                .ok_or_else(|| anyhow!("vault not unlocked"))?
                .clone();

            {
                let vault = self
                    .vault
                    .as_mut()
                    .ok_or_else(|| anyhow!("vault not loaded"))?;

                vault.remove_entry(&label)?;
                self.store.save_vault(vault, &key)?;
            }

            // Reload entries after the mutable borrow is released
            let vault = self
                .vault
                .as_ref()
                .ok_or_else(|| anyhow!("vault not loaded"))?;
            self.entries = self.load_entries(vault, &key)?;

            // Update selection
            if self.entries.is_empty() {
                self.entry_list_state.select(None);
            } else if index >= self.entries.len() {
                self.entry_list_state.select(Some(self.entries.len() - 1));
            }

            self.set_status(format!("Deleted '{}'", label));
        }

        Ok(())
    }

    fn handle_key_event(&mut self, key: event::KeyEvent) {
        match self.state {
            AppState::Locked => self.handle_locked_input(key),
            AppState::Unlocked => self.handle_unlocked_input(key),
            AppState::AddEntry => self.handle_add_entry_input(key),
            AppState::ViewEntry => self.handle_view_entry_input(key),
        }
    }

    fn handle_locked_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Enter => {
                if let Err(e) = self.unlock_vault() {
                    self.set_status(format!("Unlock failed: {}", e));
                    self.master_password.clear();
                }
            }
            KeyCode::Char(c) => {
                self.master_password.push(c);
            }
            KeyCode::Backspace => {
                self.master_password.pop();
            }
            _ => {}
        }
    }

    fn handle_unlocked_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('a') => {
                self.state = AppState::AddEntry;
                self.current_field = InputField::Label;
            }
            KeyCode::Char('d') => {
                if let Err(e) = self.delete_selected_entry() {
                    self.set_status(format!("Delete failed: {}", e));
                }
            }
            KeyCode::Char('c') => {
                if let Some(index) = self.entry_list_state.selected() {
                    let password = self.entries.get(index).map(|e| e.password.clone());
                    if let Some(password) = password {
                        self.copy_to_clipboard(&password);
                    }
                }
            }
            KeyCode::Enter => {
                if self.entry_list_state.selected().is_some() {
                    self.state = AppState::ViewEntry;
                    self.show_password = false;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.select_previous();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.select_next();
            }
            _ => {}
        }
    }

    fn handle_view_entry_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state = AppState::Unlocked;
            }
            KeyCode::Char('p') => {
                self.show_password = !self.show_password;
            }
            KeyCode::Char('c') => {
                if let Some(index) = self.entry_list_state.selected() {
                    let password = self.entries.get(index).map(|e| e.password.clone());
                    if let Some(password) = password {
                        self.copy_to_clipboard(&password);
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_add_entry_input(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input_label.clear();
                self.input_username.clear();
                self.input_password.clear();
                self.input_url.clear();
                self.input_notes.clear();
                self.state = AppState::Unlocked;
            }
            KeyCode::Tab => {
                self.current_field = match self.current_field {
                    InputField::Label => InputField::Username,
                    InputField::Username => InputField::EntryPassword,
                    InputField::EntryPassword => InputField::Url,
                    InputField::Url => InputField::Notes,
                    InputField::Notes => InputField::Label,
                    _ => InputField::Label,
                };
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Err(e) = self.add_entry() {
                    self.set_status(format!("Save failed: {}", e));
                }
            }
            KeyCode::Char(c) => {
                let field = match self.current_field {
                    InputField::Label => &mut self.input_label,
                    InputField::Username => &mut self.input_username,
                    InputField::EntryPassword => &mut self.input_password,
                    InputField::Url => &mut self.input_url,
                    InputField::Notes => &mut self.input_notes,
                    _ => return,
                };
                field.push(c);
            }
            KeyCode::Backspace => {
                let field = match self.current_field {
                    InputField::Label => &mut self.input_label,
                    InputField::Username => &mut self.input_username,
                    InputField::EntryPassword => &mut self.input_password,
                    InputField::Url => &mut self.input_url,
                    InputField::Notes => &mut self.input_notes,
                    _ => return,
                };
                field.pop();
            }
            _ => {}
        }
    }

    fn select_next(&mut self) {
        let i = match self.entry_list_state.selected() {
            Some(i) => {
                if i >= self.entries.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        if !self.entries.is_empty() {
            self.entry_list_state.select(Some(i));
        }
    }

    fn select_previous(&mut self) {
        let i = match self.entry_list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.entries.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        if !self.entries.is_empty() {
            self.entry_list_state.select(Some(i));
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        match self.state {
            AppState::Locked => self.draw_locked_screen(frame),
            AppState::Unlocked => self.draw_unlocked_screen(frame),
            AppState::AddEntry => self.draw_add_entry_screen(frame),
            AppState::ViewEntry => self.draw_view_entry_screen(frame),
        }
    }

    fn draw_locked_screen(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        let title = Paragraph::new("Lilypad Password Manager")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default());
        frame.render_widget(title, chunks[0]);

        let password_display = "*".repeat(self.master_password.len());
        let password_input = Paragraph::new(password_display)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Master Password"),
            )
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(password_input, chunks[1]);

        let help = Paragraph::new("Enter: Unlock | Esc: Quit")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(help, chunks[2]);

        if let Some(ref msg) = self.status_message {
            let status = Paragraph::new(msg.as_str()).style(Style::default().fg(Color::Red));
            frame.render_widget(status, chunks[3]);
        }
    }

    fn draw_unlocked_screen(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);

        let title = Paragraph::new("Lilypad Vault")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::BOTTOM));
        frame.render_widget(title, chunks[0]);

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|entry| {
                let username = if entry.username.is_empty() {
                    "(no username)"
                } else {
                    &entry.username
                };
                let updated = format_timestamp_relative(entry.updated_at);
                ListItem::new(Line::from(vec![
                    Span::styled(&entry.label, Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" - "),
                    Span::styled(username, Style::default().fg(Color::Gray)),
                    Span::raw(" ("),
                    Span::styled(updated, Style::default().fg(Color::DarkGray)),
                    Span::raw(")"),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Entries"))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, chunks[1], &mut self.entry_list_state);

        let help_text = "a: Add | d: Delete | c: Copy Password | Enter: View | q: Quit";
        let status_text = self.status_message.as_deref().unwrap_or(help_text);
        let help = Paragraph::new(status_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::TOP));
        frame.render_widget(help, chunks[2]);
    }

    fn draw_add_entry_screen(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        let title = Paragraph::new("Add New Entry")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        frame.render_widget(title, chunks[0]);

        let fields = [
            ("Label", &self.input_label, InputField::Label),
            ("Username", &self.input_username, InputField::Username),
            ("Password", &self.input_password, InputField::EntryPassword),
            ("URL", &self.input_url, InputField::Url),
            ("Notes", &self.input_notes, InputField::Notes),
        ];

        for (i, (name, value, field)) in fields.iter().enumerate() {
            let style = if *field == self.current_field {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let display_value = if *field == InputField::EntryPassword {
                "*".repeat(value.len())
            } else {
                (*value).clone()
            };

            let input = Paragraph::new(display_value)
                .block(Block::default().borders(Borders::ALL).title(*name))
                .style(style);
            frame.render_widget(input, chunks[i + 1]);
        }

        let help = Paragraph::new("Tab: Next Field | Ctrl+Enter: Save | Esc: Cancel")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(help, chunks[6]);
    }

    fn draw_view_entry_screen(&self, frame: &mut Frame) {
        let area = frame.area();

        let entry = match self.entry_list_state.selected() {
            Some(i) => match self.entries.get(i) {
                Some(e) => e,
                None => return,
            },
            None => return,
        };

        let popup_area = centered_rect(60, 50, area);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", entry.label))
            .style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let lines = vec![
            Line::from(vec![
                Span::styled("Username: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(if entry.username.is_empty() {
                    "(none)"
                } else {
                    &entry.username
                }),
            ]),
            Line::from(vec![
                Span::styled("Password: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(if self.show_password {
                    &entry.password
                } else {
                    "********"
                }),
            ]),
            Line::from(vec![
                Span::styled("URL: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(if entry.url.is_empty() {
                    "(none)"
                } else {
                    &entry.url
                }),
            ]),
            Line::from(vec![
                Span::styled("Notes: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(if entry.notes.is_empty() {
                    "(none)"
                } else {
                    &entry.notes
                }),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Updated: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format_timestamp_relative(entry.updated_at)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "p: Toggle Password | c: Copy | Esc: Close",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new()?;

    // Main loop
    loop {
        // Check timers
        app.check_status_clear();
        app.check_clipboard_clear();

        // Draw
        terminal.draw(|f| app.draw(f))?;

        // Handle events with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key_event(key);
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
