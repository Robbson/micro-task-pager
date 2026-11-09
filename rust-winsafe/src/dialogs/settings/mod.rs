//! winsafe Settings dialog loaded from a fixed `.rc` dialog template.
//!
//! The three pages are `TabPage` dialogs; winsafe handles the tab selection and
//! resizes the selected page to the tab's display rectangle. All edits target a
//! working copy (`SettingsDraft`); the caller applies the result only when the
//! user presses Save.

mod draft;
mod ids;

use std::cell::RefCell;
use std::mem::size_of;
use std::rc::Rc;

use winsafe::{self as w, co, gui, prelude::*};

use windows::Win32::Foundation::{COLORREF, HWND as WinHWND, RECT};
use windows::Win32::UI::Controls::Dialogs::{CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SetWindowPos,
};

use super::placement::{self, Rect};
use crate::monitors;
use crate::settings::{DesktopBorderStyle, Settings};
use draft::{Field, Section, SettingsDraft};

#[derive(Clone)]
struct Controls {
    view_mode: gui::ComboBox,
    pager_height: gui::Edit,
    icon_pad_x: gui::Edit,
    icon_pad_y: gui::Edit,
    opacity: gui::Trackbar,
    opacity_label: gui::Label,
    border_style: gui::ComboBox,
    desktop_bg: gui::Button,
    desktop_border: gui::Button,
    active_desktop_border: gui::Button,
    window_bg: gui::Button,
    window_border: gui::Button,
    active_window_border: gui::Button,
    active_window_bg: gui::Button,
    primary_delay: gui::Edit,
    secondary_delay: gui::Edit,
    show_taskbar: gui::CheckBox,
}

type Reverts = Rc<RefCell<Vec<(Field, gui::Button)>>>;

/// Shows the modal Settings dialog and returns the accepted settings.
pub fn show(parent: &gui::WindowMain, initial: &Settings) -> Option<Settings> {
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
        icon_pad_x: gui::Edit::new_dlg(&layout_page, ids::IDC_ICON_PAD_X, dont_move),
        icon_pad_y: gui::Edit::new_dlg(&layout_page, ids::IDC_ICON_PAD_Y, dont_move),
        opacity: gui::Trackbar::new_dlg(&theme_page, ids::IDC_OPACITY, dont_move),
        opacity_label: gui::Label::new_dlg(&theme_page, ids::IDC_OPACITY_LABEL, dont_move),
        border_style: gui::ComboBox::new_dlg(&theme_page, ids::IDC_BORDER_STYLE, dont_move),
        desktop_bg: gui::Button::new_dlg(&theme_page, ids::IDC_DESKTOP_BG, dont_move),
        desktop_border: gui::Button::new_dlg(&theme_page, ids::IDC_DESKTOP_BORDER, dont_move),
        active_desktop_border: gui::Button::new_dlg(
            &theme_page,
            ids::IDC_ACTIVE_DESKTOP_BORDER,
            dont_move,
        ),
        window_bg: gui::Button::new_dlg(&theme_page, ids::IDC_WINDOW_BG, dont_move),
        window_border: gui::Button::new_dlg(&theme_page, ids::IDC_WINDOW_BORDER, dont_move),
        active_window_border: gui::Button::new_dlg(
            &theme_page,
            ids::IDC_ACTIVE_WINDOW_BORDER,
            dont_move,
        ),
        active_window_bg: gui::Button::new_dlg(&theme_page, ids::IDC_ACTIVE_WINDOW_BG, dont_move),
        primary_delay: gui::Edit::new_dlg(&misc_page, ids::IDC_PRIMARY_DELAY, dont_move),
        secondary_delay: gui::Edit::new_dlg(&misc_page, ids::IDC_SECONDARY_DELAY, dont_move),
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
    );
    attach_edits(&controls, &draft, &reverts);
    attach_combos(&controls, &draft, &reverts);
    attach_color_buttons(&controls, &draft, &reverts);
    attach_opacity(&controls, &draft, &reverts);
    attach_checkbox(&controls, &draft, &reverts);
    attach_buttons(&wnd, &tabs, &controls, &draft, &reverts, &result);

    let controls_init = controls.clone();
    let draft_init = Rc::clone(&draft);
    let reverts_init = Rc::clone(&reverts);
    let wnd_init = wnd.clone();
    let parent_init = parent.clone();
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

        let working = draft_init.borrow().working().clone();
        load_all(&controls_init, &working);
        refresh_reverts(&reverts_init, &draft_init);
        position_dialog(&wnd_init, &parent_init);
        Ok(true)
    });

    wnd.show_modal(parent).ok()?;
    result.borrow_mut().take()
}

fn build_reverts(
    layout_page: &gui::TabPage,
    theme_page: &gui::TabPage,
    misc_page: &gui::TabPage,
    controls: &Controls,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
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
        let button2 = button.clone();
        button.on().bn_clicked(move || {
            draft2.borrow_mut().revert(field);
            let working = draft2.borrow().working().clone();
            load_all(&controls2, &working);
            refresh_reverts(&reverts2, &draft2);
            let _ = button2.hwnd().ShowWindow(co::SW::HIDE);
            Ok(())
        });
        reverts.borrow_mut().push((field, button));
    }
}

fn attach_edits(controls: &Controls, draft: &Rc<RefCell<SettingsDraft>>, reverts: &Reverts) {
    attach_edit(&controls.pager_height, Field::PagerHeight, draft, reverts);
    attach_edit(&controls.icon_pad_x, Field::IconPaddingX, draft, reverts);
    attach_edit(&controls.icon_pad_y, Field::IconPaddingY, draft, reverts);
    attach_edit(
        &controls.primary_delay,
        Field::PrimaryUpdateDelay,
        draft,
        reverts,
    );
    attach_edit(
        &controls.secondary_delay,
        Field::SecondaryUpdateDelay,
        draft,
        reverts,
    );
}

fn attach_edit(
    edit: &gui::Edit,
    field: Field,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
) {
    let edit2 = edit.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
    edit.on().en_change(move || {
        if let Ok(text) = edit2.text()
            && let Ok(value) = text.trim().parse::<f64>()
        {
            draft2.borrow_mut().set_number(field, value);
            refresh_reverts(&reverts2, &draft2);
        }
        Ok(())
    });
}

fn attach_combos(controls: &Controls, draft: &Rc<RefCell<SettingsDraft>>, reverts: &Reverts) {
    let combo = &controls.view_mode;
    let combo2 = combo.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
    combo.on().cbn_sel_change(move || {
        if let Some(index) = combo2.items().selected_index() {
            draft2
                .borrow_mut()
                .set_index(Field::ViewModeDefault, index as usize);
            refresh_reverts(&reverts2, &draft2);
        }
        Ok(())
    });

    let combo = &controls.border_style;
    let combo2 = combo.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
    combo.on().cbn_sel_change(move || {
        if let Some(index) = combo2.items().selected_index() {
            draft2
                .borrow_mut()
                .set_index(Field::DesktopBorderStyle, index as usize);
            refresh_reverts(&reverts2, &draft2);
        }
        Ok(())
    });
}

fn attach_color_buttons(
    controls: &Controls,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
) {
    let buttons = [
        (&controls.desktop_bg, Field::DesktopBackgroundColor),
        (&controls.desktop_border, Field::DesktopBorderColor),
        (
            &controls.active_desktop_border,
            Field::ActiveDesktopBorderColor,
        ),
        (&controls.window_bg, Field::WindowBackgroundColor),
        (&controls.window_border, Field::WindowBorderColor),
        (
            &controls.active_window_border,
            Field::ActiveWindowBorderColor,
        ),
        (
            &controls.active_window_bg,
            Field::ActiveWindowBackgroundColor,
        ),
    ];

    for (button, field) in buttons {
        let button2 = button.clone();
        let draft2 = Rc::clone(draft);
        let reverts2 = Rc::clone(reverts);
        button.on().bn_clicked(move || {
            let current = {
                let working = draft2.borrow();
                color_value(working.working(), field)
            };
            if let Some(argb) = choose_color(button2.hwnd(), current) {
                draft2.borrow_mut().set_color(field, argb);
                set_color_button(&button2, argb);
                refresh_reverts(&reverts2, &draft2);
            }
            Ok(())
        });
    }
}

fn attach_opacity(controls: &Controls, draft: &Rc<RefCell<SettingsDraft>>, reverts: &Reverts) {
    let track2 = controls.opacity.clone();
    let label2 = controls.opacity_label.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
    controls.opacity.on().wm_h_scroll(move |_| {
        let percent = track2.pos();
        draft2.borrow_mut().set_opacity_percent(percent as f64);
        let _ = label2.set_text_and_resize(&format!("{percent}%"));
        refresh_reverts(&reverts2, &draft2);
        Ok(())
    });
}

fn attach_checkbox(controls: &Controls, draft: &Rc<RefCell<SettingsDraft>>, reverts: &Reverts) {
    let check2 = controls.show_taskbar.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
    controls.show_taskbar.on().bn_clicked(move || {
        draft2
            .borrow_mut()
            .set_check(Field::ShowAppInTaskbar, check2.is_checked());
        refresh_reverts(&reverts2, &draft2);
        Ok(())
    });
}

fn attach_buttons(
    wnd: &gui::WindowModal,
    tabs: &gui::Tab,
    controls: &Controls,
    draft: &Rc<RefCell<SettingsDraft>>,
    reverts: &Reverts,
    result: &Rc<RefCell<Option<Settings>>>,
) {
    let dont_move = (gui::Horz::None, gui::Vert::None);

    let defaults = gui::Button::new_dlg(wnd, ids::IDC_SETTINGS_DEFAULTS, dont_move);
    let tabs2 = tabs.clone();
    let controls2 = controls.clone();
    let draft2 = Rc::clone(draft);
    let reverts2 = Rc::clone(reverts);
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

fn load_all(controls: &Controls, settings: &Settings) {
    controls
        .view_mode
        .items()
        .select(Some(settings.view_mode_default.as_u8() as u32));
    let _ = controls
        .pager_height
        .set_text(&settings.pager_height.to_string());
    let _ = controls
        .icon_pad_x
        .set_text(&settings.icon_padding_x.to_string());
    let _ = controls
        .icon_pad_y
        .set_text(&settings.icon_padding_y.to_string());

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

    set_color_button(&controls.desktop_bg, settings.desktop_background_color);
    set_color_button(&controls.desktop_border, settings.desktop_border_color);
    set_color_button(
        &controls.active_desktop_border,
        settings.active_desktop_border_color,
    );
    set_color_button(&controls.window_bg, settings.window_background_color);
    set_color_button(&controls.window_border, settings.window_border_color);
    set_color_button(
        &controls.active_window_border,
        settings.active_window_border_color,
    );
    set_color_button(
        &controls.active_window_bg,
        settings.active_window_background_color,
    );

    let _ = controls
        .primary_delay
        .set_text(&settings.primary_update_delay.to_string());
    let _ = controls
        .secondary_delay
        .set_text(&settings.secondary_update_delay.to_string());
    controls
        .show_taskbar
        .set_check(settings.show_app_in_taskbar);
}

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

fn set_color_button(button: &gui::Button, argb: u32) {
    let _ = button
        .hwnd()
        .SetWindowText(&format!("#{:06X}", argb & 0x00FF_FFFF));
}

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
