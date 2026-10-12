//! Lilypad's widget styles.
//!
//! Every colour comes from the active Colony palette
//! (`colony_ui::theme::Palette`), which follows the user's theme, accent and
//! high-contrast choice; the interface density shapes radii, borders and
//! shadows.

use colony_ui::theme::{accent_key_to_color, contrast_on, Palette};
use iced::widget::{button, container, text_input};
use iced::{Background, Border, Color, Shadow, Vector};
use lilypad_core::{EntryColor, HealthGrade};

/// The interface density: radii, borders and shadows, independent of the
/// colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiVariation {
    #[default]
    Default,
    Sharp,
    Rounded,
    Compact,
    Spacious,
}

impl UiVariation {
    pub const ALL: [UiVariation; 5] = [
        UiVariation::Default,
        UiVariation::Sharp,
        UiVariation::Rounded,
        UiVariation::Compact,
        UiVariation::Spacious,
    ];

    /// The i18n keys of its name and description.
    pub fn label_keys(self) -> (&'static str, &'static str) {
        match self {
            UiVariation::Default => ("density_default", "density_default_desc"),
            UiVariation::Sharp => ("density_sharp", "density_sharp_desc"),
            UiVariation::Rounded => ("density_rounded", "density_rounded_desc"),
            UiVariation::Compact => ("density_compact", "density_compact_desc"),
            UiVariation::Spacious => ("density_spacious", "density_spacious_desc"),
        }
    }

    pub fn from_index(index: usize) -> Self {
        *Self::ALL.get(index).unwrap_or(&UiVariation::Default)
    }

    pub fn to_index(self) -> usize {
        Self::ALL.iter().position(|v| *v == self).unwrap_or(0)
    }

    /// One value per variation, in [`Self::ALL`] order.
    fn pick(self, values: [f32; 5]) -> f32 {
        values[self.to_index()]
    }

    fn card_radius(self) -> f32 {
        self.pick([12.0, 0.0, 20.0, 4.0, 16.0])
    }

    fn button_radius(self) -> f32 {
        self.pick([8.0, 0.0, 16.0, 4.0, 14.0])
    }

    fn modal_radius(self) -> f32 {
        self.pick([16.0, 0.0, 24.0, 8.0, 20.0])
    }

    fn elevated_radius(self) -> f32 {
        self.pick([8.0, 0.0, 14.0, 3.0, 12.0])
    }

    fn ghost_radius(self) -> f32 {
        self.pick([6.0, 0.0, 12.0, 3.0, 10.0])
    }

    fn card_shadow(self) -> Shadow {
        shadow(
            self.pick([0.1, 0.0, 0.08, 0.05, 0.12]),
            self.pick([2.0, 0.0, 4.0, 1.0, 4.0]),
            self.pick([8.0, 0.0, 12.0, 2.0, 16.0]),
        )
    }

    fn button_shadow(self) -> Shadow {
        shadow(
            self.pick([0.2, 0.0, 0.16, 0.1, 0.24]),
            2.0,
            self.pick([4.0, 0.0, 6.0, 1.0, 8.0]),
        )
    }

    fn modal_shadow(self) -> Shadow {
        let alpha = if self == UiVariation::Sharp {
            0.0
        } else {
            0.25
        };
        shadow(alpha, 8.0, self.pick([24.0, 0.0, 32.0, 12.0, 40.0]))
    }

    fn border_width(self) -> f32 {
        self.pick([1.0, 1.0, 0.0, 1.0, 0.5])
    }

    fn secondary_border_width(self) -> f32 {
        self.pick([1.5, 2.0, 0.0, 1.0, 1.0])
    }
}

/// A black shadow at `alpha`, dropped by `offset`.
fn shadow(alpha: f32, offset: f32, blur_radius: f32) -> Shadow {
    Shadow {
        color: tint(Color::BLACK, alpha),
        offset: Vector::new(0.0, offset),
        blur_radius,
    }
}

/// `c` at the given opacity.
pub fn tint(c: Color, alpha: f32) -> Color {
    Color { a: alpha, ..c }
}

/// The iced theme for widgets Lilypad does not style itself (sliders,
/// scrollbars, the entry-form pickers), built from the same palette.
pub fn iced_theme() -> iced::Theme {
    iced::Theme::custom(
        "Lilypad",
        iced::theme::Palette {
            background: Palette::BG_PRIMARY(),
            text: Palette::TEXT_PRIMARY(),
            primary: Palette::ACCENT(),
            success: Palette::SUCCESS(),
            warning: Palette::WARNING(),
            danger: Palette::ERROR(),
        },
    )
}

// ---------------------------------------------------------------------------
// Colours with a meaning
// ---------------------------------------------------------------------------

/// The badge colour of a vault health grade.
pub fn health_grade_color(grade: HealthGrade) -> Color {
    match grade {
        HealthGrade::A | HealthGrade::B => Palette::SUCCESS(),
        HealthGrade::C => Palette::WARNING(),
        HealthGrade::D => accent("orange"),
        HealthGrade::F => Palette::ERROR(),
    }
}

/// The colour of a password strength level, 1 (very weak) to 5 (very
/// strong); 0 is unknown.
pub fn strength_color(level: u8) -> Color {
    match level {
        0 => Palette::TEXT_MUTED(),
        1 => Palette::ERROR(),
        2 => accent("orange"),
        3 => Palette::WARNING(),
        4 => accent("green"),
        _ => Palette::SUCCESS(),
    }
}

/// The colour of an entry's colour label. Labels map to the shared Colony
/// accents, so they read the same on every theme; grey is the palette's
/// muted ink.
pub fn entry_color(c: EntryColor) -> Color {
    match c {
        EntryColor::Red => accent("red"),
        EntryColor::Orange => accent("orange"),
        EntryColor::Yellow => accent("yellow"),
        EntryColor::Green => accent("green"),
        EntryColor::Blue => accent("blue"),
        EntryColor::Purple => accent("indigo"),
        EntryColor::Pink => accent("violet"),
        EntryColor::Gray => Palette::TEXT_MUTED(),
    }
}

/// A shipped Colony accent, or the active accent if the key is unknown.
fn accent(key: &str) -> Color {
    accent_key_to_color(key).unwrap_or_else(Palette::ACCENT)
}

// ---------------------------------------------------------------------------
// Containers
// ---------------------------------------------------------------------------

/// The window background.
pub fn app_container(_v: UiVariation) -> container::Style {
    container::Style {
        background: Some(Background::Color(Palette::BG_PRIMARY())),
        text_color: Some(Palette::TEXT_PRIMARY()),
        ..Default::default()
    }
}

/// The sidebar.
pub fn sidebar_container(v: UiVariation) -> container::Style {
    container::Style {
        background: Some(Background::Color(Palette::BG_SIDEBAR())),
        text_color: Some(Palette::TEXT_PRIMARY()),
        border: Border {
            color: Palette::BORDER_SUBTLE(),
            width: v.border_width(),
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// Cards and panels.
pub fn card_container(v: UiVariation) -> container::Style {
    container::Style {
        background: Some(Background::Color(Palette::BG_CARD())),
        text_color: Some(Palette::TEXT_PRIMARY()),
        border: Border {
            color: Palette::BORDER_SUBTLE(),
            width: v.border_width(),
            radius: v.card_radius().into(),
        },
        shadow: v.card_shadow(),
        snap: false,
    }
}

/// A surface raised above a card.
pub fn elevated_container(v: UiVariation) -> container::Style {
    container::Style {
        background: Some(Background::Color(Palette::BG_CARD_HOVER())),
        text_color: Some(Palette::TEXT_PRIMARY()),
        border: Border {
            color: Palette::BORDER_SUBTLE(),
            width: v.border_width(),
            radius: v.elevated_radius().into(),
        },
        ..Default::default()
    }
}

/// A dialog over the dimmed window.
pub fn modal_container(v: UiVariation) -> container::Style {
    container::Style {
        background: Some(Background::Color(Palette::BG_CARD())),
        text_color: Some(Palette::TEXT_PRIMARY()),
        border: Border {
            color: Palette::BORDER_SUBTLE(),
            width: v.border_width(),
            radius: v.modal_radius().into(),
        },
        shadow: v.modal_shadow(),
        snap: false,
    }
}

/// The dimmed window behind a dialog.
pub fn modal_backdrop() -> container::Style {
    container::Style {
        background: Some(Background::Color(tint(Color::BLACK, 0.55))),
        ..Default::default()
    }
}

/// Small badges: tags, counts.
pub fn badge_container() -> container::Style {
    container::Style {
        background: Some(Background::Color(Palette::BG_CARD_HOVER())),
        border: Border {
            radius: 3.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// The tile behind an entry type glyph.
pub fn icon_badge_container() -> container::Style {
    container::Style {
        background: Some(Background::Color(Palette::BG_CARD_HOVER())),
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A small filled dot (password strength).
pub fn dot_indicator(color: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A filled pill: a health grade badge, a colour bar.
pub fn filled(color: Color, radius: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            radius: radius.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A banner tinted with `color` (a health warning on an entry).
pub fn banner(color: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(tint(color, 0.14))),
        border: Border {
            radius: 8.0.into(),
            color: tint(color, 0.4),
            width: 1.0,
        },
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

type ButtonStyle = Box<dyn Fn(&iced::Theme, button::Status) -> button::Style>;

/// A filled button on `fill`, its text in whichever of near-black and
/// near-white reads on it.
fn filled_button(v: UiVariation, fill: Color, hover: Color, pressed: Color) -> ButtonStyle {
    Box::new(move |_, status| {
        let background = match status {
            button::Status::Hovered => hover,
            button::Status::Pressed => pressed,
            button::Status::Disabled => Palette::BTN_DEFAULT(),
            button::Status::Active => fill,
        };
        button::Style {
            background: Some(Background::Color(background)),
            text_color: contrast_on(background),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: v.button_radius().into(),
            },
            shadow: if status == button::Status::Pressed {
                Shadow::default()
            } else {
                v.button_shadow()
            },
            snap: false,
        }
    })
}

/// The main action: accent fill.
pub fn primary_style(v: UiVariation) -> ButtonStyle {
    let accent = Palette::ACCENT();
    filled_button(v, accent, tint(accent, 0.85), tint(accent, 0.7))
}

/// A destructive action: error fill.
pub fn danger_style(v: UiVariation) -> ButtonStyle {
    filled_button(
        v,
        Palette::ERROR(),
        Palette::BTN_TRASH_HOVER(),
        Palette::BTN_TRASH_PRESSED(),
    )
}

/// An outlined button in the accent.
pub fn secondary_style(v: UiVariation) -> ButtonStyle {
    Box::new(move |_, status| {
        let accent = Palette::ACCENT();
        let background = match status {
            button::Status::Hovered => Palette::BG_CARD_HOVER(),
            button::Status::Pressed => tint(accent, 0.15),
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(Background::Color(background)),
            text_color: accent,
            border: Border {
                color: accent,
                width: v.secondary_border_width(),
                radius: v.button_radius().into(),
            },
            ..Default::default()
        }
    })
}

/// A text button: no fill until hovered.
pub fn ghost_style(v: UiVariation) -> ButtonStyle {
    Box::new(move |_, status| {
        let (background, text_color) = match status {
            button::Status::Hovered => (Palette::BG_CARD_HOVER(), Palette::TEXT_PRIMARY()),
            button::Status::Pressed => (Palette::BG_CARD_PRESSED(), Palette::ACCENT()),
            _ => (Color::TRANSPARENT, Palette::TEXT_SECONDARY()),
        };
        button::Style {
            background: Some(Background::Color(background)),
            text_color,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: v.ghost_radius().into(),
            },
            ..Default::default()
        }
    })
}

// ---------------------------------------------------------------------------
// Text inputs
// ---------------------------------------------------------------------------

/// Text fields: `bg_input`, the accent edge while focused.
pub fn text_input_style(
    v: UiVariation,
) -> impl Fn(&iced::Theme, text_input::Status) -> text_input::Style {
    move |_, status| {
        let focused = matches!(status, text_input::Status::Focused { .. });
        text_input::Style {
            background: Background::Color(Palette::BG_INPUT()),
            border: Border {
                color: if focused {
                    Palette::ACCENT()
                } else {
                    Palette::BORDER_SUBTLE()
                },
                width: if focused { 2.0 } else { v.border_width() },
                radius: v.button_radius().into(),
            },
            icon: Palette::TEXT_MUTED(),
            placeholder: Palette::TEXT_PLACEHOLDER(),
            value: Palette::TEXT_PRIMARY(),
            selection: tint(Palette::ACCENT(), 0.4),
        }
    }
}
