//! Session restore (spec §6.4): what a tab and the window store in `session.json` and `.rogg` files, and how a stored
//! tab is opened again.

use crate::derived::DerivedSource;
use crate::document::{DocumentInit, Source};
use crate::tab::{Tab, TabInit};
use gpui_kit::{App, Bounds, Pixels, WindowBounds, point, px, size};
use rogg_core::session::{SessionSource, TabState, WindowState};
use rogg_core::source::SourceRef;
use std::sync::Arc;
use std::time::Duration;

/// `session.json` is written this often while the app runs (spec §6.4), and on quit.
pub const AUTOSAVE: Duration = Duration::from_secs(30);

/// The window's minimum size (`window::window_options`).
pub(crate) const MIN_W: f32 = 760.;
pub(crate) const MIN_H: f32 = 520.;

/// How a tab's source is stored in a session; None for streams and in-memory texts (spec §6.4: not restored).
pub fn session_source(source: &Source) -> Option<SessionSource> {
    match source {
        Source::File(path) => Some(SessionSource::File { path: path.clone() }),
        Source::Member(source) => Some(SessionSource::Member { source: source.clone() }),
        Source::Derived(d) => d.spec.clone().map(|spec| SessionSource::Derived(Box::new(spec))),
        Source::Stream { .. } | Source::Memory { .. } => None,
    }
}

/// Whether every path a stored source names is valid UTF-8. JSON holds only UTF-8, so one odd file name would make the
/// whole session fail to save; such a tab is left out instead.
fn storable(source: &SessionSource) -> bool {
    fn source_ref(r: &SourceRef) -> bool {
        r.root_path().to_str().is_some()
    }
    match source {
        SessionSource::File { path } => path.to_str().is_some(),
        SessionSource::Member { source } => source_ref(source),
        SessionSource::Derived(spec) => storable(&spec.parent),
    }
}

pub fn window_state(bounds: WindowBounds, sidebar_visible: bool, sidebar_width: Pixels) -> WindowState {
    let (b, maximized) = match bounds {
        WindowBounds::Windowed(b) => (b, false),
        WindowBounds::Maximized(b) | WindowBounds::Fullscreen(b) => (b, true),
    };
    WindowState {
        x: f32::from(b.origin.x),
        y: f32::from(b.origin.y),
        width: f32::from(b.size.width),
        height: f32::from(b.size.height),
        maximized,
        sidebar_visible,
        sidebar_width: Some(f32::from(sidebar_width)),
    }
}

/// The frame to open the window with, never smaller than its minimum size.
pub fn bounds_for(state: &WindowState) -> WindowBounds {
    let b = Bounds { origin: point(px(state.x), px(state.y)), size: size(px(state.width.max(MIN_W)), px(state.height.max(MIN_H))) };
    if state.maximized { WindowBounds::Maximized(b) } else { WindowBounds::Windowed(b) }
}

/// How much of a restored window must show on one display to be left where it was (enough to grab and drag it).
const MIN_VISIBLE: f32 = 100.;

/// A stored frame placed on the current displays (the first one is the primary). gpui restores a frame as is, so one
/// saved on a display that is now smaller or unplugged could open out of sight: when less than 100 × 100 px of it
/// shows on any display, it opens centred on the first one, its size clamped to that display. With no display known
/// the frame is left as stored.
pub fn fit_to_displays(frame: Bounds<Pixels>, displays: &[Bounds<Pixels>]) -> Bounds<Pixels> {
    let visible = displays.iter().any(|d| {
        let overlap = frame.intersect(d);
        f32::from(overlap.size.width) >= MIN_VISIBLE && f32::from(overlap.size.height) >= MIN_VISIBLE
    });
    match displays.first() {
        Some(first) if !visible => Bounds::centered_at(first.center(), frame.size.min(&first.size)),
        _ => frame,
    }
}

/// `bounds_for` a stored frame whose values are all finite; None otherwise (the window opens at its default frame).
pub fn restorable_bounds(state: &WindowState) -> Option<WindowBounds> {
    [state.x, state.y, state.width, state.height].iter().all(|v| v.is_finite()).then(|| bounds_for(state))
}

/// A tab's restorable state; None for streams, in-memory texts and sources whose paths are not valid UTF-8.
pub fn tab_state(tab: &Tab, cx: &App) -> Option<TabState> {
    let d = tab.doc.read(cx);
    let source = session_source(&d.source).filter(storable)?;
    Some(TabState {
        source,
        profile: Some(tab.profile.to_string()),
        filter: d.filter.spec.clone(),
        pins: d.filter.pins.iter().copied().collect(),
        selection: tab.session_selection(),
        top: tab.session_top(cx),
        split: tab.split,
        wrap: [tab.full.read(cx).wrap(), tab.filtered.read(cx).wrap()],
        follow: tab.follow,
    })
}

/// The tab to open for a stored tab, and its init.
pub fn restore_tab(state: TabState) -> (Source, TabInit) {
    let source = match state.source {
        SessionSource::File { path } => Source::File(path),
        SessionSource::Member { source } => Source::Member(source),
        SessionSource::Derived(spec) => Source::Derived(Arc::new(DerivedSource::restored(*spec))),
    };
    let init = TabInit {
        doc: DocumentInit { spec: state.filter, pins: state.pins.into_iter().collect(), ..DocumentInit::default() },
        selection: state.selection,
        wrap: state.wrap,
        follow: state.follow,
        split: Some(state.split),
        top: state.top,
        profile: state.profile.map(Into::into),
        ..TabInit::default()
    };
    (source, init)
}
