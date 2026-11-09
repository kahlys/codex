//! The colour palette, in one place.
//!
//! Colours are `0xRRGGBB` literals meant to be wrapped with [`gpui::rgb`] at
//! the use site: `.bg(rgb(theme::SURFACE))`. Keeping them here means the look
//! of the app can be changed without hunting through the page code. (The
//! shades are Catppuccin Mocha-like: a dark, low-contrast theme.)

/// Window background.
pub const WINDOW: u32 = 0x1e1e2e;
/// Cards and list rows.
pub const SURFACE: u32 = 0x181825;
/// A row or card under the pointer.
pub const SURFACE_HOVER: u32 = 0x26263a;
/// Text on an accent-coloured element.
pub const ON_ACCENT: u32 = 0x1e1e2e;

/// The "search" field at the top of the list page.
pub const FIELD: u32 = 0x313244;
/// The field under the pointer.
pub const FIELD_HOVER: u32 = 0x3b3f5c;

/// Buttons, and the outline around the field.
pub const CONTROL: u32 = 0x45475a;
/// A button under the pointer.
pub const CONTROL_HOVER: u32 = 0x585b70;

/// Highlights: links, headings, the primary button.
pub const ACCENT: u32 = 0x89b4fa;

/// Body text.
pub const TEXT: u32 = 0xcdd6f4;
/// Secondary text: the status line, the loading message.
pub const TEXT_MUTED: u32 = 0x9399b2;
/// Tertiary text: hints, labels, the folder under a file name.
pub const TEXT_FAINT: u32 = 0x7f849c;
/// Quietest text: the footer brand.
pub const TEXT_DIM: u32 = 0x585b70;

/// Errors.
pub const DANGER: u32 = 0xf38ba8;
