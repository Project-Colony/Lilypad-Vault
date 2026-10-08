//! Font Configuration
//!
//! JetBrainsMono Nerd Font with icons for the Lilypad application.

use iced::alignment::Horizontal;
use iced::widget::{container, text};
use iced::{Color, Element, Font, Length};

// Font data embedded at compile time
pub const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFont-Regular.ttf");
pub const JETBRAINS_MONO_BOLD: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFont-Bold.ttf");
pub const JETBRAINS_MONO_SEMIBOLD: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFont-SemiBold.ttf");
pub const JETBRAINS_MONO_MEDIUM: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFont-Medium.ttf");
pub const JETBRAINS_MONO_LIGHT: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFont-Light.ttf");

// Font definitions
pub const FONT_REGULAR: Font = Font::with_name("JetBrainsMono Nerd Font");
pub const FONT_BOLD: Font = Font {
    weight: iced::font::Weight::Bold,
    ..Font::with_name("JetBrainsMono Nerd Font")
};
pub const FONT_SEMIBOLD: Font = Font {
    weight: iced::font::Weight::Semibold,
    ..Font::with_name("JetBrainsMono Nerd Font")
};
pub const FONT_MEDIUM: Font = Font {
    weight: iced::font::Weight::Medium,
    ..Font::with_name("JetBrainsMono Nerd Font")
};
pub const FONT_LIGHT: Font = Font {
    weight: iced::font::Weight::Light,
    ..Font::with_name("JetBrainsMono Nerd Font")
};

// ============================================================================
// Nerd Font Icons
// ============================================================================
// Reference: https://www.nerdfonts.com/cheat-sheet

// Icon library for the Lilypad UI. All icons reference Nerd Font codepoints.
// Not all icons are used yet — this is a curated palette for the UI.
#[allow(dead_code)]
pub mod icons {
    // Navigation & UI
    pub const VAULT: &str = "\u{f023}"; //
    pub const LOCK: &str = "\u{f023}"; //
    pub const LOCK_OPEN: &str = "\u{f3c1}"; //
    pub const KEY: &str = "\u{f084}"; //
    pub const SHIELD: &str = "\u{f132}"; //
    pub const SHIELD_CHECK: &str = "\u{f2f7}"; // 󰅗
    pub const COG: &str = "\u{f013}"; //
    pub const GEAR: &str = "\u{f013}"; //
    pub const SEARCH: &str = "\u{f002}"; //
    pub const PLUS: &str = "\u{f067}"; //
    pub const CLOSE: &str = "\u{f00d}"; //
    pub const CHECK: &str = "\u{f00c}"; //
    pub const CHEVRON_DOWN: &str = "\u{f078}"; //
    pub const CHEVRON_RIGHT: &str = "\u{f054}"; //
    pub const ELLIPSIS: &str = "\u{f141}"; //

    // Categories / Navigation tabs
    pub const CREDENTIAL: &str = "\u{f084}"; //
    pub const HEART: &str = "\u{f004}"; //
    pub const HEART_PULSE: &str = "\u{f21e}"; //
    pub const DICE: &str = "\u{f522}"; //
    pub const WAND: &str = "\u{f0d0}"; //  magic wand — password generator
    pub const SYNC: &str = "\u{f021}"; //
    pub const REFRESH: &str = "\u{f021}"; //
    pub const USER: &str = "\u{f007}"; //
    pub const USER_SHIELD: &str = "\u{f505}"; //

    // Actions
    pub const COPY: &str = "\u{f0c5}"; //
    pub const EYE: &str = "\u{f06e}"; //
    pub const EYE_SLASH: &str = "\u{f070}"; //
    pub const EDIT: &str = "\u{f044}"; //
    pub const TRASH: &str = "\u{f1f8}"; //
    pub const SAVE: &str = "\u{f0c7}"; //
    pub const DOWNLOAD: &str = "\u{f019}"; //
    pub const UPLOAD: &str = "\u{f093}"; //
    pub const EXTERNAL_LINK: &str = "\u{f08e}"; //

    // Status indicators
    pub const CIRCLE: &str = "\u{f111}"; //
    pub const CIRCLE_CHECK: &str = "\u{f058}"; //
    pub const CIRCLE_XMARK: &str = "\u{f057}"; //
    pub const CIRCLE_EXCLAMATION: &str = "\u{f06a}"; //
    pub const TRIANGLE_EXCLAMATION: &str = "\u{f071}"; //

    // Technology / Brands
    pub const GITHUB: &str = "\u{f09b}"; //
    pub const GLOBE: &str = "\u{f0ac}"; //
    pub const ENVELOPE: &str = "\u{f0e0}"; //
    pub const PHONE: &str = "\u{f095}"; //
    pub const FOLDER: &str = "\u{f07b}"; //
    pub const TAG: &str = "\u{f02b}"; //
    pub const TAGS: &str = "\u{f02c}"; //

    // Devices
    pub const DESKTOP: &str = "\u{f108}"; //
    pub const LAPTOP: &str = "\u{f109}"; //
    pub const MOBILE: &str = "\u{f10b}"; //

    // Entry types
    pub const CREDIT_CARD: &str = "\u{f09d}"; //
    pub const ID_CARD: &str = "\u{f2c2}"; //
    pub const STICKY_NOTE: &str = "\u{f249}"; //
    pub const CERTIFICATE: &str = "\u{f0a3}"; //
    pub const WIFI: &str = "\u{f1eb}"; //
    pub const SERVER: &str = "\u{f233}"; //

    // Misc
    pub const CLOCK: &str = "\u{f017}"; //
    pub const CALENDAR: &str = "\u{f073}"; //
    pub const INFO: &str = "\u{f129}"; //
    pub const QUESTION: &str = "\u{f128}"; //
    pub const STAR: &str = "\u{f005}"; //
    pub const FLOWER: &str = "\u{e240}"; //  (dev icon, closest to lilypad)
    pub const LEAF: &str = "\u{f06c}"; //
    pub const SORT: &str = "\u{f0dc}"; //
}

/// Create a centered Nerd Font icon element.
///
/// Nerd Font glyphs have asymmetric left/right side bearings within their monospace
/// advance width, causing icons to appear shifted right. We compensate by applying
/// extra right padding to nudge the glyph left within the container.
pub fn centered_icon<M: 'static>(glyph: &str, size: f32) -> Element<'static, M> {
    let nudge = size * 0.18;
    container(text(glyph.to_string()).size(size).font(FONT_REGULAR))
        .width(Length::Fixed(size + nudge))
        .padding(iced::Padding {
            top: 0.0,
            right: nudge,
            bottom: 0.0,
            left: 0.0,
        })
        .align_x(Horizontal::Center)
        .into()
}

/// Create a centered Nerd Font icon element with an explicit color.
pub fn centered_icon_colored<M: 'static>(
    glyph: &str,
    size: f32,
    color: Color,
) -> Element<'static, M> {
    let nudge = size * 0.18;
    container(
        text(glyph.to_string())
            .size(size)
            .font(FONT_REGULAR)
            .color(color),
    )
    .width(Length::Fixed(size + nudge))
    .padding(iced::Padding {
        top: 0.0,
        right: nudge,
        bottom: 0.0,
        left: 0.0,
    })
    .align_x(Horizontal::Center)
    .into()
}
