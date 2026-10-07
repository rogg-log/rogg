use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, size};
use gpui_kit::component::Sizable;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, SharedString, TestSupportExt, Window, div, px};

/// `.find`: 24px search box with icon, a borderless gpui-kit Input (112px) and a hit counter.
#[derive(IntoElement)]
pub struct FindField {
    id: SharedString,
    input: Entity<InputState>,
    count: Option<SharedString>,
}

impl FindField {
    pub fn new(id: impl Into<SharedString>, input: Entity<InputState>) -> Self {
        Self { id: id.into(), input, count: None }
    }

    pub fn count(mut self, count: Option<SharedString>) -> Self {
        self.count = count;
        self
    }

    /// Creates the input state a FindField renders.
    pub fn state(placeholder: impl Into<SharedString>, window: &mut Window, cx: &mut App) -> Entity<InputState> {
        let placeholder = placeholder.into();
        cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
    }
}

impl RenderOnce for FindField {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let s = cx.rogg().text_scale();
        // Input::h is multi-line only and shadows Styled::h, so call the trait methods explicitly.
        let input = Input::new(&self.input).xsmall().appearance(false).p_0().text_size(s.px(10.)).line_height(px(14.)).text_color(p.text);
        let input = Styled::w(Styled::h(input, px(size::FIND_H - 2.)), px(size::FIND_INPUT_W));
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .h(px(size::FIND_H))
            .px(px(6.))
            .gap(px(5.))
            .bg(p.side)
            .border_1()
            .border_color(p.border)
            .rounded(px(4.))
            .child(icon(IconName::Search).size(px(12.)).text_color(p.faint))
            .child(input)
            // `.found` is always present (empty without a query), so its leading 5px gap is part of the width.
            .child(div().flex_none().text_size(s.px(9.)).text_color(p.faint).whitespace_nowrap().children(self.count))
    }
}
