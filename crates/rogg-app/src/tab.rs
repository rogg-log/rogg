//! One open log in the window: its document, the two log panes with their find fields, selection, follow and
//! the expression editor (spec §5.6).

use crate::document::{Document, DocumentInit, Pane, Source};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, SharedString, Subscription, Window};
use rogg_ui::controls::FindField;
use rogg_ui::layout::MainWindowParts;
use rogg_ui::logview::{LogRowSource, LogViewEvent, LogViewState, ScrollTo, VecRows};
use std::rc::Rc;

/// Labels the demo sets to match the reference; None means "derive from the document".
#[derive(Clone, Debug, Default)]
pub struct TabLabels {
    pub crumbs: Option<Vec<SharedString>>,
    pub profile: Option<SharedString>,
    pub note: Option<SharedString>,
    pub full_count: Option<SharedString>,
    pub tooltip: Option<SharedString>,
}

#[derive(Clone, Debug, Default)]
pub struct TabInit {
    pub doc: DocumentInit,
    /// Display number to select in both panes once the text loads (not reported in the status bar until the user
    /// selects, as in the reference).
    pub selection: Option<u64>,
    pub labels: TabLabels,
}

pub enum TabEvent {
    Toast(SharedString),
    ShowHistory,
}

/// Document revisions already turned into rows.
#[derive(Clone, Copy, Default, PartialEq)]
struct Applied {
    text: u64,
    filter: u64,
    search: [u64; 2],
}

pub struct Tab {
    pub doc: Entity<Document>,
    pub full: Entity<LogViewState>,
    pub filtered: Entity<LogViewState>,
    /// Tab-cycle order: full log, filter line, filtered log.
    pub focus: [FocusHandle; 3],
    pub find: [Entity<InputState>; 2],
    pub editing: Option<Entity<InputState>>,
    pub follow: bool,
    pub split: f32,
    pub labels: TabLabels,
    /// The pane ⌘F, ⌘G and Wrap act on.
    pub last_pane: Pane,
    selection: Option<u64>,
    /// False while `selection` is only the initial one (demo, `--line`): the footer stays empty until the user selects.
    selection_announced: bool,
    pending_selection: Option<u64>,
    applied: Applied,
    edit_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<TabEvent> for Tab {}

impl Tab {
    pub fn new(source: Source, init: TabInit, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let doc = cx.new(|cx| Document::new(source, init.doc, cx));
        let empty: Rc<dyn LogRowSource> = Rc::new(VecRows::new(Vec::new()));
        let full = cx.new(|_| LogViewState::new(empty.clone()));
        let filtered = cx.new(|_| LogViewState::new(empty));
        let find = [FindField::state("Find in full log", window, cx), FindField::state("Find in results", window, cx)];
        let subscriptions = vec![
            cx.observe(&doc, |tab, _, cx| tab.sync_rows(cx)),
            cx.subscribe(&full, |tab, _, ev: &LogViewEvent, cx| tab.on_log_event(Pane::Full, ev, cx)),
            cx.subscribe(&filtered, |tab, _, ev: &LogViewEvent, cx| tab.on_log_event(Pane::Filtered, ev, cx)),
            cx.subscribe_in(&find[0], window, |tab, input, ev: &InputEvent, _, cx| tab.on_find_event(Pane::Full, input, ev, cx)),
            cx.subscribe_in(&find[1], window, |tab, input, ev: &InputEvent, _, cx| tab.on_find_event(Pane::Filtered, input, ev, cx)),
        ];
        let mut tab = Self {
            doc,
            full,
            filtered,
            focus: [cx.focus_handle(), cx.focus_handle(), cx.focus_handle()],
            find,
            editing: None,
            follow: true,
            split: MainWindowParts::DEFAULT_SPLIT,
            labels: init.labels,
            last_pane: Pane::Full,
            selection: None,
            selection_announced: false,
            pending_selection: init.selection,
            applied: Applied::default(),
            edit_subscription: None,
            _subscriptions: subscriptions,
        };
        tab.sync_rows(cx);
        tab
    }

    pub fn title(&self, cx: &App) -> SharedString {
        self.doc.read(cx).title()
    }

    pub fn state(&self, pane: Pane) -> &Entity<LogViewState> {
        match pane {
            Pane::Full => &self.full,
            Pane::Filtered => &self.filtered,
        }
    }

    /// The selected display number: what ⌘B, Return and find-Enter act on. May be an initial selection the user has
    /// not touched yet.
    pub fn selection(&self) -> Option<u64> {
        self.selection
    }

    /// The display number the user last selected in either pane (shown as `Line N`); none for an initial selection.
    pub fn announced_selection(&self) -> Option<u64> {
        self.selection.filter(|_| self.selection_announced)
    }

    /// Rebuilds the row sources whose document data changed.
    pub fn sync_rows(&mut self, cx: &mut Context<Self>) {
        let (now, full_rows, filtered_rows) = {
            let doc = self.doc.read(cx);
            let now = Applied {
                text: doc.text_revision(),
                filter: doc.filter.revision,
                search: [doc.search(Pane::Full).revision, doc.search(Pane::Filtered).revision],
            };
            let base_changed = now.text != self.applied.text || now.filter != self.applied.filter;
            let full = (base_changed || now.search[0] != self.applied.search[0]).then(|| doc.full_rows()).flatten();
            let filtered = (base_changed || now.search[1] != self.applied.search[1]).then(|| doc.filtered_rows()).flatten();
            (now, full, filtered)
        };
        let loaded = now.text != self.applied.text;
        self.applied = now;
        if let Some(rows) = full_rows {
            self.full.update(cx, |s, cx| s.set_rows(rows, cx));
        }
        if let Some(rows) = filtered_rows {
            self.filtered.update(cx, |s, cx| s.set_rows(rows, cx));
        }
        if loaded && let Some(number) = self.pending_selection.take() {
            let in_filtered = self.filtered.read(cx).rows().row_for_number(number).is_some();
            self.full.update(cx, |s, cx| s.set_selected(Some(number), cx));
            self.filtered.update(cx, |s, cx| s.set_selected(in_filtered.then_some(number), cx));
            self.selection = Some(number);
            self.selection_announced = false;
        }
        cx.notify();
    }

    /// Selects display `number` in `pane` (centring it when `center`) and mirrors it into the other pane.
    pub fn select(&mut self, pane: Pane, number: u64, center: bool, cx: &mut Context<Self>) {
        let state = self.state(pane).clone();
        let row = state.read(cx).rows().row_for_number(number);
        state.update(cx, |s, cx| {
            s.set_selected(Some(number), cx);
            if center && let Some(row) = row {
                s.scroll_to_row(row, ScrollTo::Center, cx);
            }
        });
        self.sync_selection(pane, number, cx);
    }

    fn on_log_event(&mut self, pane: Pane, ev: &LogViewEvent, cx: &mut Context<Self>) {
        self.last_pane = pane;
        match ev {
            LogViewEvent::Selected { number, .. } => self.sync_selection(pane, *number, cx),
            LogViewEvent::TogglePin { number } => self.toggle_pin(*number, cx),
            LogViewEvent::AddTerm { text, exclude } => {
                let (text, exclude) = (text.clone(), *exclude);
                self.doc.update(cx, |d, cx| d.add_term(&text, exclude, cx));
            }
            LogViewEvent::TailLeft => {}
        }
    }

    /// Spec §5.6: a filtered selection reveals the line in the full log while following; a full-log selection
    /// selects the filtered row when it is shown (scrolling only if needed) and clears it otherwise.
    fn sync_selection(&mut self, from: Pane, number: u64, cx: &mut Context<Self>) {
        self.selection = Some(number);
        self.selection_announced = true;
        match from {
            Pane::Filtered if self.follow => {
                let row = self.full.read(cx).rows().row_for_number(number);
                self.full.update(cx, |s, cx| {
                    s.set_selected(Some(number), cx);
                    if let Some(row) = row {
                        s.scroll_to_row(row, ScrollTo::Center, cx);
                    }
                });
            }
            Pane::Filtered => {}
            Pane::Full => {
                let row = self.filtered.read(cx).rows().row_for_number(number);
                self.filtered.update(cx, |s, cx| {
                    s.set_selected(row.map(|_| number), cx);
                    if let Some(row) = row {
                        s.scroll_to_row(row, ScrollTo::Visible, cx);
                    }
                });
            }
        }
        cx.notify();
    }

    /// Pins or unpins display `number` in both panes.
    pub fn toggle_pin(&mut self, number: u64, cx: &mut Context<Self>) {
        let base = self.doc.read(cx).number_base();
        if let Some(line) = number.checked_sub(base) {
            self.doc.update(cx, |d, cx| d.toggle_pin(line, cx));
        }
    }

    /// ⌘B.
    pub fn pin_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(number) = self.selection {
            self.toggle_pin(number, cx);
        }
    }

    /// Return: centres the selected line in the full log.
    pub fn reveal_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(number) = self.selection {
            self.select(Pane::Full, number, true, cx);
        }
    }

    /// ⌘G: selects and centres display `number` in the full log; false when the log has no such line.
    pub fn go_to_line(&mut self, number: u64, cx: &mut Context<Self>) -> bool {
        if self.full.read(cx).rows().row_for_number(number).is_none() {
            return false;
        }
        self.last_pane = Pane::Full;
        self.select(Pane::Full, number, true, cx);
        true
    }

    /// Enter / Shift-Enter in a find field: the next / previous hit after the selection.
    pub fn jump_to_hit(&mut self, pane: Pane, backwards: bool, cx: &mut Context<Self>) {
        let base = self.doc.read(cx).number_base();
        let from = self.selection.and_then(|n| n.checked_sub(base));
        let Some(line) = self.doc.read(cx).next_hit(pane, from, backwards) else { return };
        self.last_pane = pane;
        self.select(pane, base + line, true, cx);
    }

    pub fn toggle_wrap(&mut self, pane: Pane, cx: &mut Context<Self>) {
        self.state(pane).clone().update(cx, |s, cx| {
            let wrap = !s.wrap();
            s.set_wrap(wrap, cx);
        });
        cx.notify();
    }

    /// True once the document's text is loaded, i.e. the panes, band and find fields are rendered (a loading or
    /// failed tab shows a placeholder instead, whose focus handles are attached to nothing).
    pub fn is_ready(&self, cx: &App) -> bool {
        self.doc.read(cx).text().is_some()
    }

    /// False while the filtered pane shows the no-matches state (see `tab_view`), where `focus[2]` is unattached.
    fn filtered_focusable(&self, cx: &App) -> bool {
        let doc = self.doc.read(cx);
        !doc.filter.view.shown_lines().is_empty() || doc.filter.progress.is_some()
    }

    /// True when focus sits on a handle that is not attached to a rendered element: keys would then bypass the
    /// workspace's actions, so the workspace takes focus back.
    pub fn has_unrendered_focus(&self, window: &Window, cx: &App) -> bool {
        if !self.is_ready(cx) {
            return self.has_focus(window, cx);
        }
        !self.filtered_focusable(cx) && self.focus[2].contains_focused(window, cx)
    }

    /// True when focus is inside one of the tab's regions, find fields or the expression editor.
    pub fn has_focus(&self, window: &Window, cx: &App) -> bool {
        let inputs = self.find.iter().map(|i| i.focus_handle(cx)).chain(self.editing.iter().map(|i| i.focus_handle(cx)));
        self.focus.iter().cloned().chain(inputs).any(|f| f.contains_focused(window, cx))
    }

    /// ⌘F: focuses the find field of the pane used last.
    pub fn focus_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_ready(cx) {
            return;
        }
        let input = self.find[self.last_pane.index()].clone();
        input.update(cx, |i, cx| i.focus(window, cx));
    }

    /// Tab / ⇧Tab: full log → filter line → filtered log (spec §5.9). The filtered log is skipped while its pane
    /// shows the no-matches state, where its focus handle is attached to nothing.
    pub fn cycle_focus(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_ready(cx) {
            return;
        }
        let filtered_focusable = self.filtered_focusable(cx);
        let current = self.focus.iter().position(|f| f.contains_focused(window, cx));
        let mut next = match current {
            None => 0,
            Some(i) if forward => (i + 1) % 3,
            Some(i) => (i + 2) % 3,
        };
        if next == 2 && !filtered_focusable {
            next = if forward { 0 } else { 1 };
        }
        self.focus[next].focus(window, cx);
        match next {
            0 => self.last_pane = Pane::Full,
            2 => self.last_pane = Pane::Filtered,
            _ => {}
        }
        cx.notify();
    }

    pub fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        self.follow = !self.follow;
        cx.notify();
    }

    fn on_find_event(&mut self, pane: Pane, input: &Entity<InputState>, ev: &InputEvent, cx: &mut Context<Self>) {
        match ev {
            InputEvent::Change => {
                let query = input.read(cx).value().to_string();
                self.doc.update(cx, |d, cx| d.set_search(pane, &query, cx));
            }
            InputEvent::PressEnter { shift, .. } => self.jump_to_hit(pane, *shift, cx),
            InputEvent::Focus => self.last_pane = pane,
            _ => {}
        }
    }

    /// Edit mode (edit button, ⌘L, Enter on the filter line): the chips become an expression input.
    pub fn start_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() || !self.is_ready(cx) {
            return;
        }
        let text = self.doc.read(cx).filter.spec.expression();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        input.update(cx, |i, cx| i.focus(window, cx));
        self.edit_subscription = Some(cx.subscribe_in(&input, window, |tab, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                tab.apply_edit(window, cx);
            }
        }));
        self.editing = Some(input);
        cx.notify();
    }

    /// Applies the edited expression; an invalid one keeps the editor open with the error under it.
    pub fn apply_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self.editing.clone() else { return };
        let text = input.read(cx).value().to_string();
        if self.doc.update(cx, |d, cx| d.set_expression(&text, cx)).is_ok() {
            self.cancel_edit(window, cx);
        }
    }

    /// Escape: closes the editor, keeps the applied filter and clears a rejected expression's error.
    pub fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = None;
        self.edit_subscription = None;
        self.doc.update(cx, |d, cx| d.clear_filter_error(cx));
        self.focus[1].focus(window, cx);
        cx.notify();
    }
}
