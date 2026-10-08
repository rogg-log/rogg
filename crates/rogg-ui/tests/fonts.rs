mod common;
use gpui_kit::TestAppContext;

#[test]
fn every_bundled_font_is_embedded() {
    for name in ["Inter-Regular", "Inter-Medium", "Inter-SemiBold", "JetBrainsMono-Regular", "JetBrainsMono-Bold"] {
        let path = format!("fonts/{name}.ttf");
        let data = gpui_kit::AssetSource::load(&rogg_ui::assets::Assets, &path).unwrap().unwrap_or_else(|| panic!("{path} missing"));
        assert!(data.len() > 10_000, "{path} looks truncated");
    }
}

#[gpui_kit::test]
fn registering_the_fonts_succeeds(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        rogg_ui::fonts::register(cx);
    });
}
