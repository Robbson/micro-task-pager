//! Pure pager geometry in device-independent pixels (DIPs).
//!
//! The layout is shared by painting and hit testing, so a click can never land
//! somewhere different from what the user sees.
//!
//! # Coordinate spaces
//!
//! Three spaces meet here and it helps to know which is which:
//!
//! - **Physical pixels**: monitor and window rectangles from Win32. They are
//!   absolute virtual-screen coordinates and change with the monitor DPI.
//! - **Device pixels inside the pager**: what the C# renderer draws in. Borders
//!   and icons look sharp only when they land on whole device pixels.
//! - **DIPs**: what reactor and WinUI use for element positions and sizes.
//!
//! [`build`] converts physical monitor rectangles to DIPs, rounds every tile and
//! icon edge to a whole device pixel using the window `scale`, and divides the
//! result back by `scale`, so the view positions DIPs that land exactly on the
//! device-pixel grid. Hit testing then runs in the same DIP space.
//!
//! The geometry follows the C# `ScreenRenderer`, with the deviations recorded
//! in `ARCHITECTURE.md` §8: per-`(desktop, screen)` view modes, DIP units, and
//! preview rectangles clipped to their monitor before they are positioned.

use windows::Win32::Foundation::HWND;

use crate::monitors::MonitorInfo;
use crate::settings::{DesktopBorderStyle, Settings, ViewMode};
use crate::windows::{WindowInfo, WindowSnapshot};

/// The icon edge in DIPs, matching the C# 16 px icon at 100%.
pub const ICON_SIZE: f64 = 16.0;
/// Border and separator thickness in DIPs.
pub const BORDER_SIZE: f64 = 1.0;
/// Overflow label size in DIPs.
pub const OVERFLOW_FONT_SIZE: f64 = 12.0;

/// A rectangle in pager client DIPs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Rect {
    pub fn new(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn width(self) -> f64 {
        self.right - self.left
    }

    pub fn height(self) -> f64 {
        self.bottom - self.top
    }

    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    /// Intersection of two rectangles, or `None` when they do not overlap.
    pub fn intersect(self, other: Self) -> Option<Self> {
        let left = self.left.max(other.left);
        let top = self.top.max(other.top);
        let right = self.right.min(other.right);
        let bottom = self.bottom.min(other.bottom);

        (right > left && bottom > top).then_some(Self {
            left,
            top,
            right,
            bottom,
        })
    }

    pub fn translate(self, dx: f64, dy: f64) -> Self {
        Self {
            left: self.left + dx,
            top: self.top + dy,
            right: self.right + dx,
            bottom: self.bottom + dy,
        }
    }
}

/// One clickable cell in a screen.
#[derive(Clone, Debug)]
pub struct CellLayout {
    pub hwnd: HWND,
    pub rect: Rect,
    /// Pixel-snapped icon rectangle inside the cell.
    pub icon: Rect,
    pub is_active: bool,
}

/// The `+N` tile shown by Micro Taskbar when not every window fits.
#[derive(Clone, Debug)]
pub struct OverflowLayout {
    pub rect: Rect,
    pub text: String,
}

/// Layout for one physical monitor inside one desktop column.
#[derive(Clone, Debug)]
pub struct ScreenLayout {
    pub index: usize,
    pub x: f64,
    pub width: f64,
    pub view_mode: ViewMode,
    pub cells: Vec<CellLayout>,
    pub overflow: Option<OverflowLayout>,
}

/// Layout for one virtual desktop column.
#[derive(Clone, Debug)]
pub struct DesktopLayout {
    pub index: u32,
    pub screens: Vec<ScreenLayout>,
}

/// The complete pager client area.
#[derive(Clone, Debug, Default)]
pub struct PagerLayout {
    pub width: f64,
    pub height: f64,
    /// Border and separator thickness in DIPs, snapped to whole device pixels.
    pub border: f64,
    pub desktop_width: f64,
    pub desktop_count: u32,
    pub desktops: Vec<DesktopLayout>,
}

/// What is under a point in the pager.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub desktop_index: u32,
    pub screen_index: usize,
    pub hwnd: Option<HWND>,
}

impl PagerLayout {
    /// Returns the desktop column at a client x coordinate.
    pub fn desktop_at(&self, x: f64) -> Option<u32> {
        if x < 0.0 || x >= self.width || self.desktop_width <= 0.0 {
            return None;
        }

        let index = (x / self.desktop_width).floor();
        if index < 0.0 || index >= f64::from(self.desktop_count) {
            None
        } else {
            Some(index as u32)
        }
    }

    /// Finds the desktop, screen, and window cell at a client point.
    pub fn hit_test(&self, x: f64, y: f64) -> Option<Hit> {
        let desktop_index = self.desktop_at(x)?;
        if y < 0.0 || y >= self.height {
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
            .find(|cell| cell.rect.contains(x, y))
            .map(|cell| cell.hwnd);

        Some(Hit {
            desktop_index,
            screen_index,
            hwnd,
        })
    }
}

/// Builds the pager layout from a snapshot, monitors, and nested view modes.
///
/// Coordinates are snapped to whole physical pixels. WinUI antialiases
/// fractional DIP positions, which blurs 1 px borders and resamples icons, so
/// the reactor layout is computed on the same pixel grid as the C# renderer and
/// converted back to DIPs. `dpi` is the window or system DPI (96 = 100%).
pub fn build(
    snapshot: &WindowSnapshot,
    monitors: &[MonitorInfo],
    view_modes: &[Vec<ViewMode>],
    settings: &Settings,
    dpi: u32,
) -> PagerLayout {
    let scale = f64::from(dpi.max(96)) / 96.0;
    let pager_height_px = ((f64::from(settings.pager_height.max(1)) * scale).round() as i32).max(1);
    let pager_height = f64::from(pager_height_px) / scale;
    let border_px = (BORDER_SIZE * scale).round().max(1.0);
    let border = border_px / scale;
    let desktop_count = snapshot.desktop_count.max(1);

    if monitors.is_empty() {
        return PagerLayout {
            width: 1.0,
            height: pager_height,
            border,
            desktop_width: 1.0,
            desktop_count,
            desktops: Vec::new(),
        };
    }

    let screen_count = monitors.len();
    let screen_widths: Vec<f64> = monitors
        .iter()
        .map(|monitor| screen_width_px(monitor, pager_height_px) as f64 / scale)
        .collect();
    let desktop_width = screen_widths.iter().sum::<f64>().max(1.0);
    let width = desktop_width * f64::from(desktop_count);

    let mut desktops = Vec::with_capacity(desktop_count as usize);

    for desktop_index in 0..desktop_count {
        let mut screens = Vec::with_capacity(screen_count);
        let desktop_x = desktop_width * f64::from(desktop_index);
        // Screen and cell coordinates are absolute client coordinates. Keeping
        // them local to the desktop would make desktop 1 draw over desktop 0.
        let mut screen_x = desktop_x;

        for (screen_index, monitor) in monitors.iter().enumerate() {
            let screen_width = screen_widths[screen_index];
            let view_mode = view_modes
                .get(desktop_index as usize)
                .and_then(|modes| modes.get(screen_index))
                .copied()
                .unwrap_or(settings.view_mode_default);

            let windows: Vec<&WindowInfo> = snapshot
                .windows_on_desktop(desktop_index)
                .filter(|window| !window.is_minimized)
                .filter(|window| monitor_for_window(window, monitors) == Some(screen_index))
                .collect();

            let (cells, overflow) = match view_mode {
                ViewMode::Preview => (
                    preview_cells(
                        &windows,
                        monitor,
                        screen_x,
                        pager_height,
                        screen_width,
                        scale,
                        border,
                    ),
                    None,
                ),
                ViewMode::MicroTaskbar => micro_taskbar_cells(
                    &windows,
                    screen_x,
                    screen_width,
                    pager_height,
                    settings,
                    scale,
                ),
            };

            screens.push(ScreenLayout {
                index: screen_index,
                x: screen_x,
                width: screen_width,
                view_mode,
                cells,
                overflow,
            });
            screen_x += screen_width;
        }

        desktops.push(DesktopLayout {
            index: desktop_index,
            screens,
        });
    }

    PagerLayout {
        width,
        height: pager_height,
        border,
        desktop_width,
        desktop_count,
        desktops,
    }
}

fn preview_cells(
    windows: &[&WindowInfo],
    monitor: &MonitorInfo,
    screen_x: f64,
    pager_height: f64,
    screen_width: f64,
    scale: f64,
    border: f64,
) -> Vec<CellLayout> {
    let reduction = pager_height / f64::from(monitor.height().max(1));
    let screen_rect = Rect::new(0.0, 0.0, screen_width, pager_height);
    let mut sorted = windows.to_vec();
    sorted.sort_by_key(|window| window.z_order);

    sorted
        .into_iter()
        .filter_map(|window| {
            let left = snap(
                f64::from(window.rect.left - monitor.rect.left) * reduction,
                scale,
            );
            let top = snap(
                f64::from(window.rect.top - monitor.rect.top) * reduction,
                scale,
            );
            let right = snap(
                left + (f64::from(window.width()) * reduction - border).max(0.0),
                scale,
            );
            let bottom = snap(
                top + (f64::from(window.height()) * reduction - border).max(0.0),
                scale,
            );

            let cell = Rect::new(left, top, right, bottom);
            let cell = cell.intersect(screen_rect)?.translate(screen_x, 0.0);

            Some(CellLayout {
                hwnd: window.hwnd,
                rect: cell,
                icon: centered_icon(cell, scale),
                is_active: window.is_active,
            })
        })
        .collect()
}

fn micro_taskbar_cells(
    windows: &[&WindowInfo],
    screen_x: f64,
    screen_width: f64,
    height: f64,
    settings: &Settings,
    scale: f64,
) -> (Vec<CellLayout>, Option<OverflowLayout>) {
    let mut sorted = windows.to_vec();
    sorted.sort_by_key(|window| (window.process_id, window.hwnd.0 as usize));

    // The C# renderer positions tiles with integer pixel arithmetic. Using the
    // same grid keeps icon columns aligned and avoids half-pixel resampling.
    let icon_px = (ICON_SIZE * scale).round().max(1.0) as i64;
    let padding_x_px = (f64::from(settings.icon_padding_x) * scale).round() as i64;
    let padding_y_px = (f64::from(settings.icon_padding_y) * scale).round() as i64;
    let tile_width_px = (icon_px + padding_x_px).max(1);
    let tile_height_px = (icon_px + padding_y_px).max(1);
    let screen_width_px = (screen_width * scale).round().max(1.0) as i64;
    let height_px = (height * scale).round().max(1.0) as i64;

    let icons = sorted.len() as i64;
    let columns = (screen_width_px / tile_width_px).max(1);
    let rows = (height_px / tile_height_px).max(1);

    let mut remaining = 0i64;
    let center_x_px;
    let center_y_px;

    if icons > columns {
        let required_rows = (icons + columns - 1) / columns;
        remaining = icons % columns;

        if required_rows > rows {
            remaining = icons - columns * rows;
            center_y_px = (height_px % tile_height_px) / 2;
        } else {
            center_y_px = (height_px % (tile_height_px * required_rows)) / 2;
        }

        center_x_px = (screen_width_px % tile_width_px) / 2;
    } else {
        center_x_px = (screen_width_px - tile_width_px * icons) / 2;
        center_y_px = (height_px - tile_height_px) / 2;
    }

    let root_x_px = (screen_x * scale).round() as i64 + center_x_px;
    let root_y_px = center_y_px;
    let mut cells = Vec::new();
    let mut overflow = None;
    let mut col = 0i64;
    let mut row = 0i64;

    for window in sorted {
        let left_px = root_x_px + col * tile_width_px;
        let top_px = root_y_px + row * tile_height_px;
        let rect = physical_rect(left_px, top_px, tile_width_px, tile_height_px, scale);

        if remaining > 0 && col == columns - 1 && row == rows - 1 {
            overflow = Some(OverflowLayout {
                rect,
                text: format!("+{}", remaining + 1),
            });
            break;
        }

        let icon = physical_rect(
            left_px + tile_width_px / 2 - icon_px / 2,
            top_px + tile_height_px / 2 - icon_px / 2,
            icon_px,
            icon_px,
            scale,
        );

        cells.push(CellLayout {
            hwnd: window.hwnd,
            rect,
            icon,
            is_active: window.is_active,
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

/// Snaps a DIP value to the nearest whole device pixel.
fn snap(value: f64, scale: f64) -> f64 {
    (value * scale).round() / scale
}

/// Width of a monitor in the pager, in whole physical pixels.
fn screen_width_px(monitor: &MonitorInfo, height_px: i32) -> i64 {
    (i64::from(monitor.width().max(1)) * i64::from(height_px) / i64::from(monitor.height().max(1)))
        .max(1)
}

/// Converts a physical-pixel rectangle to DIPs.
fn physical_rect(left: i64, top: i64, width: i64, height: i64, scale: f64) -> Rect {
    let left = left as f64 / scale;
    let top = top as f64 / scale;
    Rect::new(
        left,
        top,
        left + width as f64 / scale,
        top + height as f64 / scale,
    )
}

/// Centers the 16 DIP icon inside a cell on the device pixel grid.
///
/// The result is clipped to the cell so a small preview window cannot draw its
/// icon over a neighboring cell.
fn centered_icon(cell: Rect, scale: f64) -> Rect {
    let icon_px = (ICON_SIZE * scale).round().max(1.0) as i64;
    let left_px = (cell.left * scale).round() as i64;
    let top_px = (cell.top * scale).round() as i64;
    let width_px = (cell.width() * scale).round() as i64;
    let height_px = (cell.height() * scale).round() as i64;

    let icon_left = left_px + width_px / 2 - icon_px / 2;
    let icon_top = top_px + height_px / 2 - icon_px / 2;
    let left = icon_left.max(left_px);
    let top = icon_top.max(top_px);
    let right = (icon_left + icon_px).min(left_px + width_px);
    let bottom = (icon_top + icon_px).min(top_px + height_px);

    if right <= left || bottom <= top {
        return Rect::default();
    }

    physical_rect(left, top, right - left, bottom - top, scale)
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

/// Border color for a desktop, used by the view and by tests.
pub fn desktop_border_color(
    snapshot: &WindowSnapshot,
    desktop_index: u32,
    settings: &Settings,
) -> u32 {
    if desktop_index == snapshot.current_desktop {
        settings.active_desktop_border_color
    } else {
        settings.desktop_border_color
    }
}

/// Whether the desktop border uses an underline instead of a box.
pub fn desktop_border_style(settings: &Settings) -> DesktopBorderStyle {
    settings.desktop_border_style
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::c_void;
    use windows::Win32::Foundation::RECT;
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
            hwnd: HWND(id as *mut c_void),
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

    fn modes(mode: ViewMode, desktops: usize, screens: usize) -> Vec<Vec<ViewMode>> {
        vec![vec![mode; screens]; desktops]
    }

    #[test]
    fn size_is_snapped_to_whole_pixels() {
        let layout = build(
            &snapshot(Vec::new(), 2),
            &[monitor()],
            &modes(ViewMode::MicroTaskbar, 2, 1),
            &settings(ViewMode::MicroTaskbar),
            96,
        );

        // 1920 * 40 / 1080 = 71.11 px; the C# renderer truncates to 71.
        assert_eq!(layout.height, 40.0);
        assert_eq!(layout.desktop_width, 71.0);
        assert_eq!(layout.width, 142.0);
        assert_eq!(layout.border, 1.0);
        assert_eq!(layout.desktops.len(), 2);
    }

    #[test]
    fn coordinates_land_on_the_device_pixel_grid() {
        let mut scaled = monitor();
        scaled.rect.right = 2560;
        scaled.rect.bottom = 1440;
        let layout = build(
            &snapshot(Vec::new(), 2),
            &[scaled],
            &modes(ViewMode::MicroTaskbar, 2, 1),
            &settings(ViewMode::MicroTaskbar),
            144,
        );

        // 2560 * 60 / 1440 = 106.67 px, truncated to 106 for the desktop width.
        assert_eq!(layout.height, 40.0);
        assert!((layout.desktop_width - 106.0 / 1.5).abs() < 1e-9);
        assert!((layout.border - 2.0 / 1.5).abs() < 1e-9);
        assert!((layout.width - 2.0 * 106.0 / 1.5).abs() < 1e-9);
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
            &modes(ViewMode::Preview, 1, 1),
            &settings(ViewMode::Preview),
            96,
        );

        let hit = layout
            .hit_test(10.0, 10.0)
            .expect("point is inside the preview");

        assert_eq!(hit.desktop_index, 0);
        assert_eq!(hit.screen_index, 0);
        assert!(hit.hwnd.is_some());
    }

    #[test]
    fn preview_orders_cells_back_to_front_and_hit_tests_the_top_window() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 960,
            bottom: 540,
        };
        let lower = window(1, 0, rect);
        let mut upper = window(2, 0, rect);
        upper.is_active = true;

        let layout = build(
            &snapshot(vec![lower.clone(), upper.clone()], 1),
            &[monitor()],
            &modes(ViewMode::Preview, 1, 1),
            &settings(ViewMode::Preview),
            96,
        );

        let cells = &layout.desktops[0].screens[0].cells;
        assert_eq!(cells[0].hwnd, lower.hwnd);
        assert_eq!(cells[1].hwnd, upper.hwnd);
        assert!(cells[1].is_active);
        assert_eq!(
            layout.hit_test(10.0, 10.0).and_then(|hit| hit.hwnd),
            Some(upper.hwnd)
        );
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
            &modes(ViewMode::MicroTaskbar, 1, 1),
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
            &modes(ViewMode::MicroTaskbar, 2, 1),
            &settings(ViewMode::MicroTaskbar),
            96,
        );

        let cell = &layout.desktops[1].screens[0].cells[0];

        assert!(cell.rect.left >= layout.desktop_width);
    }

    #[test]
    fn per_desktop_view_mode_is_used_for_hit_testing() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 960,
            bottom: 540,
        };
        let modes = vec![vec![ViewMode::MicroTaskbar], vec![ViewMode::Preview]];
        let layout = build(
            &snapshot(vec![window(1, 1, rect)], 2),
            &[monitor()],
            &modes,
            &settings(ViewMode::MicroTaskbar),
            96,
        );

        assert_eq!(
            layout.desktops[0].screens[0].view_mode,
            ViewMode::MicroTaskbar
        );
        assert_eq!(layout.desktops[1].screens[0].view_mode, ViewMode::Preview);
        assert!(layout.desktops[1].screens[0].cells[0].rect.width() > 20.0);
    }

    #[test]
    fn preview_cells_are_clipped_to_the_monitor() {
        let rect = RECT {
            left: -500,
            top: 0,
            right: 500,
            bottom: 100,
        };
        let layout = build(
            &snapshot(vec![window(1, 0, rect)], 1),
            &[monitor()],
            &modes(ViewMode::Preview, 1, 1),
            &settings(ViewMode::Preview),
            96,
        );

        let cell = &layout.desktops[0].screens[0].cells[0];
        assert!(cell.rect.left >= 0.0);
        assert!(cell.rect.right <= layout.desktop_width);
    }
}
