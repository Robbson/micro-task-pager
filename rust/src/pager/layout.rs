//! Pure pager geometry in device-independent pixels (DIPs).
//!
//! The layout is shared by painting and hit testing, so a click can never land
//! somewhere different from what the user sees. Monitor rectangles and window
//! rectangles are physical pixels; only their aspect ratio and relative
//! position are used, so the result is identical in DIPs without a DPI
//! parameter.
//!
//! The geometry follows the C# `ScreenRenderer`, with the deviations recorded
//! in `ARCHITECTURE.md` §12: per-`(desktop, screen)` view modes, DIP units, and
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
pub fn build(
    snapshot: &WindowSnapshot,
    monitors: &[MonitorInfo],
    view_modes: &[Vec<ViewMode>],
    settings: &Settings,
) -> PagerLayout {
    let pager_height = f64::from(settings.pager_height.max(1));
    let desktop_count = snapshot.desktop_count.max(1);

    if monitors.is_empty() {
        return PagerLayout {
            width: 1.0,
            height: pager_height,
            desktop_width: 1.0,
            desktop_count,
            desktops: Vec::new(),
        };
    }

    let screen_count = monitors.len();
    let screen_widths: Vec<f64> = monitors
        .iter()
        .map(|monitor| aspect_width(monitor, pager_height))
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
                    preview_cells(&windows, monitor, screen_x, pager_height),
                    None,
                ),
                ViewMode::MicroTaskbar => {
                    micro_taskbar_cells(&windows, screen_x, screen_width, pager_height, settings)
                }
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
) -> Vec<CellLayout> {
    let scale = pager_height / f64::from(monitor.height().max(1));
    let screen_rect = Rect::new(0.0, 0.0, aspect_width(monitor, pager_height), pager_height);
    let mut sorted = windows.to_vec();
    sorted.sort_by_key(|window| window.z_order);

    sorted
        .into_iter()
        .filter_map(|window| {
            let left = f64::from(window.rect.left - monitor.rect.left) * scale;
            let top = f64::from(window.rect.top - monitor.rect.top) * scale;
            let width = (f64::from(window.width()) * scale - BORDER_SIZE).max(0.0);
            let height = (f64::from(window.height()) * scale - BORDER_SIZE).max(0.0);

            let cell = Rect::new(left, top, left + width, top + height);
            let cell = cell.intersect(screen_rect)?.translate(screen_x, 0.0);

            Some(CellLayout {
                hwnd: window.hwnd,
                rect: cell,
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
) -> (Vec<CellLayout>, Option<OverflowLayout>) {
    let mut sorted = windows.to_vec();
    sorted.sort_by_key(|window| (window.process_id, window.hwnd.0 as usize));

    let icon_padding_x = f64::from(settings.icon_padding_x);
    let icon_padding_y = f64::from(settings.icon_padding_y);
    let tile_width = (ICON_SIZE + icon_padding_x).max(1.0);
    let tile_height = (ICON_SIZE + icon_padding_y).max(1.0);

    let icons = sorted.len();
    let columns = (screen_width / tile_width).floor().max(1.0);
    let rows = (height / tile_height).floor().max(1.0);

    let mut remaining = 0usize;
    let center_x;
    let center_y;

    if icons as f64 > columns {
        let required_rows = (icons as f64 / columns).ceil();
        remaining = icons % columns as usize;

        if required_rows > rows {
            remaining = icons - columns as usize * rows as usize;
            center_y = frac(height, tile_height) / 2.0;
        } else {
            center_y = frac(height, tile_height * required_rows) / 2.0;
        }

        center_x = frac(screen_width, tile_width) / 2.0;
    } else {
        center_x = (screen_width - tile_width * icons as f64) / 2.0;
        center_y = (height - tile_height) / 2.0;
    }

    let root_x = screen_x + center_x;
    let root_y = center_y;
    let mut cells = Vec::new();
    let mut overflow = None;
    let mut col = 0i32;
    let mut row = 0i32;

    for window in sorted {
        let left = root_x + f64::from(col) * tile_width;
        let top = root_y + f64::from(row) * tile_height;
        let rect = Rect::new(left, top, left + tile_width, top + tile_height);

        if remaining > 0 && col == columns as i32 - 1 && row == rows as i32 - 1 {
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
        });

        if col < columns as i32 - 1 {
            col += 1;
        } else {
            row += 1;
            col = 0;
        }
    }

    (cells, overflow)
}

fn frac(value: f64, divisor: f64) -> f64 {
    if divisor <= 0.0 {
        0.0
    } else {
        value - (value / divisor).floor() * divisor
    }
}

fn aspect_width(monitor: &MonitorInfo, pager_height: f64) -> f64 {
    (f64::from(monitor.width()) * pager_height / f64::from(monitor.height().max(1))).max(1.0)
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
    fn size_keeps_the_monitor_aspect_ratio() {
        let layout = build(
            &snapshot(Vec::new(), 2),
            &[monitor()],
            &modes(ViewMode::MicroTaskbar, 2, 1),
            &settings(ViewMode::MicroTaskbar),
        );

        assert!((layout.height - 40.0).abs() < f64::EPSILON);
        assert!((layout.desktop_width - 1920.0 / 1080.0 * 40.0).abs() < 0.001);
        assert!((layout.width - layout.desktop_width * 2.0).abs() < 0.001);
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
            &modes(ViewMode::Preview, 1, 1),
            &settings(ViewMode::Preview),
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
        );

        let cell = &layout.desktops[0].screens[0].cells[0];
        assert!(cell.rect.left >= 0.0);
        assert!(cell.rect.right <= layout.desktop_width);
    }
}
