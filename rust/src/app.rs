//! Application state and event handling.
//!
//! `App` is the single-threaded coordinator between the Win32 window, the
//! virtual desktop API, the window snapshotter, and the renderer. It owns no
//! Win32 window procedure code; `window.rs` forwards messages here.
//!
//! The flow on each timer tick is:
//!
//! 1. Capture a [`WindowSnapshot`] through the supervised desktop API.
//! 2. Rebuild the [`PagerLayout`] for the current monitors and view modes.
//! 3. Resize the window if the desktop count or monitor arrangement changed.
//! 4. Invalidate the window only when the snapshot hash or layout changed.
//!
//! Mouse handling uses the same layout that was painted, so hit testing cannot
//! disagree with the visible cells.

use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::InvalidateRect;
use windows::Win32::UI::Controls::{
    HIMAGELIST, ILC_COLOR32, ILC_MASK, ImageList_BeginDrag, ImageList_Create, ImageList_Destroy,
    ImageList_DragEnter, ImageList_DragLeave, ImageList_DragMove, ImageList_EndDrag,
    ImageList_ReplaceIcon,
};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, GetWindowRect, HWND_TOPMOST, IDC_ARROW, IDC_SIZEALL,
    LoadCursorW, MB_ICONINFORMATION, MB_OK, MessageBoxW, PostMessageW, SM_CXDRAG, SM_CYDRAG,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetCursor, SetForegroundWindow,
    SetWindowPos, WM_CLOSE,
};
use windows::core::{PCWSTR, w};

use crate::desktop::{self, SupervisedDesktop, VirtualDesktopApi};
use crate::logging;
use crate::monitors::{self, MonitorInfo};
use crate::render::{self, PagerLayout};
use crate::settings::{self, Settings, ViewMode};
use crate::windows::{WindowSnapshot, WindowSnapshotter};

pub const CMD_TOGGLE_VIEW: u32 = 1;
pub const CMD_POSITION_RESTORE: u32 = 2;
pub const CMD_POSITION_SAVE: u32 = 3;
pub const CMD_POSITION_DEFAULT: u32 = 4;
pub const CMD_ABOUT: u32 = 5;
pub const CMD_EXIT: u32 = 6;

pub const TIMER_PRIMARY: usize = 1;
pub const TIMER_SECONDARY: usize = 2;

enum DragState {
    Pager {
        start_cursor_x: i32,
        start_cursor_y: i32,
        start_rect: RECT,
    },
    Window {
        hwnd: HWND,
        origin_desktop: u32,
        start_x: i32,
        start_y: i32,
        active: bool,
        image_list: Option<HIMAGELIST>,
    },
}

/// Application state owned by the Win32 message loop.
pub struct App {
    pub settings: Settings,
    pub view_modes: Vec<ViewMode>,
    pub desktop: SupervisedDesktop,
    pub snapshotter: WindowSnapshotter,
    pub snapshot: WindowSnapshot,
    pub monitors: Vec<MonitorInfo>,
    pub layout: PagerLayout,
    pub dpi: u32,
    pub hwnd: HWND,
    window_size: (i32, i32),
    last_hash: u64,
    drag: Option<DragState>,
    menu_point: POINT,
    menu_open: bool,
}

impl App {
    /// Opens the desktop API and captures the first snapshot.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let settings = Settings::load();
        let dpi = unsafe { GetDpiForSystem() };
        let dpi = if dpi == 0 { 96 } else { dpi };
        let monitors = monitors::enumerate();
        let desktop = desktop::open()?;
        let mut snapshotter = WindowSnapshotter::new();
        let snapshot = snapshotter.capture(&desktop)?;
        let view_modes = normalize_view_modes(
            &settings.renderer_view_modes,
            snapshot.desktop_count,
            monitors.len(),
            settings.view_mode_default,
        );
        let layout = render::build(&snapshot, &monitors, &view_modes, &settings, dpi);

        logging::info(format!(
            "pager started: build layout {}x{}, {} desktops, {} monitors, dpi {}",
            layout.width,
            layout.height,
            snapshot.desktop_count,
            monitors.len(),
            dpi
        ));

        Ok(Self {
            settings,
            view_modes,
            desktop,
            snapshotter,
            snapshot,
            monitors,
            window_size: (layout.width, layout.height),
            layout,
            dpi,
            hwnd: HWND::default(),
            last_hash: 0,
            drag: None,
            menu_point: POINT::default(),
            menu_open: false,
        })
    }

    /// Stores the window handle once `CreateWindowExW` has run.
    pub fn set_hwnd(&mut self, hwnd: HWND) {
        self.hwnd = hwnd;
    }

    /// Initial window position: saved location or bottom-center of the primary
    /// monitor's work area.
    pub fn initial_position(&self) -> (i32, i32) {
        settings::load_pager_location().unwrap_or_else(|| self.default_position())
    }

    /// Paints the current layout.
    pub fn on_paint(&self, hwnd: HWND) {
        render::gdi::paint_window(hwnd, &self.layout, &self.snapshot, &self.settings, self.dpi);
    }

    /// Handles the primary and secondary timers.
    pub fn on_timer(&mut self, timer: usize) {
        match timer {
            TIMER_PRIMARY => self.refresh(false),
            TIMER_SECONDARY => {
                let monitors = monitors::enumerate();
                let monitors_changed = monitors != self.monitors;
                if monitors_changed {
                    self.monitors = monitors;
                    self.normalize_view_modes();
                    self.refresh(true);
                } else {
                    match self.desktop.count() {
                        Ok(count) if count != self.snapshot.desktop_count => self.refresh(true),
                        Ok(_) => {}
                        Err(error) => logging::warn(format!("desktop count failed: {error}")),
                    }
                }
                self.reassert_topmost();
            }
            _ => {}
        }
    }

    /// Rebuilds the snapshot and layout, and invalidates only when needed.
    pub fn refresh(&mut self, force: bool) {
        let snapshot = match self.snapshotter.capture(&self.desktop) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                logging::error(format!("snapshot failed: {error}"));
                return;
            }
        };

        let desktop_count_changed = snapshot.desktop_count != self.snapshot.desktop_count;
        self.snapshot = snapshot;

        if desktop_count_changed {
            self.normalize_view_modes();
        }

        self.rebuild_layout();
        let resized = self.resize_window_if_needed();
        let changed = force || resized || self.snapshot.hash != self.last_hash;
        self.last_hash = self.snapshot.hash;

        if changed && !self.hwnd.0.is_null() {
            unsafe {
                let _ = InvalidateRect(Some(self.hwnd), None, false);
            }
        }
    }

    /// Handles the start of a drag or a click.
    pub fn on_mouse_down(&mut self, x: i32, y: i32, middle: bool, shift: bool) {
        if middle || shift {
            let mut rect = RECT::default();
            let mut cursor = POINT::default();
            unsafe {
                let _ = GetWindowRect(self.hwnd, &mut rect);
                let _ = GetCursorPos(&mut cursor);
            }
            self.drag = Some(DragState::Pager {
                start_cursor_x: cursor.x,
                start_cursor_y: cursor.y,
                start_rect: rect,
            });
            set_cursor(IDC_SIZEALL);
            return;
        }

        let Some(hit) = self.layout.hit_test(x, y) else {
            self.drag = None;
            return;
        };

        let Some(hwnd) = hit.hwnd else {
            self.drag = None;
            return;
        };

        self.drag = Some(DragState::Window {
            hwnd,
            origin_desktop: hit.desktop_index,
            start_x: x,
            start_y: y,
            active: false,
            image_list: None,
        });
    }

    /// Moves the pager or arms a window drag.
    pub fn on_mouse_move(&mut self, x: i32, y: i32, left_down: bool) {
        let pager = match &self.drag {
            Some(DragState::Pager {
                start_cursor_x,
                start_cursor_y,
                start_rect,
            }) => Some((*start_cursor_x, *start_cursor_y, *start_rect)),
            _ => None,
        };

        if let Some((start_cursor_x, start_cursor_y, rect)) = pager {
            let mut cursor = POINT::default();
            unsafe {
                let _ = GetCursorPos(&mut cursor);
            }
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    rect.left + cursor.x - start_cursor_x,
                    rect.top + cursor.y - start_cursor_y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            return;
        }

        let drag_threshold_x = unsafe { GetSystemMetrics(SM_CXDRAG) }.max(1);
        let drag_threshold_y = unsafe { GetSystemMetrics(SM_CYDRAG) }.max(1);

        let mut activate = None;
        if let Some(DragState::Window {
            hwnd,
            start_x,
            start_y,
            active,
            ..
        }) = &mut self.drag
            && left_down
            && !*active
            && ((x - *start_x).abs() > drag_threshold_x || (y - *start_y).abs() > drag_threshold_y)
        {
            *active = true;
            activate = Some(*hwnd);
        }

        if let Some(hwnd) = activate {
            self.start_drag_image(hwnd, x, y);
        } else if matches!(
            &self.drag,
            Some(DragState::Window {
                active: true,
                image_list: Some(_),
                ..
            })
        ) {
            unsafe {
                let _ = ImageList_DragMove(x, y);
            }
        }
    }

    /// Creates the semi-transparent ghost icon shown next to the cursor while a
    /// window is dragged.
    ///
    /// `ImageList_BeginDrag`/`ImageList_DragEnter` are the same mechanism used
    /// by the common list controls. The image list owns a copy of the icon, so
    /// it must stay alive until `ImageList_EndDrag`.
    fn start_drag_image(&mut self, hwnd: HWND, x: i32, y: i32) {
        let Some(icon) = self
            .snapshot
            .windows
            .iter()
            .find(|window| window.hwnd == hwnd)
            .and_then(|window| window.icon)
        else {
            return;
        };

        let size = render::scale(16, self.dpi).max(16);
        let image_list = unsafe { ImageList_Create(size, size, ILC_COLOR32 | ILC_MASK, 1, 1) };
        if image_list.0 == 0 {
            return;
        }

        let index = unsafe { ImageList_ReplaceIcon(image_list, -1, icon.handle()) };
        if index < 0 {
            unsafe {
                let _ = ImageList_Destroy(Some(image_list));
            }
            return;
        }

        if !unsafe { ImageList_BeginDrag(image_list, 0, size / 2, size / 2) }.as_bool() {
            unsafe {
                let _ = ImageList_Destroy(Some(image_list));
            }
            return;
        }

        if !unsafe { ImageList_DragEnter(self.hwnd, x, y) }.as_bool() {
            unsafe {
                ImageList_EndDrag();
                let _ = ImageList_Destroy(Some(image_list));
            }
            return;
        }

        if let Some(DragState::Window {
            image_list: slot, ..
        }) = &mut self.drag
        {
            *slot = Some(image_list);
        } else {
            self.destroy_drag_image(image_list);
        }
    }

    fn destroy_drag_image(&self, image_list: HIMAGELIST) {
        unsafe {
            let _ = ImageList_DragLeave(self.hwnd);
            ImageList_EndDrag();
            let _ = ImageList_Destroy(Some(image_list));
        }
    }

    /// Ends any drag in progress and releases the image-list lock. Called when
    /// capture is lost or the window is closing.
    pub fn cancel_drag(&mut self) {
        if let Some(DragState::Window {
            image_list: Some(image_list),
            ..
        }) = self.drag.take()
        {
            self.destroy_drag_image(image_list);
        }
    }

    /// Completes a drag or performs the click action.
    pub fn on_mouse_up(&mut self, x: i32, y: i32) {
        let drag = self.drag.take();
        set_cursor(IDC_ARROW);

        match drag {
            Some(DragState::Pager { .. }) => {}
            Some(DragState::Window {
                hwnd,
                origin_desktop,
                active,
                image_list,
                ..
            }) => {
                if let Some(image_list) = image_list {
                    self.destroy_drag_image(image_list);
                }

                if active {
                    if let Some(target) = self.layout.desktop_at(x)
                        && target != origin_desktop
                        && let Err(error) = self.desktop.move_window_to(hwnd, target)
                    {
                        logging::error(format!("moving window failed: {error}"));
                    }
                    self.refresh(true);
                } else {
                    self.click_at(x, y);
                }
            }
            None => self.click_at(x, y),
        }
    }

    /// Toggles the view mode under the cursor when Shift is held.
    pub fn on_wheel(&mut self, x: i32, y: i32, shift: bool) {
        if shift {
            self.toggle_mode(x, y);
        }
    }

    /// Remembers the client point where the context menu was opened.
    pub fn set_menu_point(&mut self, x: i32, y: i32) {
        self.menu_point = POINT { x, y };
    }

    /// Suspends topmost reassertion while a popup menu is open.
    pub fn set_menu_open(&mut self, open: bool) {
        self.menu_open = open;
    }

    /// Handles a context-menu command.
    pub fn on_command(&mut self, command: u32) {
        match command {
            CMD_TOGGLE_VIEW => self.toggle_mode(self.menu_point.x, self.menu_point.y),
            CMD_POSITION_RESTORE => {
                if let Some((x, y)) = settings::load_pager_location() {
                    self.move_to(x, y);
                }
            }
            CMD_POSITION_SAVE => self.save_position(),
            CMD_POSITION_DEFAULT => {
                let (x, y) = self.default_position();
                self.move_to(x, y);
            }
            CMD_ABOUT => self.show_about(),
            CMD_EXIT => unsafe {
                let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            },
            _ => {}
        }
    }

    fn click_at(&mut self, x: i32, y: i32) {
        let Some(hit) = self.layout.hit_test(x, y) else {
            return;
        };

        if hit.desktop_index != self.snapshot.current_desktop {
            if let Err(error) = self.desktop.switch_to(hit.desktop_index) {
                logging::error(format!("switching desktop failed: {error}"));
            } else {
                self.refresh(true);
            }
            return;
        }

        if let Some(hwnd) = hit.hwnd {
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }

    fn toggle_mode(&mut self, x: i32, y: i32) {
        let Some(hit) = self.layout.hit_test(x, y) else {
            return;
        };
        let index = hit.desktop_index as usize * self.layout.screen_count + hit.screen_index;

        let Some(mode) = self.view_modes.get_mut(index) else {
            return;
        };

        *mode = next_mode(*mode);
        settings::save_view_modes(&self.view_modes);
        self.rebuild_layout();

        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    fn rebuild_layout(&mut self) {
        self.layout = render::build(
            &self.snapshot,
            &self.monitors,
            &self.view_modes,
            &self.settings,
            self.dpi,
        );
    }

    fn resize_window_if_needed(&mut self) -> bool {
        let size = (self.layout.width, self.layout.height);
        if size == self.window_size || self.hwnd.0.is_null() {
            return false;
        }

        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                None,
                0,
                0,
                size.0.max(1),
                size.1.max(1),
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        self.window_size = size;
        true
    }

    fn normalize_view_modes(&mut self) {
        self.view_modes = normalize_view_modes(
            &self.view_modes,
            self.snapshot.desktop_count,
            self.monitors.len(),
            self.settings.view_mode_default,
        );
    }

    fn default_position(&self) -> (i32, i32) {
        let Some(primary) = monitors::primary(&self.monitors) else {
            return (0, 0);
        };

        let x = primary.work_rect.left
            + (primary.work_rect.right - primary.work_rect.left - self.layout.width) / 2;
        let y = primary.work_rect.bottom - self.layout.height;
        (x, y)
    }

    fn move_to(&self, x: i32, y: i32) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                None,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    fn save_position(&self) {
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(self.hwnd, &mut rect) }.is_ok() {
            settings::save_pager_location(rect.left, rect.top);
        }
    }

    fn show_about(&self) {
        unsafe {
            let _ = MessageBoxW(
                Some(self.hwnd),
                w!("Micro Task Pager\nRust migration\n\nVirtual desktop pager"),
                w!("About Micro Task Pager"),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }

    fn reassert_topmost(&self) {
        if self.hwnd.0.is_null() || self.menu_open {
            return;
        }

        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

fn normalize_view_modes(
    existing: &[ViewMode],
    desktop_count: u32,
    screen_count: usize,
    default_mode: ViewMode,
) -> Vec<ViewMode> {
    let needed = (desktop_count as usize).saturating_mul(screen_count);
    let mut modes = vec![default_mode; needed];
    for (index, mode) in existing.iter().take(needed).enumerate() {
        modes[index] = *mode;
    }
    modes
}

fn next_mode(mode: ViewMode) -> ViewMode {
    match mode {
        ViewMode::Preview => ViewMode::MicroTaskbar,
        ViewMode::MicroTaskbar => ViewMode::Preview,
    }
}

fn set_cursor(cursor: PCWSTR) {
    unsafe {
        if let Ok(handle) = LoadCursorW(None, cursor) {
            SetCursor(Some(handle));
        }
    }
}
