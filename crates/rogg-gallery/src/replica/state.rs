use super::data::{self, FIRST_LINE_NUMBER, Filtered, SAMPLES, STAMP_LEN, Term};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext, Context, Entity, SharedString, Subscription, Window};
use rogg_ui::chrome::StatPart;
use rogg_ui::controls::FindField;
use rogg_ui::filter::ChipState;
use rogg_ui::logview::{LogLine, LogRow, LogViewEvent, LogViewState, RowTint, Span, SpanStyle, VecRows};
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ReplicaDialog {
    AllTabs,
    Profile,
    Appearance,
    Shortcuts,
    History,
}

impl ReplicaDialog {
    pub const ALL: [ReplicaDialog; 5] = [ReplicaDialog::AllTabs, ReplicaDialog::Profile, ReplicaDialog::Appearance, ReplicaDialog::Shortcuts, ReplicaDialog::History];

    pub fn slug(self) -> &'static str {
        match self {
            ReplicaDialog::AllTabs => "all-tabs",
            ReplicaDialog::Profile => "profile",
            ReplicaDialog::Appearance => "appearance",
            ReplicaDialog::Shortcuts => "shortcuts",
            ReplicaDialog::History => "history",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.slug() == slug)
    }
}

/// The interactive mockup: same data, defaults and behavior as the reference HTML.
pub struct ReplicaState {
    pub terms: Vec<Term>,
    pub regex: bool,
    pub case_sensitive: bool,
    pub context: usize,
    pub pins: BTreeSet<usize>,
    pub selected: Option<usize>,
    /// Shown in the footer only after the user clicks a row (the reference starts with an empty status).
    pub status_selection: Option<usize>,
    pub follow: bool,
    pub sidebar_visible: bool,
    pub tabs: Vec<&'static str>,
    pub active_tab: usize,
    pub dialog: Option<ReplicaDialog>,
    pub editing: Option<Entity<InputState>>,
    pub error: Option<SharedString>,
    pub top: Entity<LogViewState>,
    pub bottom: Entity<LogViewState>,
    pub find_top: Entity<InputState>,
    pub find_bottom: Entity<InputState>,
    pub filtered: Filtered,
    _subscriptions: Vec<Subscription>,
}

impl ReplicaState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let empty = || Rc::new(VecRows::new(vec![]));
        let top = cx.new(|_| LogViewState::new(empty()));
        let bottom = cx.new(|_| LogViewState::new(empty()));
        let find_top = FindField::state("Find in full log", window, cx);
        let find_bottom = FindField::state("Find in results", window, cx);
        let subscriptions = vec![
            cx.subscribe(&top, |this, _, ev: &LogViewEvent, cx| this.on_log_event(ev, false, cx)),
            cx.subscribe(&bottom, |this, _, ev: &LogViewEvent, cx| this.on_log_event(ev, true, cx)),
            cx.subscribe_in(&find_top, window, |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.rebuild(cx)
                }
            }),
            cx.subscribe_in(&find_bottom, window, |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.rebuild(cx)
                }
            }),
        ];
        let mut this = Self {
            terms: data::reference_terms(),
            regex: false,
            case_sensitive: false,
            context: 0,
            pins: BTreeSet::from([7]),
            selected: Some(12),
            status_selection: None,
            follow: true,
            sidebar_visible: true,
            tabs: vec!["api.log", "worker.log", "gateway.log"],
            active_tab: 0,
            dialog: None,
            editing: None,
            error: None,
            top,
            bottom,
            find_top,
            find_bottom,
            filtered: Filtered::default(),
            _subscriptions: subscriptions,
        };
        this.rebuild(cx);
        this
    }

    fn query(input: &Entity<InputState>, cx: &App) -> String {
        input.read(cx).value().to_string().to_ascii_lowercase()
    }

    fn line(&self, i: usize, bottom: bool, context: bool, query: &str) -> LogLine {
        let text = data::line_text(i);
        let mut spans = vec![Span { range: 0..STAMP_LEN, style: SpanStyle::Faint }];
        if bottom {
            for (r, slot) in data::hit_spans(&text[STAMP_LEN..], &self.terms, self.case_sensitive) {
                spans.push(Span { range: r.start + STAMP_LEN..r.end + STAMP_LEN, style: SpanStyle::Hit(slot) });
            }
        }
        if !query.is_empty() {
            let lower = text.to_ascii_lowercase();
            let mut from = 0;
            while let Some(at) = lower[from..].find(query) {
                spans.push(Span { range: from + at..from + at + query.len(), style: SpanStyle::SearchHit });
                from += at + query.len();
            }
        }
        LogLine {
            number: FIRST_LINE_NUMBER + i as u64,
            text: text.into(),
            spans,
            pinned: self.pins.contains(&i),
            context,
            tint: (SAMPLES[i].0 == "ERROR").then_some(RowTint::Error),
        }
    }

    pub fn rebuild(&mut self, cx: &mut Context<Self>) {
        self.filtered = data::filter(&self.terms, &self.pins, self.context, self.case_sensitive);
        let q_top = Self::query(&self.find_top, cx);
        let q_bottom = Self::query(&self.find_bottom, cx);
        let top_rows: Vec<LogRow> = (0..SAMPLES.len()).map(|i| LogRow::Line(self.line(i, false, false, &q_top))).collect();
        let mut bottom_rows = Vec::new();
        let mut prev: Option<usize> = None;
        for &i in &self.filtered.visible {
            if let Some(prev) = prev {
                if i > prev + 1 {
                    bottom_rows.push(LogRow::Gap { omitted: (i - prev - 1) as u64 });
                }
            }
            bottom_rows.push(LogRow::Line(self.line(i, true, !self.filtered.anchors.contains(&i), &q_bottom)));
            prev = Some(i);
        }
        let selected = self.selected.map(|i| FIRST_LINE_NUMBER + i as u64);
        self.top.update(cx, |s, cx| {
            s.set_rows(Rc::new(VecRows::new(top_rows)), cx);
            s.set_selected(selected, cx);
        });
        self.bottom.update(cx, |s, cx| {
            s.set_rows(Rc::new(VecRows::new(bottom_rows)), cx);
            s.set_selected(selected, cx);
        });
        cx.notify();
    }

    fn on_log_event(&mut self, ev: &LogViewEvent, from_bottom: bool, cx: &mut Context<Self>) {
        match ev {
            LogViewEvent::Selected { number, .. } => {
                let i = (*number - FIRST_LINE_NUMBER) as usize;
                self.selected = Some(i);
                self.status_selection = Some(i);
                if from_bottom && self.follow {
                    if let Some(row) = self.top.read(cx).rows().row_for_number(*number) {
                        self.top.update(cx, |s, cx| s.center_on_row(row, cx));
                    }
                }
                self.rebuild(cx);
            }
            LogViewEvent::TogglePin { number } => self.toggle_pin((*number - FIRST_LINE_NUMBER) as usize, cx),
        }
    }

    pub fn toggle_pin(&mut self, line: usize, cx: &mut Context<Self>) {
        if !self.pins.remove(&line) {
            self.pins.insert(line);
        }
        self.rebuild(cx);
    }

    pub fn cycle(&mut self, i: usize, cx: &mut Context<Self>) {
        if let Some(t) = self.terms.get_mut(i) {
            t.state = t.state.next();
        }
        self.rebuild(cx);
    }

    pub fn set_context(&mut self, n: usize, cx: &mut Context<Self>) {
        self.context = n;
        self.rebuild(cx);
    }

    pub fn toggle_regex(&mut self, cx: &mut Context<Self>) {
        self.regex = !self.regex;
        cx.notify();
    }

    pub fn toggle_case(&mut self, cx: &mut Context<Self>) {
        self.case_sensitive = !self.case_sensitive;
        self.rebuild(cx);
    }

    pub fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        self.follow = !self.follow;
        cx.notify();
    }

    pub fn toggle_wrap(&mut self, bottom: bool, cx: &mut Context<Self>) {
        let state = if bottom { self.bottom.clone() } else { self.top.clone() };
        state.update(cx, |s, cx| {
            let wrap = !s.wrap();
            s.set_wrap(wrap, cx)
        });
        cx.notify();
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    pub fn select_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        self.active_tab = i.min(self.tabs.len().saturating_sub(1));
        cx.notify();
    }

    pub fn close_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 && i < self.tabs.len() {
            self.tabs.remove(i);
            // Reference closeTab: only closing the active tab re-selects (the tab now at its index);
            // closing to the left shifts the active index so the same tab stays active.
            if i < self.active_tab {
                self.active_tab -= 1;
            } else if i == self.active_tab {
                self.active_tab = i.min(self.tabs.len() - 1);
            }
        }
        cx.notify();
    }

    pub fn open_dialog(&mut self, d: ReplicaDialog, cx: &mut Context<Self>) {
        self.dialog = Some(d);
        cx.notify();
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }

    pub fn start_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.terms.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" || ");
        self.editing = Some(cx.new(|cx| InputState::new(window, cx).default_value(text)));
        cx.notify();
    }

    /// Splits on `||`, keeps the state of terms whose text is unchanged, new terms start as Include.
    pub fn apply_edit(&mut self, cx: &mut Context<Self>) {
        if let Some(input) = self.editing.take() {
            let value = input.read(cx).value().to_string();
            self.terms = value
                .split("||")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|text| Term { text: text.to_string(), state: self.terms.iter().find(|t| t.text == text).map(|t| t.state).unwrap_or(ChipState::Include) })
                .collect();
            self.error = None;
        }
        self.rebuild(cx);
    }

    pub fn summary(&self) -> String {
        let count = |s: ChipState| self.terms.iter().filter(|t| t.state == s).count();
        format!("{} include · {} exclude · {} off", count(ChipState::Include), count(ChipState::Exclude), count(ChipState::Off))
    }

    pub fn stats(&self) -> Vec<StatPart> {
        let added = self.pins.iter().filter(|p| !self.filtered.matched.contains(p)).count();
        let context = self.filtered.visible.iter().filter(|i| !self.filtered.anchors.contains(i)).count();
        let mut parts = vec![
            StatPart::Strong(self.filtered.matched.len().to_string().into()),
            StatPart::Text(format!(" matches / {} lines · ", SAMPLES.len()).into()),
            StatPart::Strong(self.pins.len().to_string().into()),
            StatPart::Text(" pinned".into()),
        ];
        if added > 0 {
            parts.push(StatPart::Text(format!(" ({added} added)").into()));
        }
        parts.push(StatPart::Text(" · ".into()));
        parts.push(StatPart::Strong(context.to_string().into()));
        parts.push(StatPart::Text(" context".into()));
        parts
    }

    /// `N hits` for a panel's find field (rows containing the query), or None when empty.
    pub fn hit_count(&self, bottom: bool, cx: &App) -> Option<SharedString> {
        let q = Self::query(if bottom { &self.find_bottom } else { &self.find_top }, cx);
        if q.is_empty() {
            return None;
        }
        let rows: Vec<usize> = if bottom { self.filtered.visible.clone() } else { (0..SAMPLES.len()).collect() };
        let n = rows.iter().filter(|&&i| data::line_text(i).to_ascii_lowercase().contains(&q)).count();
        Some(format!("{n} hits").into())
    }
}
