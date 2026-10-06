use gpui_kit::TestAppContext;
use gpui_kit::component::Theme as KitTheme;
use rogg_ui::theme::{ActiveRoggTheme, Appearance, Density, Palette, RoggTheme};

#[gpui_kit::test]
fn install_sets_global_and_kit_mode(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        RoggTheme::new(Appearance::Light, Density::Compact).install(cx);
        assert_eq!(cx.rogg().palette, Palette::light());
        assert!(!KitTheme::global(cx).mode.is_dark());
        RoggTheme::new(Appearance::Dark, Density::Comfortable).install(cx);
        assert_eq!(cx.rogg().palette, Palette::dark());
        assert!(KitTheme::global(cx).mode.is_dark());
        assert_eq!(KitTheme::global(cx).font_family.as_ref(), ".SystemUIFont");
    });
}
