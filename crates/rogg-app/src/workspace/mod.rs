//! The main window view: title bar, tabs, sidebar, dialogs, toasts and the app's actions.

pub mod chrome;
pub mod keys;

use crate::actions::{self, ActivateTab, CloseTab, Dismiss, Export, NextTab, OpenAsTab, OpenFile, OpenFolder, OpenRecent, OpenSettings, PrevTab, ShowAbout, ShowShortcuts, ToggleSidebar};
use crate::args::Args;
use crate::document::Source;
use crate::history::HistoryStore;
use crate::recent::RecentStore;
use crate::sidebar_model::FixedSidebar;
use crate::tab::{Tab, TabEvent, TabInit};
use crate::tab_view::{self, TabBody};
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Div, DragMoveEvent, Entity, EntityId, ExternalPaths, FocusHandle, Pixels, ScrollHandle, SharedString, Stateful, Subscription,
    Window, div, px,
};
use rogg_ui::dialogs::{self, ModalLayer};
use rogg_ui::empty;
use rogg_ui::layout::{BandDrag, LayoutMeasure, MainWindowLayout, MainWindowParts, SidebarDrag, SingleParts};
use crate::settings::SettingsStore;
use rogg_core::settings::{DensitySetting, ThemeSetting};
use rogg_ui::theme::size;
use rogg_ui::titlebar::{TabItem, TitleBar};
use rogg_ui::toast::ToastStack;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

/// How long a toast stays up.
const TOAST_TIME: Duration = Duration::from_secs(4);

#[derive(Clone)]
pub enum Dialog {
    Shortcuts,
    AllTabs,
    History,
    GoToLine(Entity<InputState>),
    Settings,
    FontPicker,
    Profiles(Entity<crate::dialogs::profiles::ProfileEditor>),
    Statistics,
    Export,
    About,
    FullLine { number: u64, text: SharedString, view: Entity<TextareaState>, cut: bool },
}

pub struct Workspace {
    pub(crate) focus: FocusHandle,
    pub sidebar_visible: bool,
    pub sidebar_width: Pixels,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) tabs: Vec<Entity<Tab>>,
    pub(crate) active: usize,
    tab_subscriptions: HashMap<EntityId, Vec<Subscription>>,
    pub(crate) measure: Rc<LayoutMeasure>,
    pub toasts: Vec<(u64, SharedString)>,
    next_toast: u64,
    traffic_lights: bool,
    pub(crate) tab_scroll: ScrollHandle,
    pub(crate) collapsed: HashSet<SharedString>,
    pub(crate) fixed_sidebar: Option<FixedSidebar>,
    pub(crate) sources: crate::sources::SourcesState,
    /// Unfinished captures of stdin and FIFOs opened in this window, with their display names; polled for errors.
    pub(crate) streams: Vec<(SharedString, rogg_core::stream::StreamCapture)>,
    streams_polling: bool,
    stdin_opened: bool,
    /// A theme and density pinned by the host (the gallery), not saved in the settings.
    pub(crate) theme_override: Option<(ThemeSetting, DensitySetting)>,
    goto_subscription: Option<Subscription>,
    /// The open dialog entity's events (the profile editor); dropped when the dialog closes.
    dialog_subscription: Option<Subscription>,
    /// A dialog just closed: its input's focus handle outlives it until the frame is replaced, so check again after.
    refocus_after_dialog: bool,
    _appearance_subscription: Subscription,
    /// The window's frame as last observed, for the session.
    last_window: Option<rogg_core::session::WindowState>,
    /// Frame changes and app quit (the session is saved on quit).
    _session_subscriptions: Vec<Subscription>,
    /// Why the last session save failed; None after a successful one. A failure is toasted once, not every 30 s.
    pub session_error: Option<String>,
    /// Why the last settings save failed; None after a successful one. Toasted once, like `session_error`.
    pub settings_error: Option<String>,
    /// The last profile save's failure message; None after a successful one. Toasted once.
    pub profile_error: Option<String>,
    /// The Export dialog's line options (spec §6.5).
    pub export_options: rogg_core::export::ExportOptions,
    /// The Export dialog's session scope: all tabs, or only the active one.
    pub export_all_tabs: bool,
    /// The folder last shown in Finder (About › Show in Finder), remembered for tests.
    last_revealed: Option<PathBuf>,
    /// Whether `reveal` asks the OS to show the folder. Tests turn it off: gpui's test platform panics on it.
    #[doc(hidden)]
    pub reveal_in_finder: bool,
}

/// A `Fn(&mut Window, &mut App)` callback that runs `f` on the workspace.
pub(crate) fn act(this: &Entity<Workspace>, f: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let this = this.clone();
    move |window, cx| this.update(cx, |ws, cx| f(ws, window, cx))
}

/// Like `act`, for callbacks carrying an index.
pub(crate) fn act_ix(
    this: &Entity<Workspace>,
    f: impl Fn(&mut Workspace, usize, &mut Window, &mut Context<Workspace>) + 'static,
) -> impl Fn(usize, &mut Window, &mut App) + 'static {
    let this = this.clone();
    move |i, window, cx| this.update(cx, |ws, cx| f(ws, i, window, cx))
}

impl Workspace {
    pub fn new(args: Args, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut ws = Self {
            focus,
            sidebar_visible: true,
            sidebar_width: px(size::SIDEBAR_W),
            dialog: None,
            tabs: Vec::new(),
            active: 0,
            tab_subscriptions: HashMap::new(),
            measure: Rc::new(LayoutMeasure::default()),
            toasts: Vec::new(),
            next_toast: 0,
            traffic_lights: false,
            tab_scroll: ScrollHandle::new(),
            collapsed: HashSet::new(),
            fixed_sidebar: None,
            sources: Default::default(),
            streams: Vec::new(),
            streams_polling: false,
            stdin_opened: false,
            theme_override: None,
            goto_subscription: None,
            dialog_subscription: None,
            refocus_after_dialog: false,
            last_window: None,
            _session_subscriptions: Vec::new(),
            session_error: None,
            settings_error: None,
            profile_error: None,
            export_options: rogg_core::export::ExportOptions { context: true, gaps: true, numbers: false },
            export_all_tabs: false,
            last_revealed: None,
            reveal_in_finder: true,
            _appearance_subscription: cx.observe_window_appearance(window, |ws, window, cx| {
                if ws.theme_override.map_or(SettingsStore::get(cx).appearance.theme, |(theme, _)| theme) == ThemeSetting::System {
                    ws.apply_appearance(window, cx);
                }
            }),
        };
        if cx.try_global::<HistoryStore>().is_some_and(|h| h.recovered) {
            ws.toast("Filter history was unreadable and has been reset; the old file was kept as history.json.bak", cx);
        }
        if cx.try_global::<RecentStore>().is_some_and(|r| r.recovered) {
            ws.toast("Recent files were unreadable and have been reset; the old file was kept as recent.json.bak", cx);
        }
        if cx.try_global::<SettingsStore>().is_some_and(|s| s.recovered) {
            ws.toast("Settings were unreadable and have been reset; the old file was kept as settings.toml.bak", cx);
        }
        let recovered: Vec<String> = cx
            .try_global::<crate::profiles::ProfileStore>()
            .map(|s| s.recovered.iter().filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).collect())
            .unwrap_or_default();
        for file in recovered {
            ws.toast(format!("Profile {file} was unreadable; the old file was kept as {file}.bak"), cx);
        }
        ws._session_subscriptions = vec![
            cx.observe_window_bounds(window, |ws, window, _| {
                ws.last_window = Some(crate::session::window_state(window.window_bounds(), ws.sidebar_visible, ws.sidebar_width));
            }),
            cx.on_app_quit(|ws, cx| {
                ws.save_session_now(cx);
                async {}
            }),
        ];
        // Closing the last window quits the app after the workspace is gone, too late for `on_app_quit`: save here.
        let this = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            this.update(cx, |ws, cx| ws.save_session_now(cx)).ok();
            true
        });
        ws.last_window = Some(crate::session::window_state(window.window_bounds(), ws.sidebar_visible, ws.sidebar_width));
        if !args.demo
            && SettingsStore::get(cx).behavior.restore_session
            && let Some(path) = cx.try_global::<crate::paths::AppPaths>().map(|p| p.session.clone())
        {
            match rogg_core::session::Session::load(&path) {
                Ok((session, recovered)) => {
                    if recovered {
                        ws.toast("Session was unreadable and has been reset; the old file was kept as session.json.bak", cx);
                    }
                    ws.restore_session(session, window, cx);
                }
                Err(e) => ws.toast(format!("Could not restore the session: {e}"), cx),
            }
        }
        ws.start_autosave(window, cx);
        ws.open_paths(args.paths, window, cx);
        if args.demo {
            crate::demo::open(&mut ws, window, cx);
        }
        ws
    }

    /// The window and its tabs (spec §6.4); with `all == false`, only the active tab (`.rogg` "this tab").
    pub fn capture_session(&self, all: bool, cx: &App) -> rogg_core::session::Session {
        let picked: Vec<&Entity<Tab>> = if all { self.tabs.iter().collect() } else { self.tabs.get(self.active).into_iter().collect() };
        let mut active = 0;
        let mut tabs = Vec::new();
        for tab in picked {
            if let Some(state) = crate::session::tab_state(tab.read(cx), cx) {
                if all && self.tabs.get(self.active).is_some_and(|t| t.entity_id() == tab.entity_id()) {
                    active = tabs.len();
                }
                tabs.push(state);
            }
        }
        let window = self.last_window.clone().map(|mut w| {
            w.sidebar_visible = self.sidebar_visible;
            w.sidebar_width = Some(f32::from(self.sidebar_width));
            w
        });
        rogg_core::session::Session { version: rogg_core::session::SESSION_VERSION, window, tabs, active }.sanitized()
    }

    /// Writes `session.json` (quit, every 30 s). The demo never overwrites the user's session.
    pub fn save_session_now(&mut self, cx: &mut Context<Self>) {
        if self.fixed_sidebar.is_some() {
            return;
        }
        let Some(path) = cx.try_global::<crate::paths::AppPaths>().map(|p| p.session.clone()) else { return };
        match self.capture_session(true, cx).save(&path) {
            Ok(()) => self.session_error = None,
            Err(e) => {
                let message = e.to_string();
                if self.session_error.replace(message.clone()).is_none() {
                    self.toast(format!("Could not save the session: {message}"), cx);
                }
            }
        }
    }

    fn start_autosave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(crate::session::AUTOSAVE).await;
                if this.update(cx, |ws, cx| ws.save_session_now(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Opens a stored session's tabs after the ones already open; returns how many were opened. Archive members put
    /// their archive in the sidebar, as reopening from Recent does. A profile that no longer exists becomes Default.
    pub fn restore_session(&mut self, session: rogg_core::session::Session, window: &mut Window, cx: &mut Context<Self>) -> usize {
        if let Some(w) = &session.window {
            self.sidebar_visible = w.sidebar_visible;
            if let Some(width) = w.sidebar_width.filter(|v| v.is_finite()) {
                self.drag_sidebar(px(width), cx);
            }
        }
        let first = self.tabs.len();
        let count = session.tabs.len();
        for state in session.tabs {
            if let rogg_core::session::SessionSource::Member { source } = &state.source {
                self.show_archive_of(source, window, cx);
            }
            let (source, mut init) = crate::session::restore_tab(state);
            let known = |name: &SharedString| cx.try_global::<crate::profiles::ProfileStore>().is_none_or(|s| s.contains(name));
            if init.profile.as_ref().is_some_and(|p| !known(p)) {
                init.profile = Some(rogg_core::profile::DEFAULT_PROFILE.into());
            }
            self.open_source(source, init, window, cx);
        }
        if count > 0 {
            self.activate(first + session.active.min(count - 1), window, cx);
        }
        count
    }

    /// A `.rogg` file (spec §6.5): restores its tabs; a broken file is reported and left as it is. The window's frame
    /// and sidebar stay as they are.
    pub fn open_rogg(&mut self, path: &std::path::Path, window: &mut Window, cx: &mut Context<Self>) {
        match rogg_core::session::Session::read_rogg(path) {
            Ok(session) => {
                self.restore_session(rogg_core::session::Session { window: None, ..session }, window, cx);
            }
            Err(e) => {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.toast(format!("Could not open {name}: {e}"), cx);
            }
        }
    }

    /// Rebuilds every tab's rows after a profile or the theme changed.
    pub fn restyle_tabs(&mut self, cx: &mut Context<Self>) {
        for tab in self.tabs.clone() {
            tab.update(cx, |t, cx| t.refresh_style(cx));
        }
        cx.notify();
    }

    pub fn tabs(&self) -> &[Entity<Tab>] {
        &self.tabs
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active_tab(&self) -> Option<Entity<Tab>> {
        self.tabs.get(self.active).cloned()
    }

    pub fn open_source(&mut self, source: Source, init: TabInit, window: &mut Window, cx: &mut Context<Self>) -> Entity<Tab> {
        let tab = cx.new(|cx| Tab::new(source, init, window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&tab, window, |ws, _, ev: &TabEvent, window, cx| ws.on_tab_event(ev, window, cx)),
            cx.observe(&tab, |_, _, cx| cx.notify()),
        ];
        self.tab_subscriptions.insert(tab.entity_id(), subscriptions);
        self.tabs.push(tab.clone());
        self.activate(self.tabs.len() - 1, window, cx);
        tab
    }

    pub fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else { return };
        self.active = index;
        self.tab_scroll.scroll_to_item(index);
        let title = tab.read(cx).title(cx);
        window.set_window_title(&title);
        cx.notify();
    }

    pub fn toast(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = self.next_toast;
        self.next_toast += 1;
        self.toasts.push((id, message.into()));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_TIME).await;
            this.update(cx, |ws, cx| ws.dismiss_toast(id, cx)).ok();
        })
        .detach();
        cx.notify();
    }

    pub fn dismiss_toast(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|(t, _)| *t != id);
        cx.notify();
    }

    fn on_tab_event(&mut self, event: &TabEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            TabEvent::Toast(message) => self.toast(message.clone(), cx),
            TabEvent::ShowHistory => self.open_dialog(Dialog::History, cx),
            TabEvent::ShowProfiles => self.open_profile_editor(window, cx),
            TabEvent::ShowStatistics => self.open_dialog(Dialog::Statistics, cx),
            TabEvent::FocusCrumb(i) => self.focus_crumb(*i, window, cx),
            TabEvent::OpenAsTab => self.open_as_tab(window, cx),
            TabEvent::ShowExport => self.open_export(cx),
            TabEvent::ShowFullLine { number, text } => self.open_full_line(*number, text.clone(), window, cx),
        }
    }

    /// "Show full line…": opens the line in a read-only text box (at most 1 MB shown) with Copy.
    pub fn open_full_line(&mut self, number: u64, text: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let (shown, cut) = rogg_ui::dialogs::full_line_shown(&text);
        let shown = SharedString::from(shown.to_string());
        let view = cx.new(|cx| TextareaState::new(window, cx).default_value(shown));
        self.open_dialog(Dialog::FullLine { number, text, view, cut }, cx);
    }

    /// The open full-line dialog's line number and whole text (tests).
    pub fn full_line_dialog(&self) -> Option<(u64, SharedString)> {
        match &self.dialog {
            Some(Dialog::FullLine { number, text, .. }) => Some((*number, text.clone())),
            _ => None,
        }
    }

    pub fn open_export(&mut self, cx: &mut Context<Self>) {
        if self.active_tab().is_some() {
            self.open_dialog(Dialog::Export, cx);
        }
    }

    /// Export lines…: the save panel, then the background export.
    pub(crate) fn prompt_export_lines(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        let d = tab.read(cx).doc.read(cx);
        let dir = d.source.path().and_then(|p| p.parent()).map(Path::to_path_buf).unwrap_or_else(rogg_core::paths::home_dir);
        // `my app.log ⟩ filtered` suggests `my app-filtered.log`.
        let title = d.title();
        let name = title.split(" ⟩ ").next().unwrap_or_default();
        let stem = name.strip_suffix(".log").unwrap_or(name).to_string();
        let rx = cx.prompt_for_new_path(&dir, Some(&format!("{stem}-filtered.log")));
        // The tab the panel was opened for, even if another one is active when it returns (and nothing once it closed).
        let tab = tab.downgrade();
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |ws, cx| {
                    if let Some(tab) = tab.upgrade() {
                        ws.export_tab_lines_to(&tab, path, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    /// Exports the active tab's filtered lines to `dst`.
    pub fn export_lines_to(&mut self, dst: PathBuf, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        self.export_tab_lines_to(&tab, dst, cx);
    }

    pub(crate) fn export_tab_lines_to(&mut self, tab: &Entity<Tab>, dst: PathBuf, cx: &mut Context<Self>) {
        let options = self.export_options;
        let doc = tab.read(cx).doc.clone();
        match doc.update(cx, |d, cx| d.export_lines(options, dst, cx)) {
            Ok(()) => self.close_dialog(cx),
            Err(message) => self.toast(message, cx),
        }
    }

    /// Save session…: this tab or all tabs as a `.rogg` file, written in the background.
    pub(crate) fn prompt_export_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_new_path(&rogg_core::paths::home_dir(), Some("session.rogg"));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |ws, cx| ws.export_session_to(path, cx)).ok();
            }
        })
        .detach();
    }

    pub fn export_session_to(&mut self, dst: PathBuf, cx: &mut Context<Self>) {
        let session = self.capture_session(self.export_all_tabs, cx);
        if session.tabs.is_empty() {
            self.toast("Nothing to save: streams and in-memory tabs are not saved", cx);
            return;
        }
        let name = dst.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let task = cx.background_executor().spawn(async move { session.save(&dst) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |ws, cx| match result {
                Ok(()) => ws.toast(format!("Saved session to {name}"), cx),
                Err(e) => ws.toast(format!("Could not save {name}: {e}"), cx),
            })
            .ok();
        })
        .detach();
        self.close_dialog(cx);
    }

    /// Copy filter expression (spec §6.5).
    pub fn copy_expression(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        let expression = tab.read(cx).doc.read(cx).filter.spec.expression();
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(expression));
        self.toast("Filter expression copied", cx);
    }

    fn focus_crumb(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        let Some(id) = tab.read(cx).crumb_tab(i, cx) else { return };
        if let Some(index) = self.tabs.iter().position(|t| t.entity_id() == id) {
            self.activate(index, window, cx);
        }
    }

    /// ⇧⌘T (spec §4.9, §5.6): the active tab's filtered rows, frozen into a new tab numbered from 1.
    pub fn open_as_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        let (source, init) = {
            let t = tab.read(cx);
            let d = t.doc.read(cx);
            let Some(text) = d.text().cloned() else { return };
            // A running scan or a view build in flight: the shown lines are not the ones the new filter will show.
            if !d.filter_settled() {
                self.toast("Filtering is still running — open as tab when it finishes", cx);
                return;
            }
            let lines = d.filter.view.shown_lines().clone();
            if lines.is_empty() {
                self.toast("No lines to open: the filtered log is empty", cx);
                return;
            }
            let live = d.is_live() || matches!(d.source, Source::Stream { .. });
            let snapshot = live.then(|| rogg_core::clock::local_hms(std::time::SystemTime::now()));
            let spec = crate::session::session_source(&d.source).map(|parent| rogg_core::session::DerivedSpec {
                parent,
                filter: d.filter.spec.clone(),
                pins: d.filter.pins.iter().copied().collect(),
                lines: live.then(|| lines.to_vec()),
                snapshot: snapshot.clone(),
            });
            let mut crumbs = t.crumbs(cx);
            crumbs.push("filtered".into());
            let source = crate::derived::DerivedSource {
                title: format!("{} ⟩ filtered", d.title()).into(),
                crumbs,
                tooltip: crate::derived::expression_label(&d.filter.spec),
                snapshot,
                spec,
                origin: crate::derived::DerivedOrigin::Lines { parent: text, lines },
            };
            let mut init = TabInit::from_settings(&SettingsStore::get(cx));
            init.profile = Some(t.profile.clone());
            init.crumb_tabs = t.crumb_tabs_for_child(cx);
            (Source::Derived(std::sync::Arc::new(source)), init)
        };
        self.open_source(source, init, window, cx);
    }

    /// Draw the traffic lights ourselves (headless captures have no native ones).
    pub fn set_traffic_lights(&mut self, on: bool, cx: &mut Context<Self>) {
        self.traffic_lights = on;
        cx.notify();
    }

    pub fn open_dialog(&mut self, dialog: Dialog, cx: &mut Context<Self>) {
        if matches!(dialog, Dialog::Settings) {
            crate::settings::cache_log_fonts(cx);
        }
        self.dialog = Some(dialog);
        cx.notify();
    }

    /// About › Show in Finder for data folder `index`. A missing folder is created first, so Finder has one to show.
    pub fn reveal_data_folder(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some((_, path)) = crate::about::data_folders(cx).into_iter().nth(index) {
            let _ = std::fs::create_dir_all(&path);
            self.reveal(path, cx);
        }
    }

    /// Shows `path` in Finder, and remembers it for tests.
    pub fn reveal(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.reveal_in_finder {
            cx.reveal_path(&path);
        }
        self.last_revealed = Some(path);
    }

    pub fn last_revealed(&self) -> Option<&Path> {
        self.last_revealed.as_deref()
    }

    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.open_dialog(Dialog::Settings, cx);
    }

    /// The font picker's choice: saves it and returns to Settings.
    pub fn pick_log_font(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let name = name.to_string();
        self.change_settings(move |s| s.appearance.log_font = name, window, cx);
        self.open_settings(cx);
    }

    /// The profile editor for the active tab's profile (spec §6.1).
    pub fn open_profile_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::dialogs::profiles::{ProfileEditor, ProfileEditorEvent};
        let tab = self.active_tab();
        let profile = tab.as_ref().map(|t| t.read(cx).profile.to_string()).unwrap_or_else(|| SettingsStore::get(cx).behavior.default_profile);
        let editor = cx.new(|_| ProfileEditor::new(tab.map(|t| t.downgrade()), profile));
        self.dialog_subscription = Some(cx.subscribe_in(&editor, window, |ws, _, ev: &ProfileEditorEvent, _window, cx| match ev {
            ProfileEditorEvent::Close => ws.close_dialog(cx),
            ProfileEditorEvent::Changed => ws.profiles_changed(cx),
            ProfileEditorEvent::Renamed { from, to } => ws.profile_renamed(from, to, cx),
            ProfileEditorEvent::Toast(message) => ws.toast(message.clone(), cx),
        }));
        self.open_dialog(Dialog::Profiles(editor), cx);
    }

    pub fn profile_editor(&self) -> Option<Entity<crate::dialogs::profiles::ProfileEditor>> {
        match &self.dialog {
            Some(Dialog::Profiles(editor)) => Some(editor.clone()),
            _ => None,
        }
    }

    /// After any profile change: tabs whose profile was deleted move to Default, and so does the default-profile
    /// setting; every tab restyles.
    fn profiles_changed(&mut self, cx: &mut Context<Self>) {
        let gone = |name: &str, cx: &App| cx.try_global::<crate::profiles::ProfileStore>().is_some_and(|s| !s.contains(name));
        for tab in self.tabs.clone() {
            if gone(&tab.read(cx).profile, cx) {
                tab.update(cx, |t, cx| t.set_profile(rogg_core::profile::DEFAULT_PROFILE.into(), cx));
            }
        }
        if gone(&SettingsStore::get(cx).behavior.default_profile, cx) {
            SettingsStore::update(cx, |s| s.behavior.default_profile = rogg_core::profile::DEFAULT_PROFILE.into());
        }
        self.restyle_tabs(cx);
        self.report_settings_error(cx);
        self.report_profile_error(cx);
    }

    /// After a settings change: toasts a failed save once, on the first failure after a success (like
    /// `session_error`); a successful save clears it. The dialog says changes are saved: a lost edit must not go
    /// unnoticed.
    pub(crate) fn report_settings_error(&mut self, cx: &mut Context<Self>) {
        match cx.try_global::<SettingsStore>().and_then(|s| s.last_error.clone()) {
            Some(e) => {
                if self.settings_error.replace(e.clone()).is_none() {
                    self.toast(format!("Could not save settings: {e}"), cx);
                }
            }
            None => self.settings_error = None,
        }
    }

    /// After a profile change: like `report_settings_error`, for the profile library.
    fn report_profile_error(&mut self, cx: &mut Context<Self>) {
        let profile = cx.try_global::<crate::profiles::ProfileStore>().and_then(|s| s.last_error.clone());
        match profile {
            Some(message) => {
                if self.profile_error.replace(message.clone()).is_none() {
                    self.toast(message, cx);
                }
            }
            None => self.profile_error = None,
        }
    }

    /// A renamed profile: tabs showing it and the default-profile setting follow the new name.
    fn profile_renamed(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        for tab in self.tabs.clone() {
            if tab.read(cx).profile.as_ref() == from {
                let to = SharedString::from(to.to_string());
                tab.update(cx, |t, cx| t.set_profile(to, cx));
            }
        }
        if SettingsStore::get(cx).behavior.default_profile == from {
            let to = to.to_string();
            SettingsStore::update(cx, |s| s.behavior.default_profile = to);
        }
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        self.goto_subscription = None;
        self.dialog_subscription = None;
        self.refocus_after_dialog = true;
        cx.notify();
    }

    /// Escape: closes a dialog, else cancels the expression editor.
    pub(crate) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            self.close_dialog(cx);
        } else if let Some(tab) = self.active_tab() {
            tab.update(cx, |t, cx| {
                if t.editing.is_some() {
                    t.cancel_edit(window, cx);
                }
            });
        }
    }

    fn titlebar(&self, this: &Entity<Self>, cx: &App) -> TitleBar {
        let items = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let t = tab.read(cx);
                let d = t.doc.read(cx);
                let tooltip = t.labels.tooltip.clone().unwrap_or_else(|| d.source.tooltip());
                TabItem { label: d.title(), tooltip, active: i == self.active }
            })
            .collect();
        TitleBar::new(items)
            .sidebar_width(self.sidebar_width)
            .traffic_lights(self.traffic_lights)
            .tab_scroll(self.tab_scroll.clone())
            .on_select_tab(act_ix(this, |ws, i, window, cx| ws.activate(i, window, cx)))
            .on_close_tab(act_ix(this, |ws, i, window, cx| ws.close_tab(i, window, cx)))
            .on_toggle_sidebar(act(this, |ws, _, cx| ws.toggle_sidebar(cx)))
            .on_all_tabs(act(this, |ws, _, cx| ws.open_dialog(Dialog::AllTabs, cx)))
            .on_shortcuts(act(this, |ws, _, cx| ws.open_dialog(Dialog::Shortcuts, cx)))
            .on_settings(act(this, |ws, _, cx| ws.open_settings(cx)))
    }

    fn dialog_element(&self, dialog: &Dialog, this: &Entity<Self>, cx: &App) -> AnyElement {
        let close = act(this, |ws, _, cx| ws.close_dialog(cx));
        match dialog {
            Dialog::Shortcuts => dialogs::shortcuts(&actions::shortcut_rows())
                .description("Keyboard shortcuts for the full log, the filter line and the filtered log.")
                .on_close(close)
                .into_any_element(),
            Dialog::AllTabs => {
                let tabs: Vec<(SharedString, bool)> = self.tabs.iter().enumerate().map(|(i, t)| (t.read(cx).title(cx), i == self.active)).collect();
                let pick = act_ix(this, |ws, i, window, cx| {
                    ws.activate(i, window, cx);
                    ws.close_dialog(cx);
                });
                dialogs::all_tabs(&tabs, Rc::new(pick)).description("All open tabs, including those outside the visible header.").on_close(close).into_any_element()
            }
            Dialog::History => {
                let entries: Vec<(SharedString, SharedString)> = cx
                    .try_global::<HistoryStore>()
                    .map(|h| h.entries().iter().map(|e| (SharedString::from(e.expression()), SharedString::from(e.summary()))).collect())
                    .unwrap_or_default();
                dialogs::history_picker(&entries, Rc::new(act_ix(this, |ws, i, _, cx| ws.restore_history(i, cx))))
                    .description("Restore an expression together with its term states and matching options.")
                    .on_close(close)
                    .into_any_element()
            }
            Dialog::GoToLine(input) => dialogs::go_to_line(input.clone()).description("Selects the line in the full log.").on_close(close).into_any_element(),
            Dialog::Settings => crate::dialogs::settings::render(this, cx).on_close(close).into_any_element(),
            Dialog::FontPicker => crate::dialogs::fonts::render(this, cx).on_close(act(this, |ws, _, cx| ws.open_settings(cx))).into_any_element(),
            Dialog::Profiles(editor) => editor.clone().into_any_element(),
            Dialog::Statistics => {
                let rows = self
                    .active_tab()
                    .map(|t| {
                        let d = t.read(cx).doc.read(cx);
                        crate::dialogs::stats::rows(d.filter.stats.as_ref(), d.filter.progress)
                    })
                    .unwrap_or_else(|| crate::dialogs::stats::rows(None, None));
                dialogs::statistics(&rows).description("Counts for the active tab's filter; they update as it runs.").on_close(close).into_any_element()
            }
            Dialog::Export => crate::dialogs::export::render(self, this, cx).on_close(close).into_any_element(),
            Dialog::FullLine { number, text, view, cut } => {
                let whole = text.clone();
                dialogs::full_line(*number, view.clone(), *cut, Rc::new(move |_, cx: &mut App| cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(whole.to_string()))))
                    .on_close(close)
                    .into_any_element()
            }
            Dialog::About => dialogs::about(&crate::about::view(cx), Rc::new(act_ix(this, |ws, i, _, cx| ws.reveal_data_folder(i, cx))))
                .on_close(close)
                .into_any_element(),
        }
    }

    /// The modal layer and the toasts, over everything else. The container has no handlers, so it does not catch
    /// the mouse.
    fn overlay(&self, this: &Entity<Self>, cx: &App) -> Option<AnyElement> {
        let modal = self.dialog.as_ref().map(|d| ModalLayer::new(self.dialog_element(d, this, cx)).on_dismiss(act(this, |ws, _, cx| ws.close_dialog(cx))));
        if modal.is_none() && self.toasts.is_empty() {
            return None;
        }
        let dismiss = this.clone();
        Some(
            div()
                .absolute()
                .inset_0()
                .children(modal)
                .when(!self.toasts.is_empty(), |d| {
                    d.child(ToastStack::new(self.toasts.clone()).on_dismiss(move |id, _, cx| dismiss.update(cx, |ws, cx| ws.dismiss_toast(id, cx))))
                })
                .into_any_element(),
        )
    }

    fn welcome(&self, this: &Entity<Self>, cx: &App) -> AnyElement {
        let open_file: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFile), cx));
        let open_folder: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFolder), cx));
        let recent = match &self.fixed_sidebar {
            Some(fixed) => fixed.recent.clone(),
            None => chrome::recent_items(cx),
        };
        empty::welcome(recent, open_file, open_folder, Rc::new(act_ix(this, |ws, i, window, cx| ws.open_recent(i, window, cx)))).into_any_element()
    }

    fn root(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        keys::bind(
            div()
            .id("root")
            .size_full()
            .key_context(actions::WORKSPACE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|ws, _: &Dismiss, window, cx| ws.dismiss(window, cx)))
            .on_action(keys::unless_dialog(cx, |ws, _: &ShowShortcuts, _, cx| ws.open_dialog(Dialog::Shortcuts, cx)))
            .on_action(cx.listener(|ws, _: &OpenSettings, _, cx| ws.open_settings(cx)))
            .on_action(cx.listener(|ws, _: &ShowAbout, _, cx| ws.open_dialog(Dialog::About, cx)))
            .on_action(cx.listener(|ws, _: &OpenFile, window, cx| ws.prompt_open(false, window, cx)))
            .on_action(cx.listener(|ws, _: &OpenFolder, window, cx| ws.prompt_open(true, window, cx)))
            .on_action(cx.listener(|ws, action: &OpenRecent, window, cx| ws.open_recent(action.index, window, cx)))
            .on_action(keys::unless_dialog(cx, |ws, _: &CloseTab, window, cx| {
                let index = ws.active;
                ws.close_tab(index, window, cx);
            }))
            .on_action(keys::unless_dialog(cx, |ws, _: &NextTab, window, cx| ws.cycle_tab(true, window, cx)))
            .on_action(keys::unless_dialog(cx, |ws, _: &PrevTab, window, cx| ws.cycle_tab(false, window, cx)))
            .on_action(keys::unless_dialog(cx, |ws, action: &ActivateTab, window, cx| ws.activate(action.index, window, cx)))
            .on_action(keys::unless_dialog(cx, |ws, _: &ToggleSidebar, _, cx| ws.toggle_sidebar(cx)))
            .on_action(keys::unless_dialog(cx, |ws, _: &OpenAsTab, window, cx| ws.open_as_tab(window, cx)))
            .on_action(keys::unless_dialog(cx, |ws, _: &Export, _, cx| ws.open_export(cx)))
            .on_drop(cx.listener(|ws, paths: &ExternalPaths, window, cx| ws.open_paths(paths.paths().to_vec(), window, cx)))
            .on_drag_move(cx.listener(|ws, e: &DragMoveEvent<BandDrag>, _, cx| {
                let top = e.drag(cx).band_top(e.event.position.y);
                ws.drag_band(top, cx);
            }))
            .on_drag_move(cx.listener(|ws, e: &DragMoveEvent<SidebarDrag>, _, cx| {
                let width = e.drag(cx).width(e.event.position.x);
                ws.drag_sidebar(width, cx);
            })),
            cx,
        )
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Keep keys flowing to the window's actions when the focused element went away (an applied editor, a
        // closed dialog's input).
        // Also when focus sits on a handle the active tab does not render (a placeholder tab, an empty filtered pane).
        let unrendered = self.active_tab().is_some_and(|t| t.read(cx).has_unrendered_focus(window, cx));
        let dialog_closed = std::mem::take(&mut self.refocus_after_dialog);
        if window.focused(cx).is_none() || unrendered {
            cx.defer_in(window, |ws, window, cx| ws.focus.focus(window, cx));
        } else if dialog_closed {
            // The closed dialog's input stays the focused handle for a frame or two (gpui keeps the previous frame's
            // elements alive); take focus back unless the close already moved it into the tab.
            cx.defer_in(window, |ws, window, cx| {
                let in_tab = ws.active_tab().is_some_and(|t| t.read(cx).has_focus(window, cx));
                if !in_tab {
                    ws.focus.focus(window, cx);
                }
            });
        }
        let this = cx.entity();
        let titlebar = self.titlebar(&this, cx).into_any_element();
        let overlay = self.overlay(&this, cx);
        let sidebar = self.sidebar(&this, cx);
        let content = match self.active_tab() {
            None => MainWindowLayout::single(SingleParts { titlebar, sidebar, info_bar: None, body: self.welcome(&this, cx), status: None, overlay })
                .sidebar_resizable(true)
                .into_any_element(),
            Some(tab) => {
                let parts = tab_view::parts(&tab, window, cx);
                match parts.body {
                    TabBody::Panes { top_header, top_body, band, bottom_header, bottom_body, split } => MainWindowLayout::new(MainWindowParts {
                        titlebar,
                        sidebar,
                        info_bar: parts.info_bar,
                        top_header,
                        top_body,
                        band,
                        bottom_header,
                        bottom_body,
                        status: parts.status,
                        overlay,
                        split,
                    })
                    .measure(self.measure.clone())
                    .sidebar_resizable(true)
                    .into_any_element(),
                    TabBody::Placeholder(body) => {
                        MainWindowLayout::single(SingleParts { titlebar, sidebar, info_bar: Some(parts.info_bar), body, status: Some(parts.status), overlay })
                            .sidebar_resizable(true)
                            .into_any_element()
                    }
                }
            }
        };
        self.root(cx).child(content)
    }
}
