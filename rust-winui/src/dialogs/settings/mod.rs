//! Reactor Settings window.
//!
//! The dialog is a secondary WinUI window with the C# dialog's Layout, Theme,
//! and Misc sections. All edits target a working copy; the pager receives live
//! previews and only persists settings when OK is pressed.

mod draft;
mod view;

use std::time::Duration;

use windows_reactor::{Color, Component, ComponentContext, ContentDialogResult, View, ViewContext};

use super::{DialogEvent, DialogLink};
use crate::settings::Settings;
use draft::{Field, Section, SettingsDraft};

/// Window title used by the pager to reassert topmost for the open dialog.
pub const WINDOW_TITLE: &str = "Micro Task Pager Settings";

const PREVIEW_DELAY_MS: u64 = 80;
const CLIENT_SIZE: (f64, f64) = (640.0, 720.0);

/// Parent-supplied Settings window input.
#[derive(Clone, PartialEq)]
pub struct SettingsInput {
    initial: Settings,
    link: Option<DialogLink>,
}

impl SettingsInput {
    /// Input for the pager-owned dialog.
    pub fn linked(initial: Settings, link: DialogLink) -> Self {
        Self {
            initial,
            link: Some(link),
        }
    }

    /// Input for the standalone example; previews and persistence are no-ops.
    pub fn standalone(initial: Settings) -> Self {
        Self {
            initial,
            link: None,
        }
    }
}

/// A color currently being edited in the modal color picker.
struct ColorEdit {
    field: Field,
    color: Color,
}

/// Public message type; the variants stay private to this module tree.
#[derive(Clone)]
pub struct Message(MessageKind);

#[derive(Clone)]
enum MessageKind {
    SelectSection(usize),
    Number(Field, f64),
    Step(Field, f64),
    Opacity(f64),
    Combo(Field, Option<usize>),
    Check(Field, bool),
    OpenColor(Field),
    ColorChanged(Color),
    ColorDialogClosed(ContentDialogResult),
    Revert(Field),
    ResetSection,
    Ok,
    Cancel,
    Preview,
    Noop,
}

/// The Settings window component.
pub struct SettingsDialog {
    draft: SettingsDraft,
    link: Option<DialogLink>,
    section: Section,
    color_edit: Option<ColorEdit>,
    preview_scheduled: bool,
    preview_dirty: bool,
    finished: bool,
}

impl Component for SettingsDialog {
    type Input = SettingsInput;
    type Message = Message;

    fn create(input: &SettingsInput, _context: &ComponentContext<Self>) -> Self {
        Self {
            draft: SettingsDraft::new(input.initial.clone()),
            link: input.link.clone(),
            section: Section::Layout,
            color_edit: None,
            preview_scheduled: false,
            preview_dirty: false,
            finished: false,
        }
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message.0 {
            MessageKind::SelectSection(index) => self.section = Section::from_index(index),
            MessageKind::Number(field, value) => {
                self.draft.set_number(field, value);
                self.mark_changed(context);
            }
            MessageKind::Step(field, delta) => {
                let value = number_value(self.draft.working(), field) + delta;
                self.draft.set_number(field, value);
                self.mark_changed(context);
            }
            MessageKind::Opacity(percent) => {
                self.draft.set_opacity_percent(percent);
                self.mark_changed(context);
            }
            MessageKind::Combo(field, index) => {
                if let Some(index) = index {
                    self.draft.set_index(field, index);
                    self.mark_changed(context);
                }
            }
            MessageKind::Check(field, value) => {
                self.draft.set_check(field, value);
                self.mark_changed(context);
            }
            MessageKind::OpenColor(field) => {
                self.color_edit = Some(ColorEdit {
                    field,
                    color: color_from_argb(field_value(self.draft.working(), field)),
                });
            }
            MessageKind::ColorChanged(color) => {
                if let Some(edit) = &mut self.color_edit {
                    edit.color = color;
                }
            }
            MessageKind::ColorDialogClosed(result) => {
                if let Some(edit) = self.color_edit.take()
                    && result == ContentDialogResult::Primary
                {
                    self.draft
                        .set_color(edit.field, argb_from_color(edit.color));
                    self.mark_changed(context);
                }
            }
            MessageKind::Revert(field) => {
                self.draft.revert(field);
                self.mark_changed(context);
            }
            MessageKind::ResetSection => {
                self.draft.reset_section(self.section);
                self.mark_changed(context);
            }
            MessageKind::Ok => {
                if let Some(link) = &self.link {
                    link.send(DialogEvent::Applied(self.draft.working().clone()));
                }
                self.finished = true;
                let _ = context.window().request_close();
            }
            MessageKind::Cancel => {
                if let Some(link) = &self.link {
                    link.send(DialogEvent::Cancelled(self.draft.original().clone()));
                }
                self.finished = true;
                let _ = context.window().request_close();
            }
            MessageKind::Preview => {
                self.preview_scheduled = false;
                if self.preview_dirty && !self.finished {
                    self.preview_dirty = false;
                    if let Some(link) = &self.link {
                        link.send(DialogEvent::Preview(self.draft.working().clone()));
                    }
                }
            }
            MessageKind::Noop => {}
        }
    }

    fn view(&self, _input: &SettingsInput, context: &mut ViewContext<Self>) -> View {
        view::build(self, context)
    }
}

impl SettingsDialog {
    /// Coalesces rapid edits (held NumberBox arrows, slider drags) the same way
    /// the C# dialog does, so the pager is not rebuilt for every increment.
    fn mark_changed(&mut self, context: &ComponentContext<Self>) {
        if self.finished {
            return;
        }

        self.preview_dirty = true;
        if self.preview_scheduled {
            return;
        }

        self.preview_scheduled = true;
        let _ = context.spawn_background(|_| {
            std::thread::sleep(Duration::from_millis(PREVIEW_DELAY_MS));
            Message(MessageKind::Preview)
        });
    }
}

impl Drop for SettingsDialog {
    fn drop(&mut self) {
        if let Some(link) = &self.link {
            if !self.finished {
                link.send(DialogEvent::Cancelled(self.draft.original().clone()));
            }
            link.send(DialogEvent::Closed);
        }
    }
}

/// The current numeric value for a numeric field.
fn number_value(settings: &Settings, field: Field) -> f64 {
    match field {
        Field::PagerHeight => f64::from(settings.pager_height),
        Field::IconPaddingX => f64::from(settings.icon_padding_x),
        Field::IconPaddingY => f64::from(settings.icon_padding_y),
        Field::PrimaryUpdateDelay => f64::from(settings.primary_update_delay),
        Field::SecondaryUpdateDelay => f64::from(settings.secondary_update_delay),
        _ => 0.0,
    }
}

/// The color currently stored for a color field.
fn field_value(settings: &Settings, field: Field) -> u32 {
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

pub(super) fn color_from_argb(argb: u32) -> Color {
    Color::argb(
        (argb >> 24) as u8,
        (argb >> 16) as u8,
        (argb >> 8) as u8,
        argb as u8,
    )
}

fn argb_from_color(color: Color) -> u32 {
    (u32::from(color.a) << 24)
        | (u32::from(color.r) << 16)
        | (u32::from(color.g) << 8)
        | u32::from(color.b)
}

pub(super) fn client_size() -> (f64, f64) {
    CLIENT_SIZE
}
