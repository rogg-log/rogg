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

#[gpui_kit::test]
fn a_short_filtered_pane_keeps_recent_filters_inside_it(cx: &mut gpui_kit::TestAppContext) {
    use rogg_ui::empty::{RecentFilter, no_matches_with_recent};
    use std::cell::Cell;
    common::setup(cx);
    let picked = Rc::new(Cell::new(None));
    let hits = Rc::new(Cell::new((0, 0)));
    let (p, h) = (picked.clone(), hits.clone());
    let w = common::open(cx, 1000., 700., move |_, _| {
        let part = |id: &'static str, height: f32| div().id(id).test_support().h(px(height)).into_any_element();
        let (h1, h2) = (h.clone(), h.clone());
        let p = p.clone();
        let recent = (0..3).map(|i| RecentFilter { expression: format!("term-{i}").into(), summary: "1 include".into() }).collect();
        MainWindowLayout::new(MainWindowParts {
            titlebar: part("t", 42.),
            sidebar: None,
            info_bar: part("i", 33.),
            top_header: part("th", 35.),
            top_body: div().into_any_element(),
            band: FilterBand::new(vec![]).into_any_element(),
            bottom_header: div().id("bh").test_support().h(px(35.)).on_click(move |_, _, _| h1.set((h1.get().0 + 1, h1.get().1))).into_any_element(),
            bottom_body: no_matches_with_recent("empty-results", recent, Rc::new(move |i, _, _| p.set(Some(i)))).into_any_element(),
            status: div().id("s").test_support().h(px(29.)).on_click(move |_, _, _| h2.set((h2.get().0, h2.get().1 + 1))).into_any_element(),
            overlay: None,
            split: 0.7,
        })
        .into_any_element()
    });
    let body = common::bounds(cx, w, "empty-results");
    assert!(body.size.height < px(180.), "a short filtered pane: {body:?}");
    common::click(cx, w, "bh");
    common::click(cx, w, "s");
    assert_eq!(picked.get(), None, "no recent filter spills over the pane header or the status bar");
    assert_eq!(hits.get(), (1, 1), "the header and the status bar keep their clicks");
    assert!(common::bounds(cx, w, "recent-filters").top() >= body.top(), "the list starts inside the pane, under its title");
    // The rest of the list scrolls into view.
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.scroll("empty-results", gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-300.))), cx);
    })
    .unwrap();
    assert!(common::bounds(cx, w, "recent-filter-2").bottom() <= body.bottom(), "scrolled to the end of the list");
    common::click(cx, w, "recent-filter-2");
    assert_eq!(picked.get(), Some(2));
}

fn expanded_layout(cx: &mut TestAppContext, slot: rogg_ui::layout::PaneSlot) -> gpui_kit::AnyWindowHandle {
    common::setup(cx);
    common::open(cx, 1440., 900., move |_, _| {
        MainWindowLayout::new(MainWindowParts {
            titlebar: TitleBar::new(vec![]).into_any_element(),
            sidebar: Some(Sidebar::new().into_any_element()),
            info_bar: InfoBar::new(vec!["a.log".into()], "Default".into()).into_any_element(),
            top_header: PaneHeader::new("pane-top-head", "Full log", "").into_any_element(),
            top_body: div().id("top-body").test_support().size_full().into_any_element(),
            band: FilterBand::new(vec![]).into_any_element(),
            bottom_header: PaneHeader::new("pane-bottom-head", "Filtered log", "").into_any_element(),
            bottom_body: div().id("bottom-body").test_support().size_full().into_any_element(),
            status: StatusBar::new("UTF-8".into()).into_any_element(),
            overlay: None,
            split: MainWindowParts::DEFAULT_SPLIT,
        })
        .expanded(Some(slot))
        .into_any_element()
    })
}

#[gpui_kit::test]
fn an_expanded_top_pane_fills_the_window_below_the_title_bar(cx: &mut TestAppContext) {
    let w = expanded_layout(cx, rogg_ui::layout::PaneSlot::Top);
    let pane = common::bounds(cx, w, "pane-top");
    assert_eq!((pane.origin.x, pane.origin.y, pane.size.width, pane.size.height), (px(0.), px(42.), px(1440.), px(900. - 42.)));
    assert!(common::exists(cx, w, "pane-top-head") && common::exists(cx, w, "top-body"), "the pane keeps its header");
    for gone in ["sidebar", "meta", "filter-band", "pane-bottom", "bottom-body", "statusbar", "sidebar-resize"] {
        assert!(!common::exists(cx, w, gone), "{gone} should be hidden");
    }
}

#[gpui_kit::test]
fn an_expanded_bottom_pane_fills_the_window_below_the_title_bar(cx: &mut TestAppContext) {
    let w = expanded_layout(cx, rogg_ui::layout::PaneSlot::Bottom);
    let pane = common::bounds(cx, w, "pane-bottom");
    assert_eq!((pane.origin.y, pane.size.width, pane.size.height), (px(42.), px(1440.), px(900. - 42.)));
    for gone in ["sidebar", "meta", "filter-band", "pane-top", "top-body", "statusbar"] {
        assert!(!common::exists(cx, w, gone), "{gone} should be hidden");
    }
}
