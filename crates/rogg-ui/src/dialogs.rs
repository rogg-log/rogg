use crate::controls::{ActionButton, IconButton};
use crate::form::SectionLabel;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, Color, size};
use crate::{Callback, IndexCallback};
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, BoxShadow, Entity, FontWeight, MouseButton, SharedString, TestSupportExt, Window, div, px, relative};
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
    width: f32,
}

impl DialogFrame {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self { title: title.into(), description: None, body: vec![], note: None, on_close: None, width: size::DIALOG_W }
    }

    /// Overrides the default 500px width.
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
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
            .w(px(self.width))
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

/// The About dialog's content, prepared by the app.
#[derive(Clone, Debug, PartialEq)]
pub struct AboutView {
    /// `rogg 0.1.0`.
    pub version: SharedString,
    pub tagline: SharedString,
    /// (label, value): commit, build date, Rust.
    pub build: Vec<(SharedString, SharedString)>,
    /// (label, folder path); each row has a Show in Finder button.
    pub data: Vec<(SharedString, SharedString)>,
    pub built_with: SharedString,
    /// (crate, license).
    pub credits: Vec<(SharedString, SharedString)>,
}

/// About rogg (spec A4): header, Build, Data (with Show in Finder) and Credits sections.
pub fn about(view: &AboutView, on_reveal: IndexCallback) -> DialogFrame {
    let mut frame = DialogFrame::new("About rogg").child(AboutHeader { version: view.version.clone(), tagline: view.tagline.clone() });
    frame = frame.child(SectionLabel::new("Build"));
    for (i, (k, v)) in view.build.iter().enumerate() {
        frame = frame.child(RuleRow::text(format!("about-build-{i}"), k.clone()).trailing(v.clone()));
    }
    frame = frame.child(SectionLabel::new("Data"));
    for (i, (label, path)) in view.data.iter().enumerate() {
        frame = frame.child(DataRow { index: i, label: label.clone(), path: path.clone(), on_reveal: on_reveal.clone() });
    }
    frame = frame.child(SectionLabel::new("Credits")).child(AboutNote(view.built_with.clone()));
    for (i, (name, license)) in view.credits.iter().enumerate() {
        frame = frame.child(RuleRow::code(format!("about-credit-{i}"), name.clone()).trailing(license.clone()));
    }
    frame
}

#[derive(IntoElement)]
struct AboutHeader {
    version: SharedString,
    tagline: SharedString,
}

impl RenderOnce for AboutHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = cx.rogg().text_scale();
        let p = cx.rogg().palette;
        div()
            .id("about-header")
            .test_support()
            .flex()
            .flex_col()
            .gap(px(4.))
            .py(px(12.))
            .child(div().text_size(s.px(15.)).font_weight(FontWeight::MEDIUM).child(self.version))
            .child(div().text_size(s.px(12.)).text_color(p.muted).child(self.tagline))
    }
}

#[derive(IntoElement)]
struct AboutNote(SharedString);

impl RenderOnce for AboutNote {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div().pb(px(6.)).text_size(cx.rogg().text_scale().px(12.)).text_color(cx.rogg().palette.muted).child(self.0)
    }
}

/// Bytes of a line the full-line dialog shows; Copy still copies the whole line.
pub const FULL_LINE_SHOWN_BYTES: usize = 1024 * 1024;

/// The part of `text` the full-line dialog shows (at most 1 MB, cut on a char boundary), and whether it was cut.
pub fn full_line_shown(text: &str) -> (&str, bool) {
    let end = text.floor_char_boundary(FULL_LINE_SHOWN_BYTES.min(text.len()));
    (&text[..end], end < text.len())
}

/// "Show full line…" (spec B2): the whole line, read-only, selectable and wrapped, in the log font, with Copy.
pub fn full_line(number: u64, view: Entity<TextareaState>, cut: bool, on_copy: Callback) -> DialogFrame {
    let mut frame = DialogFrame::new(format!("Line {number}")).width(size::FULL_LINE_DIALOG_W).child(FullLineText { view });
    if cut {
        frame = frame.child(AboutNote("The line is longer than 1 MB: the first 1 MB is shown. Copy copies the whole line.".into()));
    }
    frame.child(div().flex().justify_end().pt(px(14.)).child(ActionButton::new("full-line-copy", "Copy").primary().on_click(move |w, cx| on_copy(w, cx))))
}

#[derive(IntoElement)]
struct FullLineText {
    view: Entity<TextareaState>,
}

impl RenderOnce for FullLineText {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        div()
            .id("full-line-text")
            .test_support()
            .mt(px(12.))
            .h(px(size::FULL_LINE_TEXT_H))
            .p(px(8.))
            .bg(t.palette.bg)
            .border_1()
            .border_color(t.palette.border)
            .rounded(px(5.))
            // The style goes on the Textarea itself: its Input root sets its own text size, which would override an inherited one.
            .child(
                Textarea::new(&self.view)
                    .readonly(true)
                    .appearance(false)
                    .h(relative(1.))
                    .font_family(t.fonts.log.clone())
                    .text_size(t.metrics.log_font_size)
                    .text_color(t.palette.log_text),
            )
    }
}

/// A data folder: label, path (mono, truncated) and Show in Finder.
#[derive(IntoElement)]
struct DataRow {
    index: usize,
    label: SharedString,
    path: SharedString,
    on_reveal: IndexCallback,
}

impl RenderOnce for DataRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let s = t.text_scale();
        let p = t.palette;
        let (i, on_reveal) = (self.index, self.on_reveal);
        div()
            .id(SharedString::from(format!("about-data-{i}")))
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
                    .flex_1()
                    .min_w_0()
                    .gap(px(3.))
                    .child(self.label)
                    .child(div().truncate().font_family(t.fonts.mono.clone()).text_size(s.px(11.)).text_color(p.muted).child(self.path)),
            )
            .child(ActionButton::new(SharedString::from(format!("about-reveal-{i}")), "Show in Finder").on_click(move |w, cx| on_reveal(i, w, cx)))
    }
}

#[cfg(test)]
mod tests {
    use super::{FULL_LINE_SHOWN_BYTES, full_line_shown};

    #[test]
    fn the_full_line_dialog_shows_at_most_1_mb_on_a_char_boundary() {
        assert_eq!(full_line_shown("short"), ("short", false));
        let long = "é".repeat(FULL_LINE_SHOWN_BYTES); // 2 MB of 2-byte chars
        let (shown, cut) = full_line_shown(&long);
        assert!(cut && shown.len() <= FULL_LINE_SHOWN_BYTES && shown.len() > FULL_LINE_SHOWN_BYTES - 2);
        assert!(long.is_char_boundary(shown.len()));
    }
}
