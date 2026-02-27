//! Lilypad Theme System
//!
//! Provides beautiful, cohesive themes for the Lilypad password manager.
//! Includes three distinct themes: Classic Green, Night Bloom, and Pond Light.

use iced::widget::{button, container, scrollable, text_input};
use iced::{Background, Border, Color, Shadow, Vector};

/// Lilypad color palette
#[derive(Debug, Clone, Copy)]
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

    /// Solar Flare theme - warm orange/amber dark theme
    pub fn solar_flare() -> Self {
        Self {
            primary: Color::from_rgb8(245, 158, 11),
            secondary: Color::from_rgb8(234, 88, 12),
            success: Color::from_rgb8(20, 184, 166),
            warning: Color::from_rgb8(250, 204, 21),
            danger: Color::from_rgb8(244, 63, 94),
            background: Color::from_rgb8(20, 15, 12),
            surface: Color::from_rgb8(32, 25, 20),
            surface_variant: Color::from_rgb8(45, 35, 28),
            text_primary: Color::from_rgb8(255, 247, 237),
            text_secondary: Color::from_rgb8(214, 197, 180),
            text_muted: Color::from_rgb8(156, 140, 125),
            border: Color::from_rgb8(65, 52, 42),
            hover: Color::from_rgba8(245, 158, 11, 0.1),
        }
    }

    /// Arctic theme - cool blue-tinted dark theme
    pub fn arctic() -> Self {
        Self {
            primary: Color::from_rgb8(56, 189, 248),
            secondary: Color::from_rgb8(14, 165, 233),
            success: Color::from_rgb8(52, 211, 153),
            warning: Color::from_rgb8(251, 191, 36),
            danger: Color::from_rgb8(248, 113, 113),
            background: Color::from_rgb8(8, 12, 20),
            surface: Color::from_rgb8(14, 20, 32),
            surface_variant: Color::from_rgb8(22, 30, 44),
            text_primary: Color::from_rgb8(240, 245, 255),
            text_secondary: Color::from_rgb8(170, 185, 210),
            text_muted: Color::from_rgb8(110, 125, 150),
            border: Color::from_rgb8(35, 48, 66),
            hover: Color::from_rgba8(56, 189, 248, 0.1),
        }
    }

    /// Catppuccin Mocha - soft pastel dark theme
    pub fn catppuccin_mocha() -> Self {
        Self {
            primary: Color::from_rgb8(203, 166, 247),    // Mauve
            secondary: Color::from_rgb8(137, 180, 250),  // Blue
            success: Color::from_rgb8(166, 227, 161),    // Green
            warning: Color::from_rgb8(249, 226, 175),    // Yellow
            danger: Color::from_rgb8(243, 139, 168),     // Red
            background: Color::from_rgb8(30, 30, 46),    // Base
            surface: Color::from_rgb8(49, 50, 68),       // Surface0
            surface_variant: Color::from_rgb8(69, 71, 90), // Surface1
            text_primary: Color::from_rgb8(205, 214, 244), // Text
            text_secondary: Color::from_rgb8(186, 194, 222), // Subtext1
            text_muted: Color::from_rgb8(166, 173, 200), // Subtext0
            border: Color::from_rgb8(88, 91, 112),       // Surface2
            hover: Color::from_rgba8(203, 166, 247, 0.1),
        }
    }

    /// Catppuccin Latte - soft pastel light theme
    pub fn catppuccin_latte() -> Self {
        Self {
            primary: Color::from_rgb8(136, 57, 239),     // Mauve
            secondary: Color::from_rgb8(30, 102, 245),   // Blue
            success: Color::from_rgb8(64, 160, 43),      // Green
            warning: Color::from_rgb8(223, 142, 29),     // Yellow
            danger: Color::from_rgb8(210, 15, 57),       // Red
            background: Color::from_rgb8(239, 241, 245), // Base
            surface: Color::from_rgb8(230, 233, 239),    // Mantle
            surface_variant: Color::from_rgb8(220, 224, 232), // Crust
            text_primary: Color::from_rgb8(76, 79, 105), // Text
            text_secondary: Color::from_rgb8(92, 95, 119), // Subtext1
            text_muted: Color::from_rgb8(108, 111, 133), // Subtext0
            border: Color::from_rgb8(188, 192, 204),     // Surface1
            hover: Color::from_rgba8(136, 57, 239, 0.08),
        }
    }

    /// Gruvbox Dark - retro warm dark theme
    pub fn gruvbox_dark() -> Self {
        Self {
            primary: Color::from_rgb8(215, 153, 33),     // Yellow
            secondary: Color::from_rgb8(254, 128, 25),   // Orange
            success: Color::from_rgb8(184, 187, 38),     // Green
            warning: Color::from_rgb8(250, 189, 47),     // Bright Yellow
            danger: Color::from_rgb8(251, 73, 52),       // Red
            background: Color::from_rgb8(40, 40, 40),    // bg0
            surface: Color::from_rgb8(60, 56, 54),       // bg1
            surface_variant: Color::from_rgb8(80, 73, 69), // bg2
            text_primary: Color::from_rgb8(235, 219, 178), // fg
            text_secondary: Color::from_rgb8(213, 196, 161), // fg1
            text_muted: Color::from_rgb8(168, 153, 132), // gray
            border: Color::from_rgb8(102, 92, 84),       // bg3
            hover: Color::from_rgba8(215, 153, 33, 0.1),
        }
    }

    /// Gruvbox Light - retro warm light theme
    pub fn gruvbox_light() -> Self {
        Self {
            primary: Color::from_rgb8(121, 116, 14),     // Green
            secondary: Color::from_rgb8(7, 102, 120),    // Cyan
            success: Color::from_rgb8(66, 123, 88),      // Aqua
            warning: Color::from_rgb8(181, 118, 20),     // Yellow
            danger: Color::from_rgb8(157, 0, 6),         // Red
            background: Color::from_rgb8(251, 241, 199), // bg0
            surface: Color::from_rgb8(242, 229, 188),    // bg1
            surface_variant: Color::from_rgb8(235, 219, 178), // bg2
            text_primary: Color::from_rgb8(40, 40, 40),  // fg0
            text_secondary: Color::from_rgb8(60, 56, 54), // fg1
            text_muted: Color::from_rgb8(102, 92, 84),   // gray
            border: Color::from_rgb8(213, 196, 161),     // bg3
            hover: Color::from_rgba8(121, 116, 14, 0.08),
        }
    }

    /// Everblush - cool dark pastel theme
    pub fn everblush() -> Self {
        Self {
            primary: Color::from_rgb8(103, 176, 232),    // Blue
            secondary: Color::from_rgb8(196, 127, 213),  // Purple
            success: Color::from_rgb8(140, 207, 126),    // Green
            warning: Color::from_rgb8(229, 199, 107),    // Yellow
            danger: Color::from_rgb8(229, 116, 116),     // Red
            background: Color::from_rgb8(20, 27, 30),    // bg
            surface: Color::from_rgb8(35, 42, 45),       // surface
            surface_variant: Color::from_rgb8(45, 58, 61), // elevated
            text_primary: Color::from_rgb8(218, 218, 218), // fg
            text_secondary: Color::from_rgb8(179, 185, 184), // fg dim
            text_muted: Color::from_rgb8(120, 130, 128), // muted
            border: Color::from_rgb8(55, 68, 71),        // border
            hover: Color::from_rgba8(103, 176, 232, 0.1),
        }
    }

    /// Kanagawa - Japanese ink painting inspired dark theme
    pub fn kanagawa() -> Self {
        Self {
            primary: Color::from_rgb8(127, 180, 202),    // Crystal blue
            secondary: Color::from_rgb8(210, 126, 153),  // Sakura pink
            success: Color::from_rgb8(106, 149, 137),    // Spring green
            warning: Color::from_rgb8(255, 158, 59),     // Autumn orange
            danger: Color::from_rgb8(228, 104, 118),     // Peach red
            background: Color::from_rgb8(31, 31, 40),    // Sumi ink
            surface: Color::from_rgb8(42, 42, 55),       // Surface
            surface_variant: Color::from_rgb8(54, 54, 70), // Surface1
            text_primary: Color::from_rgb8(220, 215, 186), // Fuji white
            text_secondary: Color::from_rgb8(200, 192, 147), // Old white
            text_muted: Color::from_rgb8(114, 113, 105), // Faded gray
            border: Color::from_rgb8(72, 72, 88),        // Border
            hover: Color::from_rgba8(127, 180, 202, 0.1),
        }
    }

    /// Nord - arctic north-bluish dark theme
    pub fn nord() -> Self {
        Self {
            primary: Color::from_rgb8(136, 192, 208),    // Nord8 Frost
            secondary: Color::from_rgb8(129, 161, 193),  // Nord9
            success: Color::from_rgb8(163, 190, 140),    // Nord14 Green
            warning: Color::from_rgb8(235, 203, 139),    // Nord13 Yellow
            danger: Color::from_rgb8(191, 97, 106),      // Nord11 Red
            background: Color::from_rgb8(46, 52, 64),    // Nord0
            surface: Color::from_rgb8(59, 66, 82),       // Nord1
            surface_variant: Color::from_rgb8(67, 76, 94), // Nord2
            text_primary: Color::from_rgb8(236, 239, 244), // Nord6
            text_secondary: Color::from_rgb8(216, 222, 233), // Nord4
            text_muted: Color::from_rgb8(76, 86, 106),   // Nord3
            border: Color::from_rgb8(76, 86, 106),       // Nord3
            hover: Color::from_rgba8(136, 192, 208, 0.1),
        }
    }

    /// Dracula - dark theme with vibrant colors
    pub fn dracula() -> Self {
        Self {
            primary: Color::from_rgb8(189, 147, 249),    // Purple
            secondary: Color::from_rgb8(255, 121, 198),  // Pink
            success: Color::from_rgb8(80, 250, 123),     // Green
            warning: Color::from_rgb8(241, 250, 140),    // Yellow
            danger: Color::from_rgb8(255, 85, 85),       // Red
            background: Color::from_rgb8(40, 42, 54),    // Background
            surface: Color::from_rgb8(68, 71, 90),       // Current Line
            surface_variant: Color::from_rgb8(80, 83, 102), // Selection
            text_primary: Color::from_rgb8(248, 248, 242), // Foreground
            text_secondary: Color::from_rgb8(210, 210, 220),
            text_muted: Color::from_rgb8(98, 114, 164),  // Comment
            border: Color::from_rgb8(98, 114, 164),      // Comment
            hover: Color::from_rgba8(189, 147, 249, 0.1),
        }
    }

    /// Solarized Dark - precision color scheme
    pub fn solarized_dark() -> Self {
        Self {
            primary: Color::from_rgb8(38, 139, 210),     // Blue
            secondary: Color::from_rgb8(42, 161, 152),   // Cyan
            success: Color::from_rgb8(133, 153, 0),      // Green
            warning: Color::from_rgb8(181, 137, 0),      // Yellow
            danger: Color::from_rgb8(220, 50, 47),       // Red
            background: Color::from_rgb8(0, 43, 54),     // Base03
            surface: Color::from_rgb8(7, 54, 66),        // Base02
            surface_variant: Color::from_rgb8(17, 64, 76),
            text_primary: Color::from_rgb8(131, 148, 150), // Base0
            text_secondary: Color::from_rgb8(147, 161, 161), // Base1
            text_muted: Color::from_rgb8(88, 110, 117),  // Base01
            border: Color::from_rgb8(88, 110, 117),      // Base01
            hover: Color::from_rgba8(38, 139, 210, 0.1),
        }
    }

    /// Tokyo Night - dark theme inspired by Tokyo city lights
    pub fn tokyo_night() -> Self {
        Self {
            primary: Color::from_rgb8(122, 162, 247),    // Blue
            secondary: Color::from_rgb8(187, 154, 247),  // Purple
            success: Color::from_rgb8(158, 206, 106),    // Green
            warning: Color::from_rgb8(224, 175, 104),    // Yellow
            danger: Color::from_rgb8(247, 118, 142),     // Red
            background: Color::from_rgb8(26, 27, 38),    // Night bg
            surface: Color::from_rgb8(36, 40, 59),       // Storm bg
            surface_variant: Color::from_rgb8(52, 56, 78),
            text_primary: Color::from_rgb8(169, 177, 214), // fg
            text_secondary: Color::from_rgb8(120, 124, 153), // dark5
            text_muted: Color::from_rgb8(86, 95, 137),   // comment
            border: Color::from_rgb8(60, 64, 86),
            hover: Color::from_rgba8(122, 162, 247, 0.1),
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
    SolarFlare,
    Arctic,
    CatppuccinMocha,
    CatppuccinLatte,
    GruvboxDark,
    GruvboxLight,
    Everblush,
    Kanagawa,
    Nord,
    Dracula,
    SolarizedDark,
    TokyoNight,
}

impl LilypadTheme {
    /// Get the palette for this theme
    pub fn palette(&self) -> LilypadPalette {
        match self {
            LilypadTheme::ClassicGreen => LilypadPalette::classic_green(),
            LilypadTheme::NightBloom => LilypadPalette::night_bloom(),
            LilypadTheme::PondLight => LilypadPalette::pond_light(),
            LilypadTheme::SolarFlare => LilypadPalette::solar_flare(),
            LilypadTheme::Arctic => LilypadPalette::arctic(),
            LilypadTheme::CatppuccinMocha => LilypadPalette::catppuccin_mocha(),
            LilypadTheme::CatppuccinLatte => LilypadPalette::catppuccin_latte(),
            LilypadTheme::GruvboxDark => LilypadPalette::gruvbox_dark(),
            LilypadTheme::GruvboxLight => LilypadPalette::gruvbox_light(),
            LilypadTheme::Everblush => LilypadPalette::everblush(),
            LilypadTheme::Kanagawa => LilypadPalette::kanagawa(),
            LilypadTheme::Nord => LilypadPalette::nord(),
            LilypadTheme::Dracula => LilypadPalette::dracula(),
            LilypadTheme::SolarizedDark => LilypadPalette::solarized_dark(),
            LilypadTheme::TokyoNight => LilypadPalette::tokyo_night(),
        }
    }

    /// Get theme display name
    pub fn name(&self) -> &'static str {
        match self {
            LilypadTheme::ClassicGreen => "Classic Green",
            LilypadTheme::NightBloom => "Night Bloom",
            LilypadTheme::PondLight => "Pond Light",
            LilypadTheme::SolarFlare => "Solar Flare",
            LilypadTheme::Arctic => "Arctic",
            LilypadTheme::CatppuccinMocha => "Catppuccin Mocha",
            LilypadTheme::CatppuccinLatte => "Catppuccin Latte",
            LilypadTheme::GruvboxDark => "Gruvbox Dark",
            LilypadTheme::GruvboxLight => "Gruvbox Light",
            LilypadTheme::Everblush => "Everblush",
            LilypadTheme::Kanagawa => "Kanagawa",
            LilypadTheme::Nord => "Nord",
            LilypadTheme::Dracula => "Dracula",
            LilypadTheme::SolarizedDark => "Solarized Dark",
            LilypadTheme::TokyoNight => "Tokyo Night",
        }
    }

    /// Get all available themes
    pub const ALL: [LilypadTheme; 15] = [
        LilypadTheme::ClassicGreen,
        LilypadTheme::NightBloom,
        LilypadTheme::PondLight,
        LilypadTheme::SolarFlare,
        LilypadTheme::Arctic,
        LilypadTheme::CatppuccinMocha,
        LilypadTheme::CatppuccinLatte,
        LilypadTheme::GruvboxDark,
        LilypadTheme::GruvboxLight,
        LilypadTheme::Everblush,
        LilypadTheme::Kanagawa,
        LilypadTheme::Nord,
        LilypadTheme::Dracula,
        LilypadTheme::SolarizedDark,
        LilypadTheme::TokyoNight,
    ];

    /// Convert from index
    pub fn from_index(index: usize) -> Self {
        *Self::ALL.get(index).unwrap_or(&LilypadTheme::ClassicGreen)
    }

    /// Convert to index
    pub fn to_index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

// ============================================================================
// UI Variations
// ============================================================================

/// UI variation that changes the visual feel (radius, shadows, borders)
/// independently of the color theme.
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

    pub fn name(&self) -> &'static str {
        match self {
            UiVariation::Default => "Default",
            UiVariation::Sharp => "Sharp",
            UiVariation::Rounded => "Rounded",
            UiVariation::Compact => "Compact",
            UiVariation::Spacious => "Spacious",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            UiVariation::Default => "Balanced, modern look",
            UiVariation::Sharp => "Clean, brutalist edges",
            UiVariation::Rounded => "Soft, friendly bubbles",
            UiVariation::Compact => "Tight, efficient UI",
            UiVariation::Spacious => "Airy, luxurious feel",
        }
    }

    pub fn from_index(index: usize) -> Self {
        *Self::ALL.get(index).unwrap_or(&UiVariation::Default)
    }

    pub fn to_index(self) -> usize {
        Self::ALL.iter().position(|v| *v == self).unwrap_or(0)
    }

    // --- Radius helpers ---

    pub fn card_radius(&self) -> f32 {
        match self {
            UiVariation::Default => 12.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 20.0,
            UiVariation::Compact => 4.0,
            UiVariation::Spacious => 16.0,
        }
    }

    pub fn button_radius(&self) -> f32 {
        match self {
            UiVariation::Default => 8.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 16.0,
            UiVariation::Compact => 4.0,
            UiVariation::Spacious => 14.0,
        }
    }

    pub fn input_radius(&self) -> f32 {
        match self {
            UiVariation::Default => 8.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 16.0,
            UiVariation::Compact => 4.0,
            UiVariation::Spacious => 14.0,
        }
    }

    pub fn modal_radius(&self) -> f32 {
        match self {
            UiVariation::Default => 16.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 24.0,
            UiVariation::Compact => 8.0,
            UiVariation::Spacious => 20.0,
        }
    }

    pub fn elevated_radius(&self) -> f32 {
        match self {
            UiVariation::Default => 8.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 14.0,
            UiVariation::Compact => 3.0,
            UiVariation::Spacious => 12.0,
        }
    }

    pub fn ghost_radius(&self) -> f32 {
        match self {
            UiVariation::Default => 6.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 12.0,
            UiVariation::Compact => 3.0,
            UiVariation::Spacious => 10.0,
        }
    }

    pub fn scroller_radius(&self) -> f32 {
        match self {
            UiVariation::Default => 4.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 8.0,
            UiVariation::Compact => 2.0,
            UiVariation::Spacious => 6.0,
        }
    }

    pub fn toast_radius(&self) -> f32 {
        match self {
            UiVariation::Default => 12.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 20.0,
            UiVariation::Compact => 4.0,
            UiVariation::Spacious => 16.0,
        }
    }

    // --- Shadow helpers ---

    pub fn card_shadow_blur(&self) -> f32 {
        match self {
            UiVariation::Default => 8.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 12.0,
            UiVariation::Compact => 2.0,
            UiVariation::Spacious => 16.0,
        }
    }

    pub fn card_shadow_offset(&self) -> f32 {
        match self {
            UiVariation::Default => 2.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 4.0,
            UiVariation::Compact => 1.0,
            UiVariation::Spacious => 4.0,
        }
    }

    pub fn card_shadow_alpha(&self) -> f32 {
        match self {
            UiVariation::Default => 0.1,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 0.08,
            UiVariation::Compact => 0.05,
            UiVariation::Spacious => 0.12,
        }
    }

    pub fn button_shadow_blur(&self) -> f32 {
        match self {
            UiVariation::Default => 4.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 6.0,
            UiVariation::Compact => 1.0,
            UiVariation::Spacious => 8.0,
        }
    }

    pub fn modal_shadow_blur(&self) -> f32 {
        match self {
            UiVariation::Default => 24.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 32.0,
            UiVariation::Compact => 12.0,
            UiVariation::Spacious => 40.0,
        }
    }

    pub fn header_shadow_blur(&self) -> f32 {
        match self {
            UiVariation::Default => 10.0,
            UiVariation::Sharp => 0.0,
            UiVariation::Rounded => 14.0,
            UiVariation::Compact => 4.0,
            UiVariation::Spacious => 18.0,
        }
    }

    // --- Border helpers ---

    pub fn border_width(&self) -> f32 {
        match self {
            UiVariation::Default => 1.0,
            UiVariation::Sharp => 1.0,
            UiVariation::Rounded => 0.0,
            UiVariation::Compact => 1.0,
            UiVariation::Spacious => 0.5,
        }
    }

    pub fn secondary_border_width(&self) -> f32 {
        match self {
            UiVariation::Default => 1.5,
            UiVariation::Sharp => 2.0,
            UiVariation::Rounded => 0.0,
            UiVariation::Compact => 1.0,
            UiVariation::Spacious => 1.0,
        }
    }
}

/// Health grade colors (used when health view renders grade badges)
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

/// Password strength colors (for entry health indicators)
pub fn strength_color(strength: u8, palette: &LilypadPalette) -> Color {
    match strength {
        0..=20 => palette.danger,
        21..=40 => Color::from_rgb8(249, 115, 22),
        41..=60 => palette.warning,
        61..=80 => Color::from_rgb8(132, 204, 22),
        _ => palette.success,
    }
}

/// Map an EntryColor to an Iced Color
pub fn entry_color_to_iced(color: &lilypad_core::EntryColor) -> Color {
    match color {
        lilypad_core::EntryColor::Red => Color::from_rgb8(239, 68, 68),
        lilypad_core::EntryColor::Orange => Color::from_rgb8(249, 115, 22),
        lilypad_core::EntryColor::Yellow => Color::from_rgb8(234, 179, 8),
        lilypad_core::EntryColor::Green => Color::from_rgb8(34, 197, 94),
        lilypad_core::EntryColor::Blue => Color::from_rgb8(59, 130, 246),
        lilypad_core::EntryColor::Purple => Color::from_rgb8(139, 92, 246),
        lilypad_core::EntryColor::Pink => Color::from_rgb8(236, 72, 153),
        lilypad_core::EntryColor::Gray => Color::from_rgb8(107, 114, 128),
    }
}

// ============================================================================
// Custom Container Styles
// ============================================================================

/// Style for the main application container
pub fn app_container(theme: LilypadTheme, _v: UiVariation) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.background)),
        text_color: Some(palette.text_primary),
        ..Default::default()
    }
}

/// Style for card/panel containers
pub fn card_container(theme: LilypadTheme, v: UiVariation) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: Some(palette.text_primary),
        border: Border {
            color: palette.border,
            width: v.border_width(),
            radius: v.card_radius().into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, v.card_shadow_alpha()),
            offset: Vector::new(0.0, v.card_shadow_offset()),
            blur_radius: v.card_shadow_blur(),
        },
        snap: false,
    }
}

/// Style for elevated surface containers
pub fn elevated_container(theme: LilypadTheme, v: UiVariation) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.surface_variant)),
        text_color: Some(palette.text_primary),
        border: Border {
            color: palette.border,
            width: v.border_width(),
            radius: v.elevated_radius().into(),
        },
        ..Default::default()
    }
}

/// Style for header container
pub fn header_container(theme: LilypadTheme, v: UiVariation) -> container::Style {
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
            color: Color::from_rgba(0.0, 0.0, 0.0, if v == UiVariation::Sharp { 0.0 } else { 0.15 }),
            offset: Vector::new(0.0, 2.0),
            blur_radius: v.header_shadow_blur(),
        },
        snap: false,
    }
}

/// Style for navigation bar container
pub fn nav_container(theme: LilypadTheme, v: UiVariation) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: Some(palette.text_primary),
        border: Border {
            color: palette.border,
            width: v.border_width(),
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// Style for sidebar container
pub fn sidebar_container(theme: LilypadTheme, v: UiVariation) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.surface_variant)),
        text_color: Some(palette.text_primary),
        border: Border {
            color: palette.border,
            width: v.border_width(),
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// Modal overlay background
pub fn modal_overlay(_theme: LilypadTheme, _v: UiVariation) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.6))),
        ..Default::default()
    }
}

/// Modal content container
pub fn modal_container(theme: LilypadTheme, v: UiVariation) -> container::Style {
    let palette = theme.palette();
    container::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: Some(palette.text_primary),
        border: Border {
            color: palette.border,
            width: v.border_width(),
            radius: v.modal_radius().into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, if v == UiVariation::Sharp { 0.0 } else { 0.25 }),
            offset: Vector::new(0.0, 8.0),
            blur_radius: v.modal_shadow_blur(),
        },
        snap: false,
    }
}

// ============================================================================
// Custom Button Styles
// ============================================================================

/// Primary action button style
pub fn primary_button(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(palette.primary)),
        text_color: Color::from_rgb8(10, 20, 30),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: v.button_radius().into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, v.card_shadow_alpha() * 2.0),
            offset: Vector::new(0.0, 2.0),
            blur_radius: v.button_shadow_blur(),
        },
        snap: false,
    }
}

/// Primary button hovered
pub fn primary_button_hovered(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    let mut style = primary_button(theme, v);
    style.background = Some(Background::Color(lighten_color(palette.primary, 0.1)));
    style.shadow.blur_radius = v.button_shadow_blur() * 2.0;
    style
}

/// Secondary/outline button style
pub fn secondary_button(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: palette.primary,
        border: Border {
            color: palette.primary,
            width: v.secondary_border_width(),
            radius: v.button_radius().into(),
        },
        ..Default::default()
    }
}

/// Secondary button hovered
pub fn secondary_button_hovered(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    let mut style = secondary_button(theme, v);
    style.background = Some(Background::Color(palette.hover));
    style
}

/// Ghost/text button style
pub fn ghost_button(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: palette.text_secondary,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: v.ghost_radius().into(),
        },
        ..Default::default()
    }
}

/// Ghost button hovered
pub fn ghost_button_hovered(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    let mut style = ghost_button(theme, v);
    style.background = Some(Background::Color(palette.hover));
    style.text_color = palette.text_primary;
    style
}

/// Danger button style
pub fn danger_button(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(palette.danger)),
        text_color: Color::WHITE,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: v.button_radius().into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, v.card_shadow_alpha() * 2.0),
            offset: Vector::new(0.0, 2.0),
            blur_radius: v.button_shadow_blur(),
        },
        snap: false,
    }
}

/// Danger button hovered
pub fn danger_button_hovered(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    let mut style = danger_button(theme, v);
    style.background = Some(Background::Color(lighten_color(palette.danger, 0.1)));
    style
}

/// Navigation tab button (inactive)
pub fn nav_button(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: palette.text_muted,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: v.button_radius().into(),
        },
        ..Default::default()
    }
}

/// Navigation tab button (active)
pub fn nav_button_active(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(palette.hover)),
        text_color: palette.primary,
        border: Border {
            color: palette.primary,
            width: 0.0,
            radius: v.button_radius().into(),
        },
        ..Default::default()
    }
}

/// Icon button style
pub fn icon_button(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: palette.text_secondary,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: v.ghost_radius().into(),
        },
        ..Default::default()
    }
}

/// Icon button hovered
pub fn icon_button_hovered(theme: LilypadTheme, v: UiVariation) -> button::Style {
    let palette = theme.palette();
    let mut style = icon_button(theme, v);
    style.background = Some(Background::Color(palette.surface_variant));
    style.text_color = palette.primary;
    style
}

// ============================================================================
// Custom Text Input Styles
// ============================================================================

/// Standard text input style
pub fn text_input_style(theme: LilypadTheme, v: UiVariation) -> text_input::Style {
    let palette = theme.palette();
    text_input::Style {
        background: Background::Color(palette.surface_variant),
        border: Border {
            color: palette.border,
            width: v.border_width(),
            radius: v.input_radius().into(),
        },
        icon: palette.text_muted,
        placeholder: palette.text_muted,
        value: palette.text_primary,
        selection: palette.primary,
    }
}

/// Focused text input style
pub fn text_input_focused(theme: LilypadTheme, v: UiVariation) -> text_input::Style {
    let palette = theme.palette();
    let mut style = text_input_style(theme, v);
    style.border.color = palette.primary;
    style.border.width = 2.0;
    style
}

/// Error text input style (for form validation feedback)
pub fn text_input_error(theme: LilypadTheme, v: UiVariation) -> text_input::Style {
    let palette = theme.palette();
    let mut style = text_input_style(theme, v);
    style.border.color = palette.danger;
    style.border.width = 2.0;
    style
}

// ============================================================================
// Custom Scrollable Style
// ============================================================================

/// Scrollable style
pub fn scrollable_style(theme: LilypadTheme, v: UiVariation) -> scrollable::Style {
    let palette = theme.palette();
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: scrollable::Rail {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(palette.border),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: v.scroller_radius().into(),
                },
            },
        },
        horizontal_rail: scrollable::Rail {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(palette.border),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: v.scroller_radius().into(),
                },
            },
        },
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            shadow: Shadow::default(),
            icon: palette.text_secondary,
        },
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

/// Darken a color by a factor (0.0 to 1.0) - counterpart to lighten_color
pub fn darken_color(color: Color, factor: f32) -> Color {
    Color {
        r: (color.r * (1.0 - factor)).max(0.0),
        g: (color.g * (1.0 - factor)).max(0.0),
        b: (color.b * (1.0 - factor)).max(0.0),
        a: color.a,
    }
}

/// Toast notification container (floating)
pub fn toast_container(_theme: LilypadTheme, v: UiVariation) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(30, 30, 30, 0.92))),
        text_color: Some(Color::WHITE),
        border: Border {
            color: Color::from_rgba8(255, 255, 255, 0.1),
            width: v.border_width(),
            radius: v.toast_radius().into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, if v == UiVariation::Sharp { 0.0 } else { 0.4 }),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: v.card_shadow_blur(),
        },
        ..Default::default()
    }
}

/// Create a color with alpha
pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}
