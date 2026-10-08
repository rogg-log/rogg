//! The bundled font pair for Linux and Windows chrome (SIL OFL 1.1; see the OFL texts next to the files).

use gpui_kit::{App, AssetSource};

const FILES: [&str; 5] = ["Inter-Regular", "Inter-Medium", "Inter-SemiBold", "JetBrainsMono-Regular", "JetBrainsMono-Bold"];

/// Registers the bundled fonts with gpui's text system. Harmless on macOS (the Mac style keeps SF), needed there too
/// so the gallery can render the other styles.
pub fn register(cx: &mut App) {
    let fonts = FILES
        .iter()
        .filter_map(|name| crate::assets::Assets.load(&format!("fonts/{name}.ttf")).ok().flatten())
        .collect::<Vec<_>>();
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        eprintln!("rogg: could not register the bundled fonts: {e:#}");
    }
}
