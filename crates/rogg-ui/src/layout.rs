use crate::theme::ActiveRoggTheme;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, TestSupportExt, Window, div, px, relative};

pub struct MainWindowParts {
    pub titlebar: AnyElement,
    pub sidebar: Option<AnyElement>,
    pub info_bar: AnyElement,
    pub top_header: AnyElement,
    pub top_body: AnyElement,
    pub band: AnyElement,
    pub bottom_header: AnyElement,
    pub bottom_body: AnyElement,
    pub status: AnyElement,
    pub overlay: Option<AnyElement>,
    /// Top pane share of the height left after fixed rows (reference `flex:1.1` vs `flex:1`).
    pub split: f32,
}

impl MainWindowParts {
    pub const DEFAULT_SPLIT: f32 = 1.1 / 2.1;
}

/// Composes the window exactly like the reference `.app`: title bar, then sidebar + main column.
#[derive(IntoElement)]
pub struct MainWindowLayout {
    parts: MainWindowParts,
}

impl MainWindowLayout {
    pub fn new(parts: MainWindowParts) -> Self {
        Self { parts }
    }
}

impl RenderOnce for MainWindowLayout {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        let parts = self.parts;
        let split = parts.split.clamp(0.1, 0.9);
        let pane = |id: &'static str, grow: f32, header: AnyElement, body: AnyElement| {
            div()
                .id(id)
                .test_support()
                .relative()
                .flex()
                .flex_col()
                .flex_grow(grow)
                .flex_shrink(1.)
                .flex_basis(relative(0.))
                .min_h_0()
                .child(header)
                .child(div().flex_1().min_h_0().child(body))
        };
        div()
            .id("app")
            .test_support()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.text)
            .font_family(t.fonts.ui.clone())
            .text_size(px(13.))
            .child(parts.titlebar)
            .child(
                div()
                    .id("workspace")
                    .test_support()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .children(parts.sidebar)
                    .child(
                        div()
                            .id("main")
                            .test_support()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(parts.info_bar)
                            .child(pane("pane-top", split, parts.top_header, parts.top_body))
                            .child(parts.band)
                            .child(pane("pane-bottom", 1. - split, parts.bottom_header, parts.bottom_body))
                            .child(parts.status),
                    ),
            )
            .children(parts.overlay)
    }
}
