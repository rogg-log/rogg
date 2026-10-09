//! Local wall-clock time for labels (the `Snapshot · HH:MM:SS` note of a derived tab, spec §5.6).

use std::time::{SystemTime, UNIX_EPOCH};

/// `HH:MM:SS` of `time` in the local time zone; `00:00:00` if the time cannot be converted.
#[cfg(unix)]
pub fn local_hms(time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as libc::time_t;
    // SAFETY: `localtime_r` only reads `secs` and writes into `tm`.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
        return "00:00:00".into();
    }
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

/// `HH:MM:SS` of `time` in the local time zone; `00:00:00` if the time cannot be converted.
#[cfg(windows)]
pub fn local_hms(time: SystemTime) -> String {
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};

    // FILETIME counts 100 ns ticks since 1601-01-01; the Unix epoch is 11_644_473_600 s later.
    const EPOCH_DIFF_SECS: u64 = 11_644_473_600;
    let secs = time.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let Some(ticks) = secs.checked_add(EPOCH_DIFF_SECS).and_then(|s| s.checked_mul(10_000_000)) else {
        return "00:00:00".into();
    };
    let ft = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    // SAFETY: both calls only read their inputs and write into the zeroed `SYSTEMTIME`s; a null zone means "the
    // current time zone".
    let mut utc: SYSTEMTIME = unsafe { std::mem::zeroed() };
    let mut local: SYSTEMTIME = unsafe { std::mem::zeroed() };
    let ok = unsafe { FileTimeToSystemTime(&ft, &mut utc) != 0 && SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) != 0 };
    if !ok {
        return "00:00:00".into();
    }
    format!("{:02}:{:02}:{:02}", local.wHour, local.wMinute, local.wSecond)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn parts(s: &str) -> (u32, u32, u32) {
        let v: Vec<u32> = s.split(':').map(|p| p.parse().unwrap()).collect();
        (v[0], v[1], v[2])
    }

    #[test]
    fn formats_hh_mm_ss_and_advances_with_time() {
        let t0 = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let a = local_hms(t0);
        assert_eq!(a.len(), 8);
        let (h, m, s) = parts(&a);
        assert!(h < 24 && m < 60 && s < 60);
        let (_, m1, s1) = parts(&local_hms(t0 + Duration::from_secs(61)));
        assert_eq!(s1, (s + 1) % 60);
        assert_eq!(m1, (m + 1 + (s + 1) / 60) % 60);
    }
}
