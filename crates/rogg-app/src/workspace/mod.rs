//! The main window view: title bar, tabs, sidebar, dialogs, toasts and the app's actions.

use crate::actions::{self, Dismiss, OpenFile, OpenFolder, ShowShortcuts};
use crate::args::Args;
use crate::document::Source;
use crate::tab::{Tab, TabEvent, TabInit};
use crate::tab_view::{self, TabBody};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Context, Div, Entity, EntityId, FocusHandle, Pixels, SharedString, Stateful, Subscription, Window, div, px};
use rogg_ui::dialogs::{self, ModalLayer};
use rogg_ui::empty;
use rogg_ui::layout::{LayoutMeasure, MainWindowLayout, MainWindowParts, SingleParts};
use rogg_ui::theme::size;
use rogg_ui::titlebar::{TabItem, TitleBar};
use rogg_ui::toast::ToastStack;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

/// How long a toast stays up.
const TOAST_TIME: Duration = Duration::from_secs(4);

#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    Shortcuts,
}

pub struct Workspace {
    pub(crate) focus: FocusHandle,
    #[expect(dead_code)] // read by the layout once the sidebar arrives (Task 12)
    pub(crate) sidebar_visible: bool,
    pub(crate) sidebar_width: Pixels,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) tabs: Vec<Entity<Tab>>,
    pub(crate) active: usize,
    tab_subscriptions: HashMap<EntityId, Vec<Subscription>>,
    pub(crate) measure: Rc<LayoutMeasure>,
    pub toasts: Vec<(u64, SharedString)>,
    next_toast: u64,
    traffic_lights: bool,
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
        };
        ws.open_paths(args.paths, window, cx);
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

    /// Opens each path in a tab, or activates the tab already showing it.
    pub fn open_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        for path in paths {
            let path = path.canonicalize().unwrap_or(path);
            let existing = self.tabs.iter().position(|t| t.read(cx).doc.read(cx).source.path() == Some(path.as_path()));
            match existing {
                Some(index) => self.activate(index, window, cx),
                None => {
                    self.open_source(Source::File(path), TabInit::default(), window, cx);
                }
            }
        }
    }

    pub fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else { return };
        self.active = index;
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
            TabEvent::ShowHistory => {}
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
                let tooltip = t.labels.tooltip.clone().or_else(|| d.source.path().map(|p| SharedString::from(p.display().to_string()))).unwrap_or_else(|| d.title());
                TabItem { label: d.title(), tooltip, active: i == self.active }
            })
            .collect();
        TitleBar::new(items)
            .sidebar_width(self.sidebar_width)
            .traffic_lights(self.traffic_lights)
            .on_select_tab(act_ix(this, |ws, i, window, cx| ws.activate(i, window, cx)))
            .on_shortcuts(act(this, |ws, _, cx| ws.open_dialog(Dialog::Shortcuts, cx)))
    }

    fn dialog_element(&self, dialog: &Dialog, this: &Entity<Self>) -> AnyElement {
        match dialog {
            Dialog::Shortcuts => dialogs::shortcuts(&actions::shortcut_rows())
                .description("Keyboard shortcuts for the full log, the filter line and the filtered log.")
                .on_close(act(this, |ws, _, cx| ws.close_dialog(cx)))
                .into_any_element(),
        }
    }

    /// The modal layer and the toasts, over everything else. The container has no handlers, so it does not catch
    /// the mouse.
    fn overlay(&self, this: &Entity<Self>) -> Option<AnyElement> {
        let modal = self.dialog.as_ref().map(|d| ModalLayer::new(self.dialog_element(d, this)).on_dismiss(act(this, |ws, _, cx| ws.close_dialog(cx))));
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

    fn welcome(&self) -> AnyElement {
        let open_file: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFile), cx));
        let open_folder: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFolder), cx));
        empty::welcome(vec![], open_file, open_folder, Rc::new(|_: usize, _: &mut Window, _: &mut App| {})).into_any_element()
    }

    fn root(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("root")
            .size_full()
            .key_context(actions::WORKSPACE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|ws, _: &Dismiss, window, cx| ws.dismiss(window, cx)))
            .on_action(cx.listener(|ws, _: &ShowShortcuts, _, cx| ws.open_dialog(Dialog::Shortcuts, cx)))
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Keep keys flowing to the window's actions when the focused element went away (an applied editor, a
        // closed dialog's input).
        if window.focused(cx).is_none() {
            cx.defer_in(window, |ws, window, cx| ws.focus.focus(window, cx));
        }
        let this = cx.entity();
        let titlebar = self.titlebar(&this, cx).into_any_element();
        let overlay = self.overlay(&this);
        let sidebar: Option<AnyElement> = None;
        let content = match self.active_tab() {
            None => MainWindowLayout::single(SingleParts { titlebar, sidebar, info_bar: None, body: self.welcome(), status: None, overlay }).into_any_element(),
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
                            .into_any_element()
                    }
                }
            }
        };
        self.root(cx).child(content)
    }
}
