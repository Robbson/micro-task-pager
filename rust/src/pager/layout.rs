//! Pure pager geometry in device-independent pixels (DIPs).
//!
//! Phase 8 only needs the shell size: one desktop is the sum of the monitor
//! aspect widths at the configured pager height, and the full width is that
//! sum multiplied by the desktop count. The rest of the content layout is
//! added in Phase 9. Keeping this pure makes it testable without WinUI.

use crate::monitors::MonitorInfo;
use crate::settings::Settings;
use crate::windows::WindowSnapshot;

/// Client size of the pager in DIPs.
pub fn pager_size(
    snapshot: &WindowSnapshot,
    monitors: &[MonitorInfo],
    settings: &Settings,
) -> (f64, f64) {
    let height = f64::from(settings.pager_height.max(1));
    let desktop_width: f64 = monitors
        .iter()
        .map(|monitor| {
            let monitor_height = monitor.height().max(1) as f64;
            height * monitor.width().max(1) as f64 / monitor_height
        })
        .sum();

    let width = desktop_width.max(1.0) * f64::from(snapshot.desktop_count.max(1));
    (width.max(1.0), height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::RECT;

    use crate::windows::{WindowInfo, WindowSnapshot};

    fn monitor() -> MonitorInfo {
        MonitorInfo {
            rect: RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            work_rect: RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            },
            primary: true,
        }
    }

    fn snapshot(desktop_count: u32) -> WindowSnapshot {
        WindowSnapshot {
            windows: Vec::<WindowInfo>::new(),
            desktop_ids: Vec::new(),
            desktop_count,
            current_desktop: 0,
            foreground: None,
            hash: 0,
        }
    }

    #[test]
    fn size_keeps_the_monitor_aspect_ratio() {
        let settings = Settings {
            pager_height: 40,
            ..Settings::default()
        };

        let (width, height) = pager_size(&snapshot(1), &[monitor()], &settings);

        assert!((height - 40.0).abs() < f64::EPSILON);
        assert!((width - 1920.0 / 1080.0 * 40.0).abs() < 0.001);
    }

    #[test]
    fn size_multiplies_each_monitor_per_desktop() {
        let settings = Settings {
            pager_height: 40,
            ..Settings::default()
        };

        let (width, _) = pager_size(&snapshot(2), &[monitor()], &settings);
        let desktop_width = 1920.0 / 1080.0 * 40.0;

        assert!((width - desktop_width * 2.0).abs() < 0.001);
    }

    #[test]
    fn size_is_positive_for_invalid_settings_and_empty_snapshots() {
        let settings = Settings {
            pager_height: 0,
            ..Settings::default()
        };

        let (width, height) = pager_size(&snapshot(0), &[], &settings);

        assert!(width >= 1.0);
        assert!(height >= 1.0);
    }
}
