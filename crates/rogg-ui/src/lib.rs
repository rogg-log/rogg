//! rogg's design system: theme, icons and components shared by the app and the gallery.

pub mod theme;
pub mod assets;
pub mod icons;

pub use theme::{ActiveRoggTheme, RoggTheme};

/// Crate version, used by the gallery title.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
