//! winsafe main window host.
//!
//! Replaces the hand-written `WNDCLASSW`/`CreateWindowExW`/`WndProc`/message
//! loop from the original Win32 implementation with a winsafe
//! [`gui::WindowMain`]. Rendering, hit testing, and COM calls still live in
//! [`App`]; the winsafe events only decode input and forward it.
//!
//! winsafe stores its own pointer in `GWLP_USERDATA`, so the `App` pointer is
//! kept in a thread-local slot instead of the window user data.

use std::cell::Cell;
use std::error::Error;

use winsafe::{self as w, co, gui, prelude::*};

use windows::Win32::Foundation::{COLORREF, HWND as WinHWND, POINT as WinPoint};
use windows::Win32::Graphics::Gdi::{InvalidateRect, ScreenToClient};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, SetForegroundWindow,
    SetLayeredWindowAttributes, SetMenuInfo, SetTimer, SetWindowPos, TrackPopupMenu,
    HWND_NOTOPMOST, HWND_TOPMOST, LWA_ALPHA, MENUINFO, MF_POPUP, MF_SEPARATOR, MF_STRING,
    MIM_APPLYTOSUBMENUS, MIM_STYLE, MNS_NOCHECK, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TPM_TOPALIGN,
};

const MK_LBUTTON: u16 = 0x0001;
const MK_SHIFT: u16 = 0x0004;

use crate::app::{
    App, CMD_ABOUT, CMD_EXIT, CMD_POSITION_DEFAULT, CMD_POSITION_RESTORE, CMD_POSITION_SAVE,
    CMD_TOGGLE_VIEW, TIMER_PRIMARY, TIMER_RESTORE, TIMER_SECONDARY,
};

thread_local! {
    /// Raw `App` pointer for the current UI thread.
    ///
    /// winsafe owns `GWLP_USERDATA`, so the original implementation's slot is
    /// not available. The pointer is installed in `wm_create` and cleared after
    /// `run_main` returns; all access is on the owning thread.
    static APP: Cell<*mut App> = const { Cell::new(std::ptr::null_mut()) };
}

/// Creates the pager window and runs the winsafe message loop.
pub fn run(app: App) -> Result<(), Box<dyn Error>> {
    let (x, y) = app.initial_position();
    let width = app.layout.width.max(1);
    let height = app.layout.height.max(1);
    let primary_delay = app.settings.primary_update_delay.max(16);
    let secondary_delay = app.settings.secondary_update_delay.max(16);

    let mut ex_style = co::WS_EX::TOPMOST | co::WS_EX::LAYERED;
    if app.settings.show_app_in_taskbar {
        // Pinning to all desktops only works for applications that appear in
        // the taskbar.
        ex_style |= co::WS_EX::APPWINDOW;
    }

    let wnd = gui::WindowMain::new(gui::WindowMainOpts {
        title: "Micro Task Pager",
        size: (width, height),
        style: co::WS::POPUP | co::WS::VISIBLE,
        ex_style,
        class_style: co::CS::HREDRAW | co::CS::VREDRAW,
        class_bg_brush: gui::Brush::None,
        process_dlg_msgs: false,
        ..Default::default()
    });

    let app_ptr = Box::into_raw(Box::new(app));
    install_events(&wnd, app_ptr, x, y, primary_delay, secondary_delay);

    let result = wnd.run_main(Some(co::SW::SHOWNOACTIVATE));

    APP.with(|slot| slot.set(std::ptr::null_mut()));
    // SAFETY: `wm_create` installed this pointer and the message loop has
    // ended, so no event can observe it any more.
    let mut app = unsafe { Box::from_raw(app_ptr) };
    app.cancel_drag();
    result.map_err(|error| -> Box<dyn Error> { error.to_string().into() })?;
    Ok(())
}

fn install_events(
    wnd: &gui::WindowMain,
    app_ptr: *mut App,
    x: i32,
    y: i32,
    primary_delay: u32,
    secondary_delay: u32,
) {
    let window = wnd.clone();
    wnd.on().wm_create(move |_| {
        let hwnd = win_hwnd(window.hwnd());
        APP.with(|slot| slot.set(app_ptr));
        // SAFETY: the pointer was created in `run` and lives until the loop
        // returns. This is the owning UI thread.
        let app = unsafe { &mut *app_ptr };
        app.set_hwnd(hwnd);

        // winsafe centers the window; restore the remembered pager position.
        // SAFETY: `hwnd` belongs to this window and the flags only move it.
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                None,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        apply_opacity(hwnd, app.settings.background_opacity);
        // SAFETY: timers are installed on the pager's own window on the UI
        // thread and are removed when the window is destroyed.
        unsafe {
            let _ = SetTimer(Some(hwnd), TIMER_PRIMARY, primary_delay, None);
            let _ = SetTimer(Some(hwnd), TIMER_SECONDARY, secondary_delay, None);
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
        app.start_tracking();
        app.try_pin();
        Ok(0)
    });

    let window = wnd.clone();
    wnd.on().wm_paint(move || {
        let hwnd = win_hwnd(window.hwnd());
        with_app(|app| app.on_paint(hwnd));
        Ok(())
    });

    wnd.on().wm_erase_bkgnd(|_| Ok(1));

    wnd.on().wm_timer(TIMER_PRIMARY, || {
        with_app(|app| app.on_timer(TIMER_PRIMARY));
        Ok(())
    });
    wnd.on().wm_timer(TIMER_SECONDARY, || {
        with_app(|app| app.on_timer(TIMER_SECONDARY));
        Ok(())
    });
    wnd.on().wm_timer(TIMER_RESTORE, || {
        with_app(|app| app.on_timer(TIMER_RESTORE));
        Ok(())
    });

    let window = wnd.clone();
    wnd.on().wm_l_button_down(move |p: w::msg::WmLButtonDown| {
        let shift = p.vkey_code.raw() & MK_SHIFT != 0;
        with_app(|app| app.on_mouse_down(p.coords.x, p.coords.y, false, shift));
        // SAFETY: captures the mouse on the pager's own window.
        unsafe {
            SetCapture(win_hwnd(window.hwnd()));
        }
        Ok(())
    });

    wnd.on().wm_l_button_up(|p: w::msg::WmLButtonUp| {
        with_app(|app| app.on_mouse_up(p.coords.x, p.coords.y));
        // SAFETY: releases the capture taken on button-down.
        unsafe {
            let _ = ReleaseCapture();
        }
        Ok(())
    });

    let window = wnd.clone();
    wnd.on().wm_m_button_down(move |p: w::msg::WmMButtonDown| {
        with_app(|app| app.on_mouse_down(p.coords.x, p.coords.y, true, false));
        // SAFETY: captures the mouse on the pager's own window.
        unsafe {
            SetCapture(win_hwnd(window.hwnd()));
        }
        Ok(())
    });

    wnd.on().wm_m_button_up(|p: w::msg::WmMButtonUp| {
        with_app(|app| app.on_mouse_up(p.coords.x, p.coords.y));
        // SAFETY: releases the capture taken on button-down.
        unsafe {
            let _ = ReleaseCapture();
        }
        Ok(())
    });

    wnd.on().wm_mouse_move(|p: w::msg::WmMouseMove| {
        let left_down = p.vkey_code.raw() & MK_LBUTTON != 0;
        with_app(|app| app.on_mouse_move(p.coords.x, p.coords.y, left_down));
        Ok(())
    });

    let window = wnd.clone();
    wnd.on().wm_mouse_wheel(move |p: w::msg::WmMouseWheel| {
        let hwnd = win_hwnd(window.hwnd());
        let mut point = WinPoint {
            x: p.coords.x,
            y: p.coords.y,
        };
        // SAFETY: converts the message's screen coordinates for this window.
        unsafe {
            let _ = ScreenToClient(hwnd, &mut point);
        }
        let shift = p.keys.raw() & MK_SHIFT != 0;
        with_app(|app| app.on_wheel(point.x, point.y, shift));
        Ok(())
    });

    let window = wnd.clone();
    wnd.on().wm_r_button_up(move |_| {
        let hwnd = win_hwnd(window.hwnd());
        with_app(|app| show_context_menu(hwnd, app, &window));
        Ok(())
    });

    wnd.on().wm_capture_changed(|_| {
        with_app(App::cancel_drag);
        Ok(())
    });

    wnd.on().wm_display_change(|_| {
        with_app(|app| app.refresh(true));
        Ok(())
    });
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|slot| {
        let pointer = slot.get();
        if pointer.is_null() {
            None
        } else {
            // SAFETY: the pointer is installed on this thread in `wm_create`
            // and cleared only after the message loop ends.
            Some(f(unsafe { &mut *pointer }))
        }
    })
}

fn win_hwnd(hwnd: &w::HWND) -> WinHWND {
    WinHWND(hwnd.ptr())
}

fn show_context_menu(hwnd: WinHWND, app: &mut App, parent: &gui::WindowMain) {
    unsafe {
        let mut screen = WinPoint::default();
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

        // Native menus have no per-item icon column without owner-drawing the
        // whole menu, so keep the C# glyphs as leading labels. Removing the
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
            CMD_TOGGLE_VIEW as usize,
            windows::core::w!("↔  Toggle View Mode"),
        );
        let _ = AppendMenuW(
            position,
            MF_STRING,
            CMD_POSITION_RESTORE as usize,
            windows::core::w!("Restore"),
        );
        let _ = AppendMenuW(
            position,
            MF_STRING,
            CMD_POSITION_SAVE as usize,
            windows::core::w!("Save"),
        );
        let _ = AppendMenuW(
            position,
            MF_STRING,
            CMD_POSITION_DEFAULT as usize,
            windows::core::w!("Default"),
        );
        // Populate the submenu before attaching it to the main menu. An empty
        // submenu attached first is not guaranteed to become visible later.
        let _ = AppendMenuW(
            menu,
            MF_POPUP,
            position.0 as usize,
            windows::core::w!("⌖  Position"),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            CMD_ABOUT as usize,
            windows::core::w!("ℹ  About"),
        );
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            CMD_EXIT as usize,
            windows::core::w!("✕  Exit"),
        );

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
            app.on_command(command, parent);
        }
    }
}

fn apply_opacity(hwnd: WinHWND, opacity: f64) {
    let opacity = opacity.clamp(0.25, 1.0);
    let alpha = (opacity * 255.0).round() as u8;
    unsafe {
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
    }
}
