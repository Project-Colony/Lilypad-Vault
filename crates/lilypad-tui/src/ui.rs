//! Rendering of every screen.

use crate::app::{App, FormField, Screen};
use lilypad_core::i18n::{t, tf};
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
        let hint = t(match self.screen {
            Screen::Locked => "tui_hint_locked",
            Screen::CreateVault => "tui_hint_create",
            Screen::Unlocked if self.searching => "tui_hint_search",
            Screen::Unlocked => "tui_hint_list",
            Screen::View => "tui_hint_view",
            Screen::Form => "tui_hint_form",
            Screen::Generate => "tui_hint_generate",
            Screen::Help => "tui_hint_help",
            Screen::ConfirmDelete => "tui_hint_confirm",
            Screen::Trash => "tui_hint_trash",
            Screen::ConfirmPurge => "tui_hint_confirm_purge",
        });
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
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", t("sidebar_vaults"))),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, cols[0], &mut self.vault_list);

        let masked: String = "•".repeat(self.master_password.chars().count());
        let pw = Paragraph::new(format!(
            "\n  {}:\n  {masked}",
            t("unlock_password_placeholder")
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", t("unlock_button"))),
        );
        frame.render_widget(pw, cols[1]);
    }

    pub fn draw_create(&mut self, frame: &mut Frame, area: Rect) {
        let masked: String = "•".repeat(self.new_vault_password.chars().count());
        let name_marker = if self.create_focus_name { "> " } else { "  " };
        let pw_marker = if self.create_focus_name { "  " } else { "> " };
        let body = format!(
            "\n {name_marker}{}: {}\n\n {pw_marker}{}: {masked}\n\n  {}",
            t("create_name_placeholder"),
            self.new_vault_name,
            t("unlock_password_placeholder"),
            t("tui_create_help"),
        );
        frame.render_widget(
            Paragraph::new(body)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" {} ", t("create_title"))),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    pub fn draw_list(&mut self, frame: &mut Frame, area: Rect) {
        let title = if self.search_query.is_empty() {
            tf("tui_entries", &[("count", &self.entries.len())])
        } else {
            tf(
                "tui_search",
                &[
                    ("query", &self.search_query),
                    ("count", &self.entries.len()),
                ],
            )
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
        let title = tf("tui_trash", &[("count", &self.trashed.len())]);
        let items: Vec<ListItem> = if self.trashed.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                t("tui_empty"),
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
                lines.push(field_line("field_username", u));
            }
            if let Some(u) = &v.url {
                lines.push(field_line("field_url", u));
            }
            if !v.tags.is_empty() {
                lines.push(field_line("field_tags", &v.tags.join(", ")));
            }
        }
        if let Some(secret) = &self.view_reveal {
            let pw = if self.view_show_password {
                secret.password.clone()
            } else {
                "•".repeat(secret.password.chars().count().max(8))
            };
            lines.push(field_line("field_password", &pw));
            if let Some(n) = &secret.notes {
                lines.push(field_line("field_notes", n));
            }
            if secret.totp_secret.is_some() {
                lines.push(field_line("field_totp", t("tui_totp_configured")));
            }
            if !secret.attachments.is_empty() {
                lines.push(field_line(
                    "tui_attachments",
                    &secret.attachments.len().to_string(),
                ));
            }
        }
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" {} ", t("tui_entry"))),
                )
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
                format!("{marker}{:<20} {value}", format!("{}:", field.label())),
                style,
            )));
        }
        let title = format!(
            " {} ",
            if self.form.editing_original.is_some() {
                t("form_edit_title")
            } else {
                t("form_new_title")
            }
        );
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
            "\n {}\n\n {} A-Z (U)   {} a-z (L)\n {} 0-9 (D)   {} !@# (S)\n\n {}:\n  {}\n",
            tf("generator_length", &[("length", &self.gen_length)]),
            toggle(self.gen_upper),
            toggle(self.gen_lower),
            toggle(self.gen_digits),
            toggle(self.gen_symbols),
            t("tui_result"),
            if self.gen_result.is_empty() {
                t("tui_press_r")
            } else {
                &self.gen_result
            },
        );
        frame.render_widget(
            Paragraph::new(body).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", t("generator_title"))),
            ),
            area,
        );
    }

    pub fn draw_confirm(&self, frame: &mut Frame) {
        let area = centered_rect(50, 20, frame.area());
        frame.render_widget(Clear, area);
        let label = self.selected_entry_label().unwrap_or_default();
        frame.render_widget(
            Paragraph::new(format!(
                "\n {}\n\n {}",
                tf("confirm_trash_body", &[("label", &label)]),
                t("tui_yes_no")
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", t("confirm_trash_title"))),
            ),
            area,
        );
    }

    pub fn draw_confirm_purge(&self, frame: &mut Frame) {
        let area = centered_rect(55, 22, frame.area());
        frame.render_widget(Clear, area);
        let label = self.selected_trash_label().unwrap_or_default();
        frame.render_widget(
            Paragraph::new(format!(
                "\n {}\n\n {}",
                tf("confirm_delete_body", &[("label", &label)]),
                t("tui_yes_no")
            ))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", t("confirm_delete_title"))),
            ),
            area,
        );
    }

    pub fn draw_help(&self, frame: &mut Frame) {
        let area = centered_rect(60, 70, frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(t("tui_help")).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", t("tui_help_title"))),
            ),
            area,
        );
    }
}

/// One `Label: value` line of the entry view.
fn field_line(key: &str, value: &str) -> Line<'static> {
    Line::from(format!("{:<20} {value}", format!("{}:", t(key))))
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
