use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, QuitMode, WindowBounds, WindowOptions, px, size};
use rogg_gallery::gallery::{GalleryView, Page};
use rogg_ui::theme::{Appearance, Density, RoggTheme};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--capture") {
        match rogg_gallery::capture::CaptureOptions::parse(&args) {
            Ok(opts) => {
                if let Err(e) = rogg_gallery::capture::run(opts) {
                    eprintln!("capture failed: {e:#}");
                    std::process::exit(1);
                }
                return;
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        }
    }
    let page = args
        .iter()
        .position(|a| a == "--page")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| Page::from_slug(s))
        .unwrap_or(Page::Tokens);

    let chrome = args.iter().position(|a| a == "--chrome").and_then(|i| args.get(i + 1)).map(String::as_str);
    let chrome = rogg_gallery::capture::parse_chrome(chrome).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });

    gpui_kit::application()
        .with_assets(rogg_ui::assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            rogg_ui::fonts::register(cx);
            RoggTheme::new(Appearance::Dark, Density::Compact).with_chrome(chrome).install(cx);
            let state = std::env::temp_dir().join(format!("rogg-gallery-{}", std::process::id()));
            rogg_app::install_globals(cx, rogg_app::paths::AppPaths::under(&state));
            let bounds = Bounds::centered(None, size(px(1640.), px(1000.)), cx);
            gpui_kit::open_window(
                WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
                cx,
                move |window, cx| cx.new(|cx| GalleryView::new(page, true, window, cx)),
            )
            .expect("open gallery window");
            cx.activate(true);
            if std::env::var_os("ROGG_GALLERY_SMOKE").is_some() {
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(Duration::from_millis(1500)).await;
                    println!("smoke ok");
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
        });
}
