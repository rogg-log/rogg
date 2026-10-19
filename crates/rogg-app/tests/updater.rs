mod common;

use common::http::{Gate, Route, TestServer};
use gpui_kit::SharedString;
use rogg_app::updater::{Elevator, Failure, UpdateState, Updater, UpdaterConfig};
use rogg_core::platform::Os;
use rogg_core::update::Install;
use rogg_genlog::bundle::{BundleKind, write_archive};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The tests pin the platform so the asset name doesn't depend on the host.
const ASSET: &str = "rogg-linux-x86_64.tar.gz";
const ASSET_PATH: &str = "/rogg-linux-x86_64.tar.gz";
const NEW: &[u8] = b"new rogg build 14";

type Calls = Arc<Mutex<Vec<(PathBuf, PathBuf)>>>;

/// A temp folder holding `bin/rogg` (the installed copy) and `cache/update` (the work dir).
struct Fixture {
    dir: tempfile::TempDir,
    calls: Calls,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("bin")).unwrap();
        fs::write(dir.path().join("bin/rogg"), b"old").unwrap();
        Fixture { dir, calls: Arc::default() }
    }

    fn target(&self) -> PathBuf {
        self.dir.path().join("bin/rogg")
    }

    fn work_dir(&self) -> PathBuf {
        self.dir.path().join("cache/update")
    }

    /// A config whose elevator records its calls and returns `result`.
    fn config_with(&self, server: &TestServer, running: Option<u64>, result: Result<(), Failure>) -> UpdaterConfig {
        let calls = self.calls.clone();
        let elevator: Elevator = Arc::new(move |staged: &Path, target: &Path| {
            calls.lock().unwrap().push((staged.to_path_buf(), target.to_path_buf()));
            result.clone()
        });
        UpdaterConfig {
            base: server.base(),
            running_build: running,
            install: Install::Binary { path: self.target() },
            os: Os::Linux,
            arch: "x86_64".into(),
            work_dir: self.work_dir(),
            elevator,
        }
    }

    fn config(&self, server: &TestServer, running: Option<u64>) -> UpdaterConfig {
        self.config_with(server, running, Ok(()))
    }

    fn work_dir_is_empty(&self) -> bool {
        fs::read_dir(self.work_dir()).map(|mut d| d.next().is_none()).unwrap_or(true)
    }
}

/// A `.tar.gz` with `rogg` holding `bytes`.
fn archive(bytes: &[u8]) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(ASSET);
    write_archive(&path, BundleKind::TarGz, &[("rogg", bytes)]).unwrap();
    fs::read(path).unwrap()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Build 14's manifest, listing `asset` (name, sha256, size) when given.
fn manifest(build: u64, asset: Option<(&str, &str, usize)>) -> Vec<u8> {
    let assets = match asset {
        Some((name, sha, size)) => format!(r#"[{{"name":"{name}","os":"linux","arch":"x86_64","sha256":"{sha}","size":{size}}}]"#),
        None => "[]".into(),
    };
    format!(
        r#"{{"version":"0.1.0","build":{build},"commit":"a1b2c3d4e5f60718293a4b5c6d7e8f9012345678","date":"2026-10-18","channel":"stable","assets":{assets}}}"#
    )
    .into_bytes()
}

/// A server publishing build 14 with `archive` as the platform's asset (served by `route`).
fn release_server(archive: &[u8], route: Route) -> TestServer {
    let json = manifest(14, Some((ASSET, &sha256(archive), archive.len())));
    TestServer::start(vec![("/manifest.json", Route::Bytes(200, json)), (ASSET_PATH, route)])
}

/// Polls until no job runs (up to 10 s), calling `each` with the state after every poll.
fn run_with(updater: &mut Updater, mut each: impl FnMut(&UpdateState)) {
    let start = Instant::now();
    while updater.poll() {
        each(updater.state());
        assert!(start.elapsed() < Duration::from_secs(10), "the job didn't finish: {:?}", updater.state());
        std::thread::sleep(Duration::from_millis(5));
    }
    each(updater.state());
}

fn run(updater: &mut Updater) {
    run_with(updater, |_| {});
}

/// Polls until `done` holds for the state (up to 10 s).
fn run_until(updater: &mut Updater, mut done: impl FnMut(&UpdateState) -> bool) {
    let start = Instant::now();
    loop {
        updater.poll();
        if done(updater.state()) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "never reached the state: {:?}", updater.state());
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// An updater that has found build 14 (running 11).
fn available(fixture: &Fixture, server: &TestServer) -> Updater {
    available_with(fixture.config(server, Some(11)))
}

fn available_with(config: UpdaterConfig) -> Updater {
    let mut updater = Updater::new(config);
    assert!(updater.check(false));
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Available(_)), "{:?}", updater.state());
    updater
}

fn failure(state: &UpdateState) -> Option<&Failure> {
    match state {
        UpdateState::Failed { failure, .. } => Some(failure),
        _ => None,
    }
}

fn badge(label: &str, tooltip: &str) -> Option<(SharedString, SharedString)> {
    Some((SharedString::from(label.to_string()), SharedString::from(tooltip.to_string())))
}

/// Pseudo-random bytes, so gzip can't shrink the archive below the progress step.
fn noise(n: usize) -> Vec<u8> {
    let mut x: u32 = 0x1234_5678;
    (0..n)
        .map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (x >> 24) as u8
        })
        .collect()
}

#[test]
fn a_newer_build_becomes_available() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let updater = available(&fixture, &server);
    match updater.state() {
        UpdateState::Available(release) => {
            assert_eq!(release.manifest.build, 14);
            assert_eq!(release.asset.name, ASSET);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(updater.badge(), badge("Update", "rogg 0.1.0 build 14 is available"));
}

#[test]
fn the_same_build_is_up_to_date() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let mut updater = Updater::new(fixture.config(&server, Some(14)));
    assert!(updater.check(true));
    assert!(matches!(updater.state(), UpdateState::Checking { manual: true }), "{:?}", updater.state());
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::UpToDate), "{:?}", updater.state());
    assert_eq!(updater.badge(), None);
}

#[test]
fn a_silent_up_to_date_check_stays_idle() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let mut updater = Updater::new(fixture.config(&server, Some(14)));
    updater.check(false);
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Idle), "{:?}", updater.state());
}

#[test]
fn a_local_build_never_checks() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let mut updater = Updater::new(fixture.config(&server, None));
    updater.check(false);
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Idle), "{:?}", updater.state());
    updater.check(true);
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::LocalBuild), "{:?}", updater.state());
    assert_eq!(server.hits("/manifest.json"), 0);
    assert_eq!(updater.badge(), None);
}

#[test]
fn startup_errors_are_silent_and_manual_ones_are_not() {
    let fixture = Fixture::new();
    let server = TestServer::start(vec![("/manifest.json", Route::Bytes(500, b"oops".to_vec()))]);
    let mut updater = Updater::new(fixture.config(&server, Some(11)));
    updater.check(false);
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Idle), "{:?}", updater.state());
    updater.check(true);
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Failed { release: None, failure: Failure::Http(500) }), "{:?}", updater.state());
    assert_eq!(server.hits("/manifest.json"), 2);
}

#[test]
fn a_bad_manifest_fails_a_manual_check() {
    let fixture = Fixture::new();
    let server = TestServer::start(vec![("/manifest.json", Route::Bytes(200, b"{not json".to_vec()))]);
    let mut updater = Updater::new(fixture.config(&server, Some(11)));
    updater.check(true);
    run(&mut updater);
    assert_eq!(failure(updater.state()), Some(&Failure::BadManifest));
}

#[test]
fn a_missing_asset_is_no_update() {
    let fixture = Fixture::new();
    let server = TestServer::start(vec![("/manifest.json", Route::Bytes(200, manifest(14, None)))]);
    let mut updater = Updater::new(fixture.config(&server, Some(11)));
    updater.check(false);
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Idle), "{:?}", updater.state());
    updater.check(true);
    run(&mut updater);
    assert_eq!(failure(updater.state()), Some(&Failure::NoAsset));
}

#[test]
fn an_unsupported_install_is_no_update() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let mut config = fixture.config(&server, Some(11));
    config.install = Install::Unsupported;
    let mut updater = Updater::new(config);
    updater.check(true);
    run(&mut updater);
    assert_eq!(failure(updater.state()), Some(&Failure::NoAsset));
}

#[test]
fn an_update_replaces_the_installed_binary() {
    let fixture = Fixture::new();
    let new = noise(400_000);
    let data = archive(&new);
    assert!(data.len() > 300_000, "the archive must span several progress steps");
    let gate = Gate::default();
    let server = release_server(&data, Route::Gated(200, data.clone(), 150_000, gate.clone()));
    let mut updater = available(&fixture, &server);

    let mut seen: Vec<u64> = Vec::new();
    let mut record = |state: &UpdateState| {
        if let UpdateState::Downloading { done, .. } = state
            && seen.last() != Some(done)
        {
            seen.push(*done);
        }
    };
    assert!(updater.start_update());
    record(updater.state());
    // Held at the gate: some progress arrives before the rest of the body is sent.
    run_until(&mut updater, |state| matches!(state, UpdateState::Downloading { done, .. } if *done > 0));
    record(updater.state());
    gate.open();
    run_with(&mut updater, &mut record);

    assert!(seen.len() >= 2, "{seen:?}");
    assert_eq!(seen[0], 0);
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "{seen:?}");
    match updater.state() {
        UpdateState::Installed(release) => assert_eq!(release.manifest.build, 14),
        other => panic!("{other:?}"),
    }
    assert_eq!(fs::read(fixture.target()).unwrap(), new);
    assert_eq!(updater.badge(), badge("Restart to update", "Build 14 is installed. Restart rogg to use it."));
    assert!(fixture.work_dir_is_empty());
    assert!(fixture.calls.lock().unwrap().is_empty(), "a writable folder needs no elevation");
}

#[test]
fn a_bad_checksum_changes_nothing() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let json = manifest(14, Some((ASSET, &"0".repeat(64), data.len())));
    let server = TestServer::start(vec![("/manifest.json", Route::Bytes(200, json)), (ASSET_PATH, Route::Bytes(200, data))]);
    let mut updater = available(&fixture, &server);
    assert!(updater.start_update());
    run(&mut updater);
    assert!(
        matches!(updater.state(), UpdateState::Failed { release: Some(_), failure: Failure::Checksum }),
        "{:?}",
        updater.state()
    );
    assert_eq!(fs::read(fixture.target()).unwrap(), b"old");
    assert!(fixture.work_dir_is_empty());
    assert_eq!(updater.badge(), None);
}

#[test]
fn an_archive_without_rogg_fails_to_unpack() {
    let fixture = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(ASSET);
    write_archive(&path, BundleKind::TarGz, &[("README", b"no binary")]).unwrap();
    let data = fs::read(path).unwrap();
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let mut updater = available(&fixture, &server);
    updater.start_update();
    run(&mut updater);
    assert_eq!(failure(updater.state()), Some(&Failure::Unpack));
    assert_eq!(fs::read(fixture.target()).unwrap(), b"old");
    assert!(fixture.work_dir_is_empty());
}

#[test]
fn a_failed_download_keeps_the_release() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(404, Vec::new()));
    let mut updater = available(&fixture, &server);
    updater.start_update();
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Failed { release: Some(_), failure: Failure::Http(404) }), "{:?}", updater.state());
    assert!(fixture.work_dir_is_empty());
}

#[test]
fn cancel_returns_to_available() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Stall);
    let mut updater = available(&fixture, &server);
    assert!(updater.start_update());
    assert!(matches!(updater.state(), UpdateState::Downloading { done: 0, .. }), "{:?}", updater.state());
    run_until(&mut updater, |_| server.hits(ASSET_PATH) == 1);
    updater.cancel();
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Available(_)), "{:?}", updater.state());
    assert_eq!(fs::read(fixture.target()).unwrap(), b"old");
    assert!(fixture.work_dir_is_empty());
}

#[cfg(unix)]
fn read_only(dir: &Path, read_only: bool) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir, fs::Permissions::from_mode(if read_only { 0o555 } else { 0o755 })).unwrap();
}

#[cfg(unix)]
#[test]
fn an_unwritable_folder_uses_the_elevator() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let mut updater = available(&fixture, &server);
    let bin = fixture.dir.path().join("bin");
    read_only(&bin, true);
    updater.start_update();
    run(&mut updater);
    read_only(&bin, false);
    assert!(matches!(updater.state(), UpdateState::Installed(_)), "{:?}", updater.state());
    let staged = fixture.work_dir().join("14/staged/rogg");
    assert_eq!(*fixture.calls.lock().unwrap(), vec![(staged, fixture.target())]);
    assert!(fixture.work_dir_is_empty());
}

#[cfg(unix)]
#[test]
fn a_declined_elevation_fails_cleanly() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let mut updater = available_with(fixture.config_with(&server, Some(11), Err(Failure::Declined)));
    let bin = fixture.dir.path().join("bin");
    read_only(&bin, true);
    updater.start_update();
    run(&mut updater);
    read_only(&bin, false);
    assert!(matches!(updater.state(), UpdateState::Failed { release: Some(_), failure: Failure::Declined }), "{:?}", updater.state());
    assert_eq!(Failure::Declined.message(), "Admin rights were declined.");
    assert_eq!(fs::read(fixture.target()).unwrap(), b"old");
    assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    assert!(fixture.work_dir_is_empty());
}

#[test]
fn checking_again_after_install_keeps_installed() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let mut updater = available(&fixture, &server);
    updater.start_update();
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Installed(_)), "{:?}", updater.state());

    assert!(updater.check(true));
    assert!(matches!(updater.state(), UpdateState::Installed(_)), "{:?}", updater.state());
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Installed(_)), "{:?}", updater.state());
    assert_eq!(server.hits("/manifest.json"), 2);
    assert_eq!(server.hits(ASSET_PATH), 1);
    // Nothing to download again either.
    assert!(!updater.start_update());
    assert_eq!(server.hits(ASSET_PATH), 1);
}

#[test]
fn no_second_job_while_one_runs() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Stall);
    let mut updater = available(&fixture, &server);
    assert!(updater.start_update());
    let before = format!("{:?}", updater.state());
    assert!(before.starts_with("Downloading"), "{before}");
    assert!(!updater.start_update());
    assert!(!updater.check(true));
    assert_eq!(format!("{:?}", updater.state()), before);
    assert!(updater.poll(), "the download is still running");
    assert_eq!(server.hits("/manifest.json"), 1);
    updater.cancel();
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Available(_)), "{:?}", updater.state());
}

#[test]
fn a_check_is_refused_while_one_runs() {
    let fixture = Fixture::new();
    let server = TestServer::start(vec![("/manifest.json", Route::Stall)]);
    let mut updater = Updater::new(fixture.config(&server, Some(11)));
    assert!(updater.check(true));
    assert!(!updater.check(true));
    assert!(!updater.start_update());
    assert!(matches!(updater.state(), UpdateState::Checking { manual: true }), "{:?}", updater.state());
}

#[test]
fn start_update_needs_a_release() {
    let fixture = Fixture::new();
    let server = TestServer::start(vec![]);
    let mut updater = Updater::new(fixture.config(&server, Some(11)));
    assert!(!updater.start_update());
    assert!(matches!(updater.state(), UpdateState::Idle));
    assert!(!updater.poll());
}

#[test]
fn failure_messages_match_the_spec() {
    assert_eq!(Failure::Offline.message(), "No network connection.");
    assert_eq!(Failure::TimedOut.message(), "The download timed out.");
    assert_eq!(Failure::Http(404).message(), "GitHub returned 404.");
    assert_eq!(Failure::BadManifest.message(), "The release information couldn't be read.");
    assert_eq!(Failure::NoAsset.message(), "No update is published for this platform.");
    assert_eq!(Failure::Checksum.message(), "The download is damaged (checksum mismatch). Nothing was changed.");
    assert_eq!(Failure::Unpack.message(), "The download couldn't be unpacked. Nothing was changed.");
    assert_eq!(Failure::Declined.message(), "Admin rights were declined.");
    assert_eq!(Failure::NoPkexec.message(), "rogg can't ask for admin rights here (pkexec not found).");
    assert_eq!(Failure::RolledBack.message(), "The update couldn't be installed. The current version was kept.");
    assert_eq!(
        Failure::RollbackFailed(PathBuf::from("/x/rogg.rogg-old")).message(),
        "The update failed. The previous version is at /x/rogg.rogg-old."
    );
}
