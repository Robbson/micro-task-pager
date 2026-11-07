//! Reactor pager component.
//!
//! The component owns the snapshot, layout, input state, icon sources, and the
//! native window shim. Reactor has no timer API, so both polling loops are
//! self-rescheduling background tasks; the background closure only sleeps and
//! returns a message. COM and window enumeration run in `update`, which reactor
//! always calls on the UI thread.

mod icons;
mod input;
mod layout;
mod view;

use std::time::Duration;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::w;
use windows_reactor::{App, Component, ComponentContext, LocalSender, View, ViewContext};

use crate::desktop::{self, SupervisedDesktop, VirtualDesktopApi};
use crate::dialogs::{DialogEvent, DialogLink, about, settings as settings_dialog};
use crate::foreground::ForegroundTracker;
use crate::monitors::{self, MonitorInfo};
use crate::native_window::{self, ContextCommand, PagerWindow, WheelHook};
use crate::settings::{self, Settings, ViewMode};
use crate::windows::{WindowSnapshot, WindowSnapshotter};

use icons::IconSources;
use input::{InputState, ReleaseAction};
use layout::PagerLayout;

/// Window title used both by reactor and by the native HWND lookup.
const WINDOW_TITLE: &str = "Micro Task Pager";
const MIN_TICK_DELAY_MS: u32 = 16;
const ATTACH_RETRY_DELAY_MS: u32 = 50;
const RESTORE_DELAY_MS: u64 = 150;
const ERROR_WINDOW_SIZE: (f64, f64) = (420.0, 160.0);

/// Messages handled on the reactor UI thread.
#[derive(Clone, Debug)]
pub enum Message {
    /// Snapshot and layout refresh.
    PrimaryTick,
    /// Topmost, pinning, monitor, and foreground maintenance.
    SecondaryTick,
    PointerPressed {
        x: f64,
        y: f64,
        left: bool,
        middle: bool,
        right: bool,
        shift: bool,
    },
    PointerMoved {
        x: f64,
        y: f64,
    },
    PointerReleased {
        x: f64,
        y: f64,
    },
    PointerCaptureLost,
    Wheel {
        x: f64,
        y: f64,
        shift: bool,
    },
    RestoreLastActive(u32),
    /// A live preview or close notification from an About/Settings window.
    Dialog(DialogEvent),
    /// Closes the window, used by the startup-error view.
    Close,
}

/// Reactor pager component.
pub struct Pager {
    settings: Settings,
    view_modes: Vec<Vec<ViewMode>>,
    desktop: Option<SupervisedDesktop>,
    snapshotter: WindowSnapshotter,
    snapshot: WindowSnapshot,
    icons: IconSources,
    monitors: Vec<MonitorInfo>,
    layout: PagerLayout,
    window: Option<PagerWindow>,
    wheel_hook: Option<WheelHook>,
    pinned: bool,
    client_size: (f64, f64),
    startup_error: Option<String>,
    sender: LocalSender<Message>,
    input: InputState,
    menu_point: (f64, f64),
    foreground: Option<ForegroundTracker>,
    startup_foreground: Option<HWND>,
    /// True while a secondary About or Settings window is open.
    dialog_open: bool,
    /// Whether the open dialog's HWND has been observed yet.
    dialog_seen: bool,
    /// Ticks since the dialog window disappeared; a fallback for a missing
    /// close notification.
    dialog_missing_ticks: u32,
}

/// Starts the reactor pager window.
///
/// The caller must initialize the COM apartment on this thread first. Reactor
/// keeps the WinUI message loop on the same thread, so every virtual-desktop
/// call made by the component stays in the apartment.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Pinning to all desktops works by application ID, not by window handle, so
    // the ID must be set before reactor creates the window.
    // SAFETY: the string is a valid static UTF-16 literal and the call only
    // changes process-wide shell metadata.
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(w!("MicroTaskPager.Rust"));
    }

    App::run_component::<Pager>(()).map_err(Into::into)
}

impl Component for Pager {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), context: &ComponentContext<Self>) -> Self {
        let settings = Settings::load();
        let startup_foreground = native_window::foreground_window();
        let sender = context.sender();

        match Self::start(settings.clone(), sender.clone(), startup_foreground) {
            Ok(pager) => {
                Self::schedule_primary(context, MIN_TICK_DELAY_MS);
                Self::schedule_secondary(context, settings.secondary_update_delay);
                pager
            }
            Err(error) => {
                crate::logging::error(format!("reactor pager startup failed: {error}"));
                Self::failed(settings, sender, error)
            }
        }
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message {
            Message::PrimaryTick => {
                self.refresh_snapshot();
                self.attach_window();
                self.sync_window_size();
                self.poll_foreground();
                self.maintain_dialog();
                let delay = if self.window.is_some() {
                    self.settings.primary_update_delay
                } else {
                    ATTACH_RETRY_DELAY_MS
                };
                Self::schedule_primary(context, delay);
            }
            Message::SecondaryTick => {
                self.refresh_environment();
                self.attach_window();
                self.maintain_window();
                self.maintain_dialog();
                self.try_pin();
                let delay = if self.window.is_some() {
                    self.settings.secondary_update_delay
                } else {
                    ATTACH_RETRY_DELAY_MS
                };
                Self::schedule_secondary(context, delay);
            }
            Message::PointerPressed {
                x,
                y,
                left,
                middle,
                right,
                shift,
            } => {
                if right {
                    self.input.right_press();
                    native_window::set_drag_cursor(false);
                    self.menu_point = (x, y);
                    if let Some(hwnd) = self.window.as_ref().map(PagerWindow::hwnd)
                        && let Some(command) =
                            native_window::show_context_menu(hwnd, self.dialog_open)
                    {
                        self.run_menu_command(command, context);
                    }
                } else if left || middle {
                    let hit = self.layout.hit_test(x, y);
                    let cursor = native_window::cursor_position();
                    let origin = self.window.as_ref().and_then(PagerWindow::origin);
                    self.input.press(input::PressInput {
                        hit,
                        x,
                        y,
                        middle,
                        shift,
                        cursor,
                        origin,
                    });
                    if matches!(self.input.drag(), Some(input::Drag::Pager { .. })) {
                        native_window::set_drag_cursor(true);
                    }
                }
            }
            Message::PointerMoved { x, y } => {
                if let Some(input::Drag::Pager {
                    start_cursor,
                    start_origin,
                }) = self.input.drag()
                {
                    if let Some(cursor) = native_window::cursor_position()
                        && let Some(window) = self.window.as_ref()
                    {
                        let _ = window.move_to(
                            start_origin.0 + cursor.0 - start_cursor.0,
                            start_origin.1 + cursor.1 - start_cursor.1,
                        );
                    }
                    // WinUI may reset its cursor during the pointer move, so
                    // reapply the move cursor after each drag update.
                    native_window::set_drag_cursor(true);
                } else if self.input.pointer_moved(x, y) {
                    native_window::set_drag_cursor(true);
                }
            }
            Message::PointerReleased { x, y } => {
                native_window::set_drag_cursor(false);
                match self.input.release(&self.layout, x, y) {
                    ReleaseAction::None => {}
                    ReleaseAction::Click { desktop, hwnd } => self.click(desktop, hwnd, context),
                    ReleaseAction::MoveWindow {
                        hwnd,
                        target_desktop,
                    } => self.move_window(hwnd, target_desktop),
                }
            }
            Message::PointerCaptureLost => {
                // Reactor can lose capture while a view rebuild follows a
                // pointer move. Keep an active gesture alive; the release still
                // arrives with the final coordinates. Only clear when no drag
                // is in progress.
                if self.input.drag().is_none() {
                    self.input.cancel();
                    native_window::set_drag_cursor(false);
                }
            }
            Message::Wheel { x, y, shift } => {
                if shift {
                    self.toggle_mode(x, y);
                }
            }
            Message::RestoreLastActive(desktop) => self.restore_last_active(desktop),
            Message::Dialog(event) => self.handle_dialog_event(event),
            Message::Close => {
                let _ = context.window().request_close();
            }
        }
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        view::build(self, context)
    }
}

impl Pager {
    fn start(
        settings: Settings,
        sender: LocalSender<Message>,
        startup_foreground: Option<HWND>,
    ) -> Result<Self, String> {
        let monitors = monitors::enumerate();
        let desktop = desktop::open().map_err(|error| error.to_string())?;
        let mut snapshotter = WindowSnapshotter::new();
        let snapshot = snapshotter
            .capture(&desktop)
            .map_err(|error| error.to_string())?;
        let view_modes = initial_view_modes(&settings, snapshot.desktop_count, monitors.len());
        let mut icons = IconSources::default();
        icons.refresh(&snapshot, icon_pixels(native_window::system_dpi()));
        let layout = layout::build(
            &snapshot,
            &monitors,
            &view_modes,
            &settings,
            native_window::system_dpi(),
        );
        let client_size = positive_size(&layout);

        crate::logging::info(format!(
            "reactor pager started: {}x{} dip, {} desktops, {} monitors, {} windows",
            client_size.0,
            client_size.1,
            snapshot.desktop_count,
            monitors.len(),
            snapshot.windows.len()
        ));

        Ok(Self {
            settings,
            view_modes,
            desktop: Some(desktop),
            snapshotter,
            snapshot,
            icons,
            monitors,
            layout,
            window: None,
            wheel_hook: None,
            pinned: false,
            client_size,
            startup_error: None,
            sender,
            input: InputState::default(),
            menu_point: (0.0, 0.0),
            foreground: None,
            startup_foreground,
            dialog_open: false,
            dialog_seen: false,
            dialog_missing_ticks: 0,
        })
    }

    fn failed(settings: Settings, sender: LocalSender<Message>, error: String) -> Self {
        Self {
            settings,
            view_modes: Vec::new(),
            desktop: None,
            snapshotter: WindowSnapshotter::new(),
            snapshot: empty_snapshot(),
            icons: IconSources::default(),
            monitors: monitors::enumerate(),
            layout: PagerLayout::default(),
            window: None,
            wheel_hook: None,
            pinned: false,
            client_size: ERROR_WINDOW_SIZE,
            startup_error: Some(error),
            sender,
            input: InputState::default(),
            menu_point: (0.0, 0.0),
            foreground: None,
            startup_foreground: None,
            dialog_open: false,
            dialog_seen: false,
            dialog_missing_ticks: 0,
        }
    }

    fn schedule_primary(context: &ComponentContext<Self>, delay: u32) {
        let delay = Duration::from_millis(u64::from(delay.max(MIN_TICK_DELAY_MS)));
        let _ = context.spawn_background(move |_| {
            std::thread::sleep(delay);
            Message::PrimaryTick
        });
    }

    fn schedule_secondary(context: &ComponentContext<Self>, delay: u32) {
        let delay = Duration::from_millis(u64::from(delay.max(MIN_TICK_DELAY_MS)));
        let _ = context.spawn_background(move |_| {
            std::thread::sleep(delay);
            Message::SecondaryTick
        });
    }

    fn schedule_restore(context: &ComponentContext<Self>, desktop: u32) {
        let delay = Duration::from_millis(RESTORE_DELAY_MS);
        let _ = context.spawn_background(move |_| {
            std::thread::sleep(delay);
            Message::RestoreLastActive(desktop)
        });
    }

    fn refresh_snapshot(&mut self) {
        let Some(desktop) = self.desktop.as_ref() else {
            return;
        };

        let previous_count = self.snapshot.desktop_count;
        let previous_hash = self.snapshot.hash;
        match self.snapshotter.capture(desktop) {
            Ok(snapshot) => {
                let desktop_count = snapshot.desktop_count.max(1);
                let hash_changed = snapshot.hash != previous_hash;
                let desktop_count_changed = desktop_count != previous_count;
                self.snapshot = snapshot;
                if desktop_count_changed {
                    self.resize_view_modes(desktop_count, self.monitors.len());
                }
                if hash_changed {
                    self.refresh_icons();
                }
                if hash_changed || desktop_count_changed {
                    self.rebuild_layout();
                }
            }
            Err(error) => crate::logging::error(format!("snapshot failed: {error}")),
        }
    }

    fn refresh_environment(&mut self) {
        let monitors = monitors::enumerate();
        if monitors != self.monitors {
            self.monitors = monitors;
            self.resize_view_modes(self.snapshot.desktop_count.max(1), self.monitors.len());
            self.rebuild_layout();
            self.refresh_icons();
        }
        self.sync_icon_size();

        let Some(desktop) = self.desktop.as_ref() else {
            return;
        };
        match desktop.count() {
            Ok(count) if count != self.snapshot.desktop_count => self.refresh_snapshot(),
            Ok(_) => {}
            Err(error) => crate::logging::warn(format!("desktop count failed: {error}")),
        }
    }

    fn rebuild_layout(&mut self) {
        let dpi = self
            .window
            .as_ref()
            .map(|window| native_window::window_dpi(window.hwnd()))
            .unwrap_or_else(native_window::system_dpi);
        self.layout = layout::build(
            &self.snapshot,
            &self.monitors,
            &self.view_modes,
            &self.settings,
            dpi,
        );
        self.client_size = positive_size(&self.layout);
    }

    fn resize_view_modes(&mut self, desktop_count: u32, screen_count: usize) {
        let desktop_count = desktop_count.max(1) as usize;
        let mut modes = vec![vec![self.settings.view_mode_default; screen_count]; desktop_count];

        for (desktop, row) in self.view_modes.iter().enumerate().take(desktop_count) {
            for (screen, mode) in row.iter().enumerate().take(screen_count) {
                modes[desktop][screen] = *mode;
            }
        }

        self.view_modes = modes;
    }

    fn poll_foreground(&mut self) {
        let Some(hwnd) = ForegroundTracker::take_pending() else {
            return;
        };
        if let (Some(tracker), Some(desktop)) = (self.foreground.as_mut(), self.desktop.as_ref()) {
            tracker.remember(hwnd, desktop);
        }
    }

    fn attach_window(&mut self) {
        if self.window.is_some() {
            return;
        }

        let Some(hwnd) = native_window::find_pager_window(WINDOW_TITLE) else {
            return;
        };

        let saved = settings::load_pager_location();
        let window = match PagerWindow::attach(
            hwnd,
            self.settings.show_app_in_taskbar,
            self.settings.background_opacity,
        ) {
            Ok(window) => window,
            Err(error) => {
                crate::logging::warn(format!("attaching the pager window failed: {error}"));
                return;
            }
        };

        if let Err(error) = window.ensure_size(self.client_size.0, self.client_size.1) {
            crate::logging::warn(format!("sizing the pager window failed: {error}"));
        }
        if let Err(error) = window.place(saved) {
            crate::logging::warn(format!("placing the pager window failed: {error}"));
        }
        if let Err(error) = window.reassert_topmost() {
            crate::logging::warn(format!("setting pager topmost failed: {error}"));
        }

        let sender = self.sender.clone();
        let wheel_target = native_window::find_content_window(hwnd).unwrap_or(hwnd);
        match WheelHook::install(hwnd, move |x, y, shift| {
            if shift {
                sender.send(Message::Wheel { x, y, shift })
            } else {
                false
            }
        }) {
            Ok(hook) => {
                self.wheel_hook = Some(hook);
                crate::logging::info(format!(
                    "wheel/cursor hooks installed; cursor subclass on {wheel_target:?}"
                ));
            }
            Err(error) => {
                crate::logging::warn(format!("installing the wheel hook failed: {error}"))
            }
        }

        self.install_foreground(hwnd);
        crate::logging::info(format!("reactor pager window attached: {hwnd:?}"));
        self.window = Some(window);
        self.refresh_icons();
    }

    fn refresh_icons(&mut self) {
        let dpi = self
            .window
            .as_ref()
            .map(|window| native_window::window_dpi(window.hwnd()))
            .unwrap_or_else(native_window::system_dpi);
        self.icons.refresh(&self.snapshot, icon_pixels(dpi));
    }

    fn sync_icon_size(&mut self) {
        let dpi = self
            .window
            .as_ref()
            .map(|window| native_window::window_dpi(window.hwnd()))
            .unwrap_or_else(native_window::system_dpi);
        if self.icons.size_px() != icon_pixels(dpi) {
            self.refresh_icons();
        }
    }

    fn install_foreground(&mut self, hwnd: HWND) {
        if self.foreground.is_some() {
            return;
        }

        match ForegroundTracker::install(hwnd) {
            Ok(mut tracker) => {
                if let (Some(startup), Some(desktop)) =
                    (self.startup_foreground.take(), self.desktop.as_ref())
                {
                    tracker.remember(startup, desktop);
                }
                self.foreground = Some(tracker);
            }
            Err(error) => {
                crate::logging::warn(format!("installing the foreground tracker failed: {error}"));
            }
        }
    }

    fn sync_window_size(&self) {
        if let Some(window) = &self.window
            && let Err(error) = window.ensure_size(self.client_size.0, self.client_size.1)
        {
            crate::logging::warn(format!("sizing the pager window failed: {error}"));
        }
    }

    fn maintain_window(&mut self) {
        let Some(window) = self.window.as_mut() else {
            return;
        };

        if let Err(error) = window.ensure_styles() {
            crate::logging::warn(format!("maintaining pager styles failed: {error}"));
        }
        if let Err(error) = window.ensure_size(self.client_size.0, self.client_size.1) {
            crate::logging::warn(format!("maintaining pager size failed: {error}"));
        }
        // While a dialog is open, its window must stay above the pager.
        if self.dialog_open {
            return;
        }
        if let Err(error) = window.reassert_topmost() {
            crate::logging::warn(format!("reasserting pager topmost failed: {error}"));
        }
        if window.is_covered() {
            window.restore_covered();
        }
    }

    fn try_pin(&mut self) {
        if self.pinned || !self.settings.show_app_in_taskbar {
            return;
        }

        let Some(hwnd) = self.window.as_ref().map(PagerWindow::hwnd) else {
            return;
        };
        let Some(desktop) = self.desktop.as_ref() else {
            return;
        };

        match desktop.pin_application(hwnd) {
            Ok(_) => {
                self.pinned = true;
                crate::logging::info("reactor pager application pinned to all desktops");
            }
            Err(error) => {
                crate::logging::warn(format!("pinning the reactor pager failed: {error}"));
            }
        }
    }

    fn click(&mut self, desktop_index: u32, hwnd: Option<HWND>, context: &ComponentContext<Self>) {
        if desktop_index != self.snapshot.current_desktop {
            let result = self
                .desktop
                .as_ref()
                .map(|desktop| desktop.switch_to(desktop_index));
            match result {
                Some(Ok(())) => {
                    self.refresh_snapshot();
                    Self::schedule_restore(context, desktop_index);
                }
                Some(Err(error)) => {
                    crate::logging::warn(format!("switching desktops failed: {error}"));
                }
                None => {}
            }
        } else if let Some(hwnd) = hwnd
            && !native_window::activate_window(hwnd)
        {
            crate::logging::warn(format!("could not focus window {hwnd:?}"));
        }
    }

    fn move_window(&mut self, hwnd: HWND, target_desktop: u32) {
        let Some(desktop) = self.desktop.as_ref() else {
            return;
        };

        match desktop.move_window_to(hwnd, target_desktop) {
            Ok(()) => {
                crate::logging::info(format!("moved window {hwnd:?} to desktop {target_desktop}"));
                self.refresh_snapshot();
            }
            Err(error) => {
                crate::logging::warn(format!("moving window {hwnd:?} failed: {error}"));
            }
        }
    }

    fn restore_last_active(&mut self, desktop_index: u32) {
        // Refresh first so a just-completed switch is reflected in
        // `current_desktop` before deciding whether to restore.
        self.refresh_snapshot();
        if self.snapshot.current_desktop != desktop_index {
            return;
        }
        if let (Some(tracker), Some(desktop)) = (self.foreground.as_mut(), self.desktop.as_ref()) {
            tracker.restore(desktop_index, desktop);
        }
    }

    fn toggle_mode(&mut self, x: f64, y: f64) {
        let Some(hit) = self.layout.hit_test(x, y) else {
            return;
        };
        let Some(row) = self.view_modes.get_mut(hit.desktop_index as usize) else {
            return;
        };
        let Some(mode) = row.get_mut(hit.screen_index) else {
            return;
        };

        *mode = match *mode {
            ViewMode::Preview => ViewMode::MicroTaskbar,
            ViewMode::MicroTaskbar => ViewMode::Preview,
        };
        let new_mode = *mode;
        let flattened = flatten_view_modes(&self.view_modes);
        settings::save_view_modes(&flattened);
        self.rebuild_layout();
        crate::logging::info(format!(
            "view mode for desktop {} screen {} is {new_mode:?}",
            hit.desktop_index, hit.screen_index
        ));
    }

    fn dialog_link(&self) -> DialogLink {
        let sender = self.sender.clone();
        DialogLink::new(move |event| {
            let _ = sender.send(Message::Dialog(event));
        })
    }

    fn open_settings(&mut self, context: &ComponentContext<Self>) {
        if self.dialog_open {
            return;
        }

        let input =
            settings_dialog::SettingsInput::linked(self.settings.clone(), self.dialog_link());
        if context.open_window(View::component::<settings_dialog::SettingsDialog>(input)) {
            self.dialog_open = true;
            self.dialog_seen = false;
            self.dialog_missing_ticks = 0;
            crate::logging::info("opened the Settings window");
        } else {
            crate::logging::warn("the Settings window request was rejected");
        }
    }

    fn open_about(&mut self, context: &ComponentContext<Self>) {
        if self.dialog_open {
            return;
        }

        let input = about::AboutInput::linked(self.dialog_link());
        if context.open_window(View::component::<about::About>(input)) {
            self.dialog_open = true;
            self.dialog_seen = false;
            self.dialog_missing_ticks = 0;
            crate::logging::info("opened the About window");
        } else {
            crate::logging::warn("the About window request was rejected");
        }
    }

    fn handle_dialog_event(&mut self, event: DialogEvent) {
        match event {
            DialogEvent::Preview(settings) => self.apply_settings(settings, false),
            DialogEvent::Applied(settings) => self.apply_settings(settings, true),
            DialogEvent::Cancelled(settings) => self.apply_settings(settings, false),
            DialogEvent::Closed => {
                self.dialog_open = false;
                self.dialog_seen = false;
                self.dialog_missing_ticks = 0;
                if let Some(window) = &self.window {
                    let _ = window.reassert_topmost();
                }
            }
        }
    }

    /// Applies a complete settings snapshot to the live pager.
    ///
    /// Color and opacity changes are picked up by the next view publication.
    /// Layout-affecting values rebuild the layout immediately so a preview of
    /// the pager height or icon padding is visible while the dialog is open.
    fn apply_settings(&mut self, settings: Settings, persist: bool) {
        let pager_height_changed = settings.pager_height != self.settings.pager_height;
        let icon_padding_changed = settings.icon_padding_x != self.settings.icon_padding_x
            || settings.icon_padding_y != self.settings.icon_padding_y;
        let taskbar_changed = settings.show_app_in_taskbar != self.settings.show_app_in_taskbar;
        let opacity_changed = settings.background_opacity != self.settings.background_opacity;

        self.settings = settings;

        if let Some(window) = self.window.as_mut() {
            if taskbar_changed {
                window.set_show_in_taskbar(self.settings.show_app_in_taskbar);
            }
            if opacity_changed {
                window.set_opacity(self.settings.background_opacity);
            }
            if taskbar_changed && let Err(error) = window.ensure_styles() {
                crate::logging::warn(format!("applying settings styles failed: {error}"));
            }
        }

        if pager_height_changed || icon_padding_changed {
            self.rebuild_layout();
            self.sync_window_size();
        }

        if persist {
            self.settings.renderer_view_modes = flatten_view_modes(&self.view_modes);
            self.settings.save();
            crate::logging::info("settings applied and saved");
        }
    }

    /// Keeps the open dialog above the topmost pager and clears a stale
    /// `dialog_open` flag if its window disappeared without a close message.
    fn maintain_dialog(&mut self) {
        if !self.dialog_open {
            return;
        }

        let hwnd = native_window::find_pager_window(settings_dialog::WINDOW_TITLE)
            .or_else(|| native_window::find_pager_window(about::WINDOW_TITLE));

        if let Some(hwnd) = hwnd {
            self.dialog_seen = true;
            self.dialog_missing_ticks = 0;
            if let Err(error) = native_window::reassert_topmost(hwnd) {
                crate::logging::warn(format!("reasserting dialog topmost failed: {error}"));
            }
            return;
        }

        self.dialog_missing_ticks = self.dialog_missing_ticks.saturating_add(1);
        let limit = if self.dialog_seen { 2 } else { 25 };
        if self.dialog_missing_ticks > limit {
            self.dialog_open = false;
            self.dialog_seen = false;
            self.dialog_missing_ticks = 0;
            if let Some(window) = &self.window {
                let _ = window.reassert_topmost();
            }
            crate::logging::warn("dialog window disappeared without a close notification");
        }
    }

    fn run_menu_command(&mut self, command: ContextCommand, context: &ComponentContext<Self>) {
        match command {
            ContextCommand::RestorePosition => {
                if let Some(window) = &self.window {
                    let _ = window.place(settings::load_pager_location());
                }
            }
            ContextCommand::SavePosition => {
                if let Some(window) = &self.window
                    && let Some((x, y)) = window.origin()
                {
                    settings::save_pager_location(x, y);
                }
            }
            ContextCommand::DefaultPosition => {
                if let Some(window) = &self.window {
                    let _ = window.place(None);
                }
            }
            ContextCommand::ToggleViewMode => {
                self.toggle_mode(self.menu_point.0, self.menu_point.1)
            }
            ContextCommand::Settings => self.open_settings(context),
            ContextCommand::About => self.open_about(context),
            ContextCommand::Exit => {
                let _ = context.window().request_close();
            }
        }
    }
}

impl Drop for Pager {
    fn drop(&mut self) {
        native_window::set_drag_cursor(false);
    }
}

fn initial_view_modes(
    settings: &Settings,
    desktop_count: u32,
    screen_count: usize,
) -> Vec<Vec<ViewMode>> {
    let desktop_count = desktop_count.max(1) as usize;
    let mut modes = vec![vec![settings.view_mode_default; screen_count]; desktop_count];

    for (desktop, row) in modes.iter_mut().enumerate() {
        for (screen, mode) in row.iter_mut().enumerate() {
            if let Some(value) = settings
                .renderer_view_modes
                .get(desktop * screen_count + screen)
            {
                *mode = *value;
            }
        }
    }

    modes
}

fn flatten_view_modes(modes: &[Vec<ViewMode>]) -> Vec<ViewMode> {
    modes.iter().flat_map(|row| row.iter().copied()).collect()
}

fn positive_size(layout: &PagerLayout) -> (f64, f64) {
    (layout.width.max(1.0), layout.height.max(1.0))
}

fn icon_pixels(dpi: u32) -> i32 {
    (layout::ICON_SIZE * f64::from(dpi) / 96.0).round().max(1.0) as i32
}

fn empty_snapshot() -> WindowSnapshot {
    WindowSnapshot {
        windows: Vec::new(),
        desktop_ids: Vec::new(),
        desktop_count: 1,
        current_desktop: 0,
        foreground: None,
        hash: 0,
    }
}
