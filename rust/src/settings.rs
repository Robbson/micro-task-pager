use windows_registry::{CURRENT_USER, Key};

const REGISTRY_PATH: &str = r"SOFTWARE\MicroTaskPager";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Preview,
    MicroTaskbar,
}

impl ViewMode {
    fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Preview),
            1 => Some(Self::MicroTaskbar),
            _ => None,
        }
    }
}

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

fn decode_view_modes(bytes: &[u8], fallback: ViewMode) -> Vec<ViewMode> {
    bytes
        .iter()
        .map(|byte| ViewMode::from_u32(u32::from(*byte)).unwrap_or(fallback))
        .collect()
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
