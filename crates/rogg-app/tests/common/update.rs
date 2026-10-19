//! Updater fixtures shared by the updater and update-UI tests: an installed copy and work dir in a temp folder,
//! release archives and manifests, a `TestServer` publishing them, and helpers that poll an `Updater` until it settles.
use super::http::{Route, TestServer};
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
pub const ASSET: &str = "rogg-linux-x86_64.tar.gz";
pub const ASSET_PATH: &str = "/rogg-linux-x86_64.tar.gz";
pub const NEW: &[u8] = b"new rogg build 14";

pub type Calls = Arc<Mutex<Vec<(PathBuf, PathBuf)>>>;

/// An updater that can never reach the network or install anything: a local build (no check ever fetches), an
/// unsupported install, an unroutable base, and an elevator that fails the test if it is called. `common::init`
/// installs it before the app's globals, so no test talks to GitHub (R10: CI builds the tests with a build number).
pub fn offline_config(work_dir: PathBuf) -> UpdaterConfig {
    UpdaterConfig {
        elevator: Arc::new(|_: &Path, _: &Path| panic!("the offline test updater never elevates")),
        ..UpdaterConfig::disabled(work_dir)
    }
}

/// A temp folder holding `bin/rogg` (the installed copy) and `cache/update` (the work dir).
pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub calls: Calls,
}

impl Fixture {
    pub fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("bin")).unwrap();
        fs::write(dir.path().join("bin/rogg"), b"old").unwrap();
        Fixture { dir, calls: Arc::default() }
    }

    pub fn target(&self) -> PathBuf {
        self.dir.path().join("bin/rogg")
    }

    pub fn work_dir(&self) -> PathBuf {
        self.dir.path().join("cache/update")
    }

    /// A config whose elevator records its calls and returns `result`.
    pub fn config_with(&self, server: &TestServer, running: Option<u64>, result: Result<(), Failure>) -> UpdaterConfig {
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

    pub fn config(&self, server: &TestServer, running: Option<u64>) -> UpdaterConfig {
        self.config_with(server, running, Ok(()))
    }

    pub fn work_dir_is_empty(&self) -> bool {
        fs::read_dir(self.work_dir()).map(|mut d| d.next().is_none()).unwrap_or(true)
    }
}

/// A `.tar.gz` with `rogg` holding `bytes`.
pub fn archive(bytes: &[u8]) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(ASSET);
    write_archive(&path, BundleKind::TarGz, &[("rogg", bytes)]).unwrap();
    fs::read(path).unwrap()
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Build `build`'s manifest, listing `asset` (name, sha256, size) when given.
pub fn manifest(build: u64, asset: Option<(&str, &str, usize)>) -> Vec<u8> {
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
pub fn release_server(archive: &[u8], route: Route) -> TestServer {
    let json = manifest(14, Some((ASSET, &sha256(archive), archive.len())));
    TestServer::start(vec![("/manifest.json", Route::Bytes(200, json)), (ASSET_PATH, route)])
}

/// Polls until no job runs (up to 10 s), calling `each` with the state after every poll.
pub fn run_with(updater: &mut Updater, mut each: impl FnMut(&UpdateState)) {
    let start = Instant::now();
    while updater.poll() {
        each(updater.state());
        assert!(start.elapsed() < Duration::from_secs(10), "the job didn't finish: {:?}", updater.state());
        std::thread::sleep(Duration::from_millis(5));
    }
    each(updater.state());
}

pub fn run(updater: &mut Updater) {
    run_with(updater, |_| {});
}

/// Polls until `done` holds for the state (up to 10 s).
pub fn run_until(updater: &mut Updater, mut done: impl FnMut(&UpdateState) -> bool) {
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
pub fn available(fixture: &Fixture, server: &TestServer) -> Updater {
    available_with(fixture.config(server, Some(11)))
}

pub fn available_with(config: UpdaterConfig) -> Updater {
    let mut updater = Updater::new(config);
    assert!(updater.check(false));
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::Available(_)), "{:?}", updater.state());
    updater
}

pub fn failure(state: &UpdateState) -> Option<&Failure> {
    match state {
        UpdateState::Failed { failure, .. } => Some(failure),
        _ => None,
    }
}

pub fn badge(label: &str, tooltip: &str) -> Option<(SharedString, SharedString)> {
    Some((SharedString::from(label.to_string()), SharedString::from(tooltip.to_string())))
}

/// Pseudo-random bytes, so gzip can't shrink the archive below the progress step.
pub fn noise(n: usize) -> Vec<u8> {
    let mut x: u32 = 0x1234_5678;
    (0..n)
        .map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (x >> 24) as u8
        })
        .collect()
}

#[cfg(unix)]
pub fn read_only(dir: &Path, read_only: bool) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir, fs::Permissions::from_mode(if read_only { 0o555 } else { 0o755 })).unwrap();
}
