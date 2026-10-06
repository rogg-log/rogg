use crate::IndexCallback;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, size};
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::prelude::*;
use gpui_kit::{App, SharedString, TextRun, Window, div, px, relative};
use std::rc::Rc;

/// The `Context` select in the filter band: a 19px bordered, native-select-like button opening a gpui-kit dropdown menu.
#[derive(IntoElement)]
pub struct CompactSelect {
    id: SharedString,
    options: Vec<SharedString>,
    selected: usize,
    on_select: Option<IndexCallback>,
}

impl CompactSelect {
    pub fn new(id: impl Into<SharedString>, options: Vec<SharedString>, selected: usize) -> Self {
        Self { id: id.into(), options, selected, on_select: None }
    }

    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for CompactSelect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        // `.filter-bottom select` inherits the muted color and 10px font; like a native select, it is as wide as its
        // widest option, so the label box is sized by measuring every option.
        let font = gpui_kit::font(t.fonts.ui.clone());
        let label_w = self
            .options
            .iter()
            .map(|o| {
                let run = TextRun { len: o.len(), font: font.clone(), color: p.muted.hsla(), background_color: None, underline: None, strikethrough: None };
                window.text_system().shape_line(o.clone(), px(10.), &[run], None).width
            })
            .fold(px(0.), |a, b| a.max(b));
        let variant = ButtonCustomVariant::new(cx)
            .color(p.bg.hsla())
            .foreground(p.muted.hsla())
            .hover(p.hover.hsla())
            .active(p.hover.hsla())
            .shadow(false);
        let label = self.options.get(self.selected).cloned().unwrap_or_default();
        let options = self.options.clone();
        let selected = self.selected;
        let on_select = self.on_select.clone();
        Button::new(self.id)
            .custom(variant)
            // The custom variant renders its color at 80% alpha; the reference select is opaque `var(--bg)`.
            .bg(p.bg)
            .h(px(size::SELECT_H))
            .pl(px(7.5))
            .pr(px(3.))
            .border_1()
            .border_color(p.border)
            .rounded(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(3.))
                    .text_size(px(10.))
                    .line_height(relative(size::LINE_HEIGHT))
                    .text_color(p.muted)
                    .child(div().w(label_w).child(label))
                    .child(icon(IconName::SelectCaret).size(px(13.)).text_color(p.muted)),
            )
            .dropdown_menu(move |menu, _, _| {
                options.iter().enumerate().fold(menu, |menu, (i, option)| {
                    let on_select = on_select.clone();
                    menu.item(PopupMenuItem::new(option.clone()).checked(i == selected).on_click(move |_, window, cx| {
                        if let Some(f) = &on_select {
                            f(i, window, cx)
                        }
                    }))
                })
            })
    }
}
