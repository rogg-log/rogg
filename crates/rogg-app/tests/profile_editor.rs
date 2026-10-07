mod common;
use gpui_kit::{AnyWindowHandle, AppContext, BorrowAppContext, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::dialogs::profiles::ProfileEditor;
use rogg_app::profiles::ProfileStore;
use rogg_app::settings::SettingsStore;
use rogg_app::tab::Tab;
use rogg_app::workspace::Workspace;

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub w: AnyWindowHandle,
    pub ws: Entity<Workspace>,
    pub tab: Entity<Tab>,
    pub editor: Entity<ProfileEditor>,
}

pub fn open_editor(cx: &mut TestAppContext) -> Fixture {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", &["2026-10-05 14:32:08.142  ERROR  disk full", "ok"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    common::click(cx, w, "profile");
    let editor = ws.read_with(cx, |ws, _| ws.profile_editor()).expect("the info bar's Profile button opens the editor");
    Fixture { dir, w, ws, tab, editor }
}

fn names(cx: &mut TestAppContext) -> Vec<String> {
    cx.update(|cx| cx.global::<ProfileStore>().names())
}

fn tab_profile(f: &Fixture, cx: &mut TestAppContext) -> String {
    f.tab.read_with(cx, |t, _| t.profile.to_string())
}

#[gpui_kit::test]
fn the_editor_opens_on_the_tabs_profile(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    assert_eq!(f.editor.read_with(cx, |e, _| e.profile_name().to_string()), "Default");
    assert!(common::exists(cx, f.w, "profile-rule-3"));
    assert!(common::exists(cx, f.w, "palette-slot-3"));
}

#[gpui_kit::test]
fn new_duplicate_and_delete_profiles(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-new");
    assert!(names(cx).contains(&"New profile".to_string()));
    assert_eq!(tab_profile(&f, cx), "New profile", "choosing a profile in the editor assigns it to the tab");
    assert!(f.dir.path().join("profiles/New profile.toml").exists());
    common::click(cx, f.w, "profile-duplicate");
    assert!(names(cx).contains(&"New profile copy".to_string()));
    // Another tab showing the profile about to be deleted.
    let log = common::write_log(f.dir.path(), "b.log", &["other"]);
    cx.update_window(f.w, |_, window, cx| f.ws.update(cx, |ws, cx| ws.open_paths(vec![log], window, cx))).unwrap();
    let other = f.ws.read_with(cx, |ws, _| ws.tabs()[1].clone());
    other.update(cx, |t, cx| t.set_profile("New profile copy".into(), cx));
    common::click(cx, f.w, "profile-delete");
    assert!(!names(cx).contains(&"New profile copy".to_string()));
    assert_eq!(tab_profile(&f, cx), "Default");
    assert_eq!(other.read_with(cx, |t, _| t.profile.to_string()), "Default", "every tab whose profile was deleted moves to Default");
    f.editor.update(cx, |e, cx| e.select_profile("Default", cx));
    common::click(cx, f.w, "profile-delete");
    assert!(names(cx).contains(&"Default".to_string()), "Default cannot be deleted");
}

#[gpui_kit::test]
fn rules_toggle_reorder_add_and_delete(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-rule-1-toggle");
    let enabled = cx.update(|cx| cx.global::<ProfileStore>().get("Default").rules[1].enabled);
    assert!(!enabled);
    let tint = f.tab.read_with(cx, |t, cx| match t.full.read(cx).rows().row(0) {
        rogg_ui::logview::LogRow::Line(l) => l.tint,
        _ => None,
    });
    assert!(tint.is_none(), "open tabs restyle at once");
    common::click(cx, f.w, "profile-rule-1-down");
    let patterns: Vec<String> = cx.update(|cx| cx.global::<ProfileStore>().get("Default").rules.iter().map(|r| r.pattern.clone()).collect());
    assert_eq!(patterns[2], r"\bERROR\b");
    common::click(cx, f.w, "rule-add");
    let count = cx.update(|cx| cx.global::<ProfileStore>().get("Default").rules.len());
    assert_eq!(count, 5);
    assert_eq!(f.editor.read_with(cx, |e, _| e.selected_rule()), Some(4));
    cx.update_window(f.w, |_, window, cx| f.editor.update(cx, |e, cx| e.delete_rule(4, window, cx))).unwrap();
    assert_eq!(cx.update(|cx| cx.global::<ProfileStore>().get("Default").rules.len()), 4);
}

#[gpui_kit::test]
fn renaming_follows_tabs_and_the_default_setting(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-new");
    cx.update(|cx| {
        SettingsStore::update(cx, |s| s.behavior.default_profile = "New profile".into());
    });
    common::click(cx, f.w, "profile-rename");
    let input = f.editor.read_with(cx, |e, _| e.rename_input()).unwrap();
    cx.update_window(f.w, |_, window, cx| input.update(cx, |i, cx| i.set_value("Ops", window, cx))).unwrap();
    common::click(cx, f.w, "profile-rename-apply");
    assert!(names(cx).contains(&"Ops".to_string()) && !names(cx).contains(&"New profile".to_string()));
    assert_eq!(tab_profile(&f, cx), "Ops");
    cx.update(|cx| assert_eq!(SettingsStore::get(cx).behavior.default_profile, "Ops"));
    assert!(f.dir.path().join("profiles/Ops.toml").exists() && !f.dir.path().join("profiles/New profile.toml").exists());
    common::click(cx, f.w, "profile-rename");
    let input = f.editor.read_with(cx, |e, _| e.rename_input()).unwrap();
    cx.update_window(f.w, |_, window, cx| input.update(cx, |i, cx| i.set_value("Default", window, cx))).unwrap();
    common::click(cx, f.w, "profile-rename-apply");
    assert!(common::exists(cx, f.w, "profile-rename-error"));
    assert!(names(cx).contains(&"Ops".to_string()));
}

fn rename(f: &Fixture, cx: &mut TestAppContext, to: &str) {
    common::click(cx, f.w, "profile-rename");
    let input = f.editor.read_with(cx, |e, _| e.rename_input()).unwrap();
    let to = to.to_string();
    cx.update_window(f.w, |_, window, cx| input.update(cx, |i, cx| i.set_value(to, window, cx))).unwrap();
    common::click(cx, f.w, "profile-rename-apply");
}

#[gpui_kit::test]
fn a_rename_refuses_names_that_would_share_another_profiles_file(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-new");
    rename(&f, cx, "default");
    assert!(common::exists(cx, f.w, "profile-rename-error"), "default shares Default's file");
    assert!(names(cx).contains(&"New profile".to_string()));
    rename(&f, cx, "new profile");
    assert!(!common::exists(cx, f.w, "profile-rename-error"), "a profile may change the case of its own name");
    assert!(names(cx).contains(&"new profile".to_string()) && !names(cx).contains(&"New profile".to_string()));
    assert_eq!(tab_profile(&f, cx), "new profile");
    let files: Vec<String> = std::fs::read_dir(f.dir.path().join("profiles")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    let kept = files.iter().filter(|n| n.eq_ignore_ascii_case("new profile.toml")).count();
    assert_eq!(kept, 1, "the case-only rename keeps its one file: {files:?}");
    let reloaded = ProfileStore::load(f.dir.path().join("profiles"));
    assert!(reloaded.contains("new profile"), "{:?}", reloaded.names());
    rename(&f, cx, "new/profile");
    assert!(!common::exists(cx, f.w, "profile-rename-error"));
    cx.update(|cx| {
        cx.update_global::<ProfileStore, _>(|s, _| s.add(rogg_core::profile::Profile { name: "Ops".into(), rules: vec![], hit_palette: vec![] }));
    });
    rename(&f, cx, "OPS");
    assert!(common::exists(cx, f.w, "profile-rename-error"), "OPS shares Ops's file");
    assert_eq!(f.editor.read_with(cx, |e, _| e.profile_name().to_string()), "new/profile");
}
