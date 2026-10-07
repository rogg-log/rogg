use super::pressed;
use crate::theme::{ActiveRoggTheme, Color};
use crate::IndexCallback;
use gpui_kit::prelude::*;
use gpui_kit::{App, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

/// `.segmented`: bordered group of mutually exclusive options.
#[derive(IntoElement)]
pub struct Segmented {
    id: SharedString,
    options: Vec<SharedString>,
    selected: usize,
    on_select: Option<IndexCallback>,
}

impl Segmented {
    pub fn new(id: impl Into<SharedString>, options: Vec<SharedString>, selected: usize) -> Self {
        Self { id: id.into(), options, selected, on_select: None }
    }

    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let s = cx.rogg().text_scale();
        let id = self.id.clone();
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .gap(px(3.))
            .p(px(3.))
            .border_1()
            .border_color(p.border)
            .rounded(px(7.))
            .children(self.options.into_iter().enumerate().map(|(i, label)| {
                let on_select = self.on_select.clone();
                div()
                    .id(SharedString::from(format!("{id}-{i}")))
                    .test_support()
                    .px(px(11.))
                    .py(px(6.))
                    .text_size(s.px(12.))
                    .rounded(px(5.))
                    .border_1()
                    .cursor_pointer()
                    .map(|d| {
                        if i == self.selected {
                            pressed(d, &p)
                        } else {
                            d.text_color(p.text).border_color(Color::TRANSPARENT).hover(move |s| s.bg(p.hover))
                        }
                    })
                    .child(label)
                    .on_click(move |_, window, cx| {
                        if let Some(f) = &on_select {
                            f(i, window, cx)
                        }
                    })
            }))
    }
}
