//! The update wiring in the window: the startup check, the title-bar badge, the Update dialog and its buttons, and
//! Check for Updates….
mod common;

use common::http::{Gate, Route, TestServer};
use common::update::*;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};
use rogg_app::actions::CheckForUpdates;
use rogg_app::args::Args;
use rogg_app::settings::SettingsStore;
use rogg_app::updater::{UpdateState, Updater, UpdaterConfig};
use rogg_app::workspace::Workspace;
use std::fs;
use std::time::Duration;

/// The startup check runs this long after the window opens.
const STARTUP: Duration = Duration::from_secs(3);

/// A window whose updater uses `config` (replacing `common::setup`'s offline one); `check_updates` is the setting.
fn start(cx: &mut TestAppContext, config: UpdaterConfig, check_updates: bool) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>) {
    let dir = common::setup(cx);
    cx.update(|cx| {
        cx.set_global(Updater::new(config));
        SettingsStore::update(cx, |s| s.behavior.check_updates = check_updates);
    });
    let (w, ws) = common::open_workspace(cx, Args::default());
    (dir, w, ws)
}

fn state(cx: &mut TestAppContext) -> UpdateState {
    cx.read(|cx| cx.global::<Updater>().state().clone())
}

fn badge_label(cx: &mut TestAppContext) -> Option<String> {
    cx.read(|cx| cx.global::<Updater>().badge().map(|(label, _)| label.to_string()))
}

/// The open Update dialog's lines (and progress label) and button ids; None when it isn't open.
fn dialog(cx: &mut TestAppContext, ws: &Entity<Workspace>) -> Option<(Vec<String>, Vec<&'static str>)> {
    ws.read_with(cx, |ws, cx| ws.update_dialog_content(cx))
}

fn lines(cx: &mut TestAppContext, ws: &Entity<Workspace>) -> Vec<String> {
    dialog(cx, ws).expect("the Update dialog is open").0
}

fn buttons(cx: &mut TestAppContext, ws: &Entity<Workspace>) -> Vec<&'static str> {
    dialog(cx, ws).expect("the Update dialog is open").1
}

/// Runs the startup check and waits for the badge.
fn startup_badge(cx: &mut TestAppContext, w: AnyWindowHandle) {
    cx.executor().advance_clock(STARTUP);
    common::wait_until(cx, |cx| common::exists(cx, w, "update-badge"));
}

fn check_from_the_menu(cx: &mut TestAppContext, w: AnyWindowHandle) {
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(CheckForUpdates), cx)).unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn the_startup_check_shows_the_badge(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let (_dir, w, ws) = start(cx, fixture.config(&server, Some(11)), true);
    assert!(!common::exists(cx, w, "update-badge"));
    startup_badge(cx, w);
    assert_eq!(badge_label(cx).as_deref(), Some("Update"));
    assert!(dialog(cx, &ws).is_none(), "the startup check opens no dialog");
    common::click(cx, w, "update-badge");
    assert!(common::exists(cx, w, "update-body"));
    assert_eq!(lines(cx, &ws), ["rogg 0.1.0 · build 14 · 2026-10-18 · a1b2c3d", "You have build 11."]);
    assert_eq!(buttons(cx, &ws), ["update-now", "update-notes", "update-later"]);
    common::click(cx, w, "update-later");
    assert!(dialog(cx, &ws).is_none());
    assert!(common::exists(cx, w, "update-badge"), "Later keeps the badge");
}

#[gpui_kit::test]
fn the_setting_turns_the_startup_check_off(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let (_dir, w, _ws) = start(cx, fixture.config(&server, Some(11)), false);
    cx.executor().advance_clock(Duration::from_secs(5));
    cx.run_until_parked();
    assert!(matches!(state(cx), UpdateState::Idle), "{:?}", state(cx));
    assert!(!common::exists(cx, w, "update-badge"));
    assert_eq!(server.hits("/manifest.json"), 0);
}

#[gpui_kit::test]
fn update_then_restart_badge(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let (_dir, w, ws) = start(cx, fixture.config(&server, Some(11)), true);
    startup_badge(cx, w);
    common::click(cx, w, "update-badge");
    common::click(cx, w, "update-now");
    common::wait_until(cx, |cx| badge_label(cx).as_deref() == Some("Restart to update"));
    assert!(common::exists(cx, w, "update-badge"));
    assert_eq!(lines(cx, &ws), ["Build 14 is installed. Restart rogg to use it."]);
    assert_eq!(buttons(cx, &ws), ["update-close"]);
    assert_eq!(fs::read(fixture.target()).unwrap(), NEW);
    common::click(cx, w, "update-close");
    assert!(dialog(cx, &ws).is_none());
    // A manual check after the install keeps Installed, and the dialog says so.
    check_from_the_menu(cx, w);
    assert_eq!(lines(cx, &ws), ["Build 14 is installed. Restart rogg to use it."], "while checking");
    common::wait_until(cx, |_| server.hits("/manifest.json") == 2);
    assert_eq!(lines(cx, &ws), ["Build 14 is installed. Restart rogg to use it."]);
}

#[gpui_kit::test]
fn closing_the_dialog_keeps_the_download_running(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let gate = Gate::default();
    let server = release_server(&data, Route::Gated(200, data.clone(), 10, gate.clone()));
    let (_dir, w, ws) = start(cx, fixture.config(&server, Some(11)), true);
    startup_badge(cx, w);
    common::click(cx, w, "update-badge");
    common::click(cx, w, "update-now");
    // Held at the gate: the asset was requested and its first bytes sent.
    common::wait_until(cx, |_| server.hits(ASSET_PATH) == 1);
    assert!(matches!(state(cx), UpdateState::Downloading { .. }), "{:?}", state(cx));
    assert_eq!(buttons(cx, &ws), ["update-cancel"]);
    assert!(common::exists(cx, w, "update-progress"));
    common::press(cx, w, "escape");
    assert!(dialog(cx, &ws).is_none(), "Escape closes the dialog");
    assert!(common::exists(cx, w, "update-badge"), "the badge reopens it");
    gate.open();
    common::wait_until(cx, |cx| badge_label(cx).as_deref() == Some("Restart to update"));
    assert!(dialog(cx, &ws).is_none(), "the finished job opens nothing");
    assert_eq!(fs::read(fixture.target()).unwrap(), NEW);
}

#[gpui_kit::test]
fn cancel_returns_to_the_offer(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Stall);
    let (_dir, w, ws) = start(cx, fixture.config(&server, Some(11)), true);
    startup_badge(cx, w);
    common::click(cx, w, "update-badge");
    common::click(cx, w, "update-now");
    common::wait_until(cx, |_| server.hits(ASSET_PATH) == 1);
    common::click(cx, w, "update-cancel");
    common::wait_until(cx, |cx| matches!(state(cx), UpdateState::Available(_)));
    assert_eq!(buttons(cx, &ws), ["update-now", "update-notes", "update-later"]);
    assert_eq!(fs::read(fixture.target()).unwrap(), b"old");
}

#[gpui_kit::test]
fn release_notes_opens_the_release_page(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let (_dir, w, ws) = start(cx, fixture.config(&server, Some(11)), true);
    startup_badge(cx, w);
    common::click(cx, w, "update-badge");
    common::click(cx, w, "update-notes");
    assert_eq!(
        ws.read_with(cx, |ws, _| ws.last_opened_url().map(str::to_string)).as_deref(),
        Some("https://github.com/rogg-log/rogg/releases/tag/v0.1.0-build.14")
    );
    assert!(dialog(cx, &ws).is_some(), "the dialog stays open");
}

#[gpui_kit::test]
fn check_for_updates_from_the_menu_when_up_to_date(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let gate = Gate::default();
    let json = manifest(11, None);
    let server = TestServer::start(vec![("/manifest.json", Route::Gated(200, json, 0, gate.clone()))]);
    let (_dir, w, ws) = start(cx, fixture.config(&server, Some(11)), true);
    check_from_the_menu(cx, w);
    assert_eq!(lines(cx, &ws), ["Checking for updates…"]);
    assert_eq!(buttons(cx, &ws), ["update-close"]);
    gate.open();
    common::wait_until(cx, |cx| matches!(state(cx), UpdateState::UpToDate));
    assert_eq!(lines(cx, &ws), ["rogg is up to date (build 11)."]);
    assert_eq!(buttons(cx, &ws), ["update-close"]);
    assert!(!common::exists(cx, w, "update-badge"));
    // The startup check doesn't run over a manual one's result.
    cx.executor().advance_clock(STARTUP);
    cx.run_until_parked();
    assert!(matches!(state(cx), UpdateState::UpToDate), "{:?}", state(cx));
    assert_eq!(server.hits("/manifest.json"), 1);
}

#[gpui_kit::test]
fn a_local_build_says_so(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let server = TestServer::start(vec![]);
    let (_dir, w, ws) = start(cx, fixture.config(&server, None), true);
    cx.executor().advance_clock(STARTUP);
    cx.run_until_parked();
    assert!(!common::exists(cx, w, "update-badge"));
    check_from_the_menu(cx, w);
    assert_eq!(lines(cx, &ws), ["This is a local build. Updates apply to release builds only."]);
    assert_eq!(buttons(cx, &ws), ["update-close"]);
    assert_eq!(server.hits("/manifest.json"), 0);
}

#[gpui_kit::test]
fn a_failed_manual_check_retries_the_check(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let server = TestServer::start(vec![("/manifest.json", Route::Bytes(500, b"oops".to_vec()))]);
    let (_dir, w, ws) = start(cx, fixture.config(&server, Some(11)), false);
    check_from_the_menu(cx, w);
    common::wait_until(cx, |cx| matches!(state(cx), UpdateState::Failed { .. }));
    assert_eq!(lines(cx, &ws), ["GitHub returned 500."]);
    assert_eq!(buttons(cx, &ws), ["update-retry", "update-close"], "no release to show notes for");
    common::click(cx, w, "update-retry");
    common::wait_until(cx, |cx| matches!(state(cx), UpdateState::Failed { .. }) && server.hits("/manifest.json") == 2);
}

#[cfg(unix)]
#[gpui_kit::test]
fn copy_install_command_on_missing_pkexec(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let (_dir, w, ws) = start(cx, fixture.config_with(&server, Some(11), Err(rogg_app::updater::Failure::NoPkexec)), true);
    startup_badge(cx, w);
    common::click(cx, w, "update-badge");
    let bin = fixture.dir.path().join("bin");
    read_only(&bin, true);
    common::click(cx, w, "update-now");
    common::wait_until(cx, |cx| matches!(state(cx), UpdateState::Failed { .. }));
    assert_eq!(lines(cx, &ws), ["rogg can't ask for admin rights here (pkexec not found)."]);
    assert_eq!(buttons(cx, &ws), ["update-retry", "update-notes", "update-copy-command", "update-close"]);
    common::click(cx, w, "update-copy-command");
    let copied = cx.read_from_clipboard().and_then(|c| c.text());
    assert_eq!(copied.as_deref(), Some("curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash"));
    // Retry runs the update again (R11), which fails the same way.
    common::click(cx, w, "update-retry");
    common::wait_until(cx, |cx| matches!(state(cx), UpdateState::Failed { .. }) && fixture.calls.lock().unwrap().len() == 2);
    read_only(&bin, false);
    assert_eq!(server.hits(ASSET_PATH), 2);
}

/// R10: CI builds the tests with a build number, so a test app on the default config would check GitHub. The shared
/// setup installs an updater that can't.
#[gpui_kit::test]
fn the_test_setup_never_reaches_the_network(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.read(|cx| {
        let config = cx.global::<Updater>().config();
        assert_eq!(config.running_build, None);
        assert_ne!(config.base.trim_end_matches('/'), rogg_app::updater::http::DEFAULT_BASE);
    });
}
