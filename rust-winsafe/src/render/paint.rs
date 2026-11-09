//! Drawing instructions for a finished [`PagerLayout`].
//!
//! [`paint_layout`] walks the layout in a fixed order and emits calls through a
//! [`Painter`]. It knows nothing about GDI, double buffering, or window handles,
//! so the paint order can be reviewed without platform code. The backend lives
//! in [`super::gdi`].
//!
//! The order matters: the background is filled first, then each screen clips
//! and paints its cells, then separators and desktop borders are drawn on top.
//! Painting a cell outside its screen is impossible because each screen pushes a
//! clip rectangle.

use windows::Win32::Foundation::RECT;

use crate::settings::{DesktopBorderStyle, Settings, ViewMode};
use crate::windows::WindowSnapshot;

use super::layout::{CellLayout, PagerLayout};
use super::{Painter, scale};

/// Emits all drawing calls for one frame.
pub fn paint_layout(
    painter: &mut dyn Painter,
    layout: &PagerLayout,
    snapshot: &WindowSnapshot,
    settings: &Settings,
    dpi: u32,
) {
    let border = scale(1, dpi).max(1);
    let icon_size = scale(16, dpi).max(1);

    painter.fill_rect(
        RECT {
            left: 0,
            top: 0,
            right: layout.width,
            bottom: layout.height,
        },
        settings.desktop_background_color,
    );

    for desktop in &layout.desktops {
        for (screen_index, screen) in desktop.screens.iter().enumerate() {
            let clip = RECT {
                left: screen.x,
                top: 0,
                right: screen.x + screen.width,
                bottom: layout.height,
            };
            painter.push_clip(clip);

            match screen.view_mode {
                ViewMode::Preview => {
                    for cell in &screen.cells {
                        paint_preview_cell(painter, cell, settings, border, icon_size);
                    }
                }
                ViewMode::MicroTaskbar => {
                    for cell in &screen.cells {
                        paint_micro_cell(painter, cell, settings, icon_size);
                    }
                    if let Some(overflow) = &screen.overflow {
                        painter.draw_text_centered(
                            &overflow.text,
                            overflow.rect,
                            settings.active_window_border_color,
                            scale(12, dpi).max(1),
                        );
                    }
                }
            }

            painter.pop_clip();

            if screen_index + 1 < desktop.screens.len() {
                // C# uses Color.FromArgb(64, LightBlue), which blends with the
                // background. GDI FillRect has no alpha, so this constant is
                // the pre-blended result over the default desktop background.
                painter.fill_rect(
                    RECT {
                        left: screen.x + screen.width,
                        top: 0,
                        right: screen.x + screen.width + border,
                        bottom: layout.height,
                    },
                    0xFF5B_666A,
                );
            }
        }

        let border_color = if desktop.index == snapshot.current_desktop {
            settings.active_desktop_border_color
        } else {
            settings.desktop_border_color
        };

        match settings.desktop_border_style {
            DesktopBorderStyle::Box => {
                let inset = border / 2;
                painter.stroke_rect(
                    RECT {
                        left: desktop.x + inset,
                        top: inset,
                        right: desktop.x + desktop.width - border + inset,
                        bottom: layout.height - border + inset,
                    },
                    border_color,
                    border,
                );
            }
            DesktopBorderStyle::Underline => {
                painter.fill_rect(
                    RECT {
                        left: desktop.x,
                        top: layout.height - border,
                        right: desktop.x + desktop.width,
                        bottom: layout.height,
                    },
                    border_color,
                );
            }
        }
    }
}

/// Paints a preview cell: background, centered icon, then the window border.
fn paint_preview_cell(
    painter: &mut dyn Painter,
    cell: &CellLayout,
    settings: &Settings,
    border: i32,
    icon_size: i32,
) {
    let fill = if cell.is_active {
        settings.active_window_background_color
    } else {
        settings.window_background_color
    };
    let border_color = if cell.is_active {
        settings.active_window_border_color
    } else {
        settings.window_border_color
    };

    painter.fill_rect(cell.rect, fill);

    let width = cell.rect.right - cell.rect.left;
    let height = cell.rect.bottom - cell.rect.top;
    let center_x = cell.rect.left + width / 2;
    let center_y = cell.rect.top + height / 2;
    if let Some(icon) = cell.icon {
        painter.draw_icon(icon, center_x, center_y, icon_size);
    }

    let inset = border / 2;
    painter.stroke_rect(
        RECT {
            left: cell.rect.left + inset,
            top: cell.rect.top + inset,
            right: cell.rect.right - border + inset,
            bottom: cell.rect.bottom - border + inset,
        },
        border_color,
        border,
    );
}

/// Paints a Micro Taskbar tile: only the active window gets a background.
fn paint_micro_cell(
    painter: &mut dyn Painter,
    cell: &CellLayout,
    settings: &Settings,
    icon_size: i32,
) {
    if cell.is_active {
        painter.fill_rect(cell.rect, settings.active_window_background_color);
    }

    let width = cell.rect.right - cell.rect.left;
    let height = cell.rect.bottom - cell.rect.top;
    if let Some(icon) = cell.icon {
        painter.draw_icon(
            icon,
            cell.rect.left + width / 2,
            cell.rect.top + height / 2,
            icon_size,
        );
    }
}
