use crate::theme::{ActiveRoggTheme, size};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Bounds, CursorStyle, EmptyView, Pixels, TestSupportExt, Window, canvas, div, px, relative};
use std::cell::Cell;
use std::rc::Rc;

pub struct MainWindowParts {
    pub titlebar: AnyElement,
    pub sidebar: Option<AnyElement>,
    pub info_bar: AnyElement,
    pub top_header: AnyElement,
    pub top_body: AnyElement,
    pub band: AnyElement,
    pub bottom_header: AnyElement,
    pub bottom_body: AnyElement,
    pub status: AnyElement,
    pub overlay: Option<AnyElement>,
    /// Top pane share of the height left after fixed rows (reference `flex:1.1` vs `flex:1`).
    pub split: f32,
}

impl MainWindowParts {
    pub const DEFAULT_SPLIT: f32 = 1.1 / 2.1;
}

/// Window content without the log panes: the welcome view, and a tab that is loading or failed.
pub struct SingleParts {
    pub titlebar: AnyElement,
    pub sidebar: Option<AnyElement>,
    pub info_bar: Option<AnyElement>,
    pub body: AnyElement,
    pub status: Option<AnyElement>,
    pub overlay: Option<AnyElement>,
}

/// Drag payload of the filter band handle; the app turns it into a split ratio with `LayoutMeasure`.
#[derive(Clone, Copy, Debug)]
pub struct BandDrag;

/// Drag payload of the sidebar's right edge; the app turns the pointer x into a sidebar width.
#[derive(Clone, Copy, Debug)]
pub struct SidebarDrag;

/// Bounds of the main column and the filter band, recorded during layout so a band drag can be mapped to a
/// split ratio.
#[derive(Default)]
pub struct LayoutMeasure {
    main: Cell<Bounds<Pixels>>,
    band: Cell<Bounds<Pixels>>,
}

impl LayoutMeasure {
    /// The split that puts the band's top edge at window `y`, clamped to `0.1..=0.9`; None before the first layout.
    pub fn split_at(&self, y: Pixels) -> Option<f32> {
        let (main, band) = (self.main.get(), self.band.get());
        let panes = f32::from(main.size.height) - size::INFO_BAR_H - size::STATUS_BAR_H - f32::from(band.size.height);
        if panes < 1. {
            return None;
        }
        let top = f32::from(main.top()) + size::INFO_BAR_H;
        Some(((f32::from(y) - top) / panes).clamp(0.1, 0.9))
    }
}

enum Body {
    Panes(MainWindowParts),
    Single(SingleParts),
}

/// Composes the window exactly like the reference `.app`: title bar, then sidebar + main column.
#[derive(IntoElement)]
pub struct MainWindowLayout {
    body: Body,
    measure: Option<Rc<LayoutMeasure>>,
    sidebar_resizable: bool,
}

impl MainWindowLayout {
    pub fn new(parts: MainWindowParts) -> Self {
        Self { body: Body::Panes(parts), measure: None, sidebar_resizable: false }
    }

    pub fn single(parts: SingleParts) -> Self {
        Self { body: Body::Single(parts), measure: None, sidebar_resizable: false }
    }

    /// Record the main column and band bounds into `measure` on every layout.
    pub fn measure(mut self, measure: Rc<LayoutMeasure>) -> Self {
        self.measure = Some(measure);
        self
    }

    /// Show a drag edge on the sidebar's right border.
    pub fn sidebar_resizable(mut self, on: bool) -> Self {
        self.sidebar_resizable = on;
        self
    }
}

/// An invisible element filling its (relative) parent that records the parent's bounds during prepaint.
fn recorder(measure: Rc<LayoutMeasure>, band: bool) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            if band {
                measure.band.set(bounds)
            } else {
                measure.main.set(bounds)
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

fn side(sidebar: Option<AnyElement>, resizable: bool) -> Option<AnyElement> {
    sidebar.map(|s| {
        div()
            .relative()
            .flex_none()
            .h_full()
            .child(s)
            .when(resizable, |d| {
                d.child(
                    div()
                        .id("sidebar-resize")
                        .test_support()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right(px(-3.))
                        .w(px(6.))
                        .cursor(CursorStyle::ResizeColumn)
                        .on_drag(SidebarDrag, |_, _, _, cx| cx.new(|_| EmptyView)),
                )
            })
            .into_any_element()
    })
}

impl RenderOnce for MainWindowLayout {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        let root = div()
            .id("app")
            .test_support()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.text)
            .font_family(t.fonts.ui.clone())
            .text_size(px(13.))
            .line_height(relative(size::LINE_HEIGHT));
        match self.body {
            Body::Panes(parts) => {
                let split = parts.split.clamp(0.1, 0.9);
                let pane = |id: &'static str, grow: f32, header: AnyElement, body: AnyElement| {
                    div()
                        .id(id)
                        .test_support()
                        .relative()
                        .flex()
                        .flex_col()
                        .flex_grow(grow)
                        .flex_shrink(1.)
                        .flex_basis(relative(0.))
                        .min_h_0()
                        .child(header)
                        .child(div().flex_1().min_h_0().child(body))
                };
                let band = div().relative().flex_none().child(parts.band).when_some(self.measure.clone(), |d, m| d.child(recorder(m, true)));
                root.child(parts.titlebar)
                    .child(
                        div()
                            .id("workspace")
                            .test_support()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .children(side(parts.sidebar, self.sidebar_resizable))
                            .child(
                                div()
                                    .id("main")
                                    .test_support()
                                    .relative()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .child(parts.info_bar)
                                    .child(pane("pane-top", split, parts.top_header, parts.top_body))
                                    .child(band)
                                    .child(pane("pane-bottom", 1. - split, parts.bottom_header, parts.bottom_body))
                                    .child(parts.status)
                                    .when_some(self.measure, |d, m| d.child(recorder(m, false))),
                            ),
                    )
                    .children(parts.overlay)
            }
            Body::Single(parts) => root
                .child(parts.titlebar)
                .child(
                    div()
                        .id("workspace")
                        .test_support()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .children(side(parts.sidebar, self.sidebar_resizable))
                        .child(
                            div()
                                .id("main")
                                .test_support()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .children(parts.info_bar)
                                .child(div().flex_1().min_h_0().child(parts.body))
                                .children(parts.status),
                        ),
                )
                .children(parts.overlay),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{point, size as gsize};

    #[test]
    fn split_at_maps_the_band_top_to_a_ratio() {
        let m = LayoutMeasure::default();
        assert_eq!(m.split_at(px(300.)), None, "no layout yet");
        // main column 600 px tall from y = 42; band 80 px tall: panes share 600 - 33 - 29 - 80 = 458 px from y = 75.
        m.main.set(Bounds::new(point(px(211.), px(42.)), gsize(px(1000.), px(600.))));
        m.band.set(Bounds::new(point(px(211.), px(300.)), gsize(px(1000.), px(80.))));
        let half = m.split_at(px(75. + 229.)).unwrap();
        assert!((half - 0.5).abs() < 1e-4, "{half}");
        assert_eq!(m.split_at(px(0.)), Some(0.1));
        assert_eq!(m.split_at(px(2000.)), Some(0.9));
    }
}
