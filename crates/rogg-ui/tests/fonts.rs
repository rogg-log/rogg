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

/// The test text system lists no families (`all_font_names` is empty even for installed fonts), so the families
/// cannot be asserted by name; registration succeeding with all five files is what can be checked.
#[gpui_kit::test]
fn registering_the_fonts_succeeds(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        rogg_ui::fonts::try_register(cx).expect("the five bundled fonts register");
    });
}
