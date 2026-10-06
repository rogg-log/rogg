//! Every palette value must equal the reference CSS. The test reads the HTML directly, so the two cannot drift apart.
use rogg_ui::theme::{Color, Palette};
use std::collections::HashMap;

fn css() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/superpowers/specs/assets/rogg-main-window.html");
    let html = std::fs::read_to_string(path).expect("reference html");
    let start = html.find("<style>").unwrap() + 7;
    let end = html.find("</style>").unwrap();
    html[start..end].replace('\n', "")
}

/// Declarations of the rule whose selector is exactly `selector`, which must follow a `}`.
fn rule(css: &str, selector: &str) -> HashMap<String, String> {
    let needle = format!("}}{selector}{{");
    let at = css.find(&needle).unwrap_or_else(|| panic!("rule {selector} not found")) + needle.len();
    let body = &css[at..at + css[at..].find('}').unwrap()];
    body.split(';')
        .filter_map(|d| d.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn vars(css: &str, block_start: &str) -> HashMap<String, Color> {
    let at = css.find(block_start).unwrap() + block_start.len();
    let body = &css[at..at + css[at..].find('}').unwrap()];
    body.split(';')
        .filter_map(|d| d.split_once(':'))
        .filter_map(|(k, v)| Some((k.trim().trim_start_matches("--").to_string(), Color::parse_css(v.trim())?)))
        .collect()
}

/// First hex color inside a declaration value (handles `1px solid #49534f`); `var(--x)` resolves through `vars`.
fn color_in(value: &str, vars: &HashMap<String, Color>) -> Color {
    if let Some(name) = value.strip_prefix("var(--") {
        return vars[name.trim_end_matches(')')];
    }
    let hex: String = value[value.find('#').expect("hex")..].chars().take_while(|c| *c == '#' || c.is_ascii_hexdigit()).collect();
    Color::parse_css(&hex).unwrap()
}

#[test]
fn dark_palette_matches_css() {
    let css = css();
    let root = vars(&css, ":root{color-scheme:dark;");
    let p = Palette::dark();
    for (name, value) in [
        ("page", p.page), ("bg", p.bg), ("side", p.side), ("raised", p.raised), ("hover", p.hover),
        ("border", p.border), ("line", p.line), ("text", p.text), ("muted", p.muted), ("faint", p.faint),
        ("accent", p.accent), ("accent-bg", p.accent_bg), ("error", p.error), ("error-bg", p.error_bg),
        ("amber", p.amber), ("amber-bg", p.amber_bg), ("selection", p.selection),
    ] {
        assert_eq!(value, root[name], "--{name}");
    }
    assert_eq!(p.log_text, color_in(&rule(&css, ".row .line")["color"], &root));
    assert_eq!(p.error_row_tint, color_in(&rule(&css, ".row.error-line")["background"], &root));
    assert_eq!(p.band_border, color_in(&rule(&css, ".filter-band")["border-top"], &root));
    assert_eq!(p.band_handle, color_in(&rule(&css, ".filter-band:before")["background"], &root));
    assert_eq!(p.accent_border, color_in(&rule(&css, "button[aria-pressed=true]")["border-color"], &root));
    assert_eq!(p.chip_divider, color_in(&rule(&css, ".state")["border-right"], &root));
    assert_eq!(p.swatch_border, color_in(&rule(&css, ".swatch")["border"], &root));
    assert_eq!(p.scrollbar_thumb, color_in(&rule(&css, ".logs")["scrollbar-color"], &root));
    assert_eq!(p.traffic[0], color_in(&rule(&css, ".traffic span")["background"], &root));
    assert_eq!(p.traffic[1], color_in(&rule(&css, ".traffic span:nth-child(2)")["background"], &root));
    assert_eq!(p.traffic[2], color_in(&rule(&css, ".traffic span:nth-child(3)")["background"], &root));
    for slot in 0..4 {
        let hit = rule(&css, &format!(".hit-{slot}"));
        assert_eq!(p.hits[slot].fg, color_in(&hit["color"], &root), "hit-{slot} fg");
        assert_eq!(p.hits[slot].bg, color_in(&hit["background"], &root), "hit-{slot} bg");
    }
    let token = rule(&css, ".token");
    assert_eq!(p.chips[0].border, color_in(&token["border"], &root));
    assert_eq!(p.chips[0].bg, color_in(&token["background"], &root));
    assert_eq!(p.chips[0].term, color_in(&rule(&css, ".term")["color"], &root));
    for slot in 1..4 {
        let t = rule(&css, &format!(".token[data-color=\"{slot}\"]"));
        assert_eq!(p.chips[slot].border, color_in(&t["border-color"], &root), "chip {slot} border");
        assert_eq!(p.chips[slot].bg, color_in(&t["background"], &root), "chip {slot} bg");
        let term = rule(&css, &format!(".token[data-color=\"{slot}\"] .term"));
        assert_eq!(p.chips[slot].term, color_in(&term["color"], &root), "chip {slot} term");
    }
}

#[test]
fn light_palette_matches_css() {
    let css = css();
    let root = vars(&css, ":root{color-scheme:dark;");
    let mut light = root.clone();
    light.extend(vars(&css, ".light{color-scheme:light;"));
    let p = Palette::light();
    for (name, value) in [
        ("page", p.page), ("bg", p.bg), ("side", p.side), ("raised", p.raised), ("hover", p.hover),
        ("border", p.border), ("line", p.line), ("text", p.text), ("muted", p.muted), ("faint", p.faint),
        ("accent", p.accent), ("accent-bg", p.accent_bg), ("error", p.error), ("amber", p.amber),
        ("selection", p.selection),
    ] {
        assert_eq!(value, light[name], "light --{name}");
    }
    assert_eq!(p.log_text, color_in(&rule(&css, ".light .row .line")["color"], &light));
    for slot in 0..4 {
        let hit = rule(&css, &format!(".light .hit-{slot}"));
        assert_eq!(p.hits[slot].fg, color_in(&hit["color"], &light), "light hit-{slot} fg");
        assert_eq!(p.hits[slot].bg, color_in(&hit["background"], &light), "light hit-{slot} bg");
    }
    let token = rule(&css, ".light .token");
    assert_eq!(p.chips[0].border, color_in(&token["border-color"], &light));
    assert_eq!(p.chips[0].bg, color_in(&token["background"], &light));
    assert_eq!(p.chips[0].term, light["error"]);
    assert_eq!(p.chips[1].term, light["amber"]);
    for slot in 1..4 {
        let t = rule(&css, &format!(".light .token[data-color=\"{slot}\"]"));
        assert_eq!(p.chips[slot].border, color_in(&t["border-color"], &light));
        assert_eq!(p.chips[slot].bg, color_in(&t["background"], &light));
    }
    for slot in 2..4 {
        let term = rule(&css, &format!(".light .token[data-color=\"{slot}\"] .term"));
        assert_eq!(p.chips[slot].term, color_in(&term["color"], &light));
    }
}

#[test]
fn named_lists_every_token_once() {
    let names: Vec<_> = Palette::dark().named().into_iter().map(|(n, _)| n).collect();
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(names.len(), unique.len());
    assert!(names.contains(&"accent") && names.contains(&"hit-3 bg") && names.contains(&"chip-2 term"));
}
