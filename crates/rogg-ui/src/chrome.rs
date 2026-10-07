use crate::controls::{FindField, TextButton, ToolButton};
use crate::icons::IconName;
use crate::theme::{ActiveRoggTheme, size};
use crate::{Callback, IndexCallback};
use gpui_kit::prelude::*;
use gpui_kit::{App, FontWeight, SharedString, TestSupportExt, Window, div, px, relative};
use std::rc::Rc;

/// `.meta`: breadcrumb on the left, `● Profile: X⌄ | Export` on the right.
#[derive(IntoElement)]
pub struct InfoBar {
    crumbs: Vec<SharedString>,
    profile: SharedString,
    snapshot: Option<SharedString>,
    on_crumb: Option<IndexCallback>,
    on_profile: Option<Callback>,
    on_export: Option<Callback>,
}

impl InfoBar {
    pub fn new(crumbs: Vec<SharedString>, profile: SharedString) -> Self {
        Self { crumbs, profile, snapshot: None, on_crumb: None, on_profile: None, on_export: None }
    }

    /// `Snapshot · 14:32:08` note shown for tabs opened from a live source's filtered view.
    pub fn snapshot(mut self, note: Option<SharedString>) -> Self {
        self.snapshot = note;
        self
    }

    pub fn on_crumb(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_crumb = Some(Rc::new(f));
        self
    }

    pub fn on_profile(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_profile = Some(Rc::new(f));
        self
    }

    pub fn on_export(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_export = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for InfoBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let sep = move |s: &'static str| div().flex_none().mx(px(3.)).text_color(p.faint).child(s);
        let count = self.crumbs.len();
        let on_profile = self.on_profile;
        let on_export = self.on_export;
        let mut bar = div()
            .id("meta")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(7.))
            .h(px(size::INFO_BAR_H))
            .px(px(13.))
            .border_b_1()
            .border_color(p.line)
            .text_size(px(11.))
            .text_color(p.muted)
            .whitespace_nowrap();
        for (i, crumb) in self.crumbs.into_iter().enumerate() {
            let on_crumb = self.on_crumb.clone();
            bar = bar.child(
                div()
                    .id(SharedString::from(format!("crumb-{i}")))
                    .test_support()
                    .min_w_0()
                    .truncate()
                    .cursor_pointer()
                    .child(crumb)
                    .on_click(move |_, window, cx| {
                        if let Some(f) = &on_crumb {
                            f(i, window, cx)
                        }
                    }),
            );
            if i + 1 < count {
                bar = bar.child(sep("›"));
            }
        }
        bar.when_some(self.snapshot, |d, s| d.child(div().id("snapshot").test_support().flex_none().text_color(p.faint).child(s))).child(
            div()
                .ml_auto()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(9.))
                .child(TextButton::meta("profile", format!("Profile: {}⌄", self.profile)).dot().on_click(move |w, cx| {
                    if let Some(f) = &on_profile {
                        f(w, cx)
                    }
                }))
                .child(sep("|"))
                .child(TextButton::meta("export", "Export").icon(IconName::Export).on_click(move |w, cx| {
                    if let Some(f) = &on_export {
                        f(w, cx)
                    }
                })),
        )
    }
}

/// `.pane-head`: title, count, then right-aligned find field and tool buttons.
#[derive(IntoElement)]
pub struct PaneHeader {
    id: SharedString,
    title: SharedString,
    count: SharedString,
    find: Option<FindField>,
    tools: Vec<ToolButton>,
}

impl PaneHeader {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>, count: impl Into<SharedString>) -> Self {
        Self { id: id.into(), title: title.into(), count: count.into(), find: None, tools: vec![] }
    }

    pub fn find(mut self, find: FindField) -> Self {
        self.find = Some(find);
        self
    }

    pub fn tool(mut self, tool: ToolButton) -> Self {
        self.tools.push(tool);
        self
    }
}

impl RenderOnce for PaneHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(9.))
            .h(t.metrics.pane_header_height)
            .px(px(12.))
            .border_b_1()
            .border_color(p.line)
            .whitespace_nowrap()
            .child(div().min_w_0().truncate().text_size(px(11.)).font_weight(FontWeight(550.)).text_color(p.text).child(self.title))
            .child(div().flex_none().text_size(px(10.)).text_color(p.faint).child(self.count))
            .child(div().ml_auto().flex_none().flex().items_center().gap(px(6.)).children(self.find).children(self.tools))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StatPart {
    Strong(SharedString),
    Text(SharedString),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub label: SharedString,
    pub fraction: f32,
}

/// A progress fraction as a bar width: clamped to `0..=1`, and empty for NaN or ±∞ (e.g. 0/0 on an empty file).
pub(crate) fn bar_fraction(f: f32) -> f32 {
    if f.is_finite() { f.clamp(0., 1.) } else { 0. }
}

/// `.footer`: statistics, selected line, then progress and the encoding note on the right.
#[derive(IntoElement)]
pub struct StatusBar {
    note: SharedString,
    stats: Vec<StatPart>,
    selection: Option<SharedString>,
    progress: Option<Progress>,
    on_stats: Option<Callback>,
}

impl StatusBar {
    pub fn new(note: SharedString) -> Self {
        Self { note, stats: vec![], selection: None, progress: None, on_stats: None }
    }

    pub fn stats(mut self, stats: Vec<StatPart>) -> Self {
        self.stats = stats;
        self
    }

    /// Makes the statistics clickable (opens the statistics popover, spec 4.11).
    pub fn on_stats(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stats = Some(Rc::new(f));
        self
    }

    pub fn selection(mut self, selection: Option<SharedString>) -> Self {
        self.selection = selection;
        self
    }

    pub fn progress(mut self, progress: Option<Progress>) -> Self {
        self.progress = progress;
        self
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        div()
            .id("statusbar")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(15.))
            .h(px(size::STATUS_BAR_H))
            .px(px(12.))
            .bg(p.side)
            .border_t_1()
            .border_color(p.border)
            .text_size(px(10.))
            .text_color(p.muted)
            .whitespace_nowrap()
            .child(
                div()
                    .id("footer-stats")
                    .test_support()
                    .flex()
                    .items_center()
                    .when_some(self.on_stats, |d, f| d.cursor_pointer().on_click(move |_, window, cx| f(window, cx)))
                    .children(self.stats.into_iter().map(|part| match part {
                        StatPart::Strong(s) => div().font_weight(FontWeight::MEDIUM).text_color(p.text).child(s),
                        StatPart::Text(s) => div().child(s),
                    })),
            )
            .when_some(self.selection, |d, s| d.child(div().child(s)))
            .child(
                div()
                    .ml_auto()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .when_some(self.progress, |d, pr| {
                        d.child(
                            div()
                                .id("progress")
                                .test_support()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div().flex_none().w(px(80.)).h(px(3.)).rounded(px(2.)).bg(p.border).child(
                                        div()
                                            .id("progress-fill")
                                            .test_support()
                                            .h_full()
                                            .rounded(px(2.))
                                            .bg(p.accent)
                                            .w(relative(bar_fraction(pr.fraction))),
                                    ),
                                )
                                .child(pr.label),
                        )
                    })
                    .child(div().text_color(p.faint).child(self.note)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::bar_fraction;

    #[test]
    fn bar_fraction_clamps_and_zeroes_non_finite() {
        for (f, expected) in [(0.25, 0.25), (1.5, 1.), (-0.5, 0.), (f32::NAN, 0.), (f32::INFINITY, 0.), (f32::NEG_INFINITY, 0.)] {
            assert_eq!(bar_fraction(f), expected, "fraction {f}");
        }
    }
}
