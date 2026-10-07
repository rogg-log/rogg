use crate::controls::{ActionButton, IconButton};
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, Color, size};
use crate::{Callback, IndexCallback};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, BoxShadow, Entity, FontWeight, MouseButton, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

/// Space kept free around a dialog that is taller than the window; the body scrolls instead.
const VIEWPORT_MARGIN: f32 = 80.;

/// The reference `dialog`: 500px, 22px padding, 10px radius, side background, large soft shadow.
#[derive(IntoElement)]
pub struct DialogFrame {
    title: SharedString,
    description: Option<SharedString>,
    body: Vec<AnyElement>,
    note: Option<SharedString>,
    on_close: Option<Callback>,
}

impl DialogFrame {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self { title: title.into(), description: None, body: vec![], note: None, on_close: None }
    }

    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
        self
    }

    pub fn note(mut self, text: impl Into<SharedString>) -> Self {
        self.note = Some(text.into());
        self
    }

    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for DialogFrame {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let s = cx.rogg().text_scale();
        let on_close = self.on_close;
        div()
            .id("dialog")
            .test_support()
            .occlude()
            .flex()
            .flex_col()
            .max_h(window.viewport_size().height - px(VIEWPORT_MARGIN))
            .w(px(size::DIALOG_W))
            .p(px(size::DIALOG_PAD))
            .bg(p.side)
            .border_1()
            .border_color(p.border)
            .rounded(px(10.))
            .shadow(vec![BoxShadow::new(px(0.), px(20.), p.dialog_shadow.hsla()).blur_radius(px(90.))])
            .text_color(p.text)
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .child(div().pb(px(6.)).text_size(s.px(17.)).font_weight(FontWeight(550.)).child(self.title))
                    .child(IconButton::glyph("dialog-close", "✕").on_click(move |w, cx| {
                        if let Some(f) = &on_close {
                            f(w, cx)
                        }
                    })),
            )
            .child(
                div()
                    .id("dialog-body")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when_some(self.description, |d, text| {
                        d.child(div().my(px(12.)).text_size(s.px(12.)).line_height(px(20.4)).text_color(p.muted).child(text))
                    })
                    .children(self.body)
                    .when_some(self.note, |d, text| {
                        d.child(div().my(px(10.)).text_size(s.px(10.)).line_height(px(17.)).text_color(p.muted).child(text))
                    }),
            )
    }
}

/// Full-window backdrop that centres a dialog; clicking outside the dialog calls `on_dismiss`.
#[derive(IntoElement)]
pub struct ModalLayer {
    child: AnyElement,
    on_dismiss: Option<Callback>,
}

impl ModalLayer {
    pub fn new(child: impl IntoElement) -> Self {
        Self { child: child.into_any_element(), on_dismiss: None }
    }

    pub fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ModalLayer {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let on_dismiss = self.on_dismiss;
        div()
            .id("modal-layer")
            .test_support()
            .occlude()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(p.backdrop)
            .on_mouse_down(MouseButton::Left, move |_, w, cx| {
                if let Some(f) = &on_dismiss {
                    f(w, cx)
                }
            })
            .child(self.child)
    }
}

/// `.profile-rule`: top border, optional 21px swatch, leading code/text, trailing muted summary.
#[derive(IntoElement)]
pub struct RuleRow {
    id: SharedString,
    swatch: Option<Color>,
    lead: SharedString,
    code: bool,
    trailing: Option<SharedString>,
}

impl RuleRow {
    pub fn code(id: impl Into<SharedString>, lead: impl Into<SharedString>) -> Self {
        Self { id: id.into(), swatch: None, lead: lead.into(), code: true, trailing: None }
    }

    pub fn text(id: impl Into<SharedString>, lead: impl Into<SharedString>) -> Self {
        Self { id: id.into(), swatch: None, lead: lead.into(), code: false, trailing: None }
    }

    pub fn swatch(mut self, c: Color) -> Self {
        self.swatch = Some(c);
        self
    }

    pub fn trailing(mut self, text: impl Into<SharedString>) -> Self {
        self.trailing = Some(text.into());
        self
    }
}

impl RenderOnce for RuleRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let s = t.text_scale();
        let p = t.palette;
        div()
            .id(self.id)
            .test_support()
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(14.))
            .border_t_1()
            .border_color(p.border)
            .text_size(s.px(12.))
            .when_some(self.swatch, |d, c| d.child(div().flex_none().size(px(21.)).rounded(px(4.)).border_1().border_color(p.swatch_border).bg(c)))
            .child(div().when(self.code, |d| d.font_family(t.fonts.mono.clone())).child(self.lead))
            .when_some(self.trailing, |d, text| d.child(div().ml_auto().text_size(s.px(11.)).text_color(p.muted).child(text)))
    }
}

/// `.shortcut`: label left, accent monospace keys right, bottom border.
#[derive(IntoElement)]
pub struct ShortcutRow {
    id: SharedString,
    label: SharedString,
    keys: SharedString,
}

impl RenderOnce for ShortcutRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let s = t.text_scale();
        let p = t.palette;
        div()
            .id(self.id)
            .test_support()
            .flex()
            .justify_between()
            .py(px(9.))
            .border_b_1()
            .border_color(p.border)
            .text_size(s.px(12.))
            .child(self.label)
            .child(div().font_family(t.fonts.mono.clone()).text_size(s.px(11.)).text_color(p.accent).child(self.keys))
    }
}

/// `.tab-picker-item`: file icon, label, check mark on the current entry.
#[derive(IntoElement)]
pub struct PickerItem {
    id: SharedString,
    label: SharedString,
    current: bool,
    on_click: Option<Callback>,
}

impl RenderOnce for PickerItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let s = cx.rogg().text_scale();
        let on_click = self.on_click;
        div()
            .id(self.id)
            .test_support()
            .flex()
            .items_center()
            .w_full()
            .gap(px(10.))
            .p(px(9.))
            // A reference <button>: 1px transparent border, icon in the inherited text color.
            .border_1()
            .border_color(Color::TRANSPARENT)
            .rounded(px(5.))
            .text_size(s.px(12.))
            .cursor_pointer()
            .when(self.current, |d| d.bg(p.selection))
            .hover(move |s| s.bg(p.hover))
            .child(icon(IconName::File).size(px(15.)).text_color(p.text))
            .child(self.label)
            .when(self.current, |d| d.child(div().ml_auto().text_color(p.accent).child("✓")))
            .on_click(move |_, w, cx| {
                if let Some(f) = &on_click {
                    f(w, cx)
                }
            })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileRuleView {
    pub swatch: Color,
    pub pattern: SharedString,
    pub summary: SharedString,
}

/// A list of choices with the current one checked (`.tab-picker-item` rows), e.g. All tabs or the log font picker.
pub fn picker(title: impl Into<SharedString>, items: &[(SharedString, bool)], on_pick: IndexCallback) -> DialogFrame {
    items.iter().enumerate().fold(DialogFrame::new(title), |d, (i, (label, current))| {
        let on_pick = on_pick.clone();
        d.child(PickerItem {
            id: format!("picker-{i}").into(),
            label: label.clone(),
            current: *current,
            on_click: Some(Rc::new(move |w, cx| on_pick(i, w, cx))),
        })
    })
}

pub fn all_tabs(tabs: &[(SharedString, bool)], on_pick: IndexCallback) -> DialogFrame {
    picker("Open files", tabs, on_pick)
}

pub fn profile(name: &str, rules: &[ProfileRuleView]) -> DialogFrame {
    rules.iter().enumerate().fold(DialogFrame::new(format!("Highlighting · {name}")), |d, (i, r)| {
        d.child(RuleRow::code(format!("rule-{i}"), r.pattern.clone()).swatch(r.swatch).trailing(r.summary.clone()))
    })
}

pub fn appearance(rows: &[(SharedString, SharedString)]) -> DialogFrame {
    rows.iter().enumerate().fold(DialogFrame::new("Appearance"), |d, (i, (k, v))| d.child(RuleRow::text(format!("setting-{i}"), k.clone()).trailing(v.clone())))
}

pub fn shortcuts(rows: &[(SharedString, SharedString)]) -> DialogFrame {
    rows.iter().enumerate().fold(DialogFrame::new("Keyboard navigation"), |d, (i, (label, keys))| {
        d.child(ShortcutRow { id: format!("shortcut-row-{i}").into(), label: label.clone(), keys: keys.clone() })
    })
}

pub fn history(entries: &[SharedString]) -> DialogFrame {
    entries.iter().enumerate().fold(DialogFrame::new("Filter history"), |d, (i, e)| d.child(RuleRow::code(format!("history-{i}"), e.clone())))
}

/// Filter history whose rows restore an entry when clicked. Each entry is `(expression, states summary)`,
/// e.g. `("error || timeout", "1 include · 1 exclude · 0 off")` (spec §5.8: expression and states).
pub fn history_picker(entries: &[(SharedString, SharedString)], on_pick: IndexCallback) -> DialogFrame {
    entries.iter().enumerate().fold(DialogFrame::new("Filter history"), |d, (i, (expression, states))| {
        let on_pick = on_pick.clone();
        d.child(
            div()
                .id(SharedString::from(format!("history-pick-{i}")))
                .test_support()
                .cursor_pointer()
                .on_click(move |_, w, cx| on_pick(i, w, cx))
                .child(RuleRow::code(format!("history-{i}"), expression.clone()).trailing(states.clone())),
        )
    })
}

pub fn statistics(rows: &[(SharedString, SharedString)]) -> DialogFrame {
    rows.iter().enumerate().fold(DialogFrame::new("Filtered log statistics"), |d, (i, (k, v))| d.child(RuleRow::text(format!("stat-{i}"), k.clone()).trailing(v.clone())))
}

pub fn export(options: &[(SharedString, bool)], on_toggle: IndexCallback, on_export: Callback) -> DialogFrame {
    let frame = options.iter().enumerate().fold(DialogFrame::new("Export filtered lines"), |d, (i, (label, on))| {
        let on_toggle = on_toggle.clone();
        d.child(
            div()
                .id(SharedString::from(format!("export-option-{i}")))
                .cursor_pointer()
                .on_click(move |_, w, cx| on_toggle(i, w, cx))
                .child(RuleRow::text(format!("export-row-{i}"), label.clone()).trailing(if *on { "On" } else { "Off" })),
        )
    });
    frame.child(div().flex().justify_end().pt(px(14.)).child(ActionButton::new("export-run", "Export…").primary().on_click(move |w, cx| on_export(w, cx))))
}

pub fn go_to_line(input: Entity<InputState>) -> DialogFrame {
    DialogFrame::new("Go to line").child(GoToLineField { input })
}

#[derive(IntoElement)]
struct GoToLineField {
    input: Entity<InputState>,
}

impl RenderOnce for GoToLineField {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let s = t.text_scale();
        let p = t.palette;
        div()
            .mt(px(12.))
            .px(px(8.))
            .py(px(6.))
            .bg(p.bg)
            .border_1()
            .border_color(p.accent)
            .rounded(px(4.))
            .child(Input::new(&self.input).appearance(false).p_0().font_family(t.fonts.mono.clone()).text_size(s.px(12.)).text_color(p.text))
    }
}
