mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, div, px};
use rogg_ui::chrome::{InfoBar, PaneHeader, StatusBar};
use rogg_ui::filter::FilterBand;
use rogg_ui::layout::{MainWindowLayout, MainWindowParts};
use rogg_ui::sidebar::Sidebar;
use rogg_ui::titlebar::TitleBar;

#[gpui_kit::test]
fn panes_split_remaining_height_1_1_to_1(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1440., 900., |_, _| {
        MainWindowLayout::new(MainWindowParts {
            titlebar: TitleBar::new(vec![]).into_any_element(),
            sidebar: Some(Sidebar::new().into_any_element()),
            info_bar: InfoBar::new(vec!["a.log".into()], "Default".into()).into_any_element(),
            top_header: PaneHeader::new("pane-top-head", "Full log", "").into_any_element(),
            top_body: div().size_full().into_any_element(),
            band: FilterBand::new(vec![]).into_any_element(),
            bottom_header: PaneHeader::new("pane-bottom-head", "Filtered log", "").into_any_element(),
            bottom_body: div().size_full().into_any_element(),
            status: StatusBar::new("UTF-8".into()).into_any_element(),
            overlay: None,
            split: MainWindowParts::DEFAULT_SPLIT,
        })
        .into_any_element()
    });
    assert_eq!(common::bounds(cx, w, "titlebar").origin.y, px(0.));
    assert_eq!(common::bounds(cx, w, "meta").origin.y, px(42.));
    assert_eq!(common::bounds(cx, w, "meta").origin.x, px(211.));
    assert_eq!(common::bounds(cx, w, "statusbar").bottom(), px(900.));
    let top = common::bounds(cx, w, "pane-top").size.height;
    let bottom = common::bounds(cx, w, "pane-bottom").size.height;
    assert!((f32::from(top) / f32::from(bottom) - 1.1).abs() < 0.01, "top {top:?} bottom {bottom:?}");
}
