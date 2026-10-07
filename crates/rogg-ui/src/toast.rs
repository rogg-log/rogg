//! Transient notifications (spec §5.7): a stack at the bottom right, above the status bar.

use crate::controls::IconButton;
use crate::theme::{ActiveRoggTheme, size};
use gpui_kit::prelude::*;
use gpui_kit::{App, BoxShadow, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

type DismissCallback = Rc<dyn Fn(u64, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct ToastStack {
    toasts: Vec<(u64, SharedString)>,
    on_dismiss: Option<DismissCallback>,
}

impl ToastStack {
    pub fn new(toasts: Vec<(u64, SharedString)>) -> Self {
        Self { toasts, on_dismiss: None }
    }

    pub fn on_dismiss(mut self, f: impl Fn(u64, &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ToastStack {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let on_dismiss = self.on_dismiss;
        div()
            .id("toasts")
            .test_support()
            .absolute()
            .right(px(14.))
            .bottom(px(size::STATUS_BAR_H + 12.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(8.))
            .children(self.toasts.into_iter().map(|(id, message)| {
                let on_dismiss = on_dismiss.clone();
                div()
                    .id(SharedString::from(format!("toast-{id}")))
                    .test_support()
                    .occlude()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .max_w(px(360.))
                    .pl(px(14.))
                    .pr(px(6.))
                    .py(px(6.))
                    .bg(p.side)
                    .border_1()
                    .border_color(p.border)
                    .rounded(px(8.))
                    .shadow(vec![BoxShadow::new(px(0.), px(8.), p.dialog_shadow.hsla()).blur_radius(px(30.))])
                    .text_size(px(11.))
                    .text_color(p.text)
                    .child(div().flex_1().min_w_0().child(message))
                    .child(IconButton::glyph(SharedString::from(format!("toast-close-{id}")), "✕").compact().on_click(move |w, cx| {
                        if let Some(f) = &on_dismiss {
                            f(id, w, cx)
                        }
                    }))
            }))
    }
}
