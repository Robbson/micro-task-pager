//! Icon lookup and a small per-window cache.
//!
//! The shell returns borrowed `HICON` handles; the application must not call
//! `DestroyIcon` on them, because the window or its class owns them. The
//! fallback icon (`IDI_APPLICATION`) is a shared system icon for the same
//! reason.
//!
//! Phase 3 stores the handle and detects icon changes. Phase 4 converts it to a
//! GDI bitmap when rendering; that is also when a sized bitmap cache matters.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::hash::{Hash, Hasher};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    GCLP_HICON, GetClassLongPtrW, HICON, ICON_SMALL2, IDI_APPLICATION, LoadIconW, SMTO_ABORTIFHUNG,
    SMTO_BLOCK, SendMessageTimeoutW, WM_GETICON,
};

/// A borrowed icon handle supplied by the shell.
///
/// The handle is shared and must not be destroyed by the pager. `Hash` is
/// implemented manually because `HICON` itself does not implement it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowIcon(HICON);

impl WindowIcon {
    /// Returns the raw handle for the renderer. Kept crate-private so the raw
    /// handle does not leak into the public API.
    pub(crate) fn handle(self) -> HICON {
        self.0
    }
}

impl Hash for WindowIcon {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.0.0 as usize).hash(state);
    }
}

/// Per-window icon cache.
///
/// The source handle is queried on every snapshot, because a window can change
/// its icon. The cache keeps one entry per window so the renderer can reuse a
/// converted bitmap later, and `retain` removes entries for windows that are
/// gone.
#[derive(Default)]
pub(crate) struct IconCache {
    entries: HashMap<usize, Option<WindowIcon>>,
}

impl IconCache {
    /// Looks up the current icon and updates the cache entry.
    pub(crate) fn get_or_load(&mut self, hwnd: HWND) -> Option<WindowIcon> {
        let icon = load(hwnd);
        self.entries.insert(hwnd.0 as usize, icon);
        icon
    }

    /// Drops entries for windows that are no longer in the snapshot.
    pub(crate) fn retain(&mut self, active: &HashSet<usize>) {
        self.entries.retain(|hwnd, _| active.contains(hwnd));
    }
}

/// Queries `WM_GETICON`, then the window class icon, then the system default.
///
/// `SendMessageTimeoutW` is used instead of `SendMessage` so a hung application
/// cannot block the pager forever. The timeout is short; the fallback chain
/// still produces an icon for normal windows.
fn load(hwnd: HWND) -> Option<WindowIcon> {
    let mut result = 0usize;
    unsafe {
        let _ = SendMessageTimeoutW(
            hwnd,
            WM_GETICON,
            WPARAM(ICON_SMALL2 as usize),
            LPARAM(0),
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            100,
            Some(&mut result as *mut usize),
        );
    }

    if result != 0 {
        return Some(WindowIcon(HICON(result as *mut c_void)));
    }

    let class_icon = unsafe { GetClassLongPtrW(hwnd, GCLP_HICON) };
    if class_icon != 0 {
        return Some(WindowIcon(HICON(class_icon as *mut c_void)));
    }

    unsafe { LoadIconW(None, IDI_APPLICATION) }
        .ok()
        .map(WindowIcon)
        .filter(|icon| !icon.0.0.is_null())
}
