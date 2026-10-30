//! Tracks the last real foreground window per virtual desktop.
//!
//! Tracking is event-driven: `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` avoids
//! missing a quick activation that a 200 ms poll would skip. The hook is
//! `WINEVENT_OUTOFCONTEXT`, so Windows delivers callbacks on the UI thread's
//! message loop.
//!
//! The callback cannot access the desktop API, so it only stores the handle in
//! an atomic. The next timer tick in [`crate::app::App`] consumes it and maps
//! the window to a desktop. This keeps COM calls on the UI thread and keeps the
//! `extern "system"` callback tiny.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_SYSTEM_FOREGROUND, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, SW_RESTORE, SetForegroundWindow, ShowWindow, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS,
};

use crate::desktop::VirtualDesktopApi;
use crate::logging;

static PENDING_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

/// Last foreground window for each desktop index.
pub struct ForegroundTracker {
    hook: HWINEVENTHOOK,
    last_active: HashMap<u32, HWND>,
    ignored_window: HWND,
    own_process_id: u32,
}

impl ForegroundTracker {
    /// Installs the foreground hook.
    ///
    /// `ignored_window` is the pager itself; its own activations must not become
    /// the "last active window" of a desktop.
    pub fn install(ignored_window: HWND) -> Result<Self, windows::core::Error> {
        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(foreground_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };

        if hook.0.is_null() {
            return Err(windows::core::Error::from_thread());
        }

        Ok(Self {
            hook,
            last_active: HashMap::new(),
            ignored_window,
            own_process_id: std::process::id(),
        })
    }

    /// Consumes a pending foreground event recorded by the hook callback.
    pub fn take_pending() -> Option<HWND> {
        let value = PENDING_FOREGROUND.swap(0, Ordering::AcqRel);
        if value == 0 {
            None
        } else {
            Some(HWND(value as *mut c_void))
        }
    }

    /// Records `hwnd` as the last active window of its desktop.
    pub fn remember(&mut self, hwnd: HWND, desktop: &dyn VirtualDesktopApi) {
        if hwnd.0.is_null() || hwnd == self.ignored_window {
            return;
        }

        let mut process_id = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut process_id as *mut u32));
        }
        if process_id == self.own_process_id {
            return;
        }

        if unsafe { GetWindowTextLengthW(hwnd) } <= 0 {
            return;
        }

        if let Ok(Some(index)) = desktop.index_of_window(hwnd) {
            self.last_active.insert(index, hwnd);
        }
    }

    /// Restores the remembered window for `desktop_index`, if it is still valid
    /// and still belongs to that desktop.
    pub fn restore(&mut self, desktop_index: u32, desktop: &dyn VirtualDesktopApi) {
        let Some(&hwnd) = self.last_active.get(&desktop_index) else {
            return;
        };

        if hwnd.0.is_null()
            || !unsafe { IsWindow(Some(hwnd)) }.as_bool()
            || !unsafe { IsWindowVisible(hwnd) }.as_bool()
        {
            self.last_active.remove(&desktop_index);
            return;
        }

        match desktop.index_of_window(hwnd) {
            Ok(Some(index)) if index == desktop_index => {}
            _ => return,
        }

        if unsafe { IsIconic(hwnd) }.as_bool() {
            unsafe {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
        }

        if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
            logging::info(format!(
                "could not restore last active window {hwnd:?} on desktop {desktop_index}"
            ));
        }
    }
}

impl Drop for ForegroundTracker {
    fn drop(&mut self) {
        if !self.hook.0.is_null() {
            unsafe {
                let _ = UnhookWinEvent(self.hook);
            }
        }
    }
}

unsafe extern "system" fn foreground_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND {
        PENDING_FOREGROUND.store(hwnd.0 as isize, Ordering::Release);
    }
}
