mod chrome;
mod color;
mod kit;
mod metrics;
mod palette;

pub use chrome::{CHROME_ENV, ChromeStyle};
pub use color::Color;
pub use metrics::{Density, Metrics, size};
pub use palette::{ChipColor, HitColor, Palette};

use gpui_kit::{App, Global, Pixels, SharedString, px};

/// System UI font (SF Pro).
pub const UI_FONT: &str = ".SystemUIFont";
/// System monospaced font (SF Mono). The family name "SF Mono" silently falls back to Helvetica in gpui.
pub const MONO_FONT: &str = ".AppleSystemUIFontMonospaced";
/// The bundled UI font of the Linux and Windows chrome (registered by `fonts::register`).
pub const INTER: &str = "Inter";
/// The bundled monospaced font of the Linux and Windows chrome.
pub const JETBRAINS_MONO: &str = "JetBrains Mono";

/// UI text sizes relative to the reference's 13 px system font (spec §6.2 "UI font size"); `px(11.)` at 13 px stays 11.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextScale(pub f32);

impl TextScale {
    pub fn px(self, n: f32) -> Pixels {
        px(n * self.0)
    }
}

/// The reference UI font size.
pub const BASE_UI_FONT_SIZE: f32 = 13.;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Appearance {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Fonts {
    pub ui: SharedString,
    pub mono: SharedString,
    /// Log text in both panes; the line-number gutter keeps `mono`.
    pub log: SharedString,
    pub ui_size: Pixels,
}

#[derive(Clone, PartialEq, Debug)]
pub struct RoggTheme {
    pub appearance: Appearance,
    pub density: Density,
    pub palette: Palette,
    pub metrics: Metrics,
    pub fonts: Fonts,
    pub chrome: ChromeStyle,
}

impl Global for RoggTheme {}

impl RoggTheme {
    pub fn new(appearance: Appearance, density: Density) -> Self {
        let palette = match appearance {
            Appearance::Dark => Palette::dark(),
            Appearance::Light => Palette::light(),
        };
        Self {
            appearance,
            density,
            palette,
            metrics: density.metrics(),
            fonts: Fonts { ui: UI_FONT.into(), mono: MONO_FONT.into(), log: MONO_FONT.into(), ui_size: px(13.) },
            chrome: ChromeStyle::Mac,
        }
        .with_chrome(ChromeStyle::current())
    }

    /// The chrome style and its font pair (SF on Mac; the bundled Inter and JetBrains Mono elsewhere). A custom log
    /// font (`with_log_font`) set before this is replaced, so call `with_chrome` first.
    pub fn with_chrome(mut self, chrome: ChromeStyle) -> Self {
        self.chrome = chrome;
        let (ui, mono) = if chrome.is_mac() { (UI_FONT, MONO_FONT) } else { (INTER, JETBRAINS_MONO) };
        self.fonts.ui = ui.into();
        self.fonts.mono = mono.into();
        self.fonts.log = mono.into();
        self
    }

    /// The log font family and compact size from the settings (spec §6.2).
    pub fn with_log_font(mut self, family: SharedString, compact_size: f32) -> Self {
        self.fonts.log = family;
        self.metrics = self.density.metrics_for(compact_size);
        self
    }

    pub fn text_scale(&self) -> TextScale {
        TextScale(f32::from(self.fonts.ui_size) / BASE_UI_FONT_SIZE)
    }

    pub fn with_ui_font_size(mut self, size: f32) -> Self {
        self.fonts.ui_size = px(size);
        self
    }

    /// Makes this the active theme and mirrors it into gpui-kit. Call after `gpui_kit::init`.
    pub fn install(self, cx: &mut App) {
        kit::apply(&self, cx);
        cx.set_global(self);
        cx.refresh_windows();
    }
}

pub trait ActiveRoggTheme {
    fn rogg(&self) -> &RoggTheme;
}

impl ActiveRoggTheme for App {
    fn rogg(&self) -> &RoggTheme {
        self.global::<RoggTheme>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_picks_palette_and_metrics() {
        let t = RoggTheme::new(Appearance::Light, Density::Comfortable);
        assert_eq!(t.palette, Palette::light());
        assert_eq!(t.metrics, Density::Comfortable.metrics());
        assert_eq!(t.fonts.mono.as_ref(), MONO_FONT);
    }

    #[test]
    fn log_font_overrides_family_and_metrics_only() {
        let t = RoggTheme::new(Appearance::Dark, Density::Compact).with_log_font("Menlo".into(), 14.);
        assert_eq!(t.fonts.log.as_ref(), "Menlo");
        assert_eq!(t.fonts.mono.as_ref(), MONO_FONT, "chips, dialogs and the gutter keep SF Mono");
        assert_eq!(t.metrics, Density::Compact.metrics_for(14.));
        assert_eq!(RoggTheme::new(Appearance::Dark, Density::Compact).fonts.log.as_ref(), MONO_FONT);
    }

    #[test]
    fn the_font_pair_follows_the_chrome_style() {
        let mac = RoggTheme::new(Appearance::Dark, Density::Compact).with_chrome(ChromeStyle::Mac);
        assert_eq!((mac.fonts.ui.as_ref(), mac.fonts.mono.as_ref(), mac.fonts.log.as_ref()), (UI_FONT, MONO_FONT, MONO_FONT));
        for style in [ChromeStyle::Linux, ChromeStyle::Windows] {
            let t = RoggTheme::new(Appearance::Dark, Density::Compact).with_chrome(style);
            assert_eq!((t.fonts.ui.as_ref(), t.fonts.mono.as_ref(), t.fonts.log.as_ref()), (INTER, JETBRAINS_MONO, JETBRAINS_MONO), "{style:?}");
            assert_eq!(t.chrome, style);
        }
    }

    #[test]
    fn new_uses_the_build_os_style() {
        assert_eq!(RoggTheme::new(Appearance::Dark, Density::Compact).chrome, ChromeStyle::current());
    }

    #[test]
    fn text_scale_follows_the_ui_font_size() {
        let t = RoggTheme::new(Appearance::Dark, Density::Compact);
        assert_eq!(t.text_scale().px(11.), px(11.), "13 px is the reference size: nothing scales");
        let big = t.clone().with_ui_font_size(16.);
        assert_eq!(big.fonts.ui_size, px(16.));
        assert!((f32::from(big.text_scale().px(13.)) - 16.).abs() < 1e-4);
        assert!((f32::from(big.text_scale().px(10.)) - 10. * 16. / 13.).abs() < 1e-4);
    }
}
