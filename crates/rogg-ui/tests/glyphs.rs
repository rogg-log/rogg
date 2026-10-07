//! Refinements 1 (A1, A2): the `⌄` text glyph is gone from the UI. Disclosure uses the ChevronRight icon (rotated
//! when open), and text buttons have no trailing chevron.
use std::path::Path;

fn scan(dir: &Path, hits: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            scan(&path, hits);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            for (n, line) in text.lines().enumerate() {
                if line.contains('⌄') {
                    hits.push(format!("{}:{}", path.display(), n + 1));
                }
            }
        }
    }
}

#[test]
fn no_down_chevron_glyphs_remain_in_ui_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut hits = Vec::new();
    for dir in ["src", "../rogg-app/src", "../rogg-gallery/src"] {
        scan(&root.join(dir), &mut hits);
    }
    assert!(hits.is_empty(), "⌄ glyphs left: {hits:?}");
}
