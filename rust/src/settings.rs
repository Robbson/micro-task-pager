//! Settings that are compatible with the existing C# application.
//!
//! The C# pager stores its configuration in
//! `HKEY_CURRENT_USER\SOFTWARE\MicroTaskPager`. During the migration both
//! implementations share that key, so the Rust reader accepts the types the C#
//! code writes (mostly `REG_DWORD`, but `BackgroundOpacity` is a `REG_SZ`) and
//! falls back to the defaults when a value is missing or malformed.
//!
//! The `Vec<ViewMode>` list is still flat because the registry stores one byte
//! per renderer. `App::normalize_view_modes` maps it to the
//! `desktop * screen_count + screen` order once monitor enumeration is
//! available.

use windows_registry::{CURRENT_USER, Key, Type};

const REGISTRY_PATH: &str = r"SOFTWARE\MicroTaskPager";

/// The two ways a desktop/monitor can be rendered.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Scaled window rectangles with application icons.
    Preview,
    /// Grid of application icons.
    MicroTaskbar,
}

impl ViewMode {
    /// Byte used in the `REG_BINARY` view-mode array.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Preview),
            1 => Some(Self::MicroTaskbar),
            _ => None,
        }
    }
}

/// How the border around a desktop is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopBorderStyle {
    Box,
    Underline,
}

impl DesktopBorderStyle {
    fn from_u32(value: u32) -> Self {
        match value {
            1 => Self::Underline,
            _ => Self::Box,
        }
    }
}

/// Values that control layout, colors, refresh timing, and window behavior.
///
/// Colors are stored as 32-bit ARGB values, matching the integers the C#
/// application writes. Keeping the same representation avoids conversion
/// mistakes while both implementations read the same registry key.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub view_mode_default: ViewMode,
    pub renderer_view_modes: Vec<ViewMode>,
    pub pager_height: i32,
    pub icon_padding_x: i32,
    pub icon_padding_y: i32,
    pub background_opacity: f64,
    pub desktop_border_style: DesktopBorderStyle,
    pub desktop_background_color: u32,
    pub desktop_border_color: u32,
    pub active_desktop_border_color: u32,
    pub window_background_color: u32,
    pub active_window_background_color: u32,
    pub window_border_color: u32,
    pub active_window_border_color: u32,
    pub primary_update_delay: u32,
    pub secondary_update_delay: u32,
    pub show_app_in_taskbar: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            view_mode_default: ViewMode::MicroTaskbar,
            renderer_view_modes: Vec::new(),
            pager_height: 40,
            icon_padding_x: 0,
            icon_padding_y: 0,
            background_opacity: 1.0,
            desktop_border_style: DesktopBorderStyle::Box,
            desktop_background_color: 0xFF40_4040,
            desktop_border_color: 0xFF20_2020,
            active_desktop_border_color: 0xFFAD_D8E6,
            window_background_color: 0xFF80_8080,
            active_window_background_color: 0xFFC0_C0C0,
            window_border_color: 0xFFC0_C0C0,
            active_window_border_color: 0xFFFF_FFFF,
            primary_update_delay: 200,
            secondary_update_delay: 500,
            show_app_in_taskbar: true,
        }
    }
}

impl Settings {
    /// Loads the registry values, falling back to `Default` for anything that
    /// is missing or cannot be parsed.
    ///
    /// This deliberately does not return an error: a missing key simply means
    /// "use defaults", which is also what the C# application does. A partially
    /// created key is treated the same way, one value at a time.
    pub fn load() -> Self {
        let mut settings = Self::default();
        let Ok(key) = CURRENT_USER.open(REGISTRY_PATH) else {
            return settings;
        };

        let default_mode = settings.view_mode_default;
        settings.view_mode_default =
            ViewMode::from_u32(read_u32(&key, "ViewModeDefault", 1)).unwrap_or(default_mode);
        settings.renderer_view_modes = key
            .get_bytes("RendererViewModes")
            .map(|bytes| decode_view_modes(&bytes, default_mode))
            .unwrap_or_default();
        settings.pager_height = read_u32(&key, "PagerHeight", 40) as i32;
        settings.icon_padding_x = read_u32(&key, "IconPaddingX", 0) as i32;
        settings.icon_padding_y = read_u32(&key, "IconPaddingY", 0) as i32;
        settings.background_opacity = read_opacity(&key, "BackgroundOpacity", 1.0);
        settings.desktop_border_style =
            DesktopBorderStyle::from_u32(read_u32(&key, "DesktopBorderStyle", 0));
        settings.desktop_background_color = read_u32(
            &key,
            "DesktopBackgroundColor",
            settings.desktop_background_color,
        );
        settings.desktop_border_color =
            read_u32(&key, "DesktopBorderColor", settings.desktop_border_color);
        settings.active_desktop_border_color = read_u32(
            &key,
            "ActiveDesktopBorderColor",
            settings.active_desktop_border_color,
        );
        settings.window_background_color = read_u32(
            &key,
            "WindowBackgroundColor",
            settings.window_background_color,
        );
        settings.active_window_background_color = read_u32(
            &key,
            "ActiveWindowBackgroundColor",
            settings.active_window_background_color,
        );
        settings.window_border_color =
            read_u32(&key, "WindowBorderColor", settings.window_border_color);
        settings.active_window_border_color = read_u32(
            &key,
            "ActiveWindowBorderColor",
            settings.active_window_border_color,
        );
        settings.primary_update_delay =
            read_u32(&key, "PrimaryUpdateDelay", settings.primary_update_delay);
        settings.secondary_update_delay = read_u32(
            &key,
            "SecondaryUpdateDelay",
            settings.secondary_update_delay,
        );
        settings.show_app_in_taskbar = read_u32(
            &key,
            "ShowAppInTaskbar",
            u32::from(settings.show_app_in_taskbar),
        ) != 0;

        settings
    }
}

/// Reads a number, accepting a string as well because older or hand-edited
/// registry values are not guaranteed to have the type the C# code writes.
fn read_u32(key: &Key, name: &str, fallback: u32) -> u32 {
    if let Ok(value) = key.get_u32(name) {
        return value;
    }

    if let Ok(value) = key.get_string(name)
        && let Ok(parsed) = value.trim().parse::<u32>()
    {
        return parsed;
    }

    fallback
}

/// Reads the opacity, which the C# application stores as a decimal string.
/// A DWORD is accepted as a fallback.
fn read_opacity(key: &Key, name: &str, fallback: f64) -> f64 {
    if let Ok(value) = key.get_string(name)
        && let Some(parsed) = parse_opacity(&value)
    {
        return parsed;
    }

    if let Ok(value) = key.get_u32(name) {
        return f64::from(value);
    }

    fallback
}

fn parse_opacity(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

/// Converts the registry byte array into view modes. Unknown bytes use the
/// configured default rather than failing the whole import.
fn decode_view_modes(bytes: &[u8], fallback: ViewMode) -> Vec<ViewMode> {
    bytes
        .iter()
        .map(|byte| ViewMode::from_u32(u32::from(*byte)).unwrap_or(fallback))
        .collect()
}

/// Reads the saved pager position, if both coordinates are present.
pub fn load_pager_location() -> Option<(i32, i32)> {
    let key = CURRENT_USER.open(REGISTRY_PATH).ok()?;
    let x = key.get_u32("PagerLocationX").ok()? as i32;
    let y = key.get_u32("PagerLocationY").ok()? as i32;
    Some((x, y))
}

/// Saves the pager position in the same values the C# application uses.
pub fn save_pager_location(x: i32, y: i32) {
    let Ok(key) = CURRENT_USER.create(REGISTRY_PATH) else {
        return;
    };
    let _ = key.set_u32("PagerLocationX", x as u32);
    let _ = key.set_u32("PagerLocationY", y as u32);
}

/// Saves the per-desktop/screen view modes as the flat `REG_BINARY` array the
/// C# application expects.
pub fn save_view_modes(modes: &[ViewMode]) {
    let Ok(key) = CURRENT_USER.create(REGISTRY_PATH) else {
        return;
    };
    let bytes: Vec<u8> = modes.iter().map(|mode| mode.as_u8()).collect();
    let _ = key.set_bytes("RendererViewModes", Type::Bytes, &bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_reference_application() {
        let settings = Settings::default();

        assert_eq!(settings.view_mode_default, ViewMode::MicroTaskbar);
        assert!(settings.renderer_view_modes.is_empty());
        assert_eq!(settings.pager_height, 40);
        assert_eq!(settings.background_opacity, 1.0);
        assert_eq!(settings.desktop_border_style, DesktopBorderStyle::Box);
        assert_eq!(settings.desktop_background_color, 0xFF40_4040);
        assert_eq!(settings.primary_update_delay, 200);
        assert_eq!(settings.secondary_update_delay, 500);
        assert!(settings.show_app_in_taskbar);
    }

    #[test]
    fn view_modes_map_registry_values() {
        assert_eq!(ViewMode::from_u32(0), Some(ViewMode::Preview));
        assert_eq!(ViewMode::from_u32(1), Some(ViewMode::MicroTaskbar));
        assert_eq!(ViewMode::from_u32(2), None);
    }

    #[test]
    fn invalid_view_mode_bytes_fall_back_to_default() {
        assert_eq!(
            decode_view_modes(&[0, 1, 9], ViewMode::MicroTaskbar),
            vec![
                ViewMode::Preview,
                ViewMode::MicroTaskbar,
                ViewMode::MicroTaskbar
            ]
        );
    }

    #[test]
    fn opacity_parses_invariant_strings() {
        assert_eq!(parse_opacity("0.75"), Some(0.75));
        assert_eq!(parse_opacity(" 1 "), Some(1.0));
        assert_eq!(parse_opacity("not-a-number"), None);
    }
}
