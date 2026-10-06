use gpui_kit::{AppContext, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use rogg_gallery::gallery::{GalleryView, Page};
use rogg_ui::theme::{Appearance, Density, RoggTheme};

fn render_page(cx: &mut TestAppContext, page: Page, appearance: Appearance, density: Density) {
    cx.update(|cx| RoggTheme::new(appearance, density).install(cx));
    let handle = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(1440.), px(900.)) })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, move |window, cx| cx.new(|cx| GalleryView::new(page, true, window, cx))).unwrap().0
    });
    cx.update_window(handle, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn every_page_renders_in_every_theme(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init); // once per app; render_page only switches themes
    for page in Page::ALL {
        for appearance in [Appearance::Dark, Appearance::Light] {
            for density in [Density::Compact, Density::Comfortable] {
                render_page(cx, page, appearance, density);
            }
        }
    }
}

#[test]
fn slugs_round_trip() {
    for page in Page::ALL {
        assert_eq!(Page::from_slug(page.slug()), Some(page));
    }
    assert_eq!(Page::from_slug("nope"), None);
}

#[gpui_kit::test]
fn replica_starts_in_reference_state(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        RoggTheme::new(Appearance::Dark, Density::Compact).install(cx);
    });
    let (handle, view) = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(1440.), px(900.)) })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| GalleryView::new(Page::Replica, false, window, cx))).unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
    })
    .unwrap();
    let replica = view.read_with(cx, |v, _| v.replica.clone());
    let (visible, bottom_rows, summary) = replica.read_with(cx, |r, cx| (r.filtered.visible.clone(), r.bottom.read(cx).rows().row_count(), r.summary()));
    assert_eq!(visible, vec![7, 12, 15, 20, 23, 28, 34]);
    assert_eq!(bottom_rows, 7 + 6, "7 lines + 6 gap rows");
    assert_eq!(summary, "2 include · 1 exclude · 1 off");
    replica.update(cx, |r, cx| r.cycle(2, cx)); // healthcheck NOT -> IN
    let matched = replica.read_with(cx, |r, _| r.filtered.matched.len());
    assert!(matched > 6);
}
