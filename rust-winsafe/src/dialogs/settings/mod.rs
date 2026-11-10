//! winsafe Settings dialog loaded from a fixed `.rc` dialog template.
//!
//! The three pages are `TabPage` dialogs; winsafe handles the tab selection and
//! resizes the selected page to the tab's display rectangle. All edits target a
//! working copy (`SettingsDraft`); the caller applies the result only when the
//! user presses Save.
//!
//! # Reading guide
//!
//! - [`show`] creates the controls and registers all events, then blocks in the
//!   modal loop and returns the accepted settings.
//! - `build_reverts` and the `attach_*` functions translate winsafe events into
//!   draft updates, revert-button refreshes, and live previews.
//! - `load_all` writes the draft into the controls; `read_edits` and `commit`
//!   read the controls back on Save.
//! - Colors use owner-drawn buttons and `ChooseColorW`; placement reuses the
//!   C# right/left/below/above sequence.
//!
//! The draft, its limits, and the placement algorithm are shared with
//! `rust-winui`; keep both sides behavior-compatible.

mod ids;
mod tooltips;

use std::cell::RefCell;
use std::mem::size_of;
use std::rc::Rc;

use winsafe::{self as w, co, gui, prelude::*};

use windows::Win32::Foundation::{COLORREF, HWND as WinHWND, RECT};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, DeleteObject, FillRect, FrameRect, HGDIOBJ};
use windows::Win32::UI::Controls::DRAWITEMSTRUCT;
use windows::Win32::UI::Controls::Dialogs::{CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SetWindowPos,
};

use super::placement::{self, Rect};
use crate::monitors;
use crate::settings::{DesktopBorderStyle, Settings};
use micro_task_pager_core::dialogs::settings_draft as draft;
use micro_task_pager_core::dialogs::settings_draft::{Field, Section, SettingsDraft};

#[derive(Clone)]
struct Controls {
    view_mode: gui::ComboBox,
    pager_height: gui::Edit,
    pager_height_spin: gui::UpDown,
    icon_pad_x: gui::Edit,
    icon_pad_x_spin: gui::UpDown,
    icon_pad_y: gui::Edit,
    icon_pad_y_spin: gui::UpDown,
    opacity: gui::Trackbar,
    opacity_label: gui::Label,
    border_style: gui::ComboBox,
    desktop_bg: gui::Button,
    desktop_bg_preview: gui::Label,
    desktop_border: gui::Button,
    desktop_border_preview: gui::Label,
    active_desktop_border: gui::Button,
    active_desktop_border_preview: gui::Label,
    window_bg: gui::Button,
    window_bg_preview: gui::Label,
    window_border: gui::Button,
    window_border_preview: gui::Label,
    active_window_border: gui::Button,
    active_window_border_preview: gui::Label,
    active_window_bg: gui::Button,
    active_window_bg_preview: gui::Label,
    primary_delay: gui::Edit,
    primary_delay_spin: gui::UpDown,
    secondary_delay: gui::Edit,
    secondary_delay_spin: gui::UpDown,
    show_taskbar: gui::CheckBox,
}

type Reverts = Rc<RefCell<Vec<(Field, gui::Button)>>>;

/// Undo arrow shown on every revert button. The RC compiler in this build does
/// not reliably encode Unicode string literals, so the template keeps `R` and
/// the glyph is applied in `wm_init_dialog`, after the page dialogs exist.
const REVERT_GLYPH: &str = "\u{21BA}";

/// Revert buttons per tab page, used to attach the shared tooltip.
const LAYOUT_REVERTS: [u16; 4] = [
    ids::IDC_REVERT_VIEW_MODE,
    ids::IDC_REVERT_PAGER_HEIGHT,
    ids::IDC_REVERT_ICON_PAD_X,
    ids::IDC_REVERT_ICON_PAD_Y,
];
const THEME_REVERTS: [u16; 9] = [
    ids::IDC_REVERT_OPACITY,
    ids::IDC_REVERT_BORDER_STYLE,
    ids::IDC_REVERT_DESKTOP_BG,
    ids::IDC_REVERT_DESKTOP_BORDER,
    ids::IDC_REVERT_ACTIVE_DESKTOP_BORDER,
    ids::IDC_REVERT_WINDOW_BG,
    ids::IDC_REVERT_WINDOW_BORDER,
    ids::IDC_REVERT_ACTIVE_WINDOW_BORDER,
    ids::IDC_REVERT_ACTIVE_WINDOW_BG,
];
const MISC_REVERTS: [u16; 3] = [
    ids::IDC_REVERT_PRIMARY_DELAY,
    ids::IDC_REVERT_SECONDARY_DELAY,
    ids::IDC_REVERT_SHOW_TASKBAR,
];

// ---- Entry point --------------------------------------------------------

/// Shows the modal Settings dialog and returns the accepted settings.
///
/// `preview` applies the working copy to the live pager while the dialog is
/// open; it must not persist anything.
pub fn show(
    parent: &gui::WindowMain,
    initial: &Settings,
    preview: Rc<dyn Fn(&Settings)>,
) -> Option<Settings> {
    let dont_move = (gui::Horz::None, gui::Vert::None);
    let wnd = gui::WindowModal::new_dlg(ids::IDD_SETTINGS);

    let layout_page = gui::TabPage::new_dlg(&wnd, ids::IDD_SETTINGS_LAYOUT);
    let theme_page = gui::TabPage::new_dlg(&wnd, ids::IDD_SETTINGS_THEME);
    let misc_page = gui::TabPage::new_dlg(&wnd, ids::IDD_SETTINGS_MISC);

    let tabs = gui::Tab::new_dlg(
        &wnd,
        ids::IDC_SETTINGS_TABS,
        dont_move,
        &[
            ("Layout", layout_page.clone()),
            ("Theme", theme_page.clone()),
            ("Misc", misc_page.clone()),
        ],
    );

    let controls = Controls {
        view_mode: gui::ComboBox::new_dlg(&layout_page, ids::IDC_VIEW_MODE, dont_move),
        pager_height: gui::Edit::new_dlg(&layout_page, ids::IDC_PAGER_HEIGHT, dont_move),
        pager_height_spin: gui::UpDown::new_dlg(
            &layout_page,
            ids::IDC_PAGER_HEIGHT_SPIN,
            dont_move,
        ),
        icon_pad_x: gui::Edit::new_dlg(&layout_page, ids::IDC_ICON_PAD_X, dont_move),
        icon_pad_x_spin: gui::UpDown::new_dlg(&layout_page, ids::IDC_ICON_PAD_X_SPIN, dont_move),
        icon_pad_y: gui::Edit::new_dlg(&layout_page, ids::IDC_ICON_PAD_Y, dont_move),
        icon_pad_y_spin: gui::UpDown::new_dlg(&layout_page, ids::IDC_ICON_PAD_Y_SPIN, dont_move),
        opacity: gui::Trackbar::new_dlg(&theme_page, ids::IDC_OPACITY, dont_move),
        opacity_label: gui::Label::new_dlg(&theme_page, ids::IDC_OPACITY_LABEL, dont_move),
        border_style: gui::ComboBox::new_dlg(&theme_page, ids::IDC_BORDER_STYLE, dont_move),
        desktop_bg: gui::Button::new_dlg(&theme_page, ids::IDC_DESKTOP_BG, dont_move),
        desktop_bg_preview: gui::Label::new_dlg(
            &theme_page,
            ids::IDC_DESKTOP_BG_PREVIEW,
            dont_move,
        ),
        desktop_border: gui::Button::new_dlg(&theme_page, ids::IDC_DESKTOP_BORDER, dont_move),
        desktop_border_preview: gui::Label::new_dlg(
            &theme_page,
            ids::IDC_DESKTOP_BORDER_PREVIEW,
            dont_move,
        ),
        active_desktop_border: gui::Button::new_dlg(
            &theme_page,
            ids::IDC_ACTIVE_DESKTOP_BORDER,
            dont_move,
        ),
        active_desktop_border_preview: gui::Label::new_dlg(
            &theme_page,
            ids::IDC_ACTIVE_DESKTOP_BORDER_PREVIEW,
            dont_move,
        ),
        window_bg: gui::Button::new_dlg(&theme_page, ids::IDC_WINDOW_BG, dont_move),
        window_bg_preview: gui::Label::new_dlg(&theme_page, ids::IDC_WINDOW_BG_PREVIEW, dont_move),
        window_border: gui::Button::new_dlg(&theme_page, ids::IDC_WINDOW_BORDER, dont_move),
        window_border_preview: gui::Label::new_dlg(
            &theme_page,
            ids::IDC_WINDOW_BORDER_PREVIEW,
            dont_move,
        ),
        active_window_border: gui::Button::new_dlg(
            &theme_page,
            ids::IDC_ACTIVE_WINDOW_BORDER,
            dont_move,
        ),
        active_window_border_preview: gui::Label::new_dlg(
            &theme_page,
            ids::IDC_ACTIVE_WINDOW_BORDER_PREVIEW,
            dont_move,
        ),
        active_window_bg: gui::Button::new_dlg(&theme_page, ids::IDC_ACTIVE_WINDOW_BG, dont_move),
        active_window_bg_preview: gui::Label::new_dlg(
            &theme_page,
            ids::IDC_ACTIVE_WINDOW_BG_PREVIEW,
            dont_move,
        ),
        primary_delay: gui::Edit::new_dlg(&misc_page, ids::IDC_PRIMARY_DELAY, dont_move),
        primary_delay_spin: gui::UpDown::new_dlg(
            &misc_page,
            ids::IDC_PRIMARY_DELAY_SPIN,
            dont_move,
        ),
        secondary_delay: gui::Edit::new_dlg(&misc_page, ids::IDC_SECONDARY_DELAY, dont_move),
        secondary_delay_spin: gui::UpDown::new_dlg(
            &misc_page,
            ids::IDC_SECONDARY_DELAY_SPIN,
            dont_move,
        ),
        show_taskbar: gui::CheckBox::new_dlg(&misc_page, ids::IDC_SHOW_TASKBAR, dont_move),
    };

    let draft = Rc::new(RefCell::new(SettingsDraft::new(initial.clone())));
    let result = Rc::new(RefCell::new(None::<Settings>));
    let reverts: Reverts = Rc::new(RefCell::new(Vec::new()));

    build_reverts(
        &layout_page,
        &theme_page,
        &misc_page,
        &controls,
        &draft,
        &reverts,
        &preview,
    );
    attach_edits(&controls, &draft, &reverts, &preview);
    attach_combos(&controls, &draft, &reverts, &preview);
    attach_color_buttons(&controls, &draft, &reverts, &preview);
    attach_color_draw(&theme_page, &draft);
    attach_opacity(&controls, &draft, &reverts, &preview);
    attach_checkbox(&controls, &draft, &reverts, &preview);
    attach_buttons(&wnd, &tabs, &controls, &draft, &reverts, &result, &preview);

    let controls_init = controls.clone();
    let draft_init = Rc::clone(&draft);
    let reverts_init = Rc::clone(&reverts);
    let wnd_init = wnd.clone();
    let parent_init = parent.clone();
    let tooltips_init: Rc<RefCell<Option<tooltips::Tooltips>>> = Rc::new(RefCell::new(None));
    wnd.on().wm_init_dialog(move |_| {
        let _ = controls_init
            .view_mode
            .items()
            .add(&["Preview", "MicroTaskbar"]);
        let _ = controls_init
            .border_style
            .items()
            .add(&["Box", "Underline"]);
        controls_init.opacity.set_range(25, 100);
        controls_init
            .pager_height_spin
            .set_range(draft::MIN_PAGER_HEIGHT, draft::MAX_PAGER_HEIGHT);
        controls_init
            .icon_pad_x_spin
            .set_range(-draft::MAX_ICON_PADDING, draft::MAX_ICON_PADDING);
        controls_init
            .icon_pad_y_spin
            .set_range(-draft::MAX_ICON_PADDING, draft::MAX_ICON_PADDING);
        controls_init
            .primary_delay_spin
            .set_range(draft::MIN_PRIMARY_DELAY as i32, draft::MAX_DELAY as i32);
        controls_init
            .secondary_delay_spin
            .set_range(draft::MIN_SECONDARY_DELAY as i32, draft::MAX_DELAY as i32);

        // SAFETY: these controls were created by the dialog and live on this
        // thread; UDM_SETBUDDY only associates the spinner with its edit.
        unsafe {
            controls_init
                .opacity
                .hwnd()
                .SendMessage(w::msg::TbmSetTicFreq { freq: 5 });
            let _ = controls_init
                .pager_height_spin
                .hwnd()
                .SendMessage(w::msg::UdmSetBuddy {
                    hbuddy: controls_init.pager_height.hwnd(),
                });
            let _ = controls_init
                .icon_pad_x_spin
                .hwnd()
                .SendMessage(w::msg::UdmSetBuddy {
                    hbuddy: controls_init.icon_pad_x.hwnd(),
                });
            let _ = controls_init
                .icon_pad_y_spin
                .hwnd()
                .SendMessage(w::msg::UdmSetBuddy {
                    hbuddy: controls_init.icon_pad_y.hwnd(),
                });
            let _ = controls_init
                .primary_delay_spin
                .hwnd()
                .SendMessage(w::msg::UdmSetBuddy {
                    hbuddy: controls_init.primary_delay.hwnd(),
                });
            let _ = controls_init
                .secondary_delay_spin
                .hwnd()
                .SendMessage(w::msg::UdmSetBuddy {
                    hbuddy: controls_init.secondary_delay.hwnd(),
                });
        }

        let working = draft_init.borrow().working().clone();
        load_all(&controls_init, &working);
        refresh_reverts(&reverts_init, &draft_init);
        for (_, button) in reverts_init.borrow().iter() {
            let _ = button.hwnd().SetWindowText(REVERT_GLYPH);
        }
        install_tooltips(
            &wnd_init,
            &layout_page,
            &theme_page,
            &misc_page,
            &tooltips_init,
        );
        position_dialog(&wnd_init, &parent_init);
        Ok(true)
    });

    wnd.show_modal(parent).ok()?;
    result.borrow_mut().take()
}

/// Creates the dialog tooltip control and attaches every help text.
///
/// The tab pages are separate child dialogs, so each page's controls are looked
/// up through that page's handle. `slot` keeps the [`tooltips::Tooltips`] value
/// alive for the dialog lifetime; the tooltip text buffers it owns must outlive
/// the native tools.
fn install_tooltips(
    wnd: &gui::WindowModal,
    layout_page: &gui::TabPage,
    theme_page: &gui::TabPage,
    misc_page: &gui::TabPage,
    slot: &Rc<RefCell<Option<tooltips::Tooltips>>>,
) {
    let dialog = win_hwnd(wnd.hwnd());
    let Some(mut tips) = tooltips::Tooltips::new(dialog) else {
        return;
    };

    tips.add_defaults(dialog);
    tips.add_labels(win_hwnd(layout_page.hwnd()));
    tips.add_labels(win_hwnd(misc_page.hwnd()));
    tips.add_reverts(win_hwnd(layout_page.hwnd()), &LAYOUT_REVERTS);
    tips.add_reverts(win_hwnd(theme_page.hwnd()), &THEME_REVERTS);
    tips.add_reverts(win_hwnd(misc_page.hwnd()), &MISC_REVERTS);

    crate::logging::info(format!(
        "settings help tooltips installed: {} tools",
        tips.tool_count()
    ));
    *slot.borrow_mut() = Some(tips);
}

fn win_hwnd(hwnd: &w::HWND) -> WinHWND {
    WinHWND(hwnd.ptr())
}

// ---- Event wiring -------------------------------------------------------

/// Creates the per-row revert buttons and wires them to the draft.
///
/// Revert buttons start hidden and are shown by `refresh_reverts` whenever the
/// field differs from the value the dialog opened with.
fn build_reverts(
    layout_page: &gui::TabPage,
    theme_page: &gui::TabPage,
    misc_page: &gui::TabPage,
    controls: &Controls,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
    preview: &Rc<dyn Fn(&Settings)>,
) {
    let fields = [
        (
            layout_page,
            ids::IDC_REVERT_VIEW_MODE,
            Field::ViewModeDefault,
        ),
        (
            layout_page,
            ids::IDC_REVERT_PAGER_HEIGHT,
            Field::PagerHeight,
        ),
        (layout_page, ids::IDC_REVERT_ICON_PAD_X, Field::IconPaddingX),
        (layout_page, ids::IDC_REVERT_ICON_PAD_Y, Field::IconPaddingY),
        (
            theme_page,
            ids::IDC_REVERT_OPACITY,
            Field::BackgroundOpacity,
        ),
        (
            theme_page,
            ids::IDC_REVERT_BORDER_STYLE,
            Field::DesktopBorderStyle,
        ),
        (
            theme_page,
            ids::IDC_REVERT_DESKTOP_BG,
            Field::DesktopBackgroundColor,
        ),
        (
            theme_page,
            ids::IDC_REVERT_DESKTOP_BORDER,
            Field::DesktopBorderColor,
        ),
        (
            theme_page,
            ids::IDC_REVERT_ACTIVE_DESKTOP_BORDER,
            Field::ActiveDesktopBorderColor,
        ),
        (
            theme_page,
            ids::IDC_REVERT_WINDOW_BG,
            Field::WindowBackgroundColor,
        ),
        (
            theme_page,
            ids::IDC_REVERT_WINDOW_BORDER,
            Field::WindowBorderColor,
        ),
        (
            theme_page,
            ids::IDC_REVERT_ACTIVE_WINDOW_BORDER,
            Field::ActiveWindowBorderColor,
        ),
        (
            theme_page,
            ids::IDC_REVERT_ACTIVE_WINDOW_BG,
            Field::ActiveWindowBackgroundColor,
        ),
        (
            misc_page,
            ids::IDC_REVERT_PRIMARY_DELAY,
            Field::PrimaryUpdateDelay,
        ),
        (
            misc_page,
            ids::IDC_REVERT_SECONDARY_DELAY,
            Field::SecondaryUpdateDelay,
        ),
        (
            misc_page,
            ids::IDC_REVERT_SHOW_TASKBAR,
            Field::ShowAppInTaskbar,
        ),
    ];

    for (page, id, field) in fields {
        let button = gui::Button::new_dlg(page, id, (gui::Horz::None, gui::Vert::None));
        let draft2 = Rc::clone(draft);
        let reverts2 = Rc::clone(reverts);
        let controls2 = controls.clone();
        let preview2 = Rc::clone(preview);
        let button2 = button.clone();
        button.on().bn_clicked(move || {
            draft2.borrow_mut().revert(field);
            let working = draft2.borrow().working().clone();
            load_all(&controls2, &working);
            refresh_reverts(&reverts2, &draft2);
            preview2(&working);
            let _ = button2.hwnd().ShowWindow(co::SW::HIDE);
            Ok(())
        });
        reverts.borrow_mut().push((field, button));
    }
}

/// Wires the numeric edits to the draft.
fn attach_edits(
    controls: &Controls,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
    preview: &Rc<dyn Fn(&Settings)>,
) {
    attach_edit(
        &controls.pager_height,
        Field::PagerHeight,
        draft,
        reverts,
        preview,
    );
    attach_edit(
        &controls.icon_pad_x,
        Field::IconPaddingX,
        draft,
        reverts,
        preview,
    );
    attach_edit(
        &controls.icon_pad_y,
        Field::IconPaddingY,
        draft,
        reverts,
        preview,
    );
    attach_edit(
        &controls.primary_delay,
        Field::PrimaryUpdateDelay,
        draft,
        reverts,
        preview,
    );
    attach_edit(
        &controls.secondary_delay,
        Field::SecondaryUpdateDelay,
        draft,
        reverts,
        preview,
    );
}

/// Parses one numeric edit on every change and previews the result.
fn attach_edit(
    edit: &gui::Edit,
    field: Field,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
    preview: &Rc<dyn Fn(&Settings)>,
) {
    let edit2 = edit.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
    let preview2 = Rc::clone(preview);
    edit.on().en_change(move || {
        if let Ok(text) = edit2.text()
            && let Ok(value) = text.trim().parse::<f64>()
        {
            draft2.borrow_mut().set_number(field, value);
            refresh_reverts(&reverts2, &draft2);
            let working = draft2.borrow().working().clone();
            preview2(&working);
        }
        Ok(())
    });
}

/// Wires the view-mode and border-style combos to the draft.
fn attach_combos(
    controls: &Controls,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
    preview: &Rc<dyn Fn(&Settings)>,
) {
    let combo = &controls.view_mode;
    let combo2 = combo.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
    let preview2 = Rc::clone(preview);
    combo.on().cbn_sel_change(move || {
        if let Some(index) = combo2.items().selected_index() {
            draft2
                .borrow_mut()
                .set_index(Field::ViewModeDefault, index as usize);
            refresh_reverts(&reverts2, &draft2);
            let working = draft2.borrow().working().clone();
            preview2(&working);
        }
        Ok(())
    });

    let combo = &controls.border_style;
    let combo2 = combo.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
    let preview2 = Rc::clone(preview);
    combo.on().cbn_sel_change(move || {
        if let Some(index) = combo2.items().selected_index() {
            draft2
                .borrow_mut()
                .set_index(Field::DesktopBorderStyle, index as usize);
            refresh_reverts(&reverts2, &draft2);
            let working = draft2.borrow().working().clone();
            preview2(&working);
        }
        Ok(())
    });
}

/// Wires the color Choose buttons to the picker and previews the chosen color.
fn attach_color_buttons(
    controls: &Controls,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
    preview: &Rc<dyn Fn(&Settings)>,
) {
    let rows = [
        (
            &controls.desktop_bg,
            &controls.desktop_bg_preview,
            Field::DesktopBackgroundColor,
        ),
        (
            &controls.desktop_border,
            &controls.desktop_border_preview,
            Field::DesktopBorderColor,
        ),
        (
            &controls.active_desktop_border,
            &controls.active_desktop_border_preview,
            Field::ActiveDesktopBorderColor,
        ),
        (
            &controls.window_bg,
            &controls.window_bg_preview,
            Field::WindowBackgroundColor,
        ),
        (
            &controls.window_border,
            &controls.window_border_preview,
            Field::WindowBorderColor,
        ),
        (
            &controls.active_window_border,
            &controls.active_window_border_preview,
            Field::ActiveWindowBorderColor,
        ),
        (
            &controls.active_window_bg,
            &controls.active_window_bg_preview,
            Field::ActiveWindowBackgroundColor,
        ),
    ];

    for (button, swatch, field) in rows {
        let button2 = button.clone();
        let swatch2 = swatch.clone();
        let draft2 = Rc::clone(draft);
        let reverts2 = Rc::clone(reverts);
        let preview2 = Rc::clone(preview);
        button.on().bn_clicked(move || {
            let current = {
                let working = draft2.borrow();
                color_value(working.working(), field)
            };
            if let Some(argb) = choose_color(button2.hwnd(), current) {
                draft2.borrow_mut().set_color(field, argb);
                invalidate_preview(&swatch2);
                refresh_reverts(&reverts2, &draft2);
                let working = draft2.borrow().working().clone();
                preview2(&working);
            }
            Ok(())
        });
    }
}

/// Wires the opacity trackbar and its percentage label.
fn attach_opacity(
    controls: &Controls,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
    preview: &Rc<dyn Fn(&Settings)>,
) {
    let track2 = controls.opacity.clone();
    let label2 = controls.opacity_label.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
    let preview2 = Rc::clone(preview);
    controls.opacity.on().wm_h_scroll(move |_| {
        let percent = track2.pos();
        draft2.borrow_mut().set_opacity_percent(percent as f64);
        let _ = label2.set_text_and_resize(&format!("{percent}%"));
        refresh_reverts(&reverts2, &draft2);
        let working = draft2.borrow().working().clone();
        preview2(&working);
        Ok(())
    });
}

/// Wires the taskbar checkbox.
fn attach_checkbox(
    controls: &Controls,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
    preview: &Rc<dyn Fn(&Settings)>,
) {
    let check2 = controls.show_taskbar.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
    let preview2 = Rc::clone(preview);
    controls.show_taskbar.on().bn_clicked(move || {
        draft2
            .borrow_mut()
            .set_check(Field::ShowAppInTaskbar, check2.is_checked());
        refresh_reverts(&reverts2, &draft2);
        let working = draft2.borrow().working().clone();
        preview2(&working);
        Ok(())
    });
}

/// Wires Defaults, Save, Cancel, and the Escape accelerator.
fn attach_buttons(
    wnd: &gui::WindowModal,
    tabs: &gui::Tab,
    controls: &Controls,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
    result: &Rc<RefCell<Option<Settings>>>,
    preview: &Rc<dyn Fn(&Settings)>,
) {
    let dont_move = (gui::Horz::None, gui::Vert::None);

    let defaults = gui::Button::new_dlg(wnd, ids::IDC_SETTINGS_DEFAULTS, dont_move);
    let tabs2 = tabs.clone();
    let controls2 = controls.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
    let preview2 = Rc::clone(preview);
    defaults.on().bn_clicked(move || {
        let section = tabs2
            .items()
            .selected()
            .map(|item| Section::from_index(item.index() as usize))
            .unwrap_or_default();
        draft2.borrow_mut().reset_section(section);
        let working = draft2.borrow().working().clone();
        load_all(&controls2, &working);
        refresh_reverts(&reverts2, &draft2);
        preview2(&working);
        Ok(())
    });

    let save = gui::Button::new_dlg(wnd, ids::IDOK, dont_move);
    let controls2 = controls.clone();
    let draft2 = Rc::clone(draft);
    let result2 = Rc::clone(result);
    let wnd2 = wnd.clone();
    save.on()
        .bn_clicked(move || commit(&controls2, &draft2, &result2, &wnd2));

    let controls2 = controls.clone();
    let draft2 = Rc::clone(draft);
    let result2 = Rc::clone(result);
    let wnd2 = wnd.clone();
    wnd.on().wm_command_acc_menu(co::DLGID::OK, move || {
        commit(&controls2, &draft2, &result2, &wnd2)
    });

    let cancel = gui::Button::new_dlg(wnd, ids::IDCANCEL, dont_move);
    let wnd2 = wnd.clone();
    cancel.on().bn_clicked(move || {
        wnd2.hwnd().EndDialog(0)?;
        Ok(())
    });

    let wnd2 = wnd.clone();
    wnd.on().wm_command_acc_menu(co::DLGID::CANCEL, move || {
        wnd2.hwnd().EndDialog(0)?;
        Ok(())
    });
}

/// Reads the edits, stores the working copy, and closes the dialog.
fn commit(
    controls: &Controls,
    draft: &Rc<RefCell<SettingsDraft>>,
    result: &Rc<RefCell<Option<Settings>>>,
    wnd: &gui::WindowModal,
) -> w::AnyResult<()> {
    read_edits(controls, draft);
    *result.borrow_mut() = Some(draft.borrow().working().clone());
    wnd.hwnd().EndDialog(0)?;
    Ok(())
}

// ---- Loading and reading controls ---------------------------------------

/// Reads every edit back into the draft; called on Save so a value that was
/// typed but never blurred is not lost.
fn read_edits(controls: &Controls, draft: &Rc<RefCell<SettingsDraft>>) {
    let edits = [
        (&controls.pager_height, Field::PagerHeight),
        (&controls.icon_pad_x, Field::IconPaddingX),
        (&controls.icon_pad_y, Field::IconPaddingY),
        (&controls.primary_delay, Field::PrimaryUpdateDelay),
        (&controls.secondary_delay, Field::SecondaryUpdateDelay),
    ];

    for (edit, field) in edits {
        if let Ok(text) = edit.text()
            && let Ok(value) = text.trim().parse::<f64>()
        {
            draft.borrow_mut().set_number(field, value);
        }
    }
}

/// Writes a complete settings value into every control.
fn load_all(controls: &Controls, settings: &Settings) {
    controls
        .view_mode
        .items()
        .select(Some(settings.view_mode_default.as_u8() as u32));
    let _ = controls
        .pager_height
        .set_text(&settings.pager_height.to_string());
    controls.pager_height_spin.set_pos(settings.pager_height);
    let _ = controls
        .icon_pad_x
        .set_text(&settings.icon_padding_x.to_string());
    controls.icon_pad_x_spin.set_pos(settings.icon_padding_x);
    let _ = controls
        .icon_pad_y
        .set_text(&settings.icon_padding_y.to_string());
    controls.icon_pad_y_spin.set_pos(settings.icon_padding_y);

    let opacity = (settings.background_opacity * 100.0).round() as u32;
    controls.opacity.set_pos(opacity.clamp(25, 100));
    let _ = controls
        .opacity_label
        .set_text_and_resize(&format!("{opacity}%"));

    controls
        .border_style
        .items()
        .select(Some(match settings.desktop_border_style {
            DesktopBorderStyle::Box => 0,
            DesktopBorderStyle::Underline => 1,
        }));

    invalidate_preview(&controls.desktop_bg_preview);
    invalidate_preview(&controls.desktop_border_preview);
    invalidate_preview(&controls.active_desktop_border_preview);
    invalidate_preview(&controls.window_bg_preview);
    invalidate_preview(&controls.window_border_preview);
    invalidate_preview(&controls.active_window_border_preview);
    invalidate_preview(&controls.active_window_bg_preview);

    let _ = controls
        .primary_delay
        .set_text(&settings.primary_update_delay.to_string());
    controls
        .primary_delay_spin
        .set_pos(settings.primary_update_delay as i32);
    let _ = controls
        .secondary_delay
        .set_text(&settings.secondary_update_delay.to_string());
    controls
        .secondary_delay_spin
        .set_pos(settings.secondary_update_delay as i32);
    controls
        .show_taskbar
        .set_check(settings.show_app_in_taskbar);
}

/// Shows or hides each revert button based on whether its field differs from
/// the opening settings.
fn refresh_reverts(reverts: &Reverts, draft: &Rc<RefCell<SettingsDraft>>) {
    let states: Vec<(gui::Button, bool)> = {
        let draft = draft.borrow();
        reverts
            .borrow()
            .iter()
            .map(|(field, button)| (button.clone(), draft.is_modified(*field)))
            .collect()
    };

    for (button, show) in states {
        let _ = button
            .hwnd()
            .ShowWindow(if show { co::SW::SHOW } else { co::SW::HIDE });
    }
}

// ---- Colors and placement -----------------------------------------------

/// Places the dialog next to the pager using the shared placement algorithm
/// and keeps it topmost so the pager cannot cover it.
fn position_dialog(wnd: &gui::WindowModal, parent: &gui::WindowMain) {
    let mut owner = RECT::default();
    let mut dialog = RECT::default();
    unsafe {
        let _ = GetWindowRect(WinHWND(parent.hwnd().ptr()), &mut owner);
        let _ = GetWindowRect(WinHWND(wnd.hwnd().ptr()), &mut dialog);
    }

    let size = (dialog.right - dialog.left, dialog.bottom - dialog.top);
    let work_areas: Vec<Rect> = monitors::enumerate()
        .iter()
        .map(|monitor| monitor.work_rect.into())
        .collect();
    let (x, y) = placement::find_placement(owner.into(), size, &work_areas);

    unsafe {
        let _ = SetWindowPos(
            WinHWND(wnd.hwnd().ptr()),
            Some(HWND_TOPMOST),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Repaints a color preview so its owner-drawn swatch reflects the draft.
fn invalidate_preview(preview: &gui::Label) {
    let _ = preview.hwnd().InvalidateRect(None, true);
}

/// Handles `WM_DRAWITEM` for the color previews on the Theme page.
fn attach_color_draw(page: &gui::TabPage, draft: &Rc<RefCell<SettingsDraft>>) {
    let draft2 = Rc::clone(draft);
    page.on().wm(co::WM::DRAWITEM, move |p| {
        // SAFETY: WM_DRAWITEM carries a DRAWITEMSTRUCT pointer in lParam.
        let item = unsafe { &*(p.lparam as *const DRAWITEMSTRUCT) };
        if let Some(field) = color_field(item.CtlID as u16) {
            let argb = {
                let draft = draft2.borrow();
                color_value(draft.working(), field)
            };
            draw_color_preview(item, argb);
        }
        Ok(0)
    });
}

/// Maps a color preview control id to its draft field.
fn color_field(id: u16) -> Option<Field> {
    match id {
        ids::IDC_DESKTOP_BG_PREVIEW => Some(Field::DesktopBackgroundColor),
        ids::IDC_DESKTOP_BORDER_PREVIEW => Some(Field::DesktopBorderColor),
        ids::IDC_ACTIVE_DESKTOP_BORDER_PREVIEW => Some(Field::ActiveDesktopBorderColor),
        ids::IDC_WINDOW_BG_PREVIEW => Some(Field::WindowBackgroundColor),
        ids::IDC_WINDOW_BORDER_PREVIEW => Some(Field::WindowBorderColor),
        ids::IDC_ACTIVE_WINDOW_BORDER_PREVIEW => Some(Field::ActiveWindowBorderColor),
        ids::IDC_ACTIVE_WINDOW_BG_PREVIEW => Some(Field::ActiveWindowBackgroundColor),
        _ => None,
    }
}

/// Fills a color preview with its color and a black frame.
fn draw_color_preview(item: &DRAWITEMSTRUCT, argb: u32) {
    unsafe {
        // Settings store ARGB, but COLORREF expects 0x00BBGGRR.
        let brush = CreateSolidBrush(COLORREF(colorref_from_argb(argb)));
        FillRect(item.hDC, &item.rcItem, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));

        let border = CreateSolidBrush(COLORREF(0));
        FrameRect(item.hDC, &item.rcItem, border);
        let _ = DeleteObject(HGDIOBJ(border.0));
    }
}

/// Reads a color field from settings, or zero for non-color fields.
fn color_value(settings: &Settings, field: Field) -> u32 {
    match field {
        Field::DesktopBackgroundColor => settings.desktop_background_color,
        Field::DesktopBorderColor => settings.desktop_border_color,
        Field::ActiveDesktopBorderColor => settings.active_desktop_border_color,
        Field::WindowBackgroundColor => settings.window_background_color,
        Field::WindowBorderColor => settings.window_border_color,
        Field::ActiveWindowBorderColor => settings.active_window_border_color,
        Field::ActiveWindowBackgroundColor => settings.active_window_background_color,
        _ => 0,
    }
}

/// Opens the native color picker seeded with `argb`; returns the new ARGB value
/// when the user accepts.
fn choose_color(owner: &w::HWND, argb: u32) -> Option<u32> {
    let mut custom = [COLORREF(0); 16];
    let mut choose = CHOOSECOLORW {
        lStructSize: size_of::<CHOOSECOLORW>() as u32,
        hwndOwner: WinHWND(owner.ptr()),
        rgbResult: COLORREF(colorref_from_argb(argb)),
        lpCustColors: custom.as_mut_ptr(),
        Flags: CC_FULLOPEN | CC_RGBINIT,
        ..Default::default()
    };

    // SAFETY: `choose` and `custom` live for the duration of the modal call.
    let accepted = unsafe { ChooseColorW(&mut choose).as_bool() };
    accepted.then(|| argb_from_colorref(choose.rgbResult))
}

fn colorref_from_argb(argb: u32) -> u32 {
    let red = (argb >> 16) & 0xFF;
    let green = (argb >> 8) & 0xFF;
    let blue = argb & 0xFF;
    red | (green << 8) | (blue << 16)
}

fn argb_from_colorref(color: COLORREF) -> u32 {
    let red = color.0 & 0xFF;
    let green = (color.0 >> 8) & 0xFF;
    let blue = (color.0 >> 16) & 0xFF;
    0xFF00_0000 | (red << 16) | (green << 8) | blue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorref_round_trips_argb() {
        let argb = 0xFF12_3456;
        assert_eq!(argb_from_colorref(COLORREF(colorref_from_argb(argb))), argb);
    }
}
