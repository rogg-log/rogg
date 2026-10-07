//! The main window view: title bar, tabs, sidebar, dialogs, toasts and the app's actions.

pub mod chrome;
pub mod keys;

use crate::actions::{self, ActivateTab, CloseTab, Dismiss, NextTab, OpenFile, OpenFolder, OpenRecent, PrevTab, ShowShortcuts, ToggleSidebar};
use crate::args::Args;
use crate::document::Source;
use crate::history::HistoryStore;
use crate::recent::RecentStore;
use crate::sidebar_model::FixedSidebar;
use crate::tab::{Tab, TabEvent, TabInit};
use crate::tab_view::{self, TabBody};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Div, DragMoveEvent, Entity, EntityId, ExternalPaths, FocusHandle, Pixels, ScrollHandle, SharedString, Stateful, Subscription,
    Window, div, px,
};
use rogg_ui::dialogs::{self, ModalLayer};
use rogg_ui::empty;
use rogg_ui::layout::{BandDrag, LayoutMeasure, MainWindowLayout, MainWindowParts, SidebarDrag, SingleParts};
use keys::ThemeChoice;
use rogg_ui::theme::{ActiveRoggTheme, Density, size};
use rogg_ui::titlebar::{TabItem, TitleBar};
use rogg_ui::toast::ToastStack;
use std::collections::{HashMap, HashSet};
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
    pub(crate) theme: ThemeChoice,
    pub(crate) density: Density,
    goto_subscription: Option<Subscription>,
    /// A dialog just closed: its input's focus handle outlives it until the frame is replaced, so check again after.
    refocus_after_dialog: bool,
    _appearance_subscription: Subscription,
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
            theme: ThemeChoice::System,
            density: cx.rogg().density,
            goto_subscription: None,
            refocus_after_dialog: false,
            _appearance_subscription: cx.observe_window_appearance(window, |ws, window, cx| {
                if ws.theme == ThemeChoice::System {
                    ws.apply_theme(window, cx);
                }
            }),
        };
        if cx.try_global::<HistoryStore>().is_some_and(|h| h.recovered) {
            ws.toast("Filter history was unreadable and has been reset; the old file was kept as history.json.bak", cx);
        }
        if cx.try_global::<RecentStore>().is_some_and(|r| r.recovered) {
            ws.toast("Recent files were unreadable and have been reset; the old file was kept as recent.json.bak", cx);
        }
        ws.open_paths(args.paths, window, cx);
        if args.demo {
            crate::demo::open(&mut ws, window, cx);
        }
        ws
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

    fn on_tab_event(&mut self, event: &TabEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event {
            TabEvent::Toast(message) => self.toast(message.clone(), cx),
            TabEvent::ShowHistory => self.open_dialog(Dialog::History, cx),
        }
    }

    /// Draw the traffic lights ourselves (headless captures have no native ones).
    pub fn set_traffic_lights(&mut self, on: bool, cx: &mut Context<Self>) {
        self.traffic_lights = on;
        cx.notify();
    }

    pub fn open_dialog(&mut self, dialog: Dialog, cx: &mut Context<Self>) {
        self.dialog = Some(dialog);
        cx.notify();
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        self.goto_subscription = None;
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
            .on_settings(act(this, |ws, _, cx| ws.toast("Settings arrive in a later version", cx)))
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
                    .map(|h| h.entries().iter().map(|e| (SharedString::from(e.expression()), SharedString::from(e.to_spec().summary()))).collect())
                    .unwrap_or_default();
                dialogs::history_picker(&entries, Rc::new(act_ix(this, |ws, i, _, cx| ws.restore_history(i, cx))))
                    .description("Restore an expression together with its term states and matching options.")
                    .on_close(close)
                    .into_any_element()
            }
            Dialog::GoToLine(input) => dialogs::go_to_line(input.clone()).description("Selects the line in the full log.").on_close(close).into_any_element(),
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
            .on_drop(cx.listener(|ws, paths: &ExternalPaths, window, cx| ws.open_paths(paths.paths().to_vec(), window, cx)))
            .on_drag_move(cx.listener(|ws, e: &DragMoveEvent<BandDrag>, _, cx| ws.drag_band(e.event.position.y, cx)))
            .on_drag_move(cx.listener(|ws, e: &DragMoveEvent<SidebarDrag>, _, cx| ws.drag_sidebar(e.event.position.x, cx))),
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
