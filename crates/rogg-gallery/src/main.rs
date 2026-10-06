use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, Context, QuitMode, Window, WindowBounds, WindowOptions, div, px, rgb, size};
use std::time::Duration;

struct Hello;

impl Render for Hello {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x191c1f))
            .text_color(rgb(0xe2e6e9))
            .child(format!("rogg gallery {}", rogg_ui::version()))
    }
}

fn main() {
    gpui_kit::application()
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            gpui_kit::open_window(
                WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
                cx,
                |_, cx| cx.new(|_| Hello),
            )
            .expect("open gallery window");
            cx.activate(true);
            if std::env::var_os("ROGG_GALLERY_SMOKE").is_some() {
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(Duration::from_millis(1500)).await;
                    println!("smoke ok");
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
        });
}
