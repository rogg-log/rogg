use crate::controls::{CompactSelect, FlagButton, IconButton, TextButton};
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, Color, size};
use crate::{Callback, IndexCallback};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Entity, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ChipState {
    Include,
    Off,
    Exclude,
}

impl ChipState {
    /// Include → Off → Exclude → Include (reference hint).
    pub fn next(self) -> Self {
        match self {
            ChipState::Include => ChipState::Off,
            ChipState::Off => ChipState::Exclude,
            ChipState::Exclude => ChipState::Include,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ChipState::Include => "IN",
            ChipState::Off => "OFF",
            ChipState::Exclude => "NOT",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            ChipState::Include => IconName::Check,
            ChipState::Off => IconName::Off,
            ChipState::Exclude => IconName::Minus,
        }
    }
}

/// `.token`: clickable state segment + monospace term; colored by slot, neutral when off, struck through when excluded.
#[derive(IntoElement)]
pub struct FilterChip {
    index: usize,
    text: SharedString,
    state: ChipState,
    on_cycle: Option<Callback>,
}

impl FilterChip {
    pub fn new(index: usize, text: impl Into<SharedString>, state: ChipState) -> Self {
        Self { index, text: text.into(), state, on_cycle: None }
    }

    pub fn on_cycle(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cycle = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for FilterChip {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        let slot = p.chips[self.index % p.chips.len()];
        let (border, bg, term) = match self.state {
            ChipState::Off => (p.border, Color::TRANSPARENT, p.faint),
            _ => (slot.border, slot.bg, slot.term),
        };
        let i = self.index;
        let on_cycle = self.on_cycle;
        div()
            .id(SharedString::from(format!("token-{i}")))
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .h(px(size::CHIP_H))
            .border_1()
            .border_color(border)
            .rounded(px(5.))
            .bg(bg)
            .font_family(t.fonts.mono.clone())
            .text_size(px(12.))
            .child(
                div()
                    .id(SharedString::from(format!("token-state-{i}")))
                    .test_support()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(4.))
                    .h_full()
                    // `.state` is a button: 1px transparent border on top/bottom/left plus 5px padding.
                    .pl(px(6.))
                    .pr(px(5.))
                    .rounded_l(px(3.))
                    .border_r_1()
                    .border_color(p.chip_divider)
                    .font_family(t.fonts.ui.clone())
                    .text_size(px(9.))
                    .text_color(p.muted)
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(icon(self.state.icon()).size(px(11.)).text_color(p.muted))
                    .child(self.state.label())
                    .on_click(move |_, window, cx| {
                        if let Some(f) = &on_cycle {
                            f(window, cx)
                        }
                    }),
            )
            .child(div().pl(px(6.)).pr(px(8.)).text_color(term).when(self.state == ChipState::Exclude, |d| d.line_through()).child(self.text))
    }
}

/// `.filter-band`: chips (or expression editor), flags, and the History / Context / summary / hint row.
#[derive(IntoElement)]
pub struct FilterBand {
    terms: Vec<(SharedString, ChipState)>,
    regex: bool,
    case_sensitive: bool,
    editing: Option<Entity<InputState>>,
    error: Option<SharedString>,
    context_options: Vec<SharedString>,
    context_selected: usize,
    summary: SharedString,
    hint: SharedString,
    on_cycle: Option<IndexCallback>,
    on_edit: Option<Callback>,
    on_apply: Option<Callback>,
    on_toggle_regex: Option<Callback>,
    on_toggle_case: Option<Callback>,
    on_history: Option<Callback>,
    on_context: Option<IndexCallback>,
}

impl FilterBand {
    pub fn new(terms: Vec<(SharedString, ChipState)>) -> Self {
        Self {
            terms,
            regex: false,
            case_sensitive: false,
            editing: None,
            error: None,
            context_options: vec!["None".into(), "±1 line".into(), "±2 lines".into(), "±3 lines".into()],
            context_selected: 0,
            summary: SharedString::default(),
            hint: "Click a state to cycle Include → Off → Exclude".into(),
            on_cycle: None,
            on_edit: None,
            on_apply: None,
            on_toggle_regex: None,
            on_toggle_case: None,
            on_history: None,
            on_context: None,
        }
    }

    pub fn regex(mut self, on: bool) -> Self {
        self.regex = on;
        self
    }

    pub fn case_sensitive(mut self, on: bool) -> Self {
        self.case_sensitive = on;
        self
    }

    pub fn editing(mut self, input: Option<Entity<InputState>>) -> Self {
        self.editing = input;
        self
    }

    pub fn error(mut self, error: Option<SharedString>) -> Self {
        self.error = error;
        self
    }

    pub fn context(mut self, options: Vec<SharedString>, selected: usize) -> Self {
        self.context_options = options;
        self.context_selected = selected;
        self
    }

    pub fn summary(mut self, summary: SharedString) -> Self {
        self.summary = summary;
        self
    }

    pub fn hint(mut self, hint: SharedString) -> Self {
        self.hint = hint;
        self
    }

    pub fn on_cycle(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_cycle = Some(Rc::new(f));
        self
    }

    pub fn on_edit(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_edit = Some(Rc::new(f));
        self
    }

    pub fn on_apply(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_apply = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_regex(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_regex = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_case(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_case = Some(Rc::new(f));
        self
    }

    pub fn on_history(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_history = Some(Rc::new(f));
        self
    }

    pub fn on_context(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_context = Some(Rc::new(f));
        self
    }
}

fn call(cb: &Option<Callback>) -> impl Fn(&mut Window, &mut App) + 'static {
    let cb = cb.clone();
    move |w, cx| {
        if let Some(f) = &cb {
            f(w, cx)
        }
    }
}

impl RenderOnce for FilterBand {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        let editing = self.editing.is_some();

        let chips_or_editor: AnyElement = match self.editing.clone() {
            Some(input) => div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .id("expression")
                        .test_support()
                        .flex_1()
                        .min_w(px(80.))
                        .px(px(8.))
                        .py(px(6.))
                        .bg(p.bg)
                        .border_1()
                        .border_color(p.accent)
                        .rounded(px(4.))
                        .child(
                            Input::new(&input).appearance(false).p_0().font_family(t.fonts.mono.clone()).text_size(px(12.)).text_color(p.text),
                        ),
                )
                .child(
                    div()
                        .id("apply")
                        .test_support()
                        .flex_none()
                        .px(px(8.))
                        .py(px(5.))
                        .border_1()
                        .border_color(p.accent)
                        .rounded(px(5.))
                        .text_size(px(11.))
                        .text_color(p.accent)
                        .cursor_pointer()
                        .hover(move |s| s.bg(p.hover))
                        .child("Apply ↵")
                        .on_click({
                            let f = call(&self.on_apply);
                            move |_, w, cx| f(w, cx)
                        }),
                )
                .into_any_element(),
            None => {
                let mut row = div().id("filters").test_support().flex().flex_1().min_w_0().items_center().gap(px(7.)).overflow_x_scroll();
                for (i, (text, state)) in self.terms.iter().cloned().enumerate() {
                    if i > 0 {
                        row = row.child(div().flex_none().font_family(t.fonts.mono.clone()).text_size(px(11.)).text_color(p.faint).child("||"));
                    }
                    let on_cycle = self.on_cycle.clone();
                    row = row.child(FilterChip::new(i, text, state).on_cycle(move |w, cx| {
                        if let Some(f) = &on_cycle {
                            f(i, w, cx)
                        }
                    }));
                }
                row.into_any_element()
            }
        };

        let actions = div()
            .id("filter-actions")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .ml(px(6.))
            .when(!editing, |d| d.child(IconButton::icon("edit-filters", IconName::Edit).on_click(call(&self.on_edit))))
            .child(FlagButton::new("flag-regex", ".*").pressed(self.regex).on_click(call(&self.on_toggle_regex)))
            .child(FlagButton::new("flag-case", "Aa").pressed(self.case_sensitive).on_click(call(&self.on_toggle_case)));

        let on_context = self.on_context.clone();
        let bottom = div()
            .id("filter-bottom")
            .test_support()
            .flex()
            .items_center()
            .gap(px(13.))
            .mt(px(8.))
            .text_size(px(10.))
            .text_color(p.muted)
            .child(TextButton::small("history", "History⌄").icon(IconName::History).on_click(call(&self.on_history)))
            .child(
                div().flex().items_center().gap(px(5.)).child("Context").child(
                    CompactSelect::new("context-select", self.context_options, self.context_selected).on_select(move |i, w, cx| {
                        if let Some(f) = &on_context {
                            f(i, w, cx)
                        }
                    }),
                ),
            )
            .child(div().child(self.summary))
            .child(div().ml_auto().text_size(px(9.)).text_color(p.faint).child(self.hint));

        div()
            .id("filter-band")
            .test_support()
            .relative()
            .flex_none()
            .bg(p.side)
            // gpui paints an element's own border over its children, which would cut through the handle; the 1px
            // lines are drawn as children instead, before the handle.
            .border_t_1()
            .border_b_1()
            .border_color(Color::TRANSPARENT)
            .pt(px(10.))
            .px(px(12.))
            .pb(px(8.))
            .child(div().absolute().top(px(-1.)).left_0().right_0().h(px(1.)).bg(p.band_border))
            .child(div().absolute().bottom(px(-1.)).left_0().right_0().h(px(1.)).bg(p.band_border))
            .child(
                div()
                    .absolute()
                    .top(px(-2.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(div().w(px(30.)).h(px(3.)).rounded(px(5.)).bg(p.band_handle)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_h(px(size::FILTER_LINE_MIN_H))
                    .child(icon(IconName::Filter).size(px(15.)).mr(px(4.)).text_color(p.accent))
                    .child(chips_or_editor)
                    .child(actions),
            )
            .when_some(self.error, |d, e| {
                d.child(
                    div()
                        .id("filter-error")
                        .test_support()
                        .mt(px(6.))
                        .text_size(px(10.))
                        .text_color(p.error)
                        .child(format!("Previous results retained · {e}")),
                )
            })
            .child(bottom)
    }
}
