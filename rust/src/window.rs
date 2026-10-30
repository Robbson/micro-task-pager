//! Win32 main window, message loop, timers, and context menu.
//!
//! The window procedure only decodes messages and forwards them to [`App`].
//! Rendering, hit testing, and COM calls live in other modules. This keeps the
//! `extern "system"` boundary small and makes panics across FFI unlikely.

use std::error::Error;

use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, HBRUSH, InvalidateRect, PAINTSTRUCT, ScreenToClient,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CS_HREDRAW, CS_VREDRAW, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyMenu, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetMessageW,
    GetWindowLongPtrW, HWND_NOTOPMOST, HWND_TOPMOST, IDC_ARROW, KillTimer, LWA_ALPHA, LoadCursorW,
    MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, PostQuitMessage, RegisterClassW, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetForegroundWindow, SetLayeredWindowAttributes,
    SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, TPM_LEFTALIGN, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TPM_TOPALIGN, TrackPopupMenu, TranslateMessage, WM_CAPTURECHANGED, WM_CLOSE,
    WM_COMMAND, WM_DESTROY, WM_DISPLAYCHANGE, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONUP, WM_TIMER,
    WNDCLASSW, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};
use windows::core::{Error as CoreError, PCWSTR, w};

const MK_LBUTTON: usize = 0x0001;
const MK_SHIFT: usize = 0x0004;

use crate::app::{
    App, CMD_ABOUT, CMD_EXIT, CMD_POSITION_DEFAULT, CMD_POSITION_RESTORE, CMD_POSITION_SAVE,
    CMD_TOGGLE_VIEW, TIMER_PRIMARY, TIMER_SECONDARY,
};

const WINDOW_CLASS: PCWSTR = w!("MicroTaskPagerWindow");
const WINDOW_TITLE: PCWSTR = w!("Micro Task Pager");

/// Creates the pager window and runs the message loop.
pub fn run(app: App) -> Result<(), Box<dyn Error>> {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(PCWSTR::null())?.0);
        let cursor = LoadCursorW(None, IDC_ARROW)?;
        let window_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: cursor,
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            lpszClassName: WINDOW_CLASS,
            ..Default::default()
        };

        if RegisterClassW(&window_class) == 0 {
            return Err(CoreError::from_thread().into());
        }

        let (x, y) = app.initial_position();
        let width = app.layout.width.max(1);
        let height = app.layout.height.max(1);
        let app_ptr = Box::into_raw(Box::new(app));

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_APPWINDOW,
            WINDOW_CLASS,
            WINDOW_TITLE,
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance),
            None,
        )?;

        let app = &mut *app_ptr;
        app.set_hwnd(hwnd);
        apply_opacity(hwnd, app.settings.background_opacity);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, app_ptr as isize);

        SetTimer(
            Some(hwnd),
            TIMER_PRIMARY,
            app.settings.primary_update_delay.max(16),
            None,
        );
        SetTimer(
            Some(hwnd),
            TIMER_SECONDARY,
            app.settings.secondary_update_delay.max(16),
            None,
        );

        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = InvalidateRect(Some(hwnd), None, false);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        let _ = KillTimer(Some(hwnd), TIMER_PRIMARY);
        let _ = KillTimer(Some(hwnd), TIMER_SECONDARY);
        let mut app = Box::from_raw(app_ptr);
        app.cancel_drag();
        drop(app);
    }

    Ok(())
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let app = unsafe { app_mut(hwnd) };
    let shift = (wparam.0 & MK_SHIFT) != 0;

    match message {
        WM_PAINT => {
            if let Some(app) = app {
                app.on_paint(hwnd);
            } else {
                unsafe {
                    let mut paint = PAINTSTRUCT::default();
                    let _ = BeginPaint(hwnd, &mut paint);
                    let _ = EndPaint(hwnd, &paint);
                }
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_TIMER => {
            if let Some(app) = app {
                app.on_timer(wparam.0);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            if let Some(app) = app {
                let (x, y) = client_point(lparam);
                app.on_mouse_down(x, y, false, shift);
            }
            unsafe {
                SetCapture(hwnd);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if let Some(app) = app {
                let (x, y) = client_point(lparam);
                app.on_mouse_up(x, y);
            }
            unsafe {
                let _ = ReleaseCapture();
            }
            LRESULT(0)
        }
        WM_MBUTTONDOWN => {
            if let Some(app) = app {
                let (x, y) = client_point(lparam);
                app.on_mouse_down(x, y, true, false);
            }
            unsafe {
                SetCapture(hwnd);
            }
            LRESULT(0)
        }
        WM_MBUTTONUP => {
            if let Some(app) = app {
                let (x, y) = client_point(lparam);
                app.on_mouse_up(x, y);
            }
            unsafe {
                let _ = ReleaseCapture();
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if let Some(app) = app {
                let (x, y) = client_point(lparam);
                let left_down = (wparam.0 & MK_LBUTTON) != 0;
                app.on_mouse_move(x, y, left_down);
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            if let Some(app) = app {
                let mut point = screen_point(lparam);
                unsafe {
                    let _ = ScreenToClient(hwnd, &mut point);
                }
                app.on_wheel(point.x, point.y, shift);
            }
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            if let Some(app) = app {
                show_context_menu(hwnd, app);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            if let Some(app) = app {
                app.on_command((wparam.0 & 0xFFFF) as u32);
            }
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            if let Some(app) = app {
                app.cancel_drag();
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            if let Some(app) = app {
                app.refresh(true);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

unsafe fn app_mut(hwnd: HWND) -> Option<&'static mut App> {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
    if pointer.is_null() {
        None
    } else {
        // SAFETY: the pointer is installed in `run` and cleared only when the
        // window is destroyed; all messages are handled on the owning thread.
        Some(unsafe { &mut *pointer })
    }
}

fn show_context_menu(hwnd: HWND, app: &mut App) {
    unsafe {
        let mut screen = POINT::default();
        let _ = GetCursorPos(&mut screen);
        let mut client = screen;
        let _ = ScreenToClient(hwnd, &mut client);
        app.set_menu_point(client.x, client.y);

        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let Ok(position) = CreatePopupMenu() else {
            let _ = DestroyMenu(menu);
            return;
        };

        let _ = AppendMenuW(
            menu,
            MF_STRING,
            CMD_TOGGLE_VIEW as usize,
            w!("Toggle View Mode"),
        );
        let _ = AppendMenuW(
            position,
            MF_STRING,
            CMD_POSITION_RESTORE as usize,
            w!("Restore"),
        );
        let _ = AppendMenuW(position, MF_STRING, CMD_POSITION_SAVE as usize, w!("Save"));
        let _ = AppendMenuW(
            position,
            MF_STRING,
            CMD_POSITION_DEFAULT as usize,
            w!("Default"),
        );
        // Populate the submenu before attaching it to the main menu. An empty
        // submenu attached first is not guaranteed to become visible later.
        let _ = AppendMenuW(menu, MF_POPUP, position.0 as usize, w!("Position"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, CMD_ABOUT as usize, w!("About"));
        let _ = AppendMenuW(menu, MF_STRING, CMD_EXIT as usize, w!("Exit"));

        app.set_menu_open(true);
        // A topmost pager would otherwise cover its own popup menu. Drop
        // topmost for the menu, and suppress the timer-driven reassertion while
        // `TrackPopupMenu` runs its modal loop.
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
        let command = TrackPopupMenu(
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
        app.set_menu_open(false);

        let command = command.0 as u32;
        if command != 0 {
            app.on_command(command);
        }
    }
}

fn apply_opacity(hwnd: HWND, opacity: f64) {
    let opacity = opacity.clamp(0.25, 1.0);
    let alpha = (opacity * 255.0).round() as u8;
    unsafe {
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
    }
}

fn client_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

fn screen_point(lparam: LPARAM) -> POINT {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    POINT { x, y }
}
