//! Deterministic synthetic logs for tests and benchmarks (spec §8). No third-party dependencies.

use std::io::{self, BufWriter, Write};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenEncoding {
    Utf8,
    Utf16Le,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenOptions {
    pub size_bytes: u64,
    pub seed: u64,
    pub crlf: bool,
    /// Emit a ~100 KB line every N lines (exercises long-line handling).
    pub long_line_every: Option<u64>,
    pub encoding: GenEncoding,
}

impl Default for GenOptions {
    fn default() -> Self {
        Self { size_bytes: 1 << 20, seed: 42, crlf: false, long_line_every: None, encoding: GenEncoding::Utf8 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenStats {
    pub lines: u64,
    pub bytes: u64,
}

/// SplitMix64: tiny, fast, and identical on every platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len() as u64) as usize]
    }
}

const SOURCES: [&str; 12] =
    ["server", "router", "auth", "pool", "client", "healthcheck", "config", "queue", "worker", "circuit", "payments", "database"];
const TARGETS: [&str; 4] = ["payments-primary", "payments-secondary", "cache", "metrics"];
const METHODS: [&str; 3] = ["GET", "POST", "PUT"];
const PATHS: [&str; 4] = ["/v1/payments", "/v1/accounts", "/v1/authorize", "/status"];

fn level(rng: &mut Rng) -> &'static str {
    match rng.below(100) {
        0..60 => "INFO ",
        60..85 => "DEBUG",
        85..95 => "WARN ",
        _ => "ERROR",
    }
}

fn message(rng: &mut Rng) -> String {
    let req = rng.below(0x10000);
    match rng.below(10) {
        0 => format!("request started method={} path={} request_id=req_{req:04x}", rng.pick(&METHODS), rng.pick(&PATHS)),
        1 => format!("request completed status={} duration={}ms request_id=req_{req:04x}", [200, 200, 404, 502, 503][rng.below(5) as usize], rng.below(40_000)),
        2 => format!("retry attempt={} backoff={}ms target={}", 1 + rng.below(5), 250 << rng.below(4), rng.pick(&TARGETS)),
        3 => format!("upstream timeout after {}ms request_id=req_{req:04x} host={}.internal:8443", 1000 + rng.below(30_000), rng.pick(&TARGETS)),
        4 => format!("connection reset by peer upstream={} request_id=req_{req:04x}", rng.pick(&TARGETS)),
        5 => format!("healthcheck upstream={} status=ok latency={}ms", rng.pick(&TARGETS), rng.below(50)),
        6 => format!("job failed error=DEADLINE_EXCEEDED job_id=job_{} attempt={}", rng.below(10_000), 1 + rng.below(4)),
        7 => format!("acquired connection pool=primary active={} idle={}", rng.below(50), rng.below(20)),
        8 => format!("query completed rows={} duration={}ms", rng.below(1_000), rng.below(200)),
        _ => format!("flush completed counters={} histograms={} duration={}ms", rng.below(200), rng.below(40), rng.below(10)),
    }
}

/// Writes synthetic log lines until at least `size_bytes` bytes have been written.
pub fn generate(opts: &GenOptions, out: &mut impl Write) -> io::Result<GenStats> {
    let mut rng = Rng(opts.seed);
    let mut stats = GenStats::default();
    let mut clock_ms: u64 = (14 * 3600 + 32 * 60 + 8) * 1000 + 142;
    let mut line = String::with_capacity(256);
    let mut encoded = Vec::with_capacity(512);
    if opts.encoding == GenEncoding::Utf16Le {
        out.write_all(&[0xFF, 0xFE])?;
        stats.bytes += 2;
    }
    while stats.bytes < opts.size_bytes {
        clock_ms += rng.below(250);
        let day_ms = clock_ms % (24 * 3600 * 1000);
        let (h, m, s, ms) = (day_ms / 3_600_000, day_ms / 60_000 % 60, day_ms / 1000 % 60, day_ms % 1000);
        line.clear();
        use std::fmt::Write as _;
        let _ = write!(line, "2026-10-05 {h:02}:{m:02}:{s:02}.{ms:03}  {}  {:<12} {}", level(&mut rng), rng.pick(&SOURCES), message(&mut rng));
        if opts.long_line_every.is_some_and(|n| n > 0 && (stats.lines + 1) % n == 0) {
            line.push_str(" payload=");
            line.extend(std::iter::repeat_n('x', 100_000));
        }
        line.push_str(if opts.crlf { "\r\n" } else { "\n" });
        encoded.clear();
        match opts.encoding {
            GenEncoding::Utf8 => encoded.extend_from_slice(line.as_bytes()),
            GenEncoding::Utf16Le => encoded.extend(line.encode_utf16().flat_map(u16::to_le_bytes)),
        }
        out.write_all(&encoded)?;
        stats.bytes += encoded.len() as u64;
        stats.lines += 1;
    }
    Ok(stats)
}

pub fn generate_file(path: &Path, opts: &GenOptions) -> io::Result<GenStats> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = BufWriter::with_capacity(1 << 20, std::fs::File::create(path)?);
    let stats = generate(opts, &mut out)?;
    out.flush()?;
    Ok(stats)
}

/// `"1G"`, `"512M"`, `"64K"`, `"2GB"`, `"1024"` (binary units, case-insensitive).
pub fn parse_size(text: &str) -> Option<u64> {
    let t = text.trim().to_ascii_uppercase();
    let t = t.strip_suffix('B').unwrap_or(&t);
    let (digits, shift) = match t.chars().last()? {
        'K' => (&t[..t.len() - 1], 10),
        'M' => (&t[..t.len() - 1], 20),
        'G' => (&t[..t.len() - 1], 30),
        _ => (t, 0),
    };
    digits.parse::<u64>().ok().map(|n| n << shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r#gen(opts: &GenOptions) -> (Vec<u8>, GenStats) {
        let mut out = Vec::new();
        let stats = generate(opts, &mut out).unwrap();
        (out, stats)
    }

    #[test]
    fn deterministic_per_seed_and_sized() {
        let opts = GenOptions { size_bytes: 64 * 1024, ..Default::default() };
        let (a, sa) = r#gen(&opts);
        let (b, _) = r#gen(&opts);
        let (c, _) = r#gen(&GenOptions { seed: 7, ..opts.clone() });
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.len() as u64 >= opts.size_bytes && a.len() < opts.size_bytes as usize + 512);
        assert_eq!(sa.bytes, a.len() as u64);
        assert_eq!(sa.lines, a.iter().filter(|&&x| x == b'\n').count() as u64);
    }

    #[test]
    fn lines_follow_the_reference_format() {
        let (out, _) = r#gen(&GenOptions { size_bytes: 16 * 1024, ..Default::default() });
        let text = String::from_utf8(out).unwrap();
        for line in text.lines() {
            assert!(line.starts_with("2026-10-05 "), "{line}");
            assert_eq!(&line[23..25], "  ");
            let level = &line[25..30];
            assert!(["INFO ", "DEBUG", "WARN ", "ERROR"].contains(&level), "{line}");
        }
        assert!(text.contains("timeout") && text.contains("request_id=req_") && text.contains("healthcheck"));
    }

    #[test]
    fn crlf_utf16_and_long_lines() {
        let (crlf, _) = r#gen(&GenOptions { size_bytes: 8 * 1024, crlf: true, ..Default::default() });
        assert!(crlf.windows(2).filter(|w| w[1] == b'\n').all(|w| w[0] == b'\r'));
        let (utf16, stats) = r#gen(&GenOptions { size_bytes: 8 * 1024, encoding: GenEncoding::Utf16Le, ..Default::default() });
        assert_eq!(&utf16[..2], &[0xFF, 0xFE]);
        assert_eq!(utf16.len() % 2, 0);
        let decoded = String::from_utf16(&utf16[2..].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()).unwrap();
        assert_eq!(decoded.lines().count() as u64, stats.lines);
        let (long, _) = r#gen(&GenOptions { size_bytes: 300 * 1024, long_line_every: Some(10), ..Default::default() });
        assert!(long.split(|&b| b == b'\n').any(|l| l.len() > 100_000));
    }

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("1G"), Some(1 << 30));
        assert_eq!(parse_size("512m"), Some(512 << 20));
        assert_eq!(parse_size("64K"), Some(64 << 10));
        assert_eq!(parse_size("2GB"), Some(2 << 30));
        assert_eq!(parse_size("1024"), Some(1024));
        assert_eq!(parse_size("lots"), None);
    }
}
