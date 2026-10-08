//! One open log in the window: its document, the two log panes with their find fields, selection, follow and
//! the expression editor (spec §5.6).

use crate::document::{Document, DocumentEvent, DocumentInit, Pane, Source};
use crate::profiles::{ProfileStore, RowStyler};
use crate::settings::SettingsStore;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext, Context, Entity, EntityId, EventEmitter, FocusHandle, Focusable, SharedString, Subscription, Window};
use rogg_core::stats::{group_digits, noun};
use rogg_ui::controls::FindField;
use rogg_ui::layout::MainWindowParts;
use rogg_ui::logview::{LogRowSource, LogViewEvent, LogViewState, ScrollTo, VecRows};
use rogg_ui::theme::{ActiveRoggTheme, Appearance};
use std::rc::Rc;
use std::sync::Arc;

/// Labels the demo sets to match the reference; None means "derive from the document".
#[derive(Clone, Debug, Default)]
pub struct TabLabels {
    pub crumbs: Option<Vec<SharedString>>,
    pub profile: Option<SharedString>,
    pub note: Option<SharedString>,
    pub full_count: Option<SharedString>,
    pub tooltip: Option<SharedString>,
}

#[derive(Clone, Debug)]
pub struct TabInit {
    pub doc: DocumentInit,
    /// Display number to select in both panes once the text loads (not reported in the status bar until the user
    /// selects, as in the reference).
    pub selection: Option<u64>,
    pub labels: TabLabels,
    /// Wrap of the full and the filtered log.
    pub wrap: [bool; 2],
    /// Follow selection (spec §5.6).
    pub follow: bool,
    /// Split ratio; None keeps the default.
    pub split: Option<f32>,
    /// Display number to scroll to the top of each pane once the text loads (session restore).
    pub top: [Option<u64>; 2],
    /// The tab's highlighting profile; None takes the settings' default profile.
    pub profile: Option<SharedString>,
    /// A derived tab's parent tabs, one per breadcrumb segment before its own (None where no tab is behind it).
    pub crumb_tabs: Vec<Option<EntityId>>,
}

impl Default for TabInit {
    fn default() -> Self {
        Self {
            doc: DocumentInit::default(),
            selection: None,
            labels: TabLabels::default(),
            wrap: [false, false],
            follow: true,
            split: None,
            top: [None, None],
            profile: None,
            crumb_tabs: Vec::new(),
        }
    }
}

impl TabInit {
    /// A tab for a file the user opened: filter flags, context, wrap and follow from the settings (spec §6.2).
    pub fn from_settings(settings: &rogg_core::settings::Settings) -> Self {
        let b = &settings.behavior;
        let spec = rogg_core::filter::FilterSpec { terms: Vec::new(), regex: b.regex, case_sensitive: b.case_sensitive, context: b.context };
        Self { doc: DocumentInit { spec, ..DocumentInit::default() }, wrap: [b.wrap, b.wrap], follow: b.follow_selection, profile: Some(b.default_profile.clone().into()), ..Self::default() }
    }
}

pub enum TabEvent {
    Toast(SharedString),
    ShowHistory,
    /// The info bar's Profile button: open the highlighting profile editor.
    ShowProfiles,
    /// The footer's statistics area: open the statistics popover.
    ShowStatistics,
    /// A breadcrumb segment was clicked: focus the tab behind it.
    FocusCrumb(usize),
    /// The filtered pane's "Open as tab" button.
    OpenAsTab,
    /// The info bar's Export button: open the Export dialog.
    ShowExport,
    /// The context menu's "Show full line…": the whole text of the line.
    ShowFullLine { number: u64, text: SharedString },
}

/// Spec §4.5 reload toast.
pub const RELOADED_TOAST: &str = "File was truncated or replaced — reloaded.";

/// Live follow (spec §4.5): following keeps both panes at their end as lines arrive; paused counts what arrived.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiveFollow {
    pub following: bool,
    /// Lines that arrived since following was paused (`+1,204` on the control).
    pub new_lines: u64,
}

/// Document revisions already turned into rows.
#[derive(Clone, Copy, Default, PartialEq)]
struct Applied {
    load: u64,
    text: u64,
    pins: u64,
    filter: u64,
    search: [u64; 2],
    style: u64,
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
    pub live: LiveFollow,
    live_started: bool,
    /// Whether the full log's end was in view just before its rows first grew (read before `extend_rows`, which
    /// moves the end away); consumed by the first `Grew`.
    end_in_view: Option<bool>,
    pub split: f32,
    pub labels: TabLabels,
    /// The pane ⌘F, ⌘G and Wrap act on.
    pub last_pane: Pane,
    /// The tab's highlighting profile (spec §6.1: each tab has one).
    pub profile: SharedString,
    /// The tabs behind each breadcrumb segment except the last (a derived tab's parents).
    pub crumb_tabs: Vec<Option<EntityId>>,
    self_id: EntityId,
    styler: Option<Arc<RowStyler>>,
    /// What `styler` was built from: the profile store's revision, the theme and the profile name.
    style_key: Option<(u64, Appearance, SharedString)>,
    style_revision: u64,
    selection: Option<u64>,
    /// False while `selection` is only the initial one (demo, `--line`): the footer stays empty until the user selects.
    selection_announced: bool,
    pending_selection: Option<u64>,
    /// Rows to scroll to the top of each pane once the text loads.
    pending_top: [Option<u64>; 2],
    applied: Applied,
    /// The filter spec the rows were last synced against: a change (not just a rescan or growth, which bump the
    /// filter revision too) clears the character selections.
    applied_spec: Option<rogg_core::filter::FilterSpec>,
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
            cx.subscribe(&doc, |tab, _, ev: &DocumentEvent, cx| tab.on_doc_event(ev, cx)),
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
            follow: init.follow,
            live: LiveFollow::default(),
            live_started: false,
            end_in_view: None,
            split: init.split.unwrap_or(MainWindowParts::DEFAULT_SPLIT),
            labels: init.labels,
            last_pane: Pane::Full,
            // A name no profile has (a stale default-profile setting) is Default, as on session restore.
            profile: Some(init.profile.unwrap_or_else(|| SettingsStore::get(cx).behavior.default_profile.into()))
                .filter(|p| cx.try_global::<ProfileStore>().is_none_or(|s| s.contains(p)))
                .unwrap_or_else(|| rogg_core::profile::DEFAULT_PROFILE.into()),
            crumb_tabs: init.crumb_tabs,
            self_id: cx.entity_id(),
            styler: None,
            style_key: None,
            style_revision: 0,
            selection: None,
            selection_announced: false,
            pending_selection: init.selection,
            pending_top: init.top,
            applied: Applied::default(),
            applied_spec: None,
            edit_subscription: None,
            _subscriptions: subscriptions,
        };
        if init.wrap[0] {
            tab.full.update(cx, |s, cx| s.set_wrap(true, cx));
        }
        if init.wrap[1] {
            tab.filtered.update(cx, |s, cx| s.set_wrap(true, cx));
        }
        if matches!(tab.doc.read(cx).source, Source::Stream { .. }) {
            tab.live_started = true;
            tab.set_following(true, cx);
        }
        tab.refresh_style(cx);
        tab.sync_rows(cx);
        tab
    }

    /// Rebuilds the row styler when the profile, its contents (the store revision) or the theme changed; the rows of
    /// both panes are rebuilt with it.
    pub fn refresh_style(&mut self, cx: &mut Context<Self>) {
        let key = (ProfileStore::revision(cx), cx.rogg().appearance, self.profile.clone());
        if self.style_key.as_ref() == Some(&key) {
            return;
        }
        self.styler = Some(Arc::new(RowStyler::for_profile(cx, &self.profile)));
        self.style_key = Some(key);
        self.style_revision += 1;
        self.sync_rows(cx);
    }

    /// The tab's highlighting profile (spec §6.1: each tab has one).
    pub fn set_profile(&mut self, name: SharedString, cx: &mut Context<Self>) {
        self.profile = name;
        self.refresh_style(cx);
        cx.notify();
    }

    pub fn styler(&self) -> Option<Arc<RowStyler>> {
        self.styler.clone()
    }

    fn on_doc_event(&mut self, ev: &DocumentEvent, cx: &mut Context<Self>) {
        match ev {
            DocumentEvent::Reloaded => {
                // A reloaded file is not live until it grows again; a stream keeps following.
                self.live = LiveFollow::default();
                self.end_in_view = None;
                let stream = matches!(self.doc.read(cx).source, Source::Stream { .. });
                self.live_started = stream;
                self.set_following(stream, cx);
                cx.emit(TabEvent::Toast(RELOADED_TOAST.into()));
            }
            DocumentEvent::Grew { lines } => {
                let end_in_view = self.end_in_view.take();
                if !self.doc.read(cx).is_live() {
                    return;
                }
                if !self.live_started {
                    // A file that starts growing follows when its end was in view (`tail -f`); else it waits, counting.
                    self.live_started = true;
                    let following = end_in_view.unwrap_or_else(|| self.full.read(cx).at_bottom());
                    self.set_following(following, cx);
                    if !following {
                        self.live.new_lines = *lines;
                    }
                } else if !self.live.following {
                    self.live.new_lines += lines;
                }
                cx.notify();
            }
            DocumentEvent::Exported(Ok((n, name))) => {
                cx.emit(TabEvent::Toast(format!("Exported {} {} to {name}", group_digits(*n), noun(*n, "line", "lines")).into()))
            }
            DocumentEvent::Exported(Err(e)) => cx.emit(TabEvent::Toast(format!("Could not export: {e}").into())),
        }
    }

    fn set_following(&mut self, on: bool, cx: &mut Context<Self>) {
        self.live.following = on;
        if on {
            self.live.new_lines = 0;
        }
        self.full.update(cx, |s, cx| s.set_tail(on, cx));
        self.filtered.update(cx, |s, cx| s.set_tail(on, cx));
        cx.notify();
    }

    /// Space / the live control: pause or resume following (live documents only).
    pub fn toggle_live(&mut self, cx: &mut Context<Self>) {
        if self.doc.read(cx).is_live() {
            let on = !self.live.following;
            self.set_following(on, cx);
        }
    }

    pub fn title(&self, cx: &App) -> SharedString {
        self.doc.read(cx).title()
    }

    /// The info-bar breadcrumb: the demo's label when set, else the document's.
    pub fn crumbs(&self, cx: &App) -> Vec<SharedString> {
        self.labels.crumbs.clone().unwrap_or_else(|| self.doc.read(cx).source.crumbs())
    }

    /// The tab behind breadcrumb segment `i`: this tab for the last segment, the parent tabs of a derived tab before it.
    pub fn crumb_tab(&self, i: usize, cx: &App) -> Option<EntityId> {
        let last = self.crumbs(cx).len().saturating_sub(1);
        if i == last { Some(self.self_id) } else { self.crumb_tabs.get(i).copied().flatten() }
    }

    /// Breadcrumb tabs for a tab derived from this one: one entry per segment of this tab's breadcrumb.
    pub fn crumb_tabs_for_child(&self, cx: &App) -> Vec<Option<EntityId>> {
        let n = self.crumbs(cx).len();
        (0..n).map(|i| self.crumb_tab(i, cx)).collect()
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

    /// The selection a session stores: a restored one not applied yet (the text is still loading or failed) counts.
    pub fn session_selection(&self) -> Option<u64> {
        self.pending_selection.or(self.selection)
    }

    /// The top line of each pane a session stores: a restored one not applied yet counts.
    pub fn session_top(&self, cx: &App) -> [Option<u64>; 2] {
        [Pane::Full, Pane::Filtered].map(|pane| self.pending_top_of(pane, cx).or_else(|| self.state(pane).read(cx).top_number()))
    }

    /// A restored top line still to apply; void once the user has scrolled that pane (it would yank the view back).
    fn pending_top_of(&self, pane: Pane, cx: &App) -> Option<u64> {
        self.pending_top[pane.index()].filter(|_| self.state(pane).read(cx).user_scrolls() == 0)
    }

    /// The display number the user last selected in either pane (shown as `Line N`); none for an initial selection.
    pub fn announced_selection(&self) -> Option<u64> {
        self.selection.filter(|_| self.selection_announced)
    }

    /// Rebuilds the row sources whose document data changed: the full log on text, pins or its search; the filtered log
    /// on text, the filtered view or its search. When only the text grew, the full log's rows are extended (keeping
    /// measured wrapped heights).
    pub fn sync_rows(&mut self, cx: &mut Context<Self>) {
        let (now, full_rows, filtered_rows) = {
            let doc = self.doc.read(cx);
            let now = Applied {
                load: doc.load_revision(),
                text: doc.text_revision(),
                pins: doc.filter.pins_revision,
                filter: doc.filter.revision,
                search: [doc.search(Pane::Full).revision, doc.search(Pane::Filtered).revision],
                style: self.style_revision,
            };
            let a = self.applied;
            let full_changed = now.text != a.text || now.pins != a.pins || now.search[0] != a.search[0] || now.style != a.style;
            let filtered_changed = now.text != a.text || now.filter != a.filter || now.search[1] != a.search[1] || now.style != a.style;
            (now, full_changed.then(|| doc.full_rows(self.styler.clone())).flatten(), filtered_changed.then(|| doc.filtered_rows(self.styler.clone())).flatten())
        };
        let spec_changed = {
            let spec = &self.doc.read(cx).filter.spec;
            let changed = self.applied_spec.as_ref() != Some(spec);
            if changed {
                self.applied_spec = Some(spec.clone());
            }
            changed
        };
        let before = self.applied;
        let loaded = now.load != before.load;
        let only_grew = !loaded && now.pins == before.pins && now.search[0] == before.search[0] && now.style == before.style;
        self.applied = now;
        if let Some(rows) = full_rows {
            if only_grew && !self.live_started && self.end_in_view.is_none() {
                self.end_in_view = Some(self.full.read(cx).at_bottom());
            }
            self.full.update(cx, |s, cx| if only_grew { s.extend_rows(rows, cx) } else { s.set_rows(rows, cx) });
        }
        if let Some(rows) = filtered_rows {
            self.filtered.update(cx, |s, cx| s.set_rows(rows, cx));
            // The tab's selection may become shown only now (a background scan finished, a filter changed): select it
            // in the filtered pane too, without scrolling (M2 carry-over).
            if let Some(number) = self.selection
                && self.filtered.read(cx).selected().is_none()
                && self.filtered.read(cx).rows().row_for_number(number).is_some()
            {
                self.filtered.update(cx, |s, cx| s.set_selected(Some(number), cx));
            }
        }
        if spec_changed {
            // A new filter clears the character selections (spec B3), in both panes.
            self.full.update(cx, |s, cx| s.clear_char_selection(cx));
            self.filtered.update(cx, |s, cx| s.clear_char_selection(cx));
        }
        // A restored top line: the full log's rows are complete once the text loads; the filtered log's only once its
        // filter has settled (a background scan may still be running at load, with no rows yet).
        let ready = [loaded, self.doc.read(cx).filter_settled()];
        for (pane, ready) in [Pane::Full, Pane::Filtered].into_iter().zip(ready) {
            if !ready {
                continue;
            }
            let state = self.state(pane).clone();
            let top = self.pending_top_of(pane, cx);
            self.pending_top[pane.index()] = None;
            if let Some(row) = top.and_then(|n| state.read(cx).rows().nearest_row(n)) {
                state.update(cx, |s, cx| s.scroll_to_row(row, ScrollTo::Top, cx));
            }
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
            LogViewEvent::Selected { number, .. } => {
                // The user moved in this pane (a click, the keyboard): a restored top must not yank it back.
                self.pending_top[pane.index()] = None;
                self.sync_selection(pane, *number, cx)
            }
            LogViewEvent::TogglePin { number } => self.toggle_pin(*number, cx),
            LogViewEvent::AddTerm { text, exclude } => {
                let (text, exclude) = (text.clone(), *exclude);
                self.doc.update(cx, |d, cx| d.add_term(&text, exclude, cx));
            }
            LogViewEvent::ShowFullLine { number } => {
                let state = if pane == Pane::Full { &self.full } else { &self.filtered };
                let rows = state.read(cx).rows().clone();
                if let Some(text) = rows.row_for_number(*number).and_then(|row| rows.copy_text(row)) {
                    cx.emit(TabEvent::ShowFullLine { number: *number, text: text.into() });
                }
            }
            LogViewEvent::TailLeft => {
                if pane == Pane::Full && self.live.following {
                    self.live.following = false;
                    self.filtered.update(cx, |s, cx| s.set_tail(false, cx));
                    cx.notify();
                }
            }
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
