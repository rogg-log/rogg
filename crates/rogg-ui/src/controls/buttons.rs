use super::pressed;
use crate::icons::{IconName, chevron, icon};
use crate::theme::{ActiveRoggTheme, Color, size};
use crate::Callback;
use gpui_kit::prelude::*;
use gpui_kit::{App, FontWeight, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

pub enum ButtonContent {
    Icon(IconName),
    Glyph(SharedString),
    ChevronDown,
}

/// `.icon-btn`: a square-ish ghost button holding a 15px icon or a text glyph (⌘, ✕).
#[derive(IntoElement)]
pub struct IconButton {
    id: SharedString,
    content: ButtonContent,
    compact: bool,
    force_hover: bool,
    on_click: Option<Callback>,
}

impl IconButton {
    pub fn icon(id: impl Into<SharedString>, name: IconName) -> Self {
        Self { id: id.into(), content: ButtonContent::Icon(name), compact: false, force_hover: false, on_click: None }
    }

    pub fn glyph(id: impl Into<SharedString>, glyph: impl Into<SharedString>) -> Self {
        Self { id: id.into(), content: ButtonContent::Glyph(glyph.into()), compact: false, force_hover: false, on_click: None }
    }

    /// The "open files" button: the chevron, pointing down.
    pub fn chevron_down(id: impl Into<SharedString>) -> Self {
        Self { id: id.into(), content: ButtonContent::ChevronDown, compact: false, force_hover: false, on_click: None }
    }

    /// The 25px title-bar variant (`.title-actions .icon-btn`).
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Render the hover state permanently (gallery specimens).
    pub fn force_hover(mut self, on: bool) -> Self {
        self.force_hover = on;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let s = cx.rogg().text_scale();
        let (side, pad_x, pad_y) =
            if self.compact { (size::TITLEBAR_ICON_BTN, 5., 3.) } else { (size::ICON_BTN, 6., 4.) };
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(5.))
            .min_w(px(side))
            .h(px(side))
            .px(px(pad_x))
            .py(px(pad_y))
            .rounded(px(5.))
            .border_1()
            .border_color(Color::TRANSPARENT)
            .text_color(p.muted)
            .text_size(s.px(13.))
            .cursor_pointer()
            .when(self.force_hover, |d| d.bg(p.hover))
            .hover(move |s| s.bg(p.hover))
            .child(match self.content {
                ButtonContent::Icon(name) => icon(name).size(px(15.)).text_color(p.muted).into_any_element(),
                ButtonContent::Glyph(g) => div().child(g).into_any_element(),
                ButtonContent::ChevronDown => chevron(true).size(px(13.)).text_color(p.muted).into_any_element(),
            })
            .when_some(self.on_click, |d, f| d.on_click(move |_, window, cx| f(window, cx)))
    }
}

/// `.flag`: the `.*` and `Aa` toggles next to the filter line.
#[derive(IntoElement)]
pub struct FlagButton {
    id: SharedString,
    label: SharedString,
    pressed: bool,
    on_click: Option<Callback>,
}

impl FlagButton {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), pressed: false, on_click: None }
    }

    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for FlagButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let s = t.text_scale();
        let p = t.palette;
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(size::FLAG_H))
            .min_w(px(size::FLAG_MIN_W))
            .px(px(6.))
            .py(px(3.))
            .rounded(px(5.))
            .border_1()
            .font_family(t.fonts.mono.clone())
            .text_size(s.px(13.))
            .cursor_pointer()
            .map(|d| {
                if self.pressed {
                    pressed(d, &p)
                } else {
                    d.text_color(p.muted).border_color(p.border).hover(move |s| s.bg(p.hover))
                }
            })
            .child(self.label)
            .when_some(self.on_click, |d, f| d.on_click(move |_, window, cx| f(window, cx)))
    }
}

/// `.tool`: icon + 10px label buttons in pane headers (Wrap, Follow selection, Open as tab).
#[derive(IntoElement)]
pub struct ToolButton {
    id: SharedString,
    icon: IconName,
    label: SharedString,
    pressed: bool,
    badge: Option<SharedString>,
    on_click: Option<Callback>,
}

impl ToolButton {
    pub fn new(id: impl Into<SharedString>, icon: IconName, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), icon, label: label.into(), pressed: false, badge: None, on_click: None }
    }

    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    /// Small trailing counter, e.g. `+1,204` lines arrived while live follow is paused.
    pub fn badge(mut self, badge: Option<SharedString>) -> Self {
        self.badge = badge;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ToolButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let s = cx.rogg().text_scale();
        let fg = if self.pressed { p.accent } else { p.muted };
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(5.))
            .h(px(size::TOOL_H))
            .px(px(7.))
            .py(px(3.))
            .rounded(px(5.))
            .border_1()
            .text_size(s.px(10.))
            .cursor_pointer()
            .map(|d| {
                if self.pressed {
                    pressed(d, &p)
                } else {
                    d.text_color(p.muted).border_color(Color::TRANSPARENT).hover(move |s| s.bg(p.hover))
                }
            })
            .child(icon(self.icon).size(px(13.)).text_color(fg))
            .child(self.label)
            .when_some(self.badge, |d, b| d.child(div().text_color(p.accent).child(b)))
            .when_some(self.on_click, |d, f| d.on_click(move |_, window, cx| f(window, cx)))
    }
}

/// Text buttons: `.meta button` (11px, Profile / Export) and `.filter-bottom button` (10px, History).
#[derive(IntoElement)]
pub struct TextButton {
    id: SharedString,
    label: SharedString,
    icon: Option<IconName>,
    dot: bool,
    text_size: f32,
    icon_size: f32,
    pad: (f32, f32),
    gap: f32,
    on_click: Option<Callback>,
}

impl TextButton {
    pub fn meta(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), icon: None, dot: false, text_size: 11., icon_size: 15., pad: (6., 4.), gap: 5., on_click: None }
    }

    pub fn small(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), icon: None, dot: false, text_size: 10., icon_size: 12., pad: (4., 2.), gap: 4., on_click: None }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Leading 5px accent dot (`.subtle-dot`).
    pub fn dot(mut self) -> Self {
        self.dot = true;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for TextButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let s = cx.rogg().text_scale();
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(self.gap))
            .px(px(self.pad.0))
            .py(px(self.pad.1))
            .rounded(px(5.))
            .border_1()
            .border_color(Color::TRANSPARENT)
            .text_size(s.px(self.text_size))
            .text_color(p.muted)
            .cursor_pointer()
            .hover(move |s| s.bg(p.hover))
            .when(self.dot, |d| d.child(div().size(px(5.)).rounded_full().bg(p.accent)))
            .when_some(self.icon, |d, i| d.child(icon(i).size(px(self.icon_size)).text_color(p.muted)))
            .child(self.label)
            .when_some(self.on_click, |d, f| d.on_click(move |_, window, cx| f(window, cx)))
    }
}

/// `.decision button`: bordered 11px buttons for empty states and dialogs; muted text (inherited in the reference),
/// primary uses the accent.
#[derive(IntoElement)]
pub struct ActionButton {
    id: SharedString,
    label: SharedString,
    primary: bool,
    on_click: Option<Callback>,
}

impl ActionButton {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), primary: false, on_click: None }
    }

    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ActionButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let s = cx.rogg().text_scale();
        div()
            .id(self.id)
            .test_support()
            .flex_none()
            .px(px(10.))
            .py(px(7.))
            .rounded(px(5.))
            .border_1()
            .text_size(s.px(11.))
            .font_weight(FontWeight::NORMAL)
            .cursor_pointer()
            .map(|d| if self.primary { d.text_color(p.accent).border_color(p.accent_border) } else { d.text_color(p.muted).border_color(p.border) })
            .hover(move |s| s.bg(p.hover))
            .child(self.label)
            .when_some(self.on_click, |d, f| d.on_click(move |_, window, cx| f(window, cx)))
    }
}
