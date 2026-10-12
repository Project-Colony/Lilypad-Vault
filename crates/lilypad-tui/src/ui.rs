//! Rendering of every screen.

use crate::app::{App, FormField, Screen};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

impl App {
    pub fn draw(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(frame.area());
        self.draw_title(frame, chunks[0]);
        match self.screen {
            Screen::Locked => self.draw_locked(frame, chunks[1]),
            Screen::CreateVault => self.draw_create(frame, chunks[1]),
            Screen::Trash | Screen::ConfirmPurge => self.draw_trash(frame, chunks[1]),
            Screen::Unlocked
            | Screen::ConfirmDelete
            | Screen::View
            | Screen::Form
            | Screen::Generate
            | Screen::Help => self.draw_list(frame, chunks[1]),
        }
        self.draw_status(frame, chunks[2]);

        // Overlays
        match self.screen {
            Screen::View => self.draw_view(frame),
            Screen::Form => self.draw_form(frame),
            Screen::Generate => self.draw_generate(frame),
            Screen::Help => self.draw_help(frame),
            Screen::ConfirmDelete => self.draw_confirm(frame),
            Screen::ConfirmPurge => self.draw_confirm_purge(frame),
            _ => {}
        }
    }

    pub fn draw_title(&self, frame: &mut Frame, area: Rect) {
        let title = match &self.session {
            Some(s) => format!(" Lilypad - {} ", s.name()),
            None => " Lilypad ".to_string(),
        };
        frame.render_widget(
            Paragraph::new(title).style(Style::default().fg(Color::Black).bg(Color::Green)),
            area,
        );
    }

    pub fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let hint = match self.screen {
            Screen::Locked => "type password · ↑↓ vault · Enter unlock · ^N new vault · Esc quit",
            Screen::CreateVault => "Tab switch field · Enter create · Esc back",
            Screen::Unlocked if self.searching => "type to filter · Enter done · Esc clear",
            Screen::Unlocked => "a add · e edit · d del · Enter view · c/u/t copy · / search · g gen · T trash · l lock · ? help · q quit",
            Screen::View => "p show/hide · c copy pw · u copy user · t copy totp · Esc back",
            Screen::Form => "Tab next · Ctrl+Enter save · Esc cancel",
            Screen::Generate => "r regen · +/- length · U/L/D/S toggle sets · c copy · Esc back",
            Screen::Help => "Esc close",
            Screen::ConfirmDelete => "y confirm · any other key cancel",
            Screen::Trash => "r restore · x delete forever · ↑↓ move · Esc back",
            Screen::ConfirmPurge => "y delete FOREVER · any other key cancel",
        };
        let text = self.status.clone().unwrap_or_else(|| hint.to_string());
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(Color::DarkGray)),
            area,
        );
    }

    pub fn draw_locked(&mut self, frame: &mut Frame, area: Rect) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        let items: Vec<ListItem> = self
            .vaults
            .iter()
            .map(|v| ListItem::new(v.clone()))
            .collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Vaults "))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, cols[0], &mut self.vault_list);

        let masked: String = "•".repeat(self.master_password.chars().count());
        let pw = Paragraph::new(format!("\n  Master password:\n  {masked}"))
            .block(Block::default().borders(Borders::ALL).title(" Unlock "));
        frame.render_widget(pw, cols[1]);
    }

    pub fn draw_create(&mut self, frame: &mut Frame, area: Rect) {
        let masked: String = "•".repeat(self.new_vault_password.chars().count());
        let name_marker = if self.create_focus_name { "> " } else { "  " };
        let pw_marker = if self.create_focus_name { "  " } else { "> " };
        let body = format!(
            "\n {name_marker}Vault name:      {}\n\n {pw_marker}Master password: {masked}\n\n  Tab to switch field, Enter to create.",
            self.new_vault_name
        );
        frame.render_widget(
            Paragraph::new(body)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Create vault "),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    pub fn draw_list(&mut self, frame: &mut Frame, area: Rect) {
        let title = if self.search_query.is_empty() {
            format!(" Entries ({}) ", self.entries.len())
        } else {
            format!(" Search: {} ({}) ", self.search_query, self.entries.len())
        };
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|e| {
                let mut spans = vec![Span::raw(e.label.clone())];
                if let Some(u) = &e.username {
                    spans.push(Span::styled(
                        format!("  {u}"),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                ListItem::new(Line::from(spans))
            })
            .collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut self.entry_list);
    }

    pub fn draw_trash(&mut self, frame: &mut Frame, area: Rect) {
        let title = format!(" Trash ({}) ", self.trashed.len());
        let items: Vec<ListItem> = if self.trashed.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                "  (empty)",
                Style::default().fg(Color::DarkGray),
            )))]
        } else {
            self.trashed
                .iter()
                .map(|e| {
                    let mut spans = vec![Span::raw(e.label.clone())];
                    if let Some(u) = &e.username {
                        spans.push(Span::styled(
                            format!("  {u}"),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect()
        };
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut self.trash_list);
    }

    pub fn draw_view(&self, frame: &mut Frame) {
        let area = centered_rect(70, 60, frame.area());
        frame.render_widget(Clear, area);
        let view = self.entries.iter().find(|e| e.label == self.view_label);
        let mut lines = vec![Line::from(Span::styled(
            self.view_label.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ))];
        lines.push(Line::from(""));
        if let Some(v) = view {
            if let Some(u) = &v.username {
                lines.push(Line::from(format!("Username: {u}")));
            }
            if let Some(u) = &v.url {
                lines.push(Line::from(format!("URL:      {u}")));
            }
            if !v.tags.is_empty() {
                lines.push(Line::from(format!("Tags:     {}", v.tags.join(", "))));
            }
        }
        if let Some(secret) = &self.view_reveal {
            let pw = if self.view_show_password {
                secret.password.clone()
            } else {
                "•".repeat(secret.password.chars().count().max(8))
            };
            lines.push(Line::from(format!("Password: {pw}")));
            if let Some(n) = &secret.notes {
                lines.push(Line::from(format!("Notes:    {n}")));
            }
            if secret.totp_secret.is_some() {
                lines.push(Line::from("TOTP:     configured (t to copy)"));
            }
            if !secret.attachments.is_empty() {
                lines.push(Line::from(format!(
                    "Attachments: {}",
                    secret.attachments.len()
                )));
            }
        }
        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title(" Entry "))
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    pub fn draw_form(&self, frame: &mut Frame) {
        let area = centered_rect(70, 70, frame.area());
        frame.render_widget(Clear, area);
        let focus = self.form_focus();
        let mut lines = Vec::new();
        for field in FormField::ORDER {
            let value = match field {
                FormField::Label => self.form.label.clone(),
                FormField::Username => self.form.username.clone(),
                FormField::Url => self.form.url.clone(),
                FormField::Password => "•".repeat(self.form.password.chars().count()),
                FormField::Notes => self.form.notes.clone(),
                FormField::Totp => self.form.totp.clone(),
            };
            let marker = if field == focus { "> " } else { "  " };
            let style = if field == focus {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("{marker}{:<13} {value}", format!("{}:", field.label())),
                style,
            )));
        }
        let title = if self.form.editing_original.is_some() {
            " Edit entry "
        } else {
            " Add entry "
        };
        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title(title))
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    pub fn draw_generate(&self, frame: &mut Frame) {
        let area = centered_rect(60, 50, frame.area());
        frame.render_widget(Clear, area);
        let toggle = |b: bool| if b { "[x]" } else { "[ ]" };
        let body = format!(
            "\n Length: {}\n\n {} Upper (U)   {} Lower (L)\n {} Digits (D)  {} Symbols (S)\n\n Result:\n  {}\n",
            self.gen_length,
            toggle(self.gen_upper),
            toggle(self.gen_lower),
            toggle(self.gen_digits),
            toggle(self.gen_symbols),
            if self.gen_result.is_empty() { "(press r)" } else { &self.gen_result },
        );
        frame.render_widget(
            Paragraph::new(body).block(Block::default().borders(Borders::ALL).title(" Generate ")),
            area,
        );
    }

    pub fn draw_confirm(&self, frame: &mut Frame) {
        let area = centered_rect(50, 20, frame.area());
        frame.render_widget(Clear, area);
        let label = self.selected_entry_label().unwrap_or_default();
        frame.render_widget(
            Paragraph::new(format!(
                "\n Delete entry '{label}'?\n\n y = yes, any other key = no"
            ))
            .block(Block::default().borders(Borders::ALL).title(" Confirm ")),
            area,
        );
    }

    pub fn draw_confirm_purge(&self, frame: &mut Frame) {
        let area = centered_rect(55, 22, frame.area());
        frame.render_widget(Clear, area);
        let label = self.selected_trash_label().unwrap_or_default();
        frame.render_widget(
            Paragraph::new(format!(
                "\n Permanently delete '{label}'?\n This cannot be undone.\n\n y = yes, any other key = no"
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Delete forever "),
            ),
            area,
        );
    }

    pub fn draw_help(&self, frame: &mut Frame) {
        let area = centered_rect(60, 70, frame.area());
        frame.render_widget(Clear, area);
        let body = "\n Navigation\n  j/k or ↑/↓   move\n  Enter or v   view entry\n  /            search\n\n Actions\n  a  add entry     e  edit entry\n  d  to Trash      T  open Trash\n  g  generate      l  lock vault\n  c  copy password u  copy username\n  t  copy TOTP\n\n Trash\n  r  restore       x  delete forever\n\n  q / Esc  quit\n";
        frame.render_widget(
            Paragraph::new(body).block(Block::default().borders(Borders::ALL).title(" Help ")),
            area,
        );
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
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
        .split(vertical[1])[1]
}
