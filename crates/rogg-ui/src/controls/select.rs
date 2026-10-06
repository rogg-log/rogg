use crate::theme::{ActiveRoggTheme, size};
use crate::IndexCallback;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::prelude::*;
use gpui_kit::{App, SharedString, Window, px};
use std::rc::Rc;

/// The `Context` select in the filter band: a 19px bordered button opening a gpui-kit dropdown menu.
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
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let variant = ButtonCustomVariant::new(cx)
            .color(p.bg.hsla())
            .foreground(p.text.hsla())
            .hover(p.hover.hsla())
            .active(p.hover.hsla())
            .shadow(false);
        let label = self.options.get(self.selected).cloned().unwrap_or_default();
        let options = self.options.clone();
        let selected = self.selected;
        let on_select = self.on_select.clone();
        Button::new(self.id)
            .custom(variant)
            .label(label)
            .dropdown_caret(true)
            .h(px(size::SELECT_H))
            .px(px(3.))
            .text_size(px(10.))
            .border_1()
            .border_color(p.border)
            .rounded(px(4.))
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
