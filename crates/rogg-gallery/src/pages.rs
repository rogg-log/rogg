use crate::fixtures;
use crate::gallery::{GalleryView, Page};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Div, FontWeight, SharedString, Window, div, px};
use rogg_ui::chrome::{InfoBar, PaneHeader, Progress, StatPart, StatusBar};
use rogg_ui::controls::{ActionButton, CompactSelect, FindField, FlagButton, IconButton, Segmented, TextButton, ToolButton};
use rogg_ui::dialogs;
use rogg_ui::empty;
use rogg_ui::filter::{ChipState, FilterBand, FilterChip};
use rogg_ui::form::{EditableRuleRow, FormRow, PaletteSlot, SectionLabel, Stepper, Swatch, TextField, Toggle};
use rogg_ui::icons::IconName;
use rogg_ui::logview::LogView;
use rogg_ui::sidebar::{Sidebar, SidebarFooter};
use rogg_ui::theme::{ActiveRoggTheme, Palette};
use rogg_ui::titlebar::{TabItem, TitleBar};
use std::rc::Rc;

pub fn render(view: &mut GalleryView, window: &mut Window, cx: &mut Context<GalleryView>) -> AnyElement {
    let p = cx.rogg().palette;
    match view.page {
        Page::Tokens => tokens(cx),
        Page::Controls => controls(view, &p),
        Page::Tabs => tabs(&p),
        Page::Sidebar => sidebar(&p),
        Page::Filter => filter(view, &p),
        Page::LogView => logview(view, &p),
        Page::Chrome => chrome(view, &p),
        Page::Empty => empty_states(&p),
        Page::Dialogs => dialogs_page(view, &p),
        Page::App => {
            let app = match view.app.clone() {
                Some(app) => app,
                None => {
                    let app = GalleryView::new_app(window, cx);
                    view.app = Some(app.clone());
                    app
                }
            };
            if view.chrome {
                div().w(px(1440.)).h(px(900.)).flex().border_1().border_color(p.border).rounded(px(10.)).overflow_hidden().child(app).into_any_element()
            } else {
                div().size_full().child(app).into_any_element()
            }
        }
        Page::Replica => {
            let window_el = crate::replica::replica_window(&view.replica.clone(), cx);
            if view.chrome {
                div()
                    .w(px(1440.))
                    .h(px(900.))
                    .flex()
                    .border_1()
                    .border_color(p.border)
                    .rounded(px(10.))
                    .overflow_hidden()
                    .child(window_el)
                    .into_any_element()
            } else {
                window_el
            }
        }
    }
}

fn section(title: &str, body: impl IntoElement, p: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .mb(px(28.))
        .child(div().text_size(px(10.)).font_weight(FontWeight(600.)).text_color(p.muted).child(title.to_uppercase()))
        .child(body)
}

fn row() -> Div {
    div().flex().flex_wrap().items_center().gap(px(12.))
}

fn frame(w: f32, h: f32, p: &Palette, child: impl IntoElement) -> Div {
    div().w(px(w)).h(px(h)).flex().flex_col().border_1().border_color(p.border).rounded(px(6.)).overflow_hidden().bg(p.bg).child(child)
}

fn tokens(cx: &mut Context<GalleryView>) -> AnyElement {
    let t = cx.rogg().clone();
    let p = t.palette;
    let swatches = row().children(p.named().into_iter().map(|(name, color)| {
        div()
            .w(px(150.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().h(px(36.)).rounded(px(6.)).border_1().border_color(p.border).bg(color))
            .child(div().text_size(px(11.)).child(name))
            .child(div().text_size(px(10.)).font_family(t.fonts.mono.clone()).text_color(p.faint).child(format!("#{:08x}", color.0)))
    }));
    let scale = div().flex().flex_col().gap(px(6.)).children(
        [(9., "9 · hints"), (10., "10 · tools, stats"), (11., "11 · meta, tabs"), (12., "12 · body, dialogs"), (13., "13 · base"), (14., "14 · brand"), (15., "15 · empty title"), (17., "17 · dialog title")]
            .map(|(size, label)| div().text_size(px(size)).child(label)),
    );
    let mono = div().flex().flex_col().gap(px(6.)).font_family(t.fonts.mono.clone()).children(
        [10., 11., 12., 13.].map(|size| div().text_size(px(size)).child(format!("{size}px  2026-10-05 14:32:12.144  ERROR  client  upstream timeout"))),
    );
    let m = t.metrics;
    let metrics = div().text_size(px(12.)).text_color(p.muted).child(format!(
        "density {:?}: row {:?}, log font {:?}, pane header {:?}",
        t.density, m.row_height, m.log_font_size, m.pane_header_height
    ));
    div()
        .child(section("Palette", swatches, &p))
        .child(section("UI type scale (SF Pro)", scale, &p))
        .child(section("Log type (SF Mono)", mono, &p))
        .child(section("Metrics", metrics, &p))
        .into_any_element()
}

fn controls(view: &GalleryView, p: &Palette) -> AnyElement {
    let icons = row().children(IconName::ALL.iter().enumerate().map(|(i, name)| IconButton::icon(SharedString::from(format!("icon-{i}")), *name)));
    let buttons = row()
        .child(IconButton::icon("ib-normal", IconName::Edit))
        .child(IconButton::icon("ib-hover", IconName::Edit).force_hover(true))
        .child(IconButton::glyph("ib-chevron", "⌄").compact())
        .child(IconButton::glyph("ib-cmd", "⌘").compact())
        .child(IconButton::icon("ib-settings", IconName::Settings).compact())
        .child(IconButton::glyph("ib-close", "✕"));
    let flags = row()
        .child(FlagButton::new("flag-regex-off", ".*"))
        .child(FlagButton::new("flag-regex-on", ".*").pressed(true))
        .child(FlagButton::new("flag-case-off", "Aa"))
        .child(FlagButton::new("flag-case-on", "Aa").pressed(true));
    let tools = row()
        .child(ToolButton::new("tool-wrap-off", IconName::Wrap, "Wrap"))
        .child(ToolButton::new("tool-wrap-on", IconName::Wrap, "Wrap").pressed(true))
        .child(ToolButton::new("tool-follow", IconName::Follow, "Follow selection").pressed(true))
        .child(ToolButton::new("tool-open-tab", IconName::OpenTab, "Open as tab"))
        .child(ToolButton::new("tool-pause", IconName::Pause, "Paused").pressed(true).badge(Some("+1,204".into())));
    let text_buttons = row()
        .child(TextButton::meta("tb-profile", "Profile: Services⌄").dot())
        .child(TextButton::meta("tb-export", "Export").icon(IconName::Export))
        .child(TextButton::small("tb-history", "History⌄").icon(IconName::History))
        .child(ActionButton::new("ab-primary", "This direction works").primary())
        .child(ActionButton::new("ab-normal", "Needs adjustment"));
    let fields = row()
        .child(FindField::new("find-empty", view.inputs.find_empty.clone()))
        .child(FindField::new("find-query", view.inputs.find_query.clone()).count(Some("3 hits".into())))
        .child(Segmented::new("segmented", vec!["Compact".into(), "Comfortable".into()], 0))
        .child(CompactSelect::new("select", vec!["None".into(), "±1 line".into(), "±2 lines".into(), "±3 lines".into()], 0));
    let swatch_colors = [p.error, p.amber, p.muted, p.faint, p.accent, p.hits[0].fg, p.hits[1].fg, p.hits[2].fg, p.hits[3].fg];
    let form = row()
        .child(Toggle::new("specimen-toggle-on", true))
        .child(Toggle::new("specimen-toggle-off", false))
        .child(Stepper::new("specimen-stepper", "12 px"))
        .children(swatch_colors.into_iter().enumerate().map(|(i, c)| Swatch::new(SharedString::from(format!("specimen-swatch-{i}")), c).selected(i == 0)))
        .child(TextField::new("specimen-text-field", view.inputs.form_text.clone()).mono(true).width(110.));
    div()
        .child(section("All icons (15px)", icons, p))
        .child(section("Icon buttons: normal, hover, title bar", buttons, p))
        .child(section("Flags: off / on", flags, p))
        .child(section("Tool buttons", tools, p))
        .child(section("Text and action buttons", text_buttons, p))
        .child(section("Fields", fields, p))
        .child(section("Form controls: toggles, stepper, swatches, text field", form, p))
        .into_any_element()
}

fn tab_items(labels: &[&str], active: usize) -> Vec<TabItem> {
    labels.iter().enumerate().map(|(i, l)| TabItem { label: (*l).into(), tooltip: fixtures::BUNDLE.into(), active: i == active }).collect()
}

fn tabs(p: &Palette) -> AnyElement {
    let many: Vec<String> = (0..12).map(|i| format!("service-{i}-with-long-name.log")).collect();
    let many: Vec<&str> = many.iter().map(|s| s.as_str()).collect();
    div()
        .child(section("Reference: 3 tabs, api.log active", frame(1200., 42., p, TitleBar::new(tab_items(&["api.log", "worker.log", "gateway.log"], 0)).traffic_lights(true)), p))
        .child(section("Hovered inactive tab", frame(1200., 42., p, TitleBar::new(tab_items(&["api.log", "worker.log", "gateway.log"], 0)).traffic_lights(true).force_hover_tab(Some(1))), p))
        .child(section("12 long tabs in 900px (scrolls)", frame(900., 42., p, TitleBar::new(tab_items(&many, 3)).traffic_lights(true)), p))
        .into_any_element()
}

fn sidebar(p: &Palette) -> AnyElement {
    row()
        .child(section(
            "Bundle tree, recent, footer",
            frame(211., 640., p, Sidebar::new().count("10 files".into()).rows(fixtures::sidebar_rows("api.log")).recent(fixtures::recent()).footer(Some(fixtures::sidebar_footer()))),
            p,
        ))
        .child(section(
            "Folder tree: nested archive listing, unreadable archive, compressed log",
            frame(
                211.,
                640.,
                p,
                Sidebar::new()
                    .count("2 files".into())
                    .rows(fixtures::sidebar_tree_rows())
                    .footer(Some(SidebarFooter { title: "support-2026-10-05".into(), lines: vec!["2 files · 450 MB".into()] })),
            ),
            p,
        ))
        .into_any_element()
}

fn filter(view: &GalleryView, p: &Palette) -> AnyElement {
    let states = [ChipState::Include, ChipState::Off, ChipState::Exclude];
    let chips = div().flex().flex_col().gap(px(8.)).children(states.map(|state| {
        row().children((0..4).map(move |slot| FilterChip::new(slot, ["error", "timeout", "healthcheck", "retry"][slot], state)))
    }));
    let terms = fixtures::reference_terms();
    let many: Vec<(SharedString, ChipState)> = (0..20).map(|i| (format!("term{i}").into(), states[i % 3])).collect();
    div()
        .child(section("Chips: slot × state", chips, p))
        .child(section("Reference band", frame(1000., 90., p, FilterBand::new(terms.clone()).summary("2 include · 1 exclude · 1 off".into())), p))
        .child(section(
            "Editing with error",
            frame(1000., 110., p, FilterBand::new(terms.clone()).editing(Some(view.inputs.expression.clone())).error(Some("unclosed group".into())).summary("2 include · 1 exclude · 1 off".into())),
            p,
        ))
        .child(section("Regex + case on, context ±2", frame(1000., 90., p, FilterBand::new(terms).regex(true).case_sensitive(true).context(vec!["None".into(), "±1 line".into(), "±2 lines".into(), "±3 lines".into()], 2)), p))
        .child(section("20 chips in 900px", frame(900., 90., p, FilterBand::new(many)), p))
        .into_any_element()
}

fn logview(view: &GalleryView, p: &Palette) -> AnyElement {
    div()
        .child(section("No wrap: hover (row 2), selected, pinned, context, gap, hits, search, error tint, long line, horizontal bar (shown on hover)", frame(1100., 300., p, LogView::new("log-compact", view.log_compact.clone())), p))
        .child(section("Wrap on (700px)", frame(700., 420., p, LogView::new("log-wrapped", view.log_wrapped.clone())), p))
        .into_any_element()
}

fn chrome(view: &GalleryView, p: &Palette) -> AnyElement {
    let stats = vec![
        StatPart::Strong("6".into()),
        StatPart::Text(" matches / 40 lines · ".into()),
        StatPart::Strong("1".into()),
        StatPart::Text(" pinned (1 added) · ".into()),
        StatPart::Strong("0".into()),
        StatPart::Text(" context".into()),
    ];
    div()
        .child(section("Info bar", frame(1100., 33., p, InfoBar::new(vec![fixtures::BUNDLE.into(), "api.log".into()], "Services".into())), p))
        .child(section(
            "Pane headers",
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(frame(1100., 40., p, PaneHeader::new("ph-full", "Full log", "40 sample lines").find(FindField::new("ph-find-top", view.inputs.find_empty.clone())).tool(ToolButton::new("ph-wrap-top", IconName::Wrap, "Wrap"))))
                .child(frame(1100., 40., p, PaneHeader::new("ph-live", "Full log", "1,204,331 lines").tool(ToolButton::new("ph-live-pause", IconName::Pause, "Pause")).tool(ToolButton::new("ph-live-wrap", IconName::Wrap, "Wrap"))))
                .child(frame(
                    1100.,
                    40.,
                    p,
                    PaneHeader::new("ph-paused", "Full log", "1,205,535 lines")
                        .tool(ToolButton::new("ph-live-paused", IconName::Pause, "Paused").pressed(true).badge(Some("+1,204".into())))
                        .tool(ToolButton::new("ph-paused-wrap", IconName::Wrap, "Wrap")),
                ))
                .child(frame(
                    1100.,
                    40.,
                    p,
                    PaneHeader::new("ph-filtered", "Filtered log", "7 shown")
                        .find(FindField::new("ph-find-bottom", view.inputs.find_query.clone()).count(Some("3 hits".into())))
                        .tool(ToolButton::new("ph-open-tab", IconName::OpenTab, "Open as tab"))
                        .tool(ToolButton::new("ph-follow", IconName::Follow, "Follow selection").pressed(true))
                        .tool(ToolButton::new("ph-wrap-bottom", IconName::Wrap, "Wrap")),
                )),
            p,
        ))
        .child(section(
            "Status bar: idle, selection, progress",
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(frame(1100., 29., p, StatusBar::new("UTF-8 · LF · sample excerpt".into()).stats(stats.clone())))
                .child(frame(1100., 29., p, StatusBar::new("UTF-16LE → UTF-8 · CRLF".into()).stats(stats).selection(Some("Line 120492".into())).progress(Some(Progress { label: "Filtering 63%".into(), fraction: 0.63 })))),
            p,
        ))
        .into_any_element()
}

fn empty_states(p: &Palette) -> AnyElement {
    let noop: rogg_ui::Callback = Rc::new(|_, _| {});
    let noop_ix: rogg_ui::IndexCallback = Rc::new(|_, _, _| {});
    row()
        .child(frame(600., 300., p, empty::no_matches("empty-no-matches")))
        .child(frame(600., 300., p, empty::welcome(fixtures::recent(), noop.clone(), noop.clone(), noop_ix)))
        .child(frame(600., 300., p, empty::loading("Indexing api.log · 412 MB / 1.0 GB", Some(0.41), noop.clone())))
        .child(frame(600., 300., p, empty::loading("Extracting api.log", Some(0.42), noop.clone())))
        .child(frame(600., 300., p, empty::error("Could not read archive member", "incident-2026-10-05.zip › gateway.log", noop)))
        .into_any_element()
}

fn dialogs_page(view: &GalleryView, p: &Palette) -> AnyElement {
    let noop_ix: rogg_ui::IndexCallback = Rc::new(|_, _, _| {});
    let tabs: Vec<(SharedString, bool)> = vec![("api.log".into(), true), ("worker.log".into(), false), ("gateway.log".into(), false)];
    row()
        .items_start()
        .child(dialogs::all_tabs(&tabs, noop_ix.clone()).description(fixtures::tabs_description()))
        .child(dialogs::profile("Services", &fixtures::profile_rules()).description(fixtures::PROFILE_DESCRIPTION).note(fixtures::PROFILE_NOTE))
        .child(dialogs::appearance(&fixtures::appearance_rows()).description(fixtures::APPEARANCE_DESCRIPTION).note(fixtures::APPEARANCE_NOTE))
        .child(dialogs::shortcuts(&fixtures::shortcut_rows()).description(fixtures::SHORTCUTS_DESCRIPTION).note(fixtures::SHORTCUTS_NOTE))
        .child(dialogs::history(&fixtures::history_entries()).description(fixtures::HISTORY_DESCRIPTION).note(fixtures::HISTORY_NOTE))
        .child(dialogs::statistics(&[("Matches".into(), "6 / 40 lines".into()), ("Pinned".into(), "1 (1 added)".into()), ("error".into(), "6 hits".into()), ("timeout".into(), "7 hits".into()), ("Scan time".into(), "0.4 ms".into())]))
        .child(dialogs::export(&[("Include context lines".into(), true), ("Include gap markers".into(), false), ("Prefix line numbers".into(), true)], noop_ix, Rc::new(|_, _| {})))
        .child(dialogs::go_to_line(view.inputs.goto.clone()))
        .child(
            rogg_ui::dialogs::DialogFrame::new("Highlighting · Services")
                .child(
                    div()
                        .child(EditableRuleRow::new(0, p.error, "ERROR", "Line text + row tint · both panels").selected(true).can_move(false, true))
                        .child(EditableRuleRow::new(1, p.amber, "WARN", "Line text · both panels"))
                        .child(
                            EditableRuleRow::new(2, p.accent, "(unclosed", "Line text")
                                .enabled(false)
                                .can_move(true, false)
                                .error(Some("invalid regex: unclosed group".into())),
                        ),
                )
                .child(SectionLabel::new("Filter-match palette"))
                .child(
                    row().children(p.hits.iter().enumerate().map(|(i, h)| PaletteSlot::new(i, h.fg, h.bg).selected(i == 0))),
                ),
        )
        .child(
            rogg_ui::dialogs::DialogFrame::new("Settings").child(SectionLabel::new("Appearance")).child(
                div()
                    .child(FormRow::new("settings-theme", "Theme").control(Segmented::new("settings-theme-seg", vec!["System".into(), "Dark".into(), "Light".into()], 0)))
                    .child(FormRow::new("settings-ui-size", "UI font size").control(Stepper::new("settings-ui-size-stepper", "13 px")))
                    .child(FormRow::new("settings-wrap", "Wrap lines").control(Toggle::new("settings-wrap-toggle", false)))
                    .child(
                        FormRow::new("settings-cache", "Cache limit")
                            .hint("Extracted archive members and conversions")
                            .control(Stepper::new("settings-cache-stepper", "2 GB")),
                    ),
            ),
        )
        .child(div().w(px(500.)).text_size(px(11.)).text_color(p.faint).child("Every dialog renders here as a static specimen. The mockup replica shows them as modals over the window."))
        .into_any_element()
}
