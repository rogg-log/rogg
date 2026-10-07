//! A tab as window parts: info bar, pane headers, panes, filter band and status bar — or a loading/error
//! placeholder (spec §5.3, §5.7).

use crate::actions::FILTER_CONTEXT;
use crate::document::{LoadState, Pane};
use crate::tab::{Tab, TabEvent};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Context, Entity, SharedString, Window, div};
use rogg_core::filter::TermState;
use rogg_core::stats::{FilterStats, group_digits, noun};
use rogg_ui::chrome::{InfoBar, PaneHeader, Progress, StatPart, StatusBar};
use rogg_ui::controls::{FindField, ToolButton};
use rogg_ui::empty;
use rogg_ui::filter::{ChipState, FilterBand};
use rogg_ui::icons::IconName;
use rogg_ui::logview::LogView;
use std::rc::Rc;

pub enum TabBody {
    Panes { top_header: AnyElement, top_body: AnyElement, band: AnyElement, bottom_header: AnyElement, bottom_body: AnyElement, split: f32 },
    Placeholder(AnyElement),
}

pub struct TabParts {
    pub info_bar: AnyElement,
    pub body: TabBody,
    pub status: AnyElement,
}

const CONTEXT_OPTIONS: [&str; 4] = ["None", "±1 line", "±2 lines", "±3 lines"];

/// The context select's options: the reference presets, plus the tab's own radius when settings chose more than ±3.
pub(crate) fn context_options(context: u32) -> (Vec<SharedString>, usize) {
    let mut options: Vec<SharedString> = CONTEXT_OPTIONS.iter().map(|o| SharedString::from(*o)).collect();
    if (context as usize) < CONTEXT_OPTIONS.len() {
        return (options, context as usize);
    }
    options.push(format!("±{context} lines").into());
    (options, CONTEXT_OPTIONS.len())
}

fn tab_act(tab: &Entity<Tab>, f: impl Fn(&mut Tab, &mut Window, &mut Context<Tab>) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let tab = tab.clone();
    move |window, cx| tab.update(cx, |t, cx| f(t, window, cx))
}

fn tab_act_ix(tab: &Entity<Tab>, f: impl Fn(&mut Tab, usize, &mut Window, &mut Context<Tab>) + 'static) -> impl Fn(usize, &mut Window, &mut App) + 'static {
    let tab = tab.clone();
    move |i, window, cx| tab.update(cx, |t, cx| f(t, i, window, cx))
}

/// A button for a feature of a later milestone: says so in a toast.
fn later(tab: &Entity<Tab>, message: &'static str) -> impl Fn(&mut Window, &mut App) + 'static {
    tab_act(tab, move |_, _, cx| cx.emit(TabEvent::Toast(message.into())))
}

pub fn chip_state(state: TermState) -> ChipState {
    match state {
        TermState::Include => ChipState::Include,
        TermState::Off => ChipState::Off,
        TermState::Exclude => ChipState::Exclude,
    }
}

/// `6 matches / 40 lines · 1 pinned (1 added) · 0 context` with the counts in bold (reference footer).
fn stat_parts(stats: &FilterStats) -> Vec<StatPart> {
    let mut parts = vec![
        StatPart::Strong(group_digits(stats.matched).into()),
        StatPart::Text(
            format!(" {} / {} {} · ", noun(stats.matched, "match", "matches"), group_digits(stats.total_lines), noun(stats.total_lines, "line", "lines")).into(),
        ),
        StatPart::Strong(group_digits(stats.pinned).into()),
        StatPart::Text(" pinned".into()),
    ];
    if stats.pinned_unmatched > 0 {
        parts.push(StatPart::Text(format!(" ({} added)", group_digits(stats.pinned_unmatched)).into()));
    }
    parts.push(StatPart::Text(" · ".into()));
    parts.push(StatPart::Strong(group_digits(stats.context_lines).into()));
    parts.push(StatPart::Text(" context".into()));
    parts
}

pub fn parts(tab: &Entity<Tab>, window: &Window, cx: &App) -> TabParts {
    let t = tab.read(cx);
    let d = t.doc.read(cx);
    let crumbs = t.labels.crumbs.clone().unwrap_or_else(|| d.source.crumbs());
    let info_bar = InfoBar::new(crumbs, t.labels.profile.clone().unwrap_or_else(|| "Default".into()))
        .on_profile(later(tab, "Highlighting profiles arrive in a later version"))
        .on_export(later(tab, "Export arrives in a later version"))
        .into_any_element();
    let note = t.labels.note.clone().or_else(|| d.text().map(|text| SharedString::from(text.status_label()))).unwrap_or_default();
    let progress = d.job_progress().map(|(label, fraction)| Progress { label, fraction });
    let status = StatusBar::new(note)
        .stats(d.filter.stats.as_ref().map(stat_parts).unwrap_or_default())
        .selection(t.announced_selection().map(|n| SharedString::from(format!("Line {n}"))))
        .progress(progress)
        .into_any_element();

    let body = match &d.load {
        LoadState::Loading { fraction, label } => {
            TabBody::Placeholder(empty::loading(label.clone(), Some(*fraction), Rc::new(tab_act(tab, |t, _, cx| t.doc.update(cx, |d, cx| d.cancel_load(cx))))).into_any_element())
        }
        LoadState::Failed { message } => {
            let path = d.source.location();
            TabBody::Placeholder(empty::error(message.clone(), path, Rc::new(tab_act(tab, |t, _, cx| t.doc.update(cx, |d, cx| d.retry(cx))))).into_any_element())
        }
        LoadState::Ready(text) => {
            let lines = text.line_count();
            let full_count = t.labels.full_count.clone().unwrap_or_else(|| format!("{} {}", group_digits(lines), noun(lines, "line", "lines")).into());
            let shown = d.filter.view.shown_lines().len() as u64;
            let mut top_header = PaneHeader::new("pane-top-head", "Full log", full_count).find(FindField::new("find-top", t.find[0].clone()).count(d.search(Pane::Full).label()));
            if d.is_live() {
                let paused = !t.live.following;
                top_header = top_header.tool(
                    ToolButton::new("tool-live", IconName::Pause, if paused { "Paused" } else { "Pause" })
                        .pressed(paused)
                        .badge((paused && t.live.new_lines > 0).then(|| SharedString::from(format!("+{}", group_digits(t.live.new_lines)))))
                        .on_click(tab_act(tab, |t, _, cx| t.toggle_live(cx))),
                );
            }
            let top_header = top_header
                .tool(ToolButton::new("tool-wrap-top", IconName::Wrap, "Wrap").pressed(t.full.read(cx).wrap()).on_click(tab_act(tab, |t, _, cx| t.toggle_wrap(Pane::Full, cx))))
                .into_any_element();
            let bottom_header = PaneHeader::new("pane-bottom-head", "Filtered log", format!("{} shown", group_digits(shown)))
                .find(FindField::new("find-bottom", t.find[1].clone()).count(d.search(Pane::Filtered).label()))
                .tool(ToolButton::new("tool-open-tab", IconName::OpenTab, "Open as tab").on_click(later(tab, "Open as tab arrives in a later version")))
                .tool(ToolButton::new("tool-follow", IconName::Follow, "Follow selection").pressed(t.follow).on_click(tab_act(tab, |t, _, cx| t.toggle_follow(cx))))
                .tool(
                    ToolButton::new("tool-wrap-bottom", IconName::Wrap, "Wrap")
                        .pressed(t.filtered.read(cx).wrap())
                        .on_click(tab_act(tab, |t, _, cx| t.toggle_wrap(Pane::Filtered, cx))),
                )
                .into_any_element();
            let spec = &d.filter.spec;
            let (context_items, context_selected) = context_options(spec.context);
            let band = FilterBand::new(spec.terms.iter().map(|term| (SharedString::from(term.text.clone()), chip_state(term.state))).collect())
                .regex(spec.regex)
                .case_sensitive(spec.case_sensitive)
                .editing(t.editing.clone())
                .error(d.filter.error.clone())
                .context(context_items, context_selected)
                .summary(spec.summary().into())
                .focused(t.focus[1].is_focused(window))
                .on_cycle(tab_act_ix(tab, |t, i, _, cx| t.doc.update(cx, |d, cx| d.cycle_term(i, cx))))
                .on_edit(tab_act(tab, |t, window, cx| t.start_edit(window, cx)))
                .on_apply(tab_act(tab, |t, window, cx| t.apply_edit(window, cx)))
                .on_toggle_regex(tab_act(tab, |t, _, cx| t.doc.update(cx, |d, cx| d.toggle_regex(cx))))
                .on_toggle_case(tab_act(tab, |t, _, cx| t.doc.update(cx, |d, cx| d.toggle_case(cx))))
                .on_history(tab_act(tab, |_, _, cx| cx.emit(TabEvent::ShowHistory)))
                .on_context(tab_act_ix(tab, |t, i, _, cx| {
                    if i < CONTEXT_OPTIONS.len() {
                        t.doc.update(cx, |d, cx| d.set_context(i as u32, cx))
                    }
                }));
            // Ruling R6: a single-line Input propagates `enter`, so the filter-line context is off while editing,
            // else Enter in the editor would also dispatch `EditFilters`.
            let band = div().track_focus(&t.focus[1]).when(t.editing.is_none(), |d| d.key_context(FILTER_CONTEXT)).child(band).into_any_element();
            let top_body = LogView::new("log-top", t.full.clone()).focus(t.focus[0].clone()).into_any_element();
            let bottom_body = if shown == 0 && d.filter.progress.is_none() {
                empty::no_matches("empty-results").into_any_element()
            } else {
                LogView::new("log-bottom", t.filtered.clone()).focus(t.focus[2].clone()).into_any_element()
            };
            TabBody::Panes { top_header, top_body, band, bottom_header, bottom_body, split: t.split }
        }
    };
    TabParts { info_bar, body, status }
}

#[cfg(test)]
mod tests {
    use super::context_options;

    #[test]
    fn context_options_grow_for_settings_beyond_three() {
        assert_eq!(context_options(2), (vec!["None".into(), "±1 line".into(), "±2 lines".into(), "±3 lines".into()], 2));
        let (options, selected) = context_options(7);
        assert_eq!((options.len(), selected), (5, 4));
        assert_eq!(options[4].as_ref(), "±7 lines");
    }
}
