use crate::controls::IconButton;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, ChromeStyle, Color, Palette, size};
use crate::{Callback, IndexCallback};
use gpui_kit::prelude::*;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenu};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{
    AnyElement, App, Context, Decorations, FontWeight, MouseButton, Pixels, ScrollHandle, SharedString, TestSupportExt, Window, WindowControlArea, div, px,
};
use std::cell::Cell;
use std::rc::Rc;

/// Fills the ☰ app menu each time it opens (gpui-kit's dropdown menu builder).
pub type AppMenuBuilder = Rc<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;
/// Builds a tab's context menu; gets the right-clicked tab's index.
pub type TabMenuBuilder = Rc<dyn Fn(usize, PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

#[derive(Clone, Debug, PartialEq)]
pub struct TabItem {
    pub label: SharedString,
    pub tooltip: SharedString,
    pub active: bool,
}

/// The 42px title bar: traffic-light block + sidebar toggle + brand (as wide as the sidebar), compact tabs, drag area, actions.
/// In the Linux and Windows chrome styles the traffic-light block gives way to the ☰ app menu, and rogg-drawn window
/// controls follow the actions.
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
    app_menu: Option<AppMenuBuilder>,
    tab_menu: Option<TabMenuBuilder>,
    on_close_window: Option<Callback>,
    force_window_controls: bool,
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
            app_menu: None,
            tab_menu: None,
            on_close_window: None,
            force_window_controls: false,
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

    /// The ☰ menu's items (Linux and Windows styles; the Mac style uses the native menu bar).
    pub fn app_menu(mut self, f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static) -> Self {
        self.app_menu = Some(Rc::new(f));
        self
    }

    /// A tab's right-click menu. Right-clicking does not select the tab; the menu acts on the tab it was opened on.
    pub fn tab_menu(mut self, f: impl Fn(usize, PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static) -> Self {
        self.tab_menu = Some(Rc::new(f));
        self
    }

    /// What the drawn close control does in the Linux style; it removes the window when unset. `Window::remove_window`
    /// skips `on_window_should_close`, so the app saves its state here first. (On Windows the system closes the window
    /// through `WM_CLOSE`, which runs `on_window_should_close`.)
    pub fn on_close_window(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close_window = Some(Rc::new(f));
        self
    }

    /// Draw the Linux style's window controls even when the window is server-decorated: for mock-ups of a window in
    /// one (gallery captures, tests), whose host window has no client-side decorations.
    pub fn force_window_controls(mut self, on: bool) -> Self {
        self.force_window_controls = on;
        self
    }
}

fn button(b: IconButton, cb: Option<Callback>) -> IconButton {
    match cb {
        Some(f) => b.on_click(move |w, cx| f(w, cx)),
        None => b,
    }
}

/// Mac and Windows styles: a press drags the window; a double-click runs gpui's `titlebar_double_click`, which acts only
/// on macOS (it follows the system setting there). Windows leaves the drag area to the native caption
/// (`WindowControlArea::Drag`).
fn drag_window(e: &gpui_kit::MouseDownEvent, window: &mut Window, _: &mut App) {
    if e.click_count >= 2 {
        window.titlebar_double_click();
    } else {
        window.start_window_move();
    }
}

/// Makes `el` a title-bar drag area. Mac and Windows styles: `drag_window` on the press. Linux style (`pressed` is
/// `Some`): a press only arms the drag, and the move starts on the first mouse move with the left button held. Handing
/// the pointer to the window manager on the press (`_NET_WM_MOVERESIZE`, `xdg_toplevel.move`) would swallow the second
/// press of a double-click, which zooms (maximises or restores) the window. Zed's title bar does the same.
fn drag_area<E: InteractiveElement>(el: E, pressed: Option<Rc<Cell<bool>>>) -> E {
    let Some(pressed) = pressed else { return el.on_mouse_down(MouseButton::Left, drag_window) };
    let (down, up, out, moved) = (pressed.clone(), pressed.clone(), pressed.clone(), pressed);
    el.on_mouse_down(MouseButton::Left, move |e, window, _| {
        down.set(e.click_count < 2);
        if e.click_count >= 2 {
            window.zoom_window();
        }
    })
    .on_mouse_up(MouseButton::Left, move |_, _, _| up.set(false))
    .on_mouse_down_out(move |_, _, _| out.set(false))
    .on_mouse_move(move |e, window, _| {
        if moved.get() && e.pressed_button == Some(MouseButton::Left) {
            moved.set(false);
            window.start_window_move();
        }
    })
}

/// The Linux style's armed-drag flag for drag area `key`, kept across frames (the title bar is rebuilt every frame);
/// `None` in the other styles.
fn drag_state(key: &'static str, chrome: ChromeStyle, window: &mut Window, cx: &mut App) -> Option<Rc<Cell<bool>>> {
    (chrome == ChromeStyle::Linux).then(|| window.use_keyed_state(key, cx, |_, _| Rc::new(Cell::new(false))).read(cx).clone())
}

/// The ☰ button opening the app menu, sized like the other title-bar icon buttons.
fn app_menu_button(menu: AppMenuBuilder, p: Palette, cx: &App) -> AnyElement {
    let variant = ButtonCustomVariant::new(cx).color(Color::TRANSPARENT.hsla()).foreground(p.muted.hsla()).hover(p.hover.hsla()).active(p.hover.hsla()).shadow(false);
    div()
        .id("app-menu")
        .test_support()
        .flex_none()
        .child(
            Button::new("app-menu-button")
                .custom(variant)
                .size(px(size::TITLEBAR_ICON_BTN))
                .rounded(px(5.))
                .child(icon(IconName::Menu).size(px(15.)).text_color(p.muted))
                .dropdown_menu(move |m, window, cx| menu(m, window, cx)),
        )
        .into_any_element()
}

/// Minimise, maximise/restore and close, drawn by rogg for the Linux and Windows styles: flush 46 px rectangles with a
/// red close hover on Windows, 24 px circles with gaps on Linux. On Windows the control areas give the native
/// hit-testing (snap layouts) and the system runs the actions after the element's handlers, so the controls register no
/// clicks there; on Linux their clicks act.
fn window_controls(chrome: ChromeStyle, maximized: bool, on_close: Option<Callback>, p: Palette) -> AnyElement {
    let windows = chrome == ChromeStyle::Windows;
    let button = |id: &'static str, name: IconName, area: WindowControlArea| {
        let red_close = windows && area == WindowControlArea::Close;
        let on_close = on_close.clone();
        // An svg paints with its own text colour, so the close glyph turns white through a group hover.
        let glyph = icon(name).size(px(if windows { 14. } else { 12. })).text_color(p.muted);
        div()
            .id(id)
            .test_support()
            .group(id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .when(windows, |d| d.w(px(46.)).h(px(size::TITLEBAR_H)))
            .when(!windows, |d| d.size(px(24.)).rounded_full())
            .window_control_area(area)
            .hover(move |s| s.bg(if red_close { Color::rgb(0xc42b1c) } else { p.hover }))
            .child(if red_close { glyph.group_hover(id, |s| s.text_color(Color::rgb(0xffffff))) } else { glyph })
            .when(!windows, |d| {
                d.on_click(move |_, window, cx| match area {
                    WindowControlArea::Min => window.minimize_window(),
                    WindowControlArea::Max => window.zoom_window(),
                    WindowControlArea::Close => match &on_close {
                        Some(f) => f(window, cx),
                        None => window.remove_window(),
                    },
                    WindowControlArea::Drag => {}
                })
            })
    };
    div()
        .id("window-controls")
        .flex()
        .flex_none()
        .items_center()
        .when(!windows, |d| d.gap(px(8.)).pl(px(8.)))
        .child(button("window-minimize", IconName::Minimize, WindowControlArea::Min))
        .child(button("window-maximize", if maximized { IconName::Restore } else { IconName::Maximize }, WindowControlArea::Max))
        .child(button("window-close", IconName::Close, WindowControlArea::Close))
        .into_any_element()
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let s = cx.rogg().text_scale();
        let chrome = cx.rogg().chrome;
        let windows = chrome == ChromeStyle::Windows;
        // Linux draws its controls only for client-side decorations: X11 without a compositor, and some Wayland
        // compositors, keep server-side ones with their own controls.
        let controls = match chrome {
            ChromeStyle::Mac => false,
            ChromeStyle::Windows => true,
            ChromeStyle::Linux => self.force_window_controls || matches!(window.window_decorations(), Decorations::Client { .. }),
        };
        let (brand_drag, window_drag) = (drag_state("brand-drag", chrome, window, cx), drag_state("window-drag-state", chrome, window, cx));
        let lead = if chrome.is_mac() {
            Some(
                div()
                    .id("traffic")
                    .test_support()
                    .on_mouse_down(MouseButton::Left, drag_window)
                    .w(px(53.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .when(self.traffic_lights, |d| d.children(p.traffic.map(|c| div().size(px(10.)).rounded_full().bg(c))))
                    .into_any_element(),
            )
        } else {
            self.app_menu.map(|menu| app_menu_button(menu, p, cx))
        };
        let leading = div()
            .id("titlebar-leading")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.))
            .w(self.sidebar_width)
            .px(px(13.))
            .children(lead)
            .child(button(IconButton::icon("toggle-sidebar", IconName::Side), self.on_toggle_sidebar))
            .child(drag_area(div().id("brand").test_support().text_size(s.px(14.)).font_weight(FontWeight(600.)).text_color(p.muted).child("rogg"), brand_drag));

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
                    menu: self.tab_menu.clone(),
                }
            }));

        div()
            .id("titlebar")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .h(px(size::TITLEBAR_H))
            .pr(px(if windows { 0. } else { 10. }))
            .bg(p.side)
            .border_b_1()
            .border_color(p.border)
            .child(leading)
            .child(
                div().flex().flex_1().min_w_0().h_full().items_center().gap(px(5.)).child(tab_strip).child(drag_area(
                    div()
                        .id("window-drag")
                        .test_support()
                        .flex_1()
                        .min_w(px(28.))
                        .h_full()
                        .when(windows, |d| d.window_control_area(WindowControlArea::Drag)),
                    window_drag,
                )),
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
                    .child(button(IconButton::chevron_down("all-tabs").compact(), self.on_all_tabs))
                    .child(button(
                        if chrome.is_mac() { IconButton::glyph("shortcuts", "⌘") } else { IconButton::icon("shortcuts", IconName::Keyboard) }.compact(),
                        self.on_shortcuts,
                    ))
                    .child(button(IconButton::icon("settings", IconName::Settings).compact(), self.on_settings)),
            )
            .when(controls, |d| d.child(window_controls(chrome, window.is_maximized(), self.on_close_window, p)))
    }
}

/// `.tab-shell`: tab button + hover-revealed close button + accent underline when active. Middle-click closes;
/// right-click opens the tab menu.
#[derive(IntoElement)]
struct TabShell {
    index: usize,
    item: TabItem,
    force_hover: bool,
    on_select: Option<IndexCallback>,
    on_close: Option<IndexCallback>,
    menu: Option<TabMenuBuilder>,
}

impl RenderOnce for TabShell {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let s = cx.rogg().text_scale();
        let i = self.index;
        let active = self.item.active;
        let group: SharedString = format!("tab-group-{i}").into();
        let (on_select, on_close, on_middle) = (self.on_select, self.on_close.clone(), self.on_close);
        let menu = self.menu;
        let shell = div()
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
                    .text_size(s.px(11.))
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
                    .text_size(s.px(14.))
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
            });
        match menu {
            Some(menu) => shell.context_menu(move |m, window, cx| menu(i, m, window, cx)).into_any_element(),
            None => shell.into_any_element(),
        }
    }
}
