//! Control IDs shared between the dialog templates and the Rust wrapper.
//!
//! Keep these values in sync with `settings_ids.h`. The header is what the
//! resource compiler sees; this module is what winsafe's `new_dlg` uses.

pub(super) const IDD_SETTINGS: u16 = 200;
pub(super) const IDD_SETTINGS_LAYOUT: u16 = 201;
pub(super) const IDD_SETTINGS_THEME: u16 = 202;
pub(super) const IDD_SETTINGS_MISC: u16 = 203;

pub(super) const IDOK: u16 = 1;
pub(super) const IDCANCEL: u16 = 2;

pub(super) const IDC_SETTINGS_TABS: u16 = 1000;
pub(super) const IDC_SETTINGS_DEFAULTS: u16 = 1001;

pub(super) const IDC_VIEW_MODE: u16 = 1010;
pub(super) const IDC_VIEW_MODE_LABEL: u16 = 1060;
pub(super) const IDC_PAGER_HEIGHT: u16 = 1011;
pub(super) const IDC_PAGER_HEIGHT_LABEL: u16 = 1061;
pub(super) const IDC_PAGER_HEIGHT_SPIN: u16 = 1051;
pub(super) const IDC_ICON_PAD_X: u16 = 1012;
pub(super) const IDC_ICON_PAD_X_LABEL: u16 = 1062;
pub(super) const IDC_ICON_PAD_X_SPIN: u16 = 1052;
pub(super) const IDC_ICON_PAD_Y: u16 = 1013;
pub(super) const IDC_ICON_PAD_Y_LABEL: u16 = 1063;
pub(super) const IDC_ICON_PAD_Y_SPIN: u16 = 1053;

pub(super) const IDC_OPACITY: u16 = 1020;
pub(super) const IDC_OPACITY_LABEL: u16 = 1021;
pub(super) const IDC_BORDER_STYLE: u16 = 1022;
pub(super) const IDC_DESKTOP_BG: u16 = 1023;
pub(super) const IDC_DESKTOP_BORDER: u16 = 1024;
pub(super) const IDC_ACTIVE_DESKTOP_BORDER: u16 = 1025;
pub(super) const IDC_WINDOW_BG: u16 = 1026;
pub(super) const IDC_WINDOW_BORDER: u16 = 1027;
pub(super) const IDC_ACTIVE_WINDOW_BORDER: u16 = 1028;
pub(super) const IDC_ACTIVE_WINDOW_BG: u16 = 1029;

pub(super) const IDC_PRIMARY_DELAY: u16 = 1040;
pub(super) const IDC_PRIMARY_DELAY_LABEL: u16 = 1064;
pub(super) const IDC_PRIMARY_DELAY_SPIN: u16 = 1054;
pub(super) const IDC_SECONDARY_DELAY: u16 = 1041;
pub(super) const IDC_SECONDARY_DELAY_LABEL: u16 = 1065;
pub(super) const IDC_SECONDARY_DELAY_SPIN: u16 = 1055;
pub(super) const IDC_SHOW_TASKBAR: u16 = 1042;

pub(super) const IDC_REVERT_VIEW_MODE: u16 = 1110;
pub(super) const IDC_REVERT_PAGER_HEIGHT: u16 = 1111;
pub(super) const IDC_REVERT_ICON_PAD_X: u16 = 1112;
pub(super) const IDC_REVERT_ICON_PAD_Y: u16 = 1113;

pub(super) const IDC_REVERT_OPACITY: u16 = 1120;
pub(super) const IDC_REVERT_BORDER_STYLE: u16 = 1122;
pub(super) const IDC_REVERT_DESKTOP_BG: u16 = 1123;
pub(super) const IDC_REVERT_DESKTOP_BORDER: u16 = 1124;
pub(super) const IDC_REVERT_ACTIVE_DESKTOP_BORDER: u16 = 1125;
pub(super) const IDC_REVERT_WINDOW_BG: u16 = 1126;
pub(super) const IDC_REVERT_WINDOW_BORDER: u16 = 1127;
pub(super) const IDC_REVERT_ACTIVE_WINDOW_BORDER: u16 = 1128;
pub(super) const IDC_REVERT_ACTIVE_WINDOW_BG: u16 = 1129;

pub(super) const IDC_REVERT_PRIMARY_DELAY: u16 = 1140;
pub(super) const IDC_REVERT_SECONDARY_DELAY: u16 = 1141;
pub(super) const IDC_REVERT_SHOW_TASKBAR: u16 = 1142;
