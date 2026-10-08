use crate::chrome::bar_fraction;
use crate::controls::ActionButton;
use crate::icons::{IconName, icon};
use crate::sidebar::RecentItem;
use crate::theme::ActiveRoggTheme;
use crate::{Callback, IndexCallback};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, FontWeight, SharedString, TestSupportExt, Window, div, px, relative};

/// `.empty`: centred column with a 15px title, 12px muted body and optional actions.
#[derive(IntoElement)]
pub struct EmptyState {
    id: SharedString,
    title: SharedString,
    body: Option<SharedString>,
    children: Vec<AnyElement>,
}

impl EmptyState {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self { id: id.into(), title: title.into(), body: None, children: vec![] }
    }

    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let s = t.text_scale();
        let p = t.palette;
        div()
            .id(self.id)
            .test_support()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .font_family(t.fonts.ui.clone())
            .text_size(s.px(12.))
            .text_color(p.muted)
            .child(div().text_size(s.px(15.)).font_weight(FontWeight::MEDIUM).text_color(p.text).child(self.title))
            .when_some(self.body, |d, b| d.child(b))
            .children(self.children)
    }
}

fn call(cb: Callback) -> impl Fn(&mut Window, &mut App) + 'static {
    move |w, cx| cb(w, cx)
}

pub fn no_matches(id: impl Into<SharedString>) -> EmptyState {
    EmptyState::new(id, "No matching lines").body("Enable an include filter or pin a line in the full log.")
}

/// A recent filter in the empty filtered view: its expression and its summary (`2 include · 1 exclude · regex · ±2`).
#[derive(Clone, Debug, PartialEq)]
pub struct RecentFilter {
    pub expression: SharedString,
    pub summary: SharedString,
}

/// `No matching lines`, then up to three recent filters (spec C2); a click applies one. Without any, the view is
/// `no_matches` unchanged.
pub fn no_matches_with_recent(id: impl Into<SharedString>, recent: Vec<RecentFilter>, on_pick: IndexCallback) -> EmptyState {
    let state = no_matches(id);
    if recent.is_empty() {
        return state;
    }
    state.child(RecentFilters { items: recent, on_pick })
}

#[derive(IntoElement)]
struct RecentFilters {
    items: Vec<RecentFilter>,
    on_pick: IndexCallback,
}

impl RenderOnce for RecentFilters {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let s = t.text_scale();
        let p = t.palette;
        let on_pick = self.on_pick;
        div()
            .id("recent-filters")
            .test_support()
            .mt(px(14.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .w(px(360.))
            .child(div().px(px(10.)).pb(px(4.)).text_size(s.px(10.)).text_color(p.faint).child("Recent filters"))
            .children(self.items.into_iter().enumerate().map(move |(i, f)| {
                let on_pick = on_pick.clone();
                div()
                    .id(SharedString::from(format!("recent-filter-{i}")))
                    .test_support()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(div().truncate().font_family(t.fonts.mono.clone()).text_size(s.px(12.)).text_color(p.text).child(f.expression))
                    .child(div().text_size(s.px(10.)).text_color(p.muted).child(f.summary))
                    .on_click(move |_, w, cx| on_pick(i, w, cx))
            }))
    }
}

pub fn welcome(recent: Vec<RecentItem>, on_open_file: Callback, on_open_folder: Callback, on_recent: IndexCallback) -> EmptyState {
    let mut state = EmptyState::new("welcome", "Open a log to start")
        .body("Drop files, folders or archives here, or pipe into rogg from a terminal.")
        .child(
            div()
                .flex()
                .gap(px(7.))
                .child(ActionButton::new("open-file", "Open file…  ⌘O").primary().on_click(call(on_open_file)))
                .child(ActionButton::new("open-folder", "Open folder…  ⇧⌘O").on_click(call(on_open_folder))),
        );
    if !recent.is_empty() {
        state = state.child(WelcomeRecent { items: recent, on_recent });
    }
    state
}

/// Recent entries under the welcome actions.
#[derive(IntoElement)]
struct WelcomeRecent {
    items: Vec<RecentItem>,
    on_recent: IndexCallback,
}

impl RenderOnce for WelcomeRecent {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let on_recent = self.on_recent;
        div().mt(px(14.)).flex().flex_col().gap(px(2.)).w(px(320.)).children(self.items.into_iter().enumerate().map(move |(i, item)| {
            let on_recent = on_recent.clone();
            let fg = if item.missing { p.faint } else { p.muted };
            div()
                .id(SharedString::from(format!("welcome-recent-{i}")))
                .test_support()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(10.))
                .py(px(7.))
                .rounded(px(5.))
                .text_color(fg)
                .cursor_pointer()
                .hover(move |s| s.bg(p.hover))
                .child(icon(if item.is_folder { IconName::Folder } else { IconName::File }).size(px(13.)).text_color(fg))
                .child(item.label)
                .on_click(move |_, w, cx| on_recent(i, w, cx))
        }))
    }
}

pub fn loading(label: impl Into<SharedString>, fraction: Option<f32>, on_cancel: Callback) -> EmptyState {
    EmptyState::new("loading", label)
        .child(LoadingBar { fraction })
        .child(ActionButton::new("cancel", "Cancel").on_click(call(on_cancel)))
}

pub fn error(message: impl Into<SharedString>, path: impl Into<SharedString>, on_retry: Callback) -> EmptyState {
    EmptyState::new("error", message).body(path).child(ActionButton::new("retry", "Retry").primary().on_click(call(on_retry)))
}

#[derive(IntoElement)]
struct LoadingBar {
    fraction: Option<f32>,
}

impl RenderOnce for LoadingBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        div()
            .w(px(220.))
            .h(px(3.))
            .rounded(px(2.))
            .bg(p.border)
            .child(
                div().id("loading-fill").test_support().h_full().rounded(px(2.)).bg(p.accent).w(relative(self.fraction.map_or(0.3, bar_fraction))),
            )
    }
}
