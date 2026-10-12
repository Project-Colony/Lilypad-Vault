//! The view layer: the window shell, shared widgets and helpers.

mod entry;
mod unlock;
mod vault;

use crate::fonts;
use crate::message::Message;
use crate::state::{LilypadApp, Overlay, Screen};
use crate::theme::{self};
use iced::widget::{button, canvas, container, row, stack, text, Space};
use iced::{Alignment, Background, Color, Element, Length, Radians};
use lilypad_core::EntryType;
use std::time::Instant;

impl LilypadApp {
    pub fn view(&self) -> Element<'_, Message> {
        // The settings page is a full-screen replacement, reachable while locked
        // or unlocked; it takes precedence over the normal screen.
        let base: Element<Message> = if self.settings_open {
            self.view_settings()
        } else {
            match self.screen {
                Screen::Unlock => self.view_unlock(),
                Screen::Create => self.view_create(),
                Screen::Vault => self.view_vault(),
            }
        };

        let mut content: Element<Message> = match &self.overlay {
            Overlay::None => base,
            Overlay::Generator => stack![base, self.modal(self.view_generator())].into(),
            Overlay::AddForm => stack![base, self.modal(self.view_add_form())].into(),
            Overlay::ConfirmDelete(label) => {
                stack![base, self.modal(self.view_confirm(label))].into()
            }
        };

        if let Some(at) = self.clipboard_clear_at {
            let left = at.saturating_duration_since(Instant::now()).as_secs();
            content = stack![content, self.copy_toast(left)].into();
        }

        let (t, d) = (self.theme, self.density);
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| theme::app_container(t, d))
            .into()
    }

    pub fn icon(&self, glyph: &str, size: f32, color: Color) -> Element<'static, Message> {
        // Nerd Font glyphs have an asymmetric side-bearing (they sit right of
        // center in their advance box). Compensate with a right nudge so icons
        // look centered - especially in icon-only buttons.
        fonts::centered_icon_colored(glyph, size, color)
    }

    // -- Unlock / Create -----------------------------------------------------

    pub fn copy_toast(&self, left: u64) -> Element<'static, Message> {
        let (t, d) = (self.theme, self.density);
        let p = self.theme.palette();
        let card = container(
            row![
                self.icon(fonts::icons::CIRCLE_CHECK, 13.0, p.success),
                hgap(8.0),
                text(format!("Copied · clears in {left}s"))
                    .size(12)
                    .color(p.text_secondary),
                hgap(10.0),
                button(text("Undo").size(12))
                    .on_press(Message::ClipboardCancel)
                    .style(theme::ghost_style(t, d)),
            ]
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .style(move |_| theme::modal_container(t, d));
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(18)
            .into()
    }

    pub fn status_inline(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        match &self.status {
            Some(msg) => text(msg.clone()).size(13).color(p.text_secondary).into(),
            None => vgap(0.0).into(),
        }
    }

    pub fn centered<'a>(&self, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let (t, d) = (self.theme, self.density);
        let card = container(content)
            .padding(28)
            .width(Length::Fill)
            .max_width(440)
            .style(move |_| theme::card_container(t, d));
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    }

    pub fn modal<'a>(&self, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let (t, d) = (self.theme, self.density);
        let card = container(content)
            .padding(24)
            .max_width(560)
            .style(move |_| theme::modal_container(t, d));
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.55,
                })),
                ..Default::default()
            })
            .into()
    }
}

/// All entry types, in picker order (labels via `type_label`).
pub const ALL_TYPES: [EntryType; 8] = [
    EntryType::Login,
    EntryType::Card,
    EntryType::Identity,
    EntryType::SecureNote,
    EntryType::SoftwareLicense,
    EntryType::Wifi,
    EntryType::Server,
    EntryType::Custom,
];

/// Expiry picker choices (label, value). None = keep the current setting.
pub const EXPIRY_CHOICES: [(&str, Option<u32>); 7] = [
    ("Keep current", None),
    ("Never", Some(0)),
    ("30 days", Some(30)),
    ("60 days", Some(60)),
    ("90 days", Some(90)),
    ("180 days", Some(180)),
    ("1 year", Some(365)),
];

pub fn expiry_label_for(days: Option<u32>) -> &'static str {
    EXPIRY_CHOICES
        .iter()
        .find(|(_, v)| *v == days)
        .map(|(l, _)| *l)
        .unwrap_or("Keep current")
}

/// The iced color for an entry's color label (mirrors EntryColor::to_hex).
pub fn entry_color(c: lilypad_core::EntryColor) -> Color {
    use lilypad_core::EntryColor as C;
    match c {
        C::Red => Color::from_rgb8(0xEF, 0x44, 0x44),
        C::Orange => Color::from_rgb8(0xF9, 0x73, 0x16),
        C::Yellow => Color::from_rgb8(0xEA, 0xB3, 0x08),
        C::Green => Color::from_rgb8(0x22, 0xC5, 0x5E),
        C::Blue => Color::from_rgb8(0x3B, 0x82, 0xF6),
        C::Purple => Color::from_rgb8(0xA8, 0x55, 0xF7),
        C::Pink => Color::from_rgb8(0xEC, 0x48, 0x99),
        C::Gray => Color::from_rgb8(0x6B, 0x72, 0x80),
    }
}

/// A circular countdown ring (used for the TOTP timer).
pub struct Ring {
    fraction: f32,
    color: Color,
    track: Color,
}

impl canvas::Program<Message> for Ring {
    type State = ();
    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let radius = bounds.width.min(bounds.height) / 2.0 - 2.5;
        frame.stroke(
            &canvas::Path::circle(center, radius),
            canvas::Stroke::default()
                .with_width(3.0)
                .with_color(self.track),
        );
        let start = -std::f32::consts::FRAC_PI_2;
        let end = start + self.fraction.clamp(0.0, 1.0) * std::f32::consts::TAU;
        let arc = canvas::Path::new(|b| {
            b.arc(canvas::path::Arc {
                center,
                radius,
                start_angle: Radians(start),
                end_angle: Radians(end),
            });
        });
        frame.stroke(
            &arc,
            canvas::Stroke::default()
                .with_width(3.0)
                .with_color(self.color),
        );
        vec![frame.into_geometry()]
    }
}

pub fn grade_letter(g: lilypad_core::HealthGrade) -> &'static str {
    use lilypad_core::HealthGrade as G;
    match g {
        G::A => "A",
        G::B => "B",
        G::C => "C",
        G::D => "D",
        G::F => "F",
    }
}

/// Grade letter with a +/- refinement from where the score sits in its band
/// (e.g. score 88 in band B -> "B+"). F has no modifier.
pub fn grade_display(score: u8, g: lilypad_core::HealthGrade) -> String {
    use lilypad_core::HealthGrade as G;
    let letter = grade_letter(g);
    if matches!(g, G::F) {
        return letter.to_string();
    }
    let floor = match g {
        G::A => 90,
        G::B => 80,
        G::C => 70,
        G::D => 60,
        G::F => 0,
    };
    let within = score.saturating_sub(floor);
    let suffix = if within >= 7 {
        "+"
    } else if within <= 2 {
        "-"
    } else {
        ""
    };
    format!("{letter}{suffix}")
}

/// A translucent tint of a color, for banners.
pub fn tint(c: Color, alpha: f32) -> Color {
    Color { a: alpha, ..c }
}

/// Maps a 1..=5 strength level to a representative 0-100 score, because
/// `theme::strength_color` buckets on a 0-100 score (not the level).
pub fn strength_score(level: u8) -> u8 {
    match level {
        1 => 10,
        2 => 35,
        3 => 55,
        4 => 75,
        5 => 95,
        _ => 0,
    }
}

/// Password strength as a 1..=5 level (higher = stronger).
pub fn strength_level(pw: &str) -> u8 {
    use lilypad_core::PasswordStrength as S;
    match lilypad_core::validation::validate_password_strength(pw) {
        S::VeryWeak => 1,
        S::Weak => 2,
        S::Fair => 3,
        S::Strong => 4,
        S::VeryStrong => 5,
    }
}

pub fn type_icon(t: &EntryType) -> &'static str {
    use fonts::icons::*;
    match t {
        EntryType::Login => KEY,
        EntryType::Card => CREDIT_CARD,
        EntryType::Identity => ID_CARD,
        EntryType::SecureNote => STICKY_NOTE,
        EntryType::SoftwareLicense => CERTIFICATE,
        EntryType::Wifi => WIFI,
        EntryType::Server => SERVER,
        EntryType::Custom => LEAF,
    }
}

pub fn type_label(t: &EntryType) -> &'static str {
    match t {
        EntryType::Login => "Logins",
        EntryType::Card => "Cards",
        EntryType::Identity => "Identities",
        EntryType::SecureNote => "Notes",
        EntryType::SoftwareLicense => "Licenses",
        EntryType::Wifi => "Wifi",
        EntryType::Server => "Servers",
        EntryType::Custom => "Custom",
    }
}

pub fn vgap(px: f32) -> Space {
    Space::new().height(Length::Fixed(px))
}

pub fn hgap(px: f32) -> Space {
    Space::new().width(Length::Fixed(px))
}

pub fn hfill() -> Space {
    Space::new().width(Length::Fill)
}

pub type BtnStyle = Box<dyn Fn(&iced::Theme, button::Status) -> button::Style>;

pub fn boxed_btn(f: impl Fn(&iced::Theme, button::Status) -> button::Style + 'static) -> BtnStyle {
    Box::new(f)
}

// Async key derivation -------------------------------------------------------
