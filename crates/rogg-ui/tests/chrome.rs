mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, div, px};
use rogg_ui::chrome::{InfoBar, PaneHeader, Progress, StatPart, StatusBar};
use rogg_ui::theme::{Appearance, Density};
use rogg_ui::toast::ToastStack;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn chrome() -> gpui_kit::AnyElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(InfoBar::new(vec!["incident.zip".into(), "api.log".into()], "Services".into()))
        .child(PaneHeader::new("pane-top-head", "Full log", "40 sample lines"))
        .child(
            StatusBar::new("UTF-8 · LF".into())
                .stats(vec![StatPart::Strong("6".into()), StatPart::Text(" matches / 40 lines".into())])
                .progress(Some(Progress { label: "Indexing 41%".into(), fraction: 0.41 })),
        )
        .into_any_element()
}

#[gpui_kit::test]
fn compact_heights(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1200., 300., |_, _| chrome());
    assert_eq!(common::bounds(cx, w, "meta").size.height, px(33.));
    assert_eq!(common::bounds(cx, w, "pane-top-head").size.height, px(35.));
    assert_eq!(common::bounds(cx, w, "statusbar").size.height, px(29.));
    assert!(common::exists(cx, w, "progress"));
}

#[gpui_kit::test]
fn comfortable_pane_header_is_taller(cx: &mut TestAppContext) {
    common::setup_with(cx, Appearance::Light, Density::Comfortable);
    let w = common::open(cx, 1200., 300., |_, _| chrome());
    assert_eq!(common::bounds(cx, w, "pane-top-head").size.height, px(39.));
}

#[gpui_kit::test]
fn info_bar_callbacks_report_the_clicked_part(cx: &mut TestAppContext) {
    common::setup(cx);
    let crumb = Rc::new(Cell::new(usize::MAX));
    let profile = Rc::new(Cell::new(0));
    let export = Rc::new(Cell::new(0));
    let (c, p, e) = (crumb.clone(), profile.clone(), export.clone());
    let w = common::open(cx, 1200., 100., move |_, _| {
        let (c, p, e) = (c.clone(), p.clone(), e.clone());
        InfoBar::new(vec!["incident.zip".into(), "api.log".into()], "Services".into())
            .on_crumb(move |i, _, _| c.set(i))
            .on_profile(move |_, _| p.set(p.get() + 1))
            .on_export(move |_, _| e.set(e.get() + 1))
            .into_any_element()
    });
    common::click(cx, w, "crumb-1");
    assert_eq!((crumb.get(), profile.get(), export.get()), (1, 0, 0));
    common::click(cx, w, "profile");
    assert_eq!((profile.get(), export.get()), (1, 0));
    common::click(cx, w, "export");
    assert_eq!((profile.get(), export.get()), (1, 1));
    common::click(cx, w, "crumb-0");
    assert_eq!(crumb.get(), 0);
}

#[gpui_kit::test]
fn progress_is_absent_without_a_value_and_clamped_with_one(cx: &mut TestAppContext) {
    common::setup(cx);
    let none = common::open(cx, 600., 100., |_, _| StatusBar::new("UTF-8 · LF".into()).into_any_element());
    assert!(!common::exists(cx, none, "progress"));
    for (fraction, expected) in [(0.5, 40.), (7.0, 80.), (-1.0, 0.), (f32::NAN, 0.), (f32::INFINITY, 0.), (f32::NEG_INFINITY, 0.)] {
        let w = common::open(cx, 600., 100., move |_, _| {
            StatusBar::new("n".into()).progress(Some(Progress { label: "x".into(), fraction })).into_any_element()
        });
        assert_eq!(common::bounds(cx, w, "progress-fill").size.width, px(expected), "fraction {fraction}");
    }
}

#[gpui_kit::test]
fn snapshot_note_is_optional(cx: &mut TestAppContext) {
    common::setup(cx);
    let with = common::open(cx, 1200., 100., |_, _| {
        InfoBar::new(vec!["a".into()], "P".into()).snapshot(Some("Snapshot · 14:32:08".into())).into_any_element()
    });
    let without = common::open(cx, 1200., 100., |_, _| InfoBar::new(vec!["a".into()], "P".into()).into_any_element());
    assert!(common::exists(cx, with, "snapshot"));
    assert!(!common::exists(cx, without, "snapshot"));
}

#[gpui_kit::test]
fn toasts_stack_bottom_right_and_dismiss(cx: &mut gpui_kit::TestAppContext) {
    common::setup(cx);
    let dismissed = Rc::new(RefCell::new(Vec::new()));
    let d = dismissed.clone();
    let w = common::open(cx, 1000., 700., move |_, _| {
        let d = d.clone();
        gpui_kit::div()
            .relative()
            .size_full()
            .child(ToastStack::new(vec![(7, "History reset".into()), (8, "Second".into())]).on_dismiss(move |id, _, _| d.borrow_mut().push(id)))
            .into_any_element()
    });
    let toast = common::bounds(cx, w, "toast-7");
    assert!(toast.right() <= px(1000. - 14.) + px(0.5));
    assert!(toast.bottom() <= px(700. - 29. - 12.) + px(0.5));
    common::click(cx, w, "toast-close-7");
    assert_eq!(*dismissed.borrow(), vec![7]);
}

#[gpui_kit::test]
fn the_statistics_area_is_clickable_only_with_a_handler(cx: &mut TestAppContext) {
    common::setup(cx);
    let clicks = std::rc::Rc::new(std::cell::Cell::new(0));
    let c = clicks.clone();
    let w = common::open(cx, 900., 60., move |_, _| {
        let c = c.clone();
        StatusBar::new("UTF-8 · LF".into())
            .stats(vec![StatPart::Strong("6".into()), StatPart::Text(" matches".into())])
            .on_stats(move |_, _| c.set(c.get() + 1))
            .into_any_element()
    });
    common::click(cx, w, "footer-stats");
    assert_eq!(clicks.get(), 1);
}
