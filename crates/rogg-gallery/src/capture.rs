use crate::gallery::{GalleryView, Page};
use crate::replica::ReplicaDialog;
use gpui_kit::{AppContext, Bounds, ElementId, Entity, HeadlessAppContext, Point, Window, WindowBounds, WindowOptions, px, size};
use rogg_ui::theme::{Appearance, Density, RoggTheme};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct CaptureOptions {
    pub page: Page,
    pub dialog: Option<ReplicaDialog>,
    pub appearance: Appearance,
    pub density: Density,
    pub width: f32,
    pub height: f32,
    pub out: PathBuf,
    pub geometry: Option<PathBuf>,
}

impl CaptureOptions {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
        let page_slug = value("--capture").ok_or("--capture <page> is required")?;
        let page = Page::from_slug(&page_slug).ok_or_else(|| format!("unknown page `{page_slug}`"))?;
        let dialog = match value("--dialog") {
            Some(d) => Some(ReplicaDialog::from_slug(&d).ok_or_else(|| format!("unknown dialog `{d}`"))?),
            None => None,
        };
        if page == Page::App && matches!(dialog, Some(ReplicaDialog::Profile | ReplicaDialog::Appearance)) {
            return Err(format!("dialog `{}` is not available on the app page", dialog.unwrap().slug()));
        }
        let appearance = match value("--theme").as_deref() {
            None | Some("dark") => Appearance::Dark,
            Some("light") => Appearance::Light,
            Some(other) => return Err(format!("unknown theme `{other}`")),
        };
        let density = match value("--density").as_deref() {
            None | Some("compact") => Density::Compact,
            Some("comfortable") => Density::Comfortable,
            Some(other) => return Err(format!("unknown density `{other}`")),
        };
        let (width, height) = match value("--size") {
            None => (1440., 900.),
            Some(s) => {
                let (w, h) = s.split_once('x').ok_or("--size must look like 1440x900")?;
                (w.parse().map_err(|_| "bad width")?, h.parse().map_err(|_| "bad height")?)
            }
        };
        let out = PathBuf::from(value("--out").ok_or("--out <png> is required")?);
        Ok(Self { page, dialog, appearance, density, width, height, out, geometry: value("--geometry").map(PathBuf::from) })
    }
}

#[derive(Serialize)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl From<Bounds<gpui_kit::Pixels>> for Rect {
    fn from(b: Bounds<gpui_kit::Pixels>) -> Self {
        Rect { x: f32::from(b.origin.x), y: f32::from(b.origin.y), w: f32::from(b.size.width), h: f32::from(b.size.height) }
    }
}

#[derive(Serialize)]
struct Geometry {
    scale: f32,
    window: Rect,
    elements: BTreeMap<String, Rect>,
}

fn collect_geometry(window: &Window, view: &Entity<GalleryView>, cx: &gpui_kit::App, opts: &CaptureOptions) -> Geometry {
    let mut elements = BTreeMap::new();
    for snap in gpui_kit::base::test_support::snapshots(window) {
        if let Some(ElementId::Name(name)) = snap.path().last() {
            elements.insert(name.to_string(), Rect::from(snap.bounds()));
        }
    }
    for (prefix, state) in view.read(cx).log_states(cx) {
        let (mut line_ix, mut gap_ix) = (0, 0);
        for g in state.read(cx).last_rows() {
            let key = if g.is_gap {
                gap_ix += 1;
                format!("gap-{prefix}-{}", gap_ix - 1)
            } else {
                line_ix += 1;
                format!("row-{prefix}-{}", line_ix - 1)
            };
            elements.insert(key, Rect::from(g.bounds));
        }
    }
    Geometry { scale: 2., window: Rect { x: 0., y: 0., w: opts.width, h: opts.height }, elements }
}

/// Renders one page headlessly (Metal offscreen, scale 2) and writes a PNG plus optional geometry JSON.
pub fn run(opts: CaptureOptions) -> anyhow::Result<()> {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(rogg_ui::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    let (appearance, density) = (opts.appearance, opts.density);
    let app_state = if opts.page == Page::App { Some(seed_app_state()?) } else { None };
    cx.update(|cx| {
        gpui_kit::init(cx);
        RoggTheme::new(appearance, density).install(cx);
        if let Some(dir) = &app_state {
            rogg_app::install_globals(cx, rogg_app::paths::AppPaths::under(dir.path()));
        }
    });
    let (page, dialog) = (opts.page, opts.dialog);
    let (handle, view) = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(opts.width), px(opts.height)) })),
            focus: false,
            show: false,
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, move |window, cx| cx.new(|cx| GalleryView::for_capture(page, dialog, window, cx)))
    })?;
    cx.run_until_parked();
    let geometry = cx.update_window(handle, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.render_frame(cx);
        collect_geometry(window, &view, cx, &opts)
    })?;
    let image = cx.capture_screenshot(handle)?;
    if let Some(parent) = opts.out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.save(&opts.out)?;
    if let Some(path) = &opts.geometry {
        std::fs::write(path, serde_json::to_string_pretty(&geometry)?)?;
    }
    drop(app_state);
    Ok(())
}

/// A temp dir for the app page's state, with the reference's history entries, so the history dialog shows them.
fn seed_app_state() -> anyhow::Result<tempfile::TempDir> {
    let dir = tempfile::tempdir()?;
    let mut history = rogg_core::history::History::default();
    for spec in rogg_app::demo::reference_history().iter().rev() {
        history.record(rogg_core::history::HistoryEntry::from_spec(spec));
    }
    history.save(&dir.path().join("history.json"))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn parses_defaults_and_flags() {
        let o = CaptureOptions::parse(&args("--capture replica --out a.png")).unwrap();
        assert_eq!((o.page, o.appearance, o.density, o.width, o.height), (Page::Replica, Appearance::Dark, Density::Compact, 1440., 900.));
        let o = CaptureOptions::parse(&args("--capture tabs --theme light --density comfortable --size 800x600 --dialog history --out b.png --geometry b.json")).unwrap();
        assert_eq!((o.appearance, o.density, o.width, o.dialog), (Appearance::Light, Density::Comfortable, 800., Some(ReplicaDialog::History)));
        assert!(CaptureOptions::parse(&args("--capture nope --out x.png")).unwrap_err().contains("unknown page"));
        assert!(CaptureOptions::parse(&args("--capture tabs")).unwrap_err().contains("--out"));
    }
}
