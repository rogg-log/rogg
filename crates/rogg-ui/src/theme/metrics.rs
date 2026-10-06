use gpui_kit::{Pixels, px};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Density {
    #[default]
    Compact,
    Comfortable,
}

/// Sizes that change with density (reference `.comfortable{--row:29px;--log-font:13px}` and pane-head overrides).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Metrics {
    pub row_height: Pixels,
    pub log_font_size: Pixels,
    pub pane_header_height: Pixels,
    /// Top padding of the pin glyph inside a row (`.row .pin{padding-top}`).
    pub pin_top: Pixels,
}

impl Density {
    pub fn metrics(self) -> Metrics {
        match self {
            Density::Compact => Metrics { row_height: px(23.), log_font_size: px(12.), pane_header_height: px(35.), pin_top: px(5.) },
            Density::Comfortable => Metrics { row_height: px(29.), log_font_size: px(13.), pane_header_height: px(39.), pin_top: px(8.) },
        }
    }
}

/// Fixed sizes from the reference CSS, in logical px.
pub mod size {
    /// CSS `line-height: normal` for the system UI font (Chromium rounds SF Pro to ≈1.2 em); gpui defaults to 1.618.
    pub const LINE_HEIGHT: f32 = 1.2;
    pub const TITLEBAR_H: f32 = 42.;
    pub const SIDEBAR_W: f32 = 211.;
    pub const SIDEBAR_MIN_W: f32 = 155.;
    pub const SIDEBAR_MAX_W: f32 = 360.;
    pub const SIDE_TOP_H: f32 = 43.;
    pub const TREE_ROW_H: f32 = 30.;
    pub const INFO_BAR_H: f32 = 33.;
    pub const STATUS_BAR_H: f32 = 29.;
    pub const TAB_H: f32 = 29.;
    pub const TAB_MAX_W: f32 = 152.;
    pub const TAB_LABEL_MAX_W: f32 = 127.;
    pub const ICON_BTN: f32 = 27.;
    pub const TITLEBAR_ICON_BTN: f32 = 25.;
    pub const FLAG_H: f32 = 27.;
    pub const FLAG_MIN_W: f32 = 28.;
    pub const TOOL_H: f32 = 25.;
    pub const FIND_H: f32 = 24.;
    pub const FIND_INPUT_W: f32 = 112.;
    pub const CHIP_H: f32 = 28.;
    pub const FILTER_LINE_MIN_H: f32 = 30.;
    pub const SELECT_H: f32 = 19.;
    pub const GUTTER_PIN_W: f32 = 24.;
    pub const GUTTER_NUMBER_W: f32 = 65.;
    pub const NUMBER_PAD_R: f32 = 12.;
    pub const LINE_PAD_R: f32 = 20.;
    pub const LOGS_PAD_Y: f32 = 4.;
    pub const PIN_GLYPH: f32 = 11.;
    pub const GAP_ROW_H: f32 = 17.;
    pub const GAP_ROW_INSET: f32 = 89.;
    pub const SCROLL_THUMB_W: f32 = 6.;
    pub const SCROLL_THUMB_MIN_H: f32 = 24.;
    pub const DIALOG_W: f32 = 500.;
    pub const DIALOG_PAD: f32 = 22.;
}
