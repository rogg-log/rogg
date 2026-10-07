//! Human-readable sizes for the sidebar and its footer.

/// Decimal units, as Finder shows them: `820 B`, `14 KB`, `3.2 MB`, `1.2 GB`.
pub fn format_size(bytes: u64) -> String {
    const K: f64 = 1000.;
    let b = bytes as f64;
    if b < K {
        format!("{bytes} B")
    } else if b < K * K {
        format!("{:.0} KB", b / K)
    } else if b < K * K * K {
        format!("{:.1} MB", b / (K * K))
    } else {
        format!("{:.1} GB", b / (K * K * K))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(format_size(820), "820 B");
        assert_eq!(format_size(14_200), "14 KB");
        assert_eq!(format_size(3_240_000), "3.2 MB");
        assert_eq!(format_size(1_200_000_000), "1.2 GB");
    }
}
