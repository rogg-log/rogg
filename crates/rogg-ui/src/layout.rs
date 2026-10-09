use crate::theme::{ActiveRoggTheme, size};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Bounds, CursorStyle, Decorations, EmptyView, MouseButton, Pixels, Point, ResizeEdge, TestSupportExt, Tiling, Window, canvas, div,
    px, relative,
};
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

/// The filter band's handle being dragged. `grab` is where the pointer took the handle (px below its top), recorded
/// when the drag starts so the band does not jump.
#[derive(Clone, Default)]
pub struct BandDrag {
    pub grab: Rc<Cell<f32>>,
}

impl BandDrag {
    /// The handle sits 4 px above the band's top edge (`filter.rs`).
    const HANDLE_ABOVE: f32 = 4.;

    /// Where the band's top edge goes for the pointer at `y`.
    pub fn band_top(&self, y: Pixels) -> Pixels {
        y - px(self.grab.get()) + px(Self::HANDLE_ABOVE)
    }
}

/// The sidebar's resize edge being dragged; `grab` is the pointer's offset into the 6 px edge.
#[derive(Clone, Default)]
pub struct SidebarDrag {
    pub grab: Rc<Cell<f32>>,
}

impl SidebarDrag {
    /// The edge starts 3 px left of the sidebar's right edge.
    const EDGE_INSET: f32 = 3.;

    /// The sidebar width for the pointer at `x` (the sidebar starts at the window's left edge).
    pub fn width(&self, x: Pixels) -> Pixels {
        x - px(self.grab.get()) + px(Self::EDGE_INSET)
    }
}

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

/// Keeps the dragged handle's cursor for the whole drag: `set_window_cursor_style` overrides every hitbox cursor, and
/// this only applies while a drag with a cursor is active.
fn drag_cursor() -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |_, _, window, cx| {
            if let Some(style) = cx.active_drag_cursor_style() {
                window.set_window_cursor_style(style);
            }
        },
    )
    .absolute()
    .size_0()
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
                        .on_drag(SidebarDrag::default(), |drag: &SidebarDrag, offset: Point<Pixels>, _, cx| {
                            drag.grab.set(f32::from(offset.x));
                            cx.new(|_| EmptyView)
                        }),
                )
            })
            .into_any_element()
    })
}

impl RenderOnce for MainWindowLayout {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let s = t.text_scale();
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
            .text_size(s.px(13.))
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
                        // Clipped: a body taller than a short pane (the empty view's recent filters) must not paint or
                        // take clicks over the header, the band or the status bar.
                        .child(div().flex_1().min_h_0().overflow_hidden().child(body))
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
                    .child(drag_cursor())
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
                .children(parts.overlay)
                .child(drag_cursor()),
        }
    }
}

/// Wraps the window content with invisible strips along the window's edges and corners that start a native resize. Only
/// client-decorated windows (Linux without server-side decorations) need them; the caller checks
/// `window.window_decorations()`. Edges the window is tiled against get no strip.
pub fn resize_edges(content: AnyElement) -> impl IntoElement {
    ResizeEdges(content)
}

/// The resize strips' thickness, and the side of the corner squares.
const RESIZE_EDGE: f32 = 5.;

#[derive(IntoElement)]
struct ResizeEdges(AnyElement);

/// The edges and corners that get a resize strip: those not touching a tiled side.
fn resize_handles(tiling: Tiling) -> Vec<ResizeEdge> {
    use ResizeEdge::*;
    let free = |edge: ResizeEdge| match edge {
        Top => !tiling.top,
        Bottom => !tiling.bottom,
        Left => !tiling.left,
        Right => !tiling.right,
        TopLeft => !tiling.top && !tiling.left,
        TopRight => !tiling.top && !tiling.right,
        BottomLeft => !tiling.bottom && !tiling.left,
        BottomRight => !tiling.bottom && !tiling.right,
    };
    // Corners last, so they sit over the ends of the edge strips.
    [Top, Bottom, Left, Right, TopLeft, TopRight, BottomLeft, BottomRight].into_iter().filter(|&e| free(e)).collect()
}

fn resize_handle(edge: ResizeEdge) -> impl IntoElement {
    let w = px(RESIZE_EDGE);
    let strip = div().absolute().occlude();
    let (strip, cursor) = match edge {
        ResizeEdge::Top => (strip.top_0().left_0().right_0().h(w), CursorStyle::ResizeUpDown),
        ResizeEdge::Bottom => (strip.bottom_0().left_0().right_0().h(w), CursorStyle::ResizeUpDown),
        ResizeEdge::Left => (strip.left_0().top_0().bottom_0().w(w), CursorStyle::ResizeLeftRight),
        ResizeEdge::Right => (strip.right_0().top_0().bottom_0().w(w), CursorStyle::ResizeLeftRight),
        ResizeEdge::TopLeft => (strip.top_0().left_0().size(w), CursorStyle::ResizeUpLeftDownRight),
        ResizeEdge::BottomRight => (strip.bottom_0().right_0().size(w), CursorStyle::ResizeUpLeftDownRight),
        ResizeEdge::TopRight => (strip.top_0().right_0().size(w), CursorStyle::ResizeUpRightDownLeft),
        ResizeEdge::BottomLeft => (strip.bottom_0().left_0().size(w), CursorStyle::ResizeUpRightDownLeft),
    };
    strip.cursor(cursor).on_mouse_down(MouseButton::Left, move |_, window, cx| {
        cx.stop_propagation();
        window.start_window_resize(edge);
    })
}

impl RenderOnce for ResizeEdges {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let handles = match window.window_decorations() {
            Decorations::Client { tiling } => resize_handles(tiling),
            Decorations::Server => Vec::new(),
        };
        div().relative().size_full().child(self.0).children(handles.into_iter().map(resize_handle))
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

    #[test]
    fn drags_keep_the_grab_offset() {
        let band = BandDrag::default();
        band.grab.set(6.);
        assert_eq!(band.band_top(px(100.)), px(98.), "handle grabbed 6 px into it, 4 px above the band");
        let side = SidebarDrag::default();
        side.grab.set(1.);
        assert_eq!(side.width(px(250.)), px(252.), "edge grabbed 1 px into it, 3 px left of the sidebar edge");
    }

    #[test]
    fn resize_handles_skip_tiled_sides() {
        use ResizeEdge::*;
        assert_eq!(resize_handles(Tiling::default()).len(), 8, "a floating window resizes from every edge and corner");
        assert!(resize_handles(Tiling::tiled()).is_empty(), "a maximised window has no resize edges");
        let left_half = Tiling { top: true, bottom: true, left: true, right: false };
        assert_eq!(resize_handles(left_half), vec![Right]);
        let right_side = resize_handles(Tiling { right: true, ..Tiling::default() });
        assert!(!right_side.iter().any(|e| matches!(e, Right | TopRight | BottomRight)), "{right_side:?}");
        assert_eq!(right_side.len(), 5);
    }
}
