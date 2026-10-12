//! The view layer: the window shell, shared widgets and helpers.
//!
//! The sidebar is always there; the content area to its right shows the
//! current screen, or Preferences while it is open.

mod entry;
mod sidebar;
mod unlock;
mod vault;

pub use entry::severity_color;

use crate::icons;
use crate::message::Message;
use crate::state::{LilypadApp, Overlay, Screen};
use crate::theme;
use colony_ui::theme::Palette;
use colony_ui::{sz, widgets, Typography};
use iced::widget::{button, canvas, column, container, row, stack, text, Space};
use iced::{Alignment, Color, Element, Length, Radians};
use lilypad_core::i18n::{t, tf};
use lilypad_core::EntryType;
use std::time::Instant;

impl LilypadApp {
    pub fn view(&self) -> Element<'_, Message> {
        let content: Element<Message> = if self.preferences_open {
            self.view_preferences()
        } else {
            match self.screen {
                Screen::Unlock => self.view_unlock(),
                Screen::Create => self.view_create(),
                Screen::Vault => self.view_vault(),
            }
        };

        let mut content: Element<Message> = match &self.overlay {
            Overlay::None => content,
            Overlay::Generator => stack![content, self.modal(self.view_generator())].into(),
            Overlay::AddForm => stack![content, self.modal(self.view_add_form())].into(),
            Overlay::ConfirmDelete(label) => {
                stack![content, self.modal(self.view_confirm(label))].into()
            }
        };

        let notices = self.notices();
        if let Some(notices) = notices {
            content = stack![content, notices].into();
        }
        if let Some(at) = self.clipboard_clear_at {
            let left = at.saturating_duration_since(Instant::now()).as_secs();
            content = stack![content, self.copy_toast(left)].into();
        }

        let d = self.density;
        container(row![self.sidebar(), content])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| theme::app_container(d))
            .into()
    }

    /// The dismissible notices at the bottom of the content: "Theme
    /// applied." and the last status message.
    fn notices(&self) -> Option<Element<'_, Message>> {
        let typo = Typography::current();
        let mut col = column![].spacing(8).align_x(Alignment::Center);
        let mut any = false;
        if let Some(notice) = self.notice {
            col = col.push(widgets::toast(&typo, notice, Message::DismissNotice));
            any = true;
        }
        if let Some(status) = &self.status {
            col = col.push(self.status_toast(status));
            any = true;
        }
        any.then(|| {
            container(col)
                .padding(16)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Bottom)
                .into()
        })
    }

    /// The last status message, neutral (it may be an error): click to
    /// dismiss.
    fn status_toast(&self, status: &str) -> Element<'static, Message> {
        let d = self.density;
        button(
            row![
                text(status.to_string())
                    .size(sz(13))
                    .color(Palette::TEXT_PRIMARY()),
                hgap(10.0),
                text(widgets::icons::CLOSE)
                    .size(sz(12))
                    .font(colony_ui::fonts::glyph_font())
                    .color(Palette::TEXT_DIMMER()),
            ]
            .align_y(Alignment::Center),
        )
        .on_press(Message::Dismiss)
        .padding([8, 16])
        .style(move |_, _| {
            let card = theme::modal_container(d);
            button::Style {
                background: card.background,
                text_color: Palette::TEXT_PRIMARY(),
                border: card.border,
                shadow: card.shadow,
                snap: false,
            }
        })
        .into()
    }

    pub fn copy_toast(&self, left: u64) -> Element<'static, Message> {
        let d = self.density;
        let card = container(
            row![
                icons::icon(icons::CIRCLE_CHECK, 13, Palette::SUCCESS()),
                hgap(8.0),
                text(tf("copy_toast", &[("secs", &left)]))
                    .size(sz(12))
                    .color(Palette::TEXT_SECONDARY()),
                hgap(10.0),
                button(text(t("copy_toast_undo")).size(sz(12)))
                    .on_press(Message::ClipboardCancel)
                    .style(theme::ghost_style(d)),
            ]
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .style(move |_| theme::modal_container(d));
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(18)
            .into()
    }

    pub fn centered<'a>(&self, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let d = self.density;
        let card = container(content)
            .padding(28)
            .width(Length::Fill)
            .max_width(440.0 * colony_ui::font_scale())
            .style(move |_| theme::card_container(d));
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(16)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    }

    pub fn modal<'a>(&self, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let d = self.density;
        let card = container(content)
            .padding(24)
            .max_width(560.0 * colony_ui::font_scale())
            .style(move |_| theme::modal_container(d));
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_| theme::modal_backdrop())
            .into()
    }
}

/// All entry types, in picker order.
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

/// Expiry picker choices: (i18n key, value). None keeps the current setting,
/// Some(0) means never.
pub const EXPIRY_CHOICES: [(&str, Option<u32>); 7] = [
    ("expiry_keep", None),
    ("expiry_never", Some(0)),
    ("expiry_30_days", Some(30)),
    ("expiry_60_days", Some(60)),
    ("expiry_90_days", Some(90)),
    ("expiry_180_days", Some(180)),
    ("expiry_1_year", Some(365)),
];

/// A circular countdown ring (the TOTP timer).
pub struct Ring {
    pub fraction: f32,
    pub color: Color,
    pub track: Color,
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

/// Grade letter with a +/- refinement from where the score sits in its band
/// (e.g. score 88 in band B -> "B+"). F has no modifier.
pub fn grade_display(score: u8, g: lilypad_core::HealthGrade) -> String {
    use lilypad_core::HealthGrade as G;
    let (letter, floor) = match g {
        G::A => ("A", 90),
        G::B => ("B", 80),
        G::C => ("C", 70),
        G::D => ("D", 60),
        G::F => return "F".to_string(),
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

/// A health grade as a filled badge.
pub fn grade_badge<'a>(report: &lilypad_core::HealthReport, base: u16) -> Element<'a, Message> {
    let fill = theme::health_grade_color(report.score.grade);
    container(
        text(grade_display(report.score.score, report.score.grade))
            .size(sz(base))
            .font(colony_ui::ui_font_bold())
            .color(colony_ui::contrast_on(fill)),
    )
    .padding([1, 7])
    .style(move |_| theme::filled(fill, 5.0))
    .into()
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
    match t {
        EntryType::Login => icons::KEY,
        EntryType::Card => icons::CREDIT_CARD,
        EntryType::Identity => icons::ID_CARD,
        EntryType::SecureNote => icons::STICKY_NOTE,
        EntryType::SoftwareLicense => icons::CERTIFICATE,
        EntryType::Wifi => icons::WIFI,
        EntryType::Server => icons::SERVER,
        EntryType::Custom => icons::LEAF,
    }
}

/// The plural name of an entry type, as the sidebar lists it.
pub fn type_label(t: &EntryType) -> &'static str {
    lilypad_core::i18n::t(match t {
        EntryType::Login => "type_logins",
        EntryType::Card => "type_cards",
        EntryType::Identity => "type_identities",
        EntryType::SecureNote => "type_notes",
        EntryType::SoftwareLicense => "type_licenses",
        EntryType::Wifi => "type_wifi",
        EntryType::Server => "type_servers",
        EntryType::Custom => "type_custom",
    })
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
