//! Declarative view for the Settings window.
//!
//! Rows are built from standard WinUI controls: ComboBox, NumberBox, Slider,
//! CheckBox, and a modal ColorPicker. Each row exposes a small revert button
//! while its option differs from the value the dialog opened with.

use windows_reactor::{
    Border, Brush, Button, ButtonStyle, CheckBox, ChildrenControl, ColorPicker, ComboBox,
    ContentControl, ContentDialog, FontWeight, Grid, GridChildExt, GridLength, HorizontalAlignment,
    InfoBar, InfoBarSeverity, KeyedView, LayoutControl, NumberBox, Orientation, Pivot, PivotItem,
    PivotSlot, ScrollBarVisibility, ScrollViewer, Slider, SlotsControl, StackPanel, Stretch,
    Symbol, SymbolIcon, TextBlock, ThemeBrush, Thickness, TooltipExt, VerticalAlignment, View,
    ViewContext, Viewbox, ViewboxSlot, WindowTheme, WindowVisuals,
};

use super::draft::{Field, MAX_DELAY, MAX_ICON_PADDING, MAX_PAGER_HEIGHT};
use super::{
    Message, MessageKind, SettingsDialog, WINDOW_TITLE, client_size, color_from_argb, field_value,
};
use crate::settings::{DesktopBorderStyle, ViewMode};

const LABEL_WIDTH: f64 = 230.0;
const REVERT_WIDTH: f64 = 40.0;
const EDITOR_WIDTH: f64 = 180.0;

/// Builds the complete Settings window view.
pub(super) fn build(dialog: &SettingsDialog, context: &mut ViewContext<SettingsDialog>) -> View {
    context.window_title(WINDOW_TITLE);
    context.window_visuals(
        WindowVisuals::new()
            .client_size(client_size().0, client_size().1)
            .theme(WindowTheme::System),
    );

    let pivot = Pivot::new()
        .selected_index(Some(dialog.section.index()))
        .on_selection_changed(context.callback(|index: Option<usize>| {
            Message(MessageKind::SelectSection(index.unwrap_or(0)))
        }))
        .collection_slot(
            PivotSlot::Items,
            [
                KeyedView::new(
                    "layout",
                    PivotItem::new()
                        .header("Layout")
                        .content(layout_page(dialog, context)),
                ),
                KeyedView::new(
                    "theme",
                    PivotItem::new()
                        .header("Theme")
                        .content(theme_page(dialog, context)),
                ),
                KeyedView::new(
                    "misc",
                    PivotItem::new()
                        .header("Misc")
                        .content(misc_page(dialog, context)),
                ),
            ],
        );

    let mut children = vec![
        KeyedView::new("pivot", Border::new().grid_row(0).content(pivot)),
        KeyedView::new("footer", Border::new().grid_row(1).content(footer(context))),
    ];

    if dialog.color_edit.is_some() {
        children.push(KeyedView::new(
            "color-dialog",
            color_dialog(dialog, context),
        ));
    }

    Grid::new()
        .background(Brush::Theme(ThemeBrush::SolidBackground))
        .rows([GridLength::STAR, GridLength::Auto])
        .keyed_children(children)
}

fn layout_page(dialog: &SettingsDialog, context: &mut ViewContext<SettingsDialog>) -> View {
    let working = dialog.draft.working();
    let mode_index = match working.view_mode_default {
        ViewMode::Preview => 0,
        ViewMode::MicroTaskbar => 1,
    };

    let pager_height = number_box(
        context,
        Field::PagerHeight,
        f64::from(dialog.draft.min_pager_height()),
        f64::from(MAX_PAGER_HEIGHT),
        f64::from(working.pager_height),
    );
    let padding_x = number_box(
        context,
        Field::IconPaddingX,
        f64::from(-MAX_ICON_PADDING),
        f64::from(MAX_ICON_PADDING),
        f64::from(working.icon_padding_x),
    );

    // TODO: Why those can't be inlined? Then there is a rust borrow error for context
    let padding_y = number_box(
        context,
        Field::IconPaddingY,
        f64::from(-MAX_ICON_PADDING),
        f64::from(MAX_ICON_PADDING),
        f64::from(working.icon_padding_y),
    );

    let rendering = vec![
        row(
            dialog,
            context,
            "View Mode Default",
            Field::ViewModeDefault,
            placed(
                ComboBox::new()
                    .items_source(["Preview", "Micro Taskbar"])
                    .selected_index(Some(mode_index))
                    .width(EDITOR_WIDTH)
                    .on_selection_changed(context.callback(|index: Option<usize>| {
                        Message(MessageKind::Combo(Field::ViewModeDefault, index))
                    })),
            ),
        ),
        row(
            dialog,
            context,
            "Pager Height",
            Field::PagerHeight,
            pager_height,
        ),
    ];

    let icons = vec![
        row(
            dialog,
            context,
            "Icon Padding X",
            Field::IconPaddingX,
            padding_x,
        ),
        row(
            dialog,
            context,
            "Icon Padding Y",
            Field::IconPaddingY,
            padding_y,
        ),
    ];

    page(vec![
        section("Rendering", rendering),
        section("Icons Settings", icons),
    ])
}

fn theme_page(dialog: &SettingsDialog, context: &mut ViewContext<SettingsDialog>) -> View {
    let working = dialog.draft.working();
    let percent = working.background_opacity * 100.0;

    let opacity_slider = Slider::new()
        .minimum(25.0)
        .maximum(100.0)
        .step_frequency(1.0)
        .value(Some(percent))
        .width(EDITOR_WIDTH)
        .on_value_changed(context.callback(|percent| Message(MessageKind::Opacity(percent))));
    let opacity_label = TextBlock::new()
        .text(format!("{percent:.0}%"))
        .width(48.0)
        .vertical_alignment(VerticalAlignment::Center);
    let opacity = StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(8.0)
        .grid_column(1)
        .horizontal_alignment(HorizontalAlignment::Left)
        .children((opacity_slider, opacity_label));

    let border_style_index = match working.desktop_border_style {
        DesktopBorderStyle::Box => 0,
        DesktopBorderStyle::Underline => 1,
    };
    let border_style = ComboBox::new()
        .items_source(["Box", "Underline"])
        .selected_index(Some(border_style_index))
        .width(EDITOR_WIDTH)
        .on_selection_changed(context.callback(|index: Option<usize>| {
            Message(MessageKind::Combo(Field::DesktopBorderStyle, index))
        }));

    let general = vec![
        row(
            dialog,
            context,
            "Background Opacity",
            Field::BackgroundOpacity,
            opacity,
        ),
        row(
            dialog,
            context,
            "Desktop Border Style",
            Field::DesktopBorderStyle,
            placed(border_style),
        ),
    ];

    let desktops = vec![
        color_row(
            dialog,
            context,
            "Desktop Background Color",
            Field::DesktopBackgroundColor,
        ),
        color_row(
            dialog,
            context,
            "Desktop Border Color",
            Field::DesktopBorderColor,
        ),
        color_row(
            dialog,
            context,
            "Active Desktop Border Color",
            Field::ActiveDesktopBorderColor,
        ),
    ];

    let windows = vec![
        color_row(
            dialog,
            context,
            "Window Background Color",
            Field::WindowBackgroundColor,
        ),
        color_row(
            dialog,
            context,
            "Window Border Color",
            Field::WindowBorderColor,
        ),
        color_row(
            dialog,
            context,
            "Active Window Border Color",
            Field::ActiveWindowBorderColor,
        ),
        color_row(
            dialog,
            context,
            "Active Window Background Color",
            Field::ActiveWindowBackgroundColor,
        ),
    ];

    page(vec![
        section("General", general),
        section("Desktops", desktops),
        section("Preview Windows", windows),
    ])
}

fn misc_page(dialog: &SettingsDialog, context: &mut ViewContext<SettingsDialog>) -> View {
    let working = dialog.draft.working();

    let primary = number_box(
        context,
        Field::PrimaryUpdateDelay,
        1.0,
        f64::from(MAX_DELAY),
        f64::from(working.primary_update_delay),
    );
    let secondary = number_box(
        context,
        Field::SecondaryUpdateDelay,
        100.0,
        f64::from(MAX_DELAY),
        f64::from(working.secondary_update_delay),
    );

    let performance = vec![
        row(
            dialog,
            context,
            "Primary Update Delay (ms)",
            Field::PrimaryUpdateDelay,
            primary,
        ),
        row(
            dialog,
            context,
            "Secondary Update Delay (ms)",
            Field::SecondaryUpdateDelay,
            secondary,
        ),
    ];

    let show_in_taskbar = CheckBox::new()
        .is_checked(Some(working.show_app_in_taskbar))
        .on_is_checked_changed(context.callback(|checked: bool| {
            Message(MessageKind::Check(Field::ShowAppInTaskbar, checked))
        }));

    let pinning = vec![
        row(
            dialog,
            context,
            "Show App In Taskbar",
            Field::ShowAppInTaskbar,
            placed(show_in_taskbar),
        ),
        KeyedView::new(
            "pin-note",
            InfoBar::new()
                .title("App Pinning")
                .message("The pager is pinned to all virtual desktops when this option is enabled.")
                .severity(Some(InfoBarSeverity::Warning))
                .is_open(Some(true))
                .is_closable(Some(false))
                .margin(Thickness::new(0.0, 4.0, 0.0, 0.0)),
        ),
    ];

    page(vec![
        section("Performance", performance),
        section("App Pinning", pinning),
    ])
}

fn page(sections: Vec<KeyedView>) -> View {
    let content = StackPanel::new()
        .spacing(8.0)
        .margin(Thickness::new(8.0, 8.0, 8.0, 16.0))
        .keyed_children(sections);
    ScrollViewer::new()
        .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
        .content(content)
}

fn section(title: &str, rows: Vec<KeyedView>) -> KeyedView {
    let header = TextBlock::new()
        .text(title.to_owned())
        .font_weight(FontWeight::SEMI_BOLD)
        .margin(Thickness::new(2.0, 8.0, 0.0, 4.0));
    let body = StackPanel::new().spacing(8.0).keyed_children(rows);
    let card = Border::new()
        .background(Brush::Theme(ThemeBrush::CardBackground))
        .border_brush(Brush::Theme(ThemeBrush::CardStroke))
        .border_thickness(1.0)
        .corner_radius(4.0)
        .padding(Thickness::new(12.0, 12.0, 12.0, 12.0))
        .content(body);

    KeyedView::new(
        format!("section:{title}"),
        StackPanel::new().spacing(2.0).children((header, card)),
    )
}

/// Applies the shared editor column placement before the control is erased.
fn placed<E>(editor: E) -> View
where
    E: LayoutControl + Into<View> + 'static,
{
    editor
        .grid_column(1)
        .horizontal_alignment(HorizontalAlignment::Left)
        .into()
}

fn row(
    dialog: &SettingsDialog,
    context: &mut ViewContext<SettingsDialog>,
    label: &str,
    field: Field,
    editor: View,
) -> KeyedView {
    let label = TextBlock::new()
        .text(label.to_owned())
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(0);

    let mut children = vec![
        KeyedView::new("label", label),
        KeyedView::new("editor", editor),
    ];

    if dialog.draft.is_modified(field) {
        let icon = Viewbox::new()
            .stretch(Stretch::Uniform)
            .width(18.0)
            .height(18.0)
            .slot(ViewboxSlot::Child, SymbolIcon::new().symbol(Symbol::Undo));
        let revert = Button::new()
            .width(40.0)
            .height(32.0)
            .on_click(context.message(Message(MessageKind::Revert(field))))
            .grid_column(2)
            .content(icon)
            .tooltip("Revert to the value the dialog opened with");
        children.push(KeyedView::new("revert", revert));
    }

    let grid = Grid::new()
        .columns([
            GridLength::Pixel(LABEL_WIDTH),
            GridLength::STAR,
            GridLength::Pixel(REVERT_WIDTH),
        ])
        .column_spacing(8.0)
        .keyed_children(children);

    KeyedView::new(format!("row:{field:?}"), grid)
}

fn number_box(
    context: &mut ViewContext<SettingsDialog>,
    field: Field,
    minimum: f64,
    maximum: f64,
    value: f64,
) -> View {
    let value_box = NumberBox::new()
        .minimum(Some(minimum))
        .maximum(Some(maximum))
        .value(Some(value))
        .width(EDITOR_WIDTH)
        .on_value_changed(context.callback(move |value: Option<f64>| match value {
            Some(value) => Message(MessageKind::Number(field, value)),
            None => Message(MessageKind::Noop),
        }));

    // Reactor 0.100 does not expose NumberBox's native spin buttons, so use
    // standard buttons for discoverable single-step changes. The NumberBox
    // still accepts typed values and its own arrow keys.
    let decrease = Button::new()
        .is_enabled(Some(value > minimum))
        .on_click(context.message(Message(MessageKind::Step(field, -1.0))))
        .content(SymbolIcon::new().symbol(Symbol::Remove))
        .tooltip("Decrease by one");
    let increase = Button::new()
        .is_enabled(Some(value < maximum))
        .on_click(context.message(Message(MessageKind::Step(field, 1.0))))
        .content(SymbolIcon::new().symbol(Symbol::Add))
        .tooltip("Increase by one");

    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(6.0)
        .grid_column(1)
        .horizontal_alignment(HorizontalAlignment::Left)
        .children((value_box, decrease, increase))
}

fn color_row(
    dialog: &SettingsDialog,
    context: &mut ViewContext<SettingsDialog>,
    label: &str,
    field: Field,
) -> KeyedView {
    let argb = field_value(dialog.draft.working(), field);
    let preview = Border::new()
        .width(44.0)
        .height(24.0)
        .corner_radius(3.0)
        .border_thickness(1.0)
        .border_brush(Brush::Theme(ThemeBrush::CardStroke))
        .background(color_from_argb(argb));
    let choose = Button::new()
        .on_click(context.message(Message(MessageKind::OpenColor(field))))
        .content("Choose…");
    let editor = StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(8.0)
        .grid_column(1)
        .horizontal_alignment(HorizontalAlignment::Left)
        .children((preview, choose));

    row(dialog, context, label, field, editor)
}

fn footer(context: &mut ViewContext<SettingsDialog>) -> View {
    let defaults = Button::new()
        .on_click(context.message(Message(MessageKind::ResetSection)))
        .content("Defaults");
    let hint = TextBlock::new()
        .text("Reset the current section")
        .font_size(12.0)
        .vertical_alignment(VerticalAlignment::Center);
    let cancel = Button::new()
        .on_click(context.message(Message(MessageKind::Cancel)))
        .content("Cancel");
    let ok = Button::new()
        .style(ButtonStyle::Accent)
        .on_click(context.message(Message(MessageKind::Ok)))
        .content("OK");

    let left = StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(8.0)
        .grid_column(0)
        .children((defaults, hint));
    let right = StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(8.0)
        .grid_column(1)
        .children((cancel, ok));

    let buttons = Grid::new()
        .columns([GridLength::STAR, GridLength::Auto])
        .children((left, right));

    Border::new()
        .border_thickness(Thickness::new(0.0, 1.0, 0.0, 0.0))
        .border_brush(Brush::Theme(ThemeBrush::CardStroke))
        .padding(Thickness::new(16.0, 10.0, 16.0, 10.0))
        .content(buttons)
}

fn color_dialog(dialog: &SettingsDialog, context: &mut ViewContext<SettingsDialog>) -> View {
    let Some(edit) = &dialog.color_edit else {
        return View::empty();
    };

    let picker = ColorPicker::new()
        .color(edit.color)
        .is_alpha_enabled(Some(false))
        .on_color_changed(context.callback(|color| Message(MessageKind::ColorChanged(color))));

    ContentDialog::new()
        .title("Choose Color")
        .primary_button_text("OK")
        .close_button_text("Cancel")
        .is_open(true)
        .on_closed(context.callback(|result| Message(MessageKind::ColorDialogClosed(result))))
        .content(picker)
}
