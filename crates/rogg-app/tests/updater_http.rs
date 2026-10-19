mod common;

use common::http::{Route, TestServer};
use rogg_app::updater::http::{NetError, download, download_with_timeout, fetch_manifest};
use rogg_core::CancelToken;
use std::time::Duration;

const MANIFEST: &str = r#"{
  "version":"0.1.0","build":14,"commit":"a1b2c3d4e5f60718293a4b5c6d7e8f9012345678",
  "date":"2026-10-19","channel":"stable","assets":[]
}"#;

fn body(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

#[test]
fn fetches_and_parses_the_manifest() {
    let server = TestServer::start(vec![("/manifest.json", Route::Bytes(200, MANIFEST.into()))]);
    let manifest = fetch_manifest(&server.base()).unwrap();
    assert_eq!(manifest.build, 14);
    assert_eq!(server.hits("/manifest.json"), 1);
}

#[test]
fn maps_http_errors() {
    let server = TestServer::start(vec![("/manifest.json", Route::Bytes(404, b"nope".to_vec()))]);
    assert!(matches!(fetch_manifest(&server.base()), Err(NetError::Http(404))));
    let server = TestServer::start(vec![("/manifest.json", Route::Bytes(200, b"{not json".to_vec()))]);
    assert!(matches!(fetch_manifest(&server.base()), Err(NetError::BadManifest)));
}

#[test]
fn downloads_with_progress() {
    let data = body(300_000);
    let server = TestServer::start(vec![("/a.bin", Route::Bytes(200, data.clone()))]);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a.bin");
    let mut calls: Vec<(u64, Option<u64>)> = Vec::new();
    download(&format!("{}/a.bin", server.base()), &dest, &mut |done, total| calls.push((done, total)), &CancelToken::new()).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    assert!(!dir.path().join("a.bin.part").exists());
    assert!(calls.len() >= 3, "{calls:?}");
    assert!(calls.windows(2).all(|w| w[0].0 < w[1].0), "{calls:?}");
    assert_eq!(calls.last(), Some(&(300_000, Some(300_000))));
}

#[test]
fn cancel_stops_a_download() {
    let server = TestServer::start(vec![("/a.bin", Route::Bytes(200, body(300_000)))]);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a.bin");
    let cancel = CancelToken::new();
    let token = cancel.clone();
    let result = download(&format!("{}/a.bin", server.base()), &dest, &mut |_, _| token.cancel(), &cancel);
    assert!(matches!(result, Err(NetError::Cancelled)));
    assert!(!dest.exists());
    assert!(!dir.path().join("a.bin.part").exists());
}

#[test]
fn a_stalled_server_times_out() {
    let server = TestServer::start(vec![("/a.bin", Route::Stall)]);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a.bin");
    let result = download_with_timeout(
        &format!("{}/a.bin", server.base()),
        &dest,
        &mut |_, _| {},
        &CancelToken::new(),
        Duration::from_millis(300),
    );
    assert!(matches!(result, Err(NetError::TimedOut)));
    assert!(!dest.exists());
}

#[test]
fn offline_is_reported() {
    assert!(matches!(fetch_manifest("http://127.0.0.1:1"), Err(NetError::Offline)));
}

fn assert_cancelled_quickly(result: Result<(), NetError>, started: std::time::Instant, dir: &std::path::Path) {
    assert!(matches!(result, Err(NetError::Cancelled)), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(1), "{:?}", started.elapsed());
    assert!(!dir.join("a.bin").exists());
    assert!(!dir.join("a.bin.part").exists());
}

#[test]
fn cancel_during_the_header_wait_returns_promptly() {
    let server = TestServer::start(vec![("/a.bin", Route::Stall)]);
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancelToken::new();
    let token = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        token.cancel();
    });
    let started = std::time::Instant::now();
    let result = download_with_timeout(&format!("{}/a.bin", server.base()), &dir.path().join("a.bin"), &mut |_, _| {}, &cancel, Duration::from_secs(30));
    assert_cancelled_quickly(result, started, dir.path());
}

#[test]
fn cancel_during_a_mid_body_stall_returns_promptly() {
    let server = TestServer::start(vec![("/a.bin", Route::StallAfter(200, body(300_000), 100_000))]);
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancelToken::new();
    let token = cancel.clone();
    let started = std::time::Instant::now();
    let result = download_with_timeout(
        &format!("{}/a.bin", server.base()),
        &dir.path().join("a.bin"),
        &mut |_, _| token.cancel(),
        &cancel,
        Duration::from_secs(30),
    );
    assert_cancelled_quickly(result, started, dir.path());
}

#[test]
fn a_mid_body_stall_times_out() {
    let server = TestServer::start(vec![("/a.bin", Route::StallAfter(200, body(300_000), 100_000))]);
    let dir = tempfile::tempdir().unwrap();
    let result = download_with_timeout(
        &format!("{}/a.bin", server.base()),
        &dir.path().join("a.bin"),
        &mut |_, _| {},
        &CancelToken::new(),
        Duration::from_millis(300),
    );
    assert!(matches!(result, Err(NetError::TimedOut)), "{result:?}");
    assert!(!dir.path().join("a.bin.part").exists());
}
