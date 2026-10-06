mod buttons;
mod find_field;
mod segmented;
mod select;

pub use buttons::{ActionButton, FlagButton, IconButton, TextButton, ToolButton};
pub use find_field::FindField;
pub use segmented::Segmented;
pub use select::CompactSelect;

use crate::theme::Palette;
use gpui_kit::prelude::*;

/// `button[aria-pressed=true]`: accent text, accent-bg fill, accent border. Pressed buttons keep this fill on hover.
pub(crate) fn pressed<E: Styled>(el: E, p: &Palette) -> E {
    el.text_color(p.accent).bg(p.accent_bg).border_color(p.accent_border)
}
