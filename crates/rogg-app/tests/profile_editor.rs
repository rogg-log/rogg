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
    assert!(!common::exists(cx, f.w, "profile-delete"), "Default has no Delete button");
    assert!(!f.editor.update(cx, |e, cx| e.delete(cx)));
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
fn import_and_export_sit_by_the_note_and_renaming_gets_its_own_line(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-new");
    let toolbar = common::bounds(cx, f.w, "profile-new");
    for id in ["profile-select", "profile-duplicate", "profile-rename", "profile-delete"] {
        assert_eq!(common::bounds(cx, f.w, id).center().y, toolbar.center().y, "{id} is on the toolbar line");
    }
    let palette = common::bounds(cx, f.w, "palette-slot-0");
    let (import, export) = (common::bounds(cx, f.w, "profile-import"), common::bounds(cx, f.w, "profile-export"));
    assert!(import.origin.y > palette.bottom() && export.origin.y == import.origin.y, "on the bottom row: {import:?} {export:?}");
    let dialog = common::bounds(cx, f.w, "dialog");
    assert!(export.right() > dialog.right() - gpui_kit::px(40.), "right-aligned: {export:?} in {dialog:?}");
    common::click(cx, f.w, "profile-rename");
    // The centred dialog grew: measure the toolbar again.
    let toolbar = common::bounds(cx, f.w, "profile-new");
    let field = common::bounds(cx, f.w, "profile-name");
    assert!(field.origin.y >= toolbar.bottom(), "the name field is under the toolbar: {field:?} {toolbar:?}");
    assert_eq!(common::bounds(cx, f.w, "profile-rename-apply").center().y, field.center().y);
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

#[gpui_kit::test]
fn custom_colours_are_written_as_hex_and_applied(cx: &mut TestAppContext) {
    use gpui_kit::rgba;
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-duplicate"); // "Default copy": an editable profile with rules
    let name = f.editor.read_with(cx, |e, _| e.profile_name().to_string());
    cx.update_window(f.w, |_, window, cx| f.editor.update(cx, |e, cx| e.select_rule(Some(0), window, cx))).unwrap();
    let [color, light] = f.editor.read_with(cx, |e, _| e.rule_pickers()).expect("the rule form has pickers");
    cx.update_window(f.w, |_, window, cx| color.update(cx, |p, cx| p.select_color(rgba(0x336699ff).into(), window, cx))).unwrap();
    cx.update_window(f.w, |_, window, cx| light.update(cx, |p, cx| p.select_color(rgba(0x33669980).into(), window, cx))).unwrap();
    cx.run_until_parked();
    let rule = cx.update(|cx| cx.global::<ProfileStore>().get(&name).rules[0].clone());
    assert_eq!(rule.color.0, "#336699");
    assert_eq!(rule.light.map(|l| l.0).as_deref(), Some("#33669980"), "alpha below 1 is kept as #rrggbbaa");
    let (_, color_input, _) = f.editor.read_with(cx, |e, _| e.rule_inputs()).unwrap();
    assert_eq!(color_input.read_with(cx, |i, _| i.value().to_string()), "#336699", "the hex field shows the picked colour");

    cx.update_window(f.w, |_, window, cx| f.editor.update(cx, |e, cx| e.select_slot(Some(0), window, cx))).unwrap();
    let [fg, bg, _, light_bg] = f.editor.read_with(cx, |e, _| e.slot_pickers()).expect("the slot form has pickers");
    for (picker, value) in [(&fg, 0x112233ff_u32), (&bg, 0x445566ff), (&light_bg, 0x778899ff)] {
        cx.update_window(f.w, |_, window, cx| picker.update(cx, |p, cx| p.select_color(rgba(value).into(), window, cx))).unwrap();
    }
    cx.run_until_parked();
    let slot = cx.update(|cx| cx.global::<ProfileStore>().get(&name).hit_palette[0].clone());
    assert_eq!((slot.fg.0.as_str(), slot.bg.0.as_str()), ("#112233", "#445566"));
    assert_eq!(slot.light_bg.map(|c| c.0).as_deref(), Some("#778899"));
}

#[gpui_kit::test]
fn the_pickers_follow_the_field_and_open_on_the_sliders(cx: &mut TestAppContext) {
    use gpui_kit::rgba;
    use rogg_core::profile::derive_light;
    use rogg_ui::theme::Color;
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-duplicate");
    cx.update_window(f.w, |_, window, cx| f.editor.update(cx, |e, cx| e.select_rule(Some(0), window, cx))).unwrap();
    let [color, light] = f.editor.read_with(cx, |e, _| e.rule_pickers()).unwrap();
    assert_eq!(color.read_with(cx, |p, _| p.active_tab()), 1, "pickers open on the sliders, not the kit palette grid");
    let shown = |p: &Entity<gpui_kit::component::color_picker::ColorPickerState>, cx: &mut TestAppContext| p.read_with(cx, |p, _| p.value().map(Color::from_hsla));

    // Typing a hex into the colour field moves the picker, and the derived light variant follows.
    let (_, color_input, light_input) = f.editor.read_with(cx, |e, _| e.rule_inputs()).unwrap();
    type_into(&f, cx, &color_input, "#336699");
    cx.run_until_parked();
    assert_eq!(shown(&color, cx), Some(Color(0x336699ff)));
    assert_eq!(shown(&light, cx), Some(Color(derive_light(0x336699ff, false))), "the light picker shows the derived variant");

    // An override typed into the light field moves the light picker.
    type_into(&f, cx, &light_input, "#445566");
    cx.run_until_parked();
    assert_eq!(shown(&light, cx), Some(Color(0x445566ff)));

    // A token swatch moves the colour picker to the token colour and clears the override.
    common::click(cx, f.w, "rule-swatch-hit-2");
    cx.run_until_parked();
    let token = cx.update(|cx| {
        let p = rogg_ui::theme::ActiveRoggTheme::rogg(cx).palette;
        rogg_app::profiles::resolve_color(&rogg_core::profile::ColorSpec::token("hit-2"), None, false, &p, rogg_ui::theme::Appearance::Dark)
    });
    assert_eq!(shown(&color, cx), Some(token));
    assert_eq!(shown(&light, cx), Some(token), "a token has no light variant of its own");

    // A pick through the picker is not undone by the echo.
    cx.update_window(f.w, |_, window, cx| color.update(cx, |p, cx| p.select_color(rgba(0x112233ff).into(), window, cx))).unwrap();
    cx.run_until_parked();
    assert_eq!(shown(&color, cx), Some(Color(0x112233ff)));

    cx.update_window(f.w, |_, window, cx| f.editor.update(cx, |e, cx| e.select_slot(Some(0), window, cx))).unwrap();
    let [fg, _, light_fg, _] = f.editor.read_with(cx, |e, _| e.slot_pickers()).unwrap();
    let [fg_input, ..] = f.editor.read_with(cx, |e, _| e.slot_inputs()).unwrap();
    type_into(&f, cx, &fg_input, "#778899");
    cx.run_until_parked();
    assert_eq!(shown(&fg, cx), Some(Color(0x778899ff)));
    assert_eq!(shown(&light_fg, cx), Some(Color(derive_light(0x778899ff, false))));
}

/// Drags `picker`'s lightness slider through `values` as the mouse would (the popover open, a change per move) and
/// returns the rule colour the store held after each move.
fn drag_lightness(
    f: &Fixture,
    cx: &mut TestAppContext,
    picker: &Entity<gpui_kit::component::color_picker::ColorPickerState>,
    name: &str,
    values: &[f32],
) -> Vec<String> {
    use gpui_kit::component::slider::SliderEvent;
    picker.update(cx, |p, cx| p.set_open(true, cx));
    cx.run_until_parked();
    let slider = picker.read_with(cx, |p, _| p.sliders().lightness().clone());
    let mut seen = Vec::new();
    for v in values {
        let v = *v;
        cx.update_window(f.w, |_, window, cx| {
            slider.update(cx, |s, cx| {
                s.set_value(v, window, cx);
                cx.emit(SliderEvent::Change(s.value()));
            })
        })
        .unwrap();
        cx.run_until_parked();
        seen.push(cx.update(|cx| cx.global::<ProfileStore>().get(name).rules[0].color.0.clone()));
    }
    seen
}

fn release(cx: &mut TestAppContext, picker: &Entity<gpui_kit::component::color_picker::ColorPickerState>) {
    use gpui_kit::component::slider::SliderEvent;
    let slider = picker.read_with(cx, |p, _| p.sliders().lightness().clone());
    slider.update(cx, |s, cx| cx.emit(SliderEvent::Release(s.value())));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_slider_drag_restyles_live_and_saves_when_it_ends(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-duplicate");
    let name = f.editor.read_with(cx, |e, _| e.profile_name().to_string());
    let file = f.dir.path().join(format!("profiles/{name}.toml"));
    let on_disk = |cx: &mut TestAppContext| {
        cx.run_until_parked();
        std::fs::read_to_string(&file).unwrap()
    };
    let saves = |cx: &mut TestAppContext| cx.update(|cx| cx.global::<ProfileStore>().saves());
    select_rule(&f, cx, 0);
    let [color, _] = f.editor.read_with(cx, |e, _| e.rule_pickers()).unwrap();

    // A drag: every move restyles, nothing is written until the slider is released, then the final colour once.
    let (disk, before) = (on_disk(cx), saves(cx));
    let revision = cx.update(|cx| ProfileStore::revision(cx));
    let seen = drag_lightness(&f, cx, &color, &name, &[0.2, 0.4, 0.6]);
    assert!(seen[0] != seen[1] && seen[1] != seen[2], "the live profile follows the drag: {seen:?}");
    assert!(cx.update(|cx| ProfileStore::revision(cx)) >= revision + 3, "every move restyles the tabs");
    assert_eq!(saves(cx), before, "nothing is written while dragging");
    assert_eq!(on_disk(cx), disk);
    release(cx, &color);
    assert_eq!(saves(cx), before + 1, "the release writes the profile once");
    assert!(on_disk(cx).contains(&seen[2]), "the final colour is on disk");

    // Closing the popover writes a colour that was not released (a pick from the grid, say) …
    let seen = drag_lightness(&f, cx, &color, &name, &[0.3]);
    color.update(cx, |p, cx| p.set_open(false, cx));
    assert!(on_disk(cx).contains(&seen[0]), "closing the popover saves");
    // … and so do switching profiles and closing the editor.
    let seen = drag_lightness(&f, cx, &color, &name, &[0.35]);
    f.editor.update(cx, |e, cx| e.select_profile("Default", cx));
    assert!(on_disk(cx).contains(&seen[0]), "switching profiles saves");
    f.editor.update(cx, |e, cx| e.select_profile(&name, cx));
    select_rule(&f, cx, 0);
    let [color, _] = f.editor.read_with(cx, |e, _| e.rule_pickers()).unwrap();
    let seen = drag_lightness(&f, cx, &color, &name, &[0.45]);
    assert!(!on_disk(cx).contains(&seen[0]) && color.read_with(cx, |p, _| p.is_open()));
    f.ws.update(cx, |ws, cx| ws.close_dialog(cx)); // Escape
    assert!(f.ws.read_with(cx, |ws, _| ws.profile_editor().is_none()));
    assert!(on_disk(cx).contains(&seen[0]), "closing the editor saves");

    // Typed colours still save at once.
    common::click(cx, f.w, "profile");
    let editor = f.ws.read_with(cx, |ws, _| ws.profile_editor()).unwrap();
    editor.update(cx, |e, cx| e.select_profile(&name, cx));
    cx.update_window(f.w, |_, window, cx| editor.update(cx, |e, cx| e.select_rule(Some(0), window, cx))).unwrap();
    let (_, color_input, _) = editor.read_with(cx, |e, _| e.rule_inputs()).unwrap();
    type_into(&f, cx, &color_input, "#123456");
    assert!(on_disk(cx).contains("#123456"));
}

#[gpui_kit::test]
fn every_picker_features_rogg_colours(cx: &mut TestAppContext) {
    use rogg_core::profile::ColorSpec;
    use rogg_ui::theme::{Appearance, Palette};
    let _f = open_editor(cx);
    let ([rule, rule_light], slots) = cx.update(|cx| ProfileEditor::featured_colors(cx));
    let (dark, light) = (cx.update(|cx| rogg_ui::theme::ActiveRoggTheme::rogg(cx).palette), Palette::light());
    let token = |t: &str, p: &Palette| rogg_app::profiles::resolve_color(&ColorSpec::token(t), None, false, p, Appearance::Dark);
    assert_eq!(rule.len(), 10);
    assert_eq!(rule[4], token("error", &dark), "the rule colour features the token swatches");
    assert_eq!(rule_light[4], token("error", &light), "the light override features them as the light theme shows them");
    let hits = |p: &Palette, bg: bool| p.hits.iter().map(|h| if bg { h.bg } else { h.fg }).collect::<Vec<_>>();
    assert_eq!(slots, [hits(&dark, false), hits(&dark, true), hits(&light, false), hits(&light, true)], "slots feature the palette's hit colours");
}

#[gpui_kit::test]
fn the_rule_forms_colour_fields_keep_their_width_inside_the_dialog(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    select_rule(&f, cx, 0);
    let dialog = common::bounds(cx, f.w, "dialog");
    for id in ["rule-color", "rule-light"] {
        let field = common::bounds(cx, f.w, id);
        assert_eq!(field.size.width, gpui_kit::px(96.), "{id} keeps its width: {field:?}");
        assert!(field.right() <= dialog.right(), "{id} is inside the dialog: {field:?} in {dialog:?}");
    }
    // The other rows still fit inside the frame's padding.
    for id in ["rule-pattern", "rule-case", "rule-effect-bold", "rule-custom", "rule-delete"] {
        let b = common::bounds(cx, f.w, id);
        assert!(b.right() <= dialog.right() - gpui_kit::px(22.), "{id} is inside the dialog padding: {b:?} in {dialog:?}");
    }
    assert_eq!(common::bounds(cx, f.w, "rule-pattern").size.width, gpui_kit::px(240.));
}

#[gpui_kit::test]
fn the_slot_forms_colour_fields_keep_their_width_inside_the_dialog(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "palette-slot-0");
    let dialog = common::bounds(cx, f.w, "dialog");
    for id in ["slot-fg", "slot-bg", "slot-light-fg", "slot-light-bg"] {
        let field = common::bounds(cx, f.w, id);
        assert_eq!(field.size.width, gpui_kit::px(96.), "{id} keeps its width: {field:?}");
        assert!(field.right() <= dialog.right(), "{id} is inside the dialog: {field:?} in {dialog:?}");
    }
    for id in ["slot-bg-custom", "slot-light-bg-custom"] {
        let b = common::bounds(cx, f.w, id);
        assert!(b.right() <= dialog.right() - gpui_kit::px(22.), "{id} is inside the dialog padding: {b:?} in {dialog:?}");
    }
}
