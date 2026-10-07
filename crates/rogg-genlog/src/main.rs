use rogg_genlog::{GenEncoding, GenOptions, generate_file, parse_size};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let usage = "usage: genlog --size 1G --out FILE [--seed N] [--crlf] [--utf16le] [--long-every N]";
    let Some(out) = value("--out").map(PathBuf::from) else {
        eprintln!("{usage}");
        std::process::exit(2);
    };
    let size = value("--size").map(|s| parse_size(&s)).unwrap_or(Some(1 << 20));
    let Some(size_bytes) = size else {
        eprintln!("bad --size; {usage}");
        std::process::exit(2);
    };
    let opts = GenOptions {
        size_bytes,
        seed: value("--seed").and_then(|s| s.parse().ok()).unwrap_or(42),
        crlf: args.iter().any(|a| a == "--crlf"),
        long_line_every: value("--long-every").and_then(|s| s.parse().ok()),
        encoding: if args.iter().any(|a| a == "--utf16le") { GenEncoding::Utf16Le } else { GenEncoding::Utf8 },
    };
    match generate_file(&out, &opts) {
        Ok(stats) => println!("wrote {} lines, {} bytes to {}", stats.lines, stats.bytes, out.display()),
        Err(e) => {
            eprintln!("genlog: {e}");
            std::process::exit(1);
        }
    }
}
