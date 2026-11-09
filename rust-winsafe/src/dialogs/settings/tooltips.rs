//! Native tooltip controls for the fixed Settings dialog.
//!
//! winsafe 0.0.29 has no tooltip wrapper, so this is a small raw `windows`
//! helper. The texts mirror the C# `AppSettingsWindow` tooltips: the long ones
//! wrap because `TTM_SETMAXTIPWIDTH` is set.
//!
//! A single `tooltips_class32` window is owned by the main dialog. Tools are
//! added for controls on the main dialog and on each tab page, because the tab
//! pages are child dialogs and `GetDlgItem` must be called on the page that
//! owns the control.
//!
//! Tooltip text pointers must stay valid for as long as the tooltip exists, so
//! the converted UTF-16 buffers live in [`Tooltips`] and the caller keeps that
//! value alive for the dialog lifetime.

use std::mem::size_of;

use windows::Win32::Foundation::{HWND as WinHWND, LPARAM, WPARAM};
use windows::Win32::UI::Controls::{
    TOOLTIPS_CLASSW, TTF_IDISHWND, TTF_SUBCLASS, TTM_ACTIVATE, TTM_ADDTOOLW, TTM_SETMAXTIPWIDTH,
    TTS_ALWAYSTIP, TTS_NOPREFIX, TTTOOLINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CreateWindowExW, GetDlgItem, SendMessageW, WINDOW_STYLE, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::PWSTR;

use super::ids;

/// Widest tooltip before the control wraps the text, in physical pixels.
const MAX_TIP_WIDTH: isize = 420;

/// Tooltips attached to label controls. Labels are static text without an id in
/// the template, so each row that needs help has its own `IDC_*_LABEL` id.
const LABEL_TIPS: &[(u16, &str)] = &[
    (
        ids::IDC_VIEW_MODE_LABEL,
        "Only affects newly created desktops and their screens.",
    ),
    (
        ids::IDC_PAGER_HEIGHT_LABEL,
        "The pager's width is automatically calculated from this value and from the size of all \
         available screen space as well as the amount of virtual desktops.",
    ),
    (
        ids::IDC_ICON_PAD_X_LABEL,
        "Negative values are supported as well so less space is taken and the icons look stacked.",
    ),
    (
        ids::IDC_ICON_PAD_Y_LABEL,
        "Negative values are supported as well in case of a very small pager height, like 40px \
         which is usually the size of the Windows Taskbar.",
    ),
    (
        ids::IDC_PRIMARY_DELAY_LABEL,
        "Delay for the main refresh cycle. Affects how often the currently opened windows (on all \
         virtual desktops) are checked for changes.\n\nLower values update the pager faster, but \
         can increase CPU usage.",
    ),
    (
        ids::IDC_SECONDARY_DELAY_LABEL,
        "Delay for background/secondary updates. Affects how often the app window is checked for \
         being covered by another one, this includes the Windows Taskbar. Also (re-)pins the app \
         to all virtual desktops.\n\nSome of those operations require more processing, that's why \
         this delay should be higher than the primary one.",
    ),
];

const REVERT_TOOLTIP: &str = "Revert to default";
const DEFAULTS_TOOLTIP: &str = "Reset for the current tab only";

/// One tooltip control plus the text buffers its tools point into.
pub(super) struct Tooltips {
    hwnd: WinHWND,
    texts: Vec<Vec<u16>>,
}

impl Tooltips {
    /// Creates the tooltip window owned by `owner`.
    ///
    /// Returns `None` if the system refuses to create the control; the dialog
    /// still works without tooltips.
    pub(super) fn new(owner: WinHWND) -> Option<Self> {
        let style = WS_POPUP | WINDOW_STYLE(TTS_ALWAYSTIP | TTS_NOPREFIX);
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST,
                TOOLTIPS_CLASSW,
                windows::core::w!(""),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                Some(owner),
                None,
                None,
                None,
            )
        };
        let hwnd = match hwnd {
            Ok(hwnd) => hwnd,
            Err(error) => {
                crate::logging::warn(format!(
                    "settings tooltip control could not be created: {error}"
                ));
                return None;
            }
        };

        unsafe {
            // A tooltip created directly with CreateWindowExW starts inactive;
            // TTS_ALWAYSTIP controls when it can show, not whether it is active.
            SendMessageW(hwnd, TTM_ACTIVATE, Some(WPARAM(1)), Some(LPARAM(0)));
            SendMessageW(
                hwnd,
                TTM_SETMAXTIPWIDTH,
                Some(WPARAM(0)),
                Some(LPARAM(MAX_TIP_WIDTH)),
            );
        }

        Some(Self {
            hwnd,
            texts: Vec::with_capacity(LABEL_TIPS.len() + 20),
        })
    }

    /// Adds tooltips for label ids on the dialog or page that owns them.
    pub(super) fn add_labels(&mut self, owner: WinHWND) {
        for (id, text) in LABEL_TIPS {
            self.add(owner, *id, text);
        }
    }

    /// Adds the same revert tooltip to every row's revert button.
    pub(super) fn add_reverts(&mut self, owner: WinHWND, ids: &[u16]) {
        for id in ids {
            self.add(owner, *id, REVERT_TOOLTIP);
        }
    }

    /// Adds the Defaults-button tooltip.
    pub(super) fn add_defaults(&mut self, owner: WinHWND) {
        self.add(owner, ids::IDC_SETTINGS_DEFAULTS, DEFAULTS_TOOLTIP);
    }

    /// Number of tools that were accepted by the tooltip control.
    pub(super) fn tool_count(&self) -> usize {
        self.texts.len()
    }

    fn add(&mut self, owner: WinHWND, id: u16, text: &str) {
        // SAFETY: `owner` is a live dialog/page handle and `id` names a control
        // created from the template before `wm_init_dialog` runs.
        let Ok(control) = (unsafe { GetDlgItem(Some(owner), i32::from(id)) }) else {
            return;
        };
        if control.0.is_null() {
            return;
        }

        let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let pointer = wide.as_mut_ptr();
        self.texts.push(wide);

        let mut info = TTTOOLINFOW {
            cbSize: size_of::<TTTOOLINFOW>() as u32,
            uFlags: TTF_IDISHWND | TTF_SUBCLASS,
            hwnd: owner,
            uId: control.0 as usize,
            lpszText: PWSTR(pointer),
            ..Default::default()
        };

        // SAFETY: `info` is valid for the duration of the call and its text
        // pointer stays valid because `self.texts` owns the buffer.
        let added = unsafe {
            SendMessageW(
                self.hwnd,
                TTM_ADDTOOLW,
                Some(WPARAM(0)),
                Some(LPARAM(&mut info as *mut TTTOOLINFOW as isize)),
            )
        };
        if added.0 == 0 {
            crate::logging::warn(format!("settings tooltip could not be added for id {id}"));
        }
    }
}
