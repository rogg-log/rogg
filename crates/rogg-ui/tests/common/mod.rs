//! Headless test windows. gpui's TestAppContext uses a no-op text system, so text has zero size:
//! assert only on sizes that the reference fixes in CSS (heights, fixed widths, paddings).
#![allow(dead_code)]
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AnyWindowHandle, App, Bounds, Context, Pixels, Point, TestAppContext, Window, WindowBounds, WindowOptions, px,
    size,
};
use rogg_ui::theme::{Appearance, Density, RoggTheme};

pub fn setup(cx: &mut TestAppContext) {
    setup_with(cx, Appearance::Dark, Density::Compact);
}

pub fn setup_with(cx: &mut TestAppContext, appearance: Appearance, density: Density) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        RoggTheme::new(appearance, density).install(cx);
    });
}

/// A view that renders whatever the closure builds; used to host RenderOnce components.
pub struct Host(pub Box<dyn Fn(&mut Window, &mut App) -> AnyElement>);

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        (self.0)(window, cx)
    }
}

pub fn open(cx: &mut TestAppContext, w: f32, h: f32, build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> AnyWindowHandle {
    cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(w), px(h)) })),
            ..Default::default()
        };
        let (handle, _) = gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Host(Box::new(build)))).expect("open window");
        handle
    })
}

pub fn bounds(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) -> Bounds<Pixels> {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.find(id).bounds()
    })
    .unwrap()
}

pub fn exists(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) -> bool {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.try_find(id).is_some()
    })
    .unwrap()
}

pub fn click(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}
