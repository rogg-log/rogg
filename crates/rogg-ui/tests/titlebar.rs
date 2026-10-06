mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, px};
use rogg_ui::titlebar::{TabItem, TitleBar};
use std::cell::RefCell;
use std::rc::Rc;

fn tabs(n: usize) -> Vec<TabItem> {
    (0..n)
        .map(|i| TabItem { label: format!("service-{i}-with-a-long-name.log").into(), tooltip: "bundle.zip".into(), active: i == 0 })
        .collect()
}

#[gpui_kit::test]
fn titlebar_geometry(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(3)).traffic_lights(true).into_any_element());
    let bar = common::bounds(cx, w, "titlebar");
    assert_eq!(bar.size.height, px(42.));
    assert_eq!(common::bounds(cx, w, "titlebar-leading").size.width, px(211.));
    let tab = common::bounds(cx, w, "tab-shell-0");
    assert_eq!(tab.size.height, px(29.));
    assert!(tab.size.width <= px(152.));
    let actions = common::bounds(cx, w, "title-actions");
    assert!(actions.right() <= px(1440. - 10.) + px(0.5), "10px right padding");
}

#[gpui_kit::test]
fn many_tabs_scroll_instead_of_pushing_actions_out(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 900., 200., |_, _| TitleBar::new(tabs(12)).into_any_element());
    let actions = common::bounds(cx, w, "title-actions");
    assert!(actions.right() <= px(900.), "title actions must stay visible, got {actions:?}");
    assert!(common::bounds(cx, w, "tab-shell-11").size.width <= px(152.));
}

#[gpui_kit::test]
fn tab_select_and_close_report_index(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let e = events.clone();
    let w = common::open(cx, 1440., 200., move |_, _| {
        let (s, c) = (e.clone(), e.clone());
        TitleBar::new(tabs(3))
            .on_select_tab(move |i, _, _| s.borrow_mut().push(format!("select {i}")))
            .on_close_tab(move |i, _, _| c.borrow_mut().push(format!("close {i}")))
            .into_any_element()
    });
    common::click(cx, w, "tab-1");
    common::click(cx, w, "tab-close-2");
    assert_eq!(*events.borrow(), vec!["select 1".to_string(), "close 2".to_string()]);
}

#[gpui_kit::test]
fn overflowing_tab_strip_stays_left_of_actions(cx: &mut TestAppContext) {
    common::setup(cx);
    // 60 tabs overflow even with zero-size text; the strip must clip/scroll inside its own box.
    let w = common::open(cx, 900., 200., |_, _| TitleBar::new(tabs(60)).into_any_element());
    let strip = common::bounds(cx, w, "tabs");
    let actions = common::bounds(cx, w, "title-actions");
    assert!(strip.right() <= actions.left() + px(0.5), "tab strip {strip:?} overlaps actions {actions:?}");
    assert!(actions.right() <= px(900.), "actions out of window: {actions:?}");
}
