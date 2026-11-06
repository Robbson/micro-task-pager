//! Reactor About window reference.
//!
//! Run from `rust/` with `cargo run --example reactor_about`. This is the visual and
//! structural reference for the target About window; Phase 10 promotes it to
//! `src/dialogs/about.rs`. It requires the Windows App Runtime 2.x framework package.

use windows_reactor::*;

const LOGO: &[u8] = include_bytes!("../../src/Resources/logo.png");

#[derive(Clone, Copy)]
enum Message {
    Close,
}

struct About;

impl Component for About {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), _context: &ComponentContext<Self>) -> Self {
        Self
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        if matches!(message, Message::Close) {
            let _ = context.window().request_close();
        }
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        context.window_title("About Micro Task Pager");
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

fn main() {
    App::run_component::<About>(()).unwrap();
}
