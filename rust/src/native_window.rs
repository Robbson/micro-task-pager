//! Small Win32 shim for the reactor pager window.
//!
//! `windows-reactor` 0.100 does not expose its native window handle, and WinUI
//! does not let a component choose borderless, tool-window, layered, or topmost
//! styles. This module owns exactly that gap:
//!
//! - find the pager's top-level HWND by process ID and title,
//! - remove the caption and resize frame, and set the taskbar style,
//! - apply `WS_EX_LAYERED` alpha for the configured opacity,
//! - place the window at the saved or default position,
//! - reassert topmost and recover when another window covers the pager.
//!
//! It is deliberately not a UI abstraction. Anything that starts looking like
//! one belongs upstream in `windows-reactor` instead.

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, RECT};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetWindowLongPtrW, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, HWND_TOPMOST, IsWindowVisible,
    LWA_ALPHA, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindowAsync,
    WS_BORDER, WS_CAPTION, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME, WindowFromPoint,
};
use windows::core::BOOL;

use crate::monitors;

/// Finds the visible top-level window owned by this process with `title`.
///
/// Reactor creates its HWND after the component's first publication, so the
/// pager looks for it on the first tick rather than in `Component::create`.
pub fn find_pager_window(title: &str) -> Option<HWND> {
    let mut search = FindWindow {
        process_id: std::process::id(),
        title,
        found: None,
    };

    // SAFETY: `EnumWindows` runs the callback synchronously on this thread and
    // only receives a pointer to `search`, which lives for the whole call.
    let _ = unsafe {
        EnumWindows(
            Some(find_window_proc),
            LPARAM(&mut search as *mut FindWindow<'_> as isize),
        )
    };

    search.found
}

struct FindWindow<'a> {
    process_id: u32,
    title: &'a str,
    found: Option<HWND>,
}

unsafe extern "system" fn find_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `find_pager_window` passes a live `FindWindow` and the callback
    // never keeps the reference after returning.
    let search = unsafe { &mut *(lparam.0 as *mut FindWindow<'_>) };

    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return BOOL(1);
    }

    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id as *mut u32));
    }
    if process_id != search.process_id {
        return BOOL(1);
    }

    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return BOOL(1);
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    let title = String::from_utf16_lossy(&buffer[..copied.max(0) as usize]);
    if title == search.title {
        search.found = Some(hwnd);
        return BOOL(0);
    }

    BOOL(1)
}

/// Owns the pager HWND and applies the window properties reactor does not expose.
pub struct PagerWindow {
    hwnd: HWND,
    show_in_taskbar: bool,
    opacity: f64,
}

impl PagerWindow {
    /// Applies the initial pager styles and opacity to a reactor window.
    pub fn attach(
        hwnd: HWND,
        show_in_taskbar: bool,
        opacity: f64,
    ) -> Result<Self, windows::core::Error> {
        let mut window = Self {
            hwnd,
            show_in_taskbar,
            opacity: clamp_opacity(opacity),
        };
        window.apply_styles()?;
        window.apply_opacity();
        Ok(window)
    }

    /// Native window handle used for pinning and hit testing.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Reapplies styles if WinUI or the shell changed them.
    ///
    /// WinUI may reset window styles after a style change, so this is called on
    /// the secondary tick instead of once at startup.
    pub fn ensure_styles(&mut self) -> Result<(), windows::core::Error> {
        if self.apply_styles()? {
            self.apply_opacity();
        }
        Ok(())
    }

    /// Changes the taskbar behavior on the next `ensure_styles` call.
    pub fn set_show_in_taskbar(&mut self, show_in_taskbar: bool) {
        self.show_in_taskbar = show_in_taskbar;
    }

    /// Applies the layered-window alpha. The react view does not pre-multiply
    /// colors, so icons and text keep their own alpha.
    pub fn set_opacity(&mut self, opacity: f64) {
        self.opacity = clamp_opacity(opacity);
        self.apply_opacity();
    }

    /// Resizes the client area to the requested size in DIPs.
    ///
    /// Reactor sizes the window before this shim removes the WinUI frame, so
    /// its `ResizeClient` call includes the old non-client area. Reapplying the
    /// target size after the style change keeps the pager exactly as tall as
    /// the configured pager height.
    pub fn ensure_size(
        &self,
        width_dips: f64,
        height_dips: f64,
    ) -> Result<(), windows::core::Error> {
        // SAFETY: the handle belongs to this process and `GetDpiForWindow` only
        // reads the DPI of the window's current monitor.
        let dpi = unsafe { GetDpiForWindow(self.hwnd) }.max(96);
        let scale = f64::from(dpi) / 96.0;
        let width = (width_dips * scale).round().max(1.0) as i32;
        let height = (height_dips * scale).round().max(1.0) as i32;

        let mut client = RECT::default();
        // SAFETY: `GetClientRect` only reads the client rectangle.
        unsafe {
            GetClientRect(self.hwnd, &mut client)?;
        }
        if client.right == width && client.bottom == height {
            return Ok(());
        }

        // SAFETY: resizing does not change ownership or z-order, and the size
        // is derived from validated positive DIP values.
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
    }

    /// Moves to the saved position, or bottom-center of the primary work area.
    pub fn place(&self, saved: Option<(i32, i32)>) -> Result<(), windows::core::Error> {
        let (x, y) = saved.unwrap_or_else(|| self.default_position());
        self.move_to(x, y)
    }

    /// Moves the window without changing its size or z-order.
    pub fn move_to(&self, x: i32, y: i32) -> Result<(), windows::core::Error> {
        // SAFETY: the handle came from `find_pager_window` and belongs to this
        // process; `SetWindowPos` only changes window state on this thread.
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
    }

    /// Reasserts topmost without moving, resizing, or activating the pager.
    pub fn reassert_topmost(&self) -> Result<(), windows::core::Error> {
        // SAFETY: same invariant as `move_to`; the pager window is owned by
        // this process and only this UI thread changes its window state.
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        }
    }

    /// Whether another window is at the center point of the pager.
    pub fn is_covered(&self) -> bool {
        let Ok(rect) = self.rect() else {
            return false;
        };

        let point = POINT {
            x: (rect.left + rect.right) / 2,
            y: (rect.top + rect.bottom) / 2,
        };

        // SAFETY: `WindowFromPoint` is a read-only query and accepts screen
        // coordinates from any thread.
        let window = unsafe { WindowFromPoint(point) };
        !window.0.is_null() && window != self.hwnd
    }

    /// Re-shows the pager without activation and reasserts topmost.
    pub fn restore_covered(&self) {
        // SAFETY: the handle belongs to this process and this thread.
        unsafe {
            let _ = ShowWindowAsync(self.hwnd, SW_SHOWNOACTIVATE);
        }
        let _ = self.reassert_topmost();
    }

    fn apply_styles(&mut self) -> Result<bool, windows::core::Error> {
        // SAFETY: `GetWindowLongPtrW` only reads window state for a handle that
        // belongs to this process.
        let style = unsafe { GetWindowLongPtrW(self.hwnd, GWL_STYLE) } as u32;
        let ex_style = unsafe { GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) } as u32;

        let desired_style = (style
            & !(WS_CAPTION.0
                | WS_THICKFRAME.0
                | WS_SYSMENU.0
                | WS_MINIMIZEBOX.0
                | WS_MAXIMIZEBOX.0
                | WS_BORDER.0))
            | WS_POPUP.0;

        let taskbar_style = if self.show_in_taskbar {
            (ex_style | WS_EX_APPWINDOW.0) & !WS_EX_TOOLWINDOW.0
        } else {
            (ex_style | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0
        };
        let desired_ex_style = taskbar_style | WS_EX_LAYERED.0;

        let mut changed = false;
        if style != desired_style {
            // SAFETY: as above; the new style keeps the window alive and only
            // changes its frame behavior.
            unsafe {
                SetWindowLongPtrW(self.hwnd, GWL_STYLE, desired_style as isize);
            }
            changed = true;
        }
        if ex_style != desired_ex_style {
            // SAFETY: as above; `WS_EX_LAYERED` is applied together with the
            // alpha value in `apply_opacity`.
            unsafe {
                SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, desired_ex_style as isize);
            }
            changed = true;
        }

        if changed {
            // SAFETY: `SWP_FRAMECHANGED` tells the shell to recalculate the
            // non-client area after the style change. The position and size are
            // preserved.
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }

        Ok(changed)
    }

    fn apply_opacity(&mut self) {
        let alpha = (self.opacity * 255.0).round() as u8;
        // SAFETY: `WS_EX_LAYERED` is set by `apply_styles`, and the handle
        // belongs to this process. `LWA_ALPHA` changes only window opacity.
        let _ = unsafe { SetLayeredWindowAttributes(self.hwnd, COLORREF(0), alpha, LWA_ALPHA) };
    }

    fn rect(&self) -> Result<RECT, windows::core::Error> {
        let mut rect = RECT::default();
        // SAFETY: `GetWindowRect` only reads the window rectangle.
        unsafe {
            GetWindowRect(self.hwnd, &mut rect)?;
        }
        Ok(rect)
    }

    fn default_position(&self) -> (i32, i32) {
        let Ok(rect) = self.rect() else {
            return (0, 0);
        };
        let monitors = monitors::enumerate();
        let Some(primary) = monitors::primary(&monitors) else {
            return (0, 0);
        };

        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        let x =
            primary.work_rect.left + (primary.work_rect.right - primary.work_rect.left - width) / 2;
        let y = primary.work_rect.bottom - height;
        (x, y)
    }
}

fn clamp_opacity(opacity: f64) -> f64 {
    opacity.clamp(0.25, 1.0)
}
