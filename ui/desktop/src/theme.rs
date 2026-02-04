//! Lilypad Theme System
//!
//! Provides beautiful, cohesive themes for the Lilypad password manager.
//! Includes three distinct themes: Classic Green, Night Bloom, and Pond Light.

use iced::widget::{button, container, scrollable, text_input};
use iced::{Background, Border, Color, Shadow, Vector};

/// Lilypad color palette
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct LilypadPalette {
    /// Primary accent color (green tones)
    pub primary: Color,
    /// Secondary accent color
    pub secondary: Color,
    /// Success color (green)
    pub success: Color,
    /// Warning color (orange/yellow)
    pub warning: Color,
    /// Danger color (red)
    pub danger: Color,
    /// Background color
    pub background: Color,
    /// Surface color (cards, panels)
    pub surface: Color,
    /// Surface variant (elevated surfaces)
    pub surface_variant: Color,
    /// Text primary color
    pub text_primary: Color,
    /// Text secondary color
    pub text_secondary: Color,
    /// Text muted color
    pub text_muted: Color,
    /// Border color
    pub border: Color,
    /// Hover overlay color
    pub hover: Color,
}

impl LilypadPalette {
    /// Classic Green theme - the default Lilypad look
    pub fn classic_green() -> Self {
        Self {
            primary: Color::from_rgb8(111, 207, 151),
            secondary: Color::from_rgb8(76, 175, 80),
            success: Color::from_rgb8(34, 197, 94),
            warning: Color::from_rgb8(234, 179, 8),
            danger: Color::from_rgb8(239, 68, 68),
            background: Color::from_rgb8(14, 22, 33),
            surface: Color::from_rgb8(22, 33, 46),
            surface_variant: Color::from_rgb8(30, 44, 60),
            text_primary: Color::from_rgb8(255, 255, 255),
            text_secondary: Color::from_rgb8(200, 210, 220),
            text_muted: Color::from_rgb8(140, 150, 165),
            border: Color::from_rgb8(45, 60, 78),
            hover: Color::from_rgba8(111, 207, 151, 0.1),
        }
    }

    /// Night Bloom theme - darker, more dramatic
    pub fn night_bloom() -> Self {
        Self {
            primary: Color::from_rgb8(167, 139, 250),
            secondary: Color::from_rgb8(139, 92, 246),
            success: Color::from_rgb8(52, 211, 153),
            warning: Color::from_rgb8(251, 191, 36),
            danger: Color::from_rgb8(248, 113, 113),
            background: Color::from_rgb8(10, 10, 15),
            surface: Color::from_rgb8(18, 18, 25),
            surface_variant: Color::from_rgb8(28, 28, 38),
            text_primary: Color::from_rgb8(250, 250, 255),
            text_secondary: Color::from_rgb8(180, 180, 200),
            text_muted: Color::from_rgb8(120, 120, 140),
            border: Color::from_rgb8(40, 40, 55),
            hover: Color::from_rgba8(167, 139, 250, 0.1),
        }
    }

    /// Pond Light theme - clean, bright appearance
    pub fn pond_light() -> Self {
        Self {
            primary: Color::from_rgb8(34, 139, 34),
            secondary: Color::from_rgb8(46, 125, 50),
            success: Color::from_rgb8(22, 163, 74),
            warning: Color::from_rgb8(202, 138, 4),
            danger: Color::from_rgb8(220, 38, 38),
            background: Color::from_rgb8(250, 252, 251),
            surface: Color::from_rgb8(255, 255, 255),
            surface_variant: Color::from_rgb8(245, 247, 250),
            text_primary: Color::from_rgb8(15, 23, 42),
            text_secondary: Color::from_rgb8(71, 85, 105),
            text_muted: Color::from_rgb8(148, 163, 184),
            border: Color::from_rgb8(226, 232, 240),
            hover: Color::from_rgba8(34, 139, 34, 0.08),
        }
    }
}

/// Available theme variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LilypadTheme {
    #[default]
    ClassicGreen,
    NightBloom,
    PondLight,
}

impl LilypadTheme {
    /// Get the palette for this theme
    pub fn palette(&self) -> LilypadPalette {
        match self {
            LilypadTheme::ClassicGreen => LilypadPalette::classic_green(),
            LilypadTheme::NightBloom => LilypadPalette::night_bloom(),
            LilypadTheme::PondLight => LilypadPalette::pond_light(),
        }
    }

    /// Get theme display name
    pub fn name(&self) -> &'static str {
        match self {
            LilypadTheme::ClassicGreen => "Classic Green",
            LilypadTheme::NightBloom => "Night Bloom",
            LilypadTheme::PondLight => "Pond Light",
        }
    }

    /// Get all available themes
    pub const ALL: [LilypadTheme; 3] = [
        LilypadTheme::ClassicGreen,
        LilypadTheme::NightBloom,
        LilypadTheme::PondLight,
    ];

    /// Convert from index
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => LilypadTheme::ClassicGreen,
            1 => LilypadTheme::NightBloom,
            2 => LilypadTheme::PondLight,
            _ => LilypadTheme::ClassicGreen,
        }
    }

    /// Convert to index
    pub fn to_index(&self) -> usize {
        match self {
            LilypadTheme::ClassicGreen => 0,
            LilypadTheme::NightBloom => 1,
            LilypadTheme::PondLight => 2,
        }
    }
}

/// Health grade colors
#[allow(dead_code)]
pub fn health_grade_color(grade: &str, palette: &LilypadPalette) -> Color {
    match grade {
        "A" => Color::from_rgb8(34, 197, 94),   // Green
        "B" => Color::from_rgb8(132, 204, 22),  // Lime
        "C" => Color::from_rgb8(234, 179, 8),   // Yellow
        "D" => Color::from_rgb8(249, 115, 22),  // Orange
        "F" => Color::from_rgb8(239, 68, 68),   // Red
        _ => palette.text_muted,
    }
}

/// Password strength colors
#[allow(dead_code)]
pub fn strength_color(strength: u8, palette: &LilypadPalette) -> Color {
    match strength {
        0..=20 => palette.danger,
        21..=40 => Color::from_rgb8(249, 115, 22),
        41..=60 => palette.warning,
        61..=80 => Color::from_rgb8(132, 204, 22),
        _ => palette.success,
    }
}

// ============================================================================
// Custom Container Styles
// ============================================================================

/// Style for the main application container
pub fn app_container(theme: LilypadTheme) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.background)),
        text_color: Some(palette.text_primary),
        ..Default::default()
    }
}

/// Style for card/panel containers
pub fn card_container(theme: LilypadTheme) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: Some(palette.text_primary),
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.1),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
    }
}

/// Style for elevated surface containers
pub fn elevated_container(theme: LilypadTheme) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.surface_variant)),
        text_color: Some(palette.text_primary),
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

/// Style for header container
pub fn header_container(theme: LilypadTheme) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: Some(palette.text_primary),
        border: Border {
            color: palette.border,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.15),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 10.0,
        },
    }
}

/// Style for navigation bar container
pub fn nav_container(theme: LilypadTheme) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: Some(palette.text_primary),
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// Style for sidebar container
#[allow(dead_code)]
pub fn sidebar_container(theme: LilypadTheme) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.surface_variant)),
        text_color: Some(palette.text_primary),
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// Modal overlay background
pub fn modal_overlay(_theme: LilypadTheme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.6))),
        ..Default::default()
    }
}

/// Modal content container
pub fn modal_container(theme: LilypadTheme) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: Some(palette.text_primary),
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: 16.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.25),
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
    }
}

// ============================================================================
// Custom Button Styles
// ============================================================================

/// Primary action button style
pub fn primary_button(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(palette.primary)),
        text_color: Color::from_rgb8(10, 20, 30),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 8.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.2),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 4.0,
        },
    }
}

/// Primary button hovered
pub fn primary_button_hovered(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    let mut style = primary_button(theme);
    style.background = Some(Background::Color(lighten_color(palette.primary, 0.1)));
    style.shadow.blur_radius = 8.0;
    style
}

/// Secondary/outline button style
pub fn secondary_button(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: palette.primary,
        border: Border {
            color: palette.primary,
            width: 1.5,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

/// Secondary button hovered
pub fn secondary_button_hovered(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    let mut style = secondary_button(theme);
    style.background = Some(Background::Color(palette.hover));
    style
}

/// Ghost/text button style
pub fn ghost_button(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: palette.text_secondary,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

/// Ghost button hovered
pub fn ghost_button_hovered(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    let mut style = ghost_button(theme);
    style.background = Some(Background::Color(palette.hover));
    style.text_color = palette.text_primary;
    style
}

/// Danger button style
pub fn danger_button(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(palette.danger)),
        text_color: Color::WHITE,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 8.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.2),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 4.0,
        },
    }
}

/// Danger button hovered
pub fn danger_button_hovered(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    let mut style = danger_button(theme);
    style.background = Some(Background::Color(lighten_color(palette.danger, 0.1)));
    style
}

/// Navigation tab button (inactive)
pub fn nav_button(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: palette.text_muted,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

/// Navigation tab button (active)
pub fn nav_button_active(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(palette.hover)),
        text_color: palette.primary,
        border: Border {
            color: palette.primary,
            width: 0.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

/// Icon button style
pub fn icon_button(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: palette.text_secondary,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

/// Icon button hovered
pub fn icon_button_hovered(theme: LilypadTheme) -> button::Style {
    let palette = theme.palette();
    let mut style = icon_button(theme);
    style.background = Some(Background::Color(palette.surface_variant));
    style.text_color = palette.primary;
    style
}

// ============================================================================
// Custom Text Input Styles
// ============================================================================

/// Standard text input style
pub fn text_input_style(theme: LilypadTheme) -> text_input::Style {
    let palette = theme.palette();
    text_input::Style {
        background: Background::Color(palette.surface_variant),
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: 8.0.into(),
        },
        icon: palette.text_muted,
        placeholder: palette.text_muted,
        value: palette.text_primary,
        selection: palette.primary,
    }
}

/// Focused text input style
pub fn text_input_focused(theme: LilypadTheme) -> text_input::Style {
    let palette = theme.palette();
    let mut style = text_input_style(theme);
    style.border.color = palette.primary;
    style.border.width = 2.0;
    style
}

/// Error text input style
#[allow(dead_code)]
pub fn text_input_error(theme: LilypadTheme) -> text_input::Style {
    let palette = theme.palette();
    let mut style = text_input_style(theme);
    style.border.color = palette.danger;
    style.border.width = 2.0;
    style
}

// ============================================================================
// Custom Scrollable Style
// ============================================================================

/// Scrollable style
pub fn scrollable_style(theme: LilypadTheme) -> scrollable::Style {
    let palette = theme.palette();
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: scrollable::Rail {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border::default(),
            scroller: scrollable::Scroller {
                color: palette.border,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 4.0.into(),
                },
            },
        },
        horizontal_rail: scrollable::Rail {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border::default(),
            scroller: scrollable::Scroller {
                color: palette.border,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 4.0.into(),
                },
            },
        },
        gap: None,
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Lighten a color by a factor (0.0 to 1.0)
fn lighten_color(color: Color, factor: f32) -> Color {
    Color {
        r: (color.r + (1.0 - color.r) * factor).min(1.0),
        g: (color.g + (1.0 - color.g) * factor).min(1.0),
        b: (color.b + (1.0 - color.b) * factor).min(1.0),
        a: color.a,
    }
}

/// Darken a color by a factor (0.0 to 1.0)
#[allow(dead_code)]
fn darken_color(color: Color, factor: f32) -> Color {
    Color {
        r: (color.r * (1.0 - factor)).max(0.0),
        g: (color.g * (1.0 - factor)).max(0.0),
        b: (color.b * (1.0 - factor)).max(0.0),
        a: color.a,
    }
}

/// Create a color with alpha
#[allow(dead_code)]
pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}
