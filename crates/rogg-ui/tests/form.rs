mod common;
use gpui_kit::prelude::*;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext, TestAppContext, div, px, size};
use rogg_ui::form::{EditableRuleRow, FormRow, PaletteSlot, Stepper, TextField, Toggle};
use rogg_ui::theme::Color;
use std::cell::RefCell;
use std::rc::Rc;

fn log() -> Rc<RefCell<Vec<String>>> {
    Rc::new(RefCell::new(Vec::new()))
}

#[gpui_kit::test]
fn toggle_and_stepper_report_clicks_and_respect_bounds(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = log();
    let e = events.clone();
    let w = common::open(cx, 600., 300., move |_, _| {
        let (a, b) = (e.clone(), e.clone());
        div()
            .child(FormRow::new("row-wrap", "Wrap lines").control(Toggle::new("wrap", false).on_click(move |_, _| a.borrow_mut().push("toggle".into()))))
            .child(FormRow::new("row-size", "Log font size").control(Stepper::new("size", "20 px").bounds(true, false).on_step(move |up, _, _| b.borrow_mut().push(format!("step {up}")))))
            .into_any_element()
    });
    common::click(cx, w, "wrap");
    common::click(cx, w, "size-down");
    common::click(cx, w, "size-up");
    assert_eq!(*events.borrow(), ["toggle", "step false"], "the up button at the bound does nothing");
    assert!(common::exists(cx, w, "row-size"));
}

#[gpui_kit::test]
fn rule_rows_select_toggle_move_and_show_errors(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = log();
    let e = events.clone();
    let w = common::open(cx, 600., 300., move |_, _| {
        let (a, b, c) = (e.clone(), e.clone(), e.clone());
        EditableRuleRow::new(1, Color::rgb(0xefaaa2), "ERROR", "Line text + row tint · both panels")
            .enabled(true)
            .selected(true)
            .error(Some("invalid regex".into()))
            .can_move(true, false)
            .on_select(move |_, _| a.borrow_mut().push("select".into()))
            .on_toggle(move |_, _| b.borrow_mut().push("toggle".into()))
            .on_move(move |up, _, _| c.borrow_mut().push(format!("move {up}")))
            .into_any_element()
    });
    common::click(cx, w, "profile-rule-1-toggle");
    common::click(cx, w, "profile-rule-1-up");
    common::click(cx, w, "profile-rule-1-down");
    common::click(cx, w, "profile-rule-1");
    assert_eq!(*events.borrow(), ["toggle", "move true", "select"], "controls do not also select the row; down is disabled");
    assert!(common::exists(cx, w, "profile-rule-1-error"));
}

#[gpui_kit::test]
fn a_narrow_rule_row_keeps_its_controls_full_size(cx: &mut TestAppContext) {
    common::setup(cx);
    // Narrower than the row's fixed parts (swatch, gaps, switch, move buttons): only the pattern may give way.
    let w = common::open(cx, 120., 100., |_, _| {
        div().w(px(120.)).child(EditableRuleRow::new(0, Color::rgb(0xefaaa2), r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3}", "Match text · both panels")).into_any_element()
    });
    assert_eq!(common::bounds(cx, w, "profile-rule-0-toggle").size, size(px(28.), px(16.)));
    assert_eq!(common::bounds(cx, w, "profile-rule-0-up").size, size(px(22.), px(22.)));
    assert_eq!(common::bounds(cx, w, "profile-rule-0-down").size, size(px(22.), px(22.)));
}

#[gpui_kit::test]
fn a_text_field_is_as_tall_as_a_button(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 300., 100., |window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx).default_value("#ffaa00"));
        div().flex().items_start().child(TextField::new("hex", input).mono(true).width(96.)).into_any_element()
    });
    assert_eq!(common::bounds(cx, w, "hex").size, size(px(96.), px(22.)));
}

#[gpui_kit::test]
fn palette_slots_are_clickable(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = log();
    let e = events.clone();
    let w = common::open(cx, 400., 100., move |_, _| {
        let a = e.clone();
        PaletteSlot::new(2, Color::rgb(0xacd7ea), Color::rgb(0x315064)).selected(true).on_click(move |_, _| a.borrow_mut().push("slot".into())).into_any_element()
    });
    common::click(cx, w, "palette-slot-2");
    assert_eq!(*events.borrow(), ["slot"]);
}

#[gpui_kit::test]
fn custom_colour_renders_a_picker_trigger(cx: &mut TestAppContext) {
    use gpui_kit::component::color_picker::ColorPickerState;
    use rogg_ui::form::CustomColor;
    common::setup(cx);
    let slot: Rc<RefCell<Option<gpui_kit::Entity<ColorPickerState>>>> = Rc::default();
    let w = common::open(cx, 400., 200., move |window, cx| {
        let state = slot.borrow_mut().get_or_insert_with(|| cx.new(|cx| ColorPickerState::new(window, cx))).clone();
        CustomColor::new("rule-custom", &state).featured(vec![Color::rgb(0xff0000)]).into_any_element()
    });
    assert!(common::exists(cx, w, "rule-custom"));
}

#[gpui_kit::test]
fn custom_colour_is_a_swatch_sized_square_with_a_pen_and_still_opens_the_picker(cx: &mut TestAppContext) {
    use gpui_kit::component::color_picker::ColorPickerState;
    use rogg_ui::form::{CustomColor, Swatch};
    common::setup(cx);
    let slot: Rc<RefCell<Option<gpui_kit::Entity<ColorPickerState>>>> = Rc::default();
    let s = slot.clone();
    let w = common::open(cx, 400., 200., move |window, cx| {
        let state = s.borrow_mut().get_or_insert_with(|| cx.new(|cx| ColorPickerState::new(window, cx))).clone();
        div()
            .flex()
            .gap(px(8.))
            .child(Swatch::new("preset", Color::rgb(0xff0000)))
            .child(CustomColor::new("rule-custom", &state))
            .into_any_element()
    });
    let swatch = common::bounds(cx, w, "preset");
    let custom = common::bounds(cx, w, "rule-custom");
    assert_eq!(custom.size, swatch.size);
    assert_eq!(custom.size, size(px(21.), px(21.)));
    assert_eq!(custom.origin.y, swatch.origin.y, "lines up with the preset row");
    let pen = common::bounds(cx, w, "rule-custom-pen");
    assert_eq!(pen.size, size(px(11.), px(11.)));
    assert!(custom.contains(&pen.center()), "the pen sits inside the square");
    let state = slot.borrow().clone().unwrap();
    assert!(!cx.read_entity(&state, |s, _| s.is_open()));
    common::click(cx, w, "rule-custom");
    assert!(cx.read_entity(&state, |s, _| s.is_open()), "clicking the square opens the popover");
}

/// The square at (20, 20) in an 80x60 host, so clicks just outside it still land on the host.
fn custom_in_host(
    cx: &mut TestAppContext,
    default: Option<gpui_kit::Hsla>,
) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<gpui_kit::component::color_picker::ColorPickerState>) {
    use gpui_kit::component::color_picker::ColorPickerState;
    use gpui_kit::TestSupportExt;
    use rogg_ui::form::CustomColor;
    common::setup(cx);
    let slot: Rc<RefCell<Option<gpui_kit::Entity<ColorPickerState>>>> = Rc::default();
    let s = slot.clone();
    let w = common::open(cx, 400., 200., move |window, cx| {
        let state = s
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    let st = ColorPickerState::new(window, cx);
                    match default {
                        Some(c) => st.default_value(c),
                        None => st,
                    }
                })
            })
            .clone();
        div()
            .id("host")
            .test_support()
            .flex()
            .items_start()
            .size(px(80.))
            .pl(px(20.))
            .pt(px(20.))
            .child(CustomColor::new("rule-custom", &state))
            .into_any_element()
    });
    let state = slot.borrow().clone().unwrap();
    (w, state)
}

/// A click at `at` (relative to the host) and whether the popover opened; it is closed again afterwards.
fn click_opens(
    cx: &mut TestAppContext,
    w: gpui_kit::AnyWindowHandle,
    state: &gpui_kit::Entity<gpui_kit::component::color_picker::ColorPickerState>,
    at: (f32, f32),
) -> bool {
    use gpui_kit::point;
    use gpui_kit::test::TestWindowExt;
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click_at("host", point(px(at.0), px(at.1)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    let open = cx.read_entity(state, |s, _| s.is_open());
    cx.update_window(w, |_, window, cx| {
        state.update(cx, |s, cx| s.set_open(false, cx));
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    open
}

#[gpui_kit::test]
fn custom_colour_opens_from_its_whole_square_and_only_from_it(cx: &mut TestAppContext) {
    let (w, state) = custom_in_host(cx, None);
    // The square spans 20..41 on both axes. Corners and edges, 2 px in, all open the popover.
    for at in [(22., 22.), (39., 22.), (22., 39.), (39., 39.), (30., 22.), (22., 30.), (39., 30.), (30., 39.)] {
        assert!(click_opens(cx, w, &state, at), "a click at {at:?} inside the square opens the popover");
    }
    // Just outside, and where a text label or a bigger trigger would have been, it does not.
    for at in [(18., 30.), (30., 18.), (43., 30.), (30., 43.), (60., 30.), (30., 55.)] {
        assert!(!click_opens(cx, w, &state, at), "a click at {at:?} outside the square must not open the popover");
    }
}

#[gpui_kit::test]
fn custom_colour_shows_the_picked_colour_and_a_contrasting_pen(cx: &mut TestAppContext) {
    use rogg_ui::form::{pen_ink, PEN_DARK, PEN_LIGHT};
    use rogg_ui::theme::ActiveRoggTheme;
    let hex = |c: Color| format!("#{:08x}", c.0);
    let read = |cx: &mut TestAppContext, w, id: &'static str| {
        cx.update_window(w, |_, window, cx| {
            use gpui_kit::test::TestWindowExt;
            window.render_frame(cx);
            window.find(id).value().map(str::to_string)
        })
        .unwrap()
    };
    let text = |cx: &mut TestAppContext| cx.update(|cx| cx.rogg().palette.text);

    // A dark picked colour: that fill, light pen.
    let navy = Color::rgb(0x1a2b4c);
    let (w, state) = custom_in_host(cx, Some(navy.hsla()));
    assert_eq!(read(cx, w, "rule-custom-fill"), Some(hex(navy)));
    assert_eq!(read(cx, w, "rule-custom-pen"), Some(hex(PEN_LIGHT)));

    // Changing the value repaints: a light colour gets the dark pen.
    let cream = Color::rgb(0xf3e5ab);
    cx.update_window(w, |_, window, cx| state.update(cx, |s, cx| s.select_color(cream.hsla(), window, cx))).unwrap();
    cx.run_until_parked();
    assert_eq!(read(cx, w, "rule-custom-fill"), Some(hex(cream)));
    assert_eq!(read(cx, w, "rule-custom-pen"), Some(hex(PEN_DARK)));
    assert_eq!(read(cx, w, "rule-custom-pen"), Some(hex(pen_ink(cream, Color::TRANSPARENT))));

    // A mostly transparent colour shows the panel: the pen takes the theme text colour.
    let ghost = Color::rgb(0xf3e5ab).with_alpha(0x20);
    cx.update_window(w, |_, window, cx| state.update(cx, |s, cx| s.select_color(ghost.hsla(), window, cx))).unwrap();
    cx.run_until_parked();
    let t = text(cx);
    assert_eq!(read(cx, w, "rule-custom-pen"), Some(hex(t)));
}

#[gpui_kit::test]
fn custom_colour_without_a_value_is_a_neutral_square_with_a_visible_pen(cx: &mut TestAppContext) {
    use rogg_ui::form::pen_ink;
    use rogg_ui::theme::ActiveRoggTheme;
    let (w, _) = custom_in_host(cx, None);
    let (raised, text) = cx.update(|cx| (cx.rogg().palette.raised, cx.rogg().palette.text));
    let read = |cx: &mut TestAppContext, id: &'static str| {
        cx.update_window(w, |_, window, cx| {
            use gpui_kit::test::TestWindowExt;
            window.render_frame(cx);
            window.find(id).value().map(str::to_string)
        })
        .unwrap()
    };
    assert_eq!(read(cx, "rule-custom-fill"), Some(format!("#{:08x}", raised.0)));
    assert_eq!(read(cx, "rule-custom-pen"), Some(format!("#{:08x}", pen_ink(raised, text).0)));
}

#[gpui_kit::test]
fn a_text_field_with_a_width_does_not_shrink_in_a_crowded_row(cx: &mut TestAppContext) {
    common::setup(cx);
    // The row is narrower than its children: the field must still keep its width.
    let w = common::open(cx, 120., 100., |window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx).default_value("#ffaa00"));
        div()
            .w(px(120.))
            .flex()
            .items_start()
            .child(div().flex_none().w(px(60.)).h(px(21.)))
            .child(TextField::new("hex", input).mono(true).width(96.))
            .into_any_element()
    });
    assert_eq!(common::bounds(cx, w, "hex").size, size(px(96.), px(22.)));
}
