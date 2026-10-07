//! The main window view: title bar, tabs, sidebar, dialogs and the app's actions.

use crate::actions::{self, Dismiss, OpenFile, OpenFolder, ShowShortcuts};
use crate::args::Args;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Context, Entity, FocusHandle, Pixels, Window, div, px};
use rogg_ui::dialogs::{self, ModalLayer};
use rogg_ui::empty;
use rogg_ui::layout::{MainWindowLayout, SingleParts};
use rogg_ui::theme::size;
use rogg_ui::titlebar::TitleBar;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    Shortcuts,
}

pub struct Workspace {
    pub(crate) focus: FocusHandle,
    #[allow(dead_code)] // read by the layout once tabs and the sidebar arrive (Task 7+)
    pub(crate) sidebar_visible: bool,
    pub(crate) sidebar_width: Pixels,
    pub(crate) dialog: Option<Dialog>,
    traffic_lights: bool,
}

/// A `Fn(&mut Window, &mut App)` callback that runs `f` on the workspace.
pub(crate) fn act(this: &Entity<Workspace>, f: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let this = this.clone();
    move |window, cx| this.update(cx, |ws, cx| f(ws, window, cx))
}

/// Like `act`, for callbacks carrying an index.
#[allow(dead_code)] // first used by the tab strip (Task 7+)
pub(crate) fn act_ix(
    this: &Entity<Workspace>,
    f: impl Fn(&mut Workspace, usize, &mut Window, &mut Context<Workspace>) + 'static,
) -> impl Fn(usize, &mut Window, &mut App) + 'static {
    let this = this.clone();
    move |i, window, cx| this.update(cx, |ws, cx| f(ws, i, window, cx))
}

impl Workspace {
    pub fn new(_args: Args, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self { focus, sidebar_visible: true, sidebar_width: px(size::SIDEBAR_W), dialog: None, traffic_lights: false }
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

    pub(crate) fn dismiss(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            self.close_dialog(cx);
        }
    }

    fn titlebar(&self, this: &Entity<Self>) -> TitleBar {
        TitleBar::new(vec![])
            .sidebar_width(self.sidebar_width)
            .traffic_lights(self.traffic_lights)
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

    fn overlay(&self, this: &Entity<Self>) -> Option<AnyElement> {
        self.dialog.as_ref().map(|d| ModalLayer::new(self.dialog_element(d, this)).on_dismiss(act(this, |ws, _, cx| ws.close_dialog(cx))).into_any_element())
    }

    fn welcome(&self) -> AnyElement {
        let open_file: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFile), cx));
        let open_folder: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFolder), cx));
        empty::welcome(vec![], open_file, open_folder, Rc::new(|_: usize, _: &mut Window, _: &mut App| {})).into_any_element()
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let content = MainWindowLayout::single(SingleParts {
            titlebar: self.titlebar(&this).into_any_element(),
            sidebar: None,
            info_bar: None,
            body: self.welcome(),
            status: None,
            overlay: self.overlay(&this),
        });
        div()
            .id("root")
            .size_full()
            .key_context(actions::WORKSPACE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|ws, _: &Dismiss, window, cx| ws.dismiss(window, cx)))
            .on_action(cx.listener(|ws, _: &ShowShortcuts, _, cx| ws.open_dialog(Dialog::Shortcuts, cx)))
            .child(content)
    }
}
