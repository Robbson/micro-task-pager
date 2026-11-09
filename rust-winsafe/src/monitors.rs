//! Monitor enumeration for the pager layout.
//!
//! The pager draws one renderer per physical monitor inside each desktop
//! column. `Screen.AllScreens` does this in the C# application; here the same
//! data comes from `EnumDisplayMonitors`. Monitors are sorted left-to-right
//! and then top-to-bottom, matching the C# `OrderBy(Bounds.Left).ThenBy(Bounds.Top)`.
//!
//! The process is system DPI aware, so these rectangles are physical pixels and
//! do not change when the window moves between monitors.

use std::mem::size_of;

use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, MONITORINFOF_PRIMARY, SM_CXSCREEN, SM_CYSCREEN,
};
use windows::core::BOOL;

/// One physical monitor in virtual-screen coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MonitorInfo {
    /// Full monitor rectangle.
    pub rect: RECT,
    /// Work area (excludes the taskbar).
    pub work_rect: RECT,
    /// Whether this is the primary monitor.
    pub primary: bool,
}

impl MonitorInfo {
    /// Monitor width in physical pixels.
    pub fn width(&self) -> i32 {
        self.rect.right - self.rect.left
    }

    /// Monitor height in physical pixels.
    pub fn height(&self) -> i32 {
        self.rect.bottom - self.rect.top
    }

    /// Whether a point is inside the monitor rectangle.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.rect.left && x < self.rect.right && y >= self.rect.top && y < self.rect.bottom
    }
}

/// Enumerates monitors, sorted left-to-right and then top-to-bottom.
pub fn enumerate() -> Vec<MonitorInfo> {
    let mut monitors = Vec::new();

    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_proc),
            LPARAM(&mut monitors as *mut Vec<MonitorInfo> as isize),
        );
    }

    if monitors.is_empty() {
        // EnumDisplayMonitors should not fail on a normal desktop, but a
        // single-screen fallback keeps the pager usable in odd sessions.
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        let rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        monitors.push(MonitorInfo {
            rect,
            work_rect: rect,
            primary: true,
        });
    }

    monitors.sort_by_key(|monitor| (monitor.rect.left, monitor.rect.top));
    monitors
}

/// Returns the primary monitor, or the first monitor if none is flagged.
pub fn primary(monitors: &[MonitorInfo]) -> Option<&MonitorInfo> {
    monitors
        .iter()
        .find(|monitor| monitor.primary)
        .or_else(|| monitors.first())
}

unsafe extern "system" fn monitor_proc(
    monitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    // SAFETY: `enumerate` passes a pointer to a live Vec; the callback runs
    // synchronously and does not retain the reference.
    let monitors = unsafe { &mut *(data.0 as *mut Vec<MonitorInfo>) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        monitors.push(MonitorInfo {
            rect: info.rcMonitor,
            work_rect: info.rcWork,
            primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
        });
    }

    BOOL(1)
}
