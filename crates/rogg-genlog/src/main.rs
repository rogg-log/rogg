use rogg_genlog::{GenEncoding, GenOptions, generate_file, parse_size};
use std::path::PathBuf;

struct Args {
    size_bytes: u64,
    out: PathBuf,
    seed: u64,
    crlf: bool,
    long_line_every: Option<u64>,
    encoding: GenEncoding,
}

fn parse_args(raw_args: Vec<String>) -> Result<Args, String> {
    let usage = "usage: genlog --size 1G --out FILE [--seed N] [--crlf] [--utf16le] [--long-every N]";

    // Validate all arguments are recognized
    let known_flags = ["--size", "--out", "--seed", "--crlf", "--utf16le", "--long-every"];
    for arg in &raw_args {
        if arg.starts_with("--") && !known_flags.contains(&arg.as_str()) {
            return Err(format!("unknown argument: {}\n{}", arg, usage));
        }
    }

    let value = |flag: &str| {
        raw_args.iter().position(|a| a == flag).and_then(|i| raw_args.get(i + 1)).cloned()
    };

    // Validate flags that require values have values
    for flag in ["--size", "--out", "--seed", "--long-every"] {
        if raw_args.iter().any(|a| a == flag) && value(flag).is_none() {
            return Err(format!("flag {} requires a value\n{}", flag, usage));
        }
    }

    // Parse required --out
    let Some(out) = value("--out").map(PathBuf::from) else {
        return Err(format!("--out is required\n{}", usage));
    };

    // Parse --size
    let size = value("--size").map(|s| parse_size(&s)).unwrap_or(Some(1 << 20));
    let Some(size_bytes) = size else {
        return Err(format!("bad --size\n{}", usage));
    };

    // Parse --seed
    let seed = if let Some(s) = value("--seed") {
        s.parse::<u64>().map_err(|_| format!("bad --seed: unparseable\n{}", usage))?
    } else {
        42
    };

    // Parse --long-every
    let long_line_every = if let Some(s) = value("--long-every") {
        Some(s.parse::<u64>().map_err(|_| format!("bad --long-every: unparseable\n{}", usage))?)
    } else {
        None
    };

    // Parse flags
    let crlf = raw_args.iter().any(|a| a == "--crlf");
    let encoding = if raw_args.iter().any(|a| a == "--utf16le") { GenEncoding::Utf16Le } else { GenEncoding::Utf8 };

    Ok(Args { size_bytes, out, seed, crlf, long_line_every, encoding })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let parsed = match parse_args(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    let opts = GenOptions {
        size_bytes: parsed.size_bytes,
        seed: parsed.seed,
        crlf: parsed.crlf,
        long_line_every: parsed.long_line_every,
        encoding: parsed.encoding,
    };

    match generate_file(&parsed.out, &opts) {
        Ok(stats) => println!("wrote {} lines, {} bytes to {}", stats.lines, stats.bytes, parsed.out.display()),
        Err(e) => {
            eprintln!("genlog: {e}");
            std::process::exit(1);
        }
    }
}
