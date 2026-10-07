use rogg_core::cache::{Cache, DEFAULT_LIMIT_BYTES};
use rogg_core::stream::{StreamCapture, fd_has_data, is_stream_path};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

fn wait_for(mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "condition not reached within 10 s");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_pipe_is_copied_as_it_arrives() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path(), DEFAULT_LIMIT_BYTES);
    let (reader, mut writer) = std::io::pipe().unwrap();
    assert!(fd_has_data(reader.as_raw_fd()), "a pipe carries data");
    let capture = StreamCapture::start(move || Ok(reader), &cache).unwrap();
    assert!(capture.path().starts_with(cache.streams_dir()));
    assert!(capture.path().exists(), "the capture file exists before any data arrives");
    let name = capture.path().file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with(&format!("stream-{}-", std::process::id())) && name.ends_with(".log"), "{name}");
    writer.write_all(b"one\ntwo\n").unwrap();
    wait_for(|| std::fs::read(capture.path()).unwrap() == b"one\ntwo\n");
    assert!(!capture.is_done());
    writer.write_all(b"three\n").unwrap();
    drop(writer);
    wait_for(|| capture.is_done());
    assert_eq!(std::fs::read(capture.path()).unwrap(), b"one\ntwo\nthree\n");
    assert_eq!(capture.bytes(), 14);
    assert_eq!(capture.error(), None);
}

#[test]
fn stopping_a_capture_ends_it_at_the_next_read() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path(), DEFAULT_LIMIT_BYTES);
    let (reader, mut writer) = std::io::pipe().unwrap();
    let capture = StreamCapture::start(move || Ok(reader), &cache).unwrap();
    writer.write_all(b"one\n").unwrap();
    wait_for(|| capture.bytes() == 4);
    capture.stop();
    assert!(!capture.is_done(), "a blocked read cannot be interrupted");
    // The producer never ends; the capture finishes soon after its next write, without an error and without keeping it.
    writer.write_all(b"two\n").unwrap();
    wait_for(|| capture.is_done());
    assert_eq!(capture.error(), None);
    assert_eq!(std::fs::read(capture.path()).unwrap(), b"one\n");
    drop(writer);
}

#[test]
fn opening_errors_are_reported_not_raised() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path(), DEFAULT_LIMIT_BYTES);
    let capture = StreamCapture::start(|| Err::<std::fs::File, _>(std::io::Error::other("no writer")), &cache).unwrap();
    wait_for(|| capture.is_done());
    assert!(capture.error().unwrap().contains("no writer"));
}

#[test]
fn terminals_and_dev_null_carry_no_data_but_fifos_do() {
    let null = std::fs::File::open("/dev/null").unwrap();
    assert!(!fd_has_data(null.as_raw_fd()));
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f.log");
    std::fs::write(&file, "x").unwrap();
    assert!(fd_has_data(std::fs::File::open(&file).unwrap().as_raw_fd()), "`rogg < file.log`");
    assert!(!is_stream_path(&file));
    let fifo = dir.path().join("fifo");
    let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    assert!(is_stream_path(&fifo));
    assert!(!is_stream_path(&dir.path().join("missing")));
}

#[test]
fn a_panicking_capture_still_reports_done() {
    struct Boom;
    impl std::io::Read for Boom {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("reader exploded")
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let cache = rogg_core::cache::Cache::new(dir.path(), 1 << 30);
    let capture = rogg_core::stream::StreamCapture::start(|| Ok(Boom), &cache).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !capture.is_done() {
        assert!(std::time::Instant::now() < deadline, "done was never set");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
