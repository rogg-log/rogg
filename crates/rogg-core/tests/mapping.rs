use rogg_core::mapping::SafeMap;
use std::fs::{File, OpenOptions};

#[cfg(unix)]
#[test]
fn truncation_under_a_mapping_reads_zeros_and_slots_are_released() {
    use rogg_core::CancelToken;
    use rogg_core::mapping::guarded_count;
    use rogg_core::text::LogText;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shrinking.log");
    std::fs::write(&path, vec![b'x'; 1 << 20]).unwrap();
    let before = guarded_count();
    let map = SafeMap::map(&File::open(&path).unwrap()).unwrap();
    assert!(map.is_guarded());
    // Our handler must run on the alternate stack, like Rust's own, so stack overflows are still reported.
    // SAFETY: reads the current SIGBUS action into a zeroed struct.
    let mut current: libc::sigaction = unsafe { std::mem::zeroed() };
    unsafe { libc::sigaction(libc::SIGBUS, std::ptr::null(), &mut current) };
    assert!(current.sa_flags & libc::SA_ONSTACK != 0, "the SIGBUS handler runs on the alternate stack");
    assert_eq!(guarded_count(), before + 1);
    // `logrotate copytruncate` / `> app.log`: the file shrinks under the mapping.
    OpenOptions::new().write(true).open(&path).unwrap().set_len(100).unwrap();
    let xs = map.bytes().iter().filter(|&&b| b == b'x').count();
    assert_eq!(xs, 100, "bytes past the new end read as zeros instead of raising SIGBUS");
    assert_eq!(map.bytes()[(1 << 20) - 1], 0);
    drop(map);
    assert_eq!(guarded_count(), before, "dropping a mapping frees its slot");

    // An empty file cannot be mapped.
    let empty = dir.path().join("empty.log");
    std::fs::write(&empty, b"").unwrap();
    let error = SafeMap::map(&File::open(&empty).unwrap()).err().expect("mapping an empty file fails");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(guarded_count(), before, "a failed mapping leaves no slot behind");

    // The same through LogText: reading every line of a truncated file does not crash.
    let lines: Vec<u8> = (0..50_000).flat_map(|i| format!("line {i}\n").into_bytes()).collect();
    std::fs::write(&path, &lines).unwrap();
    let text = LogText::open(&path, dir.path(), &CancelToken::new(), |_| {}).unwrap();
    OpenOptions::new().write(true).open(&path).unwrap().set_len(7).unwrap();
    assert_eq!(text.line(0), b"line 0");
    let zero_lines = (0..text.line_count()).filter(|&i| text.line(i).iter().all(|&b| b == 0)).count();
    assert!(zero_lines > 40_000, "lines past the truncation read as zeros ({zero_lines})");
}

/// Windows refuses to shrink a file while a view of it is mapped, so no guard is needed there.
#[cfg(windows)]
#[test]
fn windows_refuses_to_truncate_a_mapped_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mapped.log");
    std::fs::write(&path, vec![b'x'; 1 << 20]).unwrap();
    let map = SafeMap::map(&File::open(&path).unwrap()).unwrap();
    assert!(OpenOptions::new().write(true).open(&path).unwrap().set_len(100).is_err());
    assert_eq!(map.bytes().len(), 1 << 20);
}
