//! Pure working-copy model for the Settings dialog.
//!
//! The dialog never edits the live [`Settings`] value. It edits a
//! [`SettingsDraft`], which remembers the value the window opened with and the
//! current working copy. Reverting an option and resetting a section are pure
//! state transitions and are unit-tested here.

use crate::settings::{DesktopBorderStyle, Settings, ViewMode};

pub(super) const MIN_PAGER_HEIGHT: i32 = 16;
pub(super) const MAX_PAGER_HEIGHT: i32 = 9999;
pub(super) const MAX_ICON_PADDING: i32 = 9999;
pub(super) const MIN_PRIMARY_DELAY: u32 = 1;
pub(super) const MIN_SECONDARY_DELAY: u32 = 100;
pub(super) const MAX_DELAY: u32 = 60_000;
pub(super) const MIN_OPACITY_PERCENT: f64 = 25.0;
pub(super) const MAX_OPACITY_PERCENT: f64 = 100.0;

/// One configurable option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    ViewModeDefault,
    PagerHeight,
    IconPaddingX,
    IconPaddingY,
    BackgroundOpacity,
    DesktopBorderStyle,
    DesktopBackgroundColor,
    DesktopBorderColor,
    ActiveDesktopBorderColor,
    WindowBackgroundColor,
    WindowBorderColor,
    ActiveWindowBorderColor,
    ActiveWindowBackgroundColor,
    PrimaryUpdateDelay,
    SecondaryUpdateDelay,
    ShowAppInTaskbar,
}

/// A Settings dialog section.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Section {
    #[default]
    Layout,
    Theme,
    Misc,
}

impl Section {
    pub(super) fn index(self) -> usize {
        match self {
            Self::Layout => 0,
            Self::Theme => 1,
            Self::Misc => 2,
        }
    }

    pub(super) fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Theme,
            2 => Self::Misc,
            _ => Self::Layout,
        }
    }
}

/// The values the dialog opened with and the current working copy.
pub(super) struct SettingsDraft {
    original: Settings,
    working: Settings,
}

impl SettingsDraft {
    pub(super) fn new(initial: Settings) -> Self {
        Self {
            original: initial.clone(),
            working: initial,
        }
    }

    pub(super) fn working(&self) -> &Settings {
        &self.working
    }

    pub(super) fn original(&self) -> &Settings {
        &self.original
    }

    /// Restores one option to the value the dialog opened with.
    pub(super) fn revert(&mut self, field: Field) {
        match field {
            Field::ViewModeDefault => {
                self.working.view_mode_default = self.original.view_mode_default;
            }
            Field::PagerHeight => self.working.pager_height = self.original.pager_height,
            Field::IconPaddingX => self.working.icon_padding_x = self.original.icon_padding_x,
            Field::IconPaddingY => self.working.icon_padding_y = self.original.icon_padding_y,
            Field::BackgroundOpacity => {
                self.working.background_opacity = self.original.background_opacity;
            }
            Field::DesktopBorderStyle => {
                self.working.desktop_border_style = self.original.desktop_border_style;
            }
            Field::DesktopBackgroundColor => {
                self.working.desktop_background_color = self.original.desktop_background_color;
            }
            Field::DesktopBorderColor => {
                self.working.desktop_border_color = self.original.desktop_border_color;
            }
            Field::ActiveDesktopBorderColor => {
                self.working.active_desktop_border_color =
                    self.original.active_desktop_border_color;
            }
            Field::WindowBackgroundColor => {
                self.working.window_background_color = self.original.window_background_color;
            }
            Field::WindowBorderColor => {
                self.working.window_border_color = self.original.window_border_color;
            }
            Field::ActiveWindowBorderColor => {
                self.working.active_window_border_color = self.original.active_window_border_color;
            }
            Field::ActiveWindowBackgroundColor => {
                self.working.active_window_background_color =
                    self.original.active_window_background_color;
            }
            Field::PrimaryUpdateDelay => {
                self.working.primary_update_delay = self.original.primary_update_delay;
            }
            Field::SecondaryUpdateDelay => {
                self.working.secondary_update_delay = self.original.secondary_update_delay;
            }
            Field::ShowAppInTaskbar => {
                self.working.show_app_in_taskbar = self.original.show_app_in_taskbar;
            }
        }
        self.clamp_pager_height();
    }

    /// Resets one section to the built-in defaults.
    pub(super) fn reset_section(&mut self, section: Section) {
        let defaults = Settings::default();
        match section {
            Section::Layout => {
                self.working.view_mode_default = defaults.view_mode_default;
                self.working.pager_height = defaults.pager_height;
                self.working.icon_padding_x = defaults.icon_padding_x;
                self.working.icon_padding_y = defaults.icon_padding_y;
            }
            Section::Theme => {
                self.working.background_opacity = defaults.background_opacity;
                self.working.desktop_border_style = defaults.desktop_border_style;
                self.working.desktop_background_color = defaults.desktop_background_color;
                self.working.desktop_border_color = defaults.desktop_border_color;
                self.working.active_desktop_border_color = defaults.active_desktop_border_color;
                self.working.window_background_color = defaults.window_background_color;
                self.working.active_window_background_color =
                    defaults.active_window_background_color;
                self.working.window_border_color = defaults.window_border_color;
                self.working.active_window_border_color = defaults.active_window_border_color;
            }
            Section::Misc => {
                self.working.primary_update_delay = defaults.primary_update_delay;
                self.working.secondary_update_delay = defaults.secondary_update_delay;
                self.working.show_app_in_taskbar = defaults.show_app_in_taskbar;
            }
        }
        self.clamp_pager_height();
    }

    /// Whether an option differs from the value the dialog opened with.
    pub(super) fn is_modified(&self, field: Field) -> bool {
        let working = &self.working;
        let original = &self.original;
        match field {
            Field::ViewModeDefault => working.view_mode_default != original.view_mode_default,
            Field::PagerHeight => working.pager_height != original.pager_height,
            Field::IconPaddingX => working.icon_padding_x != original.icon_padding_x,
            Field::IconPaddingY => working.icon_padding_y != original.icon_padding_y,
            Field::BackgroundOpacity => working.background_opacity != original.background_opacity,
            Field::DesktopBorderStyle => {
                working.desktop_border_style != original.desktop_border_style
            }
            Field::DesktopBackgroundColor => {
                working.desktop_background_color != original.desktop_background_color
            }
            Field::DesktopBorderColor => {
                working.desktop_border_color != original.desktop_border_color
            }
            Field::ActiveDesktopBorderColor => {
                working.active_desktop_border_color != original.active_desktop_border_color
            }
            Field::WindowBackgroundColor => {
                working.window_background_color != original.window_background_color
            }
            Field::WindowBorderColor => working.window_border_color != original.window_border_color,
            Field::ActiveWindowBorderColor => {
                working.active_window_border_color != original.active_window_border_color
            }
            Field::ActiveWindowBackgroundColor => {
                working.active_window_background_color != original.active_window_background_color
            }
            Field::PrimaryUpdateDelay => {
                working.primary_update_delay != original.primary_update_delay
            }
            Field::SecondaryUpdateDelay => {
                working.secondary_update_delay != original.secondary_update_delay
            }
            Field::ShowAppInTaskbar => working.show_app_in_taskbar != original.show_app_in_taskbar,
        }
    }

    /// Applies a rounded numeric control value with the C# dialog's limits.
    pub(super) fn set_number(&mut self, field: Field, value: f64) {
        if !value.is_finite() {
            return;
        }

        let value = value.round() as i64;
        match field {
            Field::PagerHeight => {
                self.working.pager_height =
                    value.clamp(i64::from(MIN_PAGER_HEIGHT), i64::from(MAX_PAGER_HEIGHT)) as i32;
                self.clamp_pager_height();
            }
            Field::IconPaddingX => {
                self.working.icon_padding_x =
                    value.clamp(-i64::from(MAX_ICON_PADDING), i64::from(MAX_ICON_PADDING)) as i32;
            }
            Field::IconPaddingY => {
                self.working.icon_padding_y =
                    value.clamp(-i64::from(MAX_ICON_PADDING), i64::from(MAX_ICON_PADDING)) as i32;
                self.clamp_pager_height();
            }
            Field::PrimaryUpdateDelay => {
                self.working.primary_update_delay =
                    value.clamp(i64::from(MIN_PRIMARY_DELAY), i64::from(MAX_DELAY)) as u32;
            }
            Field::SecondaryUpdateDelay => {
                self.working.secondary_update_delay =
                    value.clamp(i64::from(MIN_SECONDARY_DELAY), i64::from(MAX_DELAY)) as u32;
            }
            _ => {}
        }
    }

    pub(super) fn set_opacity_percent(&mut self, percent: f64) {
        if percent.is_finite() {
            self.working.background_opacity =
                percent.clamp(MIN_OPACITY_PERCENT, MAX_OPACITY_PERCENT) / 100.0;
        }
    }

    pub(super) fn set_index(&mut self, field: Field, index: usize) {
        match field {
            Field::ViewModeDefault => {
                self.working.view_mode_default = if index == 0 {
                    ViewMode::Preview
                } else {
                    ViewMode::MicroTaskbar
                };
            }
            Field::DesktopBorderStyle => {
                self.working.desktop_border_style = if index == 0 {
                    DesktopBorderStyle::Box
                } else {
                    DesktopBorderStyle::Underline
                };
            }
            _ => {}
        }
    }

    pub(super) fn set_check(&mut self, field: Field, value: bool) {
        if field == Field::ShowAppInTaskbar {
            self.working.show_app_in_taskbar = value;
        }
    }

    pub(super) fn set_color(&mut self, field: Field, argb: u32) {
        match field {
            Field::DesktopBackgroundColor => self.working.desktop_background_color = argb,
            Field::DesktopBorderColor => self.working.desktop_border_color = argb,
            Field::ActiveDesktopBorderColor => self.working.active_desktop_border_color = argb,
            Field::WindowBackgroundColor => self.working.window_background_color = argb,
            Field::WindowBorderColor => self.working.window_border_color = argb,
            Field::ActiveWindowBorderColor => self.working.active_window_border_color = argb,
            Field::ActiveWindowBackgroundColor => {
                self.working.active_window_background_color = argb
            }
            _ => {}
        }
    }

    /// Smallest pager height that still leaves room below the icon padding.
    pub(super) fn min_pager_height(&self) -> i32 {
        self.working
            .icon_padding_y
            .saturating_add(MIN_PAGER_HEIGHT)
            .max(MIN_PAGER_HEIGHT)
    }

    fn clamp_pager_height(&mut self) {
        let min = self.min_pager_height();
        self.working.pager_height = self.working.pager_height.clamp(min, MAX_PAGER_HEIGHT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> SettingsDraft {
        SettingsDraft::new(Settings::default())
    }

    #[test]
    fn revert_restores_one_option_from_the_opening_values() {
        let mut draft = draft();
        draft.set_number(Field::PagerHeight, 120.0);
        draft.set_number(Field::IconPaddingX, 8.0);

        draft.revert(Field::PagerHeight);

        assert_eq!(
            draft.working().pager_height,
            Settings::default().pager_height
        );
        assert_eq!(draft.working().icon_padding_x, 8);
        assert!(!draft.is_modified(Field::PagerHeight));
        assert!(draft.is_modified(Field::IconPaddingX));
    }

    #[test]
    fn reset_section_only_changes_the_selected_section() {
        let mut draft = draft();
        draft.set_number(Field::PagerHeight, 120.0);
        draft.set_number(Field::PrimaryUpdateDelay, 500.0);
        draft.set_color(Field::DesktopBackgroundColor, 0xFF00_0000);

        draft.reset_section(Section::Theme);

        assert_eq!(draft.working().pager_height, 120);
        assert_eq!(draft.working().primary_update_delay, 500);
        assert_eq!(
            draft.working().desktop_background_color,
            Settings::default().desktop_background_color
        );
    }

    #[test]
    fn revert_targets_the_opening_values() {
        let mut draft = draft();
        draft.set_number(Field::PagerHeight, 120.0);
        draft.set_number(Field::PagerHeight, 160.0);
        draft.revert(Field::PagerHeight);

        assert_eq!(
            draft.working().pager_height,
            Settings::default().pager_height
        );
        assert_eq!(
            draft.original().pager_height,
            Settings::default().pager_height
        );
    }

    #[test]
    fn pager_height_stays_above_the_icon_padding() {
        let mut draft = draft();
        draft.set_number(Field::IconPaddingY, 100.0);
        draft.set_number(Field::PagerHeight, 20.0);

        assert_eq!(draft.working().pager_height, 116);
        assert_eq!(draft.min_pager_height(), 116);

        draft.set_number(Field::IconPaddingY, -100.0);
        draft.set_number(Field::PagerHeight, 1.0);

        assert_eq!(draft.working().pager_height, MIN_PAGER_HEIGHT);
    }

    #[test]
    fn numeric_controls_are_clamped() {
        let mut draft = draft();
        draft.set_number(Field::PrimaryUpdateDelay, 0.0);
        draft.set_number(Field::SecondaryUpdateDelay, 10.0);
        draft.set_number(Field::IconPaddingX, 20_000.0);

        assert_eq!(draft.working().primary_update_delay, MIN_PRIMARY_DELAY);
        assert_eq!(draft.working().secondary_update_delay, MIN_SECONDARY_DELAY);
        assert_eq!(draft.working().icon_padding_x, MAX_ICON_PADDING);

        draft.set_opacity_percent(5.0);
        assert_eq!(draft.working().background_opacity, 0.25);
    }

    #[test]
    fn combo_indices_match_the_csharp_enums() {
        let mut draft = draft();
        draft.set_index(Field::ViewModeDefault, 0);
        draft.set_index(Field::DesktopBorderStyle, 1);

        assert_eq!(draft.working().view_mode_default, ViewMode::Preview);
        assert_eq!(
            draft.working().desktop_border_style,
            DesktopBorderStyle::Underline
        );
    }
}
