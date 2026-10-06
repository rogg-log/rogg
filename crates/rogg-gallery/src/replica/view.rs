use super::state::{ReplicaDialog, ReplicaState};
use crate::fixtures;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Entity, SharedString};
use rogg_ui::chrome::{InfoBar, PaneHeader, StatusBar};
use rogg_ui::controls::{FindField, ToolButton};
use rogg_ui::dialogs::{self, ModalLayer};
use rogg_ui::empty;
use rogg_ui::filter::FilterBand;
use rogg_ui::icons::IconName;
use rogg_ui::layout::{MainWindowLayout, MainWindowParts};
use rogg_ui::logview::LogView;
use rogg_ui::sidebar::Sidebar;
use rogg_ui::titlebar::{TabItem, TitleBar};
use std::rc::Rc;

fn act(state: &Entity<ReplicaState>, f: impl Fn(&mut ReplicaState, &mut gpui_kit::Context<ReplicaState>) + 'static) -> impl Fn(&mut gpui_kit::Window, &mut App) + 'static {
    let state = state.clone();
    move |_, cx| state.update(cx, |s, cx| f(s, cx))
}

fn act_ix(state: &Entity<ReplicaState>, f: impl Fn(&mut ReplicaState, usize, &mut gpui_kit::Context<ReplicaState>) + 'static) -> impl Fn(usize, &mut gpui_kit::Window, &mut App) + 'static {
    let state = state.clone();
    move |i, _, cx| state.update(cx, |s, cx| f(s, i, cx))
}

fn dialog(d: ReplicaDialog, s: &ReplicaState, state: &Entity<ReplicaState>) -> AnyElement {
    let close = act(state, |s, cx| s.close_dialog(cx));
    match d {
        ReplicaDialog::AllTabs => {
            let tabs: Vec<(SharedString, bool)> = s.tabs.iter().enumerate().map(|(i, t)| ((*t).into(), i == s.active_tab)).collect();
            let pick = act_ix(state, |s, i, cx| {
                s.select_tab(i, cx);
                s.close_dialog(cx);
            });
            dialogs::all_tabs(&tabs, Rc::new(pick)).description(fixtures::tabs_description()).on_close(close).into_any_element()
        }
        ReplicaDialog::Profile => dialogs::profile("Services", &fixtures::profile_rules())
            .description(fixtures::PROFILE_DESCRIPTION)
            .note(fixtures::PROFILE_NOTE)
            .on_close(close)
            .into_any_element(),
        ReplicaDialog::Appearance => dialogs::appearance(&fixtures::appearance_rows())
            .description(fixtures::APPEARANCE_DESCRIPTION)
            .note(fixtures::APPEARANCE_NOTE)
            .on_close(close)
            .into_any_element(),
        ReplicaDialog::Shortcuts => dialogs::shortcuts(&fixtures::shortcut_rows())
            .description(fixtures::SHORTCUTS_DESCRIPTION)
            .note(fixtures::SHORTCUTS_NOTE)
            .on_close(close)
            .into_any_element(),
        ReplicaDialog::History => dialogs::history(&fixtures::history_entries())
            .description(fixtures::HISTORY_DESCRIPTION)
            .note(fixtures::HISTORY_NOTE)
            .on_close(close)
            .into_any_element(),
    }
}

/// The reference window rebuilt from rogg-ui components and ReplicaState.
pub fn replica_window(state: &Entity<ReplicaState>, cx: &mut App) -> AnyElement {
    let s = state.read(cx);
    let tabs: Vec<TabItem> = s
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| TabItem { label: (*t).into(), tooltip: format!("{} · {}", t, fixtures::BUNDLE).into(), active: i == s.active_tab })
        .collect();
    let active = s.tabs[s.active_tab];
    let titlebar = TitleBar::new(tabs)
        .traffic_lights(true)
        .on_select_tab(act_ix(state, |s, i, cx| s.select_tab(i, cx)))
        .on_close_tab(act_ix(state, |s, i, cx| s.close_tab(i, cx)))
        .on_toggle_sidebar(act(state, |s, cx| s.toggle_sidebar(cx)))
        .on_all_tabs(act(state, |s, cx| s.open_dialog(ReplicaDialog::AllTabs, cx)))
        .on_shortcuts(act(state, |s, cx| s.open_dialog(ReplicaDialog::Shortcuts, cx)))
        .on_settings(act(state, |s, cx| s.open_dialog(ReplicaDialog::Appearance, cx)));
    let sidebar = s.sidebar_visible.then(|| {
        Sidebar::new()
            .count("10 files".into())
            .rows(fixtures::sidebar_rows(active))
            .recent(fixtures::recent())
            .footer(Some(fixtures::sidebar_footer()))
            .into_any_element()
    });
    let info_bar = InfoBar::new(vec![fixtures::BUNDLE.into(), active.into()], "Services".into())
        .on_profile(act(state, |s, cx| s.open_dialog(ReplicaDialog::Profile, cx)));
    let top_wrap = s.top.read(cx).wrap();
    let bottom_wrap = s.bottom.read(cx).wrap();
    let top_header = PaneHeader::new("pane-top-head", "Full log", "40 sample lines")
        .find(FindField::new("find-top", s.find_top.clone()).count(s.hit_count(false, cx)))
        .tool(ToolButton::new("tool-wrap-top", IconName::Wrap, "Wrap").pressed(top_wrap).on_click(act(state, |s, cx| s.toggle_wrap(false, cx))));
    let bottom_header = PaneHeader::new("pane-bottom-head", "Filtered log", format!("{} shown", s.filtered.visible.len()))
        .find(FindField::new("find-bottom", s.find_bottom.clone()).count(s.hit_count(true, cx)))
        .tool(ToolButton::new("tool-open-tab", IconName::OpenTab, "Open as tab"))
        .tool(ToolButton::new("tool-follow", IconName::Follow, "Follow selection").pressed(s.follow).on_click(act(state, |s, cx| s.toggle_follow(cx))))
        .tool(ToolButton::new("tool-wrap-bottom", IconName::Wrap, "Wrap").pressed(bottom_wrap).on_click(act(state, |s, cx| s.toggle_wrap(true, cx))));
    let band = FilterBand::new(s.terms.iter().map(|t| (SharedString::from(t.text.clone()), t.state)).collect())
        .regex(s.regex)
        .case_sensitive(s.case_sensitive)
        .editing(s.editing.clone())
        .error(s.error.clone())
        .context(vec!["None".into(), "±1 line".into(), "±2 lines".into(), "±3 lines".into()], s.context)
        .summary(s.summary().into())
        .on_cycle(act_ix(state, |s, i, cx| s.cycle(i, cx)))
        .on_edit({
            let state = state.clone();
            move |window, cx| state.update(cx, |s, cx| s.start_edit(window, cx))
        })
        .on_apply(act(state, |s, cx| s.apply_edit(cx)))
        .on_toggle_regex(act(state, |s, cx| s.toggle_regex(cx)))
        .on_toggle_case(act(state, |s, cx| s.toggle_case(cx)))
        .on_history(act(state, |s, cx| s.open_dialog(ReplicaDialog::History, cx)))
        .on_context(act_ix(state, |s, i, cx| s.set_context(i, cx)));
    let bottom_body = if s.filtered.visible.is_empty() {
        empty::no_matches("empty-results").into_any_element()
    } else {
        LogView::new("log-bottom", s.bottom.clone()).into_any_element()
    };
    let status = StatusBar::new("UTF-8 · LF · sample excerpt".into())
        .stats(s.stats())
        .selection(s.status_selection.map(|i| format!("Line {}", super::data::FIRST_LINE_NUMBER + i as u64).into()));
    let overlay = s.dialog.map(|d| ModalLayer::new(dialog(d, s, state)).on_dismiss(act(state, |s, cx| s.close_dialog(cx))).into_any_element());
    MainWindowLayout::new(MainWindowParts {
        titlebar: titlebar.into_any_element(),
        sidebar,
        info_bar: info_bar.into_any_element(),
        top_header: top_header.into_any_element(),
        top_body: LogView::new("log-top", s.top.clone()).into_any_element(),
        band: band.into_any_element(),
        bottom_header: bottom_header.into_any_element(),
        bottom_body,
        status: status.into_any_element(),
        overlay,
        split: MainWindowParts::DEFAULT_SPLIT,
    })
    .into_any_element()
}
