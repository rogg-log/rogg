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
    /// The density's log font size in the reference (compact 12 px, comfortable 13 px).
    pub fn base_font_size(self) -> f32 {
        match self {
            Density::Compact => 12.,
            Density::Comfortable => 13.,
        }
    }

    pub fn metrics(self) -> Metrics {
        self.metrics_for(12.)
    }

    /// Metrics for the log font size setting `compact_size` (spec §6.2): comfortable adds 1 px, as the reference does;
    /// the row height keeps the density's ratio to its font (23/12, 29/13) in whole pixels, and the pin glyph stays
    /// centred. Pane headers keep their height.
    pub fn metrics_for(self, compact_size: f32) -> Metrics {
        let (base_row, header, base_pin) = match self {
            Density::Compact => (23., 35., 5.),
            Density::Comfortable => (29., 39., 8.),
        };
        let font = compact_size + (self.base_font_size() - 12.);
        let row = (font * base_row / self.base_font_size()).round();
        Metrics { row_height: px(row), log_font_size: px(font), pane_header_height: px(header), pin_top: px(base_pin + ((row - base_row) / 2.).round()) }
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

#[cfg(test)]
mod scale_tests {
    use super::*;

    #[test]
    fn metrics_scale_with_the_log_font() {
        assert_eq!(Density::Compact.metrics_for(12.), Density::Compact.metrics());
        assert_eq!(Density::Comfortable.metrics_for(12.), Density::Comfortable.metrics());
        let m = Density::Compact.metrics_for(14.);
        assert_eq!((m.log_font_size, m.row_height, m.pin_top), (px(14.), px(27.), px(7.)));
        let c = Density::Comfortable.metrics_for(14.);
        assert_eq!((c.log_font_size, c.row_height), (px(15.), px(33.)));
        assert_eq!(c.pane_header_height, px(39.), "headers keep their height");
        let small = Density::Compact.metrics_for(9.);
        assert_eq!((small.row_height, small.pin_top), (px(17.), px(2.)));
    }
}
