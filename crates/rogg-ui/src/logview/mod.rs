pub mod actions;
mod element;
mod model;
pub mod scroll;
mod state;
pub mod text;

pub use element::LogViewElement;
pub use model::{LogLine, LogRow, LogRowSource, RowTint, Span, SpanStyle, VecRows};
pub use state::{ContextTarget, LogViewEvent, LogViewState, MAX_COPY_ROWS, RowGeometry, ScrollTo};

use crate::theme::ActiveRoggTheme;
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::prelude::*;
use gpui_kit::{App, ClipboardItem, Entity, FocusHandle, SharedString, TestSupportExt, Window, div};

/// A log panel: a test-support container filling its parent around the custom row element.
#[derive(IntoElement)]
pub struct LogView {
    id: SharedString,
    state: Entity<LogViewState>,
    focus: Option<FocusHandle>,
}

impl LogView {
    pub fn new(id: impl Into<SharedString>, state: Entity<LogViewState>) -> Self {
        Self { id: id.into(), state, focus: None }
    }

    /// Makes the panel focusable: mouse presses focus it, its keyboard actions dispatch in
    /// `actions::KEY_CONTEXT`, and a 2 px accent ring shows while it has focus.
    pub fn focus(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }
}

fn copy(state: &Entity<LogViewState>, with_numbers: bool, cx: &mut App) {
    if let Some(text) = state.read(cx).selected_text(with_numbers) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }
}

fn emit(state: &Entity<LogViewState>, event: LogViewEvent, cx: &mut App) {
    state.update(cx, |_, cx| cx.emit(event));
}

/// Spec §5.5 context menu, built from the target recorded by the right-click.
fn context_menu(menu: PopupMenu, state: &Entity<LogViewState>, cx: &App) -> PopupMenu {
    let Some(target) = state.read(cx).context_target().cloned() else { return menu };
    let number = target.number;
    let (s1, s2, s3) = (state.clone(), state.clone(), state.clone());
    let menu = menu
        .item(PopupMenuItem::new(if target.pinned { "Unpin" } else { "Pin" }).on_click(move |_, _, cx| emit(&s1, LogViewEvent::TogglePin { number }, cx)))
        .separator()
        .item(PopupMenuItem::new("Copy").on_click(move |_, _, cx| copy(&s2, false, cx)))
        .item(PopupMenuItem::new("Copy with line numbers").on_click(move |_, _, cx| copy(&s3, true, cx)))
        .separator();
    match target.token {
        Some(token) => {
            let (include, exclude) = (token.clone(), token.clone());
            let (s4, s5) = (state.clone(), state.clone());
            menu.item(
                PopupMenuItem::new(format!("Filter on “{token}”"))
                    .on_click(move |_, _, cx| emit(&s4, LogViewEvent::AddTerm { text: include.clone(), exclude: false }, cx)),
            )
            .item(
                PopupMenuItem::new(format!("Exclude “{token}”"))
                    .on_click(move |_, _, cx| emit(&s5, LogViewEvent::AddTerm { text: exclude.clone(), exclude: true }, cx)),
            )
        }
        None => menu.item(PopupMenuItem::new("Filter on selection").disabled(true)).item(PopupMenuItem::new("Exclude selection").disabled(true)),
    }
}

impl RenderOnce for LogView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let focused = self.focus.as_ref().is_some_and(|f| f.is_focused(window));
        let mut element = LogViewElement::new(self.state.clone());
        if let Some(f) = self.focus.clone() {
            element = element.focus(f);
        }
        let s = self.state.clone();
        let menu_state = self.state.clone();
        div()
            .id(self.id)
            .test_support()
            .relative()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .key_context(actions::KEY_CONTEXT)
            .when_some(self.focus, |d, f| d.track_focus(&f))
            .on_action({
                let s = s.clone();
                move |_: &actions::SelectPrev, _, cx| s.update(cx, |s, cx| s.move_selection(-1, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::SelectNext, _, cx| s.update(cx, |s, cx| s.move_selection(1, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::ExtendPrev, _, cx| s.update(cx, |s, cx| s.move_selection(-1, true, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::ExtendNext, _, cx| s.update(cx, |s, cx| s.move_selection(1, true, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::PagePrev, _, cx| s.update(cx, |s, cx| s.page(false, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::PageNext, _, cx| s.update(cx, |s, cx| s.page(true, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::SelectFirst, _, cx| s.update(cx, |s, cx| s.select_edge(false, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::SelectLast, _, cx| s.update(cx, |s, cx| s.select_edge(true, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::CopySelection, _, cx| copy(&s, false, cx)
            })
            .on_action(move |_: &actions::CopySelectionWithNumbers, _, cx| copy(&s, true, cx))
            .child(element)
            .when(focused, |d| d.child(div().absolute().inset_0().border_2().border_color(p.accent)))
            .context_menu(move |menu, _, cx| context_menu(menu, &menu_state, cx))
    }
}
