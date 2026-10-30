//! Pager layout and drawing instructions.
//!
//! This module does not touch Windows drawing APIs directly. It turns a
//! [`WindowSnapshot`] plus monitor geometry into a [`PagerLayout`] and then
//! emits simple drawing calls through the [`Painter`] trait. The GDI backend
//! lives in `gdi.rs`.
//!
//! Separating layout from painting has two benefits:
//!
//! - The same layout is used for drawing and for hit testing, so a click can
//!   never land somewhere different from what was painted.
//! - Layout math can be unit-tested with a fake painter and no window.
//!
//! The geometry follows `ScreenRenderer.cs` and `Desktop.cs`. In particular,
//! each monitor's pager width is its aspect-ratio width at the configured pager
//! height, so the pager keeps the monitor's shape instead of using the raw
//! pixel width.

pub mod gdi;

use windows::Win32::Foundation::{HWND, RECT};

use crate::monitors::MonitorInfo;
use crate::settings::{DesktopBorderStyle, Settings, ViewMode};
use crate::windows::{WindowIcon, WindowInfo, WindowSnapshot};

/// Scales a logical value to physical pixels (system DPI, 96 = 100%).
pub fn scale(value: i32, dpi: u32) -> i32 {
    ((value as f32) * (dpi as f32) / 96.0).round() as i32
}

/// Drawing operations the layout needs.
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

/// One clickable cell in a screen.
#[derive(Clone, Debug)]
pub struct CellLayout {
    pub hwnd: HWND,
    pub rect: RECT,
    pub is_active: bool,
    pub icon: Option<WindowIcon>,
}

/// The `+N` tile shown by Micro Taskbar when not every window fits.
#[derive(Clone, Debug)]
pub struct OverflowLayout {
    pub rect: RECT,
    pub text: String,
}

/// Layout for one physical monitor inside one desktop column.
#[derive(Clone, Debug)]
pub struct ScreenLayout {
    pub index: usize,
    pub x: i32,
    pub width: i32,
    pub height: i32,
    pub view_mode: ViewMode,
    pub cells: Vec<CellLayout>,
    pub overflow: Option<OverflowLayout>,
}

/// Layout for one virtual desktop column.
#[derive(Clone, Debug)]
pub struct DesktopLayout {
    pub index: u32,
    pub x: i32,
    pub width: i32,
    pub screens: Vec<ScreenLayout>,
}

/// The complete pager client area.
#[derive(Clone, Debug, Default)]
pub struct PagerLayout {
    pub width: i32,
    pub height: i32,
    pub desktop_width: i32,
    pub desktop_count: u32,
    pub screen_count: usize,
    pub desktops: Vec<DesktopLayout>,
}

/// What is under a point in the pager.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub desktop_index: u32,
    pub screen_index: usize,
    pub hwnd: Option<HWND>,
}

impl PagerLayout {
    /// Returns the desktop column at a client x coordinate.
    pub fn desktop_at(&self, x: i32) -> Option<u32> {
        if x < 0 || x >= self.width || self.desktop_width <= 0 {
            return None;
        }

        let index = x / self.desktop_width;
        if index < 0 || index as u32 >= self.desktop_count {
            None
        } else {
            Some(index as u32)
        }
    }

    /// Finds the desktop, screen, and window cell at a client point.
    pub fn hit_test(&self, x: i32, y: i32) -> Option<Hit> {
        let desktop_index = self.desktop_at(x)?;

        if y < 0 || y >= self.height {
            return None;
        }

        let desktop = self.desktops.get(desktop_index as usize)?;
        let screen_index = desktop
            .screens
            .iter()
            .position(|screen| x >= screen.x && x < screen.x + screen.width)
            .unwrap_or(0);
        let screen = desktop.screens.get(screen_index)?;

        // Preview cells are stored back-to-front, so iterating in reverse finds
        // the topmost window. Micro Taskbar cells do not overlap, so the same
        // loop is harmless there.
        let hwnd = screen
            .cells
            .iter()
            .rev()
            .find(|cell| rect_contains(cell.rect, x, y))
            .map(|cell| cell.hwnd);

        Some(Hit {
            desktop_index,
            screen_index,
            hwnd,
        })
    }

    /// Returns the screen layout that contains a point.
    pub fn screen_at(&self, desktop_index: u32, x: i32) -> Option<&ScreenLayout> {
        let desktop = self.desktops.get(desktop_index as usize)?;
        desktop
            .screens
            .iter()
            .find(|screen| x >= screen.x && x < screen.x + screen.width)
    }
}

/// Builds the pager layout from a snapshot and the current monitors.
pub fn build(
    snapshot: &WindowSnapshot,
    monitors: &[MonitorInfo],
    view_modes: &[ViewMode],
    settings: &Settings,
    dpi: u32,
) -> PagerLayout {
    if monitors.is_empty() {
        return PagerLayout::default();
    }

    let pager_height = scale(settings.pager_height, dpi).max(1);
    let border = scale(1, dpi).max(1);
    let screen_count = monitors.len();

    let screen_widths: Vec<i32> = monitors
        .iter()
        .map(|monitor| aspect_width(monitor, pager_height))
        .collect();
    let desktop_width = screen_widths.iter().sum::<i32>().max(1);
    let desktop_count = snapshot.desktop_count;
    let width = desktop_width
        .saturating_mul(desktop_count as i32)
        .max(desktop_width);

    let mut desktops = Vec::with_capacity(desktop_count as usize);

    for desktop_index in 0..desktop_count {
        let mut screens = Vec::with_capacity(screen_count);
        let desktop_x = desktop_width * desktop_index as i32;
        // Screen and cell coordinates are absolute client coordinates. Keeping
        // them local to the desktop would make desktop 1 draw over desktop 0.
        let mut screen_x = desktop_x;

        for (screen_index, monitor) in monitors.iter().enumerate() {
            let screen_width = screen_widths[screen_index];
            let mode_index = (desktop_index as usize)
                .saturating_mul(screen_count)
                .saturating_add(screen_index);
            let view_mode = view_modes
                .get(mode_index)
                .copied()
                .unwrap_or(settings.view_mode_default);

            let windows: Vec<&WindowInfo> = snapshot
                .windows_on_desktop(desktop_index)
                .filter(|window| !window.is_minimized)
                .filter(|window| monitor_for_window(window, monitors) == Some(screen_index))
                .collect();

            let (cells, overflow) = match view_mode {
                ViewMode::Preview => (
                    preview_cells(&windows, monitor, screen_x, pager_height, border),
                    None,
                ),
                ViewMode::MicroTaskbar => micro_taskbar_cells(
                    &windows,
                    screen_x,
                    screen_width,
                    pager_height,
                    settings,
                    dpi,
                ),
            };

            screens.push(ScreenLayout {
                index: screen_index,
                x: screen_x,
                width: screen_width,
                height: pager_height,
                view_mode,
                cells,
                overflow,
            });
            screen_x += screen_width;
        }

        desktops.push(DesktopLayout {
            index: desktop_index,
            x: desktop_x,
            width: desktop_width,
            screens,
        });
    }

    PagerLayout {
        width,
        height: pager_height,
        desktop_width,
        desktop_count,
        screen_count,
        desktops,
    }
}

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

fn preview_cells(
    windows: &[&WindowInfo],
    monitor: &MonitorInfo,
    screen_x: i32,
    pager_height: i32,
    border: i32,
) -> Vec<CellLayout> {
    let scale = pager_height as f32 / monitor.height().max(1) as f32;
    let mut sorted = windows.to_vec();
    sorted.sort_by_key(|window| window.z_order);

    sorted
        .into_iter()
        .filter_map(|window| {
            let left = screen_x + ((window.rect.left - monitor.rect.left) as f32 * scale) as i32;
            let top = ((window.rect.top - monitor.rect.top) as f32 * scale) as i32;
            let width = ((window.width() - border) as f32 * scale) as i32;
            let height = ((window.height() - border) as f32 * scale) as i32;

            if width <= 0 || height <= 0 {
                return None;
            }

            Some(CellLayout {
                hwnd: window.hwnd,
                rect: RECT {
                    left,
                    top,
                    right: left + width,
                    bottom: top + height,
                },
                is_active: window.is_active,
                icon: window.icon,
            })
        })
        .collect()
}

fn micro_taskbar_cells(
    windows: &[&WindowInfo],
    screen_x: i32,
    screen_width: i32,
    height: i32,
    settings: &Settings,
    dpi: u32,
) -> (Vec<CellLayout>, Option<OverflowLayout>) {
    let mut sorted = windows.to_vec();
    sorted.sort_by_key(|window| (window.process_id, window.hwnd.0 as usize));

    let icon_size = scale(16, dpi).max(1);
    let icon_padding_x = scale(settings.icon_padding_x, dpi);
    let icon_padding_y = scale(settings.icon_padding_y, dpi);
    let tile_width = (icon_size + icon_padding_x).max(1);
    let tile_height = (icon_size + icon_padding_y).max(1);

    let icons = sorted.len();
    let columns = (screen_width / tile_width).max(1);
    let rows = (height / tile_height).max(1);

    let mut remaining = 0usize;
    let center_x;
    let center_y;

    if icons > columns as usize {
        let required_rows = icons.div_ceil(columns as usize);
        remaining = icons % columns as usize;

        if required_rows > rows as usize {
            remaining = icons - columns as usize * rows as usize;
            center_y = (height % tile_height) / 2;
        } else {
            center_y = (height % (tile_height * required_rows as i32)) / 2;
        }

        center_x = (screen_width % tile_width) / 2;
    } else {
        center_x = (screen_width - tile_width * icons as i32) / 2;
        center_y = (height - tile_height) / 2;
    }

    let root_x = screen_x + center_x;
    let root_y = center_y;
    let mut cells = Vec::new();
    let mut overflow = None;
    let mut col = 0i32;
    let mut row = 0i32;

    for window in sorted {
        let rect = RECT {
            left: root_x + col * tile_width,
            top: root_y + row * tile_height,
            right: root_x + col * tile_width + tile_width,
            bottom: root_y + row * tile_height + tile_height,
        };

        if remaining > 0 && col == columns - 1 && row == rows - 1 {
            overflow = Some(OverflowLayout {
                rect,
                text: format!("+{}", remaining + 1),
            });
            break;
        }

        cells.push(CellLayout {
            hwnd: window.hwnd,
            rect,
            is_active: window.is_active,
            icon: window.icon,
        });

        if col < columns - 1 {
            col += 1;
        } else {
            row += 1;
            col = 0;
        }
    }

    (cells, overflow)
}

fn aspect_width(monitor: &MonitorInfo, pager_height: i32) -> i32 {
    (monitor.width() * pager_height / monitor.height().max(1)).max(1)
}

fn monitor_for_window(window: &WindowInfo, monitors: &[MonitorInfo]) -> Option<usize> {
    let center_x = window.rect.left + window.width() / 2;
    let center_y = window.rect.top + window.height() / 2;

    if let Some(index) = monitors
        .iter()
        .position(|monitor| monitor.contains(center_x, center_y))
    {
        return Some(index);
    }

    // Minimized and partially off-screen windows may have centers outside every
    // monitor. Pick the nearest monitor rectangle.
    monitors
        .iter()
        .enumerate()
        .min_by_key(|(_, monitor)| {
            let x = center_x.clamp(monitor.rect.left, monitor.rect.right);
            let y = center_y.clamp(monitor.rect.top, monitor.rect.bottom);
            let dx = center_x - x;
            let dy = center_y - y;
            dx * dx + dy * dy
        })
        .map(|(index, _)| index)
}

fn rect_contains(rect: RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::c_void;
    use windows::core::GUID;

    fn monitor() -> MonitorInfo {
        MonitorInfo {
            rect: RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            work_rect: RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            },
            primary: true,
        }
    }

    fn window(id: usize, desktop: u32, rect: RECT) -> WindowInfo {
        WindowInfo {
            hwnd: HWND(std::ptr::dangling_mut::<c_void>()),
            title: format!("Window {id}"),
            class_name: "TestWindow".to_string(),
            process_id: id as u32,
            rect,
            desktop_index: desktop,
            z_order: id,
            is_active: false,
            is_minimized: false,
            icon: None,
        }
    }

    fn snapshot(windows: Vec<WindowInfo>, desktops: u32) -> WindowSnapshot {
        WindowSnapshot {
            windows,
            desktop_ids: vec![GUID::zeroed(); desktops as usize],
            desktop_count: desktops,
            current_desktop: 0,
            foreground: None,
            hash: 0,
        }
    }

    fn settings(mode: ViewMode) -> Settings {
        Settings {
            pager_height: 40,
            view_mode_default: mode,
            ..Settings::default()
        }
    }

    #[test]
    fn monitor_width_scales_to_pager_height() {
        let layout = build(
            &snapshot(Vec::new(), 2),
            &[monitor()],
            &[],
            &settings(ViewMode::MicroTaskbar),
            96,
        );

        assert_eq!(layout.height, 40);
        assert_eq!(layout.desktop_width, 71);
        assert_eq!(layout.width, 142);
        assert_eq!(layout.desktops.len(), 2);
    }

    #[test]
    fn preview_hit_test_maps_back_to_the_window() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 960,
            bottom: 540,
        };
        let layout = build(
            &snapshot(vec![window(1, 0, rect)], 1),
            &[monitor()],
            &[],
            &settings(ViewMode::Preview),
            96,
        );

        let hit = layout
            .hit_test(10, 10)
            .expect("point is inside the preview");

        assert_eq!(hit.desktop_index, 0);
        assert_eq!(hit.screen_index, 0);
        assert!(hit.hwnd.is_some());
    }

    #[test]
    fn micro_taskbar_reports_overflow_count() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };
        let windows = (0..10).map(|index| window(index, 0, rect)).collect();
        let layout = build(
            &snapshot(windows, 1),
            &[monitor()],
            &[],
            &settings(ViewMode::MicroTaskbar),
            96,
        );

        let overflow = layout.desktops[0].screens[0]
            .overflow
            .as_ref()
            .expect("ten windows do not fit in eight tiles");

        assert_eq!(overflow.text, "+3");
    }

    #[test]
    fn second_desktop_cells_use_absolute_coordinates() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };
        let layout = build(
            &snapshot(vec![window(1, 1, rect)], 2),
            &[monitor()],
            &[],
            &settings(ViewMode::MicroTaskbar),
            96,
        );

        let cell = &layout.desktops[1].screens[0].cells[0];

        assert!(cell.rect.left >= layout.desktop_width);
    }
}
