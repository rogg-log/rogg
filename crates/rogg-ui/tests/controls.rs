mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, div, px, size};
use rogg_ui::controls::{FindField, FlagButton, IconButton, ToolButton};
use rogg_ui::icons::IconName;
use std::cell::Cell;
use std::rc::Rc;

#[gpui_kit::test]
fn icon_buttons_use_reference_box_sizes(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 400., 100., |_, _| {
        div()
            .flex()
            .child(IconButton::icon("edit", IconName::Edit))
            .child(IconButton::icon("settings", IconName::Settings).compact())
            .child(IconButton::glyph("all-tabs", "⌄").compact())
            .into_any_element()
    });
    // .icon-btn: 15px svg + 6px*2 padding + 1px*2 border = 29 wide; height fixed at 27.
    assert_eq!(common::bounds(cx, w, "edit").size, size(px(29.), px(27.)));
    // title bar variant: 15 + 5*2 + 2 = 27 wide, 25 high.
    assert_eq!(common::bounds(cx, w, "settings").size, size(px(27.), px(25.)));
    assert_eq!(common::bounds(cx, w, "all-tabs").size.height, px(25.));
    assert!(common::bounds(cx, w, "all-tabs").size.width >= px(25.));
}

#[gpui_kit::test]
fn flag_tool_and_find_heights(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 600., 100., |window, cx| {
        let input = FindField::state("Find in full log", window, cx);
        div()
            .flex()
            .child(FlagButton::new("flag-regex", ".*").pressed(true))
            .child(ToolButton::new("tool-wrap", IconName::Wrap, "Wrap"))
            .child(FindField::new("find", input).count(Some("3 hits".into())))
            .into_any_element()
    });
    let flag = common::bounds(cx, w, "flag-regex");
    assert_eq!(flag.size.height, px(27.));
    assert!(flag.size.width >= px(28.));
    assert_eq!(common::bounds(cx, w, "tool-wrap").size.height, px(25.));
    assert_eq!(common::bounds(cx, w, "find").size.height, px(24.));
}

#[gpui_kit::test]
fn empty_find_field_keeps_the_count_slot_gap(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 600., 100., |window, cx| {
        let input = FindField::state("Find in full log", window, cx);
        div().flex().child(FindField::new("find", input)).into_any_element()
    });
    // The reference keeps an empty `.found` span, so its 5px gap stays: 1 + 6 + 12 + 5 + 112 + 5 + 0 + 6 + 1 = 148.
    assert_eq!(common::bounds(cx, w, "find").size.width, px(148.));
}

#[gpui_kit::test]
fn click_invokes_callback(cx: &mut TestAppContext) {
    common::setup(cx);
    let clicked = Rc::new(Cell::new(0));
    let c = clicked.clone();
    let w = common::open(cx, 200., 100., move |_, _| {
        let c = c.clone();
        IconButton::icon("edit", IconName::Edit).on_click(move |_, _| c.set(c.get() + 1)).into_any_element()
    });
    common::click(cx, w, "edit");
    assert_eq!(clicked.get(), 1);
}
