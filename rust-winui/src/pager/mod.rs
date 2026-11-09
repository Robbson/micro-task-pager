//! Reactor pager component.
//!
//! # If you know the C# pager
//!
//! This component plays the role of `MainForm` plus `VirtualDesktop.Desktop` and
//! `ScreenRenderer`. The main differences are:
//!
//! | C# | Rust here |
//! | --- | --- |
//! | `Form` fields mutated from event handlers | `Pager` fields mutated from `update` |
//! | `OnPaint` draws with GDI+ | `view::build` returns a declarative `View` |
//! | `Invalidate()` asks for a repaint | Reactor rebuilds the view after every `update` |
//! | `Timer` ticks | `PrimaryTick`/`SecondaryTick` messages that reschedule themselves |
//! | `AppSettingsWindow` form | A reactor secondary window driven by `DialogState` |
//!
//! Reactor calls [`Component::create`] once, then [`Component::update`] for each
//! message and [`Component::view`] to render. `view` must not mutate state: the
//! displayed pager is always a pure function of the fields, so a missed update
//! cannot leave stale pixels behind.
//!
//! # State model
//!
//! The fields are grouped by concern; each group has one owner function that
//! writes it:
//!
//! | State | What it describes | Written by |
//! | --- | --- | --- |
//! | `settings`, `snapshot`, `icons` | What the pager should display | `refresh_snapshot`, `apply_settings` |
//! | `desktop`, `monitors` | Virtual desktops and physical screens | `refresh_environment` |
//! | `view_modes`, `layout`, `client_size`, `metrics_dpi` | How it is laid out | `rebuild_layout`, `sync_metrics` |
//! | `window`, `wheel_hook`, `pinned`, `foreground` | Native window and shell integration | `attach_window`, `maintain_window` |
//! | `dialog` | The single open About/Settings window | `dialog_state` |
//! | `input`, `menu_point` | Pointer, drag, and click state | `input` |
//!
//! # How one refresh works
//!
//! 1. A tick message reaches `update` on the UI thread.
//! 2. `refresh_snapshot` asks the snapshotter for windows on the current
//!    desktop and detects a changed window set with a cheap hash.
//! 3. When the set changed, `refresh_icons` encodes only new or changed icons
//!    and `rebuild_layout` recomputes geometry for every `(desktop, screen)`.
//! 4. Reactor calls `view`, which walks the cached layout and emits WinUI
//!    elements. No COM call or window enumeration happens here.
//! 5. The tick handler schedules the next tick.
//!
//! Two ticks exist because they have different costs and settings, mirroring
//! the C# update loop: the primary tick refreshes the window snapshot and the
//! layout; the secondary tick handles monitors, topmost, pinning, and the open
//! dialog. Reactor has no timer API, so each tick is a background task that
//! sleeps and returns its message; the work always happens in `update`.

mod dialog_state;
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
use crate::dialogs::placement;
use crate::dialogs::{DialogEvent, DialogLink, about, settings as settings_dialog};
use crate::foreground::ForegroundTracker;
use crate::monitors::{self, MonitorInfo};
use crate::native_window::{self, ContextCommand, PagerWindow, WheelHook};
use crate::settings::{self, Settings, ViewMode};
use crate::windows::{WindowSnapshot, WindowSnapshotter};

use dialog_state::DialogState;
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
///
/// - `PrimaryTick` and `SecondaryTick` are self-rescheduling timers.
/// - `Pointer*` and `Wheel` come from reactor input.
/// - `Dialog` comes from an open About/Settings window.
/// - `RestoreLastActive`, `PositionDialog`, and `Close` are posted by short
///   background delays.
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
    /// A short-delay retry that positions a newly created dialog.
    PositionDialog,
    /// Closes the window, used by the startup-error view.
    Close,
}

/// A pointer press with the buttons and modifier that produced it.
struct PointerPress {
    x: f64,
    y: f64,
    left: bool,
    middle: bool,
    right: bool,
    shift: bool,
}

/// Reactor pager component.
///
/// The fields are grouped the same way as the state model above. Only
/// `update` and its `on_*` handlers mutate them.
pub struct Pager {
    // What to display: settings, the window snapshot, and its icon sources.
    settings: Settings,
    snapshotter: WindowSnapshotter,
    snapshot: WindowSnapshot,
    icons: IconSources,

    // Virtual desktops and physical screens.
    desktop: Option<SupervisedDesktop>,
    monitors: Vec<MonitorInfo>,

    // How the pager is laid out in the current DPI space.
    view_modes: Vec<Vec<ViewMode>>,
    layout: PagerLayout,
    client_size: (f64, f64),
    /// DPI the current layout and icon cache were built for. Under per-monitor
    /// awareness a move to another monitor changes it, so the grid and icons
    /// must be rebuilt.
    metrics_dpi: u32,

    // The native pager window and its shell integration.
    window: Option<PagerWindow>,
    wheel_hook: Option<WheelHook>,
    pinned: bool,
    foreground: Option<ForegroundTracker>,
    startup_foreground: Option<HWND>,

    // Reactor plumbing and the startup-error view.
    sender: LocalSender<Message>,
    startup_error: Option<String>,

    // Interaction state.
    input: InputState,
    menu_point: (f64, f64),

    /// The single open About/Settings window, if any.
    dialog: DialogState,
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

    /// Called once on the UI thread before the window exists.
    ///
    /// Opens the virtual-desktop backend and builds the first snapshot and
    /// layout. A startup failure still returns a `Pager`, but in an error state
    /// so the user sees a message instead of a missing window.
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

    /// The only function that mutates pager state.
    ///
    /// It stays a flat dispatcher so the message type is the complete list of
    /// things that can happen. Each arm delegates to an `on_*` handler; see the
    /// state model at the top of this module for which handler owns which
    /// fields.
    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message {
            Message::PrimaryTick => self.on_primary_tick(context),
            Message::SecondaryTick => self.on_secondary_tick(context),
            Message::PointerPressed {
                x,
                y,
                left,
                middle,
                right,
                shift,
            } => self.on_pointer_pressed(
                PointerPress {
                    x,
                    y,
                    left,
                    middle,
                    right,
                    shift,
                },
                context,
            ),
            Message::PointerMoved { x, y } => self.on_pointer_moved(x, y),
            Message::PointerReleased { x, y } => self.on_pointer_released(x, y, context),
            Message::PointerCaptureLost => self.on_pointer_capture_lost(),
            Message::Wheel { x, y, shift } => {
                if shift {
                    self.toggle_mode(x, y);
                }
            }
            Message::RestoreLastActive(desktop) => self.restore_last_active(desktop),
            Message::Dialog(event) => self.handle_dialog_event(event),
            Message::PositionDialog => self.on_position_dialog(context),
            Message::Close => {
                let _ = context.window().request_close();
            }
        }
    }

    /// Renders the current state. Must stay pure: no COM, no enumeration, no
    /// mutation, so it is always safe for reactor to call after `update`.
    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        view::build(self, context)
    }
}

impl Pager {
    // ---- Construction and scheduling -------------------------------------

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
        let startup_dpi = native_window::system_dpi();
        icons.refresh(&snapshot, icon_pixels(startup_dpi));
        let layout = layout::build(&snapshot, &monitors, &view_modes, &settings, startup_dpi);
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
            metrics_dpi: startup_dpi,
            startup_error: None,
            sender,
            input: InputState::default(),
            menu_point: (0.0, 0.0),
            foreground: None,
            startup_foreground,
            dialog: DialogState::Closed,
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
            metrics_dpi: native_window::system_dpi(),
            startup_error: Some(error),
            sender,
            input: InputState::default(),
            menu_point: (0.0, 0.0),
            foreground: None,
            startup_foreground: None,
            dialog: DialogState::Closed,
        }
    }

    // ---- Timing helpers --------------------------------------------------
    //
    // Reactor has no timer or delay API. Each helper spawns a background task
    // that sleeps and then returns one message; the message is delivered on
    // the UI thread, where `update` does the real work.

    fn schedule_primary(context: &ComponentContext<Self>, delay: u32) {
        let delay = Duration::from_millis(u64::from(delay.max(MIN_TICK_DELAY_MS)));
        let _ = context.spawn_background(move |_| {
            std::thread::sleep(delay);
            Message::PrimaryTick
        });
    }

    /// Gives a just-created dialog a chance to be positioned before its first
    /// visible frame, without waiting for the next primary tick.
    fn schedule_dialog_position(context: &ComponentContext<Self>) {
        let _ = context.spawn_background(|_| {
            std::thread::sleep(Duration::from_millis(1));
            Message::PositionDialog
        });
    }

    fn schedule_secondary(context: &ComponentContext<Self>, delay: u32) {
        let delay = Duration::from_millis(u64::from(delay.max(MIN_TICK_DELAY_MS)));
        let _ = context.spawn_background(move |_| {
            std::thread::sleep(delay);
            Message::SecondaryTick
        });
    }

    /// Restores the last active window after the shell has finished a desktop
    /// switch. The delay matches the C# `MainForm` wait.
    fn schedule_restore(context: &ComponentContext<Self>, desktop: u32) {
        let delay = Duration::from_millis(RESTORE_DELAY_MS);
        let _ = context.spawn_background(move |_| {
            std::thread::sleep(delay);
            Message::RestoreLastActive(desktop)
        });
    }

    // ---- Tick handlers ---------------------------------------------------
    //
    // The split mirrors the two timers in the C# pager. `primary_update_delay`
    // and `secondary_update_delay` come from the settings, so the user can keep
    // the expensive snapshot refresh slow while window maintenance stays fast.

    fn on_primary_tick(&mut self, context: &ComponentContext<Self>) {
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

    fn on_secondary_tick(&mut self, context: &ComponentContext<Self>) {
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

    // ---- Input handlers --------------------------------------------------
    //
    // These translate reactor pointer messages into state transitions in
    // `input` and then into native side effects such as moving the window or
    // switching desktops. All geometry comes from the cached `layout`, so the
    // hit target is exactly what `view` drew.

    fn on_pointer_pressed(&mut self, press: PointerPress, context: &ComponentContext<Self>) {
        if press.right {
            self.input.right_press();
            native_window::set_drag_cursor(false);
            self.menu_point = (press.x, press.y);
            if let Some(hwnd) = self.window.as_ref().map(PagerWindow::hwnd)
                && let Some(command) = native_window::show_context_menu(hwnd, self.dialog.is_open())
            {
                self.run_menu_command(command, context);
            }
        } else if press.left || press.middle {
            let hit = self.layout.hit_test(press.x, press.y);
            let cursor = native_window::cursor_position();
            let origin = self.window.as_ref().and_then(PagerWindow::origin);
            self.input.press(input::PressInput {
                hit,
                x: press.x,
                y: press.y,
                middle: press.middle,
                shift: press.shift,
                cursor,
                origin,
            });
            if matches!(self.input.drag(), Some(input::Drag::Pager { .. })) {
                native_window::set_drag_cursor(true);
            }
        }
    }

    fn on_pointer_moved(&mut self, x: f64, y: f64) {
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
            // WinUI may reset its cursor during the pointer move, so reapply
            // the move cursor after each drag update.
            native_window::set_drag_cursor(true);
        } else if self.input.pointer_moved(x, y) {
            native_window::set_drag_cursor(true);
        }
    }

    fn on_pointer_released(&mut self, x: f64, y: f64, context: &ComponentContext<Self>) {
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

    fn on_pointer_capture_lost(&mut self) {
        // Reactor can lose capture while a view rebuild follows a pointer
        // move. Keep an active gesture alive; the release still arrives with
        // the final coordinates. Only clear when no drag is in progress.
        if self.input.drag().is_none() {
            self.input.cancel();
            native_window::set_drag_cursor(false);
        }
    }

    fn on_position_dialog(&mut self, context: &ComponentContext<Self>) {
        let found = self.maintain_dialog();
        if !found && self.dialog.request_position_retry() {
            Self::schedule_dialog_position(context);
        }
    }

    // ---- Snapshot, environment, and layout --------------------------------
    //
    // Everything the view needs is prepared here, on the UI thread. The view
    // itself only reads these fields.

    /// Re-enumerates windows on the current desktop.
    ///
    /// `snapshot.hash` covers everything that affects rendering (window set,
    /// titles, icons, active window, current desktop). It is a cheap way to
    /// decide whether icons and geometry must be rebuilt, so an idle desktop
    /// does no work beyond the enumeration itself.
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
        self.sync_metrics();

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
        self.metrics_dpi = dpi;
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

    // ---- Native window ---------------------------------------------------
    //
    // Reactor creates the HWND after the first publication, and WinUI has no
    // API for borderless/tool-window/layered/topmost styles. These functions
    // bridge that gap through `native_window`; no view code touches an HWND.

    /// Finds the reactor HWND, applies pager styles, and installs hooks.
    ///
    /// Runs on every tick until it succeeds, because the reactor window does
    /// not exist yet when the component is created.
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
        // The window DPI can differ from the system DPI used before the window
        // existed, so rebuild the grid and icon cache for the monitor it opened
        // on.
        self.sync_metrics();
    }

    fn refresh_icons(&mut self) {
        let dpi = self
            .window
            .as_ref()
            .map(|window| native_window::window_dpi(window.hwnd()))
            .unwrap_or_else(native_window::system_dpi);
        self.icons.refresh(&self.snapshot, icon_pixels(dpi));
    }

    /// Rebuilds the pixel grid and icon cache when the window's DPI changes.
    ///
    /// The layout snaps physical pixels to the DIP grid, so moving the pager to
    /// a monitor with a different scale factor needs a new grid; otherwise the
    /// window keeps DIP sizes computed at the old scale and appears too large or
    /// too small.
    fn sync_metrics(&mut self) {
        let dpi = self
            .window
            .as_ref()
            .map(|window| native_window::window_dpi(window.hwnd()))
            .unwrap_or_else(native_window::system_dpi);
        if self.metrics_dpi != dpi {
            self.rebuild_layout();
            self.refresh_icons();
        } else if self.icons.size_px() != icon_pixels(dpi) {
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
        if self.dialog.is_open() {
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

    // ---- Pager actions ---------------------------------------------------
    //
    // Side effects that follow a completed pointer gesture: switching
    // desktops, focusing a window, moving a window, and toggling a view mode.
    // These come from `ReleaseAction`, never from `view`.

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

    // ---- Dialogs ---------------------------------------------------------
    //
    // About and Settings are separate reactor windows (secondary components).
    // The pager never owns their widgets; it only opens them, places the
    // Settings window, and receives `DialogEvent`s through a `DialogLink`.

    fn dialog_link(&self) -> DialogLink {
        let sender = self.sender.clone();
        DialogLink::new(move |event| {
            let _ = sender.send(Message::Dialog(event));
        })
    }

    fn open_settings(&mut self, context: &ComponentContext<Self>) {
        if self.dialog.is_open() {
            return;
        }

        let input =
            settings_dialog::SettingsInput::linked(self.settings.clone(), self.dialog_link());
        let owner = self
            .window
            .as_ref()
            .and_then(PagerWindow::window_rect)
            .map(placement::Rect::from);
        if context.open_window(View::component::<settings_dialog::SettingsDialog>(input)) {
            self.dialog = DialogState::opened(owner);
            Self::schedule_dialog_position(context);
            crate::logging::info("opened the Settings window");
        } else {
            crate::logging::warn("the Settings window request was rejected");
        }
    }

    fn open_about(&mut self, context: &ComponentContext<Self>) {
        if self.dialog.is_open() {
            return;
        }

        let input = about::AboutInput::linked(self.dialog_link());
        if context.open_window(View::component::<about::About>(input)) {
            self.dialog = DialogState::opened(None);
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
                self.dialog.closed();
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

    /// Keeps the open dialog above the topmost pager and clears a stale dialog
    /// state if its window disappeared without a close message.
    ///
    /// Returns whether the dialog HWND was found. The first sighting cloaks
    /// the window, places it, and reveals it, so its default position is never
    /// presented.
    fn maintain_dialog(&mut self) -> bool {
        if !self.dialog.is_open() {
            return false;
        }

        let hwnd = native_window::find_pager_window(settings_dialog::WINDOW_TITLE)
            .or_else(|| native_window::find_pager_window(about::WINDOW_TITLE));

        if let Some(hwnd) = hwnd {
            // The placement owner is only available before the first sighting,
            // so capture it before advancing the state.
            let owner = self.dialog.owner();
            if self.dialog.window_seen() {
                let cloak = owner.is_some();
                if cloak && !native_window::set_cloaked(hwnd, true) {
                    crate::logging::warn("cloaking the Settings window failed");
                }
                self.position_dialog(hwnd, owner);
                if cloak && !native_window::set_cloaked(hwnd, false) {
                    crate::logging::warn("revealing the Settings window failed");
                }
            }
            if let Err(error) = native_window::reassert_topmost(hwnd) {
                crate::logging::warn(format!("reasserting dialog topmost failed: {error}"));
            }
            return true;
        }

        if self.dialog.window_missing() {
            if let Some(window) = &self.window {
                let _ = window.reassert_topmost();
            }
            crate::logging::warn("dialog window disappeared without a close notification");
        }
        false
    }

    /// Places the Settings window using the C# right/left/below/above sequence.
    fn position_dialog(&self, hwnd: HWND, owner: Option<placement::Rect>) {
        let Some(owner) = owner else {
            return;
        };
        let Some(rect) = native_window::window_rect(hwnd) else {
            return;
        };

        let size = (rect.right - rect.left, rect.bottom - rect.top);
        let work_areas: Vec<placement::Rect> = self
            .monitors
            .iter()
            .map(|monitor| monitor.work_rect.into())
            .collect();
        let (x, y) = placement::find_placement(owner, size, &work_areas);
        if let Err(error) = native_window::set_position(hwnd, x, y) {
            crate::logging::warn(format!("positioning the Settings window failed: {error}"));
        } else {
            crate::logging::info(format!("positioned the Settings window at {x},{y}"));
        }
    }

    // ---- Context menu ----------------------------------------------------
    //
    // `show_context_menu` blocks in `TrackPopupMenu` and returns the chosen
    // command; reactor 0.100 has no transient-menu API, so this is the one
    // interactive path that runs native code directly from an input handler.

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
