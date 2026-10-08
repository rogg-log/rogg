use super::{Appearance, RoggTheme};
use gpui_kit::component::{Theme as KitTheme, ThemeMode as KitMode};
use gpui_kit::{App, px};

/// Mirrors rogg colors and fonts into gpui-kit so its Input, menus and tooltips match the design.
/// `Theme::change` reloads kit defaults, so the overrides are applied afterwards with `Theme::update`.
pub(super) fn apply(theme: &RoggTheme, cx: &mut App) {
    let mode = match theme.appearance {
        Appearance::Dark => KitMode::Dark,
        Appearance::Light => KitMode::Light,
    };
    KitTheme::change(mode, None, cx);
    let p = theme.palette;
    let ui = theme.fonts.ui.clone();
    let mono = theme.fonts.mono.clone();
    let scale = theme.text_scale();
    KitTheme::update(cx, move |t| {
        t.font_family = ui;
        t.font_size = scale.px(13.);
        t.mono_font_family = mono;
        t.mono_font_size = px(12.);
        t.radius = px(5.);
        t.radius_lg = px(10.);
        t.shadow = false;
        t.colors.background = p.bg.hsla();
        t.colors.foreground = p.text.hsla();
        t.colors.border = p.border.hsla();
        t.colors.input = p.border.hsla();
        t.colors.muted = p.raised.hsla();
        t.colors.muted_foreground = p.faint.hsla();
        t.colors.popover = p.side.hsla();
        t.colors.accent = p.hover.hsla();
        t.colors.selection = p.selection.hsla();
        t.colors.ring = p.accent.hsla();
        t.colors.primary = p.accent.hsla();
        // The colour picker's slider tracks and its Palette/HSLA tab bar.
        t.colors.slider_bar = p.accent.hsla();
        t.colors.slider_thumb = p.text.hsla();
        t.colors.tab = p.side.hsla();
        t.colors.tab_active = p.hover.hsla();
        t.colors.tab_active_foreground = p.text.hsla();
        t.colors.tab_bar = p.side.hsla();
        t.colors.tab_bar_segmented = p.raised.hsla();
        t.colors.tab_foreground = p.muted.hsla();
    });
}
