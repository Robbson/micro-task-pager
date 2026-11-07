//! Reactor About window.
//!
//! This is the promoted `examples/reactor_about.rs` reference. The pager opens
//! it as a secondary window; the example runs it standalone with no link.

use windows_reactor::*;

use super::{DialogEvent, DialogLink};

/// Window title used by the pager to reassert topmost for the open dialog.
pub const WINDOW_TITLE: &str = "About Micro Task Pager";

const LOGO: &[u8] = include_bytes!("../../../src/Resources/logo.png");

/// Parent-supplied About window input.
#[derive(Clone, PartialEq)]
pub struct AboutInput {
    link: Option<DialogLink>,
}

impl AboutInput {
    /// Input for the standalone example.
    pub fn standalone() -> Self {
        Self { link: None }
    }

    /// Input for a dialog opened by the pager.
    pub fn linked(link: DialogLink) -> Self {
        Self { link: Some(link) }
    }
}

#[derive(Clone, Copy)]
pub enum Message {
    Close,
}

/// The About window component.
pub struct About {
    link: Option<DialogLink>,
}

impl Component for About {
    type Input = AboutInput;
    type Message = Message;

    fn create(input: &AboutInput, _context: &ComponentContext<Self>) -> Self {
        Self {
            link: input.link.clone(),
        }
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        if matches!(message, Message::Close) {
            let _ = context.window().request_close();
        }
    }

    fn view(&self, _input: &AboutInput, context: &mut ViewContext<Self>) -> View {
        context.window_title(WINDOW_TITLE);
        context.window_visuals(
            WindowVisuals::new()
                .client_size(600.0, 480.0)
                .theme(WindowTheme::Light),
        );

        Border::new().background(Color::rgb(255, 255, 255)).content(
            StackPanel::new().children((
                Image::new()
                    .source_data(EncodedImage::from_static(LOGO))
                    .width(512.0)
                    .height(278.0)
                    .margin(Thickness::new(44.0, 20.0, 44.0, 2.0)),
                TextBlock::new()
                    .text(format!("Version {}", env!("CARGO_PKG_VERSION")))
                    .font_size(13.3)
                    .foreground(Color::rgb(105, 105, 105))
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .margin(Thickness::new(0.0, 0.0, 0.0, 8.0)),
                TextBlock::new()
                    .text("Author: Robbson 2026")
                    .font_size(12.0)
                    .font_weight(FontWeight::BOLD)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .margin(Thickness::new(0.0, 0.0, 0.0, 8.0)),
                StackPanel::new()
                    .spacing(2.0)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .children((
                        TextBlock::new().text("Acknowledgments:").font_size(12.0),
                        TextBlock::new()
                            .text(" • DARKGuy (original project)")
                            .font_size(12.0),
                        TextBlock::new()
                            .text(" • MScholtes (Virtual Desktop)")
                            .font_size(12.0),
                    )),
                Button::new()
                    .on_click(context.message(Message::Close))
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .margin(Thickness::new(0.0, 18.0, 0.0, 0.0))
                    .content("Close"),
            )),
        )
    }
}

impl Drop for About {
    fn drop(&mut self) {
        if let Some(link) = &self.link {
            link.send(DialogEvent::Closed);
        }
    }
}
