//! Reactor pager component.
//!
//! Phase 8 is the shell: a borderless, topmost, layered reactor window that is
//! pinned to all virtual desktops and refreshes the window snapshot on the UI
//! thread. The visible content in Phase 9 replaces the placeholder background,
//! but the window lifecycle, tick scheduling, and native-window handoff stay
//! here.
//!
//! Reactor does not expose a timer API, so both polling loops are
//! self-rescheduling background tasks. The background closure only sleeps and
//! returns a message; COM and window enumeration run in `update`, which reactor
//! always calls on the UI thread.

mod layout;

use std::error::Error;
use std::time::Duration;

use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::w;
use windows_reactor::{
    App, Button, ChildrenControl, Color, Component, ComponentContext, ContentControl, Grid,
    LayoutControl, StackPanel, TextBlock, Thickness, View, ViewContext, WindowTheme, WindowVisuals,
};

use crate::desktop::{self, SupervisedDesktop, VirtualDesktopApi};
use crate::monitors;
use crate::native_window::{self, PagerWindow};
use crate::settings::{self, Settings};
use crate::windows::{WindowSnapshot, WindowSnapshotter};

/// Window title used both by reactor and by the native HWND lookup.
const WINDOW_TITLE: &str = "Micro Task Pager";
const MIN_TICK_DELAY_MS: u32 = 16;
const ATTACH_RETRY_DELAY_MS: u32 = 50;
const ERROR_WINDOW_SIZE: (f64, f64) = (420.0, 160.0);

/// Messages handled on the reactor UI thread.
#[derive(Clone, Copy, Debug)]
pub enum Message {
    /// Snapshot refresh.
    PrimaryTick,
    /// Topmost, pinning, monitor, and desktop-count maintenance.
    SecondaryTick,
    /// Closes the window, used by the startup-error view.
    Close,
}

/// Reactor pager component.
pub struct Pager {
    settings: Settings,
    desktop: Option<SupervisedDesktop>,
    snapshotter: WindowSnapshotter,
    snapshot: WindowSnapshot,
    monitors: Vec<monitors::MonitorInfo>,
    window: Option<PagerWindow>,
    pinned: bool,
    client_size: (f64, f64),
    startup_error: Option<String>,
}

/// Starts the reactor pager window.
///
/// The caller must initialize the COM apartment on this thread first. Reactor
/// keeps the WinUI message loop on the same thread, so every virtual-desktop
/// call made by the component stays in the apartment.
pub fn run() -> Result<(), Box<dyn Error>> {
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
        match Self::start(settings.clone()) {
            Ok(pager) => {
                Self::schedule_primary(context, MIN_TICK_DELAY_MS);
                Self::schedule_secondary(context, settings.secondary_update_delay);
                pager
            }
            Err(error) => {
                logging_error(&error);
                Self::failed(settings, error)
            }
        }
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message {
            Message::PrimaryTick => {
                self.refresh_snapshot();
                self.attach_window();
                self.sync_window_size();
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
                self.try_pin();
                let delay = if self.window.is_some() {
                    self.settings.secondary_update_delay
                } else {
                    ATTACH_RETRY_DELAY_MS
                };
                Self::schedule_secondary(context, delay);
            }
            Message::Close => {
                let _ = context.window().request_close();
            }
        }
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        context.window_title(WINDOW_TITLE);
        context.window_visuals(
            WindowVisuals::new()
                .client_size(self.client_size.0, self.client_size.1)
                .theme(WindowTheme::Dark),
        );

        let background = color(self.settings.desktop_background_color);
        if let Some(error) = &self.startup_error {
            return Grid::new()
                .background(background)
                .children((StackPanel::new()
                    .spacing(10.0)
                    .margin(Thickness::uniform(16.0))
                    .children((
                        TextBlock::new()
                            .text("Micro Task Pager could not start.")
                            .font_size(16.0)
                            .foreground(Color::rgb(255, 255, 255)),
                        TextBlock::new()
                            .text(error.clone())
                            .foreground(Color::rgb(220, 220, 220)),
                        Button::new()
                            .on_click(context.message(Message::Close))
                            .content("Close"),
                    )),));
        }

        Grid::new().background(background).children(())
    }
}

impl Pager {
    fn start(settings: Settings) -> Result<Self, String> {
        let monitors = monitors::enumerate();
        let desktop = desktop::open().map_err(|error| error.to_string())?;
        let mut snapshotter = WindowSnapshotter::new();
        let snapshot = snapshotter
            .capture(&desktop)
            .map_err(|error| error.to_string())?;
        let client_size = layout::pager_size(&snapshot, &monitors, &settings);

        crate::logging::info(format!(
            "reactor pager started: {}x{} dip, {} desktops, {} monitors",
            client_size.0,
            client_size.1,
            snapshot.desktop_count,
            monitors.len()
        ));

        Ok(Self {
            settings,
            desktop: Some(desktop),
            snapshotter,
            snapshot,
            monitors,
            window: None,
            pinned: false,
            client_size,
            startup_error: None,
        })
    }

    fn failed(settings: Settings, error: String) -> Self {
        Self {
            settings,
            desktop: None,
            snapshotter: WindowSnapshotter::new(),
            snapshot: empty_snapshot(),
            monitors: monitors::enumerate(),
            window: None,
            pinned: false,
            client_size: ERROR_WINDOW_SIZE,
            startup_error: Some(error),
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

    fn refresh_snapshot(&mut self) {
        let Some(desktop) = self.desktop.as_ref() else {
            return;
        };

        match self.snapshotter.capture(desktop) {
            Ok(snapshot) => {
                let desktop_count_changed = snapshot.desktop_count != self.snapshot.desktop_count;
                self.snapshot = snapshot;
                if desktop_count_changed {
                    self.update_client_size();
                }
            }
            Err(error) => crate::logging::error(format!("snapshot failed: {error}")),
        }
    }

    fn refresh_environment(&mut self) {
        let monitors = monitors::enumerate();
        if monitors != self.monitors {
            self.monitors = monitors;
            self.update_client_size();
        }

        let Some(desktop) = self.desktop.as_ref() else {
            return;
        };
        match desktop.count() {
            Ok(count) if count != self.snapshot.desktop_count => self.refresh_snapshot(),
            Ok(_) => {}
            Err(error) => crate::logging::warn(format!("desktop count failed: {error}")),
        }
    }

    fn update_client_size(&mut self) {
        self.client_size = layout::pager_size(&self.snapshot, &self.monitors, &self.settings);
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

        crate::logging::info(format!("reactor pager window attached: {hwnd:?}"));
        self.window = Some(window);
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

fn color(argb: u32) -> Color {
    Color::argb(
        (argb >> 24) as u8,
        (argb >> 16) as u8,
        (argb >> 8) as u8,
        argb as u8,
    )
}

fn logging_error(error: &str) {
    crate::logging::error(format!("reactor pager startup failed: {error}"));
}
