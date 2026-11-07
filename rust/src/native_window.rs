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

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_SHIFT};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallNextHookEx, CreatePopupMenu, DestroyMenu, EnumChildWindows, EnumWindows,
    GA_ROOT, GWL_EXSTYLE, GWL_STYLE, GetAncestor, GetClassNameW, GetClientRect, GetCursorPos,
    GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, HC_ACTION, HHOOK, HWND_NOTOPMOST, HWND_TOPMOST, IDC_ARROW,
    IDC_SIZEALL, IsWindowVisible, LWA_ALPHA, LoadCursorW, MENUINFO, MF_GRAYED, MF_POPUP,
    MF_SEPARATOR, MF_STRING, MIM_APPLYTOSUBMENUS, MIM_STYLE, MNS_NOCHECK, MSLLHOOKSTRUCT,
    SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SetCursor, SetForegroundWindow, SetLayeredWindowAttributes, SetMenuInfo, SetWindowLongPtrW,
    SetWindowPos, SetWindowsHookExW, ShowWindowAsync, TPM_LEFTALIGN, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TPM_TOPALIGN, TrackPopupMenu, UnhookWindowsHookEx, WH_MOUSE_LL, WM_MOUSEWHEEL,
    WM_SETCURSOR, WS_BORDER, WS_CAPTION, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME, WindowFromPoint,
};
use windows::core::{BOOL, PCWSTR, w};

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

/// Reasserts topmost for an arbitrary window owned by this process.
///
/// Used for the secondary About and Settings windows; they must sit above the
/// topmost pager while they are open.
pub fn reassert_topmost(hwnd: HWND) -> Result<(), windows::core::Error> {
    // SAFETY: the handle was returned by `find_pager_window`, so it belongs to
    // this process; `SetWindowPos` only changes z-order on this UI thread.
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    }
}

/// Finds WinUI's visible content window inside the pager.
///
/// WinUI 3 hosts the XAML island in a `DesktopChildSiteBridge` child window.
/// Mouse wheel messages are delivered to that child, not to the top-level HWND,
/// so the wheel/cursor subclass must be installed there.
pub fn find_content_window(pager: HWND) -> Option<HWND> {
    let mut found = HWND::default();

    // SAFETY: `EnumChildWindows` runs the callback synchronously and only
    // receives a pointer to `found`, which lives for the whole call.
    let _ = unsafe {
        EnumChildWindows(
            Some(pager),
            Some(find_content_proc),
            LPARAM(&mut found as *mut HWND as isize),
        )
    };

    (!found.0.is_null()).then_some(found)
}

unsafe extern "system" fn find_content_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `find_content_window` passes a live HWND slot.
    let found = unsafe { &mut *(lparam.0 as *mut HWND) };

    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return BOOL(1);
    }

    let mut buffer = [0u16; 128];
    // SAFETY: `buffer` is a valid mutable UTF-16 buffer.
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    let name = String::from_utf16_lossy(&buffer[..length.max(0) as usize]);
    if name.eq_ignore_ascii_case("Microsoft.UI.Content.DesktopChildSiteBridge") {
        *found = hwnd;
        return BOOL(0);
    }

    BOOL(1)
}

struct FindWindow<'a> {
    process_id: u32,
    title: &'a str,
    found: Option<HWND>,
}

thread_local! {
    // Reactor 0.100 has no wheel event. The low-level mouse hook and the
    // subclass both run on the UI thread, so the non-Send callback can hold a
    // `LocalSender`.
    static WHEEL_HANDLER: RefCell<Option<WheelHandler>> = RefCell::new(None);
    static PAGER_HWND: Cell<usize> = const { Cell::new(0) };
    // `WM_SETCURSOR` is handled in the content-window subclass so WinUI cannot
    // reset the move cursor while a drag is active.
    static DRAG_CURSOR: Cell<bool> = const { Cell::new(false) };
}

type WheelHandler = Box<dyn Fn(f64, f64, bool) -> bool>;

/// A wheel/cursor input hook for the pager.
///
/// WinUI's child site window does not reliably receive `WM_MOUSEWHEEL`, and the
/// top-level HWND does not see wheel messages aimed at the XAML child. A
/// low-level mouse hook is the reliable shim until reactor exposes wheel
/// events. It only consumes wheel input when Shift is down and the cursor is
/// over the pager. The subclass on the XAML child handles `WM_SETCURSOR`.
pub struct WheelHook {
    pager: HWND,
    subclass_hwnd: HWND,
    id: usize,
    pager_id: usize,
    hook: HHOOK,
}

impl WheelHook {
    /// Installs the wheel and cursor hooks. The handler receives client DIPs
    /// and Shift, and returns `true` when it handled the wheel message.
    pub fn install(
        pager: HWND,
        handler: impl Fn(f64, f64, bool) -> bool + 'static,
    ) -> Result<Self, windows::core::Error> {
        WHEEL_HANDLER.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(handler));
        });
        PAGER_HWND.with(|slot| slot.set(pager.0 as usize));

        let subclass_hwnd = find_content_window(pager).unwrap_or(pager);
        // SAFETY: the handle belongs to this process; the subclass id is local
        // to this module and the callback has the required ABI.
        let installed =
            unsafe { SetWindowSubclass(subclass_hwnd, Some(cursor_subclass_proc), 1, 0) };
        if !installed.as_bool() {
            clear_input_thread_state();
            return Err(windows::core::Error::from_thread());
        }

        // The content child and the top-level window can both receive
        // `WM_SETCURSOR` depending on the WinUI build. Install on both.
        let top_installed = unsafe { SetWindowSubclass(pager, Some(cursor_subclass_proc), 2, 0) };
        if !top_installed.as_bool() {
            unsafe {
                let _ = RemoveWindowSubclass(subclass_hwnd, Some(cursor_subclass_proc), 1);
            }
            clear_input_thread_state();
            return Err(windows::core::Error::from_thread());
        }

        // SAFETY: WH_MOUSE_LL runs the callback on the installing UI thread,
        // which has a message loop. The hook is removed in `Drop`.
        let hook =
            match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_wheel_hook_proc), None, 0) } {
                Ok(hook) => hook,
                Err(error) => {
                    unsafe {
                        let _ = RemoveWindowSubclass(subclass_hwnd, Some(cursor_subclass_proc), 1);
                        let _ = RemoveWindowSubclass(pager, Some(cursor_subclass_proc), 2);
                    }
                    clear_input_thread_state();
                    return Err(error);
                }
            };

        Ok(Self {
            pager,
            subclass_hwnd,
            id: 1,
            pager_id: 2,
            hook,
        })
    }
}

impl Drop for WheelHook {
    fn drop(&mut self) {
        set_drag_cursor(false);
        clear_input_thread_state();
        // SAFETY: unhooking and removing the same subclass are safe during
        // teardown even if the window is already gone.
        unsafe {
            let _ = UnhookWindowsHookEx(self.hook);
            let _ = RemoveWindowSubclass(self.subclass_hwnd, Some(cursor_subclass_proc), self.id);
            let _ = RemoveWindowSubclass(self.pager, Some(cursor_subclass_proc), self.pager_id);
        }
    }
}

fn clear_input_thread_state() {
    WHEEL_HANDLER.with(|slot| {
        *slot.borrow_mut() = None;
    });
    PAGER_HWND.with(|slot| slot.set(0));
}

unsafe extern "system" fn mouse_wheel_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32 {
        let message = wparam.0 as u32;
        let pager_value = PAGER_HWND.with(Cell::get);

        if message == WM_MOUSEWHEEL && pager_value != 0 {
            // SAFETY: for HC_ACTION, `lparam` points to MSLLHOOKSTRUCT.
            let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
            let pager = HWND(pager_value as *mut std::ffi::c_void);
            // SAFETY: WindowFromPoint and GetAncestor only read window state.
            let root = unsafe { GetAncestor(WindowFromPoint(info.pt), GA_ROOT) };
            let shift = unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) } < 0;

            if root == pager && shift {
                let mut point = info.pt;
                // SAFETY: the pager HWND is live while the hook is installed.
                if unsafe { ScreenToClient(pager, &mut point) }.as_bool() {
                    let dpi = unsafe { GetDpiForWindow(pager) }.max(96);
                    let scale = 96.0 / f64::from(dpi);
                    let handled = WHEEL_HANDLER.with(|slot| {
                        slot.borrow().as_ref().is_some_and(|handler| {
                            handler(f64::from(point.x) * scale, f64::from(point.y) * scale, true)
                        })
                    });
                    if handled {
                        return LRESULT(1);
                    }
                }
            }
        }
    }

    // SAFETY: unhandled input continues through the hook chain.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn cursor_subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if message == WM_SETCURSOR && DRAG_CURSOR.with(Cell::get) {
        // SAFETY: loading and setting a system cursor only changes the current
        // thread's cursor and owns no resources.
        unsafe {
            if let Ok(cursor) = LoadCursorW(None, IDC_SIZEALL) {
                SetCursor(Some(cursor));
                return LRESULT(1);
            }
        }
    }

    // SAFETY: unhandled messages continue through the original window
    // procedure chain for this window.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Current cursor position in physical screen pixels.
pub fn cursor_position() -> Option<(i32, i32)> {
    let mut point = POINT::default();
    // SAFETY: `GetCursorPos` only reads the global cursor position.
    unsafe {
        GetCursorPos(&mut point).ok()?;
    }
    Some((point.x, point.y))
}

/// Whether Shift is currently held.
pub fn shift_pressed() -> bool {
    // SAFETY: `GetAsyncKeyState` reads the global key state and does not need
    // keyboard focus, which matters while the pager is hovered but inactive.
    unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) < 0 }
}

/// Switches between the normal arrow and the move cursor.
pub fn set_drag_cursor(move_cursor: bool) {
    DRAG_CURSOR.with(|value| value.set(move_cursor));
    // SAFETY: loading and setting a system cursor only changes the current
    // thread's cursor and owns no resources. WinUI may reset the cursor on its
    // next input pass; the content-window subclass also answers `WM_SETCURSOR`
    // while a drag is active. Cursor feedback is a follow-up item.
    unsafe {
        let name = if move_cursor { IDC_SIZEALL } else { IDC_ARROW };
        if let Ok(cursor) = LoadCursorW(None, name) {
            SetCursor(Some(cursor));
        }
    }
}

/// DPI of a window, or the system DPI if the window has no monitor yet.
pub fn window_dpi(hwnd: HWND) -> u32 {
    // SAFETY: `GetDpiForWindow` only reads window DPI.
    unsafe { GetDpiForWindow(hwnd) }.max(96)
}

/// DPI of the primary display before the pager HWND exists.
pub fn system_dpi() -> u32 {
    // SAFETY: `GetDpiForSystem` only reads the system DPI.
    unsafe { GetDpiForSystem() }.max(96)
}

/// Foreground window at pager startup.
pub fn foreground_window() -> Option<HWND> {
    // SAFETY: `GetForegroundWindow` only reads the current foreground handle.
    let hwnd = unsafe { GetForegroundWindow() };
    (!hwnd.0.is_null()).then_some(hwnd)
}

/// Focuses a window without any extra WinUI wrapping.
pub fn activate_window(hwnd: HWND) -> bool {
    // SAFETY: the handle belongs to another application; `SetForegroundWindow`
    // only requests activation and does not transfer ownership.
    unsafe { SetForegroundWindow(hwnd).as_bool() }
}

/// Menu commands returned by the native context menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextCommand {
    ToggleViewMode,
    RestorePosition,
    SavePosition,
    DefaultPosition,
    Settings,
    About,
    Exit,
}

const MENU_TOGGLE: u32 = 1;
const MENU_RESTORE: u32 = 2;
const MENU_SAVE: u32 = 3;
const MENU_DEFAULT: u32 = 4;
const MENU_SETTINGS: u32 = 5;
const MENU_ABOUT: u32 = 6;
const MENU_EXIT: u32 = 7;

/// Shows the native context menu at the cursor and returns the chosen command.
///
/// Reactor 0.100 has no right-click context menu; this is a deliberate,
/// documented Win32 shim until reactor exposes transient menus. Topmost is
/// dropped for the modal menu and restored before returning, because a topmost
/// pager would otherwise cover its own popup.
///
/// `dialog_open` grays out Settings and About while a secondary window is open.
pub fn show_context_menu(hwnd: HWND, dialog_open: bool) -> Option<ContextCommand> {
    let mut screen = POINT::default();
    // SAFETY: reads the global cursor position; no window state is changed.
    unsafe {
        GetCursorPos(&mut screen).ok()?;
    }

    // SAFETY: menu handles are created and destroyed symmetrically below; the
    // handle is only used on this thread.
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let position = match CreatePopupMenu() {
            Ok(position) => position,
            Err(_) => {
                let _ = DestroyMenu(menu);
                return None;
            }
        };

        // Native menus have no per-item icon column without owner-drawing the
        // whole menu, so keep the C# glyphs as leading menu labels. Removing the
        // check column keeps the compact width.
        let menu_info = MENUINFO {
            cbSize: std::mem::size_of::<MENUINFO>() as u32,
            fMask: MIM_STYLE | MIM_APPLYTOSUBMENUS,
            dwStyle: MNS_NOCHECK,
            ..Default::default()
        };
        let _ = SetMenuInfo(menu, &menu_info);
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_TOGGLE as usize,
            w!("↔  Toggle View Mode"),
        );
        let _ = AppendMenuW(position, MF_STRING, MENU_RESTORE as usize, w!("Restore"));
        let _ = AppendMenuW(position, MF_STRING, MENU_SAVE as usize, w!("Save"));
        let _ = AppendMenuW(position, MF_STRING, MENU_DEFAULT as usize, w!("Default"));
        // Populate the submenu before attaching it; an empty submenu attached
        // first is not guaranteed to render later.
        let _ = AppendMenuW(menu, MF_POPUP, position.0 as usize, w!("⌖  Position"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let dialog_flags = if dialog_open {
            MF_STRING | MF_GRAYED
        } else {
            MF_STRING
        };
        let _ = AppendMenuW(
            menu,
            dialog_flags,
            MENU_SETTINGS as usize,
            w!("⚙  Settings"),
        );
        let _ = AppendMenuW(menu, dialog_flags, MENU_ABOUT as usize, w!("ℹ  About"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, MENU_EXIT as usize, w!("✕  Exit"));

        let _ = SetWindowPos(
            hwnd,
            Some(HWND_NOTOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = SetForegroundWindow(hwnd);
        let selection = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON,
            screen.x,
            screen.y,
            None,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );

        match selection.0 as u32 {
            MENU_TOGGLE => Some(ContextCommand::ToggleViewMode),
            MENU_RESTORE => Some(ContextCommand::RestorePosition),
            MENU_SAVE => Some(ContextCommand::SavePosition),
            MENU_DEFAULT => Some(ContextCommand::DefaultPosition),
            MENU_SETTINGS => Some(ContextCommand::Settings),
            MENU_ABOUT => Some(ContextCommand::About),
            MENU_EXIT => Some(ContextCommand::Exit),
            _ => None,
        }
    }
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

    /// Current window origin in physical screen pixels.
    pub fn origin(&self) -> Option<(i32, i32)> {
        self.rect().ok().map(|rect| (rect.left, rect.top))
    }

    /// Full window rectangle in physical screen pixels.
    pub fn window_rect(&self) -> Option<RECT> {
        self.rect().ok()
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
