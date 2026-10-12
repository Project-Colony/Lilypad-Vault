//! The Nerd Font glyphs Lilypad draws, and the helper that draws one.
//!
//! Glyphs are written as codepoints, never pasted, and always drawn in
//! `colony_ui::fonts::glyph_font()`: the dyslexia font has none of them.
//! Reference: <https://www.nerdfonts.com/cheat-sheet>.

use colony_ui::fonts::glyph_font;
use iced::alignment::Horizontal;
use iced::widget::{container, text};
use iced::{Color, Element, Length};

pub const LOCK: &str = "\u{f023}";
pub const VAULT: &str = LOCK;
pub const KEY: &str = "\u{f084}";
pub const SHIELD: &str = "\u{f132}";
pub const PLUS: &str = "\u{f067}";
pub const WAND: &str = "\u{f0d0}";
pub const SYNC: &str = "\u{f021}";
pub const USER: &str = "\u{f007}";
pub const COPY: &str = "\u{f0c5}";
pub const EDIT: &str = "\u{f044}";
pub const TRASH: &str = "\u{f1f8}";
pub const DOWNLOAD: &str = "\u{f019}";
pub const UPLOAD: &str = "\u{f093}";
pub const EXTERNAL_LINK: &str = "\u{f08e}";
pub const CIRCLE: &str = "\u{f111}";
pub const CIRCLE_CHECK: &str = "\u{f058}";
pub const TRIANGLE_EXCLAMATION: &str = "\u{f071}";
pub const GITHUB: &str = "\u{f09b}";
pub const FOLDER: &str = "\u{f07b}";
pub const TAG: &str = "\u{f02b}";
pub const CREDIT_CARD: &str = "\u{f09d}";
pub const ID_CARD: &str = "\u{f2c2}";
pub const STICKY_NOTE: &str = "\u{f249}";
pub const CERTIFICATE: &str = "\u{f0a3}";
pub const WIFI: &str = "\u{f1eb}";
pub const SERVER: &str = "\u{f233}";
pub const CLOCK: &str = "\u{f017}";
pub const STAR: &str = "\u{f005}";
pub const LEAF: &str = "\u{f06c}";
/// A water lily is not in the font; the dev-icons flower is the closest.
pub const FLOWER: &str = "\u{e240}";
pub const CHECKBOX_ON: &str = "\u{f14a}";
pub const CHECKBOX_OFF: &str = "\u{f096}";

/// A glyph at `sz(base)`, horizontally centred.
///
/// Nerd Font glyphs sit right of centre in their advance box, so a right
/// padding of 18% of the size nudges them back, which matters most in
/// icon-only buttons.
pub fn icon<'a, M: 'a>(glyph: &str, base: u16, color: Color) -> Element<'a, M> {
    let size = colony_ui::sz(base);
    let nudge = size * 0.18;
    container(
        text(glyph.to_string())
            .size(size)
            .font(glyph_font())
            .color(color),
    )
    .width(Length::Fixed(size + nudge))
    .padding(iced::Padding {
        right: nudge,
        ..iced::Padding::ZERO
    })
    .align_x(Horizontal::Center)
    .into()
}
