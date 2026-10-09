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
        // `RoggTheme::new` picks the font for the running OS: the system font on macOS, the bundled Inter elsewhere.
        let ui = if cfg!(target_os = "macos") { rogg_ui::theme::UI_FONT } else { rogg_ui::theme::INTER };
        assert_eq!(KitTheme::global(cx).font_family.as_ref(), ui);
    });
}

#[gpui_kit::test]
fn the_kit_font_size_follows_the_ui_font_size(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        RoggTheme::new(Appearance::Dark, Density::Compact).with_ui_font_size(15.).install(cx);
        assert_eq!(KitTheme::global(cx).font_size, gpui_kit::px(15.));
    });
}
