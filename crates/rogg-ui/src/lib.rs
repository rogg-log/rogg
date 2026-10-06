//! rogg's design system: theme, icons and components shared by the app and the gallery.

pub mod theme;
pub mod assets;
pub mod icons;

pub use theme::{ActiveRoggTheme, RoggTheme};

/// Crate version, used by the gallery title.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub mod controls;

use gpui_kit::{App, Window};
use std::rc::Rc;

/// Callback with no payload, used by every component.
pub type Callback = Rc<dyn Fn(&mut Window, &mut App)>;
/// Callback carrying an index (tab, chip, menu item...).
pub type IndexCallback = Rc<dyn Fn(usize, &mut Window, &mut App)>;
pub mod titlebar;
pub mod sidebar;
