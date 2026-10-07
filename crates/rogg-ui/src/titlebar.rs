use crate::controls::IconButton;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, Color, size};
use crate::{Callback, IndexCallback};
use gpui_kit::prelude::*;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{App, FontWeight, MouseButton, Pixels, ScrollHandle, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct TabItem {
    pub label: SharedString,
    pub tooltip: SharedString,
    pub active: bool,
}

/// The 42px title bar: traffic-light block + sidebar toggle + brand (as wide as the sidebar), compact tabs, drag area, actions.
#[derive(IntoElement)]
pub struct TitleBar {
    tabs: Vec<TabItem>,
    sidebar_width: Pixels,
    traffic_lights: bool,
    force_hover_tab: Option<usize>,
    tab_scroll: Option<ScrollHandle>,
    on_select_tab: Option<IndexCallback>,
    on_close_tab: Option<IndexCallback>,
    on_toggle_sidebar: Option<Callback>,
    on_all_tabs: Option<Callback>,
    on_shortcuts: Option<Callback>,
    on_settings: Option<Callback>,
}

impl TitleBar {
    pub fn new(tabs: Vec<TabItem>) -> Self {
        Self {
            tabs,
            sidebar_width: px(size::SIDEBAR_W),
            traffic_lights: false,
            force_hover_tab: None,
            tab_scroll: None,
            on_select_tab: None,
            on_close_tab: None,
            on_toggle_sidebar: None,
            on_all_tabs: None,
            on_shortcuts: None,
            on_settings: None,
        }
    }

    pub fn sidebar_width(mut self, width: Pixels) -> Self {
        self.sidebar_width = width;
        self
    }

    pub fn traffic_lights(mut self, draw: bool) -> Self {
        self.traffic_lights = draw;
        self
    }

    pub fn force_hover_tab(mut self, index: Option<usize>) -> Self {
        self.force_hover_tab = index;
        self
    }

    /// Track the tab strip's scroll so the app can reveal the active tab (`ScrollHandle::scroll_to_item`).
    pub fn tab_scroll(mut self, handle: ScrollHandle) -> Self {
        self.tab_scroll = Some(handle);
        self
    }

    pub fn on_select_tab(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select_tab = Some(Rc::new(f));
        self
    }

    pub fn on_close_tab(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_close_tab = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_sidebar(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_sidebar = Some(Rc::new(f));
        self
    }

    pub fn on_all_tabs(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_all_tabs = Some(Rc::new(f));
        self
    }

    pub fn on_shortcuts(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_shortcuts = Some(Rc::new(f));
        self
    }

    pub fn on_settings(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_settings = Some(Rc::new(f));
        self
    }
}

fn button(b: IconButton, cb: Option<Callback>) -> IconButton {
    match cb {
        Some(f) => b.on_click(move |w, cx| f(w, cx)),
        None => b,
    }
}

/// A press drags the window; a double-click zooms it like a native title bar.
fn drag_window(e: &gpui_kit::MouseDownEvent, window: &mut Window, _: &mut App) {
    if e.click_count >= 2 {
        window.titlebar_double_click();
    } else {
        window.start_window_move();
    }
}

impl RenderOnce for TitleBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let traffic = div().id("traffic").on_mouse_down(MouseButton::Left, drag_window).w(px(53.)).flex_none().flex().items_center().gap(px(7.)).when(self.traffic_lights, |d| {
            d.children(p.traffic.map(|c| div().size(px(10.)).rounded_full().bg(c)))
        });
        let leading = div()
            .id("titlebar-leading")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.))
            .w(self.sidebar_width)
            .px(px(13.))
            .child(traffic)
            .child(button(IconButton::icon("toggle-sidebar", IconName::Side), self.on_toggle_sidebar))
            .child(div().id("brand").text_size(px(14.)).font_weight(FontWeight(600.)).text_color(p.muted).on_mouse_down(MouseButton::Left, drag_window).child("rogg"));

        let tab_strip = div()
            .id("tabs")
            .test_support()
            .flex()
            .flex_shrink(1.)
            .min_w_0()
            .items_center()
            .h(px(30.))
            .gap(px(3.))
            .overflow_x_scroll()
            .when_some(self.tab_scroll, |d, h| d.track_scroll(&h))
            .children(self.tabs.into_iter().enumerate().map(|(i, tab)| {
                TabShell {
                    index: i,
                    item: tab,
                    force_hover: self.force_hover_tab == Some(i),
                    on_select: self.on_select_tab.clone(),
                    on_close: self.on_close_tab.clone(),
                }
            }));

        div()
            .id("titlebar")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .h(px(size::TITLEBAR_H))
            .pr(px(10.))
            .bg(p.side)
            .border_b_1()
            .border_color(p.border)
            .child(leading)
            .child(
                div().flex().flex_1().min_w_0().h_full().items_center().gap(px(5.)).child(tab_strip).child(
                    div()
                        .id("window-drag")
                        .flex_1()
                        .min_w(px(28.))
                        .h_full()
                        .on_mouse_down(MouseButton::Left, drag_window),
                ),
            )
            .child(
                div()
                    .id("title-actions")
                    .test_support()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(4.))
                    .pl(px(5.))
                    .child(button(IconButton::glyph("all-tabs", "⌄").compact(), self.on_all_tabs))
                    .child(button(IconButton::glyph("shortcuts", "⌘").compact(), self.on_shortcuts))
                    .child(button(IconButton::icon("settings", IconName::Settings).compact(), self.on_settings)),
            )
    }
}

/// `.tab-shell`: tab button + hover-revealed close button + accent underline when active. Middle-click closes.
#[derive(IntoElement)]
struct TabShell {
    index: usize,
    item: TabItem,
    force_hover: bool,
    on_select: Option<IndexCallback>,
    on_close: Option<IndexCallback>,
}

impl RenderOnce for TabShell {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let i = self.index;
        let active = self.item.active;
        let group: SharedString = format!("tab-group-{i}").into();
        let (on_select, on_close, on_middle) = (self.on_select, self.on_close.clone(), self.on_close);
        div()
            .id(SharedString::from(format!("tab-shell-{i}")))
            .test_support()
            .group(group.clone())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(size::TAB_H))
            .max_w(px(size::TAB_MAX_W))
            .pr(px(3.))
            .rounded(px(5.))
            .border_1()
            .map(|d| {
                if active {
                    // The border is drawn by an overlay child (below), so the accent underline can paint over it:
                    // gpui paints an element's own border on top of its children.
                    d.bg(p.raised).border_color(Color::TRANSPARENT)
                } else if self.force_hover {
                    d.bg(p.hover).border_color(Color::TRANSPARENT)
                } else {
                    d.border_color(Color::TRANSPARENT).hover(move |s| s.bg(p.hover))
                }
            })
            .on_mouse_down(MouseButton::Middle, move |_, window, cx| {
                if let Some(f) = &on_middle {
                    f(i, window, cx)
                }
            })
            .child(
                div()
                    .id(SharedString::from(format!("tab-{i}")))
                    .test_support()
                    .flex()
                    .min_w_0()
                    .items_center()
                    .gap(px(6.))
                    .px(px(7.))
                    .h(px(27.))
                    .max_w(px(size::TAB_LABEL_MAX_W))
                    .text_size(px(11.))
                    .text_color(if active { p.text } else { p.muted })
                    .cursor_pointer()
                    .when(!self.item.tooltip.is_empty(), |d| {
                        let tip = self.item.tooltip.clone();
                        d.tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                    })
                    .child(icon(IconName::File).size(px(12.)).text_color(if active { p.accent } else { p.faint }))
                    .child(div().min_w_0().truncate().child(self.item.label))
                    .on_click(move |_, window, cx| {
                        if let Some(f) = &on_select {
                            f(i, window, cx)
                        }
                    }),
            )
            .child(
                div()
                    .id(SharedString::from(format!("tab-close-{i}")))
                    .test_support()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(px(18.))
                    .h(px(20.))
                    .rounded(px(5.))
                    .text_size(px(14.))
                    .text_color(p.muted)
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(div().when(!active && !self.force_hover, |d| d.opacity(0.).group_hover(group, |s| s.opacity(1.))).child("×"))
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        if let Some(f) = &on_close {
                            f(i, window, cx)
                        }
                    }),
            )
            .when(active, |d| {
                d.child(div().absolute().top(px(-1.)).bottom(px(-1.)).left(px(-1.)).right(px(-1.)).rounded(px(5.)).border_1().border_color(p.border))
                    .child(div().absolute().bottom(px(-1.)).left(px(9.)).right(px(9.)).h(px(1.)).bg(p.accent).opacity(0.7))
            })
    }
}
