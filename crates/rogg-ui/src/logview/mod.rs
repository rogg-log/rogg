mod element;
mod model;
pub mod scroll;
mod state;
pub mod text;

pub use element::LogViewElement;
pub use model::{LogLine, LogRow, LogRowSource, RowTint, Span, SpanStyle, VecRows};
pub use state::{LogViewEvent, LogViewState, MAX_COPY_ROWS, RowGeometry, ScrollTo};

use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, SharedString, TestSupportExt, Window, div};

/// A log panel: a test-support container filling its parent around the custom row element.
#[derive(IntoElement)]
pub struct LogView {
    id: SharedString,
    state: Entity<LogViewState>,
}

impl LogView {
    pub fn new(id: impl Into<SharedString>, state: Entity<LogViewState>) -> Self {
        Self { id: id.into(), state }
    }
}

impl RenderOnce for LogView {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().id(self.id).test_support().size_full().min_h_0().overflow_hidden().child(LogViewElement::new(self.state))
    }
}
