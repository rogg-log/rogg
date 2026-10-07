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
fn deleting_the_default_profile_setting_falls_back_to_default(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-new");
    cx.update(|cx| {
        SettingsStore::update(cx, |s| s.behavior.default_profile = "New profile".into());
    });
    common::click(cx, f.w, "profile-delete");
    assert!(!names(cx).contains(&"New profile".to_string()));
    cx.update(|cx| assert_eq!(SettingsStore::get(cx).behavior.default_profile, "Default", "the setting does not name a deleted profile"));
    let saved = std::fs::read_to_string(f.dir.path().join("settings.toml")).unwrap();
    assert!(!saved.contains("New profile"), "the reset is saved: {saved}");
    let log = common::write_log(f.dir.path(), "b.log", &["other"]);
    cx.update_window(f.w, |_, window, cx| f.ws.update(cx, |ws, cx| ws.open_paths(vec![log.clone()], window, cx))).unwrap();
    let opened = f.ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    assert_eq!(opened.read_with(cx, |t, _| t.profile.to_string()), "Default");
    // A setting that names no profile (edited by hand, or saved before the reset failed) opens tabs on Default too.
    cx.update(|cx| {
        SettingsStore::update(cx, |s| s.behavior.default_profile = "Gone".into());
    });
    let log = common::write_log(f.dir.path(), "c.log", &["third"]);
    cx.update_window(f.w, |_, window, cx| f.ws.update(cx, |ws, cx| ws.open_paths(vec![log], window, cx))).unwrap();
    let third = f.ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    assert_eq!(third.read_with(cx, |t, _| t.profile.to_string()), "Default", "an unknown profile name resolves to Default");
}

#[gpui_kit::test]
fn a_failing_profile_save_is_reported_once(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt;
    let f = open_editor(cx);
    let profiles = f.dir.path().join("profiles");
    std::fs::create_dir_all(&profiles).unwrap();
    let set_mode = |mode: u32| std::fs::set_permissions(&profiles, std::fs::Permissions::from_mode(mode)).unwrap();
    set_mode(0o555);
    common::click(cx, f.w, "profile-rule-1-toggle");
    common::click(cx, f.w, "profile-rule-1-toggle");
    set_mode(0o755);
    let failures = f.ws.read_with(cx, |ws, _| ws.toasts.iter().filter(|(_, t)| t.starts_with("Could not save profile Default: ")).count());
    assert_eq!(failures, 1, "two failing edits, one toast: {:?}", f.ws.read_with(cx, |ws, _| ws.toasts.clone()));
    assert!(f.ws.read_with(cx, |ws, _| ws.profile_error.is_some()));
    common::click(cx, f.w, "profile-rule-1-toggle");
    assert!(f.ws.read_with(cx, |ws, _| ws.profile_error.is_none()), "a successful save clears the error");
    assert!(profiles.join("Default.toml").exists());
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

#[gpui_kit::test]
fn default_cannot_be_renamed(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    assert!(!common::exists(cx, f.w, "profile-rename"), "Default shows no Rename button");
    cx.update_window(f.w, |_, window, cx| {
        f.editor.update(cx, |e, cx| {
            e.start_rename(window, cx);
            e.apply_rename(window, cx);
        })
    })
    .unwrap();
    assert!(f.editor.read_with(cx, |e, _| e.rename_input()).is_none());
    assert!(!common::exists(cx, f.w, "profile-name"));
    assert!(names(cx).contains(&"Default".to_string()));
    common::click(cx, f.w, "profile-new");
    assert!(common::exists(cx, f.w, "profile-rename"), "other profiles can be renamed");
}

#[gpui_kit::test]
fn the_rule_selection_follows_reordering(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-rule-2");
    assert_eq!(f.editor.read_with(cx, |e, _| e.selected_rule()), Some(2));
    common::click(cx, f.w, "profile-rule-1-down");
    assert_eq!(f.editor.read_with(cx, |e, _| e.selected_rule()), Some(1), "the selected rule moved up into slot 1");
    common::click(cx, f.w, "profile-rule-1-down");
    assert_eq!(f.editor.read_with(cx, |e, _| e.selected_rule()), Some(2), "the selected rule itself moved down");
    common::click(cx, f.w, "profile-rule-2-up");
    assert_eq!(f.editor.read_with(cx, |e, _| e.selected_rule()), Some(1));
    common::click(cx, f.w, "profile-rule-0-down");
    assert_eq!(f.editor.read_with(cx, |e, _| e.selected_rule()), Some(0), "moving the rule above it down moves the selection up");
}

fn select_rule(f: &Fixture, cx: &mut TestAppContext, i: usize) {
    cx.update_window(f.w, |_, window, cx| f.editor.update(cx, |e, cx| e.select_rule(Some(i), window, cx))).unwrap();
}

/// Replaces an input's text the way typing does (`replace_all` emits `InputEvent::Change`).
fn type_into(f: &Fixture, cx: &mut TestAppContext, input: &gpui_kit::Entity<gpui_kit::component::input::InputState>, text: &str) {
    let text = text.to_string();
    cx.update_window(f.w, |_, window, cx| input.update(cx, |i, cx| i.replace_all(text, window, cx))).unwrap();
    cx.run_until_parked();
}

fn rule(cx: &mut TestAppContext, i: usize) -> rogg_core::profile::LineRule {
    cx.update(|cx| cx.global::<ProfileStore>().get("Default").rules[i].clone())
}

#[gpui_kit::test]
fn rule_form_edits_apply_immediately(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    select_rule(&f, cx, 0);
    assert!(common::exists(cx, f.w, "rule-form"));
    let (pattern, _, _) = f.editor.read_with(cx, |e, _| e.rule_inputs()).unwrap();
    type_into(&f, cx, &pattern, "disk");
    common::click(cx, f.w, "rule-effect-bold");
    common::click(cx, f.w, "rule-panes-1");
    common::click(cx, f.w, "rule-swatch-amber");
    common::click(cx, f.w, "rule-regex");
    let r = rule(cx, 0);
    assert_eq!(r.pattern, "disk");
    assert!(r.bold && !r.regex, "the timestamp rule was a regex; the flag toggled it off");
    assert_eq!(r.panes, rogg_core::profile::Panes::Filtered);
    assert_eq!(r.color, rogg_core::profile::ColorSpec::token("amber"));
    let saved = std::fs::read_to_string(f.dir.path().join("profiles/Default.toml")).unwrap();
    assert!(saved.contains("pattern = \"disk\""), "{saved}");
}

#[gpui_kit::test]
fn an_invalid_pattern_shows_its_error_and_keeps_other_rules(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    select_rule(&f, cx, 3);
    let (pattern, _, _) = f.editor.read_with(cx, |e, _| e.rule_inputs()).unwrap();
    type_into(&f, cx, &pattern, "(unclosed");
    assert!(cx.update(|cx| cx.global::<ProfileStore>().errors("Default").iter().any(|e| e.rule == 3)));
    assert!(common::exists(cx, f.w, "profile-rule-3-error"));
    let tint = f.tab.read_with(cx, |t, cx| match t.full.read(cx).rows().row(0) {
        rogg_ui::logview::LogRow::Line(l) => l.tint,
        _ => None,
    });
    assert!(tint.is_some(), "the ERROR rule still styles the row");
}

#[gpui_kit::test]
fn colour_fields_accept_tokens_and_hex_and_flag_the_rest(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    select_rule(&f, cx, 2);
    let (_, color, light) = f.editor.read_with(cx, |e, _| e.rule_inputs()).unwrap();
    type_into(&f, cx, &color, "#123456");
    assert_eq!(rule(cx, 2).color.0, "#123456");
    type_into(&f, cx, &color, "nope");
    assert_eq!(rule(cx, 2).color.0, "#123456", "an unreadable colour is not applied");
    assert!(f.editor.read_with(cx, |e, _| e.color_error()));
    type_into(&f, cx, &light, "#abcdef");
    assert_eq!(rule(cx, 2).light.map(|c| c.0).as_deref(), Some("#abcdef"));
    type_into(&f, cx, &light, "");
    assert_eq!(rule(cx, 2).light, None, "an empty light colour derives it again");
}

#[gpui_kit::test]
fn palette_slots_add_edit_and_remove(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "palette-slot-0");
    common::click(cx, f.w, "slot-add");
    let len = |cx: &mut TestAppContext| cx.update(|cx| cx.global::<ProfileStore>().get("Default").hit_palette.len());
    assert_eq!(len(cx), 5);
    // "Add colour" selects the new slot (clicking `palette-slot-4` now would toggle the selection off).
    assert_eq!(f.editor.read_with(cx, |e, _| e.selected_slot()), Some(4));
    let [fg, _, _, _] = f.editor.read_with(cx, |e, _| e.slot_inputs()).unwrap();
    type_into(&f, cx, &fg, "#010203");
    assert_eq!(cx.update(|cx| cx.global::<ProfileStore>().get("Default").hit_palette[4].fg.0.clone()), "#010203");
    common::click(cx, f.w, "slot-remove");
    assert_eq!(len(cx), 4);
}

#[gpui_kit::test]
fn the_rule_form_keeps_editing_its_rule_after_a_reorder(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    select_rule(&f, cx, 2);
    let (pattern, _, _) = f.editor.read_with(cx, |e, _| e.rule_inputs()).unwrap();
    let above = rule(cx, 1).pattern;
    common::click(cx, f.w, "profile-rule-2-up");
    type_into(&f, cx, &pattern, "moved");
    assert_eq!(rule(cx, 1).pattern, "moved", "the input edits the selected rule at its new index");
    assert_eq!(rule(cx, 2).pattern, above, "the rule it swapped with is untouched");
}

#[gpui_kit::test]
fn profiles_import_and_export(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    let out = f.dir.path().join("shared.roggprofile");
    f.editor.update(cx, |e, cx| e.export_profile_to(&out, cx));
    assert!(out.exists());
    f.editor.update(cx, |e, cx| e.import_profile_from(&out, cx));
    assert!(names(cx).contains(&"Default 2".to_string()), "an imported name that is taken gets a number");
    assert_eq!(f.editor.read_with(cx, |e, _| e.profile_name().to_string()), "Default 2");
    std::fs::write(&out, "rules = 1").unwrap();
    f.editor.update(cx, |e, cx| e.import_profile_from(&out, cx));
    f.ws.read_with(cx, |ws, _| assert!(ws.toasts.iter().any(|(_, t)| t.contains("shared.roggprofile")), "{:?}", ws.toasts));
}
