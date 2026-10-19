// The elevation tests need a read-only folder, so they are Unix-only and some imports go unused elsewhere.
#![cfg_attr(not(unix), allow(unused_imports))]

mod common;

use common::http::{Gate, Route, TestServer};
use common::update::*;
use rogg_app::updater::{ApplyUpdate, Failure, UpdateState, Updater};
use rogg_core::update::Install;
use rogg_genlog::bundle::{BundleKind, write_archive};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

/// A connection that drops mid-download is a network problem, not a damaged download.
#[test]
fn a_dropped_connection_is_reported_as_offline() {
    let fixture = Fixture::new();
    let data = archive(&noise(200_000));
    let server = release_server(&data, Route::Short(200, data.clone(), data.len() / 2));
    let mut updater = available(&fixture, &server);
    updater.start_update();
    run(&mut updater);
    assert_eq!(failure(updater.state()), Some(&Failure::Offline), "{:?}", updater.state());
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
    // The helper gets the verified download and its checksum, never a tree unpacked by this (user-owned) process.
    let expected = Elevated {
        request: ApplyUpdate { archive: fixture.work_dir().join("14").join(ASSET), sha256: sha256(&data), target: fixture.target() },
        archive: Some(data.clone()),
        staged: false,
    };
    assert_eq!(*fixture.calls.lock().unwrap(), vec![expected]);
    assert!(fixture.work_dir_is_empty());
}

/// A download that doesn't match its checksum is refused before any admin prompt.
#[cfg(unix)]
#[test]
fn a_bad_checksum_never_elevates() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let json = manifest(14, Some((ASSET, &"0".repeat(64), data.len())));
    let server = TestServer::start(vec![("/manifest.json", Route::Bytes(200, json)), (ASSET_PATH, Route::Bytes(200, data))]);
    let mut updater = available(&fixture, &server);
    let bin = fixture.dir.path().join("bin");
    read_only(&bin, true);
    updater.start_update();
    run(&mut updater);
    read_only(&bin, false);
    assert_eq!(failure(updater.state()), Some(&Failure::Checksum));
    assert!(fixture.calls.lock().unwrap().is_empty());
    assert_eq!(fs::read(fixture.target()).unwrap(), b"old");
    assert!(fixture.work_dir_is_empty());
}

#[cfg(unix)]
#[test]
fn the_badge_stays_while_installing() {
    let fixture = Fixture::new();
    let data = archive(NEW);
    let server = release_server(&data, Route::Bytes(200, data.clone()));
    let mut config = fixture.config(&server, Some(11));
    // The elevator holds the job in Installing until the test lets it go.
    let (release, held) = std::sync::mpsc::channel::<()>();
    let held = Arc::new(Mutex::new(held));
    config.elevator = Arc::new(move |_: &ApplyUpdate| {
        let _ = held.lock().unwrap().recv_timeout(Duration::from_secs(10));
        Ok(())
    });
    let mut updater = available_with(config);
    let bin = fixture.dir.path().join("bin");
    read_only(&bin, true);
    updater.start_update();
    run_until(&mut updater, |state| matches!(state, UpdateState::Installing(_)));
    let installing = updater.badge();
    release.send(()).unwrap();
    run(&mut updater);
    read_only(&bin, false);
    assert_eq!(installing, badge("Update", "rogg 0.1.0 build 14 is available"));
    assert!(matches!(updater.state(), UpdateState::Installed(_)), "{:?}", updater.state());
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
    // The badge stays during the download, so a closed dialog can be reopened (R6).
    assert_eq!(updater.badge(), badge("Update", "rogg 0.1.0 build 14 is available"));
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
fn a_manual_check_adopts_a_running_silent_one() {
    let fixture = Fixture::new();
    let gate = Gate::default();
    let server = TestServer::start(vec![("/manifest.json", Route::Gated(200, manifest(11, None), 0, gate.clone()))]);
    let mut updater = Updater::new(fixture.config(&server, Some(11)));
    assert!(updater.check(false));
    assert!(updater.check(true), "the running check now reports its outcome");
    assert!(matches!(updater.state(), UpdateState::Checking { manual: true }), "{:?}", updater.state());
    gate.open();
    run(&mut updater);
    assert!(matches!(updater.state(), UpdateState::UpToDate), "{:?}", updater.state());
    assert_eq!(server.hits("/manifest.json"), 1);
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
