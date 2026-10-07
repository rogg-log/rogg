mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, TestSupportExt, div, px};
use rogg_ui::chrome::{InfoBar, PaneHeader, StatusBar};
use rogg_ui::filter::FilterBand;
use rogg_ui::layout::{LayoutMeasure, MainWindowLayout, MainWindowParts, SingleParts};
use std::rc::Rc;
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

#[gpui_kit::test]
fn single_layout_has_no_panes(cx: &mut gpui_kit::TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1000., 700., |_, _| {
        MainWindowLayout::single(SingleParts {
            titlebar: div().id("titlebar").h(px(42.)).into_any_element(),
            sidebar: None,
            info_bar: None,
            body: div().id("placeholder-body").test_support().size_full().into_any_element(),
            status: None,
            overlay: None,
        })
        .into_any_element()
    });
    assert!(common::exists(cx, w, "placeholder-body"));
    assert!(!common::exists(cx, w, "pane-top"));
    assert_eq!(common::bounds(cx, w, "placeholder-body").size.height, px(700. - 42.));
}

#[gpui_kit::test]
fn measure_records_main_and_band_and_handles_exist(cx: &mut gpui_kit::TestAppContext) {
    common::setup(cx);
    let measure = Rc::new(LayoutMeasure::default());
    let m = measure.clone();
    let w = common::open(cx, 1000., 700., move |_, _| {
        let part = |id: &'static str, h: f32| div().id(id).h(px(h)).into_any_element();
        MainWindowLayout::new(MainWindowParts {
            titlebar: part("t", 42.),
            sidebar: Some(div().id("side").w(px(211.)).h_full().into_any_element()),
            info_bar: part("i", 33.),
            top_header: part("th", 35.),
            top_body: div().into_any_element(),
            band: rogg_ui::filter::FilterBand::new(vec![]).into_any_element(),
            bottom_header: part("bh", 35.),
            bottom_body: div().into_any_element(),
            status: part("s", 29.),
            overlay: None,
            split: MainWindowParts::DEFAULT_SPLIT,
        })
        .measure(m.clone())
        .sidebar_resizable(true)
        .into_any_element()
    });
    assert!(common::exists(cx, w, "band-handle"));
    assert!(common::exists(cx, w, "sidebar-resize"));
    let band = common::bounds(cx, w, "filter-band");
    let top = common::bounds(cx, w, "pane-top");
    let split = measure.split_at(band.top()).expect("measured");
    assert!((split - MainWindowParts::DEFAULT_SPLIT).abs() < 0.01, "band top maps back to the current split, got {split}");
    assert_eq!(top.top(), px(42. + 33.));
}
