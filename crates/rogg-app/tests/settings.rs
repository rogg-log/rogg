mod common;
use gpui_kit::{AppContext, Entity, TestAppContext, px};
use rogg_app::args::Args;
use rogg_app::cache::CacheStore;
use rogg_app::settings::SettingsStore;
use rogg_app::tab::Tab;
use rogg_core::settings::{DensitySetting, ThemeSetting};
use rogg_ui::theme::{ActiveRoggTheme, Appearance, Density};

#[gpui_kit::test]
fn appearance_settings_apply_live_and_persist(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| {
        ws.update(cx, |ws, cx| {
            ws.change_settings(
                |s| {
                    s.appearance.theme = ThemeSetting::Dark;
                    s.appearance.log_font_size = 14;
                    s.appearance.density = DensitySetting::Comfortable;
                    s.appearance.log_font = "Menlo".into();
                    s.appearance.ui_font_size = 15;
                },
                window,
                cx,
            )
        })
    })
    .unwrap();
    cx.update(|cx| {
        let t = cx.rogg();
        assert_eq!(t.appearance, Appearance::Dark);
        assert_eq!((t.metrics.log_font_size, t.metrics.row_height), (px(15.), px(33.)));
        assert_eq!(t.fonts.log.as_ref(), "Menlo");
        assert_eq!(t.fonts.ui_size, px(15.));
    });
    let text = std::fs::read_to_string(dir.path().join("settings.toml")).unwrap();
    assert!(text.contains("theme = \"dark\"") && text.contains("log_font = \"Menlo\""), "{text}");
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(rogg_app::actions::ThemeLight), cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(cx.rogg().appearance, Appearance::Light);
        assert_eq!(SettingsStore::get(cx).appearance.theme, ThemeSetting::Light, "View → Theme is a setting too");
    });
}

#[gpui_kit::test]
fn new_tabs_take_the_behavior_defaults(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", &["error one", "two"]);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update(|cx| {
        SettingsStore::update(cx, |s| {
            s.behavior.wrap = true;
            s.behavior.follow_selection = false;
            s.behavior.context = 2;
            s.behavior.regex = true;
            s.behavior.case_sensitive = true;
        });
    });
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![log], window, cx))).unwrap();
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, cx| ws.active_tab().is_some_and(|t| t.read(cx).is_ready(cx))));
    ws.read_with(cx, |ws, cx| {
        let tab = ws.active_tab().unwrap();
        let t = tab.read(cx);
        assert!(t.full.read(cx).wrap() && t.filtered.read(cx).wrap() && !t.follow);
        let spec = &t.doc.read(cx).filter.spec;
        assert!(spec.regex && spec.case_sensitive && spec.context == 2 && spec.terms.is_empty());
    });
}

#[gpui_kit::test]
fn cache_limit_and_scan_depth_apply(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| {
        ws.update(cx, |ws, cx| {
            ws.change_settings(
                |s| {
                    s.behavior.cache_limit_gb = 5;
                    s.behavior.scan_depth = 1;
                },
                window,
                cx,
            )
        })
    })
    .unwrap();
    cx.update(|cx| assert_eq!(CacheStore::get(cx).limit(), 5 << 30));
    let root = dir.path().join("logs");
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::write(root.join("top.log"), "x\n").unwrap();
    std::fs::write(root.join("a/b/deep.log"), "y\n").unwrap();
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![root], window, cx))).unwrap();
    common::wait_until(cx, |cx| common::exists(cx, w, "tree-row-1"));
    assert!(!common::exists(cx, w, "tree-row-2"), "depth 1 lists only the folder's own files");
}

#[gpui_kit::test]
fn unreadable_settings_toast_and_reset(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.toml"), "[appearance\n").unwrap();
    cx.update(|cx| rogg_app::init_with(cx, rogg_app::paths::AppPaths::under(dir.path())));
    let (_w, ws) = common::open_workspace(cx, Args::default());
    ws.read_with(cx, |ws, _| assert!(ws.toasts.iter().any(|(_, t)| t.contains("settings.toml.bak")), "{:?}", ws.toasts));
    assert!(dir.path().join("settings.toml.bak").exists());
}

#[gpui_kit::test]
fn a_failing_settings_save_is_reported_once(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt;
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    let set_mode = |mode: u32| std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(mode)).unwrap();
    let change = |cx: &mut TestAppContext, wrap: bool| {
        cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.change_settings(|s| s.behavior.wrap = wrap, window, cx))).unwrap();
    };
    set_mode(0o555);
    change(cx, true);
    change(cx, false);
    set_mode(0o755);
    let failures = ws.read_with(cx, |ws, _| ws.toasts.iter().filter(|(_, t)| t.starts_with("Could not save settings: ")).count());
    assert_eq!(failures, 1, "two failing changes, one toast");
    assert!(ws.read_with(cx, |ws, _| ws.settings_error.is_some()));
    change(cx, true);
    assert!(ws.read_with(cx, |ws, _| ws.settings_error.is_none()), "a successful save clears the error");
    assert!(dir.path().join("settings.toml").exists());
}

#[gpui_kit::test]
fn a_pinned_theme_keeps_its_density_and_is_never_saved(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.pin_theme(ThemeSetting::Dark, DensitySetting::Comfortable, window, cx))).unwrap();
    cx.update(|cx| {
        assert_eq!(cx.rogg().appearance, Appearance::Dark);
        assert_eq!(cx.rogg().density, Density::Comfortable);
        assert_eq!(SettingsStore::get(cx).appearance.density, DensitySetting::Compact, "the settings are untouched");
    });
    assert!(!dir.path().join("settings.toml").exists(), "a pin never writes the settings");
}

#[gpui_kit::test]
fn the_settings_dialog_applies_each_change_at_once(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, _ws) = common::open_workspace(cx, Args::default());
    common::press(cx, w, "cmd-,");
    assert!(common::exists(cx, w, "setting-theme"));
    common::click(cx, w, "setting-theme-2");
    cx.update(|cx| assert_eq!(cx.rogg().appearance, Appearance::Dark));
    common::click(cx, w, "setting-log-size-up");
    common::click(cx, w, "setting-wrap");
    common::click(cx, w, "setting-context-up");
    common::click(cx, w, "setting-context-up");
    common::click(cx, w, "setting-cache-down");
    common::click(cx, w, "setting-depth-down");
    cx.update(|cx| {
        let s = SettingsStore::get(cx);
        assert_eq!((s.appearance.log_font_size, s.behavior.wrap, s.behavior.context, s.behavior.cache_limit_gb, s.behavior.scan_depth), (13, true, 2, 10, 3));
        assert_eq!(CacheStore::get(cx).limit(), 10 << 30);
        assert_eq!(cx.rogg().metrics.log_font_size, px(13.));
    });
    let (saved, _) = rogg_core::settings::Settings::load(&dir.path().join("settings.toml")).unwrap();
    assert_eq!(saved.behavior.context, 2);
    common::press(cx, w, "escape");
    assert!(!common::exists(cx, w, "setting-theme"));
    common::click(cx, w, "settings");
    assert!(common::exists(cx, w, "setting-theme"), "the title bar button opens it too");
}

#[gpui_kit::test]
fn the_log_fonts_are_looked_up_once_when_settings_open(cx: &mut TestAppContext) {
    common::setup(cx);
    let (w, _ws) = common::open_workspace(cx, Args::default());
    assert!(cx.update(|cx| !cx.has_global::<rogg_app::settings::LogFonts>()));
    common::press(cx, w, "cmd-,");
    assert!(common::exists(cx, w, "setting-theme"));
    let cached = cx.update(|cx| cx.try_global::<rogg_app::settings::LogFonts>().map(|f| f.0.clone()));
    assert_eq!(cached.as_deref(), Some(&["SF Mono".to_string()][..]), "the test text system has no other fonts; SF Mono is always offered");
    cx.update(|cx| cx.set_global(rogg_app::settings::LogFonts(vec!["SF Mono".into(), "Menlo".into()])));
    cx.update(|cx| assert_eq!(rogg_app::settings::available_log_fonts(cx), ["SF Mono", "Menlo"], "renders read the cache"));
}

#[gpui_kit::test]
fn picking_another_log_font_returns_to_settings(cx: &mut TestAppContext) {
    common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| {
        ws.update(cx, |ws, cx| {
            ws.open_dialog(rogg_app::workspace::Dialog::FontPicker, cx);
            ws.pick_log_font("Menlo", window, cx);
        })
    })
    .unwrap();
    cx.update(|cx| assert_eq!(SettingsStore::get(cx).appearance.log_font, "Menlo"));
    assert!(common::exists(cx, w, "setting-theme"));
}

#[test]
fn monospace_detection_and_font_options() {
    use rogg_app::dialogs::fonts::is_monospace;
    use rogg_app::dialogs::settings::{context_label, log_font_options};
    assert!(is_monospace([Some(7.2), Some(7.2), Some(7.2)]));
    assert!(!is_monospace([Some(3.), Some(9.), Some(3.)]));
    assert!(!is_monospace([Some(0.), Some(0.), Some(0.)]));
    assert!(!is_monospace([None, Some(7.), Some(7.)]));
    let (options, selected) = log_font_options(&["SF Mono".into(), "Menlo".into()], "Menlo");
    assert_eq!(options.iter().map(|o| o.as_ref()).collect::<Vec<_>>(), ["SF Mono", "Menlo", "Other…"]);
    assert_eq!(selected, 1);
    let (options, selected) = log_font_options(&["SF Mono".into()], "Iosevka");
    assert_eq!(options.iter().map(|o| o.as_ref()).collect::<Vec<_>>(), ["SF Mono", "Iosevka", "Other…"]);
    assert_eq!(selected, 1);
    assert_eq!([context_label(0), context_label(1), context_label(7)], ["None", "±1 line", "±7 lines"]);
}

fn gap_rows(tab: &Entity<Tab>, cx: &mut TestAppContext) -> usize {
    tab.read_with(cx, |t, cx| {
        let state = t.filtered.read(cx);
        let rows = state.rows();
        (0..rows.row_count()).filter(|&i| rows.is_gap(i)).count()
    })
}

/// A log of 20 lines with "needle" on lines 3 and 16 (display numbers), so the filtered view has one gap.
fn open_gapped(cx: &mut TestAppContext, w: gpui_kit::AnyWindowHandle, ws: &Entity<rogg_app::workspace::Workspace>, dir: &std::path::Path, name: &str) -> Entity<Tab> {
    let lines: Vec<String> = (1..=20).map(|n| if n == 3 || n == 16 { format!("needle {n}") } else { format!("line {n}") }).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let log = common::write_log(dir, name, &refs);
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![log], window, cx))).unwrap();
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    tab.read_with(cx, |t, _| t.doc.clone()).update(cx, |d, cx| d.set_expression("needle", cx)).unwrap();
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.filtered.read(cx).rows().row_count() == 3));
    tab
}

fn set_markers(cx: &mut TestAppContext, w: gpui_kit::AnyWindowHandle, ws: &Entity<rogg_app::workspace::Workspace>, on: bool) {
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.change_settings(|s| s.behavior.omitted_markers = on, window, cx))).unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn omitted_markers_are_on_by_default_and_toggle_live_in_every_tab(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update(|cx| assert!(SettingsStore::get(cx).behavior.omitted_markers));
    let first = open_gapped(cx, w, &ws, dir.path(), "a.log");
    let second = open_gapped(cx, w, &ws, dir.path(), "b.log");
    for tab in [&first, &second] {
        assert_eq!(gap_rows(tab, cx), 1);
    }
    // Select the second match (display number 16) in both tabs.
    for tab in [&first, &second] {
        tab.update(cx, |t, cx| t.filtered.update(cx, |s, cx| s.set_selected(Some(16), cx)));
    }
    set_markers(cx, w, &ws, false);
    for tab in [&first, &second] {
        assert_eq!(gap_rows(tab, cx), 0, "no gap rows when off");
        tab.read_with(cx, |t, cx| {
            let state = t.filtered.read(cx);
            assert_eq!(state.rows().row_count(), 2, "the two matches back to back");
            assert_eq!(state.selected(), Some(16), "the selection stays on its line");
            assert_eq!(state.rows().number_at(1), Some(16));
            assert_eq!(t.full.read(cx).rows().row_count(), 20, "the full log is unchanged");
        });
    }
    let (saved, _) = rogg_core::settings::Settings::load(&dir.path().join("settings.toml")).unwrap();
    assert!(!saved.behavior.omitted_markers, "saved");
    set_markers(cx, w, &ws, true);
    for tab in [&first, &second] {
        assert_eq!(gap_rows(tab, cx), 1, "the gap rows return");
        tab.read_with(cx, |t, cx| {
            let state = t.filtered.read(cx);
            assert_eq!((state.rows().row_count(), state.selected()), (3, Some(16)));
        });
    }
}

#[gpui_kit::test]
fn a_tab_opened_with_the_markers_off_has_no_gap_rows(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    set_markers(cx, w, &ws, false);
    let lines: Vec<String> = (1..=20).map(|n| if n == 3 || n == 16 { format!("needle {n}") } else { format!("line {n}") }).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let log = common::write_log(dir.path(), "c.log", &refs);
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![log], window, cx))).unwrap();
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    tab.read_with(cx, |t, _| t.doc.clone()).update(cx, |d, cx| d.set_expression("needle", cx)).unwrap();
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.filtered.read(cx).rows().row_count() == 2));
    assert_eq!(gap_rows(&tab, cx), 0);
}

#[gpui_kit::test]
fn the_settings_dialog_toggles_the_omitted_markers(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, _ws) = common::open_workspace(cx, Args::default());
    common::press(cx, w, "cmd-,");
    assert!(common::exists(cx, w, "row-omitted") && common::exists(cx, w, "setting-omitted"));
    common::click(cx, w, "setting-omitted");
    cx.update(|cx| assert!(!SettingsStore::get(cx).behavior.omitted_markers));
    let (saved, _) = rogg_core::settings::Settings::load(&dir.path().join("settings.toml")).unwrap();
    assert!(!saved.behavior.omitted_markers);
    common::click(cx, w, "setting-omitted");
    cx.update(|cx| assert!(SettingsStore::get(cx).behavior.omitted_markers));
}
