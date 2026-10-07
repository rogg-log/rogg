use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rogg_core::CancelToken;
use rogg_core::engine::{DEFAULT_CHUNK_LINES, run_filter};
use rogg_core::filter::{CompiledFilter, FilterSpec, Term};
use rogg_core::text::LogText;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn data_file() -> PathBuf {
    let size = std::env::var("ROGG_BENCH_SIZE").unwrap_or_else(|_| "1G".into());
    let bytes = rogg_genlog::parse_size(&size).expect("ROGG_BENCH_SIZE like 1G or 256M");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/bench-data/genlog-{size}-seed42.log"));
    if !path.exists() {
        rogg_genlog::generate_file(&path, &rogg_genlog::GenOptions { size_bytes: bytes, seed: 42, ..Default::default() })
            .expect("generate bench data");
    }
    path
}

fn open(path: &Path) -> LogText {
    LogText::open(path, &std::env::temp_dir().join("rogg-bench-cache"), &CancelToken::new(), |_| {}).expect("open bench file")
}

fn engine(c: &mut Criterion) {
    let path = data_file();
    let len = std::fs::metadata(&path).unwrap().len();
    let mut group = c.benchmark_group("engine");
    group.sample_size(10).measurement_time(Duration::from_secs(30)).throughput(Throughput::Bytes(len));

    group.bench_function("index", |b| b.iter(|| open(&path)));

    let text = open(&path);
    let plain = CompiledFilter::compile(&FilterSpec {
        terms: ["timeout", "retry", "connection reset", "healthcheck"].map(Term::include).to_vec(),
        ..Default::default()
    })
    .unwrap();
    group.bench_function("filter_plain_4", |b| b.iter(|| run_filter(&text, &plain, DEFAULT_CHUNK_LINES, &CancelToken::new()).unwrap()));

    let regex = CompiledFilter::compile(&FilterSpec {
        terms: [r"request_id=req_[0-9a-f]{3}7", r"duration=\d{5,}ms"].map(Term::include).to_vec(),
        regex: true,
        ..Default::default()
    })
    .unwrap();
    group.bench_function("filter_regex_2", |b| b.iter(|| run_filter(&text, &regex, DEFAULT_CHUNK_LINES, &CancelToken::new()).unwrap()));
    group.finish();
}

criterion_group!(benches, engine);
criterion_main!(benches);
