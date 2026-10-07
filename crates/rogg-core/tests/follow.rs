use rogg_core::CancelToken;
use rogg_core::follow::{FollowEvent, Follower, POLL_FALLBACK, WatchMode};
use rogg_core::text::LogText;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn open(path: &Path) -> Arc<LogText> {
    Arc::new(LogText::open(path, &path.with_extension("cache"), &CancelToken::new(), |_| {}).unwrap())
}

/// Waits up to 10 s for an event that `want` accepts; earlier events are skipped.
fn wait(follower: &Follower, want: impl Fn(&FollowEvent) -> bool) -> FollowEvent {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(event) = follower.events().recv_timeout(Duration::from_millis(50))
            && want(&event)
        {
            return event;
        }
    }
    panic!("no matching follow event within 10 s");
}

#[test]
fn appends_arrive_as_growing_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("live.log");
    std::fs::write(&path, "one\n").unwrap();
    let follower = Follower::spawn(open(&path), WatchMode::Path(path.clone()), POLL_FALLBACK);
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"two\nthree\n").unwrap();
    let FollowEvent::Grew(text) = wait(&follower, |e| matches!(e, FollowEvent::Grew(t) if t.line_count() == 3)) else { unreachable!() };
    assert_eq!(text.line_text(2), "three");
}

#[test]
fn truncation_and_replacement_end_the_follower() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.log");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let follower = Follower::spawn(open(&path), WatchMode::Path(path.clone()), POLL_FALLBACK);
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(2).unwrap();
    wait(&follower, |e| matches!(e, FollowEvent::Truncated));

    let rotated = dir.path().join("r.log");
    std::fs::write(&rotated, "old\n").unwrap();
    let follower = Follower::spawn(open(&rotated), WatchMode::Path(rotated.clone()), POLL_FALLBACK);
    let fresh = dir.path().join("r.log.new");
    std::fs::write(&fresh, "new\n").unwrap();
    std::fs::rename(&fresh, &rotated).unwrap();
    wait(&follower, |e| matches!(e, FollowEvent::Replaced));
}

#[test]
fn poll_mode_sees_growth_without_events() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.log");
    std::fs::write(&path, "a\n").unwrap();
    let follower = Follower::spawn(open(&path), WatchMode::Poll, Duration::from_millis(20));
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"b\n").unwrap();
    wait(&follower, |e| matches!(e, FollowEvent::Grew(t) if t.line_count() == 2));
    follower.stop();
}
