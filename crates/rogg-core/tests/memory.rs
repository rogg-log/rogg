use rogg_core::CancelToken;
use rogg_core::text::LogText;
use std::path::Path;

#[test]
#[ignore = "generates/uses a 1 GiB file; run via `just bench`"]
fn index_of_one_gigabyte_fits_the_memory_budget() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bench-data/genlog-1G-seed42.log");
    if !path.exists() {
        rogg_genlog::generate_file(&path, &rogg_genlog::GenOptions { size_bytes: 1 << 30, seed: 42, ..Default::default() }).unwrap();
    }
    let text = LogText::open(&path, &std::env::temp_dir(), &CancelToken::new(), |_| {}).unwrap();
    let heap = text.index().heap_bytes();
    println!("lines: {}, index heap: {:.1} MB", text.line_count(), heap as f64 / 1e6);
    assert!(text.line_count() > 5_000_000);
    assert!(heap < 100 * 1024 * 1024, "index heap {heap} bytes exceeds the 100 MB budget");
}
