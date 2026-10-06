use crate::IndexCallback;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, Palette, size};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, FontWeight, Pixels, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum TreeRow {
    Folder { label: SharedString, expanded: bool, depth: usize },
    File { label: SharedString, size: SharedString, active: bool, depth: usize, failed: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecentItem {
    pub label: SharedString,
    pub is_folder: bool,
    pub missing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SidebarFooter {
    pub title: SharedString,
    pub lines: Vec<SharedString>,
}

/// The file browser: FILES header, archive/folder tree, RECENT list and bundle summary footer.
#[derive(IntoElement)]
pub struct Sidebar {
    width: Pixels,
    count: SharedString,
    rows: Vec<TreeRow>,
    recent: Vec<RecentItem>,
    footer: Option<SidebarFooter>,
    on_row: Option<IndexCallback>,
    on_recent: Option<IndexCallback>,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            width: px(size::SIDEBAR_W),
            count: SharedString::default(),
            rows: vec![],
            recent: vec![],
            footer: None,
            on_row: None,
            on_recent: None,
        }
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    pub fn count(mut self, count: SharedString) -> Self {
        self.count = count;
        self
    }

    pub fn rows(mut self, rows: Vec<TreeRow>) -> Self {
        self.rows = rows;
        self
    }

    pub fn recent(mut self, recent: Vec<RecentItem>) -> Self {
        self.recent = recent;
        self
    }

    pub fn footer(mut self, footer: Option<SidebarFooter>) -> Self {
        self.footer = footer;
        self
    }

    pub fn on_row(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_row = Some(Rc::new(f));
        self
    }

    pub fn on_recent(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_recent = Some(Rc::new(f));
        self
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

/// `.small-label`: 10px uppercase, 600 weight, muted.
fn small_label(text: &'static str, p: &Palette) -> gpui_kit::Div {
    div().text_size(px(10.)).font_weight(FontWeight(600.)).text_color(p.muted).child(text.to_uppercase())
}

fn tree_row(i: usize, row: TreeRow, p: Palette, on_row: Option<IndexCallback>) -> AnyElement {
    let base = div()
        .id(SharedString::from(format!("tree-row-{i}")))
        .test_support()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(size::TREE_ROW_H))
        .w_full()
        .whitespace_nowrap()
        .cursor_pointer()
        .on_click(move |_, window, cx| {
            if let Some(f) = &on_row {
                f(i, window, cx)
            }
        });
    match row {
        TreeRow::Folder { label, expanded, depth } => base
            .pl(px(9. + depth as f32 * 18.))
            .pr(px(9.))
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.text)
            .rounded(px(4.))
            .hover(move |s| s.bg(p.hover))
            .child(div().flex_none().text_color(p.muted).child(if expanded { "⌄" } else { "›" }))
            .child(icon(IconName::Folder).size(px(13.)).text_color(p.muted))
            .child(div().min_w_0().truncate().child(label))
            .into_any_element(),
        TreeRow::File { label, size: file_size, active, depth, failed } => base
            .pl(px(27. + depth.saturating_sub(1) as f32 * 18.))
            .pr(px(9.))
            .rounded(px(4.))
            .text_size(px(12.))
            .text_color(if active {
                p.text
            } else if failed {
                p.error
            } else {
                p.muted
            })
            .when(active, |d| d.bg(p.selection))
            .when(!active, |d| d.hover(move |s| s.bg(p.hover)))
            .child(icon(IconName::File).size(px(13.)).text_color(if active { p.accent } else { p.faint }))
            .child(div().min_w_0().truncate().child(label))
            .child(div().ml_auto().flex_none().text_size(px(10.)).text_color(p.faint).child(file_size))
            .into_any_element(),
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let on_row = self.on_row.clone();
        let on_recent = self.on_recent.clone();
        div()
            .id("sidebar")
            .test_support()
            .flex()
            .flex_col()
            .flex_none()
            .w(self.width)
            .h_full()
            .bg(p.side)
            .border_r_1()
            .border_color(p.border)
            .child(
                div()
                    .id("side-top")
                    .test_support()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .h(px(size::SIDE_TOP_H))
                    .px(px(15.))
                    .child(small_label("Files", &p))
                    .child(div().text_size(px(10.)).text_color(p.faint).child(self.count)),
            )
            .child(
                div()
                    .id("side-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(7.))
                    .py(px(5.))
                    .children(self.rows.into_iter().enumerate().map(|(i, row)| tree_row(i, row, p, on_row.clone())))
                    .child(div().mt(px(23.)).mx(px(9.)).mb(px(7.)).child(small_label("Recent", &p)))
                    .children(self.recent.into_iter().enumerate().map(|(i, item)| {
                        let on_recent = on_recent.clone();
                        div()
                            .id(SharedString::from(format!("recent-{i}")))
                            .test_support()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(10.))
                            .py(px(8.))
                            .text_size(px(11.))
                            .text_color(if item.missing { p.faint } else { p.muted })
                            .cursor_pointer()
                            .child(
                                icon(if item.is_folder { IconName::Folder } else { IconName::File })
                                    .size(px(15.))
                                    .text_color(if item.missing { p.faint } else { p.muted }),
                            )
                            .child(div().min_w_0().truncate().child(item.label))
                            .on_click(move |_, window, cx| {
                                if let Some(f) = &on_recent {
                                    f(i, window, cx)
                                }
                            })
                    })),
            )
            .when_some(self.footer, |d, footer| {
                d.child(
                    div()
                        .id("side-bottom")
                        .test_support()
                        .flex_none()
                        .px(px(15.))
                        .py(px(12.))
                        .border_t_1()
                        .border_color(p.border)
                        .text_size(px(10.))
                        .line_height(px(18.))
                        .text_color(p.muted)
                        .child(div().font_weight(FontWeight::MEDIUM).text_color(p.text).child(footer.title))
                        .children(footer.lines.into_iter().map(|l| div().child(l))),
                )
            })
    }
}
