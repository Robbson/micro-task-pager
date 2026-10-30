//! Window model: enumerate top-level windows and turn them into a snapshot.
//!
//! This is the Phase 3 layer between the COM desktop API and the future
//! renderer. It does not draw anything. It produces an immutable
//! [`WindowSnapshot`] that the UI can compare against the previous frame:
//!
//! - [`WindowSnapshotter`] is stateful: it remembers desktop-index failures,
//!   blacklisted windows, and icon cache entries between snapshots.
//! - [`WindowSnapshot::hash`] changes when anything that affects rendering
//!   changes, so the UI can skip a repaint when nothing moved.
//!
//! The filters and fallbacks intentionally mirror `WindowManager.cs`; the
//! failure-counter pruning is fixed compared to the C# version (see
//! `ARCHITECTURE.md` §10).

mod icons;

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetShellWindow, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
};
use windows::core::{BOOL, GUID};

use crate::desktop::{ApiError, VirtualDesktopApi};
use crate::logging;

use icons::IconCache;
pub use icons::WindowIcon;

const MAX_DESKTOP_INDEX_FAILURES: u32 = 10;
const CLASS_BLACKLIST: [&str; 1] = ["Windows.UI.Core.CoreWindow"];

/// One top-level window, as needed by the pager.
#[derive(Clone, Debug)]
pub struct WindowInfo {
    /// Native window handle. It is an opaque token, not a Rust pointer.
    pub hwnd: HWND,
    /// Window title as returned by `GetWindowTextW`.
    pub title: String,
    /// Window class name, used for blacklist checks and diagnostics.
    pub class_name: String,
    /// Process that owns the window.
    pub process_id: u32,
    /// Screen rectangle in physical pixels.
    pub rect: RECT,
    /// Index into the desktop list.
    pub desktop_index: u32,
    /// Relative z-order: larger values are closer to the top.
    ///
    /// The value is derived from `EnumWindows` order, which is top-to-bottom.
    /// It is comparable between windows in the same snapshot.
    pub z_order: usize,
    /// Whether this window was the foreground window when the snapshot was
    /// taken.
    pub is_active: bool,
    /// Whether the window is minimized. The snapshot keeps minimized windows
    /// so callers can still map handles, but the renderer skips them.
    pub is_minimized: bool,
    /// Borrowed icon handle supplied by the shell.
    pub icon: Option<WindowIcon>,
}

impl WindowInfo {
    /// Window width in physical pixels.
    pub fn width(&self) -> i32 {
        self.rect.right - self.rect.left
    }

    /// Window height in physical pixels.
    pub fn height(&self) -> i32 {
        self.rect.bottom - self.rect.top
    }
}

/// Immutable state for one frame.
#[derive(Clone, Debug)]
pub struct WindowSnapshot {
    /// Windows after filtering, sorted by handle for a stable list order.
    pub windows: Vec<WindowInfo>,
    /// Desktop GUIDs in index order.
    pub desktop_ids: Vec<GUID>,
    /// Number of virtual desktops.
    pub desktop_count: u32,
    /// Desktop that was current when the snapshot was taken.
    pub current_desktop: u32,
    /// Foreground window, if any.
    pub foreground: Option<HWND>,
    /// Change-detection hash over all fields that affect rendering.
    pub hash: u64,
}

impl WindowSnapshot {
    /// Iterates over windows that belong to one desktop.
    pub fn windows_on_desktop(&self, desktop_index: u32) -> impl Iterator<Item = &WindowInfo> {
        self.windows
            .iter()
            .filter(move |window| window.desktop_index == desktop_index)
    }
}

/// Stateful producer of [`WindowSnapshot`]s.
///
/// Keep one instance for the lifetime of the pager. It owns the failure
/// counters, the blacklist, and the icon cache, so those survive between
/// polling ticks.
#[derive(Default)]
pub struct WindowSnapshotter {
    failures: HashMap<usize, u32>,
    blacklist: HashSet<usize>,
    icons: IconCache,
}

impl WindowSnapshotter {
    /// Creates an empty snapshotter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enumerates windows and builds one snapshot.
    ///
    /// Desktop count/current desktop failures are returned to the caller; the
    /// supervisor above this layer may recreate the COM backend. Per-window
    /// desktop lookup failures are counted and do not fail the whole snapshot.
    pub fn capture(&mut self, api: &dyn VirtualDesktopApi) -> Result<WindowSnapshot, ApiError> {
        let desktop_count = api.count()?;
        let current_desktop = api.current_index()?;
        let desktop_ids = api.desktop_ids()?;
        let foreground = unsafe { GetForegroundWindow() };
        let shell_window = unsafe { GetShellWindow() };

        let (mut windows, seen, included, total) = {
            let mut builder = Builder {
                api,
                desktop_count,
                foreground,
                shell_window,
                own_process_id: std::process::id(),
                failures: &mut self.failures,
                blacklist: &mut self.blacklist,
                icons: &mut self.icons,
                windows: Vec::new(),
                seen: HashSet::new(),
                included: HashSet::new(),
                total: 0,
            };

            unsafe {
                EnumWindows(
                    Some(enum_windows_proc),
                    LPARAM(&mut builder as *mut Builder<'_> as isize),
                )?;
            }

            (
                builder.windows,
                builder.seen,
                builder.included,
                builder.total,
            )
        };

        // `EnumWindows` visits windows from top to bottom. Convert the temporary
        // top-relative index into "distance from the bottom", so larger values
        // mean closer to the top, matching the C# model.
        for window in &mut windows {
            window.z_order = total.saturating_sub(1).saturating_sub(window.z_order);
        }

        // A stable order makes the snapshot hash and future diffing predictable.
        windows.sort_by_key(|window| window.hwnd.0 as usize);

        // Remove state for windows that no longer reach the desktop lookup.
        self.failures.retain(|hwnd, _| seen.contains(hwnd));
        self.icons.retain(&included);
        let mut snapshot = WindowSnapshot {
            windows,
            desktop_ids,
            desktop_count,
            current_desktop,
            foreground: (!foreground.0.is_null()).then_some(foreground),
            hash: 0,
        };
        snapshot.hash = hash_snapshot(&snapshot);

        Ok(snapshot)
    }
}

struct Builder<'a> {
    api: &'a dyn VirtualDesktopApi,
    desktop_count: u32,
    foreground: HWND,
    shell_window: HWND,
    own_process_id: u32,
    failures: &'a mut HashMap<usize, u32>,
    blacklist: &'a mut HashSet<usize>,
    icons: &'a mut IconCache,
    windows: Vec<WindowInfo>,
    seen: HashSet<usize>,
    included: HashSet<usize>,
    total: usize,
}

impl Builder<'_> {
    fn visit(&mut self, hwnd: HWND) {
        let z_from_top = self.total;
        self.total += 1;
        let hwnd_key = hwnd.0 as usize;

        if hwnd == self.shell_window {
            return;
        }

        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return;
        }

        let title_length = unsafe { GetWindowTextLengthW(hwnd) };
        if title_length <= 0 {
            return;
        }

        let class_name = class_name(hwnd);
        if CLASS_BLACKLIST.contains(&class_name.as_str()) {
            return;
        }

        let mut process_id = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut process_id as *mut u32));
        }
        if process_id == self.own_process_id {
            return;
        }

        if self.blacklist.contains(&hwnd_key) {
            return;
        }

        // This window passed the cheap filters, so it is safe to remove a stale
        // failure counter if the desktop lookup succeeds or reports "unknown".
        self.seen.insert(hwnd_key);

        let desktop_index = match self.api.index_of_window(hwnd) {
            Ok(Some(index)) if index < self.desktop_count => {
                self.failures.remove(&hwnd_key);
                index
            }
            Ok(_) => {
                self.failures.remove(&hwnd_key);
                return;
            }
            Err(error) => {
                self.record_failure(hwnd, error);
                return;
            }
        };

        let title = window_text(hwnd, title_length);
        let mut rect = RECT::default();
        let _ = unsafe { GetWindowRect(hwnd, &mut rect) };
        let icon = self.icons.get_or_load(hwnd);

        self.included.insert(hwnd_key);
        self.windows.push(WindowInfo {
            hwnd,
            title,
            class_name,
            process_id,
            rect,
            desktop_index,
            // Temporary top-relative value; `capture` converts it after the
            // enumeration when the total count is known.
            z_order: z_from_top,
            is_active: hwnd == self.foreground,
            is_minimized: unsafe { IsIconic(hwnd) }.as_bool(),
            icon,
        });
    }

    fn record_failure(&mut self, hwnd: HWND, error: ApiError) {
        let hwnd_key = hwnd.0 as usize;
        let count = self.failures.entry(hwnd_key).or_insert(0);
        *count += 1;
        let count = *count;

        if count >= MAX_DESKTOP_INDEX_FAILURES {
            self.blacklist.insert(hwnd_key);
            self.failures.remove(&hwnd_key);
            logging::warn(format!(
                "window {hwnd:?} desktop lookup failed {count} times; blacklisted: {error}"
            ));
        }
    }
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `capture` passes a pointer to a live `Builder`; the callback runs
    // synchronously inside `EnumWindows` and does not retain the reference.
    let builder = unsafe { &mut *(lparam.0 as *mut Builder<'_>) };
    builder.visit(hwnd);
    BOOL(1)
}

fn window_text(hwnd: HWND, length: i32) -> String {
    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
}

fn class_name(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    let copied = unsafe { GetClassNameW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
}

fn hash_snapshot(snapshot: &WindowSnapshot) -> u64 {
    let mut hasher = DefaultHasher::new();

    snapshot.desktop_count.hash(&mut hasher);
    snapshot.current_desktop.hash(&mut hasher);
    snapshot
        .foreground
        .map(|hwnd| hwnd.0 as usize)
        .hash(&mut hasher);
    for id in &snapshot.desktop_ids {
        id.hash(&mut hasher);
    }

    for window in &snapshot.windows {
        (window.hwnd.0 as usize).hash(&mut hasher);
        window.title.hash(&mut hasher);
        window.class_name.hash(&mut hasher);
        window.process_id.hash(&mut hasher);
        window.rect.left.hash(&mut hasher);
        window.rect.top.hash(&mut hasher);
        window.rect.right.hash(&mut hasher);
        window.rect.bottom.hash(&mut hasher);
        window.desktop_index.hash(&mut hasher);
        window.z_order.hash(&mut hasher);
        window.is_active.hash(&mut hasher);
        window.is_minimized.hash(&mut hasher);
        window
            .icon
            .map(|icon| icon.handle().0 as usize)
            .hash(&mut hasher);
    }

    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::c_void;

    fn sample_snapshot(title: &str) -> WindowSnapshot {
        WindowSnapshot {
            windows: vec![WindowInfo {
                hwnd: HWND(std::ptr::dangling_mut::<c_void>()),
                title: title.to_string(),
                class_name: "TestWindow".to_string(),
                process_id: 42,
                rect: RECT {
                    left: 0,
                    top: 0,
                    right: 100,
                    bottom: 50,
                },
                desktop_index: 0,
                z_order: 0,
                is_active: true,
                is_minimized: false,
                icon: None,
            }],
            desktop_ids: vec![GUID::zeroed()],
            desktop_count: 1,
            current_desktop: 0,
            foreground: Some(HWND(std::ptr::dangling_mut::<c_void>())),
            hash: 0,
        }
    }

    #[test]
    fn hash_is_stable_for_identical_snapshots() {
        assert_eq!(
            hash_snapshot(&sample_snapshot("Editor")),
            hash_snapshot(&sample_snapshot("Editor"))
        );
    }

    #[test]
    fn hash_changes_when_a_rendered_field_changes() {
        assert_ne!(
            hash_snapshot(&sample_snapshot("Editor")),
            hash_snapshot(&sample_snapshot("Terminal"))
        );
    }

    #[test]
    fn windows_can_be_filtered_by_desktop() {
        let mut snapshot = sample_snapshot("Editor");
        snapshot.windows.push(WindowInfo {
            desktop_index: 1,
            ..snapshot.windows[0].clone()
        });

        assert_eq!(snapshot.windows_on_desktop(0).count(), 1);
        assert_eq!(snapshot.windows_on_desktop(1).count(), 1);
        assert_eq!(snapshot.windows_on_desktop(2).count(), 0);
    }
}
