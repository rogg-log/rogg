mod common;
use gpui_kit::{AppContext, BorrowAppContext, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::profiles::ProfileStore;
use rogg_app::tab::Tab;
use rogg_core::profile::{ColorSpec, HitEntry, LineRule, Profile, derive_light};
use rogg_core::settings::ThemeSetting;
use rogg_ui::logview::{LogLine, LogRow, RowTint, SpanStyle};
use rogg_ui::theme::{ActiveRoggTheme, Color};

const ERROR_LINE: &str = "2026-10-05 14:32:08.142  ERROR  disk full";

fn open(cx: &mut TestAppContext, lines: &[&str]) -> (tempfile::TempDir, gpui_kit::AnyWindowHandle, Entity<rogg_app::workspace::Workspace>, Entity<Tab>) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", lines);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    (dir, w, ws, tab)
}

fn full_row(tab: &Entity<Tab>, cx: &mut TestAppContext, i: usize) -> LogLine {
    tab.read_with(cx, |t, cx| match t.full.read(cx).rows().row(i) {
        LogRow::Line(l) => l,
        LogRow::Gap { .. } => panic!("gap"),
    })
}

fn add_profile(cx: &mut TestAppContext, p: Profile) {
    cx.update(|cx| cx.update_global::<ProfileStore, _>(|s, _| s.add(p)));
}

#[gpui_kit::test]
fn rows_follow_the_tab_profile(cx: &mut TestAppContext) {
    let (_dir, _w, _ws, tab) = open(cx, &[ERROR_LINE, "2026-10-05 14:32:09.000  INFO   ok"]);
    let p = cx.update(|cx| cx.rogg().palette);
    let row = full_row(&tab, cx, 0);
    assert_eq!(row.tint, Some(RowTint::Color(p.error_row_tint)));
    assert!(row.spans.iter().any(|s| s.range == (0..23) && s.style == SpanStyle::Fg(p.faint)));
    assert!(row.spans.iter().any(|s| s.style == SpanStyle::Fg(p.error)), "ERROR colours the line");
    assert_eq!(tab.read_with(cx, |t, _| t.profile.clone()).as_ref(), "Default");
    add_profile(cx, Profile { name: "Plain".into(), rules: vec![], hit_palette: vec![HitEntry::slot(0)] });
    tab.update(cx, |t, cx| t.set_profile("Plain".into(), cx));
    let plain = full_row(&tab, cx, 0);
    assert!(plain.tint.is_none() && plain.spans.is_empty());
}

#[gpui_kit::test]
fn editing_a_profile_restyles_open_tabs(cx: &mut TestAppContext) {
    let (dir, _w, ws, tab) = open(cx, &[ERROR_LINE]);
    cx.update(|cx| cx.update_global::<ProfileStore, _>(|s, _| s.edit("Default", |p| p.rules.clear())));
    ws.update(cx, |ws, cx| ws.restyle_tabs(cx));
    assert!(full_row(&tab, cx, 0).tint.is_none());
    let saved = std::fs::read_to_string(dir.path().join("profiles/Default.toml")).unwrap();
    assert!(!saved.contains("[[rules]]"), "{saved}");
}

#[gpui_kit::test]
fn filter_hits_use_the_profile_palette(cx: &mut TestAppContext) {
    let (_dir, w, ws, tab) = open(cx, &["disk full", "ok"]);
    // Hex colours are used as is in the dark theme (the headless window reports a light appearance).
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.set_theme(ThemeSetting::Dark, window, cx))).unwrap();
    let entry = HitEntry { fg: ColorSpec::hex(0x010203ff), bg: ColorSpec::hex(0x040506ff), light_fg: None, light_bg: None };
    add_profile(cx, Profile { name: "Pal".into(), rules: vec![], hit_palette: vec![entry] });
    tab.update(cx, |t, cx| t.set_profile("Pal".into(), cx));
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("disk", cx)).unwrap();
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.filtered.read(cx).rows().row_count() == 1));
    let row = tab.read_with(cx, |t, cx| match t.filtered.read(cx).rows().row(0) {
        LogRow::Line(l) => l,
        LogRow::Gap { .. } => panic!("gap"),
    });
    assert!(row.spans.iter().any(|s| s.range == (0..4) && s.style == SpanStyle::Mark { fg: Color(0x010203ff), bg: Color(0x040506ff) }), "{:?}", row.spans);
    assert_eq!(tab.read_with(cx, |t, _| t.styler().unwrap().chips().len()), 1);
}

#[gpui_kit::test]
fn the_light_theme_uses_overrides_or_derived_colours(cx: &mut TestAppContext) {
    let (_dir, w, ws, tab) = open(cx, &["disk full"]);
    let derived = LineRule { color: ColorSpec::hex(0xefaaa2ff), ..LineRule::new("disk") };
    let overridden = LineRule { color: ColorSpec::hex(0xefaaa2ff), light: Some(ColorSpec::hex(0x123456ff)), ..LineRule::new("full") };
    add_profile(cx, Profile { name: "Hex".into(), rules: vec![derived, overridden], hit_palette: vec![HitEntry::slot(0)] });
    tab.update(cx, |t, cx| t.set_profile("Hex".into(), cx));
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.set_theme(ThemeSetting::Light, window, cx))).unwrap();
    let row = full_row(&tab, cx, 0);
    assert!(row.spans.contains(&rogg_ui::logview::Span { range: 0..4, style: SpanStyle::Fg(Color(derive_light(0xefaaa2ff, false))) }));
    assert!(row.spans.contains(&rogg_ui::logview::Span { range: 5..9, style: SpanStyle::Fg(Color(0x123456ff)) }));
}

#[gpui_kit::test]
fn an_unreadable_profile_is_reported(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("profiles")).unwrap();
    std::fs::write(dir.path().join("profiles/Bad.toml"), "rules = 1").unwrap();
    cx.update(|cx| rogg_app::init_with(cx, rogg_app::paths::AppPaths::under(dir.path())));
    let (_w, ws) = common::open_workspace(cx, Args::default());
    ws.read_with(cx, |ws, _| assert!(ws.toasts.iter().any(|(_, t)| t.contains("Bad.toml.bak")), "{:?}", ws.toasts));
    cx.update(|cx| assert_eq!(cx.global::<ProfileStore>().names(), ["Default"]));
}
