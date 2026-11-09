//! Pager layout and drawing instructions.
//!
//! This module does not touch Windows drawing APIs directly. It is split the
//! same way as the WinUI pager, even though the output is GDI instead of a
//! declarative view:
//!
//! - [`layout`] turns a [`crate::windows::WindowSnapshot`] plus monitor geometry
//!   into a [`PagerLayout`]. The same layout is used for painting and hit
//!   testing, so a click can never land somewhere different from what was
//!   painted. Layout math is unit-tested without a window.
//! - [`paint`] walks a finished [`PagerLayout`] and emits simple drawing calls
//!   through the [`Painter`] trait. It contains no platform code.
//! - [`gdi`] implements [`Painter`] with a double-buffered GDI memory DC.
//!
//! Keeping layout, drawing instructions, and the GDI backend separate means the
//! geometry and the paint order can be read and tested without a window, and
//! the backend can be swapped without touching either.
//!
//! The geometry follows `ScreenRenderer.cs` and `Desktop.cs`. In particular,
//! each monitor's pager width is its aspect-ratio width at the configured pager
//! height, so the pager keeps the monitor's shape instead of using the raw
//! pixel width.

pub mod gdi;
mod layout;
mod paint;

pub use layout::{
    CellLayout, DesktopLayout, Hit, OverflowLayout, PagerLayout, ScreenLayout, build,
};
pub use paint::paint_layout;

use windows::Win32::Foundation::RECT;

use crate::windows::WindowIcon;

/// Scales a logical value to physical pixels (system DPI, 96 = 100%).
pub fn scale(value: i32, dpi: u32) -> i32 {
    ((value as f32) * (dpi as f32) / 96.0).round() as i32
}

/// Drawing operations the layout needs.
///
/// Implemented by [`gdi::GdiPainter`]. The trait keeps [`paint`] free of Win32
/// types other than the `RECT` it receives from the layout.
pub trait Painter {
    /// Fills a rectangle with a 32-bit ARGB color.
    fn fill_rect(&mut self, rect: RECT, color: u32);
    /// Draws a rectangle outline with the given width.
    fn stroke_rect(&mut self, rect: RECT, color: u32, width: i32);
    /// Draws an icon centered at `(center_x, center_y)`.
    fn draw_icon(&mut self, icon: WindowIcon, center_x: i32, center_y: i32, size: i32);
    /// Draws centered text inside `rect`.
    fn draw_text_centered(&mut self, text: &str, rect: RECT, color: u32, size: i32);
    /// Restricts drawing to `rect` until the matching `pop_clip`.
    fn push_clip(&mut self, rect: RECT);
    /// Restores the previous clipping region.
    fn pop_clip(&mut self);
}
