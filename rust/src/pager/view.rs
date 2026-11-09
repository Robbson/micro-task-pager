//! Declarative reactor view for the pager.
//!
//! This replaces the C# `ScreenRenderer.Render`/`Graphics` drawing. Instead of
//! painting pixels, it returns a tree of WinUI elements that describe the same
//! geometry:
//!
//! | C# drawing | Reactor view |
//! | --- | --- |
//! | `Desktop`/`ScreenRenderer` bounds | `Canvas` + `Rect` from `layout` |
//! | `DrawImage(wnd.Icon, ...)` | `Image` with an `EncodedImage` from `icons` |
//! | `FillRectangle` for the active cell | `Border` with the active background |
//! | `DrawString("+N")` | `TextBlock` from `layout.overflow` |
//!
//! The view is built from the cached snapshot layout; it never calls COM or
//! enumerates windows. Each desktop/screen/cell is a keyed child so window
//! identity survives snapshot refreshes.
//!
//! Reactor may rebuild the view on every `update`. Only cheap element
//! construction belongs here; snapshotting, icon encoding, and layout are
//! prepared beforehand in `Pager`, which is why this module only reads fields.
//!
//! The context menu is a native `TrackPopupMenu` in `native_window`; reactor
//! 0.100 `Menu` is a Button/SplitButton flyout and cannot serve as a right-click
//! menu. This is a documented temporary deviation until reactor exposes
//! transient menus.

use windows_reactor::{
    Border, Button, Canvas, CanvasChildExt, ChildrenControl, Color, ContentControl, Grid,
    HorizontalAlignment, Image, KeyedView, LayoutControl, PointerEventInfo, StackPanel, Stretch,
    TextBlock, Thickness, VerticalAlignment, View, ViewContext, WindowTheme, WindowVisuals,
};

use super::layout::{
    ICON_SIZE, OVERFLOW_FONT_SIZE, PagerLayout, Rect, ScreenLayout, desktop_border_color,
    desktop_border_style,
};
use super::{Message, Pager, WINDOW_TITLE};
use crate::native_window;
use crate::settings::{DesktopBorderStyle, Settings, ViewMode};
use crate::windows::WindowSnapshot;

/// Builds the complete pager view.
pub(super) fn build(pager: &Pager, context: &mut ViewContext<Pager>) -> View {
    context.window_title(WINDOW_TITLE);
    context.window_visuals(
        WindowVisuals::new()
            .client_size(pager.client_size.0, pager.client_size.1)
            .theme(WindowTheme::Dark),
    );

    if let Some(error) = &pager.startup_error {
        return error_view(pager, context, error);
    }

    let mut children = Vec::new();
    for desktop in &pager.layout.desktops {
        for (screen_position, screen) in desktop.screens.iter().enumerate() {
            build_screen(pager, desktop.index, screen, &mut children);
            if screen_position + 1 < desktop.screens.len() {
                children.push(KeyedView::new(
                    format!("separator:{}:{}", desktop.index, screen.index),
                    separator(
                        screen.x + screen.width,
                        pager.layout.height,
                        pager.layout.border,
                    ),
                ));
            }
        }
        children.push(KeyedView::new(
            format!("desktop-border:{}", desktop.index),
            desktop_border(
                desktop.index,
                &pager.layout,
                &pager.snapshot,
                &pager.settings,
            ),
        ));
    }

    if let Some(ghost) = pager.input.ghost() {
        children.push(KeyedView::new("drag-ghost", ghost_view(pager, ghost)));
    }

    let canvas = Canvas::new()
        .width(pager.layout.width)
        .height(pager.layout.height)
        .keyed_children(children);

    Border::new()
        .width(pager.layout.width)
        .height(pager.layout.height)
        .background(color(pager.settings.desktop_background_color))
        .capture_pointer_on_press(true)
        .on_pointer_pressed(
            context.callback(|info: PointerEventInfo| Message::PointerPressed {
                x: info.window_x,
                y: info.window_y,
                left: info.is_left_button_pressed,
                middle: info.is_middle_button_pressed,
                right: info.is_right_button_pressed,
                shift: native_window::shift_pressed(),
            }),
        )
        .on_pointer_moved(
            context.callback(|info: PointerEventInfo| Message::PointerMoved {
                x: info.window_x,
                y: info.window_y,
            }),
        )
        .on_pointer_released(
            context.callback(|info: PointerEventInfo| Message::PointerReleased {
                x: info.window_x,
                y: info.window_y,
            }),
        )
        .on_pointer_capture_lost(context.callback(|_| Message::PointerCaptureLost))
        .content(canvas)
}

fn build_screen(
    pager: &Pager,
    desktop_index: u32,
    screen: &ScreenLayout,
    children: &mut Vec<KeyedView>,
) {
    for cell in &screen.cells {
        let key = format!(
            "cell:{desktop_index}:{}:{}",
            screen.index, cell.hwnd.0 as usize
        );
        let view = match screen.view_mode {
            ViewMode::Preview => preview_cell(pager, cell),
            ViewMode::MicroTaskbar => micro_cell(pager, cell),
        };
        children.push(KeyedView::new(key, view));

        if cell.icon.width() > 0.0
            && let Some(image) = pager.icons.get(cell.hwnd.0 as usize)
        {
            children.push(KeyedView::new(
                format!(
                    "icon:{desktop_index}:{}:{}",
                    screen.index, cell.hwnd.0 as usize
                ),
                icon_view(image.clone(), cell.icon),
            ));
        }
    }

    if let Some(overflow) = &screen.overflow {
        children.push(KeyedView::new(
            format!("overflow:{desktop_index}:{}", screen.index),
            overflow_view(overflow, &pager.settings),
        ));
    }
}

fn preview_cell(pager: &Pager, cell: &super::layout::CellLayout) -> View {
    let fill = if cell.is_active {
        pager.settings.active_window_background_color
    } else {
        pager.settings.window_background_color
    };
    let border = if cell.is_active {
        pager.settings.active_window_border_color
    } else {
        pager.settings.window_border_color
    };

    Border::new()
        .width(cell.rect.width())
        .height(cell.rect.height())
        .canvas_left(cell.rect.left)
        .canvas_top(cell.rect.top)
        .background(color(fill))
        .border_brush(color(border))
        .border_thickness(pager.layout.border)
        .into()
}

fn micro_cell(pager: &Pager, cell: &super::layout::CellLayout) -> View {
    let tile = if cell.is_active {
        Border::new()
            .width(cell.rect.width())
            .height(cell.rect.height())
            .canvas_left(cell.rect.left)
            .canvas_top(cell.rect.top)
            .background(color(pager.settings.active_window_background_color))
    } else {
        Border::new()
            .width(cell.rect.width())
            .height(cell.rect.height())
            .canvas_left(cell.rect.left)
            .canvas_top(cell.rect.top)
    };

    tile.into()
}

fn overflow_view(overflow: &super::layout::OverflowLayout, settings: &Settings) -> View {
    Border::new()
        .width(overflow.rect.width())
        .height(overflow.rect.height())
        .canvas_left(overflow.rect.left)
        .canvas_top(overflow.rect.top)
        .content(
            TextBlock::new()
                .text(overflow.text.clone())
                .font_size(OVERFLOW_FONT_SIZE)
                .foreground(color(settings.active_window_border_color))
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
}

fn ghost_view(pager: &Pager, ghost: super::input::Ghost) -> View {
    let size = ICON_SIZE;
    let left = ghost.x - size / 2.0;
    let top = ghost.y - size / 2.0;

    match pager.icons.get(ghost.hwnd.0 as usize) {
        Some(image) => View::from(
            Image::new()
                .source_data(image.clone())
                .width(size)
                .height(size)
                .stretch(Stretch::Uniform)
                .opacity(0.7)
                .canvas_left(left)
                .canvas_top(top),
        ),
        None => View::from(
            Border::new()
                .width(size)
                .height(size)
                .opacity(0.5)
                .background(color(pager.settings.active_window_border_color))
                .canvas_left(left)
                .canvas_top(top),
        ),
    }
}

fn separator(x: f64, height: f64, border: f64) -> View {
    Border::new()
        .width(border)
        .height(height)
        .canvas_left(x)
        .canvas_top(0.0)
        .background(Color::argb(64, 173, 216, 230))
        .into()
}

fn desktop_border(
    desktop_index: u32,
    layout: &PagerLayout,
    snapshot: &WindowSnapshot,
    settings: &Settings,
) -> View {
    let border_color = desktop_border_color(snapshot, desktop_index, settings);
    let desktop_x = layout.desktop_width * f64::from(desktop_index);

    match desktop_border_style(settings) {
        DesktopBorderStyle::Box => Border::new()
            .width(layout.desktop_width)
            .height(layout.height)
            .canvas_left(desktop_x)
            .canvas_top(0.0)
            .border_brush(color(border_color))
            .border_thickness(layout.border)
            .into(),
        DesktopBorderStyle::Underline => Border::new()
            .width(layout.desktop_width)
            .height(layout.border)
            .canvas_left(desktop_x)
            .canvas_top(layout.height - layout.border)
            .background(color(border_color))
            .into(),
    }
}

fn icon_view(image: windows_reactor::EncodedImage, rect: Rect) -> View {
    Image::new()
        .source_data(image)
        .width(rect.width())
        .height(rect.height())
        .stretch(Stretch::Uniform)
        .canvas_left(rect.left)
        .canvas_top(rect.top)
        .into()
}

fn error_view(pager: &Pager, context: &mut ViewContext<Pager>, error: &str) -> View {
    Grid::new()
        .background(color(pager.settings.desktop_background_color))
        .children((StackPanel::new()
            .spacing(10.0)
            .margin(Thickness::uniform(16.0))
            .children((
                TextBlock::new()
                    .text("Micro Task Pager could not start.")
                    .font_size(16.0)
                    .foreground(Color::rgb(255, 255, 255)),
                TextBlock::new()
                    .text(error.to_owned())
                    .foreground(Color::rgb(220, 220, 220)),
                Button::new()
                    .on_click(context.message(Message::Close))
                    .content("Close"),
            )),))
}

fn color(argb: u32) -> Color {
    Color::argb(
        (argb >> 24) as u8,
        (argb >> 16) as u8,
        (argb >> 8) as u8,
        argb as u8,
    )
}
