//! Built-in row decoration until highlighting profiles arrive (M4): a leading timestamp renders faint and a line
//! whose level is ERROR gets the error row tint, as in the reference.

/// Byte length of a leading `YYYY-MM-DD HH:MM:SS` timestamp (`T` separator allowed) with an optional `.fff` or
/// `,fff` fraction of 1–9 digits; 0 when the line does not start with one.
pub fn timestamp_len(line: &str) -> usize {
    let b = line.as_bytes();
    let digits = |r: std::ops::Range<usize>| b.get(r).is_some_and(|s| s.iter().all(|c| c.is_ascii_digit()));
    let at = |i: usize, allowed: &[u8]| b.get(i).is_some_and(|c| allowed.contains(c));
    let stamp = digits(0..4)
        && at(4, b"-")
        && digits(5..7)
        && at(7, b"-")
        && digits(8..10)
        && at(10, b" T")
        && digits(11..13)
        && at(13, b":")
        && digits(14..16)
        && at(16, b":")
        && digits(17..19);
    if !stamp {
        return 0;
    }
    if at(19, b".,") {
        let fraction = b[20..].iter().take(9).take_while(|c| c.is_ascii_digit()).count();
        if fraction > 0 {
            return 20 + fraction;
        }
    }
    19
}

/// True when `ERROR` appears as a whole word within the first 96 bytes (the level column).
pub fn is_error(line: &str) -> bool {
    let head = &line.as_bytes()[..line.len().min(96)];
    head.windows(5).enumerate().any(|(i, w)| {
        w == b"ERROR" && (i == 0 || !head[i - 1].is_ascii_alphanumeric()) && head.get(i + 5).is_none_or(|c| !c.is_ascii_alphanumeric())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_timestamps() {
        assert_eq!(timestamp_len("2026-10-05 14:32:08.142  ERROR  client       upstream timeout"), 23);
        assert_eq!(timestamp_len("2026-10-05T14:32:08Z start"), 19);
        assert_eq!(timestamp_len("2026-10-05 14:32:08,5 x"), 21);
        assert_eq!(timestamp_len("2026-10-05 14:32:08."), 19, "a dot without digits is not a fraction");
        assert_eq!(timestamp_len("14:32:08 no date"), 0);
        assert_eq!(timestamp_len(""), 0);
    }

    #[test]
    fn error_level_words() {
        assert!(is_error("2026-10-05 14:32:08.142  ERROR  client  x"));
        assert!(is_error("ERROR: boom"));
        assert!(!is_error("2026-10-05 14:32:08.142  INFO   server  error=UPSTREAM_UNAVAILABLE"));
        assert!(!is_error("ERRORS were few"));
        assert!(!is_error(&format!("{}ERROR", "x ".repeat(60))), "only the level column (first 96 bytes) counts");
    }
}
