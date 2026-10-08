//! Form controls for the settings and profile-editor dialogs (spec §5.8, §6.1, §6.2), in the reference dialog's style:
//! `.profile-rule` rows, `button[aria-pressed]` states, `.swatch` squares.

use crate::controls::pressed;
use crate::icons::{icon, IconName};
use crate::theme::{ActiveRoggTheme, Color};
use crate::Callback;
use gpui_kit::component::{Icon as KitIcon, Sizable};
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerState};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Entity, FontWeight, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

/// Callback for the up/down buttons: `true` is up.
type StepCallback = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A clickable child that never lets the click reach the row underneath.
fn stop<E: InteractiveElement + StatefulInteractiveElement>(el: E, f: Option<Callback>) -> E {
    el.on_click(move |_, window, cx| {
        cx.stop_propagation();
        if let Some(f) = &f {
            f(window, cx)
        }
    })
}

/// A dialog row: label (and an optional hint) on the left, a control on the right, a 1 px top border.
#[derive(IntoElement)]
pub struct FormRow {
    id: SharedString,
    label: SharedString,
    hint: Option<SharedString>,
    control: Option<AnyElement>,
}

impl FormRow {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), hint: None, control: None }
    }

    pub fn hint(mut self, text: impl Into<SharedString>) -> Self {
        self.hint = Some(text.into());
        self
    }

    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }
}

impl RenderOnce for FormRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (p, s) = (cx.rogg().palette, cx.rogg().text_scale());
        div()
            .id(self.id)
            .test_support()
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(10.))
            .border_t_1()
            .border_color(p.border)
            .text_size(s.px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(self.label)
                    .when_some(self.hint, |d, h| d.child(div().text_size(s.px(10.)).text_color(p.faint).child(h))),
            )
            .child(div().ml_auto().flex_none().flex().items_center().gap(px(6.)).children(self.control))
    }
}

/// `.small-label`: an uppercase 10 px section title.
#[derive(IntoElement)]
pub struct SectionLabel {
    text: SharedString,
}

impl SectionLabel {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for SectionLabel {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (p, s) = (cx.rogg().palette, cx.rogg().text_scale());
        div().mt(px(16.)).mb(px(4.)).text_size(s.px(10.)).font_weight(FontWeight::SEMIBOLD).text_color(p.muted).child(self.text.to_uppercase())
    }
}

/// An on/off switch: a 28×16 pill, accent when on.
#[derive(IntoElement)]
pub struct Toggle {
    id: SharedString,
    on: bool,
    on_click: Option<Callback>,
}

impl Toggle {
    pub fn new(id: impl Into<SharedString>, on: bool) -> Self {
        Self { id: id.into(), on, on_click: None }
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let knob = div().size(px(10.)).rounded(px(5.)).bg(if self.on { p.accent } else { p.muted });
        let el = div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .w(px(28.))
            .h(px(16.))
            .px(px(2.))
            .rounded(px(8.))
            .border_1()
            .cursor_pointer()
            .map(|d| if self.on { d.justify_end().bg(p.accent_bg).border_color(p.accent_border) } else { d.bg(p.raised).border_color(p.border) })
            .child(knob);
        stop(el, self.on_click)
    }
}

fn step_button(id: SharedString, glyph: &'static str, enabled: bool, f: Option<Callback>, cx: &App) -> impl IntoElement {
    let (p, s) = (cx.rogg().palette, cx.rogg().text_scale());
    let el = div()
        .id(id)
        .test_support()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(22.))
        .rounded(px(5.))
        .border_1()
        .border_color(p.border)
        .text_size(s.px(12.))
        .text_color(if enabled { p.text } else { p.faint })
        .child(glyph)
        .when(enabled, |d| d.cursor_pointer().hover(move |st| st.bg(p.hover)));
    stop(el, if enabled { f } else { None })
}

/// `−  value  +` with step buttons; a button at its bound is faint and inert.
#[derive(IntoElement)]
pub struct Stepper {
    id: SharedString,
    value: SharedString,
    can_down: bool,
    can_up: bool,
    on_step: Option<StepCallback>,
}

impl Stepper {
    pub fn new(id: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self { id: id.into(), value: value.into(), can_down: true, can_up: true, on_step: None }
    }

    pub fn bounds(mut self, can_down: bool, can_up: bool) -> Self {
        self.can_down = can_down;
        self.can_up = can_up;
        self
    }

    pub fn on_step(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_step = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Stepper {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (p, s) = (cx.rogg().palette, cx.rogg().text_scale());
        let step = |up: bool| -> Option<Callback> {
            self.on_step.clone().map(|f| Rc::new(move |w: &mut Window, cx: &mut App| f(up, w, cx)) as Callback)
        };
        div()
            .id(self.id.clone())
            .test_support()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(step_button(format!("{}-down", self.id).into(), "−", self.can_down, step(false), cx))
            .child(div().min_w(px(64.)).flex().justify_center().text_size(s.px(12.)).text_color(p.text).child(self.value))
            .child(step_button(format!("{}-up", self.id).into(), "+", self.can_up, step(true), cx))
    }
}

/// A 21 px colour square (reference `.swatch`); selected shows the accent ring.
#[derive(IntoElement)]
pub struct Swatch {
    id: SharedString,
    color: Color,
    selected: bool,
    on_click: Option<Callback>,
}

impl Swatch {
    pub fn new(id: impl Into<SharedString>, color: Color) -> Self {
        Self { id: id.into(), color, selected: false, on_click: None }
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Swatch {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let el = div()
            .id(self.id)
            .test_support()
            .flex_none()
            .size(px(21.))
            .rounded(px(4.))
            .border_1()
            .border_color(if self.selected { p.accent } else { p.swatch_border })
            .bg(self.color)
            .when(self.on_click.is_some(), |d| d.cursor_pointer());
        stop(el, self.on_click)
    }
}

/// A one-line input in the dialog style; `error` paints its border in the error colour.
#[derive(IntoElement)]
pub struct TextField {
    id: SharedString,
    input: Entity<InputState>,
    mono: bool,
    width: Option<f32>,
    error: bool,
}

impl TextField {
    pub fn new(id: impl Into<SharedString>, input: Entity<InputState>) -> Self {
        Self { id: id.into(), input, mono: false, width: None, error: false }
    }

    pub fn mono(mut self, on: bool) -> Self {
        self.mono = on;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn error(mut self, on: bool) -> Self {
        self.error = on;
        self
    }
}

impl RenderOnce for TextField {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let (p, s) = (t.palette, t.text_scale());
        let family = if self.mono { t.fonts.mono.clone() } else { t.fonts.ui.clone() };
        // 22 px tall like the dialog's buttons: the input at its extra-small size, without its own padding (as in
        // `FindField`). Input::h is multi-line only and shadows Styled::h, so call the trait method explicitly.
        let input = Input::new(&self.input).xsmall().appearance(false).p_0().font_family(family).text_size(s.px(12.)).line_height(px(14.)).text_color(p.text);
        div()
            .id(self.id)
            .test_support()
            .flex()
            .items_center()
            .h(px(22.))
            .px(px(8.))
            .py_0()
            .bg(p.bg)
            .border_1()
            .border_color(if self.error { p.error } else { p.border })
            .rounded(px(4.))
            // An explicit width is a promise: a crowded row must not shrink the field.
            .when_some(self.width, |d, w| d.w(px(w)).flex_none())
            .child(Styled::h(input, px(20.)))
    }
}

/// A rule in the profile editor: swatch, pattern (mono), summary, then the enable switch and move buttons.
#[derive(IntoElement)]
pub struct EditableRuleRow {
    index: usize,
    swatch: Color,
    pattern: SharedString,
    summary: SharedString,
    enabled: bool,
    selected: bool,
    error: Option<SharedString>,
    can_up: bool,
    can_down: bool,
    on_select: Option<Callback>,
    on_toggle: Option<Callback>,
    on_move: Option<StepCallback>,
}

impl EditableRuleRow {
    pub fn new(index: usize, swatch: Color, pattern: impl Into<SharedString>, summary: impl Into<SharedString>) -> Self {
        Self {
            index,
            swatch,
            pattern: pattern.into(),
            summary: summary.into(),
            enabled: true,
            selected: false,
            error: None,
            can_up: true,
            can_down: true,
            on_select: None,
            on_toggle: None,
            on_move: None,
        }
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn error(mut self, error: Option<SharedString>) -> Self {
        self.error = error;
        self
    }

    pub fn can_move(mut self, up: bool, down: bool) -> Self {
        self.can_up = up;
        self.can_down = down;
        self
    }

    pub fn on_select(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    pub fn on_toggle(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }

    pub fn on_move(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_move = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for EditableRuleRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let (p, s) = (t.palette, t.text_scale());
        let i = self.index;
        let mover = |up: bool| -> Option<Callback> { self.on_move.clone().map(|f| Rc::new(move |w: &mut Window, cx: &mut App| f(up, w, cx)) as Callback) };
        let on_select = self.on_select.clone();
        div()
            .id(SharedString::from(format!("profile-rule-{i}")))
            .test_support()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(p.border)
            .when(self.selected, |d| d.bg(p.selection))
            .cursor_pointer()
            .on_click(move |_, window, cx| {
                if let Some(f) = &on_select {
                    f(window, cx)
                }
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(10.))
                    .px(px(4.))
                    .text_size(s.px(12.))
                    .when(!self.enabled, |d| d.opacity(0.5))
                    .child(div().flex_none().size(px(21.)).rounded(px(4.)).border_1().border_color(p.swatch_border).bg(self.swatch))
                    // Only the pattern gives way to a narrow row; the summary, switch and move buttons keep their size.
                    .child(div().flex_1().min_w_0().truncate().font_family(t.fonts.mono.clone()).child(self.pattern))
                    .child(div().ml_auto().flex_none().text_size(s.px(11.)).text_color(p.muted).child(self.summary))
                    .child(Toggle::new(SharedString::from(format!("profile-rule-{i}-toggle")), self.enabled).on_click({
                        let f = self.on_toggle.clone();
                        move |w, cx| {
                            if let Some(f) = &f {
                                f(w, cx)
                            }
                        }
                    }))
                    .child(step_button(format!("profile-rule-{i}-up").into(), "↑", self.can_up, mover(true), cx))
                    .child(step_button(format!("profile-rule-{i}-down").into(), "↓", self.can_down, mover(false), cx)),
            )
            .when_some(self.error, |d, e| {
                d.child(div().id(SharedString::from(format!("profile-rule-{i}-error"))).test_support().px(px(4.)).pb(px(8.)).text_size(s.px(10.)).text_color(p.error).child(e))
            })
    }
}

/// One filter-match colour slot: a `term` sample in its colours; selected shows the accent border.
#[derive(IntoElement)]
pub struct PaletteSlot {
    index: usize,
    fg: Color,
    bg: Color,
    selected: bool,
    on_click: Option<Callback>,
}

impl PaletteSlot {
    pub fn new(index: usize, fg: Color, bg: Color) -> Self {
        Self { index, fg, bg, selected: false, on_click: None }
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for PaletteSlot {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let (p, s) = (t.palette, t.text_scale());
        let el = div()
            .id(SharedString::from(format!("palette-slot-{}", self.index)))
            .test_support()
            .flex_none()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(5.))
            .border_1()
            .map(|d| if self.selected { pressed(d, &p).bg(self.bg) } else { d.border_color(p.border).bg(self.bg) })
            .font_family(t.fonts.mono.clone())
            .text_size(s.px(12.))
            .text_color(self.fg)
            .cursor_pointer()
            .child(format!("term {}", self.index + 1));
        stop(el, self.on_click)
    }
}

/// Pen ink on a light fill (near-black).
pub const PEN_DARK: Color = Color::rgb(0x14171c);
/// Pen ink on a dark fill (near-white).
pub const PEN_LIGHT: Color = Color::rgb(0xf5f6f8);

/// The pen's colour over `fill`: dark ink on a light fill, light ink on a dark one (WCAG relative luminance, split where
/// both inks contrast equally). A mostly transparent fill (alpha below one half) shows the panel, so it gets `text`.
pub fn pen_ink(fill: Color, text: Color) -> Color {
    if (fill.0 & 0xff) < 0x80 {
        return text;
    }
    let lin = |c: u32| {
        let v = (c & 0xff) as f32 / 255.;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let lum = 0.2126 * lin(fill.0 >> 24) + 0.7152 * lin(fill.0 >> 16) + 0.0722 * lin(fill.0 >> 8);
    if lum > 0.179 { PEN_DARK } else { PEN_LIGHT }
}

/// The custom-colour button (spec C1): a swatch-sized square filled with the picker's current colour and a small pen on
/// it, opening gpui-kit's colour picker in a popover: the preset colours as featured colours, the kit palette, hue,
/// saturation, lightness and alpha sliders, and a hex field. Styled by the kit theme mirror.
///
/// The kit trigger is an invisible 21 px icon (so the click target is exactly the square); rogg paints the visible
/// square and pen over it in a non-interactive overlay.
#[derive(IntoElement)]
pub struct CustomColor {
    id: SharedString,
    state: Entity<ColorPickerState>,
    featured: Vec<Color>,
}

impl CustomColor {
    pub fn new(id: impl Into<SharedString>, state: &Entity<ColorPickerState>) -> Self {
        Self { id: id.into(), state: state.clone(), featured: vec![] }
    }

    pub fn featured(mut self, colors: Vec<Color>) -> Self {
        self.featured = colors;
        self
    }
}

impl RenderOnce for CustomColor {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let fill = self.state.read(cx).value().map(Color::from_hsla);
        let shown = fill.unwrap_or(p.raised);
        let ink = pen_ink(shown, p.text);
        let pen_id = SharedString::from(format!("{}-pen", self.id));
        let fill_id = SharedString::from(format!("{}-fill", self.id));
        let trigger = KitIcon::default().path(IconName::Edit.path()).size(px(21.)).text_color(gpui_kit::hsla(0., 0., 0., 0.));
        div()
            .id(self.id)
            .test_support()
            .relative()
            .flex_none()
            // No explicit size: the root measures the kit trigger, so a label or a mis-sized trigger shows in its bounds.
            .child(
                ColorPicker::new(&self.state)
                    .small()
                    .icon(trigger)
                    .accessibility_label("Custom colour")
                    .featured_colors(self.featured.iter().map(|c| c.hsla()).collect()),
            )
            .child(
                // `aria_value` publishes the painted colours (`#rrggbbaa`) so tests can read them back.
                div()
                    .id(fill_id)
                    .test_support()
                    .aria_value(format!("#{:08x}", shown.0))
                    .absolute()
                    .top_0()
                    .left_0()
                    .size(px(21.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(p.swatch_border)
                    .bg(shown)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .id(pen_id)
                            .test_support()
                            .aria_value(format!("#{:08x}", ink.0))
                            .child(icon(IconName::Edit).size(px(11.)).text_color(ink)),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pen_ink_contrasts_with_the_fill() {
        let text = Color::rgb(0x123456);
        assert_eq!(pen_ink(Color::rgb(0xffffff), text), PEN_DARK);
        assert_eq!(pen_ink(Color::rgb(0x000000), text), PEN_LIGHT);
        assert_eq!(pen_ink(Color::TRANSPARENT, text), text);
        assert_eq!(pen_ink(Color::rgb(0xffffff).with_alpha(0x40), text), text);
        assert_eq!(pen_ink(Color::rgb(0xffff00), text), PEN_DARK);
        assert_eq!(pen_ink(Color::rgb(0x0000ff), text), PEN_LIGHT);
    }
}
