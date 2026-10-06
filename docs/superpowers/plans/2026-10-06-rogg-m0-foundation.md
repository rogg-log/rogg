# rogg M0 — Foundation & Visual Approval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build every rogg UI component as a reusable `rogg-ui` crate, show them all in a `rogg-gallery` app (including a pixel-comparable rebuild of the reference mockup), and produce a visual-diff report against `docs/superpowers/specs/assets/rogg-main-window.html` for a joint review with the user.

**Architecture:** `rogg-ui` holds the theme, icons and `RenderOnce` components; they take plain data and report intents through callbacks. The log panels are one custom gpui `Element` with its own virtualized, anchor-based scrolling. `rogg-gallery` is a lib + bin that renders the same components with canned data. Its `--capture` mode renders headlessly (gpui `HeadlessAppContext`, Metal offscreen, 2× scale) to PNG plus element geometry JSON. `tools/visual-diff` (Node + Playwright + pixelmatch) renders the HTML reference the same way and writes an HTML report.

**Tech Stack:** Rust 1.99.0 (edition 2024), `gpui-kit =0.7.1` (bundles `gpui-pre =0.3.8`), `rust-embed` 8, Node 26 + Playwright + pixelmatch + pngjs, `just`.

**Spec:** `docs/superpowers/specs/2026-10-06-rogg-design.md` (sections 3, 5, 7 and 9/M0). **API reference:** `docs/superpowers/research/2026-10-06-gpui-api-notes.md`. Every gpui/gpui-kit signature used below was checked against those notes. If the compiler disagrees with this plan on an API detail, the notes and the compiler win: adapt minimally and keep the behavior.

## Global Constraints

- Platform: macOS on Apple Silicon only.
- Toolchain: Rust `1.99.0`, pinned in `rust-toolchain.toml`. Edition `2024`, workspace resolver `3`.
- GUI dependency: only `gpui-kit = "=0.7.1"`. Never add `gpui-pre` directly.
- **Never `use gpui_kit::*;`.** With `test-support` enabled, the glob imports a `test` macro that clashes with `#[test]`, and `Size` collides with `gpui_kit::component::Size`. Use `use gpui_kit::prelude::*;` plus explicit imports.
- Colors and sizes come only from `rogg_ui::theme` (`Palette`, `Metrics`, `theme::size`). Components contain no hex literals. Values are copied verbatim from the reference CSS.
- Fonts: UI = `".SystemUIFont"`, mono = `".AppleSystemUIFontMonospaced"` (shown to users as "SF Mono"). The family name `"SF Mono"` must never be used: gpui silently falls back to Helvetica.
- Components are `RenderOnce`. They take plain data, report through `Rc<dyn Fn(..)>` callbacks, and never touch files or engines.
- Elements whose geometry is compared get `.id(<key>).test_support()`, with `<key>` taken from the key table in Task 15. Use string ids (`SharedString`), not tuple ids.
- Call `RoggTheme::install(cx)` after `gpui_kit::init(cx)` and before opening any window.
- Never call `.hover(..)` on a gpui-kit `Button`, because it trips a debug assert. Use `ButtonCustomVariant`.
- Comparison size: 1440×900 logical px at scale 2. Variants: dark/light × compact/comfortable.
- Commits: conventional style, one per task at minimum. Every message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

These failure modes are implied by the spec but easy to miss. Each one is pinned by a test in the task that owns the code.

1. **Many or long tabs:** labels ellipsize at 127 px and the tab strip scrolls, while the title-bar actions stay inside the window. Test in Task 5.
2. **Many filter terms (20 chips):** the chips scroll horizontally and the `.*` / `Aa` / edit buttons stay visible. Test in Task 8.
3. **Hostile log text:** a 200 KB line, tab and control characters, multi-byte characters at the truncation boundary, spans past the end of the text. The text is truncated at 64 KB on a char boundary, offsets are preserved, and nothing panics. Tests in Task 9.
4. **Degenerate scrolling:** zero rows, one row, content shorter than the viewport, viewport shorter than one row, centering on the first or last row. No panic, and scrolling is clamped. Tests in Task 9.
5. **Light theme parity:** every component reads colors from the active palette, and switching theme at runtime re-colors everything without rebuilding data (spans and tints are semantic). Tests in Task 2 (palette vs CSS) and Task 9 (`semantic_styles_follow_the_palette`).

---

## File Structure

```
Cargo.toml                         workspace (members, shared deps)
rust-toolchain.toml                pinned toolchain
justfile                           test / gallery / capture / visual tasks
crates/rogg-ui/
  Cargo.toml
  assets/icons/rogg/*.svg          21 mask-safe icons (16 from the reference + 5 new)
  src/lib.rs                       module list, Callback aliases
  src/theme/mod.rs                 RoggTheme global, Appearance, Fonts, ActiveRoggTheme
  src/theme/color.rs               Color(0xRRGGBBAA) + gpui conversions
  src/theme/palette.rs             Palette::dark()/light() (reference CSS values)
  src/theme/metrics.rs             Density, Metrics, `size` constants
  src/theme/kit.rs                 mirror RoggTheme into gpui-kit's Theme
  src/icons.rs                     IconName, icon()
  src/assets.rs                    AssetSource: rogg icons + gpui-kit icons
  src/controls/mod.rs              re-exports
  src/controls/buttons.rs          IconButton, FlagButton, ToolButton, TextButton, ActionButton
  src/controls/segmented.rs        Segmented
  src/controls/find_field.rs       FindField (gpui-kit Input inside our box)
  src/controls/select.rs           CompactSelect (gpui-kit Button + dropdown)
  src/titlebar.rs                  TabItem, TabShell, TitleBar
  src/sidebar.rs                   Sidebar, TreeRow, RecentItem, SidebarFooter
  src/chrome.rs                    InfoBar, PaneHeader, StatusBar
  src/filter.rs                    ChipState, FilterChip, FilterBand
  src/logview/mod.rs               re-exports, LogView wrapper
  src/logview/model.rs             LogRow, LogLine, Span, SpanStyle, RowTint, LogRowSource, VecRows
  src/logview/text.rs              display_text(), segments() (pure)
  src/logview/scroll.rs            ScrollAnchor, ScrollGeometry (pure)
  src/logview/state.rs             LogViewState entity + events
  src/logview/element.rs           LogViewElement (custom gpui Element)
  src/empty.rs                     EmptyState + no_matches/welcome/loading/error
  src/dialogs.rs                   DialogFrame, ModalLayer, rows, catalog constructors
  src/layout.rs                    MainWindowLayout
  tests/common/mod.rs              headless test window helpers
  tests/*.rs                       per-component layout/behavior tests
crates/rogg-gallery/
  Cargo.toml
  src/main.rs                      CLI: interactive or --capture
  src/lib.rs                       modules
  src/gallery.rs                   GalleryView (nav, theme/density switch, page host)
  src/fixtures.rs                  canned data for component pages
  src/pages.rs                     one render fn per page
  src/replica/mod.rs               re-exports
  src/replica/data.rs              port of the reference sample data + filter logic
  src/replica/state.rs             ReplicaState (interactive mockup)
  src/replica/view.rs              MainWindowLayout built from ReplicaState
  src/capture.rs                   headless PNG + geometry JSON
  tests/pages.rs                   every page renders
  tests/capture.rs                 capture subprocess smoke test
tools/visual-diff/
  package.json, config.mjs, reference.mjs, gallery.mjs, compare.mjs, report.mjs, check.mjs
  golden/                          approved captures (Task 17)
docs/superpowers/reviews/2026-10-06-m0-visual-review.md   accepted differences log (Task 17)
```

---

### Task 1: Toolchain, workspace and smoke window

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `justfile`
- Create: `crates/rogg-ui/Cargo.toml`, `crates/rogg-ui/src/lib.rs`
- Create: `crates/rogg-gallery/Cargo.toml`, `crates/rogg-gallery/src/main.rs`, `crates/rogg-gallery/src/lib.rs`
- Modify: `.gitignore`

**Interfaces:**
- Produces: workspace crates `rogg-ui` (lib) and `rogg-gallery` (lib + bin `rogg-gallery`); env var `ROGG_GALLERY_SMOKE=1` makes the gallery quit after 1.5 s and print `smoke ok`.

- [ ] **Step 1: Put the existing toolchain on PATH and install `just`**

Homebrew's `rustup` is already installed but keg-only, and toolchains `stable` and `1.99.0` already exist in `~/.rustup`.

```bash
grep -q 'opt/rustup/bin' ~/.zprofile 2>/dev/null || echo 'export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"' >> ~/.zprofile
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
rustup toolchain list
brew list just >/dev/null 2>&1 || brew install just
cargo --version && just --version
```
Expected: the toolchain list includes `1.99.0-aarch64-apple-darwin`; cargo and just print their versions.

- [ ] **Step 2: Write the workspace files**

`Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["crates/rogg-ui", "crates/rogg-gallery"]

[workspace.package]
version = "0.1.0"
edition = "2024"
publish = false

[workspace.dependencies]
gpui-kit = "=0.7.1"
rust-embed = { version = "8.7.2", features = ["include-exclude"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "1.99.0"
components = ["rustfmt", "clippy"]
```

`crates/rogg-ui/Cargo.toml`:
```toml
[package]
name = "rogg-ui"
version.workspace = true
edition.workspace = true
publish.workspace = true

[dependencies]
gpui-kit.workspace = true
rust-embed.workspace = true

[dev-dependencies]
gpui-kit = { workspace = true, features = ["test-support"] }
```

`crates/rogg-gallery/Cargo.toml`:
```toml
[package]
name = "rogg-gallery"
version.workspace = true
edition.workspace = true
publish.workspace = true

[dependencies]
rogg-ui = { path = "../rogg-ui" }
# test-support provides HeadlessAppContext + capture_screenshot + element snapshots for --capture (Task 15).
gpui-kit = { workspace = true, features = ["test-support"] }
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true

[dev-dependencies]
gpui-kit = { workspace = true, features = ["test-support"] }
```

`crates/rogg-ui/src/lib.rs`:
```rust
//! rogg's design system: theme, icons and components shared by the app and the gallery.

/// Crate version, used by the gallery title.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::version().is_empty());
    }
}
```

`crates/rogg-gallery/src/lib.rs`:
```rust
//! rogg component gallery: every rogg-ui component in every state, plus a rebuild of the reference mockup.
```

`crates/rogg-gallery/src/main.rs`:
```rust
use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, Context, QuitMode, Window, WindowBounds, WindowOptions, div, px, rgb, size};
use std::time::Duration;

struct Hello;

impl Render for Hello {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x191c1f))
            .text_color(rgb(0xe2e6e9))
            .child(format!("rogg gallery {}", rogg_ui::version()))
    }
}

fn main() {
    gpui_kit::application()
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            gpui_kit::open_window(
                WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
                cx,
                |_, cx| cx.new(|_| Hello),
            )
            .expect("open gallery window");
            cx.activate(true);
            if std::env::var_os("ROGG_GALLERY_SMOKE").is_some() {
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(Duration::from_millis(1500)).await;
                    println!("smoke ok");
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
        });
}
```

`justfile`:
```just
set shell := ["zsh", "-cu"]

default: test

test:
    cargo test --workspace

gallery:
    cargo run -p rogg-gallery

smoke:
    ROGG_GALLERY_SMOKE=1 cargo run -p rogg-gallery
```

Append to `.gitignore`:
```
tools/visual-diff/node_modules/
target/visual/
```

- [ ] **Step 3: Build and run the tests**

Run: `cargo test --workspace`
Expected: the first build compiles gpui (1–3 min), then `test tests::version_is_set ... ok`.

- [ ] **Step 4: Smoke-run the window**

Run: `just smoke`
Expected: a window opens for about 1.5 s, the terminal prints `smoke ok`, and the process exits with status 0. If it panics with a Metal/shader error, stop and report it; it means gpui-kit's `runtime_shaders` assumption (research notes §2) does not hold.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock rust-toolchain.toml justfile .gitignore crates docs/superpowers/research
git commit -m "build: workspace with rogg-ui and rogg-gallery smoke window

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Theme tokens from the reference CSS

**Files:**
- Create: `crates/rogg-ui/src/theme/{mod.rs,color.rs,palette.rs,metrics.rs,kit.rs}`
- Modify: `crates/rogg-ui/src/lib.rs`
- Test: `crates/rogg-ui/tests/theme_tokens.rs`, `crates/rogg-ui/tests/theme_install.rs`

**Interfaces:**
- Produces:
  - `theme::Color(pub u32)` with `Color::rgb(u32)`, `Color::rgba(u32)`, `Color::TRANSPARENT`, `.hsla()`, `.with_alpha(u8)`, `Color::parse_css(&str) -> Option<Color>`; `From<Color>` for `Hsla`, `Fill`, `Background`
  - `theme::Palette` (Copy) with the fields below, `Palette::dark()`, `Palette::light()`, `Palette::named() -> Vec<(&'static str, Color)>`
  - `theme::{HitColor { fg, bg }, ChipColor { border, bg, term }}`
  - `theme::Density { Compact, Comfortable }` with `.metrics() -> Metrics { row_height, log_font_size, pane_header_height, pin_top: Pixels }`
  - `theme::size::*` f32 constants
  - `theme::{Appearance { Dark, Light }, Fonts { ui, mono, ui_size }, RoggTheme { appearance, density, palette, metrics, fonts }}`; `RoggTheme::new(Appearance, Density)`, `RoggTheme::install(self, &mut App)`
  - trait `ActiveRoggTheme { fn rogg(&self) -> &RoggTheme }` for `App`
  - consts `theme::{UI_FONT, MONO_FONT}`

- [ ] **Step 1: Write the failing token test**

`crates/rogg-ui/tests/theme_tokens.rs`:
```rust
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
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p rogg-ui --test theme_tokens`
Expected: compile error `unresolved import rogg_ui::theme`.

- [ ] **Step 3: Implement the theme module**

`crates/rogg-ui/src/theme/color.rs`:
```rust
use gpui_kit::{Background, Fill, Hsla, rgba};

/// An sRGB color stored as `0xRRGGBBAA`, so tests can compare exact values.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Color(pub u32);

impl Color {
    pub const TRANSPARENT: Color = Color(0);

    pub const fn rgb(hex: u32) -> Self {
        Color((hex << 8) | 0xff)
    }

    pub const fn rgba(hex: u32) -> Self {
        Color(hex)
    }

    pub const fn with_alpha(self, alpha: u8) -> Self {
        Color((self.0 & 0xffff_ff00) | alpha as u32)
    }

    pub fn hsla(self) -> Hsla {
        rgba(self.0).into()
    }

    /// Parses `#rgb`, `#rgba`, `#rrggbb` and `#rrggbbaa`.
    pub fn parse_css(s: &str) -> Option<Self> {
        let hex = s.strip_prefix('#')?;
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let doubled = |h: &str| h.chars().flat_map(|c| [c, c]).collect::<String>();
        let full = match hex.len() {
            3 => format!("{}ff", doubled(hex)),
            4 => doubled(hex),
            6 => format!("{hex}ff"),
            8 => hex.to_string(),
            _ => return None,
        };
        u32::from_str_radix(&full, 16).ok().map(Color)
    }
}

impl From<Color> for Hsla {
    fn from(c: Color) -> Self {
        c.hsla()
    }
}

impl From<Color> for Background {
    fn from(c: Color) -> Self {
        c.hsla().into()
    }
}

impl From<Color> for Fill {
    fn from(c: Color) -> Self {
        c.hsla().into()
    }
}

#[cfg(test)]
mod tests {
    use super::Color;

    #[test]
    fn parses_all_css_hex_forms() {
        assert_eq!(Color::parse_css("#191c1f"), Some(Color(0x191c1fff)));
        assert_eq!(Color::parse_css("#8f352e10"), Some(Color(0x8f352e10)));
        assert_eq!(Color::parse_css("#0007"), Some(Color(0x00000077)));
        assert_eq!(Color::parse_css("#fff"), Some(Color(0xffffffff)));
        assert_eq!(Color::parse_css("191c1f"), None);
        assert_eq!(Color::parse_css("#12"), None);
        assert_eq!(Color::parse_css("#zzzzzz"), None);
    }

    #[test]
    fn rgb_sets_opaque_alpha_and_with_alpha_replaces_it() {
        assert_eq!(Color::rgb(0xa8c5b7), Color(0xa8c5b7ff));
        assert_eq!(Color::rgb(0xa8c5b7).with_alpha(0x80), Color(0xa8c5b780));
    }
}
```

`crates/rogg-ui/src/theme/palette.rs`:
```rust
use super::color::Color;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HitColor {
    pub fg: Color,
    pub bg: Color,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChipColor {
    pub border: Color,
    pub bg: Color,
    pub term: Color,
}

/// Every color the UI paints, transcribed from the reference CSS (`:root`, `.light`, and component rules).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Palette {
    pub page: Color,
    pub bg: Color,
    pub side: Color,
    pub raised: Color,
    pub hover: Color,
    pub border: Color,
    pub line: Color,
    pub text: Color,
    pub log_text: Color,
    pub muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent_bg: Color,
    pub accent_border: Color,
    pub error: Color,
    pub error_bg: Color,
    pub amber: Color,
    pub amber_bg: Color,
    pub selection: Color,
    pub error_row_tint: Color,
    pub band_border: Color,
    pub band_handle: Color,
    pub scrollbar_thumb: Color,
    pub chip_divider: Color,
    pub swatch_border: Color,
    pub backdrop: Color,
    pub dialog_shadow: Color,
    pub traffic: [Color; 3],
    pub hits: [HitColor; 4],
    pub chips: [ChipColor; 4],
}

const fn hit(fg: u32, bg: u32) -> HitColor {
    HitColor { fg: Color::rgb(fg), bg: Color::rgb(bg) }
}

const fn chip(border: u32, bg: u32, term: u32) -> ChipColor {
    ChipColor { border: Color::rgb(border), bg: Color::rgb(bg), term: Color::rgb(term) }
}

impl Palette {
    pub const fn dark() -> Self {
        Palette {
            page: Color::rgb(0x111315),
            bg: Color::rgb(0x191c1f),
            side: Color::rgb(0x1e2125),
            raised: Color::rgb(0x25292e),
            hover: Color::rgb(0x2c3137),
            border: Color::rgb(0x34393f),
            line: Color::rgb(0x292e33),
            text: Color::rgb(0xe2e6e9),
            log_text: Color::rgb(0xbbc5ce),
            muted: Color::rgb(0x929da7),
            faint: Color::rgb(0x67737e),
            accent: Color::rgb(0xa8c5b7),
            accent_bg: Color::rgb(0x263e34),
            accent_border: Color::rgb(0x50665c),
            error: Color::rgb(0xefaaa2),
            error_bg: Color::rgb(0x49302f),
            amber: Color::rgb(0xe0c283),
            amber_bg: Color::rgb(0x443b28),
            selection: Color::rgb(0x263844),
            error_row_tint: Color::rgba(0x8f352e10),
            band_border: Color::rgb(0x49534f),
            band_handle: Color::rgb(0x65776e),
            scrollbar_thumb: Color::rgb(0x4b545d),
            chip_divider: Color::rgba(0xffffff15),
            swatch_border: Color::rgba(0xffffff20),
            backdrop: Color::rgba(0x00000077),
            dialog_shadow: Color::rgba(0x00000077),
            traffic: [Color::rgb(0xec6a60), Color::rgb(0xe5bb55), Color::rgb(0x69bd63)],
            hits: [
                hit(0xffc4bb, 0x744037),
                hit(0xf2d693, 0x64502b),
                hit(0xacd7ea, 0x315064),
                hit(0xcbb5eb, 0x514169),
            ],
            chips: [
                chip(0x67514b, 0x362b29, 0xefaaa2),
                chip(0x64583d, 0x342f24, 0xe0c283),
                chip(0x415762, 0x263037, 0xa2c9d8),
                chip(0x5c4c6c, 0x302a37, 0xc2acd9),
            ],
        }
    }

    /// Light values come from `.light{...}` and `.light ...` rules; anything the reference leaves unset reuses dark.
    pub const fn light() -> Self {
        let dark = Palette::dark();
        Palette {
            page: Color::rgb(0xe9ecea),
            bg: Color::rgb(0xfcfcfa),
            side: Color::rgb(0xf2f4f1),
            raised: Color::rgb(0xe7ebe7),
            hover: Color::rgb(0xe7ece7),
            border: Color::rgb(0xd1d8d1),
            line: Color::rgb(0xe5e9e3),
            text: Color::rgb(0x29372d),
            log_text: Color::rgb(0x435047),
            muted: Color::rgb(0x69766c),
            faint: Color::rgb(0x849086),
            accent: Color::rgb(0x386a4f),
            accent_bg: Color::rgb(0xe0ece3),
            error: Color::rgb(0xa44437),
            amber: Color::rgb(0x9a6c20),
            selection: Color::rgb(0xe1ece6),
            hits: [
                hit(0x763227, 0xf1c7bc),
                hit(0x714c16, 0xead49b),
                hit(0x325e70, 0xbfdae6),
                hit(0x624580, 0xd9c9eb),
            ],
            chips: [
                chip(0xdcc4bc, 0xf8eae6, 0xa44437),
                chip(0xd8cba6, 0xf7f0dd, 0x9a6c20),
                chip(0xb9d0da, 0xe7f0f4, 0x426b7b),
                chip(0xcdbdda, 0xece5f5, 0x776083),
            ],
            ..dark
        }
    }

    /// All tokens with display names, for the gallery's token page.
    pub fn named(&self) -> Vec<(&'static str, Color)> {
        let mut v = vec![
            ("page", self.page), ("bg", self.bg), ("side", self.side), ("raised", self.raised),
            ("hover", self.hover), ("border", self.border), ("line", self.line), ("text", self.text),
            ("log text", self.log_text), ("muted", self.muted), ("faint", self.faint), ("accent", self.accent),
            ("accent-bg", self.accent_bg), ("accent-border", self.accent_border), ("error", self.error),
            ("error-bg", self.error_bg), ("amber", self.amber), ("amber-bg", self.amber_bg),
            ("selection", self.selection), ("error row tint", self.error_row_tint),
            ("band border", self.band_border), ("band handle", self.band_handle),
            ("scrollbar thumb", self.scrollbar_thumb), ("chip divider", self.chip_divider),
            ("swatch border", self.swatch_border), ("backdrop", self.backdrop),
            ("dialog shadow", self.dialog_shadow), ("traffic red", self.traffic[0]),
            ("traffic yellow", self.traffic[1]), ("traffic green", self.traffic[2]),
        ];
        const HIT: [(&str, &str); 4] = [("hit-0 fg", "hit-0 bg"), ("hit-1 fg", "hit-1 bg"), ("hit-2 fg", "hit-2 bg"), ("hit-3 fg", "hit-3 bg")];
        const CHIP: [(&str, &str, &str); 4] = [
            ("chip-0 border", "chip-0 bg", "chip-0 term"),
            ("chip-1 border", "chip-1 bg", "chip-1 term"),
            ("chip-2 border", "chip-2 bg", "chip-2 term"),
            ("chip-3 border", "chip-3 bg", "chip-3 term"),
        ];
        for (i, (fg, bg)) in HIT.iter().enumerate() {
            v.push((fg, self.hits[i].fg));
            v.push((bg, self.hits[i].bg));
        }
        for (i, (b, bg, t)) in CHIP.iter().enumerate() {
            v.push((b, self.chips[i].border));
            v.push((bg, self.chips[i].bg));
            v.push((t, self.chips[i].term));
        }
        v
    }
}
```

`crates/rogg-ui/src/theme/metrics.rs`:
```rust
use gpui_kit::{Pixels, px};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Density {
    #[default]
    Compact,
    Comfortable,
}

/// Sizes that change with density (reference `.comfortable{--row:29px;--log-font:13px}` and pane-head overrides).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Metrics {
    pub row_height: Pixels,
    pub log_font_size: Pixels,
    pub pane_header_height: Pixels,
    /// Top padding of the pin glyph inside a row (`.row .pin{padding-top}`).
    pub pin_top: Pixels,
}

impl Density {
    pub fn metrics(self) -> Metrics {
        match self {
            Density::Compact => Metrics { row_height: px(23.), log_font_size: px(12.), pane_header_height: px(35.), pin_top: px(5.) },
            Density::Comfortable => Metrics { row_height: px(29.), log_font_size: px(13.), pane_header_height: px(39.), pin_top: px(8.) },
        }
    }
}

/// Fixed sizes from the reference CSS, in logical px.
pub mod size {
    pub const TITLEBAR_H: f32 = 42.;
    pub const SIDEBAR_W: f32 = 211.;
    pub const SIDEBAR_MIN_W: f32 = 155.;
    pub const SIDEBAR_MAX_W: f32 = 360.;
    pub const SIDE_TOP_H: f32 = 43.;
    pub const TREE_ROW_H: f32 = 30.;
    pub const INFO_BAR_H: f32 = 33.;
    pub const STATUS_BAR_H: f32 = 29.;
    pub const TAB_H: f32 = 29.;
    pub const TAB_MAX_W: f32 = 152.;
    pub const TAB_LABEL_MAX_W: f32 = 127.;
    pub const ICON_BTN: f32 = 27.;
    pub const TITLEBAR_ICON_BTN: f32 = 25.;
    pub const FLAG_H: f32 = 27.;
    pub const FLAG_MIN_W: f32 = 28.;
    pub const TOOL_H: f32 = 25.;
    pub const FIND_H: f32 = 24.;
    pub const FIND_INPUT_W: f32 = 112.;
    pub const CHIP_H: f32 = 28.;
    pub const FILTER_LINE_MIN_H: f32 = 30.;
    pub const SELECT_H: f32 = 19.;
    pub const GUTTER_PIN_W: f32 = 24.;
    pub const GUTTER_NUMBER_W: f32 = 65.;
    pub const NUMBER_PAD_R: f32 = 12.;
    pub const LINE_PAD_R: f32 = 20.;
    pub const LOGS_PAD_Y: f32 = 4.;
    pub const PIN_GLYPH: f32 = 11.;
    pub const GAP_ROW_H: f32 = 17.;
    pub const GAP_ROW_INSET: f32 = 89.;
    pub const SCROLL_THUMB_W: f32 = 6.;
    pub const SCROLL_THUMB_MIN_H: f32 = 24.;
    pub const DIALOG_W: f32 = 500.;
    pub const DIALOG_PAD: f32 = 22.;
}
```

`crates/rogg-ui/src/theme/kit.rs`:
```rust
use super::{Appearance, RoggTheme};
use gpui_kit::component::{Theme as KitTheme, ThemeMode as KitMode};
use gpui_kit::{App, px};

/// Mirrors rogg colors and fonts into gpui-kit so its Input, menus and tooltips match the design.
/// `Theme::change` reloads kit defaults, so the overrides are applied afterwards with `Theme::update`.
pub(super) fn apply(theme: &RoggTheme, cx: &mut App) {
    let mode = match theme.appearance {
        Appearance::Dark => KitMode::Dark,
        Appearance::Light => KitMode::Light,
    };
    KitTheme::change(mode, None, cx);
    let p = theme.palette;
    let ui = theme.fonts.ui.clone();
    let mono = theme.fonts.mono.clone();
    KitTheme::update(cx, move |t| {
        t.font_family = ui;
        t.font_size = px(13.);
        t.mono_font_family = mono;
        t.mono_font_size = px(12.);
        t.radius = px(5.);
        t.radius_lg = px(10.);
        t.shadow = false;
        t.colors.background = p.bg.hsla();
        t.colors.foreground = p.text.hsla();
        t.colors.border = p.border.hsla();
        t.colors.input = p.border.hsla();
        t.colors.muted = p.raised.hsla();
        t.colors.muted_foreground = p.faint.hsla();
        t.colors.popover = p.side.hsla();
        t.colors.accent = p.hover.hsla();
        t.colors.selection = p.selection.hsla();
        t.colors.ring = p.accent.hsla();
        t.colors.primary = p.accent.hsla();
    });
}
```

`crates/rogg-ui/src/theme/mod.rs`:
```rust
mod color;
mod kit;
mod metrics;
mod palette;

pub use color::Color;
pub use metrics::{Density, Metrics, size};
pub use palette::{ChipColor, HitColor, Palette};

use gpui_kit::{App, Global, Pixels, SharedString, px};

/// System UI font (SF Pro).
pub const UI_FONT: &str = ".SystemUIFont";
/// System monospaced font (SF Mono). The family name "SF Mono" silently falls back to Helvetica in gpui.
pub const MONO_FONT: &str = ".AppleSystemUIFontMonospaced";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Appearance {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Fonts {
    pub ui: SharedString,
    pub mono: SharedString,
    pub ui_size: Pixels,
}

#[derive(Clone, PartialEq, Debug)]
pub struct RoggTheme {
    pub appearance: Appearance,
    pub density: Density,
    pub palette: Palette,
    pub metrics: Metrics,
    pub fonts: Fonts,
}

impl Global for RoggTheme {}

impl RoggTheme {
    pub fn new(appearance: Appearance, density: Density) -> Self {
        let palette = match appearance {
            Appearance::Dark => Palette::dark(),
            Appearance::Light => Palette::light(),
        };
        Self {
            appearance,
            density,
            palette,
            metrics: density.metrics(),
            fonts: Fonts { ui: UI_FONT.into(), mono: MONO_FONT.into(), ui_size: px(13.) },
        }
    }

    /// Makes this the active theme and mirrors it into gpui-kit. Call after `gpui_kit::init`.
    pub fn install(self, cx: &mut App) {
        kit::apply(&self, cx);
        cx.set_global(self);
        cx.refresh_windows();
    }
}

pub trait ActiveRoggTheme {
    fn rogg(&self) -> &RoggTheme;
}

impl ActiveRoggTheme for App {
    fn rogg(&self) -> &RoggTheme {
        self.global::<RoggTheme>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_picks_palette_and_metrics() {
        let t = RoggTheme::new(Appearance::Light, Density::Comfortable);
        assert_eq!(t.palette, Palette::light());
        assert_eq!(t.metrics, Density::Comfortable.metrics());
        assert_eq!(t.fonts.mono.as_ref(), MONO_FONT);
    }
}
```

Replace `crates/rogg-ui/src/lib.rs` with:
```rust
//! rogg's design system: theme, icons and components shared by the app and the gallery.

pub mod theme;

pub use theme::{ActiveRoggTheme, RoggTheme};

/// Crate version, used by the gallery title.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
```

- [ ] **Step 4: Run the token tests**

Run: `cargo test -p rogg-ui`
Expected: all of these pass: `theme_tokens` (3 tests), `color` (2 tests) and `new_picks_palette_and_metrics`. If a token assertion fails, the palette constant is wrong: fix the constant, never the test.

- [ ] **Step 5: Write the install test**

`crates/rogg-ui/tests/theme_install.rs`:
```rust
use gpui_kit::TestAppContext;
use gpui_kit::component::Theme as KitTheme;
use rogg_ui::theme::{ActiveRoggTheme, Appearance, Density, Palette, RoggTheme};

#[gpui_kit::test]
fn install_sets_global_and_kit_mode(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        RoggTheme::new(Appearance::Light, Density::Compact).install(cx);
        assert_eq!(cx.rogg().palette, Palette::light());
        assert!(!KitTheme::global(cx).mode.is_dark());
        RoggTheme::new(Appearance::Dark, Density::Comfortable).install(cx);
        assert_eq!(cx.rogg().palette, Palette::dark());
        assert!(KitTheme::global(cx).mode.is_dark());
        assert_eq!(KitTheme::global(cx).font_family.as_ref(), ".SystemUIFont");
    });
}
```

Run: `cargo test -p rogg-ui --test theme_install`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): theme tokens verified against the reference CSS

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Icons and asset source

**Files:**
- Create: `crates/rogg-ui/assets/icons/rogg/*.svg` (21 files)
- Create: `crates/rogg-ui/src/icons.rs`, `crates/rogg-ui/src/assets.rs`
- Modify: `crates/rogg-ui/src/lib.rs`
- Test: `crates/rogg-ui/tests/assets.rs`

**Interfaces:**
- Produces:
  - `icons::IconName` with variants `File, Folder, Search, Wrap, Pin, PinFilled, Filter, Follow, Check, Off, Minus, History, Edit, Export, Sun, Settings, Side, OpenTab, Pause, Close, ChevronDown`; `IconName::ALL: [IconName; 21]`; `IconName::path(self) -> &'static str`
  - `icons::icon(IconName) -> Svg`. Callers must set `.size(..)` and `.text_color(..)`, because gpui svgs paint nothing without `text_color`.
  - `assets::Assets` (an `AssetSource`)

gpui renders SVGs as one-color alpha masks. The reference `settings` icon uses circles filled with the background color to break its lines, which would come out solid. The version below splits the lines around the circles instead. `pin-filled` replaces the CSS `fill:currentColor; stroke-width:1` pinned state.

- [ ] **Step 1: Write the failing asset test**

`crates/rogg-ui/tests/assets.rs`:
```rust
use gpui_kit::AssetSource;
use rogg_ui::assets::Assets;
use rogg_ui::icons::IconName;

#[test]
fn every_icon_is_embedded_and_mask_safe() {
    for icon in IconName::ALL {
        let bytes = Assets.load(icon.path()).unwrap().unwrap_or_else(|| panic!("{icon:?} missing"));
        let svg = std::str::from_utf8(&bytes).unwrap();
        assert!(svg.contains(r#"viewBox="0 0 24 24""#), "{icon:?} viewBox");
        assert!(!svg.contains("var("), "{icon:?} uses a CSS variable; masks cannot resolve it");
    }
}

#[test]
fn icon_paths_are_unique() {
    let mut paths: Vec<_> = IconName::ALL.iter().map(|i| i.path()).collect();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), IconName::ALL.len());
}

#[test]
fn kit_icons_still_resolve_and_list_includes_ours() {
    assert!(Assets.load("icons/search.svg").unwrap().is_some());
    let listed = Assets.list("icons/rogg/").unwrap();
    assert_eq!(listed.len(), IconName::ALL.len());
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p rogg-ui --test assets`
Expected: compile error, `rogg_ui::assets` is not found.

- [ ] **Step 3: Write the SVG files**

```bash
mkdir -p crates/rogg-ui/assets/icons/rogg
mk() { printf '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">%s</svg>\n' "$2" > "crates/rogg-ui/assets/icons/rogg/$1.svg"; }
mk file '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6M8 13h8M8 17h6"/>'
mk folder '<path d="M3 7V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>'
mk search '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>'
mk wrap '<path d="M3 5h18M3 10h13a4 4 0 0 1 0 8h-5m0 0 3-3m-3 3 3 3M3 16h3"/>'
mk pin '<path d="m16 3 5 5-4 1-3 5v3l-3-3-6 6m6-6-4-4h3l5-3z"/>'
mk filter '<path d="M3 4h18l-7 8v7l-4 2v-9z"/>'
mk follow '<path d="M5 20V7a3 3 0 0 1 3-3h11m-4-3 4 3-4 3M11 13h9M11 18h9"/>'
mk check '<path d="m5 12 4 4L19 6"/>'
mk off '<circle cx="12" cy="12" r="7"/>'
mk minus '<circle cx="12" cy="12" r="8"/><path d="M8 12h8"/>'
mk history '<path d="M3 10a9 9 0 1 1 2 9M3 4v6h6m3-4v6l4 2"/>'
mk edit '<path d="m15 4 5 5M3 21l5-1L21 7l-5-5L3 15z"/>'
mk export '<path d="M12 15V3m-4 4 4-4 4 4M4 14v7h16v-7"/>'
mk sun '<circle cx="12" cy="12" r="4"/><path d="M12 1v2m0 18v2M1 12h2m18 0h2M4 4l2 2m12 12 2 2M4 20l2-2M18 6l2-2"/>'
mk settings '<path d="M4 7h2M12 7h8M4 17h8M18 17h2"/><circle cx="9" cy="7" r="3"/><circle cx="15" cy="17" r="3"/>'
mk side '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/>'
mk open-tab '<path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>'
mk pause '<path d="M9 5v14M15 5v14"/>'
mk close '<path d="M6 6l12 12M18 6 6 18"/>'
mk chevron-down '<path d="m6 9 6 6 6-6"/>'
printf '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#000" stroke="#000" stroke-width="1" stroke-linecap="round" stroke-linejoin="round"><path d="m16 3 5 5-4 1-3 5v3l-3-3-6 6m6-6-4-4h3l5-3z"/></svg>\n' > crates/rogg-ui/assets/icons/rogg/pin-filled.svg
ls crates/rogg-ui/assets/icons/rogg | wc -l
```
Expected: `21`.

- [ ] **Step 4: Implement `icons.rs` and `assets.rs`**

`crates/rogg-ui/src/icons.rs`:
```rust
use gpui_kit::prelude::*;
use gpui_kit::{Svg, svg};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IconName {
    File,
    Folder,
    Search,
    Wrap,
    Pin,
    PinFilled,
    Filter,
    Follow,
    Check,
    Off,
    Minus,
    History,
    Edit,
    Export,
    Sun,
    Settings,
    Side,
    OpenTab,
    Pause,
    Close,
    ChevronDown,
}

impl IconName {
    pub const ALL: [IconName; 21] = [
        IconName::File, IconName::Folder, IconName::Search, IconName::Wrap, IconName::Pin, IconName::PinFilled,
        IconName::Filter, IconName::Follow, IconName::Check, IconName::Off, IconName::Minus, IconName::History,
        IconName::Edit, IconName::Export, IconName::Sun, IconName::Settings, IconName::Side, IconName::OpenTab,
        IconName::Pause, IconName::Close, IconName::ChevronDown,
    ];

    pub fn path(self) -> &'static str {
        match self {
            IconName::File => "icons/rogg/file.svg",
            IconName::Folder => "icons/rogg/folder.svg",
            IconName::Search => "icons/rogg/search.svg",
            IconName::Wrap => "icons/rogg/wrap.svg",
            IconName::Pin => "icons/rogg/pin.svg",
            IconName::PinFilled => "icons/rogg/pin-filled.svg",
            IconName::Filter => "icons/rogg/filter.svg",
            IconName::Follow => "icons/rogg/follow.svg",
            IconName::Check => "icons/rogg/check.svg",
            IconName::Off => "icons/rogg/off.svg",
            IconName::Minus => "icons/rogg/minus.svg",
            IconName::History => "icons/rogg/history.svg",
            IconName::Edit => "icons/rogg/edit.svg",
            IconName::Export => "icons/rogg/export.svg",
            IconName::Sun => "icons/rogg/sun.svg",
            IconName::Settings => "icons/rogg/settings.svg",
            IconName::Side => "icons/rogg/side.svg",
            IconName::OpenTab => "icons/rogg/open-tab.svg",
            IconName::Pause => "icons/rogg/pause.svg",
            IconName::Close => "icons/rogg/close.svg",
            IconName::ChevronDown => "icons/rogg/chevron-down.svg",
        }
    }
}

/// An icon element. Always set `.size(..)` and `.text_color(..)`: gpui svgs paint only with an explicit text color.
pub fn icon(name: IconName) -> Svg {
    svg().path(name.path()).flex_shrink_0()
}
```

`crates/rogg-ui/src/assets.rs`:
```rust
use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
struct Embedded;

/// rogg's icons first, then gpui-kit's default icons. The kit source errors on unknown paths, so it must come last.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(file) = Embedded::get(path) {
            return Ok(Some(file.data));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out: Vec<SharedString> = Embedded::iter()
            .filter(|p| p.starts_with(path))
            .map(|p| SharedString::from(p.to_string()))
            .collect();
        if !path.starts_with("icons/rogg") {
            out.extend(gpui_kit::assets::Assets.list(path)?);
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}
```

Add to `crates/rogg-ui/src/lib.rs` after `pub mod theme;`:
```rust
pub mod assets;
pub mod icons;
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rogg-ui --test assets`
Expected: 3 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): mask-safe icon set and combined asset source

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 4: Test harness and primitive controls

**Files:**
- Create: `crates/rogg-ui/tests/common/mod.rs`
- Create: `crates/rogg-ui/src/controls/{mod.rs,buttons.rs,segmented.rs,find_field.rs,select.rs}`
- Modify: `crates/rogg-ui/src/lib.rs`
- Test: `crates/rogg-ui/tests/controls.rs`

**Interfaces:**
- Consumes: `theme::*`, `icons::{IconName, icon}` (Tasks 2–3).
- Produces (all in `rogg_ui::controls`, and all `RenderOnce + IntoElement`):
  - `IconButton::icon(id, IconName)`, `IconButton::glyph(id, impl Into<SharedString>)`, `.compact()` (25 px title-bar size), `.force_hover(bool)`, `.on_click(Fn(&mut Window, &mut App))`
  - `FlagButton::new(id, label)`, `.pressed(bool)`, `.on_click(..)`
  - `ToolButton::new(id, IconName, label)`, `.pressed(bool)`, `.badge(Option<SharedString>)`, `.on_click(..)`
  - `TextButton::meta(id, label)` (11 px), `TextButton::small(id, label)` (10 px), `.icon(IconName)`, `.dot()`, `.on_click(..)`
  - `ActionButton::new(id, label)`, `.primary()`, `.on_click(..)`
  - `Segmented::new(id, Vec<SharedString>, selected)`, `.on_select(Fn(usize, &mut Window, &mut App))`
  - `FindField::new(id, Entity<InputState>)`, `.count(Option<SharedString>)`; `FindField::state(placeholder, &mut Window, &mut App) -> Entity<InputState>`
  - `CompactSelect::new(id, Vec<SharedString>, selected)`, `.on_select(..)`
- In `rogg_ui`: `pub type Callback = Rc<dyn Fn(&mut Window, &mut App)>`, `pub type IndexCallback = Rc<dyn Fn(usize, &mut Window, &mut App)>`.

- [ ] **Step 1: Write the shared test harness**

`crates/rogg-ui/tests/common/mod.rs`:
```rust
//! Headless test windows. gpui's TestAppContext uses a no-op text system, so text has zero size:
//! assert only on sizes that the reference fixes in CSS (heights, fixed widths, paddings).
#![allow(dead_code)]
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AnyWindowHandle, App, Bounds, Context, Pixels, Point, TestAppContext, Window, WindowBounds, WindowOptions, px,
    size,
};
use rogg_ui::theme::{Appearance, Density, RoggTheme};

pub fn setup(cx: &mut TestAppContext) {
    setup_with(cx, Appearance::Dark, Density::Compact);
}

pub fn setup_with(cx: &mut TestAppContext, appearance: Appearance, density: Density) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        RoggTheme::new(appearance, density).install(cx);
    });
}

/// A view that renders whatever the closure builds; used to host RenderOnce components.
pub struct Host(pub Box<dyn Fn(&mut Window, &mut App) -> AnyElement>);

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        (self.0)(window, cx)
    }
}

pub fn open(cx: &mut TestAppContext, w: f32, h: f32, build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> AnyWindowHandle {
    cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(w), px(h)) })),
            ..Default::default()
        };
        let (handle, _) = gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Host(Box::new(build)))).expect("open window");
        handle
    })
}

pub fn bounds(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) -> Bounds<Pixels> {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.find(id).bounds()
    })
    .unwrap()
}

pub fn exists(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) -> bool {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.try_find(id).is_some()
    })
    .unwrap()
}

pub fn click(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}
```

- [ ] **Step 2: Write the failing control tests**

`crates/rogg-ui/tests/controls.rs`:
```rust
mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, div, px, size};
use rogg_ui::controls::{FindField, FlagButton, IconButton, ToolButton};
use rogg_ui::icons::IconName;
use std::cell::Cell;
use std::rc::Rc;

#[gpui_kit::test]
fn icon_buttons_use_reference_box_sizes(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 400., 100., |_, _| {
        div()
            .flex()
            .child(IconButton::icon("edit", IconName::Edit))
            .child(IconButton::icon("settings", IconName::Settings).compact())
            .child(IconButton::glyph("all-tabs", "⌄").compact())
            .into_any_element()
    });
    // .icon-btn: 15px svg + 6px*2 padding + 1px*2 border = 29 wide; height fixed at 27.
    assert_eq!(common::bounds(cx, w, "edit").size, size(px(29.), px(27.)));
    // title bar variant: 15 + 5*2 + 2 = 27 wide, 25 high.
    assert_eq!(common::bounds(cx, w, "settings").size, size(px(27.), px(25.)));
    assert_eq!(common::bounds(cx, w, "all-tabs").size.height, px(25.));
    assert!(common::bounds(cx, w, "all-tabs").size.width >= px(25.));
}

#[gpui_kit::test]
fn flag_tool_and_find_heights(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 600., 100., |window, cx| {
        let input = FindField::state("Find in full log", window, cx);
        div()
            .flex()
            .child(FlagButton::new("flag-regex", ".*").pressed(true))
            .child(ToolButton::new("tool-wrap", IconName::Wrap, "Wrap"))
            .child(FindField::new("find", input).count(Some("3 hits".into())))
            .into_any_element()
    });
    let flag = common::bounds(cx, w, "flag-regex");
    assert_eq!(flag.size.height, px(27.));
    assert!(flag.size.width >= px(28.));
    assert_eq!(common::bounds(cx, w, "tool-wrap").size.height, px(25.));
    assert_eq!(common::bounds(cx, w, "find").size.height, px(24.));
}

#[gpui_kit::test]
fn click_invokes_callback(cx: &mut TestAppContext) {
    common::setup(cx);
    let clicked = Rc::new(Cell::new(0));
    let c = clicked.clone();
    let w = common::open(cx, 200., 100., move |_, _| {
        let c = c.clone();
        IconButton::icon("edit", IconName::Edit).on_click(move |_, _| c.set(c.get() + 1)).into_any_element()
    });
    common::click(cx, w, "edit");
    assert_eq!(clicked.get(), 1);
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p rogg-ui --test controls`
Expected: compile error, `rogg_ui::controls` is not found.

- [ ] **Step 4: Implement the controls**

Add to `crates/rogg-ui/src/lib.rs`:
```rust
pub mod controls;

use gpui_kit::{App, Window};
use std::rc::Rc;

/// Callback with no payload, used by every component.
pub type Callback = Rc<dyn Fn(&mut Window, &mut App)>;
/// Callback carrying an index (tab, chip, menu item...).
pub type IndexCallback = Rc<dyn Fn(usize, &mut Window, &mut App)>;
```

`crates/rogg-ui/src/controls/mod.rs`:
```rust
mod buttons;
mod find_field;
mod segmented;
mod select;

pub use buttons::{ActionButton, FlagButton, IconButton, TextButton, ToolButton};
pub use find_field::FindField;
pub use segmented::Segmented;
pub use select::CompactSelect;

use crate::theme::Palette;
use gpui_kit::prelude::*;

/// `button[aria-pressed=true]`: accent text, accent-bg fill, accent border. Pressed buttons keep this fill on hover.
pub(crate) fn pressed<E: Styled>(el: E, p: &Palette) -> E {
    el.text_color(p.accent).bg(p.accent_bg).border_color(p.accent_border)
}
```

`crates/rogg-ui/src/controls/buttons.rs`:
```rust
use super::pressed;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, Color, size};
use crate::Callback;
use gpui_kit::prelude::*;
use gpui_kit::{App, FontWeight, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

pub enum ButtonContent {
    Icon(IconName),
    Glyph(SharedString),
}

/// `.icon-btn`: a square-ish ghost button holding a 15px icon or a text glyph (⌄, ⌘, ✕).
#[derive(IntoElement)]
pub struct IconButton {
    id: SharedString,
    content: ButtonContent,
    compact: bool,
    force_hover: bool,
    on_click: Option<Callback>,
}

impl IconButton {
    pub fn icon(id: impl Into<SharedString>, name: IconName) -> Self {
        Self { id: id.into(), content: ButtonContent::Icon(name), compact: false, force_hover: false, on_click: None }
    }

    pub fn glyph(id: impl Into<SharedString>, glyph: impl Into<SharedString>) -> Self {
        Self { id: id.into(), content: ButtonContent::Glyph(glyph.into()), compact: false, force_hover: false, on_click: None }
    }

    /// The 25px title-bar variant (`.title-actions .icon-btn`).
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Render the hover state permanently (gallery specimens).
    pub fn force_hover(mut self, on: bool) -> Self {
        self.force_hover = on;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let (side, pad_x, pad_y) =
            if self.compact { (size::TITLEBAR_ICON_BTN, 5., 3.) } else { (size::ICON_BTN, 6., 4.) };
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(5.))
            .min_w(px(side))
            .h(px(side))
            .px(px(pad_x))
            .py(px(pad_y))
            .rounded(px(5.))
            .border_1()
            .border_color(Color::TRANSPARENT)
            .text_color(p.muted)
            .text_size(px(13.))
            .cursor_pointer()
            .when(self.force_hover, |d| d.bg(p.hover))
            .hover(move |s| s.bg(p.hover))
            .child(match self.content {
                ButtonContent::Icon(name) => icon(name).size(px(15.)).text_color(p.muted).into_any_element(),
                ButtonContent::Glyph(g) => div().child(g).into_any_element(),
            })
            .when_some(self.on_click, |d, f| d.on_click(move |_, window, cx| f(window, cx)))
    }
}

/// `.flag`: the `.*` and `Aa` toggles next to the filter line.
#[derive(IntoElement)]
pub struct FlagButton {
    id: SharedString,
    label: SharedString,
    pressed: bool,
    on_click: Option<Callback>,
}

impl FlagButton {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), pressed: false, on_click: None }
    }

    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for FlagButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(size::FLAG_H))
            .min_w(px(size::FLAG_MIN_W))
            .px(px(6.))
            .py(px(3.))
            .rounded(px(5.))
            .border_1()
            .font_family(t.fonts.mono.clone())
            .text_size(px(13.))
            .cursor_pointer()
            .map(|d| {
                if self.pressed {
                    pressed(d, &p)
                } else {
                    d.text_color(p.muted).border_color(p.border).hover(move |s| s.bg(p.hover))
                }
            })
            .child(self.label)
            .when_some(self.on_click, |d, f| d.on_click(move |_, window, cx| f(window, cx)))
    }
}

/// `.tool`: icon + 10px label buttons in pane headers (Wrap, Follow selection, Open as tab).
#[derive(IntoElement)]
pub struct ToolButton {
    id: SharedString,
    icon: IconName,
    label: SharedString,
    pressed: bool,
    badge: Option<SharedString>,
    on_click: Option<Callback>,
}

impl ToolButton {
    pub fn new(id: impl Into<SharedString>, icon: IconName, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), icon, label: label.into(), pressed: false, badge: None, on_click: None }
    }

    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    /// Small trailing counter, e.g. `+1,204` lines arrived while live follow is paused.
    pub fn badge(mut self, badge: Option<SharedString>) -> Self {
        self.badge = badge;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ToolButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let fg = if self.pressed { p.accent } else { p.muted };
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(5.))
            .h(px(size::TOOL_H))
            .px(px(7.))
            .py(px(3.))
            .rounded(px(5.))
            .border_1()
            .text_size(px(10.))
            .cursor_pointer()
            .map(|d| {
                if self.pressed {
                    pressed(d, &p)
                } else {
                    d.text_color(p.muted).border_color(Color::TRANSPARENT).hover(move |s| s.bg(p.hover))
                }
            })
            .child(icon(self.icon).size(px(13.)).text_color(fg))
            .child(self.label)
            .when_some(self.badge, |d, b| d.child(div().text_color(p.accent).child(b)))
            .when_some(self.on_click, |d, f| d.on_click(move |_, window, cx| f(window, cx)))
    }
}

/// Text buttons: `.meta button` (11px, Profile / Export) and `.filter-bottom button` (10px, History).
#[derive(IntoElement)]
pub struct TextButton {
    id: SharedString,
    label: SharedString,
    icon: Option<IconName>,
    dot: bool,
    text_size: f32,
    icon_size: f32,
    pad: (f32, f32),
    gap: f32,
    on_click: Option<Callback>,
}

impl TextButton {
    pub fn meta(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), icon: None, dot: false, text_size: 11., icon_size: 15., pad: (6., 4.), gap: 5., on_click: None }
    }

    pub fn small(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), icon: None, dot: false, text_size: 10., icon_size: 12., pad: (4., 2.), gap: 4., on_click: None }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Leading 5px accent dot (`.subtle-dot`).
    pub fn dot(mut self) -> Self {
        self.dot = true;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for TextButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(self.gap))
            .px(px(self.pad.0))
            .py(px(self.pad.1))
            .rounded(px(5.))
            .border_1()
            .border_color(Color::TRANSPARENT)
            .text_size(px(self.text_size))
            .text_color(p.muted)
            .cursor_pointer()
            .hover(move |s| s.bg(p.hover))
            .when(self.dot, |d| d.child(div().size(px(5.)).rounded_full().bg(p.accent)))
            .when_some(self.icon, |d, i| d.child(icon(i).size(px(self.icon_size)).text_color(p.muted)))
            .child(self.label)
            .when_some(self.on_click, |d, f| d.on_click(move |_, window, cx| f(window, cx)))
    }
}

/// `.decision button`: bordered 11px buttons for empty states and dialogs; primary uses the accent.
#[derive(IntoElement)]
pub struct ActionButton {
    id: SharedString,
    label: SharedString,
    primary: bool,
    on_click: Option<Callback>,
}

impl ActionButton {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), primary: false, on_click: None }
    }

    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ActionButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        div()
            .id(self.id)
            .test_support()
            .flex_none()
            .px(px(10.))
            .py(px(7.))
            .rounded(px(5.))
            .border_1()
            .text_size(px(11.))
            .font_weight(FontWeight::NORMAL)
            .cursor_pointer()
            .map(|d| if self.primary { d.text_color(p.accent).border_color(p.accent_border) } else { d.text_color(p.text).border_color(p.border) })
            .hover(move |s| s.bg(p.hover))
            .child(self.label)
            .when_some(self.on_click, |d, f| d.on_click(move |_, window, cx| f(window, cx)))
    }
}
```

`crates/rogg-ui/src/controls/segmented.rs`:
```rust
use super::pressed;
use crate::theme::{ActiveRoggTheme, Color};
use crate::IndexCallback;
use gpui_kit::prelude::*;
use gpui_kit::{App, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

/// `.segmented`: bordered group of mutually exclusive options.
#[derive(IntoElement)]
pub struct Segmented {
    id: SharedString,
    options: Vec<SharedString>,
    selected: usize,
    on_select: Option<IndexCallback>,
}

impl Segmented {
    pub fn new(id: impl Into<SharedString>, options: Vec<SharedString>, selected: usize) -> Self {
        Self { id: id.into(), options, selected, on_select: None }
    }

    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let id = self.id.clone();
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .gap(px(3.))
            .p(px(3.))
            .border_1()
            .border_color(p.border)
            .rounded(px(7.))
            .children(self.options.into_iter().enumerate().map(|(i, label)| {
                let on_select = self.on_select.clone();
                div()
                    .id(SharedString::from(format!("{id}-{i}")))
                    .px(px(11.))
                    .py(px(6.))
                    .text_size(px(12.))
                    .rounded(px(5.))
                    .border_1()
                    .cursor_pointer()
                    .map(|d| {
                        if i == self.selected {
                            pressed(d, &p)
                        } else {
                            d.text_color(p.text).border_color(Color::TRANSPARENT).hover(move |s| s.bg(p.hover))
                        }
                    })
                    .child(label)
                    .on_click(move |_, window, cx| {
                        if let Some(f) = &on_select {
                            f(i, window, cx)
                        }
                    })
            }))
    }
}
```

`crates/rogg-ui/src/controls/find_field.rs`:
```rust
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, size};
use gpui_kit::component::Sizable;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, SharedString, TestSupportExt, Window, div, px};

/// `.find`: 24px search box with icon, a borderless gpui-kit Input (112px) and a hit counter.
#[derive(IntoElement)]
pub struct FindField {
    id: SharedString,
    input: Entity<InputState>,
    count: Option<SharedString>,
}

impl FindField {
    pub fn new(id: impl Into<SharedString>, input: Entity<InputState>) -> Self {
        Self { id: id.into(), input, count: None }
    }

    pub fn count(mut self, count: Option<SharedString>) -> Self {
        self.count = count;
        self
    }

    /// Creates the input state a FindField renders.
    pub fn state(placeholder: impl Into<SharedString>, window: &mut Window, cx: &mut App) -> Entity<InputState> {
        let placeholder = placeholder.into();
        cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
    }
}

impl RenderOnce for FindField {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        // Input::h is multi-line only and shadows Styled::h, so call the trait methods explicitly.
        let input = Input::new(&self.input).xsmall().appearance(false).p_0().text_size(px(10.)).line_height(px(14.)).text_color(p.text);
        let input = Styled::w(Styled::h(input, px(size::FIND_H - 2.)), px(size::FIND_INPUT_W));
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .h(px(size::FIND_H))
            .px(px(6.))
            .gap(px(5.))
            .bg(p.side)
            .border_1()
            .border_color(p.border)
            .rounded(px(4.))
            .child(icon(IconName::Search).size(px(12.)).text_color(p.faint))
            .child(input)
            .when_some(self.count, |d, c| d.child(div().flex_none().text_size(px(9.)).text_color(p.faint).whitespace_nowrap().child(c)))
    }
}
```

`crates/rogg-ui/src/controls/select.rs`:
```rust
use crate::theme::{ActiveRoggTheme, size};
use crate::IndexCallback;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::prelude::*;
use gpui_kit::{App, SharedString, Window, px};
use std::rc::Rc;

/// The `Context` select in the filter band: a 19px bordered button opening a gpui-kit dropdown menu.
#[derive(IntoElement)]
pub struct CompactSelect {
    id: SharedString,
    options: Vec<SharedString>,
    selected: usize,
    on_select: Option<IndexCallback>,
}

impl CompactSelect {
    pub fn new(id: impl Into<SharedString>, options: Vec<SharedString>, selected: usize) -> Self {
        Self { id: id.into(), options, selected, on_select: None }
    }

    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for CompactSelect {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let variant = ButtonCustomVariant::new(cx)
            .color(p.bg.hsla())
            .foreground(p.text.hsla())
            .hover(p.hover.hsla())
            .active(p.hover.hsla())
            .shadow(false);
        let label = self.options.get(self.selected).cloned().unwrap_or_default();
        let options = self.options.clone();
        let selected = self.selected;
        let on_select = self.on_select.clone();
        Button::new(self.id)
            .custom(variant)
            .label(label)
            .dropdown_caret(true)
            .h(px(size::SELECT_H))
            .px(px(3.))
            .text_size(px(10.))
            .border_1()
            .border_color(p.border)
            .rounded(px(4.))
            .dropdown_menu(move |menu, _, _| {
                options.iter().enumerate().fold(menu, |menu, (i, option)| {
                    let on_select = on_select.clone();
                    menu.item(PopupMenuItem::new(option.clone()).checked(i == selected).on_click(move |_, window, cx| {
                        if let Some(f) = &on_select {
                            f(i, window, cx)
                        }
                    }))
                })
            })
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rogg-ui --test controls`
Expected: 3 tests pass. If the icon button is 27 wide instead of 29, gpui is applying content-box sizing. Do not change the test; set the width explicitly to the CSS-computed value (`w(px(29.))`).

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): primitive controls (icon/flag/tool/text/action buttons, segmented, find field, compact select)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Title bar and tabs

**Files:**
- Create: `crates/rogg-ui/src/titlebar.rs`
- Modify: `crates/rogg-ui/src/lib.rs` (add `pub mod titlebar;`)
- Test: `crates/rogg-ui/tests/titlebar.rs`

**Interfaces:**
- Consumes: `controls::IconButton`, `icons`, `theme`, `IndexCallback`, `Callback`.
- Produces:
  - `titlebar::TabItem { label: SharedString, tooltip: SharedString, active: bool }`
  - `titlebar::TitleBar::new(tabs: Vec<TabItem>)`, with:
    - `.sidebar_width(Pixels)` (default 211)
    - `.traffic_lights(bool)`: draw fake traffic lights for headless capture and the gallery; the real app uses native ones
    - `.force_hover_tab(Option<usize>)`
    - `.on_select_tab(IndexCallback)`, `.on_close_tab(IndexCallback)`
    - `.on_toggle_sidebar(Fn)`, `.on_all_tabs(Fn)`, `.on_shortcuts(Fn)`, `.on_settings(Fn)`
- Element ids: `titlebar`, `titlebar-leading`, `toggle-sidebar`, `tabs`, `tab-shell-{i}`, `tab-{i}`, `tab-close-{i}`, `window-drag`, `title-actions`, `all-tabs`, `shortcuts`, `settings`.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-ui/tests/titlebar.rs`:
```rust
mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, px};
use rogg_ui::titlebar::{TabItem, TitleBar};
use std::cell::RefCell;
use std::rc::Rc;

fn tabs(n: usize) -> Vec<TabItem> {
    (0..n)
        .map(|i| TabItem { label: format!("service-{i}-with-a-long-name.log").into(), tooltip: "bundle.zip".into(), active: i == 0 })
        .collect()
}

#[gpui_kit::test]
fn titlebar_geometry(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(3)).traffic_lights(true).into_any_element());
    let bar = common::bounds(cx, w, "titlebar");
    assert_eq!(bar.size.height, px(42.));
    assert_eq!(common::bounds(cx, w, "titlebar-leading").size.width, px(211.));
    let tab = common::bounds(cx, w, "tab-shell-0");
    assert_eq!(tab.size.height, px(29.));
    assert!(tab.size.width <= px(152.));
    let actions = common::bounds(cx, w, "title-actions");
    assert!(actions.right() <= px(1440. - 10.) + px(0.5), "10px right padding");
}

#[gpui_kit::test]
fn many_tabs_scroll_instead_of_pushing_actions_out(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 900., 200., |_, _| TitleBar::new(tabs(12)).into_any_element());
    let actions = common::bounds(cx, w, "title-actions");
    assert!(actions.right() <= px(900.), "title actions must stay visible, got {actions:?}");
    assert!(common::bounds(cx, w, "tab-shell-11").size.width <= px(152.));
}

#[gpui_kit::test]
fn tab_select_and_close_report_index(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let e = events.clone();
    let w = common::open(cx, 1440., 200., move |_, _| {
        let (s, c) = (e.clone(), e.clone());
        TitleBar::new(tabs(3))
            .on_select_tab(move |i, _, _| s.borrow_mut().push(format!("select {i}")))
            .on_close_tab(move |i, _, _| c.borrow_mut().push(format!("close {i}")))
            .into_any_element()
    });
    common::click(cx, w, "tab-1");
    common::click(cx, w, "tab-close-2");
    assert_eq!(*events.borrow(), vec!["select 1".to_string(), "close 2".to_string()]);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p rogg-ui --test titlebar`
Expected: compile error, `rogg_ui::titlebar` is not found.

- [ ] **Step 3: Implement `titlebar.rs`**

```rust
use crate::controls::IconButton;
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, Color, size};
use crate::{Callback, IndexCallback};
use gpui_kit::prelude::*;
use gpui_kit::{App, FontWeight, MouseButton, Pixels, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct TabItem {
    pub label: SharedString,
    pub tooltip: SharedString,
    pub active: bool,
}

/// The 42px title bar: traffic-light block + sidebar toggle + brand (as wide as the sidebar), compact tabs, drag area, actions.
#[derive(IntoElement)]
pub struct TitleBar {
    tabs: Vec<TabItem>,
    sidebar_width: Pixels,
    traffic_lights: bool,
    force_hover_tab: Option<usize>,
    on_select_tab: Option<IndexCallback>,
    on_close_tab: Option<IndexCallback>,
    on_toggle_sidebar: Option<Callback>,
    on_all_tabs: Option<Callback>,
    on_shortcuts: Option<Callback>,
    on_settings: Option<Callback>,
}

impl TitleBar {
    pub fn new(tabs: Vec<TabItem>) -> Self {
        Self {
            tabs,
            sidebar_width: px(size::SIDEBAR_W),
            traffic_lights: false,
            force_hover_tab: None,
            on_select_tab: None,
            on_close_tab: None,
            on_toggle_sidebar: None,
            on_all_tabs: None,
            on_shortcuts: None,
            on_settings: None,
        }
    }

    pub fn sidebar_width(mut self, width: Pixels) -> Self {
        self.sidebar_width = width;
        self
    }

    pub fn traffic_lights(mut self, draw: bool) -> Self {
        self.traffic_lights = draw;
        self
    }

    pub fn force_hover_tab(mut self, index: Option<usize>) -> Self {
        self.force_hover_tab = index;
        self
    }

    pub fn on_select_tab(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select_tab = Some(Rc::new(f));
        self
    }

    pub fn on_close_tab(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_close_tab = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_sidebar(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_sidebar = Some(Rc::new(f));
        self
    }

    pub fn on_all_tabs(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_all_tabs = Some(Rc::new(f));
        self
    }

    pub fn on_shortcuts(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_shortcuts = Some(Rc::new(f));
        self
    }

    pub fn on_settings(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_settings = Some(Rc::new(f));
        self
    }
}

fn button(b: IconButton, cb: Option<Callback>) -> IconButton {
    match cb {
        Some(f) => b.on_click(move |w, cx| f(w, cx)),
        None => b,
    }
}

impl RenderOnce for TitleBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let traffic = div().w(px(53.)).flex_none().flex().items_center().gap(px(7.)).when(self.traffic_lights, |d| {
            d.children(p.traffic.map(|c| div().size(px(10.)).rounded_full().bg(c)))
        });
        let leading = div()
            .id("titlebar-leading")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.))
            .w(self.sidebar_width)
            .px(px(13.))
            .child(traffic)
            .child(button(IconButton::icon("toggle-sidebar", IconName::Side).compact(), self.on_toggle_sidebar))
            .child(div().text_size(px(14.)).font_weight(FontWeight(600.)).text_color(p.muted).child("rogg"));

        let tab_strip = div()
            .id("tabs")
            .test_support()
            .flex()
            .flex_shrink()
            .min_w_0()
            .items_center()
            .h(px(30.))
            .gap(px(3.))
            .overflow_x_scroll()
            .children(self.tabs.into_iter().enumerate().map(|(i, tab)| {
                TabShell {
                    index: i,
                    item: tab,
                    force_hover: self.force_hover_tab == Some(i),
                    on_select: self.on_select_tab.clone(),
                    on_close: self.on_close_tab.clone(),
                }
            }));

        div()
            .id("titlebar")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .h(px(size::TITLEBAR_H))
            .pr(px(10.))
            .bg(p.side)
            .border_b_1()
            .border_color(p.border)
            .child(leading)
            .child(
                div().flex().flex_1().min_w_0().h_full().items_center().gap(px(5.)).child(tab_strip).child(
                    div()
                        .id("window-drag")
                        .flex_1()
                        .min_w(px(28.))
                        .h_full()
                        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move()),
                ),
            )
            .child(
                div()
                    .id("title-actions")
                    .test_support()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(4.))
                    .pl(px(5.))
                    .child(button(IconButton::glyph("all-tabs", "⌄").compact(), self.on_all_tabs))
                    .child(button(IconButton::glyph("shortcuts", "⌘").compact(), self.on_shortcuts))
                    .child(button(IconButton::icon("settings", IconName::Settings).compact(), self.on_settings)),
            )
    }
}

/// `.tab-shell`: tab button + hover-revealed close button + accent underline when active. Middle-click closes.
#[derive(IntoElement)]
struct TabShell {
    index: usize,
    item: TabItem,
    force_hover: bool,
    on_select: Option<IndexCallback>,
    on_close: Option<IndexCallback>,
}

impl RenderOnce for TabShell {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let i = self.index;
        let active = self.item.active;
        let group: SharedString = format!("tab-group-{i}").into();
        let (on_select, on_close, on_middle) = (self.on_select, self.on_close.clone(), self.on_close);
        div()
            .id(SharedString::from(format!("tab-shell-{i}")))
            .test_support()
            .group(group.clone())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(size::TAB_H))
            .max_w(px(size::TAB_MAX_W))
            .pr(px(3.))
            .rounded(px(5.))
            .border_1()
            .map(|d| {
                if active {
                    d.bg(p.raised).border_color(p.border)
                } else if self.force_hover {
                    d.bg(p.hover).border_color(Color::TRANSPARENT)
                } else {
                    d.border_color(Color::TRANSPARENT).hover(move |s| s.bg(p.hover))
                }
            })
            .on_mouse_down(MouseButton::Middle, move |_, window, cx| {
                if let Some(f) = &on_middle {
                    f(i, window, cx)
                }
            })
            .child(
                div()
                    .id(SharedString::from(format!("tab-{i}")))
                    .test_support()
                    .flex()
                    .min_w_0()
                    .items_center()
                    .gap(px(6.))
                    .px(px(7.))
                    .h(px(27.))
                    .max_w(px(size::TAB_LABEL_MAX_W))
                    .text_size(px(11.))
                    .text_color(if active { p.text } else { p.muted })
                    .cursor_pointer()
                    .child(icon(IconName::File).size(px(12.)).text_color(if active { p.accent } else { p.faint }))
                    .child(div().min_w_0().truncate().child(self.item.label))
                    .on_click(move |_, window, cx| {
                        if let Some(f) = &on_select {
                            f(i, window, cx)
                        }
                    }),
            )
            .child(
                div()
                    .id(SharedString::from(format!("tab-close-{i}")))
                    .test_support()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(px(18.))
                    .h(px(20.))
                    .rounded(px(5.))
                    .text_size(px(14.))
                    .text_color(p.muted)
                    .cursor_pointer()
                    .when(!active && !self.force_hover, |d| d.opacity(0.).group_hover(group, |s| s.opacity(1.)))
                    .hover(move |s| s.bg(p.hover))
                    .child("×")
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        if let Some(f) = &on_close {
                            f(i, window, cx)
                        }
                    }),
            )
            .when(active, |d| d.child(div().absolute().bottom(px(-1.)).left(px(9.)).right(px(9.)).h(px(1.)).bg(p.accent).opacity(0.7)))
    }
}
```

Add `pub mod titlebar;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-ui --test titlebar`
Expected: 3 tests pass. A failing `click` on `tab-close-2` means the element is invisible to hit testing at opacity 0. In that case, render it with `.invisible()` and `group_hover(.. visible)` instead of opacity, and re-run.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): title bar with compact tabs, drag area and actions

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Sidebar

**Files:**
- Create: `crates/rogg-ui/src/sidebar.rs`
- Modify: `crates/rogg-ui/src/lib.rs` (add `pub mod sidebar;`)
- Test: `crates/rogg-ui/tests/sidebar.rs`

**Interfaces:**
- Produces:
  - `sidebar::TreeRow::{Folder { label, expanded: bool, depth: usize }, File { label, size: SharedString, active: bool, depth: usize, failed: bool }}`
  - `sidebar::RecentItem { label, is_folder: bool, missing: bool }`
  - `sidebar::SidebarFooter { title, lines: Vec<SharedString> }`
  - `sidebar::Sidebar::new()`, with `.width(Pixels)`, `.count(SharedString)`, `.rows(Vec<TreeRow>)`, `.recent(Vec<RecentItem>)`, `.footer(Option<SidebarFooter>)`, `.on_row(IndexCallback)`, `.on_recent(IndexCallback)`
- Element ids: `sidebar`, `side-top`, `tree-row-{i}`, `recent-{i}`, `side-bottom`.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-ui/tests/sidebar.rs`:
```rust
mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, px};
use rogg_ui::sidebar::{RecentItem, Sidebar, SidebarFooter, TreeRow};
use std::cell::Cell;
use std::rc::Rc;

fn sidebar() -> Sidebar {
    Sidebar::new()
        .count("10 files".into())
        .rows(vec![
            TreeRow::Folder { label: "incident.zip".into(), expanded: true, depth: 0 },
            TreeRow::File { label: "api.log".into(), size: "972 MB".into(), active: true, depth: 1, failed: false },
            TreeRow::File { label: "worker.log".into(), size: "841 MB".into(), active: false, depth: 1, failed: false },
        ])
        .recent(vec![RecentItem { label: "staging / logs".into(), is_folder: true, missing: false }])
        .footer(Some(SidebarFooter { title: "Support bundle".into(), lines: vec!["10 members · 8.4 GB expanded".into()] }))
}

#[gpui_kit::test]
fn sidebar_geometry(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 600., 700., |_, _| sidebar().into_any_element());
    assert_eq!(common::bounds(cx, w, "sidebar").size.width, px(211.));
    assert_eq!(common::bounds(cx, w, "side-top").size.height, px(43.));
    let folder = common::bounds(cx, w, "tree-row-0");
    let file = common::bounds(cx, w, "tree-row-1");
    assert_eq!(folder.size.height, px(30.));
    assert_eq!(file.size.height, px(30.));
    // .side-body padding 5px 7px: first row starts at 43 + 5, inset 7.
    assert_eq!(folder.origin.y, px(48.));
    assert_eq!(folder.origin.x, px(7.));
    assert!(common::exists(cx, w, "recent-0"));
    assert!(common::bounds(cx, w, "side-bottom").bottom() <= px(700.));
}

#[gpui_kit::test]
fn clicking_rows_reports_index(cx: &mut TestAppContext) {
    common::setup(cx);
    let hit = Rc::new(Cell::new(usize::MAX));
    let h = hit.clone();
    let w = common::open(cx, 600., 700., move |_, _| {
        let h = h.clone();
        sidebar().on_row(move |i, _, _| h.set(i)).into_any_element()
    });
    common::click(cx, w, "tree-row-2");
    assert_eq!(hit.get(), 2);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p rogg-ui --test sidebar`
Expected: compile error, `rogg_ui::sidebar` is not found.

- [ ] **Step 3: Implement `sidebar.rs`**

```rust
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, Palette, size};
use crate::IndexCallback;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, FontWeight, Pixels, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum TreeRow {
    Folder { label: SharedString, expanded: bool, depth: usize },
    File { label: SharedString, size: SharedString, active: bool, depth: usize, failed: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecentItem {
    pub label: SharedString,
    pub is_folder: bool,
    pub missing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SidebarFooter {
    pub title: SharedString,
    pub lines: Vec<SharedString>,
}

/// The file browser: FILES header, archive/folder tree, RECENT list and bundle summary footer.
#[derive(IntoElement)]
pub struct Sidebar {
    width: Pixels,
    count: SharedString,
    rows: Vec<TreeRow>,
    recent: Vec<RecentItem>,
    footer: Option<SidebarFooter>,
    on_row: Option<IndexCallback>,
    on_recent: Option<IndexCallback>,
}

impl Sidebar {
    pub fn new() -> Self {
        Self { width: px(size::SIDEBAR_W), count: SharedString::default(), rows: vec![], recent: vec![], footer: None, on_row: None, on_recent: None }
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    pub fn count(mut self, count: SharedString) -> Self {
        self.count = count;
        self
    }

    pub fn rows(mut self, rows: Vec<TreeRow>) -> Self {
        self.rows = rows;
        self
    }

    pub fn recent(mut self, recent: Vec<RecentItem>) -> Self {
        self.recent = recent;
        self
    }

    pub fn footer(mut self, footer: Option<SidebarFooter>) -> Self {
        self.footer = footer;
        self
    }

    pub fn on_row(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_row = Some(Rc::new(f));
        self
    }

    pub fn on_recent(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_recent = Some(Rc::new(f));
        self
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

/// `.small-label`: 10px uppercase, 600 weight, muted.
fn small_label(text: &'static str, p: &Palette) -> gpui_kit::Div {
    div().text_size(px(10.)).font_weight(FontWeight(600.)).text_color(p.muted).child(text.to_uppercase())
}

fn tree_row(i: usize, row: TreeRow, p: Palette, on_row: Option<IndexCallback>) -> AnyElement {
    let base = div()
        .id(SharedString::from(format!("tree-row-{i}")))
        .test_support()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(size::TREE_ROW_H))
        .w_full()
        .whitespace_nowrap()
        .cursor_pointer()
        .on_click(move |_, window, cx| {
            if let Some(f) = &on_row {
                f(i, window, cx)
            }
        });
    match row {
        TreeRow::Folder { label, expanded, depth } => base
            .pl(px(9. + depth as f32 * 18.))
            .pr(px(9.))
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.text)
            .child(div().text_color(p.muted).child(if expanded { "⌄" } else { "›" }))
            .child(icon(IconName::Folder).size(px(13.)).text_color(p.muted))
            .child(label)
            .into_any_element(),
        TreeRow::File { label, size: file_size, active, depth, failed } => base
            .pl(px(27. + depth.saturating_sub(1) as f32 * 18.))
            .pr(px(9.))
            .rounded(px(4.))
            .text_size(px(12.))
            .text_color(if active { p.text } else if failed { p.error } else { p.muted })
            .when(active, |d| d.bg(p.selection))
            .when(!active, |d| d.hover(move |s| s.bg(p.hover)))
            .child(icon(IconName::File).size(px(13.)).text_color(if active { p.accent } else { p.faint }))
            .child(div().min_w_0().truncate().child(label))
            .child(div().ml_auto().flex_none().text_size(px(10.)).text_color(p.faint).child(file_size))
            .into_any_element(),
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let on_row = self.on_row.clone();
        let on_recent = self.on_recent.clone();
        div()
            .id("sidebar")
            .test_support()
            .flex()
            .flex_col()
            .flex_none()
            .w(self.width)
            .h_full()
            .bg(p.side)
            .border_r_1()
            .border_color(p.border)
            .child(
                div()
                    .id("side-top")
                    .test_support()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .h(px(size::SIDE_TOP_H))
                    .px(px(15.))
                    .child(small_label("Files", &p))
                    .child(div().text_size(px(10.)).text_color(p.faint).child(self.count)),
            )
            .child(
                div()
                    .id("side-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(7.))
                    .py(px(5.))
                    .children(self.rows.into_iter().enumerate().map(|(i, row)| tree_row(i, row, p, on_row.clone())))
                    .child(div().mt(px(23.)).mx(px(9.)).mb(px(7.)).child(small_label("Recent", &p)))
                    .children(self.recent.into_iter().enumerate().map(|(i, item)| {
                        let on_recent = on_recent.clone();
                        div()
                            .id(SharedString::from(format!("recent-{i}")))
                            .test_support()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .px(px(10.))
                            .py(px(8.))
                            .text_size(px(11.))
                            .text_color(if item.missing { p.faint } else { p.muted })
                            .cursor_pointer()
                            .child(icon(if item.is_folder { IconName::Folder } else { IconName::File }).size(px(15.)).text_color(if item.missing { p.faint } else { p.muted }))
                            .child(div().min_w_0().truncate().child(item.label))
                            .on_click(move |_, window, cx| {
                                if let Some(f) = &on_recent {
                                    f(i, window, cx)
                                }
                            })
                    })),
            )
            .when_some(self.footer, |d, footer| {
                d.child(
                    div()
                        .id("side-bottom")
                        .test_support()
                        .flex_none()
                        .px(px(15.))
                        .py(px(12.))
                        .border_t_1()
                        .border_color(p.border)
                        .text_size(px(10.))
                        .line_height(px(18.))
                        .text_color(p.muted)
                        .child(div().font_weight(FontWeight::MEDIUM).text_color(p.text).child(footer.title))
                        .children(footer.lines.into_iter().map(|l| div().child(l))),
                )
            })
    }
}
```

Add `pub mod sidebar;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-ui --test sidebar`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): sidebar with file tree, recent list and bundle footer

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Info bar, pane header, status bar

**Files:**
- Create: `crates/rogg-ui/src/chrome.rs`
- Modify: `crates/rogg-ui/src/lib.rs` (add `pub mod chrome;`)
- Test: `crates/rogg-ui/tests/chrome.rs`

**Interfaces:**
- Consumes: `controls::{TextButton, FindField, ToolButton}`.
- Produces:
  - `chrome::InfoBar::new(crumbs: Vec<SharedString>, profile: SharedString)`, with `.snapshot(Option<SharedString>)`, `.on_crumb(IndexCallback)`, `.on_profile(Fn)`, `.on_export(Fn)`
  - `chrome::PaneHeader::new(id, title, count)`, with `.find(FindField)`, `.tool(ToolButton)` (repeatable). Its height comes from `Metrics::pane_header_height`.
  - `chrome::StatusBar::new(note)`, with `.stats(Vec<StatPart>)`, `.selection(Option<SharedString>)`, `.progress(Option<Progress>)`
  - `chrome::StatPart::{Strong(SharedString), Text(SharedString)}`
  - `chrome::Progress { label: SharedString, fraction: f32 }`
- Element ids: `meta`, `profile`, `export`, `{id}` (pane header), `statusbar`, `progress`.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-ui/tests/chrome.rs`:
```rust
mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, div, px};
use rogg_ui::chrome::{InfoBar, PaneHeader, Progress, StatPart, StatusBar};
use rogg_ui::theme::{Appearance, Density};

fn chrome() -> gpui_kit::AnyElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(InfoBar::new(vec!["incident.zip".into(), "api.log".into()], "Services".into()))
        .child(PaneHeader::new("pane-top-head", "Full log", "40 sample lines"))
        .child(
            StatusBar::new("UTF-8 · LF".into())
                .stats(vec![StatPart::Strong("6".into()), StatPart::Text(" matches / 40 lines".into())])
                .progress(Some(Progress { label: "Indexing 41%".into(), fraction: 0.41 })),
        )
        .into_any_element()
}

#[gpui_kit::test]
fn compact_heights(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1200., 300., |_, _| chrome());
    assert_eq!(common::bounds(cx, w, "meta").size.height, px(33.));
    assert_eq!(common::bounds(cx, w, "pane-top-head").size.height, px(35.));
    assert_eq!(common::bounds(cx, w, "statusbar").size.height, px(29.));
    assert!(common::exists(cx, w, "progress"));
}

#[gpui_kit::test]
fn comfortable_pane_header_is_taller(cx: &mut TestAppContext) {
    common::setup_with(cx, Appearance::Light, Density::Comfortable);
    let w = common::open(cx, 1200., 300., |_, _| chrome());
    assert_eq!(common::bounds(cx, w, "pane-top-head").size.height, px(39.));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p rogg-ui --test chrome`
Expected: compile error, `rogg_ui::chrome` is not found.

- [ ] **Step 3: Implement `chrome.rs`**

```rust
use crate::controls::{FindField, TextButton, ToolButton};
use crate::icons::IconName;
use crate::theme::{ActiveRoggTheme, size};
use crate::{Callback, IndexCallback};
use gpui_kit::prelude::*;
use gpui_kit::{App, FontWeight, SharedString, TestSupportExt, Window, div, px, relative};
use std::rc::Rc;

/// `.meta`: breadcrumb on the left, `● Profile: X⌄ | Export` on the right.
#[derive(IntoElement)]
pub struct InfoBar {
    crumbs: Vec<SharedString>,
    profile: SharedString,
    snapshot: Option<SharedString>,
    on_crumb: Option<IndexCallback>,
    on_profile: Option<Callback>,
    on_export: Option<Callback>,
}

impl InfoBar {
    pub fn new(crumbs: Vec<SharedString>, profile: SharedString) -> Self {
        Self { crumbs, profile, snapshot: None, on_crumb: None, on_profile: None, on_export: None }
    }

    /// `Snapshot · 14:32:08` note shown for tabs opened from a live source's filtered view.
    pub fn snapshot(mut self, note: Option<SharedString>) -> Self {
        self.snapshot = note;
        self
    }

    pub fn on_crumb(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_crumb = Some(Rc::new(f));
        self
    }

    pub fn on_profile(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_profile = Some(Rc::new(f));
        self
    }

    pub fn on_export(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_export = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for InfoBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let sep = move |s: &'static str| div().mx(px(3.)).text_color(p.faint).child(s);
        let count = self.crumbs.len();
        let on_profile = self.on_profile;
        let on_export = self.on_export;
        let mut bar = div()
            .id("meta")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(7.))
            .h(px(size::INFO_BAR_H))
            .px(px(13.))
            .border_b_1()
            .border_color(p.line)
            .text_size(px(11.))
            .text_color(p.muted);
        for (i, crumb) in self.crumbs.into_iter().enumerate() {
            let on_crumb = self.on_crumb.clone();
            bar = bar.child(
                div()
                    .id(SharedString::from(format!("crumb-{i}")))
                    .cursor_pointer()
                    .child(crumb)
                    .on_click(move |_, window, cx| {
                        if let Some(f) = &on_crumb {
                            f(i, window, cx)
                        }
                    }),
            );
            if i + 1 < count {
                bar = bar.child(sep("›"));
            }
        }
        bar.when_some(self.snapshot, |d, s| d.child(div().text_color(p.faint).child(s))).child(
            div()
                .ml_auto()
                .flex()
                .items_center()
                .gap(px(9.))
                .child(TextButton::meta("profile", format!("Profile: {}⌄", self.profile)).dot().on_click(move |w, cx| {
                    if let Some(f) = &on_profile {
                        f(w, cx)
                    }
                }))
                .child(sep("|"))
                .child(TextButton::meta("export", "Export").icon(IconName::Export).on_click(move |w, cx| {
                    if let Some(f) = &on_export {
                        f(w, cx)
                    }
                })),
        )
    }
}

/// `.pane-head`: title, count, then right-aligned find field and tool buttons.
#[derive(IntoElement)]
pub struct PaneHeader {
    id: SharedString,
    title: SharedString,
    count: SharedString,
    find: Option<FindField>,
    tools: Vec<ToolButton>,
}

impl PaneHeader {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>, count: impl Into<SharedString>) -> Self {
        Self { id: id.into(), title: title.into(), count: count.into(), find: None, tools: vec![] }
    }

    pub fn find(mut self, find: FindField) -> Self {
        self.find = Some(find);
        self
    }

    pub fn tool(mut self, tool: ToolButton) -> Self {
        self.tools.push(tool);
        self
    }
}

impl RenderOnce for PaneHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        div()
            .id(self.id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(9.))
            .h(t.metrics.pane_header_height)
            .px(px(12.))
            .border_b_1()
            .border_color(p.line)
            .child(div().text_size(px(11.)).font_weight(FontWeight(550.)).text_color(p.text).child(self.title))
            .child(div().text_size(px(10.)).text_color(p.faint).child(self.count))
            .child(div().ml_auto().flex().items_center().gap(px(6.)).children(self.find).children(self.tools))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StatPart {
    Strong(SharedString),
    Text(SharedString),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub label: SharedString,
    pub fraction: f32,
}

/// `.footer`: statistics, selected line, then progress and the encoding note on the right.
#[derive(IntoElement)]
pub struct StatusBar {
    note: SharedString,
    stats: Vec<StatPart>,
    selection: Option<SharedString>,
    progress: Option<Progress>,
}

impl StatusBar {
    pub fn new(note: SharedString) -> Self {
        Self { note, stats: vec![], selection: None, progress: None }
    }

    pub fn stats(mut self, stats: Vec<StatPart>) -> Self {
        self.stats = stats;
        self
    }

    pub fn selection(mut self, selection: Option<SharedString>) -> Self {
        self.selection = selection;
        self
    }

    pub fn progress(mut self, progress: Option<Progress>) -> Self {
        self.progress = progress;
        self
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        div()
            .id("statusbar")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(15.))
            .h(px(size::STATUS_BAR_H))
            .px(px(12.))
            .bg(p.side)
            .border_t_1()
            .border_color(p.border)
            .text_size(px(10.))
            .text_color(p.muted)
            .child(div().flex().items_center().children(self.stats.into_iter().map(|part| match part {
                StatPart::Strong(s) => div().font_weight(FontWeight::MEDIUM).text_color(p.text).child(s),
                StatPart::Text(s) => div().child(s),
            })))
            .when_some(self.selection, |d, s| d.child(div().child(s)))
            .child(
                div()
                    .ml_auto()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .when_some(self.progress, |d, pr| {
                        d.child(
                            div()
                                .id("progress")
                                .test_support()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .w(px(80.))
                                        .h(px(3.))
                                        .rounded(px(2.))
                                        .bg(p.border)
                                        .child(div().h_full().rounded(px(2.)).bg(p.accent).w(relative(pr.fraction.clamp(0., 1.)))),
                                )
                                .child(pr.label),
                        )
                    })
                    .child(div().text_color(p.faint).child(self.note)),
            )
    }
}
```

Add `pub mod chrome;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-ui --test chrome`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): info bar, pane header and status bar

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Filter chip and filter band

**Files:**
- Create: `crates/rogg-ui/src/filter.rs`
- Modify: `crates/rogg-ui/src/lib.rs` (add `pub mod filter;`)
- Test: `crates/rogg-ui/tests/filter.rs`

**Interfaces:**
- Consumes: `controls::{IconButton, FlagButton, TextButton, CompactSelect}`.
- Produces:
  - `filter::ChipState::{Include, Off, Exclude}` with `.next()`, `.label()`, `.icon()`
  - `filter::FilterChip::new(index, text, state)`, with `.on_cycle(Fn)`; the color slot is `index % 4`
  - `filter::FilterBand::new(terms: Vec<(SharedString, ChipState)>)`, with:
    - `.regex(bool)`, `.case_sensitive(bool)`
    - `.editing(Option<Entity<InputState>>)`, `.error(Option<SharedString>)`
    - `.context(options: Vec<SharedString>, selected: usize)`
    - `.summary(SharedString)`, `.hint(SharedString)`
    - callbacks `.on_cycle(IndexCallback)`, `.on_edit(Fn)`, `.on_apply(Fn)`, `.on_toggle_regex(Fn)`, `.on_toggle_case(Fn)`, `.on_history(Fn)`, `.on_context(IndexCallback)`
- Element ids: `filter-band`, `filters`, `token-{i}`, `token-state-{i}`, `filter-actions`, `edit-filters`, `flag-regex`, `flag-case`, `expression`, `apply`, `filter-error`, `filter-bottom`, `history`, `context-select`.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-ui/tests/filter.rs`:
```rust
mod common;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, px};
use rogg_ui::filter::{ChipState, FilterBand};
use std::cell::Cell;
use std::rc::Rc;

fn terms(n: usize) -> Vec<(gpui_kit::SharedString, ChipState)> {
    (0..n).map(|i| (format!("term{i}").into(), [ChipState::Include, ChipState::Off, ChipState::Exclude][i % 3])).collect()
}

#[test]
fn state_cycles_include_off_exclude() {
    assert_eq!(ChipState::Include.next(), ChipState::Off);
    assert_eq!(ChipState::Off.next(), ChipState::Exclude);
    assert_eq!(ChipState::Exclude.next(), ChipState::Include);
    assert_eq!([ChipState::Include.label(), ChipState::Off.label(), ChipState::Exclude.label()], ["IN", "OFF", "NOT"]);
}

#[gpui_kit::test]
fn chips_and_flags_use_reference_heights(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1200., 200., |_, _| FilterBand::new(terms(4)).into_any_element());
    assert_eq!(common::bounds(cx, w, "token-0").size.height, px(28.));
    assert_eq!(common::bounds(cx, w, "flag-regex").size.height, px(27.));
    let band = common::bounds(cx, w, "filter-band");
    let line = common::bounds(cx, w, "filters");
    // padding-top 10 + 1px top border.
    assert_eq!(line.origin.y - band.origin.y, px(11.));
}

#[gpui_kit::test]
fn twenty_chips_keep_actions_visible(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 900., 200., |_, _| FilterBand::new(terms(20)).into_any_element());
    let actions = common::bounds(cx, w, "filter-actions");
    assert!(actions.right() <= px(900. - 12.) + px(0.5), "actions pushed out: {actions:?}");
}

#[gpui_kit::test]
fn edit_mode_swaps_chips_for_expression(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1200., 200., |window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx).default_value("error || timeout"));
        FilterBand::new(terms(2)).editing(Some(input)).error(Some("regex parse error".into())).into_any_element()
    });
    assert!(common::exists(cx, w, "expression"));
    assert!(common::exists(cx, w, "apply"));
    assert!(common::exists(cx, w, "filter-error"));
    assert!(!common::exists(cx, w, "edit-filters"));
    assert!(!common::exists(cx, w, "token-0"));
}

#[gpui_kit::test]
fn clicking_state_reports_chip_index(cx: &mut TestAppContext) {
    common::setup(cx);
    let hit = Rc::new(Cell::new(usize::MAX));
    let h = hit.clone();
    let w = common::open(cx, 1200., 200., move |_, _| {
        let h = h.clone();
        FilterBand::new(terms(3)).on_cycle(move |i, _, _| h.set(i)).into_any_element()
    });
    common::click(cx, w, "token-state-1");
    assert_eq!(hit.get(), 1);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p rogg-ui --test filter`
Expected: compile error, `rogg_ui::filter` is not found.

- [ ] **Step 3: Implement `filter.rs`**

```rust
use crate::controls::{CompactSelect, FlagButton, IconButton, TextButton};
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, Color, size};
use crate::{Callback, IndexCallback};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Entity, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ChipState {
    Include,
    Off,
    Exclude,
}

impl ChipState {
    /// Include → Off → Exclude → Include (reference hint).
    pub fn next(self) -> Self {
        match self {
            ChipState::Include => ChipState::Off,
            ChipState::Off => ChipState::Exclude,
            ChipState::Exclude => ChipState::Include,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ChipState::Include => "IN",
            ChipState::Off => "OFF",
            ChipState::Exclude => "NOT",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            ChipState::Include => IconName::Check,
            ChipState::Off => IconName::Off,
            ChipState::Exclude => IconName::Minus,
        }
    }
}

/// `.token`: clickable state segment + monospace term; colored by slot, neutral when off, struck through when excluded.
#[derive(IntoElement)]
pub struct FilterChip {
    index: usize,
    text: SharedString,
    state: ChipState,
    on_cycle: Option<Callback>,
}

impl FilterChip {
    pub fn new(index: usize, text: impl Into<SharedString>, state: ChipState) -> Self {
        Self { index, text: text.into(), state, on_cycle: None }
    }

    pub fn on_cycle(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cycle = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for FilterChip {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        let slot = p.chips[self.index % p.chips.len()];
        let (border, bg, term) = match self.state {
            ChipState::Off => (p.border, Color::TRANSPARENT, p.faint),
            _ => (slot.border, slot.bg, slot.term),
        };
        let i = self.index;
        let on_cycle = self.on_cycle;
        div()
            .id(SharedString::from(format!("token-{i}")))
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .h(px(size::CHIP_H))
            .border_1()
            .border_color(border)
            .rounded(px(5.))
            .bg(bg)
            .font_family(t.fonts.mono.clone())
            .text_size(px(12.))
            .child(
                div()
                    .id(SharedString::from(format!("token-state-{i}")))
                    .test_support()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(4.))
                    .h_full()
                    .px(px(5.))
                    .rounded_l(px(3.))
                    .border_r_1()
                    .border_color(p.chip_divider)
                    .font_family(t.fonts.ui.clone())
                    .text_size(px(9.))
                    .text_color(p.muted)
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(icon(self.state.icon()).size(px(11.)).text_color(p.muted))
                    .child(self.state.label())
                    .on_click(move |_, window, cx| {
                        if let Some(f) = &on_cycle {
                            f(window, cx)
                        }
                    }),
            )
            .child(div().pl(px(6.)).pr(px(8.)).text_color(term).when(self.state == ChipState::Exclude, |d| d.line_through()).child(self.text))
    }
}

/// `.filter-band`: chips (or expression editor), flags, and the History / Context / summary / hint row.
#[derive(IntoElement)]
pub struct FilterBand {
    terms: Vec<(SharedString, ChipState)>,
    regex: bool,
    case_sensitive: bool,
    editing: Option<Entity<InputState>>,
    error: Option<SharedString>,
    context_options: Vec<SharedString>,
    context_selected: usize,
    summary: SharedString,
    hint: SharedString,
    on_cycle: Option<IndexCallback>,
    on_edit: Option<Callback>,
    on_apply: Option<Callback>,
    on_toggle_regex: Option<Callback>,
    on_toggle_case: Option<Callback>,
    on_history: Option<Callback>,
    on_context: Option<IndexCallback>,
}

impl FilterBand {
    pub fn new(terms: Vec<(SharedString, ChipState)>) -> Self {
        Self {
            terms,
            regex: false,
            case_sensitive: false,
            editing: None,
            error: None,
            context_options: vec!["None".into(), "±1 line".into(), "±2 lines".into(), "±3 lines".into()],
            context_selected: 0,
            summary: SharedString::default(),
            hint: "Click a state to cycle Include → Off → Exclude".into(),
            on_cycle: None,
            on_edit: None,
            on_apply: None,
            on_toggle_regex: None,
            on_toggle_case: None,
            on_history: None,
            on_context: None,
        }
    }

    pub fn regex(mut self, on: bool) -> Self {
        self.regex = on;
        self
    }

    pub fn case_sensitive(mut self, on: bool) -> Self {
        self.case_sensitive = on;
        self
    }

    pub fn editing(mut self, input: Option<Entity<InputState>>) -> Self {
        self.editing = input;
        self
    }

    pub fn error(mut self, error: Option<SharedString>) -> Self {
        self.error = error;
        self
    }

    pub fn context(mut self, options: Vec<SharedString>, selected: usize) -> Self {
        self.context_options = options;
        self.context_selected = selected;
        self
    }

    pub fn summary(mut self, summary: SharedString) -> Self {
        self.summary = summary;
        self
    }

    pub fn hint(mut self, hint: SharedString) -> Self {
        self.hint = hint;
        self
    }

    pub fn on_cycle(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_cycle = Some(Rc::new(f));
        self
    }

    pub fn on_edit(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_edit = Some(Rc::new(f));
        self
    }

    pub fn on_apply(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_apply = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_regex(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_regex = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_case(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_case = Some(Rc::new(f));
        self
    }

    pub fn on_history(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_history = Some(Rc::new(f));
        self
    }

    pub fn on_context(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_context = Some(Rc::new(f));
        self
    }
}

fn call(cb: &Option<Callback>) -> impl Fn(&mut Window, &mut App) + 'static {
    let cb = cb.clone();
    move |w, cx| {
        if let Some(f) = &cb {
            f(w, cx)
        }
    }
}

impl RenderOnce for FilterBand {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        let editing = self.editing.is_some();

        let chips_or_editor: AnyElement = match self.editing.clone() {
            Some(input) => div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .id("expression")
                        .test_support()
                        .flex_1()
                        .min_w(px(80.))
                        .px(px(8.))
                        .py(px(6.))
                        .bg(p.bg)
                        .border_1()
                        .border_color(p.accent)
                        .rounded(px(4.))
                        .child(
                            Input::new(&input).appearance(false).p_0().font_family(t.fonts.mono.clone()).text_size(px(12.)).text_color(p.text),
                        ),
                )
                .child(
                    div()
                        .id("apply")
                        .test_support()
                        .flex_none()
                        .px(px(8.))
                        .py(px(5.))
                        .border_1()
                        .border_color(p.accent)
                        .rounded(px(5.))
                        .text_size(px(11.))
                        .text_color(p.accent)
                        .cursor_pointer()
                        .child("Apply ↵")
                        .on_click({
                            let f = call(&self.on_apply);
                            move |_, w, cx| f(w, cx)
                        }),
                )
                .into_any_element(),
            None => {
                let mut row = div().id("filters").test_support().flex().flex_1().min_w_0().items_center().gap(px(7.)).overflow_x_scroll();
                for (i, (text, state)) in self.terms.iter().cloned().enumerate() {
                    if i > 0 {
                        row = row.child(div().flex_none().font_family(t.fonts.mono.clone()).text_size(px(11.)).text_color(p.faint).child("||"));
                    }
                    let on_cycle = self.on_cycle.clone();
                    row = row.child(FilterChip::new(i, text, state).on_cycle(move |w, cx| {
                        if let Some(f) = &on_cycle {
                            f(i, w, cx)
                        }
                    }));
                }
                row.into_any_element()
            }
        };

        let actions = div()
            .id("filter-actions")
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .ml(px(6.))
            .when(!editing, |d| d.child(IconButton::icon("edit-filters", IconName::Edit).on_click(call(&self.on_edit))))
            .child(FlagButton::new("flag-regex", ".*").pressed(self.regex).on_click(call(&self.on_toggle_regex)))
            .child(FlagButton::new("flag-case", "Aa").pressed(self.case_sensitive).on_click(call(&self.on_toggle_case)));

        let on_context = self.on_context.clone();
        let bottom = div()
            .id("filter-bottom")
            .test_support()
            .flex()
            .items_center()
            .gap(px(13.))
            .mt(px(8.))
            .text_size(px(10.))
            .text_color(p.muted)
            .child(TextButton::small("history", "History⌄").icon(IconName::History).on_click(call(&self.on_history)))
            .child(
                div().flex().items_center().gap(px(5.)).child("Context").child(
                    CompactSelect::new("context-select", self.context_options, self.context_selected).on_select(move |i, w, cx| {
                        if let Some(f) = &on_context {
                            f(i, w, cx)
                        }
                    }),
                ),
            )
            .child(div().child(self.summary))
            .child(div().ml_auto().text_size(px(9.)).text_color(p.faint).child(self.hint));

        div()
            .id("filter-band")
            .test_support()
            .relative()
            .flex_none()
            .bg(p.side)
            .border_t_1()
            .border_b_1()
            .border_color(p.band_border)
            .pt(px(10.))
            .px(px(12.))
            .pb(px(8.))
            .child(
                div()
                    .absolute()
                    .top(px(-2.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(div().w(px(30.)).h(px(3.)).rounded(px(5.)).bg(p.band_handle)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_h(px(size::FILTER_LINE_MIN_H))
                    .child(icon(IconName::Filter).size(px(15.)).mr(px(4.)).text_color(p.accent))
                    .child(chips_or_editor)
                    .child(actions),
            )
            .when_some(self.error, |d, e| {
                d.child(
                    div()
                        .id("filter-error")
                        .test_support()
                        .mt(px(6.))
                        .text_size(px(10.))
                        .text_color(p.error)
                        .child(format!("Previous results retained · {e}")),
                )
            })
            .child(bottom)
    }
}
```

Add `pub mod filter;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-ui --test filter`
Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): filter chips with IN/OFF/NOT states and the filter band

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 9: Log view model, text segmentation and scroll math (pure logic)

**Files:**
- Create: `crates/rogg-ui/src/logview/{mod.rs,model.rs,text.rs,scroll.rs}`
- Modify: `crates/rogg-ui/src/lib.rs` (add `pub mod logview;`)
- Test: unit tests inside `text.rs` and `scroll.rs`

**Interfaces:**
- Produces:
  - `logview::SpanStyle::{Faint, Muted, Error, Amber, Fg(Color), Hit(usize), SearchHit}`. Styles are semantic and resolved against the active palette at paint time, so a theme switch needs no data rebuild.
  - `logview::Span { range: Range<usize>, style: SpanStyle }`. Producers guarantee that `Hit` spans do not overlap; the earlier filter term wins.
  - `logview::RowTint::{Error, Color(Color)}` with `.resolve(&Palette) -> Color`
  - `logview::LogLine { number: u64, text: SharedString, spans: Vec<Span>, pinned: bool, context: bool, tint: Option<RowTint> }`
  - `logview::LogRow::{Line(LogLine), Gap { omitted: u64 }}`
  - trait `logview::LogRowSource: 'static`, with `row_count`, `row(ix) -> LogRow`, `is_gap(ix)`, `longest_line_chars`, `row_for_number(u64) -> Option<usize>`
  - `logview::VecRows::new(Vec<LogRow>)`, which implements the trait
  - `logview::text::{MAX_DISPLAY_BYTES, display_text(&str, usize) -> (String, bool), Segment { range, fg, hit_bg, outline }, segments(len, &[Span], base: Color, &Palette) -> Vec<Segment>}`
  - `logview::scroll::{ScrollAnchor { row: usize, offset: f32 }, ScrollGeometry { row_count, viewport, pad, height_of: &dyn Fn(usize) -> f32 }}` with `top()`, `bottom()`, `clamp(a)`, `scroll_by(a, delta)`, `center_on(row)`, `visible(a) -> Vec<(usize, f32)>`, `thumb(a) -> Option<(f32, f32)>`

- [ ] **Step 1: Write the module skeleton with failing tests**

`crates/rogg-ui/src/logview/mod.rs`:
```rust
mod model;
pub mod scroll;
pub mod text;

pub use model::{LogLine, LogRow, LogRowSource, RowTint, Span, SpanStyle, VecRows};
```

`crates/rogg-ui/src/logview/model.rs`:
```rust
use crate::theme::{Color, Palette};
use gpui_kit::SharedString;
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SpanStyle {
    Faint,
    Muted,
    Error,
    Amber,
    Fg(Color),
    /// Filter-match highlight using palette slot `n % 4`.
    Hit(usize),
    /// Per-panel search hit: accent outline.
    SearchHit,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Span {
    pub range: Range<usize>,
    pub style: SpanStyle,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RowTint {
    /// `.row.error-line` tint.
    Error,
    Color(Color),
}

impl RowTint {
    pub fn resolve(self, p: &Palette) -> Color {
        match self {
            RowTint::Error => p.error_row_tint,
            RowTint::Color(c) => c,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct LogLine {
    pub number: u64,
    pub text: SharedString,
    pub spans: Vec<Span>,
    pub pinned: bool,
    pub context: bool,
    pub tint: Option<RowTint>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum LogRow {
    Line(LogLine),
    Gap { omitted: u64 },
}

/// What a log view displays. M1/M2 implement this over the engine; M0 uses `VecRows`.
pub trait LogRowSource: 'static {
    fn row_count(&self) -> usize;
    fn row(&self, ix: usize) -> LogRow;
    fn is_gap(&self, ix: usize) -> bool;
    fn longest_line_chars(&self) -> usize;
    fn row_for_number(&self, number: u64) -> Option<usize>;
}

pub struct VecRows {
    rows: Vec<LogRow>,
    longest: usize,
}

impl VecRows {
    pub fn new(rows: Vec<LogRow>) -> Self {
        let longest = rows
            .iter()
            .map(|r| match r {
                LogRow::Line(l) => l.text.chars().count(),
                LogRow::Gap { .. } => 0,
            })
            .max()
            .unwrap_or(0);
        Self { rows, longest }
    }
}

impl LogRowSource for VecRows {
    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn row(&self, ix: usize) -> LogRow {
        self.rows[ix].clone()
    }

    fn is_gap(&self, ix: usize) -> bool {
        matches!(self.rows.get(ix), Some(LogRow::Gap { .. }))
    }

    fn longest_line_chars(&self) -> usize {
        self.longest
    }

    fn row_for_number(&self, number: u64) -> Option<usize> {
        self.rows.iter().position(|r| matches!(r, LogRow::Line(l) if l.number == number))
    }
}
```

`crates/rogg-ui/src/logview/text.rs`, tests first (the implementation goes in Step 3):
```rust
use super::model::{Span, SpanStyle};
use crate::theme::{Color, Palette};
use std::ops::Range;

/// Lines longer than this are cut for display (spec §4.4); matching still sees the full line.
pub const MAX_DISPLAY_BYTES: usize = 64 * 1024;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Segment {
    pub range: Range<usize>,
    pub fg: Color,
    pub hit_bg: Option<Color>,
    pub outline: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> Palette {
        Palette::dark()
    }

    #[test]
    fn display_text_keeps_offsets_and_replaces_controls() {
        let (s, cut) = display_text("a\tb\rc\u{1}d", 100);
        assert_eq!(s, "a b c d");
        assert!(!cut);
    }

    #[test]
    fn display_text_truncates_on_char_boundary_and_marks_it() {
        let text = format!("{}é", "x".repeat(9)); // 'é' is 2 bytes at offsets 9..11
        let (s, cut) = display_text(&text, 10);
        assert!(cut);
        assert_eq!(s, format!("{}…", "x".repeat(9)));
        let huge = "y".repeat(200 * 1024);
        let (s, cut) = display_text(&huge, MAX_DISPLAY_BYTES);
        assert!(cut);
        assert_eq!(s.len(), MAX_DISPLAY_BYTES + '…'.len_utf8());
    }

    #[test]
    fn plain_line_is_one_base_segment() {
        assert_eq!(segments(5, &[], p().log_text, &p()), vec![Segment { range: 0..5, fg: p().log_text, hit_bg: None, outline: false }]);
        assert!(segments(0, &[], p().log_text, &p()).is_empty());
    }

    #[test]
    fn faint_stamp_then_hit() {
        let spans = vec![Span { range: 0..3, style: SpanStyle::Faint }, Span { range: 5..8, style: SpanStyle::Hit(1) }];
        let segs = segments(10, &spans, p().log_text, &p());
        assert_eq!(segs.iter().map(|s| s.range.clone()).collect::<Vec<_>>(), vec![0..3, 3..5, 5..8, 8..10]);
        assert_eq!(segs[0].fg, p().faint);
        assert_eq!(segs[2].fg, p().hits[1].fg);
        assert_eq!(segs[2].hit_bg, Some(p().hits[1].bg));
        assert_eq!(segs[3].fg, p().log_text);
    }

    #[test]
    fn hit_beats_fg_and_search_adds_outline() {
        let spans = vec![
            Span { range: 0..10, style: SpanStyle::Error },
            Span { range: 2..4, style: SpanStyle::Hit(0) },
            Span { range: 3..6, style: SpanStyle::SearchHit },
        ];
        let segs = segments(10, &spans, p().log_text, &p());
        let at = |i: usize| segs.iter().find(|s| s.range.contains(&i)).unwrap().clone();
        assert_eq!(at(0).fg, p().error);
        assert_eq!(at(2).fg, p().hits[0].fg);
        assert!(!at(2).outline);
        assert!(at(3).outline && at(3).hit_bg.is_some());
        assert!(at(5).outline && at(5).fg == p().error);
    }

    #[test]
    fn semantic_styles_follow_the_palette() {
        let spans = vec![Span { range: 0..3, style: SpanStyle::Faint }, Span { range: 3..6, style: SpanStyle::Hit(0) }];
        let light = Palette::light();
        let segs = segments(6, &spans, light.log_text, &light);
        assert_eq!(segs[0].fg, light.faint);
        assert_eq!(segs[1].fg, light.hits[0].fg);
        assert_eq!(segs[1].hit_bg, Some(light.hits[0].bg));
        assert_ne!(segs[0].fg, Palette::dark().faint);
    }

    #[test]
    fn spans_past_the_end_are_clipped_and_slots_wrap() {
        let spans = vec![Span { range: 4..99, style: SpanStyle::Hit(6) }];
        let segs = segments(6, &spans, p().log_text, &p());
        assert_eq!(segs.last().unwrap().range, 4..6);
        assert_eq!(segs.last().unwrap().hit_bg, Some(p().hits[2].bg));
        let total: usize = segs.iter().map(|s| s.range.len()).sum();
        assert_eq!(total, 6);
    }
}
```

`crates/rogg-ui/src/logview/scroll.rs`, tests first:
```rust
/// Scroll position as "row `row`, scrolled `offset` px past its top". Row 0 may have a negative offset
/// down to `-pad`, so the first row starts below the list's top padding. Independent of total content
/// height, so it works for millions of rows and for unknown wrapped heights.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ScrollAnchor {
    pub row: usize,
    pub offset: f32,
}

pub struct ScrollGeometry<'a> {
    pub row_count: usize,
    pub viewport: f32,
    pub pad: f32,
    pub height_of: &'a dyn Fn(usize) -> f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo<'a>(rows: usize, viewport: f32, h: &'a dyn Fn(usize) -> f32) -> ScrollGeometry<'a> {
        ScrollGeometry { row_count: rows, viewport, pad: 4., height_of: h }
    }

    #[test]
    fn top_starts_rows_below_padding() {
        let h = |_| 23.;
        let g = geo(100, 230., &h);
        let v = g.visible(g.top());
        assert_eq!(v[0], (0, 4.));
        assert_eq!(v[1], (1, 27.));
        assert_eq!(v.len(), 10); // rows at 4..211; row 10 would start at 234 > 230
    }

    #[test]
    fn scroll_by_moves_across_rows_and_clamps() {
        let h = |_| 23.;
        let g = geo(100, 230., &h);
        let a = g.scroll_by(g.top(), 50.);
        assert_eq!(a, ScrollAnchor { row: 2, offset: 0. }); // -4 + 50 = 46 = 2 rows
        assert_eq!(g.scroll_by(a, -1000.), g.top());
        let end = g.scroll_by(a, 1e6);
        assert_eq!(end, g.bottom());
        // last row bottom + pad == viewport bottom
        let v = g.visible(end);
        let (last, y) = *v.last().unwrap();
        assert_eq!(last, 99);
        assert!((y + 23. + 4. - 230.).abs() < 1e-3);
    }

    #[test]
    fn short_content_never_scrolls() {
        let h = |_| 23.;
        let g = geo(3, 230., &h);
        assert_eq!(g.bottom(), g.top());
        assert_eq!(g.scroll_by(g.top(), 500.), g.top());
        assert!(g.thumb(g.top()).is_none());
    }

    #[test]
    fn degenerate_inputs_do_not_panic() {
        let h = |_| 23.;
        let empty = geo(0, 230., &h);
        assert!(empty.visible(empty.top()).is_empty());
        assert_eq!(empty.scroll_by(empty.top(), 100.), empty.top());
        assert_eq!(empty.center_on(0), empty.top());
        let tiny = geo(10, 5., &h);
        let a = tiny.scroll_by(tiny.top(), 40.);
        assert!(!tiny.visible(a).is_empty());
        let one = geo(1, 230., &h);
        assert_eq!(one.center_on(0), one.top());
    }

    #[test]
    fn center_on_places_row_mid_viewport_and_clamps_at_edges() {
        let h = |_| 23.;
        let g = geo(1000, 230., &h);
        let a = g.center_on(500);
        let (_, y) = *g.visible(a).iter().find(|(r, _)| *r == 500).unwrap();
        assert!((y + 11.5 - 115.).abs() < 1e-3, "row 500 centred, got y={y}");
        assert_eq!(g.center_on(0), g.top());
        assert_eq!(g.center_on(999), g.bottom());
    }

    #[test]
    fn mixed_heights_gap_rows() {
        let h = |r: usize| if r % 2 == 1 { 17. } else { 23. };
        let g = geo(10, 100., &h);
        let v = g.visible(g.top());
        assert_eq!(v[..3], [(0, 4.), (1, 27.), (2, 44.)]);
        let a = g.scroll_by(g.top(), 4. + 23. + 17.);
        assert_eq!(a, ScrollAnchor { row: 2, offset: 0. });
    }

    #[test]
    fn thumb_tracks_position() {
        let h = |_| 23.;
        let g = geo(1000, 230., &h);
        let (top, len) = g.thumb(g.top()).unwrap();
        assert_eq!(top, 0.);
        assert!((len - 0.01).abs() < 1e-3);
        let (top, _) = g.thumb(g.center_on(500)).unwrap();
        assert!(top > 0.45 && top < 0.55);
    }
}
```

Add `pub mod logview;` to `lib.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p rogg-ui logview`
Expected: compile errors, `display_text`, `segments` and `visible` are not found.

- [ ] **Step 3: Implement `display_text`, `segments` and the scroll geometry**

Add to `text.rs`, above the tests:
```rust
/// Cuts `text` to at most `max_bytes` (on a char boundary, appending `…`) and replaces ASCII control
/// characters with spaces one-for-one, so span byte offsets stay valid.
pub fn display_text(text: &str, max_bytes: usize) -> (String, bool) {
    let mut end = text.len().min(max_bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut out: String = text[..end].chars().map(|c| if c.is_ascii_control() { ' ' } else { c }).collect();
    let truncated = end < text.len();
    if truncated {
        out.push('…');
    }
    (out, truncated)
}

fn resolve(style: SpanStyle, p: &Palette) -> Option<Color> {
    match style {
        SpanStyle::Faint => Some(p.faint),
        SpanStyle::Muted => Some(p.muted),
        SpanStyle::Error => Some(p.error),
        SpanStyle::Amber => Some(p.amber),
        SpanStyle::Fg(c) => Some(c),
        SpanStyle::Hit(_) | SpanStyle::SearchHit => None,
    }
}

/// Splits `0..len` into maximal runs of identical style. Later foreground spans override earlier ones,
/// a filter hit overrides foreground (first hit wins), and search hits add an outline.
pub fn segments(len: usize, spans: &[Span], base: Color, p: &Palette) -> Vec<Segment> {
    let mut cuts = vec![0, len];
    for s in spans {
        cuts.push(s.range.start.min(len));
        cuts.push(s.range.end.min(len));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<Segment> = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a >= b {
            continue;
        }
        let mut fg = base;
        let mut hit: Option<(Color, Color)> = None;
        let mut outline = false;
        for s in spans.iter().filter(|s| s.range.start <= a && s.range.end >= b) {
            match s.style {
                SpanStyle::Hit(slot) => {
                    let h = p.hits[slot % p.hits.len()];
                    hit.get_or_insert((h.fg, h.bg));
                }
                SpanStyle::SearchHit => outline = true,
                other => fg = resolve(other, p).unwrap_or(fg),
            }
        }
        let (fg, hit_bg) = match hit {
            Some((hfg, hbg)) => (hfg, Some(hbg)),
            None => (fg, None),
        };
        if let Some(last) = out.last_mut() {
            if last.range.end == a && last.fg == fg && last.hit_bg == hit_bg && last.outline == outline {
                last.range.end = b;
                continue;
            }
        }
        out.push(Segment { range: a..b, fg, hit_bg, outline });
    }
    out
}
```

Add to `scroll.rs`, above the tests:
```rust
fn before(a: ScrollAnchor, b: ScrollAnchor) -> bool {
    a.row < b.row || (a.row == b.row && a.offset < b.offset)
}

impl ScrollGeometry<'_> {
    pub fn top(&self) -> ScrollAnchor {
        ScrollAnchor { row: 0, offset: -self.pad }
    }

    /// The furthest anchor: last row's bottom plus padding meets the viewport bottom.
    pub fn bottom(&self) -> ScrollAnchor {
        let mut remaining = self.viewport - self.pad;
        for row in (0..self.row_count).rev() {
            let h = (self.height_of)(row);
            if remaining <= h {
                return ScrollAnchor { row, offset: h - remaining };
            }
            remaining -= h;
        }
        if remaining <= self.pad && self.row_count > 0 {
            ScrollAnchor { row: 0, offset: -remaining }
        } else {
            self.top()
        }
    }

    pub fn clamp(&self, a: ScrollAnchor) -> ScrollAnchor {
        let (top, bottom) = (self.top(), self.bottom());
        if self.row_count == 0 || before(a, top) {
            top
        } else if before(bottom, a) {
            bottom
        } else {
            a
        }
    }

    /// Moves the anchor to the row that actually contains its offset (walks only rows crossed).
    fn normalize(&self, mut a: ScrollAnchor) -> ScrollAnchor {
        if self.row_count == 0 {
            return self.top();
        }
        a.row = a.row.min(self.row_count - 1);
        loop {
            if a.offset < 0. && a.row > 0 {
                a.row -= 1;
                a.offset += (self.height_of)(a.row);
                continue;
            }
            let h = (self.height_of)(a.row);
            if a.offset >= h && a.row + 1 < self.row_count {
                a.offset -= h;
                a.row += 1;
                continue;
            }
            return a;
        }
    }

    pub fn scroll_by(&self, a: ScrollAnchor, delta: f32) -> ScrollAnchor {
        self.clamp(self.normalize(ScrollAnchor { row: a.row, offset: a.offset + delta }))
    }

    pub fn center_on(&self, row: usize) -> ScrollAnchor {
        if self.row_count == 0 {
            return self.top();
        }
        let row = row.min(self.row_count - 1);
        let h = (self.height_of)(row);
        self.clamp(self.normalize(ScrollAnchor { row, offset: -(self.viewport - h) / 2. }))
    }

    /// Rows intersecting the viewport with their top y relative to the viewport top.
    pub fn visible(&self, a: ScrollAnchor) -> Vec<(usize, f32)> {
        let mut out = Vec::new();
        let mut y = -a.offset;
        let mut row = a.row;
        while row < self.row_count && y < self.viewport {
            out.push((row, y));
            y += (self.height_of)(row);
            row += 1;
        }
        out
    }

    /// Scrollbar thumb as (top fraction, length fraction); None when everything fits.
    pub fn thumb(&self, a: ScrollAnchor) -> Option<(f32, f32)> {
        if self.row_count == 0 || self.bottom() == self.top() {
            return None;
        }
        let h = (self.height_of)(a.row).max(1.);
        let count = self.row_count as f32;
        let len = (self.viewport / h / count).min(1.);
        let pos = ((a.row as f32 + (a.offset / h).max(0.)) / count).clamp(0., 1. - len);
        Some((pos, len))
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-ui logview`
Expected: 14 tests pass (7 in `text`, 7 in `scroll`).

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): log view row model, text segmentation and anchor-based scroll math

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Log view element and state

**Files:**
- Create: `crates/rogg-ui/src/logview/{state.rs,element.rs}`
- Modify: `crates/rogg-ui/src/logview/mod.rs`
- Test: `crates/rogg-ui/tests/logview.rs`

**Interfaces:**
- Consumes: Task 9 model, text and scroll; `theme`; `icons::IconName::{Pin, PinFilled}`.
- Produces:
  - `logview::LogViewState` (an entity), with:
    - `LogViewState::new(Rc<dyn LogRowSource>) -> Self`
    - `.set_rows(Rc<dyn LogRowSource>, cx)`, `.rows() -> &Rc<dyn LogRowSource>`
    - `.wrap()`, `.set_wrap(bool, cx)`
    - `.selected() -> Option<u64>`, `.set_selected(Option<u64>, cx)`
    - `.center_on_row(usize, cx)`, `.force_hover_row(Option<usize>, cx)`
    - `.last_rows() -> &[RowGeometry]`
  - `impl EventEmitter<LogViewEvent> for LogViewState`
  - `logview::LogViewEvent::{Selected { row: usize, number: u64 }, TogglePin { number: u64 }}`
  - `logview::RowGeometry { row: usize, bounds: Bounds<Pixels>, is_gap: bool }`
  - `logview::LogView::new(id, Entity<LogViewState>)` (`RenderOnce`; a test-support div that fills its parent around the element)

Painting follows the reference CSS:
- Row layout: 24 px pin gutter, 65 px right-aligned 10 px line number with 12 px right padding, then the text clipped 20 px before the right edge.
- Hit highlights are rounded (2 px) rectangles behind the text, sized to the font's ascent plus descent.
- Selected rows get the selection fill plus a 2 px accent bar on the left.
- Error-tinted rows ignore hover, because in the CSS `.row.error-line` comes after `.row:hover` with equal specificity.
- The list has 4 px top and bottom padding.
- Gap rows are 17 px, with 9 px UI text at x = 89.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-ui/tests/logview.rs`:
```rust
mod common;
use gpui_kit::prelude::*;
use gpui_kit::{AppContext, Entity, ScrollDelta, TestAppContext, point, px};
use rogg_ui::logview::{LogLine, LogRow, LogView, LogViewEvent, LogViewState, VecRows};
use std::cell::RefCell;
use std::rc::Rc;

fn rows(n: usize, gap_at: Option<usize>) -> Rc<VecRows> {
    let mut v = Vec::new();
    for i in 0..n {
        if Some(i) == gap_at {
            v.push(LogRow::Gap { omitted: 4 });
        }
        v.push(LogRow::Line(LogLine { number: 1000 + i as u64, text: format!("line {i}").into(), spans: vec![], pinned: false, context: false, tint: None }));
    }
    Rc::new(VecRows::new(v))
}

fn open(cx: &mut TestAppContext, state: Entity<LogViewState>) -> gpui_kit::AnyWindowHandle {
    common::open(cx, 800., 238., move |_, _| LogView::new("log-test", state.clone()).into_any_element())
}

fn render(cx: &mut TestAppContext, w: gpui_kit::AnyWindowHandle) {
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn rows_start_below_padding_and_gap_is_shorter(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(50, Some(2))));
    let w = open(cx, state.clone());
    render(cx, w);
    let geo = state.read_with(cx, |s, _| s.last_rows().to_vec());
    let top = common::bounds(cx, w, "log-test").origin.y;
    assert_eq!(geo[0].bounds.origin.y - top, px(4.));
    assert_eq!(geo[0].bounds.size.height, px(23.));
    assert!(geo[2].is_gap);
    assert_eq!(geo[2].bounds.size.height, px(17.));
    assert_eq!(geo[3].bounds.origin.y - top, px(4. + 23. + 23. + 17.));
}

#[gpui_kit::test]
fn click_selects_and_gutter_click_toggles_pin(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(50, None)));
    let events = Rc::new(RefCell::new(Vec::new()));
    let e = events.clone();
    cx.update(|cx| {
        cx.subscribe(&state, move |_, ev: &LogViewEvent, _| {
            e.borrow_mut().push(match ev {
                LogViewEvent::Selected { number, .. } => format!("select {number}"),
                LogViewEvent::TogglePin { number } => format!("pin {number}"),
            })
        })
        .detach();
    });
    let w = open(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click_at("log-test", point(px(300.), px(4. + 23. + 10.)), cx);
        window.click_at("log-test", point(px(10.), px(4. + 46. + 10.)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(*events.borrow(), vec!["select 1001".to_string(), "pin 1002".to_string()]);
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1001));
}

#[gpui_kit::test]
fn wheel_scrolls_and_center_on_row_centres(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(1000, None)));
    let w = open(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.scroll("log-test", ScrollDelta::Pixels(point(px(0.), px(-50.))), cx);
        window.render_frame(cx);
    })
    .unwrap();
    let first = state.read_with(cx, |s, _| s.last_rows()[0].row);
    assert_eq!(first, 2);
    state.update(cx, |s, cx| s.center_on_row(500, cx));
    render(cx, w);
    let row = state.read_with(cx, |s, _| s.last_rows().iter().find(|g| g.row == 500).cloned()).expect("row 500 visible");
    let viewport_mid = common::bounds(cx, w, "log-test").center().y;
    assert!((row.bounds.center().y - viewport_mid).abs() <= px(1.), "row 500 at {:?}, mid {viewport_mid:?}", row.bounds);
}

#[gpui_kit::test]
fn wrap_toggle_and_set_rows_reset_safely(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let w = open(cx, state.clone());
    state.update(cx, |s, cx| s.center_on_row(99, cx));
    render(cx, w);
    state.update(cx, |s, cx| {
        s.set_wrap(true, cx);
        s.set_rows(rows(3, None), cx);
    });
    render(cx, w);
    let geo = state.read_with(cx, |s, _| s.last_rows().to_vec());
    assert_eq!(geo.len(), 3);
    assert_eq!(geo[0].row, 0);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-ui --test logview`
Expected: compile errors, `LogView`, `LogViewState` and `LogViewEvent` are not found.

- [ ] **Step 3: Implement `state.rs`**

```rust
use super::model::LogRowSource;
use super::scroll::{ScrollAnchor, ScrollGeometry};
use crate::theme::size;
use gpui_kit::{Bounds, Context, EventEmitter, Pixels, Size};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct RowGeometry {
    pub row: usize,
    pub bounds: Bounds<Pixels>,
    pub is_gap: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogViewEvent {
    Selected { row: usize, number: u64 },
    TogglePin { number: u64 },
}

/// Scroll position, selection, wrap and layout caches for one log panel.
pub struct LogViewState {
    pub(crate) rows: Rc<dyn LogRowSource>,
    pub(crate) anchor: ScrollAnchor,
    pub(crate) scroll_x: f32,
    pub(crate) max_scroll_x: f32,
    pub(crate) wrap: bool,
    pub(crate) selected: Option<u64>,
    pub(crate) force_hover_row: Option<usize>,
    pub(crate) pending_center: Option<usize>,
    pub(crate) viewport: Size<Pixels>,
    pub(crate) row_h: f32,
    /// Measured heights of wrapped rows for `heights_width`; unmeasured rows use one row height.
    pub(crate) heights: HashMap<usize, f32>,
    pub(crate) heights_width: f32,
    pub(crate) last_rows: Vec<RowGeometry>,
    initialized: bool,
}

impl EventEmitter<LogViewEvent> for LogViewState {}

impl LogViewState {
    pub fn new(rows: Rc<dyn LogRowSource>) -> Self {
        Self {
            rows,
            anchor: ScrollAnchor::default(),
            scroll_x: 0.,
            max_scroll_x: 0.,
            wrap: false,
            selected: None,
            force_hover_row: None,
            pending_center: None,
            viewport: Size::default(),
            row_h: 23.,
            heights: HashMap::new(),
            heights_width: 0.,
            last_rows: Vec::new(),
            initialized: false,
        }
    }

    pub fn rows(&self) -> &Rc<dyn LogRowSource> {
        &self.rows
    }

    pub fn set_rows(&mut self, rows: Rc<dyn LogRowSource>, cx: &mut Context<Self>) {
        self.rows = rows;
        self.heights.clear();
        self.anchor = self.with_geometry(|g| g.clamp(self.anchor));
        cx.notify();
    }

    pub fn wrap(&self) -> bool {
        self.wrap
    }

    pub fn set_wrap(&mut self, wrap: bool, cx: &mut Context<Self>) {
        self.wrap = wrap;
        self.scroll_x = 0.;
        self.heights.clear();
        cx.notify();
    }

    pub fn selected(&self) -> Option<u64> {
        self.selected
    }

    pub fn set_selected(&mut self, number: Option<u64>, cx: &mut Context<Self>) {
        self.selected = number;
        cx.notify();
    }

    /// Centres `row` on the next layout (the viewport is only known during prepaint).
    pub fn center_on_row(&mut self, row: usize, cx: &mut Context<Self>) {
        self.pending_center = Some(row);
        cx.notify();
    }

    /// Paint `row` as hovered (gallery specimens).
    pub fn force_hover_row(&mut self, row: Option<usize>, cx: &mut Context<Self>) {
        self.force_hover_row = row;
        cx.notify();
    }

    /// Bounds of the rows painted in the last frame (window coordinates); used by tests and capture geometry.
    pub fn last_rows(&self) -> &[RowGeometry] {
        &self.last_rows
    }

    pub(crate) fn height_of(&self, row: usize) -> f32 {
        if self.rows.is_gap(row) {
            size::GAP_ROW_H
        } else if self.wrap {
            self.heights.get(&row).copied().unwrap_or(self.row_h)
        } else {
            self.row_h
        }
    }

    pub(crate) fn with_geometry<R>(&self, f: impl FnOnce(&ScrollGeometry) -> R) -> R {
        let height_of = |row: usize| self.height_of(row);
        let geometry = ScrollGeometry {
            row_count: self.rows.row_count(),
            viewport: f32::from(self.viewport.height),
            pad: size::LOGS_PAD_Y,
            height_of: &height_of,
        };
        f(&geometry)
    }

    /// Called at the start of prepaint with the element size and current metrics.
    pub(crate) fn begin_layout(&mut self, viewport: Size<Pixels>, text_width: f32, row_h: f32) {
        if self.row_h != row_h || (self.wrap && self.heights_width != text_width) {
            self.heights.clear();
        }
        self.row_h = row_h;
        self.heights_width = text_width;
        self.viewport = viewport;
        if !self.initialized {
            self.anchor = self.with_geometry(|g| g.top());
            self.initialized = true;
        }
        self.anchor = match self.pending_center.take() {
            Some(row) => self.with_geometry(|g| g.center_on(row)),
            None => self.with_geometry(|g| g.clamp(self.anchor)),
        };
    }

    pub(crate) fn record_heights(&mut self, measured: &[(usize, f32)]) {
        for &(row, h) in measured {
            self.heights.insert(row, h);
        }
        self.anchor = self.with_geometry(|g| g.clamp(self.anchor));
    }

    pub(crate) fn visible_rows(&self) -> Vec<(usize, f32)> {
        self.with_geometry(|g| g.visible(self.anchor))
    }

    pub(crate) fn thumb(&self) -> Option<(f32, f32)> {
        self.with_geometry(|g| g.thumb(self.anchor))
    }

    pub(crate) fn scroll_by_px(&mut self, dy: f32, dx: f32) {
        self.anchor = self.with_geometry(|g| g.scroll_by(self.anchor, dy));
        if !self.wrap {
            self.scroll_x = (self.scroll_x + dx).clamp(0., self.max_scroll_x.max(0.));
        }
    }
}
```

- [ ] **Step 4: Implement `element.rs`**

```rust
use super::model::{LogRow, LogRowSource};
use super::state::{LogViewEvent, LogViewState, RowGeometry};
use super::text::{MAX_DISPLAY_BYTES, Segment, display_text, segments};
use crate::icons::IconName;
use crate::theme::{ActiveRoggTheme, Color, RoggTheme, size};
use crate::logview::RowTint;
use gpui_kit::prelude::*;
use gpui_kit::{
    App, BorderStyle, Bounds, ContentMask, Corners, DispatchPhase, Element, ElementId, Entity, Font, GlobalElementId, Hitbox,
    HitboxBehavior, InspectorElementId, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, ScrollWheelEvent,
    ShapedLine, SharedString, Style, TextAlign, TextRun, TransformationMatrix, Window, WrappedLine, fill, font, outline, point, px,
    relative, size as gsize,
};
use std::collections::HashMap;
use std::ops::Range;

pub struct LogViewElement {
    state: Entity<LogViewState>,
}

impl LogViewElement {
    pub fn new(state: Entity<LogViewState>) -> Self {
        Self { state }
    }
}

impl IntoElement for LogViewElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

enum Shaped {
    Line(ShapedLine),
    Wrapped(Vec<WrappedLine>),
}

enum RowBody {
    Gap(ShapedLine),
    Line {
        number: u64,
        number_line: ShapedLine,
        text: Shaped,
        pinned: bool,
        tint: Option<RowTint>,
        marks: Vec<(Range<usize>, Color)>,
        outlines: Vec<Range<usize>>,
    },
}

struct PaintRow {
    row: usize,
    bounds: Bounds<Pixels>,
    hitbox: Hitbox,
    body: RowBody,
}

pub struct LogViewPrepaint {
    hitbox: Hitbox,
    rows: Vec<PaintRow>,
    thumb: Option<Bounds<Pixels>>,
    theme: RoggTheme,
    selected: Option<u64>,
    force_hover_row: Option<usize>,
    text_left: Pixels,
    text_right: Pixels,
    scroll_x: Pixels,
}

fn run(len: usize, font: &Font, fg: Color, bg: Option<Color>) -> TextRun {
    TextRun { len, font: font.clone(), color: fg.hsla(), background_color: bg.map(|c| c.hsla()), underline: None, strikethrough: None }
}

fn shape_row(rows: &dyn LogRowSource, row: usize, theme: &RoggTheme, mono: &Font, ui: &Font, wrap_width: Option<Pixels>, window: &mut Window) -> RowBody {
    let p = theme.palette;
    match rows.row(row) {
        LogRow::Gap { omitted } => {
            let label: SharedString = format!("··· {omitted} lines omitted").into();
            let runs = [run(label.len(), ui, p.faint, None)];
            RowBody::Gap(window.text_system().shape_line(label, px(9.), &runs, None))
        }
        LogRow::Line(line) => {
            let (display, _) = display_text(&line.text, MAX_DISPLAY_BYTES);
            let display: SharedString = display.into();
            let base = if line.context { p.muted } else { p.log_text };
            let segs: Vec<Segment> = segments(display.len(), &line.spans, base, &p);
            let wrapped = wrap_width.is_some();
            let runs: Vec<TextRun> = segs.iter().map(|s| run(s.range.len(), mono, s.fg, if wrapped { s.hit_bg } else { None })).collect();
            let marks = if wrapped { vec![] } else { segs.iter().filter_map(|s| s.hit_bg.map(|bg| (s.range.clone(), bg))).collect() };
            let outlines = segs.iter().filter(|s| s.outline).map(|s| s.range.clone()).collect();
            let number_text: SharedString = line.number.to_string().into();
            let number_runs = [run(number_text.len(), mono, p.faint, None)];
            let number_line = window.text_system().shape_line(number_text, px(10.), &number_runs, None);
            let font_size = theme.metrics.log_font_size;
            let text = match wrap_width {
                Some(w) => Shaped::Wrapped(
                    window.text_system().shape_text(display, font_size, &runs, Some(w), None).map(|v| v.into_vec()).unwrap_or_default(),
                ),
                None => Shaped::Line(window.text_system().shape_line(display, font_size, &runs, None)),
            };
            RowBody::Line { number: line.number, number_line, text, pinned: line.pinned, tint: line.tint, marks, outlines }
        }
    }
}

fn body_height(body: &RowBody, row_h: Pixels) -> f32 {
    match body {
        RowBody::Gap(_) => size::GAP_ROW_H,
        RowBody::Line { text: Shaped::Line(_), .. } => f32::from(row_h),
        RowBody::Line { text: Shaped::Wrapped(lines), .. } => {
            let h: f32 = lines.iter().map(|l| f32::from(l.size(row_h).height)).sum();
            h.max(f32::from(row_h))
        }
    }
}

impl Element for LogViewElement {
    type RequestLayoutState = ();
    type PrepaintState = LogViewPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        style.flex_grow = 1.;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> LogViewPrepaint {
        let theme = cx.rogg().clone();
        let row_h = theme.metrics.row_height;
        let text_left = bounds.left() + px(size::GUTTER_PIN_W + size::GUTTER_NUMBER_W);
        let text_right = bounds.right() - px(size::LINE_PAD_R);
        let text_w = f32::from(text_right - text_left).max(10.);
        let mono = font(theme.fonts.mono.clone());
        let ui = font(theme.fonts.ui.clone());

        self.state.update(cx, |s, _| s.begin_layout(bounds.size, text_w, f32::from(row_h)));

        // Wrapped rows: shape the visible set, record heights, and repeat once so positions use real heights.
        let wrap = self.state.read(cx).wrap();
        let mut bodies: HashMap<usize, RowBody> = HashMap::new();
        for _ in 0..(if wrap { 2 } else { 1 }) {
            let rows = self.state.read(cx).rows().clone();
            let visible = self.state.read(cx).visible_rows();
            let mut measured = Vec::new();
            for (row, _) in visible {
                if !bodies.contains_key(&row) {
                    let body = shape_row(rows.as_ref(), row, &theme, &mono, &ui, wrap.then(|| px(text_w)), window);
                    measured.push((row, body_height(&body, row_h)));
                    bodies.insert(row, body);
                }
            }
            if wrap {
                self.state.update(cx, |s, _| s.record_heights(&measured));
            }
        }

        // Horizontal extent for no-wrap scrolling.
        let longest = self.state.read(cx).rows().longest_line_chars() as f32;
        let font_id = window.text_system().resolve_font(&mono);
        let advance = window.text_system().ch_advance(font_id, theme.metrics.log_font_size).map(f32::from).unwrap_or(7.);
        self.state.update(cx, |s, _| s.max_scroll_x = (longest * advance - text_w).max(0.));

        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let (visible, rows_src, selected, force_hover_row, scroll_x, thumb) = {
            let s = self.state.read(cx);
            (s.visible_rows(), s.rows().clone(), s.selected(), s.force_hover_row, s.scroll_x, s.thumb())
        };
        let mut rows = Vec::with_capacity(visible.len());
        let mut geometry = Vec::with_capacity(visible.len());
        for (row, y) in visible {
            let body = match bodies.remove(&row) {
                Some(b) => b,
                None => shape_row(rows_src.as_ref(), row, &theme, &mono, &ui, wrap.then(|| px(text_w)), window),
            };
            let h = self.state.read(cx).height_of(row);
            let row_bounds = Bounds::new(point(bounds.left(), bounds.top() + px(y)), gsize(bounds.size.width, px(h)));
            let row_hitbox = window.insert_hitbox(row_bounds, HitboxBehavior::Normal);
            geometry.push(RowGeometry { row, bounds: row_bounds, is_gap: matches!(body, RowBody::Gap(_)) });
            rows.push(PaintRow { row, bounds: row_bounds, hitbox: row_hitbox, body });
        }
        self.state.update(cx, |s, _| s.last_rows = geometry);

        let thumb = thumb.map(|(top, len)| {
            let track = f32::from(bounds.size.height);
            let h = (track * len).max(size::SCROLL_THUMB_MIN_H);
            let y = (track - h) * (top / (1. - len).max(f32::EPSILON)).clamp(0., 1.);
            Bounds::new(
                point(bounds.right() - px(size::SCROLL_THUMB_W + 2.), bounds.top() + px(y)),
                gsize(px(size::SCROLL_THUMB_W), px(h)),
            )
        });

        LogViewPrepaint { hitbox, rows, thumb, theme, selected, force_hover_row, text_left, text_right, scroll_x: px(scroll_x) }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        pre: &mut LogViewPrepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let p = pre.theme.palette;
        let m = pre.theme.metrics;
        let row_h = m.row_height;
        window.paint_quad(fill(bounds, p.bg));
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for row in &pre.rows {
                match &row.body {
                    RowBody::Gap(label) => {
                        window.paint_quad(fill(row.bounds, p.bg));
                        let origin = point(row.bounds.left() + px(size::GAP_ROW_INSET), row.bounds.top());
                        label.paint(origin, row.bounds.size.height, TextAlign::Left, None, window, cx).ok();
                    }
                    RowBody::Line { number, number_line, text, pinned, tint, marks, outlines } => {
                        let hovered = pre.force_hover_row == Some(row.row) || row.hitbox.is_hovered(window);
                        let selected = pre.selected == Some(*number);
                        let bg = if selected {
                            Some(p.selection)
                        } else if let Some(t) = tint {
                            Some(t.resolve(&p))
                        } else if hovered {
                            Some(p.hover)
                        } else {
                            None
                        };
                        if let Some(bg) = bg {
                            window.paint_quad(fill(row.bounds, bg));
                        }
                        if selected {
                            window.paint_quad(fill(Bounds::new(row.bounds.origin, gsize(px(2.), row.bounds.size.height)), p.accent));
                        }
                        let pin = Bounds::new(
                            point(row.bounds.left() + px((size::GUTTER_PIN_W - size::PIN_GLYPH) / 2.), row.bounds.top() + m.pin_top),
                            gsize(px(size::PIN_GLYPH), px(size::PIN_GLYPH)),
                        );
                        if *pinned {
                            window.paint_svg(pin, IconName::PinFilled.path().into(), None, TransformationMatrix::unit(), p.accent.hsla(), cx).ok();
                        } else if hovered {
                            window.paint_svg(pin, IconName::Pin.path().into(), None, TransformationMatrix::unit(), p.faint.hsla().opacity(0.7), cx).ok();
                        }
                        let number_x =
                            row.bounds.left() + px(size::GUTTER_PIN_W + size::GUTTER_NUMBER_W - size::NUMBER_PAD_R) - number_line.width();
                        number_line.paint(point(number_x, row.bounds.top()), row_h, TextAlign::Left, None, window, cx).ok();

                        let clip = Bounds::from_corners(point(pre.text_left, row.bounds.top()), point(pre.text_right, row.bounds.bottom()));
                        let origin = point(pre.text_left - pre.scroll_x, row.bounds.top());
                        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| match text {
                            Shaped::Line(line) => {
                                let box_h = line.ascent + line.descent;
                                let box_y = row.bounds.top() + (row_h - box_h) / 2.;
                                for (range, color) in marks {
                                    let x0 = line.x_for_index(range.start);
                                    let x1 = line.x_for_index(range.end);
                                    let mark = Bounds::new(point(origin.x + x0, box_y), gsize(x1 - x0, box_h));
                                    window.paint_quad(fill(mark, *color).corner_radii(Corners::all(px(2.))));
                                }
                                line.paint(origin, row_h, TextAlign::Left, None, window, cx).ok();
                                for range in outlines {
                                    let x0 = line.x_for_index(range.start);
                                    let x1 = line.x_for_index(range.end);
                                    let ring = Bounds::new(point(origin.x + x0 - px(2.), box_y - px(2.)), gsize(x1 - x0 + px(4.), box_h + px(4.)));
                                    window.paint_quad(outline(ring, p.accent, BorderStyle::Solid));
                                }
                            }
                            Shaped::Wrapped(lines) => {
                                let mut y = origin.y;
                                for l in lines {
                                    l.paint(point(origin.x, y), row_h, TextAlign::Left, None, window, cx).ok();
                                    y += l.size(row_h).height;
                                }
                            }
                        });
                    }
                }
            }
            if let Some(thumb) = pre.thumb {
                window.paint_quad(fill(thumb, p.scrollbar_thumb).corner_radii(Corners::all(px(3.))));
            }
        });

        let state = self.state.clone();
        let hitbox = pre.hitbox.clone();
        window.on_mouse_event(move |e: &ScrollWheelEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || !hitbox.should_handle_scroll(window) {
                return;
            }
            let delta = e.delta.pixel_delta(row_h);
            state.update(cx, |s, cx| {
                s.scroll_by_px(-f32::from(delta.y), -f32::from(delta.x));
                cx.notify();
            });
            cx.stop_propagation();
        });

        let targets: Vec<(Hitbox, usize, u64)> = pre
            .rows
            .iter()
            .filter_map(|r| match r.body {
                RowBody::Line { number, .. } => Some((r.hitbox.clone(), r.row, number)),
                RowBody::Gap(_) => None,
            })
            .collect();
        let state = self.state.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || e.button != MouseButton::Left {
                return;
            }
            if let Some((hb, row, number)) = targets.iter().find(|(hb, _, _)| hb.is_hovered(window)) {
                let in_gutter = e.position.x < hb.bounds.left() + px(size::GUTTER_PIN_W);
                let (row, number) = (*row, *number);
                state.update(cx, |s, cx| {
                    if in_gutter {
                        cx.emit(LogViewEvent::TogglePin { number });
                    } else {
                        s.selected = Some(number);
                        cx.emit(LogViewEvent::Selected { row, number });
                    }
                    cx.notify();
                });
                cx.stop_propagation();
            }
        });

        let row_hitboxes: Vec<(usize, Hitbox)> = pre.rows.iter().map(|r| (r.row, r.hitbox.clone())).collect();
        let hovered_now = row_hitboxes.iter().find(|(_, h)| h.is_hovered(window)).map(|(r, _)| *r);
        window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, _| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            let now = row_hitboxes.iter().find(|(_, h)| h.is_hovered(window)).map(|(r, _)| *r);
            if now != hovered_now {
                window.refresh();
            }
        });
    }
}
```

Update `crates/rogg-ui/src/logview/mod.rs`:
```rust
mod element;
mod model;
pub mod scroll;
mod state;
pub mod text;

pub use element::LogViewElement;
pub use model::{LogLine, LogRow, LogRowSource, RowTint, Span, SpanStyle, VecRows};
pub use state::{LogViewEvent, LogViewState, RowGeometry};

use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, SharedString, TestSupportExt, Window, div};

/// A log panel: a test-support container filling its parent around the custom row element.
#[derive(IntoElement)]
pub struct LogView {
    id: SharedString,
    state: Entity<LogViewState>,
}

impl LogView {
    pub fn new(id: impl Into<SharedString>, state: Entity<LogViewState>) -> Self {
        Self { id: id.into(), state }
    }
}

impl RenderOnce for LogView {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().id(self.id).test_support().size_full().min_h_0().overflow_hidden().child(LogViewElement::new(self.state))
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rogg-ui --test logview && cargo test -p rogg-ui`
Expected: 4 log view tests pass and the full rogg-ui suite stays green. If `click_at` takes the offset in a different form (window-relative instead of element-relative), adjust the test call only, as described in research notes §15.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): virtualized log view element with pins, hits, gaps, wrap and scrolling

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Empty states and dialogs

**Files:**
- Create: `crates/rogg-ui/src/empty.rs`, `crates/rogg-ui/src/dialogs.rs`
- Modify: `crates/rogg-ui/src/lib.rs` (add `pub mod dialogs; pub mod empty;`)
- Test: `crates/rogg-ui/tests/dialogs.rs`

**Interfaces:**
- Consumes: `controls::{IconButton, ActionButton}`, `sidebar::RecentItem`.
- Produces:
  - `empty::EmptyState::new(id, title)`, with `.body(..)`, `.child(impl IntoElement)`
  - `empty::{no_matches(id), welcome(recent, on_open_file, on_open_folder, on_recent), loading(label, Option<f32>, on_cancel), error(message, path, on_retry)}`, each returning `EmptyState`
  - `dialogs::DialogFrame::new(title)`, with `.description(..)`, `.child(..)`, `.note(..)`, `.on_close(Fn)`. Its element id is `dialog`, and the close button is `dialog-close`.
  - `dialogs::ModalLayer::new(impl IntoElement)`, with `.on_dismiss(Fn)`. The id is `modal-layer`; clicking the backdrop dismisses.
  - `dialogs::{RuleRow, ShortcutRow, PickerItem}` rows
  - `dialogs::ProfileRuleView { swatch: Color, pattern: SharedString, summary: SharedString }`
  - `dialogs::{all_tabs, profile, appearance, shortcuts, history, statistics, export, go_to_line}`, which return a `DialogFrame` with title and body. Callers add the description and note.

Dialogs are our own overlay, not gpui-kit's `Dialog`. The kit hard-codes its own padding, title and footer styles, while the reference `dialog` needs exact values: 500 px wide, 22 px padding, 10 px radius, 17 px / 550 title, `#0007` backdrop.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-ui/tests/dialogs.rs`:
```rust
mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, point, px};
use rogg_ui::dialogs::{ModalLayer, shortcuts};
use rogg_ui::empty::no_matches;
use std::cell::Cell;
use std::rc::Rc;

#[gpui_kit::test]
fn dialog_frame_is_500_wide_and_centred(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1440., 900., |_, _| {
        ModalLayer::new(shortcuts(&[("Find in focused panel".into(), "⌘ F".into()), ("Edit filters".into(), "⌘ L".into())])).into_any_element()
    });
    let frame = common::bounds(cx, w, "dialog");
    assert_eq!(frame.size.width, px(500.));
    assert!((frame.center().x - px(720.)).abs() <= px(0.5));
    assert!(common::exists(cx, w, "shortcut-row-1"));
}

#[gpui_kit::test]
fn backdrop_click_dismisses_but_frame_click_does_not(cx: &mut TestAppContext) {
    common::setup(cx);
    let dismissed = Rc::new(Cell::new(0));
    let d = dismissed.clone();
    let w = common::open(cx, 1440., 900., move |_, _| {
        let d = d.clone();
        ModalLayer::new(shortcuts(&[])).on_dismiss(move |_, _| d.set(d.get() + 1)).into_any_element()
    });
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click_at("dialog", point(px(250.), px(10.)), cx);
        window.click_at("modal-layer", point(px(5.), px(5.)), cx);
    })
    .unwrap();
    assert_eq!(dismissed.get(), 1);
}

#[gpui_kit::test]
fn no_matches_fills_its_box(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 600., 300., |_, _| no_matches("empty-results").into_any_element());
    assert_eq!(common::bounds(cx, w, "empty-results").size.height, px(300.));
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-ui --test dialogs`
Expected: compile error, `rogg_ui::dialogs` is not found.

- [ ] **Step 3: Implement `empty.rs`**

```rust
use crate::controls::ActionButton;
use crate::icons::{IconName, icon};
use crate::sidebar::RecentItem;
use crate::theme::ActiveRoggTheme;
use crate::{Callback, IndexCallback};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, FontWeight, SharedString, TestSupportExt, Window, div, px, relative};

/// `.empty`: centred column with a 15px title, 12px muted body and optional actions.
#[derive(IntoElement)]
pub struct EmptyState {
    id: SharedString,
    title: SharedString,
    body: Option<SharedString>,
    children: Vec<AnyElement>,
}

impl EmptyState {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self { id: id.into(), title: title.into(), body: None, children: vec![] }
    }

    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        div()
            .id(self.id)
            .test_support()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .font_family(t.fonts.ui.clone())
            .text_size(px(12.))
            .text_color(p.muted)
            .child(div().text_size(px(15.)).font_weight(FontWeight::MEDIUM).text_color(p.text).child(self.title))
            .when_some(self.body, |d, b| d.child(b))
            .children(self.children)
    }
}

fn call(cb: Callback) -> impl Fn(&mut Window, &mut App) + 'static {
    move |w, cx| cb(w, cx)
}

pub fn no_matches(id: impl Into<SharedString>) -> EmptyState {
    EmptyState::new(id, "No matching lines").body("Enable an include filter or pin a line in the full log.")
}

pub fn welcome(recent: Vec<RecentItem>, on_open_file: Callback, on_open_folder: Callback, on_recent: IndexCallback) -> EmptyState {
    let mut state = EmptyState::new("welcome", "Open a log to start")
        .body("Drop files, folders or archives here, or pipe into rogg from a terminal.")
        .child(
            div()
                .flex()
                .gap(px(7.))
                .child(ActionButton::new("open-file", "Open file…  ⌘O").primary().on_click(call(on_open_file)))
                .child(ActionButton::new("open-folder", "Open folder…  ⇧⌘O").on_click(call(on_open_folder))),
        );
    if !recent.is_empty() {
        state = state.child(WelcomeRecent { items: recent, on_recent });
    }
    state
}

/// Recent entries under the welcome actions.
#[derive(IntoElement)]
struct WelcomeRecent {
    items: Vec<RecentItem>,
    on_recent: IndexCallback,
}

impl RenderOnce for WelcomeRecent {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let on_recent = self.on_recent;
        div().mt(px(14.)).flex().flex_col().gap(px(2.)).w(px(320.)).children(self.items.into_iter().enumerate().map(move |(i, item)| {
            let on_recent = on_recent.clone();
            let fg = if item.missing { p.faint } else { p.muted };
            div()
                .id(SharedString::from(format!("welcome-recent-{i}")))
                .test_support()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(10.))
                .py(px(7.))
                .rounded(px(5.))
                .text_color(fg)
                .cursor_pointer()
                .hover(move |s| s.bg(p.hover))
                .child(icon(if item.is_folder { IconName::Folder } else { IconName::File }).size(px(13.)).text_color(fg))
                .child(item.label)
                .on_click(move |_, w, cx| on_recent(i, w, cx))
        }))
    }
}

pub fn loading(label: impl Into<SharedString>, fraction: Option<f32>, on_cancel: Callback) -> EmptyState {
    EmptyState::new("loading", label)
        .child(LoadingBar { fraction })
        .child(ActionButton::new("cancel", "Cancel").on_click(call(on_cancel)))
}

pub fn error(message: impl Into<SharedString>, path: impl Into<SharedString>, on_retry: Callback) -> EmptyState {
    EmptyState::new("error", message).body(path).child(ActionButton::new("retry", "Retry").primary().on_click(call(on_retry)))
}

#[derive(IntoElement)]
struct LoadingBar {
    fraction: Option<f32>,
}

impl RenderOnce for LoadingBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        div()
            .w(px(220.))
            .h(px(3.))
            .rounded(px(2.))
            .bg(p.border)
            .child(div().h_full().rounded(px(2.)).bg(p.accent).w(relative(self.fraction.unwrap_or(0.3).clamp(0., 1.))))
    }
}
```

- [ ] **Step 4: Implement `dialogs.rs`**

```rust
use crate::controls::{ActionButton, IconButton};
use crate::icons::{IconName, icon};
use crate::theme::{ActiveRoggTheme, Color, size};
use crate::{Callback, IndexCallback};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, BoxShadow, Entity, FontWeight, MouseButton, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

/// The reference `dialog`: 500px, 22px padding, 10px radius, side background, large soft shadow.
#[derive(IntoElement)]
pub struct DialogFrame {
    title: SharedString,
    description: Option<SharedString>,
    body: Vec<AnyElement>,
    note: Option<SharedString>,
    on_close: Option<Callback>,
}

impl DialogFrame {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self { title: title.into(), description: None, body: vec![], note: None, on_close: None }
    }

    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
        self
    }

    pub fn note(mut self, text: impl Into<SharedString>) -> Self {
        self.note = Some(text.into());
        self
    }

    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for DialogFrame {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let on_close = self.on_close;
        div()
            .id("dialog")
            .test_support()
            .occlude()
            .w(px(size::DIALOG_W))
            .p(px(size::DIALOG_PAD))
            .bg(p.side)
            .border_1()
            .border_color(p.border)
            .rounded(px(10.))
            .shadow(vec![BoxShadow::new(px(0.), px(20.), p.dialog_shadow.hsla()).blur_radius(px(90.))])
            .text_color(p.text)
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .child(div().pb(px(6.)).text_size(px(17.)).font_weight(FontWeight(550.)).child(self.title))
                    .child(IconButton::glyph("dialog-close", "✕").on_click(move |w, cx| {
                        if let Some(f) = &on_close {
                            f(w, cx)
                        }
                    })),
            )
            .when_some(self.description, |d, text| {
                d.child(div().my(px(12.)).text_size(px(12.)).line_height(px(20.4)).text_color(p.muted).child(text))
            })
            .children(self.body)
            .when_some(self.note, |d, text| d.child(div().my(px(10.)).text_size(px(10.)).line_height(px(17.)).text_color(p.muted).child(text)))
    }
}

/// Full-window backdrop that centres a dialog; clicking outside the dialog calls `on_dismiss`.
#[derive(IntoElement)]
pub struct ModalLayer {
    child: AnyElement,
    on_dismiss: Option<Callback>,
}

impl ModalLayer {
    pub fn new(child: impl IntoElement) -> Self {
        Self { child: child.into_any_element(), on_dismiss: None }
    }

    pub fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ModalLayer {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let on_dismiss = self.on_dismiss;
        div()
            .id("modal-layer")
            .test_support()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(p.backdrop)
            .on_mouse_down(MouseButton::Left, move |_, w, cx| {
                if let Some(f) = &on_dismiss {
                    f(w, cx)
                }
            })
            .child(self.child)
    }
}

/// `.profile-rule`: top border, optional 21px swatch, leading code/text, trailing muted summary.
#[derive(IntoElement)]
pub struct RuleRow {
    id: SharedString,
    swatch: Option<Color>,
    lead: SharedString,
    code: bool,
    trailing: Option<SharedString>,
}

impl RuleRow {
    pub fn code(id: impl Into<SharedString>, lead: impl Into<SharedString>) -> Self {
        Self { id: id.into(), swatch: None, lead: lead.into(), code: true, trailing: None }
    }

    pub fn text(id: impl Into<SharedString>, lead: impl Into<SharedString>) -> Self {
        Self { id: id.into(), swatch: None, lead: lead.into(), code: false, trailing: None }
    }

    pub fn swatch(mut self, c: Color) -> Self {
        self.swatch = Some(c);
        self
    }

    pub fn trailing(mut self, text: impl Into<SharedString>) -> Self {
        self.trailing = Some(text.into());
        self
    }
}

impl RenderOnce for RuleRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        div()
            .id(self.id)
            .test_support()
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(14.))
            .border_t_1()
            .border_color(p.border)
            .text_size(px(12.))
            .when_some(self.swatch, |d, c| d.child(div().flex_none().size(px(21.)).rounded(px(4.)).border_1().border_color(p.swatch_border).bg(c)))
            .child(div().when(self.code, |d| d.font_family(t.fonts.mono.clone())).child(self.lead))
            .when_some(self.trailing, |d, s| d.child(div().ml_auto().text_size(px(11.)).text_color(p.muted).child(s)))
    }
}

/// `.shortcut`: label left, accent monospace keys right, bottom border.
#[derive(IntoElement)]
pub struct ShortcutRow {
    id: SharedString,
    label: SharedString,
    keys: SharedString,
}

impl RenderOnce for ShortcutRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        div()
            .id(self.id)
            .test_support()
            .flex()
            .justify_between()
            .py(px(9.))
            .border_b_1()
            .border_color(p.border)
            .text_size(px(12.))
            .child(self.label)
            .child(div().font_family(t.fonts.mono.clone()).text_size(px(11.)).text_color(p.accent).child(self.keys))
    }
}

/// `.tab-picker-item`: file icon, label, check mark on the current entry.
#[derive(IntoElement)]
pub struct PickerItem {
    id: SharedString,
    label: SharedString,
    current: bool,
    on_click: Option<Callback>,
}

impl RenderOnce for PickerItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let on_click = self.on_click;
        div()
            .id(self.id)
            .test_support()
            .flex()
            .items_center()
            .w_full()
            .gap(px(10.))
            .p(px(9.))
            .rounded(px(5.))
            .text_size(px(12.))
            .cursor_pointer()
            .when(self.current, |d| d.bg(p.selection))
            .hover(move |s| s.bg(p.hover))
            .child(icon(IconName::File).size(px(15.)).text_color(p.muted))
            .child(self.label)
            .when(self.current, |d| d.child(div().ml_auto().text_color(p.accent).child("✓")))
            .on_click(move |_, w, cx| {
                if let Some(f) = &on_click {
                    f(w, cx)
                }
            })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileRuleView {
    pub swatch: Color,
    pub pattern: SharedString,
    pub summary: SharedString,
}

pub fn all_tabs(tabs: &[(SharedString, bool)], on_pick: IndexCallback) -> DialogFrame {
    tabs.iter().enumerate().fold(DialogFrame::new("Open files"), |d, (i, (label, current))| {
        let on_pick = on_pick.clone();
        d.child(PickerItem {
            id: format!("picker-{i}").into(),
            label: label.clone(),
            current: *current,
            on_click: Some(Rc::new(move |w, cx| on_pick(i, w, cx))),
        })
    })
}

pub fn profile(name: &str, rules: &[ProfileRuleView]) -> DialogFrame {
    rules.iter().enumerate().fold(DialogFrame::new(format!("Highlighting · {name}")), |d, (i, r)| {
        d.child(RuleRow::code(format!("rule-{i}"), r.pattern.clone()).swatch(r.swatch).trailing(r.summary.clone()))
    })
}

pub fn appearance(rows: &[(SharedString, SharedString)]) -> DialogFrame {
    rows.iter().enumerate().fold(DialogFrame::new("Appearance"), |d, (i, (k, v))| d.child(RuleRow::text(format!("setting-{i}"), k.clone()).trailing(v.clone())))
}

pub fn shortcuts(rows: &[(SharedString, SharedString)]) -> DialogFrame {
    rows.iter().enumerate().fold(DialogFrame::new("Keyboard navigation"), |d, (i, (label, keys))| {
        d.child(ShortcutRow { id: format!("shortcut-row-{i}").into(), label: label.clone(), keys: keys.clone() })
    })
}

pub fn history(entries: &[SharedString]) -> DialogFrame {
    entries.iter().enumerate().fold(DialogFrame::new("Filter history"), |d, (i, e)| d.child(RuleRow::code(format!("history-{i}"), e.clone())))
}

pub fn statistics(rows: &[(SharedString, SharedString)]) -> DialogFrame {
    rows.iter().enumerate().fold(DialogFrame::new("Filtered log statistics"), |d, (i, (k, v))| d.child(RuleRow::text(format!("stat-{i}"), k.clone()).trailing(v.clone())))
}

pub fn export(options: &[(SharedString, bool)], on_toggle: IndexCallback, on_export: Callback) -> DialogFrame {
    let frame = options.iter().enumerate().fold(DialogFrame::new("Export filtered lines"), |d, (i, (label, on))| {
        let on_toggle = on_toggle.clone();
        d.child(
            div()
                .id(SharedString::from(format!("export-option-{i}")))
                .cursor_pointer()
                .on_click(move |_, w, cx| on_toggle(i, w, cx))
                .child(RuleRow::text(format!("export-row-{i}"), label.clone()).trailing(if *on { "On" } else { "Off" })),
        )
    });
    frame.child(div().flex().justify_end().pt(px(14.)).child(ActionButton::new("export-run", "Export…").primary().on_click(move |w, cx| on_export(w, cx))))
}

pub fn go_to_line(input: Entity<InputState>) -> DialogFrame {
    DialogFrame::new("Go to line").child(GoToLineField { input })
}

#[derive(IntoElement)]
struct GoToLineField {
    input: Entity<InputState>,
}

impl RenderOnce for GoToLineField {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        div()
            .mt(px(12.))
            .px(px(8.))
            .py(px(6.))
            .bg(p.bg)
            .border_1()
            .border_color(p.accent)
            .rounded(px(4.))
            .child(Input::new(&self.input).appearance(false).p_0().font_family(t.fonts.mono.clone()).text_size(px(12.)).text_color(p.text))
    }
}
```

Add `pub mod dialogs; pub mod empty;` to `lib.rs`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rogg-ui --test dialogs`
Expected: 3 tests pass. If the frame click also dismisses, `.occlude()` is not stopping the backdrop handler. Add `.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())` to the frame, then re-run.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): empty states, dialog frame, modal layer and dialog catalog

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Main window layout

**Files:**
- Create: `crates/rogg-ui/src/layout.rs`
- Modify: `crates/rogg-ui/src/lib.rs` (add `pub mod layout;`)
- Test: `crates/rogg-ui/tests/layout.rs`

**Interfaces:**
- Produces: `layout::MainWindowParts { titlebar, sidebar: Option<AnyElement>, info_bar, top_header, top_body, band, bottom_header, bottom_body, status, overlay: Option<AnyElement>, split: f32 }`. Every field is an `AnyElement` except `sidebar`/`overlay` (optional) and `split`. `split` is the top pane's share of free height and defaults to `1.1 / 2.1`. Also `layout::MainWindowLayout::new(MainWindowParts)`. Element ids: `app`, `workspace`, `main`, `pane-top`, `pane-bottom`.

- [ ] **Step 1: Write the failing test**

`crates/rogg-ui/tests/layout.rs`:
```rust
mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, div, px};
use rogg_ui::chrome::{InfoBar, PaneHeader, StatusBar};
use rogg_ui::filter::FilterBand;
use rogg_ui::layout::{MainWindowLayout, MainWindowParts};
use rogg_ui::sidebar::Sidebar;
use rogg_ui::titlebar::TitleBar;

#[gpui_kit::test]
fn panes_split_remaining_height_1_1_to_1(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1440., 900., |_, _| {
        MainWindowLayout::new(MainWindowParts {
            titlebar: TitleBar::new(vec![]).into_any_element(),
            sidebar: Some(Sidebar::new().into_any_element()),
            info_bar: InfoBar::new(vec!["a.log".into()], "Default".into()).into_any_element(),
            top_header: PaneHeader::new("pane-top-head", "Full log", "").into_any_element(),
            top_body: div().size_full().into_any_element(),
            band: FilterBand::new(vec![]).into_any_element(),
            bottom_header: PaneHeader::new("pane-bottom-head", "Filtered log", "").into_any_element(),
            bottom_body: div().size_full().into_any_element(),
            status: StatusBar::new("UTF-8".into()).into_any_element(),
            overlay: None,
            split: MainWindowParts::DEFAULT_SPLIT,
        })
        .into_any_element()
    });
    assert_eq!(common::bounds(cx, w, "titlebar").origin.y, px(0.));
    assert_eq!(common::bounds(cx, w, "meta").origin.y, px(42.));
    assert_eq!(common::bounds(cx, w, "meta").origin.x, px(211.));
    assert_eq!(common::bounds(cx, w, "statusbar").bottom(), px(900.));
    let top = common::bounds(cx, w, "pane-top").size.height;
    let bottom = common::bounds(cx, w, "pane-bottom").size.height;
    assert!((f32::from(top) / f32::from(bottom) - 1.1).abs() < 0.01, "top {top:?} bottom {bottom:?}");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p rogg-ui --test layout`
Expected: compile error, `rogg_ui::layout` is not found.

- [ ] **Step 3: Implement `layout.rs`**

```rust
use crate::theme::ActiveRoggTheme;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, TestSupportExt, Window, div, px, relative};

pub struct MainWindowParts {
    pub titlebar: AnyElement,
    pub sidebar: Option<AnyElement>,
    pub info_bar: AnyElement,
    pub top_header: AnyElement,
    pub top_body: AnyElement,
    pub band: AnyElement,
    pub bottom_header: AnyElement,
    pub bottom_body: AnyElement,
    pub status: AnyElement,
    pub overlay: Option<AnyElement>,
    /// Top pane share of the height left after fixed rows (reference `flex:1.1` vs `flex:1`).
    pub split: f32,
}

impl MainWindowParts {
    pub const DEFAULT_SPLIT: f32 = 1.1 / 2.1;
}

/// Composes the window exactly like the reference `.app`: title bar, then sidebar + main column.
#[derive(IntoElement)]
pub struct MainWindowLayout {
    parts: MainWindowParts,
}

impl MainWindowLayout {
    pub fn new(parts: MainWindowParts) -> Self {
        Self { parts }
    }
}

impl RenderOnce for MainWindowLayout {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        let parts = self.parts;
        let split = parts.split.clamp(0.1, 0.9);
        let pane = |id: &'static str, grow: f32, header: AnyElement, body: AnyElement| {
            div()
                .id(id)
                .test_support()
                .relative()
                .flex()
                .flex_col()
                .flex_grow(grow)
                .flex_shrink()
                .flex_basis(relative(0.))
                .min_h_0()
                .child(header)
                .child(div().flex_1().min_h_0().child(body))
        };
        div()
            .id("app")
            .test_support()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.text)
            .font_family(t.fonts.ui.clone())
            .text_size(px(13.))
            .child(parts.titlebar)
            .child(
                div()
                    .id("workspace")
                    .test_support()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .children(parts.sidebar)
                    .child(
                        div()
                            .id("main")
                            .test_support()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(parts.info_bar)
                            .child(pane("pane-top", split, parts.top_header, parts.top_body))
                            .child(parts.band)
                            .child(pane("pane-bottom", 1. - split, parts.bottom_header, parts.bottom_body))
                            .child(parts.status),
                    ),
            )
            .children(parts.overlay)
    }
}
```

The status bar in the reference spans only the main column (`.footer` is inside `.main`), so it sits under the bottom pane, not under the sidebar. That is why it is a child of `main` above.

Add `pub mod layout;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-ui`
Expected: every rogg-ui test passes.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): main window layout composing all regions

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 13: Gallery app with component pages

**Files:**
- Create: `crates/rogg-gallery/src/{gallery.rs,pages.rs,fixtures.rs}`
- Modify: `crates/rogg-gallery/src/lib.rs`, `crates/rogg-gallery/src/main.rs`
- Test: `crates/rogg-gallery/tests/pages.rs`

**Interfaces:**
- Consumes: everything in `rogg_ui`.
- Produces:
  - `gallery::Page::{Tokens, Controls, Tabs, Sidebar, Filter, LogView, Chrome, Empty, Dialogs}` with `Page::ALL`, `.slug()`, `.title()`, `Page::from_slug(&str)`
  - `gallery::GalleryView::new(page, chrome: bool, window, cx)`. `chrome = false` renders only the page (used by capture).
  - `fixtures::*`: reference data shared with the replica (bundle files, recent entries, terms, dialog contents, specimen log rows)
  - `main.rs`: `rogg-gallery [--page <slug>]` opens the interactive gallery
- Out of scope: focus rings (2 px accent, 2 px offset) arrive with keyboard navigation in M2. The reference shows none in its default state.

- [ ] **Step 1: Write the failing page test**

`crates/rogg-gallery/tests/pages.rs`:
```rust
use gpui_kit::prelude::*;
use gpui_kit::{AppContext, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use rogg_gallery::gallery::{GalleryView, Page};
use rogg_ui::theme::{Appearance, Density, RoggTheme};

fn render_page(cx: &mut TestAppContext, page: Page, appearance: Appearance, density: Density) {
    cx.update(|cx| RoggTheme::new(appearance, density).install(cx));
    let handle = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(1440.), px(900.)) })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, move |window, cx| cx.new(|cx| GalleryView::new(page, true, window, cx))).unwrap().0
    });
    cx.update_window(handle, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn every_page_renders_in_every_theme(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init); // once per app; render_page only switches themes
    for page in Page::ALL {
        for appearance in [Appearance::Dark, Appearance::Light] {
            for density in [Density::Compact, Density::Comfortable] {
                render_page(cx, page, appearance, density);
            }
        }
    }
}

#[test]
fn slugs_round_trip() {
    for page in Page::ALL {
        assert_eq!(Page::from_slug(page.slug()), Some(page));
    }
    assert_eq!(Page::from_slug("nope"), None);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p rogg-gallery --test pages`
Expected: compile error, `rogg_gallery::gallery` is not found.

- [ ] **Step 3: Implement `fixtures.rs`**

```rust
//! Canned data. Bundle, tabs and dialog contents are copied verbatim from the reference mockup.
use gpui_kit::SharedString;
use rogg_ui::dialogs::ProfileRuleView;
use rogg_ui::filter::ChipState;
use rogg_ui::logview::{LogLine, LogRow, RowTint, Span, SpanStyle};
use rogg_ui::sidebar::{RecentItem, SidebarFooter, TreeRow};
use rogg_ui::theme::Color;

pub const BUNDLE: &str = "incident-2026-10-05.zip";

pub const FILES: [(&str, &str); 10] = [
    ("api.log", "972 MB"),
    ("worker.log", "841 MB"),
    ("gateway.log", "1.0 GB"),
    ("auth.log", "713 MB"),
    ("scheduler.log", "904 MB"),
    ("database.log", "867 MB"),
    ("cache.log", "624 MB"),
    ("metrics.log", "955 MB"),
    ("events.log", "701 MB"),
    ("system.log", "884 MB"),
];

pub fn sidebar_rows(active: &str) -> Vec<TreeRow> {
    let mut rows = vec![TreeRow::Folder { label: BUNDLE.into(), expanded: true, depth: 0 }];
    rows.extend(FILES.iter().map(|(name, size)| TreeRow::File {
        label: (*name).into(),
        size: (*size).into(),
        active: *name == active,
        depth: 1,
        failed: false,
    }));
    rows
}

pub fn recent() -> Vec<RecentItem> {
    vec![
        RecentItem { label: "staging / logs".into(), is_folder: true, missing: false },
        RecentItem { label: "deployment-10-04.log".into(), is_folder: false, missing: false },
    ]
}

pub fn sidebar_footer() -> SidebarFooter {
    SidebarFooter { title: "Support bundle".into(), lines: vec!["10 members · 8.4 GB expanded".into(), "Members open individually".into()] }
}

pub fn reference_terms() -> Vec<(SharedString, ChipState)> {
    vec![
        ("error".into(), ChipState::Include),
        ("timeout".into(), ChipState::Include),
        ("healthcheck".into(), ChipState::Exclude),
        ("retry".into(), ChipState::Off),
    ]
}

pub fn tabs_description() -> &'static str {
    "All open tabs, including those outside the visible header."
}

pub fn profile_rules() -> Vec<ProfileRuleView> {
    vec![
        ProfileRuleView { swatch: Color::rgb(0x744037), pattern: "ERROR".into(), summary: "Text + subtle row tint · both panels".into() },
        ProfileRuleView { swatch: Color::rgb(0x64502b), pattern: "WARN".into(), summary: "Text · both panels".into() },
        ProfileRuleView { swatch: Color::rgb(0x315064), pattern: "request_id=\\S+".into(), summary: "Match text · both panels".into() },
    ]
}

pub const PROFILE_DESCRIPTION: &str = "A named collection of line rules for both panels, plus the filter-match palette. Example profile preview.";
pub const PROFILE_NOTE: &str = "The implementation will support editable rules, priority, colors, and profile assignment. This dialog illustrates its placement.";

pub fn appearance_rows() -> Vec<(SharedString, SharedString)> {
    vec![
        ("Theme".into(), "System / Light / Dark".into()),
        ("Log font".into(), "System monospace · 12 px".into()),
        ("Line spacing".into(), "Compact / Comfortable".into()),
        ("Default wrapping".into(), "Off · override in each panel".into()),
    ]
}

pub const APPEARANCE_DESCRIPTION: &str = "Keep global appearance separate from the active highlighting profile.";
pub const APPEARANCE_NOTE: &str = "Placement preview. Use the controls above the mockup to try appearance and density.";

pub fn shortcut_rows() -> Vec<(SharedString, SharedString)> {
    vec![
        ("Find in focused panel".into(), "⌘ F".into()),
        ("Edit filters".into(), "⌘ L".into()),
        ("Pin / unpin selected line".into(), "⌘ B".into()),
        ("Go to line".into(), "⌘ G".into()),
        ("Move through results".into(), "↑ ↓".into()),
        ("Reveal in full log".into(), "Return".into()),
    ]
}

pub const SHORTCUTS_DESCRIPTION: &str = "Proposed shortcuts for the native app.";
pub const SHORTCUTS_NOTE: &str = "Browser keyboard shortcuts remain unchanged in this mockup.";

pub fn history_entries() -> Vec<SharedString> {
    vec!["error || timeout || healthcheck || retry".into(), "request_id=req_8f29 || connection reset".into()]
}

pub const HISTORY_DESCRIPTION: &str = "Restore an expression together with its term states and matching options.";
pub const HISTORY_NOTE: &str = "Illustrative history entries.";

fn line(number: u64, text: &str, spans: Vec<Span>, pinned: bool, context: bool, tint: Option<RowTint>) -> LogRow {
    LogRow::Line(LogLine { number, text: text.into(), spans, pinned, context, tint })
}

fn stamp() -> Span {
    Span { range: 0..23, style: SpanStyle::Faint }
}

/// One row per visual state: plain, hovered (forced by the page), selected, pinned, error tint, context, gap, hits, search, very long.
pub fn log_rows() -> Vec<LogRow> {
    let long = format!("2026-10-05 14:32:12.144  ERROR  client       upstream timeout {}", "trace_id=4bf92f3577b34da6a3ce929d0e0e4736 ".repeat(12));
    vec![
        line(120480, "2026-10-05 14:32:08.142  INFO   server       request started method=POST path=/v1/payments request_id=req_8f29", vec![stamp()], false, false, None),
        line(120481, "2026-10-05 14:32:08.213  DEBUG  router       matched route=payments.create upstream=payments-primary", vec![stamp()], false, false, None),
        line(120487, "2026-10-05 14:32:10.639  INFO   config       deployment revision=8d72f6a feature_flags={payments_v2:true}", vec![stamp()], true, false, None),
        line(120488, "2026-10-05 14:32:10.710  DEBUG  client       connection opened host=payments-primary.internal port=8443", vec![stamp()], false, true, None),
        LogRow::Gap { omitted: 3 },
        line(120492, "2026-10-05 14:32:12.144  ERROR  client       upstream timeout after 30000ms request_id=req_8f29",
            vec![stamp(), Span { range: 25..30, style: SpanStyle::Hit(0) }, Span { range: 54..61, style: SpanStyle::Hit(1) }], false, false, Some(RowTint::Error)),
        line(120495, "2026-10-05 14:32:13.357  ERROR  payments     authorization failed error=UPSTREAM_UNAVAILABLE request_id=req_8f29",
            vec![stamp(), Span { range: 25..30, style: SpanStyle::Hit(0) }, Span { range: 66..71, style: SpanStyle::Hit(0) }], false, false, Some(RowTint::Error)),
        line(120500, "2026-10-05 14:32:14.792  WARN   cache        timeout reading key=account:7239 fallback=database",
            vec![stamp(), Span { range: 45..52, style: SpanStyle::Hit(1) }, Span { range: 45..52, style: SpanStyle::SearchHit }], false, false, None),
        line(120503, "2026-10-05 14:32:15.215  ERROR  worker       job failed error=DEADLINE_EXCEEDED job_id=job_4271 attempt=3",
            vec![stamp(), Span { range: 25..30, style: SpanStyle::Hit(2) }], false, false, Some(RowTint::Error)),
        line(120504, "2026-10-05 14:32:15.286  INFO   queue        job scheduled for retry job_id=job_4271 delay=60s",
            vec![stamp(), Span { range: 63..68, style: SpanStyle::Hit(3) }], false, false, None),
        line(120505, &long, vec![stamp()], false, false, Some(RowTint::Error)),
    ]
}
```

- [ ] **Step 4: Implement `gallery.rs`**

```rust
use crate::{fixtures, pages};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{AppContext, Context, Entity, FontWeight, SharedString, Window, div, px};
use rogg_ui::controls::{FindField, Segmented, TextButton};
use rogg_ui::logview::{LogViewState, VecRows};
use rogg_ui::theme::{ActiveRoggTheme, Appearance, Density, RoggTheme};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Page {
    Tokens,
    Controls,
    Tabs,
    Sidebar,
    Filter,
    LogView,
    Chrome,
    Empty,
    Dialogs,
}

impl Page {
    pub const ALL: [Page; 9] =
        [Page::Tokens, Page::Controls, Page::Tabs, Page::Sidebar, Page::Filter, Page::LogView, Page::Chrome, Page::Empty, Page::Dialogs];

    pub fn slug(self) -> &'static str {
        match self {
            Page::Tokens => "tokens",
            Page::Controls => "controls",
            Page::Tabs => "tabs",
            Page::Sidebar => "sidebar",
            Page::Filter => "filter",
            Page::LogView => "logview",
            Page::Chrome => "chrome",
            Page::Empty => "empty",
            Page::Dialogs => "dialogs",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Tokens => "Tokens",
            Page::Controls => "Controls",
            Page::Tabs => "Title bar & tabs",
            Page::Sidebar => "Sidebar",
            Page::Filter => "Filter band",
            Page::LogView => "Log view",
            Page::Chrome => "Info, pane & status bars",
            Page::Empty => "Empty states",
            Page::Dialogs => "Dialogs",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| p.slug() == slug)
    }
}

pub struct Inputs {
    pub find_empty: Entity<InputState>,
    pub find_query: Entity<InputState>,
    pub expression: Entity<InputState>,
    pub goto: Entity<InputState>,
}

pub struct GalleryView {
    pub page: Page,
    pub chrome: bool,
    pub inputs: Inputs,
    pub log_compact: Entity<LogViewState>,
    pub log_wrapped: Entity<LogViewState>,
}

impl GalleryView {
    pub fn new(page: Page, chrome: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let inputs = Inputs {
            find_empty: FindField::state("Find in full log", window, cx),
            find_query: cx.new(|cx| InputState::new(window, cx).placeholder("Find in results").default_value("timeout")),
            expression: cx.new(|cx| InputState::new(window, cx).default_value("error || timeout || healthcheck || retry")),
            goto: cx.new(|cx| InputState::new(window, cx).placeholder("Line number, e.g. 120492")),
        };
        let rows = Rc::new(VecRows::new(fixtures::log_rows()));
        let log_compact = cx.new(|_| LogViewState::new(rows.clone()));
        log_compact.update(cx, |s, cx| {
            s.set_selected(Some(120492), cx);
            s.force_hover_row(Some(1), cx);
        });
        let log_wrapped = cx.new(|_| LogViewState::new(rows));
        log_wrapped.update(cx, |s, cx| s.set_wrap(true, cx));
        Self { page, chrome, inputs, log_compact, log_wrapped }
    }
}

impl Render for GalleryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        let content = pages::render(self, window, cx);
        if !self.chrome {
            return div().size_full().bg(p.bg).text_color(p.text).font_family(t.fonts.ui.clone()).text_size(px(13.)).child(content).into_any_element();
        }
        let view = cx.entity().downgrade();
        let nav = div()
            .w(px(190.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(2.))
            .p(px(10.))
            .bg(p.side)
            .border_r_1()
            .border_color(p.border)
            .child(div().px(px(6.)).pb(px(12.)).text_size(px(14.)).font_weight(FontWeight(600.)).text_color(p.muted).child(format!("rogg gallery {}", rogg_ui::version())))
            .children(Page::ALL.map(|page| {
                let view = view.clone();
                div().rounded(px(5.)).when(page == self.page, |d| d.bg(p.selection)).child(
                    TextButton::meta(SharedString::from(format!("nav-{}", page.slug())), page.title()).on_click(move |_, cx| {
                        view.update(cx, |this, cx| {
                            this.page = page;
                            cx.notify();
                        })
                        .ok();
                    }),
                )
            }));
        let appearance_ix = if t.appearance == Appearance::Dark { 0 } else { 1 };
        let density_ix = if t.density == Density::Compact { 0 } else { 1 };
        let toolbar = div()
            .h(px(44.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(16.))
            .bg(p.side)
            .border_b_1()
            .border_color(p.border)
            .child(div().text_size(px(15.)).font_weight(FontWeight(550.)).child(self.page.title()))
            .child(div().flex_1())
            .child(Segmented::new("gallery-appearance", vec!["Dark".into(), "Light".into()], appearance_ix).on_select(|i, _, cx| {
                let density = cx.rogg().density;
                RoggTheme::new(if i == 0 { Appearance::Dark } else { Appearance::Light }, density).install(cx);
            }))
            .child(Segmented::new("gallery-density", vec!["Compact".into(), "Comfortable".into()], density_ix).on_select(|i, _, cx| {
                let appearance = cx.rogg().appearance;
                RoggTheme::new(appearance, if i == 0 { Density::Compact } else { Density::Comfortable }).install(cx);
            }));
        div()
            .size_full()
            .flex()
            .bg(p.page)
            .text_color(p.text)
            .font_family(t.fonts.ui.clone())
            .text_size(px(13.))
            .child(nav)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(toolbar)
                    .child(div().id("page").flex_1().min_h_0().overflow_y_scroll().p(px(24.)).child(content)),
            )
            .into_any_element()
    }
}
```

- [ ] **Step 5: Implement `pages.rs`**

```rust
use crate::fixtures;
use crate::gallery::{GalleryView, Page};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Div, FontWeight, SharedString, Window, div, px};
use rogg_ui::chrome::{InfoBar, PaneHeader, Progress, StatPart, StatusBar};
use rogg_ui::controls::{ActionButton, CompactSelect, FindField, FlagButton, IconButton, Segmented, TextButton, ToolButton};
use rogg_ui::dialogs;
use rogg_ui::empty;
use rogg_ui::filter::{ChipState, FilterBand, FilterChip};
use rogg_ui::icons::IconName;
use rogg_ui::logview::LogView;
use rogg_ui::sidebar::Sidebar;
use rogg_ui::theme::{ActiveRoggTheme, Palette};
use rogg_ui::titlebar::{TabItem, TitleBar};
use std::rc::Rc;

pub fn render(view: &mut GalleryView, window: &mut Window, cx: &mut Context<GalleryView>) -> AnyElement {
    let p = cx.rogg().palette;
    let _ = window;
    match view.page {
        Page::Tokens => tokens(cx),
        Page::Controls => controls(view, &p),
        Page::Tabs => tabs(&p),
        Page::Sidebar => sidebar(&p),
        Page::Filter => filter(view, &p),
        Page::LogView => logview(view, &p),
        Page::Chrome => chrome(view, &p),
        Page::Empty => empty_states(&p),
        Page::Dialogs => dialogs_page(view, &p),
    }
}

fn section(title: &str, body: impl IntoElement, p: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .mb(px(28.))
        .child(div().text_size(px(10.)).font_weight(FontWeight(600.)).text_color(p.muted).child(title.to_uppercase()))
        .child(body)
}

fn row() -> Div {
    div().flex().flex_wrap().items_center().gap(px(12.))
}

fn frame(w: f32, h: f32, p: &Palette, child: impl IntoElement) -> Div {
    div().w(px(w)).h(px(h)).flex().flex_col().border_1().border_color(p.border).rounded(px(6.)).overflow_hidden().bg(p.bg).child(child)
}

fn tokens(cx: &mut Context<GalleryView>) -> AnyElement {
    let t = cx.rogg().clone();
    let p = t.palette;
    let swatches = row().children(p.named().into_iter().map(|(name, color)| {
        div()
            .w(px(150.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().h(px(36.)).rounded(px(6.)).border_1().border_color(p.border).bg(color))
            .child(div().text_size(px(11.)).child(name))
            .child(div().text_size(px(10.)).font_family(t.fonts.mono.clone()).text_color(p.faint).child(format!("#{:08x}", color.0)))
    }));
    let scale = div().flex().flex_col().gap(px(6.)).children(
        [(9., "9 · hints"), (10., "10 · tools, stats"), (11., "11 · meta, tabs"), (12., "12 · body, dialogs"), (13., "13 · base"), (14., "14 · brand"), (15., "15 · empty title"), (17., "17 · dialog title")]
            .map(|(size, label)| div().text_size(px(size)).child(label)),
    );
    let mono = div().flex().flex_col().gap(px(6.)).font_family(t.fonts.mono.clone()).children(
        [10., 11., 12., 13.].map(|size| div().text_size(px(size)).child(format!("{size}px  2026-10-05 14:32:12.144  ERROR  client  upstream timeout"))),
    );
    let m = t.metrics;
    let metrics = div().text_size(px(12.)).text_color(p.muted).child(format!(
        "density {:?}: row {:?}, log font {:?}, pane header {:?}",
        t.density, m.row_height, m.log_font_size, m.pane_header_height
    ));
    div()
        .child(section("Palette", swatches, &p))
        .child(section("UI type scale (SF Pro)", scale, &p))
        .child(section("Log type (SF Mono)", mono, &p))
        .child(section("Metrics", metrics, &p))
        .into_any_element()
}

fn controls(view: &GalleryView, p: &Palette) -> AnyElement {
    let icons = row().children(IconName::ALL.iter().enumerate().map(|(i, name)| IconButton::icon(SharedString::from(format!("icon-{i}")), *name)));
    let buttons = row()
        .child(IconButton::icon("ib-normal", IconName::Edit))
        .child(IconButton::icon("ib-hover", IconName::Edit).force_hover(true))
        .child(IconButton::glyph("ib-chevron", "⌄").compact())
        .child(IconButton::glyph("ib-cmd", "⌘").compact())
        .child(IconButton::icon("ib-settings", IconName::Settings).compact())
        .child(IconButton::glyph("ib-close", "✕"));
    let flags = row()
        .child(FlagButton::new("flag-regex-off", ".*"))
        .child(FlagButton::new("flag-regex-on", ".*").pressed(true))
        .child(FlagButton::new("flag-case-off", "Aa"))
        .child(FlagButton::new("flag-case-on", "Aa").pressed(true));
    let tools = row()
        .child(ToolButton::new("tool-wrap-off", IconName::Wrap, "Wrap"))
        .child(ToolButton::new("tool-wrap-on", IconName::Wrap, "Wrap").pressed(true))
        .child(ToolButton::new("tool-follow", IconName::Follow, "Follow selection").pressed(true))
        .child(ToolButton::new("tool-open-tab", IconName::OpenTab, "Open as tab"))
        .child(ToolButton::new("tool-pause", IconName::Pause, "Paused").pressed(true).badge(Some("+1,204".into())));
    let text_buttons = row()
        .child(TextButton::meta("tb-profile", "Profile: Services⌄").dot())
        .child(TextButton::meta("tb-export", "Export").icon(IconName::Export))
        .child(TextButton::small("tb-history", "History⌄").icon(IconName::History))
        .child(ActionButton::new("ab-primary", "This direction works").primary())
        .child(ActionButton::new("ab-normal", "Needs adjustment"));
    let fields = row()
        .child(FindField::new("find-empty", view.inputs.find_empty.clone()))
        .child(FindField::new("find-query", view.inputs.find_query.clone()).count(Some("3 hits".into())))
        .child(Segmented::new("segmented", vec!["Compact".into(), "Comfortable".into()], 0))
        .child(CompactSelect::new("select", vec!["None".into(), "±1 line".into(), "±2 lines".into(), "±3 lines".into()], 0));
    div()
        .child(section("All icons (15px)", icons, p))
        .child(section("Icon buttons: normal, hover, title bar", buttons, p))
        .child(section("Flags: off / on", flags, p))
        .child(section("Tool buttons", tools, p))
        .child(section("Text and action buttons", text_buttons, p))
        .child(section("Fields", fields, p))
        .into_any_element()
}

fn tab_items(labels: &[&str], active: usize) -> Vec<TabItem> {
    labels.iter().enumerate().map(|(i, l)| TabItem { label: (*l).into(), tooltip: fixtures::BUNDLE.into(), active: i == active }).collect()
}

fn tabs(p: &Palette) -> AnyElement {
    let many: Vec<String> = (0..12).map(|i| format!("service-{i}-with-long-name.log")).collect();
    let many: Vec<&str> = many.iter().map(|s| s.as_str()).collect();
    div()
        .child(section("Reference: 3 tabs, api.log active", frame(1200., 42., p, TitleBar::new(tab_items(&["api.log", "worker.log", "gateway.log"], 0)).traffic_lights(true)), p))
        .child(section("Hovered inactive tab", frame(1200., 42., p, TitleBar::new(tab_items(&["api.log", "worker.log", "gateway.log"], 0)).traffic_lights(true).force_hover_tab(Some(1))), p))
        .child(section("12 long tabs in 900px (scrolls)", frame(900., 42., p, TitleBar::new(tab_items(&many, 3)).traffic_lights(true)), p))
        .into_any_element()
}

fn sidebar(p: &Palette) -> AnyElement {
    section(
        "Bundle tree, recent, footer",
        frame(211., 640., p, Sidebar::new().count("10 files".into()).rows(fixtures::sidebar_rows("api.log")).recent(fixtures::recent()).footer(Some(fixtures::sidebar_footer()))),
        p,
    )
    .into_any_element()
}

fn filter(view: &GalleryView, p: &Palette) -> AnyElement {
    let states = [ChipState::Include, ChipState::Off, ChipState::Exclude];
    let chips = div().flex().flex_col().gap(px(8.)).children(states.map(|state| {
        row().children((0..4).map(move |slot| FilterChip::new(slot, ["error", "timeout", "healthcheck", "retry"][slot], state)))
    }));
    let terms = fixtures::reference_terms();
    let many: Vec<(SharedString, ChipState)> = (0..20).map(|i| (format!("term{i}").into(), states[i % 3])).collect();
    div()
        .child(section("Chips: slot × state", chips, p))
        .child(section("Reference band", frame(1000., 90., p, FilterBand::new(terms.clone()).summary("2 include · 1 exclude · 1 off".into())), p))
        .child(section(
            "Editing with error",
            frame(1000., 110., p, FilterBand::new(terms.clone()).editing(Some(view.inputs.expression.clone())).error(Some("unclosed group".into())).summary("2 include · 1 exclude · 1 off".into())),
            p,
        ))
        .child(section("Regex + case on, context ±2", frame(1000., 90., p, FilterBand::new(terms).regex(true).case_sensitive(true).context(vec!["None".into(), "±1 line".into(), "±2 lines".into(), "±3 lines".into()], 2)), p))
        .child(section("20 chips in 900px", frame(900., 90., p, FilterBand::new(many)), p))
        .into_any_element()
}

fn logview(view: &GalleryView, p: &Palette) -> AnyElement {
    div()
        .child(section("No wrap: hover (row 2), selected, pinned, context, gap, hits, search, error tint, long line", frame(1100., 300., p, LogView::new("log-compact", view.log_compact.clone())), p))
        .child(section("Wrap on (700px)", frame(700., 420., p, LogView::new("log-wrapped", view.log_wrapped.clone())), p))
        .into_any_element()
}

fn chrome(view: &GalleryView, p: &Palette) -> AnyElement {
    let stats = vec![
        StatPart::Strong("6".into()),
        StatPart::Text(" matches / 40 lines · ".into()),
        StatPart::Strong("1".into()),
        StatPart::Text(" pinned (1 added) · ".into()),
        StatPart::Strong("0".into()),
        StatPart::Text(" context".into()),
    ];
    div()
        .child(section("Info bar", frame(1100., 33., p, InfoBar::new(vec![fixtures::BUNDLE.into(), "api.log".into()], "Services".into())), p))
        .child(section(
            "Pane headers",
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(frame(1100., 40., p, PaneHeader::new("ph-full", "Full log", "40 sample lines").find(FindField::new("ph-find-top", view.inputs.find_empty.clone())).tool(ToolButton::new("ph-wrap-top", IconName::Wrap, "Wrap"))))
                .child(frame(
                    1100.,
                    40.,
                    p,
                    PaneHeader::new("ph-filtered", "Filtered log", "7 shown")
                        .find(FindField::new("ph-find-bottom", view.inputs.find_query.clone()).count(Some("3 hits".into())))
                        .tool(ToolButton::new("ph-open-tab", IconName::OpenTab, "Open as tab"))
                        .tool(ToolButton::new("ph-follow", IconName::Follow, "Follow selection").pressed(true))
                        .tool(ToolButton::new("ph-wrap-bottom", IconName::Wrap, "Wrap")),
                )),
            p,
        ))
        .child(section(
            "Status bar: idle, selection, progress",
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(frame(1100., 29., p, StatusBar::new("UTF-8 · LF · sample excerpt".into()).stats(stats.clone())))
                .child(frame(1100., 29., p, StatusBar::new("UTF-16LE → UTF-8 · CRLF".into()).stats(stats).selection(Some("Line 120492".into())).progress(Some(Progress { label: "Filtering 63%".into(), fraction: 0.63 })))),
            p,
        ))
        .into_any_element()
}

fn empty_states(p: &Palette) -> AnyElement {
    let noop: rogg_ui::Callback = Rc::new(|_, _| {});
    let noop_ix: rogg_ui::IndexCallback = Rc::new(|_, _, _| {});
    row()
        .child(frame(600., 300., p, empty::no_matches("empty-no-matches")))
        .child(frame(600., 300., p, empty::welcome(fixtures::recent(), noop.clone(), noop.clone(), noop_ix)))
        .child(frame(600., 300., p, empty::loading("Indexing api.log · 412 MB / 1.0 GB", Some(0.41), noop.clone())))
        .child(frame(600., 300., p, empty::error("Could not read archive member", "incident-2026-10-05.zip › gateway.log", noop)))
        .into_any_element()
}

fn dialogs_page(view: &GalleryView, p: &Palette) -> AnyElement {
    let noop_ix: rogg_ui::IndexCallback = Rc::new(|_, _, _| {});
    let tabs: Vec<(SharedString, bool)> = vec![("api.log".into(), true), ("worker.log".into(), false), ("gateway.log".into(), false)];
    row()
        .items_start()
        .child(dialogs::all_tabs(&tabs, noop_ix.clone()).description(fixtures::tabs_description()))
        .child(dialogs::profile("Services", &fixtures::profile_rules()).description(fixtures::PROFILE_DESCRIPTION).note(fixtures::PROFILE_NOTE))
        .child(dialogs::appearance(&fixtures::appearance_rows()).description(fixtures::APPEARANCE_DESCRIPTION).note(fixtures::APPEARANCE_NOTE))
        .child(dialogs::shortcuts(&fixtures::shortcut_rows()).description(fixtures::SHORTCUTS_DESCRIPTION).note(fixtures::SHORTCUTS_NOTE))
        .child(dialogs::history(&fixtures::history_entries()).description(fixtures::HISTORY_DESCRIPTION).note(fixtures::HISTORY_NOTE))
        .child(dialogs::statistics(&[("Matches".into(), "6 / 40 lines".into()), ("Pinned".into(), "1 (1 added)".into()), ("error".into(), "6 hits".into()), ("timeout".into(), "7 hits".into()), ("Scan time".into(), "0.4 ms".into())]))
        .child(dialogs::export(&[("Include context lines".into(), true), ("Include gap markers".into(), false), ("Prefix line numbers".into(), true)], noop_ix, Rc::new(|_, _| {})))
        .child(dialogs::go_to_line(view.inputs.goto.clone()))
        .child(div().w(px(500.)).text_size(px(11.)).text_color(p.faint).child("Every dialog renders here as a static specimen. The mockup replica shows them as modals over the window."))
        .into_any_element()
}
```

- [ ] **Step 6: Wire up `lib.rs` and `main.rs`**

`crates/rogg-gallery/src/lib.rs`:
```rust
//! rogg component gallery: every rogg-ui component in every state, plus a rebuild of the reference mockup.
pub mod fixtures;
pub mod gallery;
pub mod pages;
```

`crates/rogg-gallery/src/main.rs`:
```rust
use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, QuitMode, WindowBounds, WindowOptions, px, size};
use rogg_gallery::gallery::{GalleryView, Page};
use rogg_ui::theme::{Appearance, Density, RoggTheme};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let page = args
        .iter()
        .position(|a| a == "--page")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| Page::from_slug(s))
        .unwrap_or(Page::Tokens);

    gpui_kit::application()
        .with_assets(rogg_ui::assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            RoggTheme::new(Appearance::Dark, Density::Compact).install(cx);
            let bounds = Bounds::centered(None, size(px(1640.), px(1000.)), cx);
            gpui_kit::open_window(
                WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
                cx,
                move |window, cx| cx.new(|cx| GalleryView::new(page, true, window, cx)),
            )
            .expect("open gallery window");
            cx.activate(true);
            if std::env::var_os("ROGG_GALLERY_SMOKE").is_some() {
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(Duration::from_millis(1500)).await;
                    println!("smoke ok");
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
        });
}
```

- [ ] **Step 7: Run the tests and look at it**

Run: `cargo test -p rogg-gallery && just smoke`
Expected: the tests pass and `smoke ok` is printed.

Then run `cargo run -p rogg-gallery -- --page controls` and visit every page. Toggle Dark/Light and Compact/Comfortable. Confirm that:
- every page renders;
- icons are visible, not blank;
- theme switching recolors everything live.

Fix any crash before committing.

- [ ] **Step 8: Commit**

```bash
git add crates/rogg-gallery
git commit -m "feat(gallery): component gallery with tokens, controls, tabs, sidebar, filter, log view, chrome, empty and dialog pages

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 14: Mockup replica

**Files:**
- Create: `crates/rogg-gallery/src/replica/{mod.rs,data.rs,state.rs,view.rs}`
- Modify: `crates/rogg-gallery/src/{lib.rs,gallery.rs,pages.rs}`
- Test: unit tests in `data.rs`; add a replica test to `crates/rogg-gallery/tests/pages.rs`

**Interfaces:**
- Consumes: `fixtures` (Task 13), all `rogg_ui` components.
- Produces:
  - `replica::data::{SAMPLES, FIRST_LINE_NUMBER, STAMP_LEN, line_text(usize) -> String, Term { text, state }, reference_terms(), filter(&[Term], &BTreeSet<usize>, context, case) -> Filtered { matched, visible, anchors }, hit_spans(&str, &[Term], case) -> Vec<(Range<usize>, usize)>}`
  - `replica::ReplicaDialog::{AllTabs, Profile, Appearance, Shortcuts, History}` with `ReplicaDialog::ALL`, `.slug()` (`all-tabs`, `profile`, `appearance`, `shortcuts`, `history`), `from_slug`
  - `replica::ReplicaState` (an entity): `new(window, cx)`; public fields `top`, `bottom: Entity<LogViewState>`, `filtered`, `dialog`; methods `cycle(i)`, `toggle_pin(line)`, `set_context(n)`, `open_dialog(d)`, `close_dialog()`
  - `replica::replica_window(&Entity<ReplicaState>, cx) -> AnyElement`
  - `Page::Replica` (slug `replica`) and `GalleryView.replica: Entity<ReplicaState>`

The replica runs the reference's own JavaScript logic, ported so the expected values below can be checked. They were computed by running the reference's script in Node:
- `matched == [12, 15, 20, 23, 28, 34]`
- with context 0: `visible == [7, 12, 15, 20, 23, 28, 34]`, gaps before 12 (4), 15 (2), 20 (4), 23 (2), 28 (4), 34 (5)
- with context 1: `visible == [7,8,11,12,13,14,15,16,19,20,21,22,23,24,27,28,29,33,34,35]`
- with context 3: 30 visible rows starting `[4, 7, 8, …]`

The replica's regex toggle only changes the button state. Regex matching belongs to the M1 engine.

- [ ] **Step 1: Write the data module with failing tests**

`crates/rogg-gallery/src/replica/data.rs`:
```rust
//! Port of the reference mockup's sample data and filter logic (`samples`, `lineText`, `passes`, `spans`).
use rogg_ui::filter::ChipState;
use std::collections::BTreeSet;
use std::ops::Range;

pub const FIRST_LINE_NUMBER: u64 = 120480;
/// `text.slice(0, 23)`: the timestamp painted faint.
pub const STAMP_LEN: usize = 23;

pub const SAMPLES: [(&str, &str, &str); 40] = [
    ("INFO ", "server", "request started method=POST path=/v1/payments request_id=req_8f29"),
    ("DEBUG", "router", "matched route=payments.create upstream=payments-primary"),
    ("INFO ", "auth", "token validated subject=svc_checkout scopes=payments:write"),
    ("DEBUG", "pool", "acquired connection pool=primary active=24 idle=8"),
    ("WARN ", "client", "retry attempt=1 backoff=250ms target=payments-primary"),
    ("INFO ", "healthcheck", "upstream=cache status=ok latency=2ms"),
    ("ERROR", "healthcheck", "timeout waiting for metrics sidecar target=metrics:9090"),
    ("INFO ", "config", "deployment revision=8d72f6a feature_flags={payments_v2:true, strict_idempotency:true} changed_by=release-controller region=eu-west-1"),
    ("DEBUG", "client", "connection opened host=payments-primary.internal port=8443 tls=1.3"),
    ("WARN ", "client", "retry attempt=2 backoff=500ms target=payments-primary"),
    ("INFO ", "queue", "payment accepted job_id=job_4271 request_id=req_8f29"),
    ("DEBUG", "worker", "processing job_id=job_4271 queue=payments priority=normal"),
    ("ERROR", "client", "upstream timeout after 30000ms request_id=req_8f29 host=payments-primary.internal:8443 path=/v1/authorize retry_count=3 circuit_state=half_open trace_id=4bf92f3577b34da6a3ce929d0e0e4736 region=eu-west-1"),
    ("WARN ", "circuit", "circuit opened upstream=payments-primary failures=5 window=60s"),
    ("INFO ", "router", "switching upstream payments-primary → payments-secondary"),
    ("ERROR", "payments", "authorization failed error=UPSTREAM_UNAVAILABLE request_id=req_8f29"),
    ("INFO ", "server", "request completed status=503 duration=30241ms request_id=req_8f29"),
    ("DEBUG", "pool", "released connection pool=primary active=23 idle=9"),
    ("INFO ", "healthcheck", "upstream=database status=ok latency=4ms"),
    ("INFO ", "server", "request started method=GET path=/v1/accounts request_id=req_91d2"),
    ("WARN ", "cache", "timeout reading key=account:7239 fallback=database"),
    ("INFO ", "database", "query completed rows=1 duration=14ms"),
    ("INFO ", "server", "request completed status=200 duration=48ms request_id=req_91d2"),
    ("ERROR", "worker", "job failed error=DEADLINE_EXCEEDED job_id=job_4271 attempt=3"),
    ("INFO ", "queue", "job scheduled for retry job_id=job_4271 delay=60s"),
    ("DEBUG", "metrics", "flush completed counters=148 histograms=23 duration=2ms"),
    ("WARN ", "healthcheck", "timeout target=payments-primary latency=5000ms"),
    ("INFO ", "circuit", "upstream probe scheduled after=30s"),
    ("ERROR", "client", "connection reset by peer upstream=payments-secondary request_id=req_a273"),
    ("INFO ", "server", "request completed status=502 duration=1304ms request_id=req_a273"),
    ("DEBUG", "router", "routing table refreshed revision=248 endpoints=12"),
    ("INFO ", "server", "request started method=GET path=/status request_id=req_b7c1"),
    ("INFO ", "server", "request completed status=200 duration=1ms request_id=req_b7c1"),
    ("WARN ", "pool", "connection pool utilization=92% active=46 limit=50"),
    ("ERROR", "database", "timeout acquiring connection after 5000ms pool=primary request_id=req_c389"),
    ("INFO ", "circuit", "upstream recovered target=payments-primary probe_status=200"),
    ("INFO ", "queue", "job resumed job_id=job_4271 attempt=4"),
    ("INFO ", "payments", "authorization completed job_id=job_4271 status=approved"),
    ("DEBUG", "metrics", "flush completed counters=153 histograms=24 duration=1ms"),
    ("INFO ", "server", "idle connections pruned count=8 remaining=16"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub text: String,
    pub state: ChipState,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filtered {
    pub matched: Vec<usize>,
    pub visible: Vec<usize>,
    pub anchors: BTreeSet<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_text_matches_reference_formula() {
        assert_eq!(line_text(0), "2026-10-05 14:32:08.142  INFO   server       request started method=POST path=/v1/payments request_id=req_8f29");
        assert_eq!(
            line_text(7),
            "2026-10-05 14:32:10.639  INFO   config       deployment revision=8d72f6a feature_flags={payments_v2:true, strict_idempotency:true} changed_by=release-controller region=eu-west-1"
        );
        assert!(line_text(12).starts_with("2026-10-05 14:32:12.144  ERROR  client       upstream timeout after 30000ms"));
        assert_eq!(line_text(39), "2026-10-05 14:32:21.361  INFO   server       idle connections pruned count=8 remaining=16");
    }

    #[test]
    fn reference_filter_results() {
        let pins = BTreeSet::from([7]);
        let f = filter(&reference_terms(), &pins, 0, false);
        assert_eq!(f.matched, vec![12, 15, 20, 23, 28, 34]);
        assert_eq!(f.visible, vec![7, 12, 15, 20, 23, 28, 34]);
        let f1 = filter(&reference_terms(), &pins, 1, false);
        assert_eq!(f1.visible, vec![7, 8, 11, 12, 13, 14, 15, 16, 19, 20, 21, 22, 23, 24, 27, 28, 29, 33, 34, 35]);
        let f3 = filter(&reference_terms(), &pins, 3, false);
        assert_eq!(f3.visible.len(), 30);
        assert_eq!(&f3.visible[..3], &[4, 7, 8]);
    }

    #[test]
    fn only_excludes_shows_everything_else_and_no_active_terms_shows_nothing() {
        let only_not = vec![Term { text: "healthcheck".into(), state: ChipState::Exclude }];
        let f = filter(&only_not, &BTreeSet::new(), 0, false);
        assert_eq!(f.matched.len(), 40 - 4); // healthcheck appears on lines 5, 6, 18, 26
        let all_off = vec![Term { text: "error".into(), state: ChipState::Off }];
        assert!(filter(&all_off, &BTreeSet::new(), 0, false).matched.is_empty());
    }

    #[test]
    fn hit_spans_earlier_term_wins_and_slots_follow_term_index() {
        let rest = &line_text(12)[STAMP_LEN..];
        assert_eq!(hit_spans(rest, &reference_terms(), false), vec![(2..7, 0), (31..38, 1)]);
        let overlapping = vec![Term { text: "timeout".into(), state: ChipState::Include }, Term { text: "out".into(), state: ChipState::Include }];
        assert_eq!(hit_spans("a timeout", &overlapping, false), vec![(2..9, 0)]);
        assert!(hit_spans("ERROR", &reference_terms(), true).is_empty());
    }
}
```

`crates/rogg-gallery/src/replica/mod.rs` (Steps 4–5 extend it):
```rust
pub mod data;
```

Add `pub mod replica;` to `crates/rogg-gallery/src/lib.rs`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-gallery replica::data`
Expected: compile errors, `line_text`, `filter`, `reference_terms` and `hit_spans` are not found.

- [ ] **Step 3: Implement the data functions**

Add to `data.rs`, above the tests:
```rust
pub fn line_text(i: usize) -> String {
    let (level, source, message) = SAMPLES[i];
    format!("2026-10-05 14:32:{:02}.{:03}  {}  {:<12} {}", 8 + i / 3, 142 + (i * 71) % 850, level, source, message)
}

pub fn reference_terms() -> Vec<Term> {
    [("error", ChipState::Include), ("timeout", ChipState::Include), ("healthcheck", ChipState::Exclude), ("retry", ChipState::Off)]
        .map(|(text, state)| Term { text: text.into(), state })
        .into()
}

fn contains(haystack: &str, needle: &str, case_sensitive: bool) -> bool {
    if case_sensitive { haystack.contains(needle) } else { haystack.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()) }
}

fn excluded(i: usize, terms: &[Term], case: bool) -> bool {
    let text = line_text(i);
    terms.iter().any(|t| t.state == ChipState::Exclude && contains(&text, &t.text, case))
}

fn passes(i: usize, terms: &[Term], case: bool) -> bool {
    let text = line_text(i);
    let includes: Vec<_> = terms.iter().filter(|t| t.state == ChipState::Include).collect();
    let has_excludes = terms.iter().any(|t| t.state == ChipState::Exclude);
    let included = if includes.is_empty() { has_excludes } else { includes.iter().any(|t| contains(&text, &t.text, case)) };
    included && !excluded(i, terms, case)
}

/// Mirrors the reference `renderBottom`: matches ∪ pins, plus ±context lines that are not excluded (or are pinned).
pub fn filter(terms: &[Term], pins: &BTreeSet<usize>, context: usize, case: bool) -> Filtered {
    let matched: Vec<usize> = (0..SAMPLES.len()).filter(|&i| passes(i, terms, case)).collect();
    let anchors: BTreeSet<usize> = matched.iter().copied().chain(pins.iter().copied()).collect();
    let mut shown = anchors.clone();
    for &i in &anchors {
        let lo = i.saturating_sub(context);
        let hi = (i + context).min(SAMPLES.len() - 1);
        for j in lo..=hi {
            if !excluded(j, terms, case) || pins.contains(&j) {
                shown.insert(j);
            }
        }
    }
    Filtered { matched, visible: shown.into_iter().collect(), anchors }
}

/// Include-term highlight ranges over `rest` (text after the timestamp). Earlier terms own overlapping bytes;
/// the slot is the term's index in the full list, as in the reference (`hit-(index % 4)`).
pub fn hit_spans(rest: &str, terms: &[Term], case: bool) -> Vec<(Range<usize>, usize)> {
    let hay = if case { rest.to_string() } else { rest.to_ascii_lowercase() };
    let mut owner: Vec<Option<usize>> = vec![None; rest.len()];
    for (index, term) in terms.iter().enumerate() {
        if term.state != ChipState::Include || term.text.is_empty() {
            continue;
        }
        let needle = if case { term.text.clone() } else { term.text.to_ascii_lowercase() };
        let mut from = 0;
        while let Some(at) = hay[from..].find(&needle) {
            let start = from + at;
            for slot in &mut owner[start..start + needle.len()] {
                slot.get_or_insert(index);
            }
            from = start + needle.len();
        }
    }
    let mut out: Vec<(Range<usize>, usize)> = Vec::new();
    for (i, o) in owner.iter().enumerate() {
        let Some(term) = *o else { continue };
        match out.last_mut() {
            Some((r, t)) if r.end == i && *t == term % 4 => r.end = i + 1,
            _ => out.push((i..i + 1, term % 4)),
        }
    }
    out
}
```

Run: `cargo test -p rogg-gallery replica::data`
Expected: 4 tests pass.

- [ ] **Step 4: Implement `state.rs`**

```rust
use super::data::{self, FIRST_LINE_NUMBER, Filtered, SAMPLES, STAMP_LEN, Term};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, SharedString, Subscription, Window};
use rogg_ui::chrome::StatPart;
use rogg_ui::controls::FindField;
use rogg_ui::filter::ChipState;
use rogg_ui::logview::{LogLine, LogRow, LogViewEvent, LogViewState, RowTint, Span, SpanStyle, VecRows};
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ReplicaDialog {
    AllTabs,
    Profile,
    Appearance,
    Shortcuts,
    History,
}

impl ReplicaDialog {
    pub const ALL: [ReplicaDialog; 5] = [ReplicaDialog::AllTabs, ReplicaDialog::Profile, ReplicaDialog::Appearance, ReplicaDialog::Shortcuts, ReplicaDialog::History];

    pub fn slug(self) -> &'static str {
        match self {
            ReplicaDialog::AllTabs => "all-tabs",
            ReplicaDialog::Profile => "profile",
            ReplicaDialog::Appearance => "appearance",
            ReplicaDialog::Shortcuts => "shortcuts",
            ReplicaDialog::History => "history",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.slug() == slug)
    }
}

/// The interactive mockup: same data, defaults and behavior as the reference HTML.
pub struct ReplicaState {
    pub terms: Vec<Term>,
    pub regex: bool,
    pub case_sensitive: bool,
    pub context: usize,
    pub pins: BTreeSet<usize>,
    pub selected: Option<usize>,
    /// Shown in the footer only after the user clicks a row (the reference starts with an empty status).
    pub status_selection: Option<usize>,
    pub follow: bool,
    pub sidebar_visible: bool,
    pub tabs: Vec<&'static str>,
    pub active_tab: usize,
    pub dialog: Option<ReplicaDialog>,
    pub editing: Option<Entity<InputState>>,
    pub error: Option<SharedString>,
    pub top: Entity<LogViewState>,
    pub bottom: Entity<LogViewState>,
    pub find_top: Entity<InputState>,
    pub find_bottom: Entity<InputState>,
    pub filtered: Filtered,
    _subscriptions: Vec<Subscription>,
}

impl ReplicaState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let empty = || Rc::new(VecRows::new(vec![]));
        let top = cx.new(|_| LogViewState::new(empty()));
        let bottom = cx.new(|_| LogViewState::new(empty()));
        let find_top = FindField::state("Find in full log", window, cx);
        let find_bottom = FindField::state("Find in results", window, cx);
        let subscriptions = vec![
            cx.subscribe(&top, |this, _, ev: &LogViewEvent, cx| this.on_log_event(ev, false, cx)),
            cx.subscribe(&bottom, |this, _, ev: &LogViewEvent, cx| this.on_log_event(ev, true, cx)),
            cx.subscribe_in(&find_top, window, |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.rebuild(cx)
                }
            }),
            cx.subscribe_in(&find_bottom, window, |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.rebuild(cx)
                }
            }),
        ];
        let mut this = Self {
            terms: data::reference_terms(),
            regex: false,
            case_sensitive: false,
            context: 0,
            pins: BTreeSet::from([7]),
            selected: Some(12),
            status_selection: None,
            follow: true,
            sidebar_visible: true,
            tabs: vec!["api.log", "worker.log", "gateway.log"],
            active_tab: 0,
            dialog: None,
            editing: None,
            error: None,
            top,
            bottom,
            find_top,
            find_bottom,
            filtered: Filtered::default(),
            _subscriptions: subscriptions,
        };
        this.rebuild(cx);
        this
    }

    fn query(input: &Entity<InputState>, cx: &App) -> String {
        input.read(cx).value().to_string().to_ascii_lowercase()
    }

    fn line(&self, i: usize, bottom: bool, context: bool, query: &str) -> LogLine {
        let text = data::line_text(i);
        let mut spans = vec![Span { range: 0..STAMP_LEN, style: SpanStyle::Faint }];
        if bottom {
            for (r, slot) in data::hit_spans(&text[STAMP_LEN..], &self.terms, self.case_sensitive) {
                spans.push(Span { range: r.start + STAMP_LEN..r.end + STAMP_LEN, style: SpanStyle::Hit(slot) });
            }
        }
        if !query.is_empty() {
            let lower = text.to_ascii_lowercase();
            let mut from = 0;
            while let Some(at) = lower[from..].find(query) {
                spans.push(Span { range: from + at..from + at + query.len(), style: SpanStyle::SearchHit });
                from += at + query.len();
            }
        }
        LogLine {
            number: FIRST_LINE_NUMBER + i as u64,
            text: text.into(),
            spans,
            pinned: self.pins.contains(&i),
            context,
            tint: (SAMPLES[i].0 == "ERROR").then_some(RowTint::Error),
        }
    }

    pub fn rebuild(&mut self, cx: &mut Context<Self>) {
        self.filtered = data::filter(&self.terms, &self.pins, self.context, self.case_sensitive);
        let q_top = Self::query(&self.find_top, cx);
        let q_bottom = Self::query(&self.find_bottom, cx);
        let top_rows: Vec<LogRow> = (0..SAMPLES.len()).map(|i| LogRow::Line(self.line(i, false, false, &q_top))).collect();
        let mut bottom_rows = Vec::new();
        let mut prev: Option<usize> = None;
        for &i in &self.filtered.visible {
            if let Some(prev) = prev {
                if i > prev + 1 {
                    bottom_rows.push(LogRow::Gap { omitted: (i - prev - 1) as u64 });
                }
            }
            bottom_rows.push(LogRow::Line(self.line(i, true, !self.filtered.anchors.contains(&i), &q_bottom)));
            prev = Some(i);
        }
        let selected = self.selected.map(|i| FIRST_LINE_NUMBER + i as u64);
        self.top.update(cx, |s, cx| {
            s.set_rows(Rc::new(VecRows::new(top_rows)), cx);
            s.set_selected(selected, cx);
        });
        self.bottom.update(cx, |s, cx| {
            s.set_rows(Rc::new(VecRows::new(bottom_rows)), cx);
            s.set_selected(selected, cx);
        });
        cx.notify();
    }

    fn on_log_event(&mut self, ev: &LogViewEvent, from_bottom: bool, cx: &mut Context<Self>) {
        match ev {
            LogViewEvent::Selected { number, .. } => {
                let i = (*number - FIRST_LINE_NUMBER) as usize;
                self.selected = Some(i);
                self.status_selection = Some(i);
                if from_bottom && self.follow {
                    if let Some(row) = self.top.read(cx).rows().row_for_number(*number) {
                        self.top.update(cx, |s, cx| s.center_on_row(row, cx));
                    }
                }
                self.rebuild(cx);
            }
            LogViewEvent::TogglePin { number } => self.toggle_pin((*number - FIRST_LINE_NUMBER) as usize, cx),
        }
    }

    pub fn toggle_pin(&mut self, line: usize, cx: &mut Context<Self>) {
        if !self.pins.remove(&line) {
            self.pins.insert(line);
        }
        self.rebuild(cx);
    }

    pub fn cycle(&mut self, i: usize, cx: &mut Context<Self>) {
        if let Some(t) = self.terms.get_mut(i) {
            t.state = t.state.next();
        }
        self.rebuild(cx);
    }

    pub fn set_context(&mut self, n: usize, cx: &mut Context<Self>) {
        self.context = n;
        self.rebuild(cx);
    }

    pub fn toggle_regex(&mut self, cx: &mut Context<Self>) {
        self.regex = !self.regex;
        cx.notify();
    }

    pub fn toggle_case(&mut self, cx: &mut Context<Self>) {
        self.case_sensitive = !self.case_sensitive;
        self.rebuild(cx);
    }

    pub fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        self.follow = !self.follow;
        cx.notify();
    }

    pub fn toggle_wrap(&mut self, bottom: bool, cx: &mut Context<Self>) {
        let state = if bottom { self.bottom.clone() } else { self.top.clone() };
        state.update(cx, |s, cx| {
            let wrap = !s.wrap();
            s.set_wrap(wrap, cx)
        });
        cx.notify();
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    pub fn select_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        self.active_tab = i.min(self.tabs.len().saturating_sub(1));
        cx.notify();
    }

    pub fn close_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 && i < self.tabs.len() {
            self.tabs.remove(i);
            self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        }
        cx.notify();
    }

    pub fn open_dialog(&mut self, d: ReplicaDialog, cx: &mut Context<Self>) {
        self.dialog = Some(d);
        cx.notify();
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }

    pub fn start_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.terms.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" || ");
        self.editing = Some(cx.new(|cx| InputState::new(window, cx).default_value(text)));
        cx.notify();
    }

    /// Splits on `||`, keeps the state of terms whose text is unchanged, new terms start as Include.
    pub fn apply_edit(&mut self, cx: &mut Context<Self>) {
        if let Some(input) = self.editing.take() {
            let value = input.read(cx).value().to_string();
            self.terms = value
                .split("||")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|text| Term { text: text.to_string(), state: self.terms.iter().find(|t| t.text == text).map(|t| t.state).unwrap_or(ChipState::Include) })
                .collect();
            self.error = None;
        }
        self.rebuild(cx);
    }

    pub fn summary(&self) -> String {
        let count = |s: ChipState| self.terms.iter().filter(|t| t.state == s).count();
        format!("{} include · {} exclude · {} off", count(ChipState::Include), count(ChipState::Exclude), count(ChipState::Off))
    }

    pub fn stats(&self) -> Vec<StatPart> {
        let added = self.pins.iter().filter(|p| !self.filtered.matched.contains(p)).count();
        let context = self.filtered.visible.iter().filter(|i| !self.filtered.anchors.contains(i)).count();
        let mut parts = vec![
            StatPart::Strong(self.filtered.matched.len().to_string().into()),
            StatPart::Text(format!(" matches / {} lines · ", SAMPLES.len()).into()),
            StatPart::Strong(self.pins.len().to_string().into()),
            StatPart::Text(" pinned".into()),
        ];
        if added > 0 {
            parts.push(StatPart::Text(format!(" ({added} added)").into()));
        }
        parts.push(StatPart::Text(" · ".into()));
        parts.push(StatPart::Strong(context.to_string().into()));
        parts.push(StatPart::Text(" context".into()));
        parts
    }

    /// `N hits` for a panel's find field (rows containing the query), or None when empty.
    pub fn hit_count(&self, bottom: bool, cx: &App) -> Option<SharedString> {
        let q = Self::query(if bottom { &self.find_bottom } else { &self.find_top }, cx);
        if q.is_empty() {
            return None;
        }
        let rows: Vec<usize> = if bottom { self.filtered.visible.clone() } else { (0..SAMPLES.len()).collect() };
        let n = rows.iter().filter(|&&i| data::line_text(i).to_ascii_lowercase().contains(&q)).count();
        Some(format!("{n} hits").into())
    }
}
```

Extend `crates/rogg-gallery/src/replica/mod.rs`:
```rust
pub mod data;
mod state;
mod view;

pub use state::{ReplicaDialog, ReplicaState};
pub use view::replica_window;
```

- [ ] **Step 5: Implement `view.rs`**

```rust
use super::state::{ReplicaDialog, ReplicaState};
use crate::fixtures;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Entity, SharedString};
use rogg_ui::chrome::{InfoBar, PaneHeader, StatusBar};
use rogg_ui::controls::{FindField, ToolButton};
use rogg_ui::dialogs::{self, ModalLayer};
use rogg_ui::empty;
use rogg_ui::filter::FilterBand;
use rogg_ui::icons::IconName;
use rogg_ui::layout::{MainWindowLayout, MainWindowParts};
use rogg_ui::logview::LogView;
use rogg_ui::sidebar::Sidebar;
use rogg_ui::titlebar::{TabItem, TitleBar};
use std::rc::Rc;

fn act(state: &Entity<ReplicaState>, f: impl Fn(&mut ReplicaState, &mut gpui_kit::Context<ReplicaState>) + 'static) -> impl Fn(&mut gpui_kit::Window, &mut App) + 'static {
    let state = state.clone();
    move |_, cx| state.update(cx, |s, cx| f(s, cx))
}

fn act_ix(state: &Entity<ReplicaState>, f: impl Fn(&mut ReplicaState, usize, &mut gpui_kit::Context<ReplicaState>) + 'static) -> impl Fn(usize, &mut gpui_kit::Window, &mut App) + 'static {
    let state = state.clone();
    move |i, _, cx| state.update(cx, |s, cx| f(s, i, cx))
}

fn dialog(d: ReplicaDialog, s: &ReplicaState, state: &Entity<ReplicaState>) -> AnyElement {
    let close = act(state, |s, cx| s.close_dialog(cx));
    match d {
        ReplicaDialog::AllTabs => {
            let tabs: Vec<(SharedString, bool)> = s.tabs.iter().enumerate().map(|(i, t)| ((*t).into(), i == s.active_tab)).collect();
            let pick = act_ix(state, |s, i, cx| {
                s.select_tab(i, cx);
                s.close_dialog(cx);
            });
            dialogs::all_tabs(&tabs, Rc::new(pick)).description(fixtures::tabs_description()).on_close(close).into_any_element()
        }
        ReplicaDialog::Profile => dialogs::profile("Services", &fixtures::profile_rules())
            .description(fixtures::PROFILE_DESCRIPTION)
            .note(fixtures::PROFILE_NOTE)
            .on_close(close)
            .into_any_element(),
        ReplicaDialog::Appearance => dialogs::appearance(&fixtures::appearance_rows())
            .description(fixtures::APPEARANCE_DESCRIPTION)
            .note(fixtures::APPEARANCE_NOTE)
            .on_close(close)
            .into_any_element(),
        ReplicaDialog::Shortcuts => dialogs::shortcuts(&fixtures::shortcut_rows())
            .description(fixtures::SHORTCUTS_DESCRIPTION)
            .note(fixtures::SHORTCUTS_NOTE)
            .on_close(close)
            .into_any_element(),
        ReplicaDialog::History => dialogs::history(&fixtures::history_entries())
            .description(fixtures::HISTORY_DESCRIPTION)
            .note(fixtures::HISTORY_NOTE)
            .on_close(close)
            .into_any_element(),
    }
}

/// The reference window rebuilt from rogg-ui components and ReplicaState.
pub fn replica_window(state: &Entity<ReplicaState>, cx: &mut App) -> AnyElement {
    let s = state.read(cx);
    let tabs: Vec<TabItem> = s
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| TabItem { label: (*t).into(), tooltip: format!("{} · {}", t, fixtures::BUNDLE).into(), active: i == s.active_tab })
        .collect();
    let active = s.tabs[s.active_tab];
    let titlebar = TitleBar::new(tabs)
        .traffic_lights(true)
        .on_select_tab(act_ix(state, |s, i, cx| s.select_tab(i, cx)))
        .on_close_tab(act_ix(state, |s, i, cx| s.close_tab(i, cx)))
        .on_toggle_sidebar(act(state, |s, cx| s.toggle_sidebar(cx)))
        .on_all_tabs(act(state, |s, cx| s.open_dialog(ReplicaDialog::AllTabs, cx)))
        .on_shortcuts(act(state, |s, cx| s.open_dialog(ReplicaDialog::Shortcuts, cx)))
        .on_settings(act(state, |s, cx| s.open_dialog(ReplicaDialog::Appearance, cx)));
    let sidebar = s.sidebar_visible.then(|| {
        Sidebar::new()
            .count("10 files".into())
            .rows(fixtures::sidebar_rows(active))
            .recent(fixtures::recent())
            .footer(Some(fixtures::sidebar_footer()))
            .into_any_element()
    });
    let info_bar = InfoBar::new(vec![fixtures::BUNDLE.into(), active.into()], "Services".into())
        .on_profile(act(state, |s, cx| s.open_dialog(ReplicaDialog::Profile, cx)));
    let top_wrap = s.top.read(cx).wrap();
    let bottom_wrap = s.bottom.read(cx).wrap();
    let top_header = PaneHeader::new("pane-top-head", "Full log", "40 sample lines")
        .find(FindField::new("find-top", s.find_top.clone()).count(s.hit_count(false, cx)))
        .tool(ToolButton::new("tool-wrap-top", IconName::Wrap, "Wrap").pressed(top_wrap).on_click(act(state, |s, cx| s.toggle_wrap(false, cx))));
    let bottom_header = PaneHeader::new("pane-bottom-head", "Filtered log", format!("{} shown", s.filtered.visible.len()))
        .find(FindField::new("find-bottom", s.find_bottom.clone()).count(s.hit_count(true, cx)))
        .tool(ToolButton::new("tool-open-tab", IconName::OpenTab, "Open as tab"))
        .tool(ToolButton::new("tool-follow", IconName::Follow, "Follow selection").pressed(s.follow).on_click(act(state, |s, cx| s.toggle_follow(cx))))
        .tool(ToolButton::new("tool-wrap-bottom", IconName::Wrap, "Wrap").pressed(bottom_wrap).on_click(act(state, |s, cx| s.toggle_wrap(true, cx))));
    let band = FilterBand::new(s.terms.iter().map(|t| (SharedString::from(t.text.clone()), t.state)).collect())
        .regex(s.regex)
        .case_sensitive(s.case_sensitive)
        .editing(s.editing.clone())
        .error(s.error.clone())
        .context(vec!["None".into(), "±1 line".into(), "±2 lines".into(), "±3 lines".into()], s.context)
        .summary(s.summary().into())
        .on_cycle(act_ix(state, |s, i, cx| s.cycle(i, cx)))
        .on_edit({
            let state = state.clone();
            move |window, cx| state.update(cx, |s, cx| s.start_edit(window, cx))
        })
        .on_apply(act(state, |s, cx| s.apply_edit(cx)))
        .on_toggle_regex(act(state, |s, cx| s.toggle_regex(cx)))
        .on_toggle_case(act(state, |s, cx| s.toggle_case(cx)))
        .on_history(act(state, |s, cx| s.open_dialog(ReplicaDialog::History, cx)))
        .on_context(act_ix(state, |s, i, cx| s.set_context(i, cx)));
    let bottom_body = if s.filtered.visible.is_empty() {
        empty::no_matches("empty-results").into_any_element()
    } else {
        LogView::new("log-bottom", s.bottom.clone()).into_any_element()
    };
    let status = StatusBar::new("UTF-8 · LF · sample excerpt".into())
        .stats(s.stats())
        .selection(s.status_selection.map(|i| format!("Line {}", super::data::FIRST_LINE_NUMBER + i as u64).into()));
    let overlay = s.dialog.map(|d| ModalLayer::new(dialog(d, s, state)).on_dismiss(act(state, |s, cx| s.close_dialog(cx))).into_any_element());
    MainWindowLayout::new(MainWindowParts {
        titlebar: titlebar.into_any_element(),
        sidebar,
        info_bar: info_bar.into_any_element(),
        top_header: top_header.into_any_element(),
        top_body: LogView::new("log-top", s.top.clone()).into_any_element(),
        band: band.into_any_element(),
        bottom_header: bottom_header.into_any_element(),
        bottom_body,
        status: status.into_any_element(),
        overlay,
        split: MainWindowParts::DEFAULT_SPLIT,
    })
    .into_any_element()
}
```

The bottom header count is `visible.len()` followed by `" shown"`, matching the reference's `visibleIndices.length + ' shown'`.

- [ ] **Step 6: Add the replica page to the gallery**

In `gallery.rs`:
- Add `Replica` to `Page` and to `Page::ALL` (10 entries).
- Add slug `"replica"` and title `"Mockup replica"`.
- Add the field `pub replica: Entity<crate::replica::ReplicaState>` and create it in `new` with `cx.new(|cx| crate::replica::ReplicaState::new(window, cx))`.

In `pages.rs`, add the match arm:
```rust
        Page::Replica => {
            let window_el = crate::replica::replica_window(&view.replica.clone(), cx);
            if view.chrome {
                div()
                    .w(px(1440.))
                    .h(px(900.))
                    .flex()
                    .border_1()
                    .border_color(p.border)
                    .rounded(px(10.))
                    .overflow_hidden()
                    .child(window_el)
                    .into_any_element()
            } else {
                window_el
            }
        }
```

Add to `crates/rogg-gallery/tests/pages.rs`:
```rust
#[gpui_kit::test]
fn replica_starts_in_reference_state(cx: &mut TestAppContext) {
    use rogg_gallery::gallery::GalleryView;
    cx.update(|cx| {
        gpui_kit::init(cx);
        RoggTheme::new(Appearance::Dark, Density::Compact).install(cx);
    });
    let (handle, view) = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(1440.), px(900.)) })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| GalleryView::new(Page::Replica, false, window, cx))).unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
    })
    .unwrap();
    let replica = view.read_with(cx, |v, _| v.replica.clone());
    let (visible, bottom_rows, summary) = replica.read_with(cx, |r, cx| (r.filtered.visible.clone(), r.bottom.read(cx).rows().row_count(), r.summary()));
    assert_eq!(visible, vec![7, 12, 15, 20, 23, 28, 34]);
    assert_eq!(bottom_rows, 7 + 6, "7 lines + 6 gap rows");
    assert_eq!(summary, "2 include · 1 exclude · 1 off");
    replica.update(cx, |r, cx| r.cycle(2, cx)); // healthcheck NOT -> IN
    let matched = replica.read_with(cx, |r, _| r.filtered.matched.len());
    assert!(matched > 6);
}
```

- [ ] **Step 7: Run the tests and look at it**

Run: `cargo test -p rogg-gallery`
Expected: every test passes.

Then run `cargo run -p rogg-gallery -- --page replica` and place it next to the HTML (`open docs/superpowers/specs/assets/rogg-main-window.html`). Exercise everything:
- chip states cycle;
- pin and unpin from both panels;
- clicking a filtered row centers the full log on it (follow);
- the Context select works;
- the Wrap toggles work;
- every dialog opens and closes (⌄, ⌘, settings, Profile, History);
- the theme and density switches work.

- [ ] **Step 8: Commit**

```bash
git add crates/rogg-gallery
git commit -m "feat(gallery): interactive rebuild of the reference mockup

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 15: Headless capture mode

**Files:**
- Create: `crates/rogg-gallery/src/capture.rs`
- Modify: `crates/rogg-gallery/src/{lib.rs,main.rs,gallery.rs}`, `justfile`
- Test: `crates/rogg-gallery/tests/capture.rs`

**Interfaces:**
- Consumes: `GalleryView`, `ReplicaState`, `ReplicaDialog` (Tasks 13–14).
- Produces:
  - CLI: `rogg-gallery --capture <page-slug> [--dialog <all-tabs|profile|appearance|shortcuts|history>] [--theme dark|light] [--density compact|comfortable] [--size WxH] --out <png> [--geometry <json>]`
  - `capture::{CaptureOptions, CaptureOptions::parse(&[String]) -> Result<CaptureOptions, String>, run(CaptureOptions) -> anyhow::Result<()>}`
  - `GalleryView::for_capture(page, dialog: Option<ReplicaDialog>, window, cx)`
  - Geometry JSON: `{ "scale": 2, "window": {"x","y","w","h"}, "elements": { "<key>": {"x","y","w","h"} } }` in logical px.

**Geometry keys.** Each key is the element's `.id(..)` (via `.test_support()`) or a painted log row. `tools/visual-diff/config.mjs` maps each key to the reference selector.

| Key | Gallery source | Reference selector |
|---|---|---|
| `titlebar`, `titlebar-leading`, `title-actions` | TitleBar ids | `.titlebar`, `.titlebar-leading`, `.title-actions` |
| `tab-shell-N` | TabShell | `.tab-shell` (nth) |
| `sidebar`, `side-top`, `side-bottom` | Sidebar ids | `.sidebar`, `.side-top`, `.side-bottom` |
| `tree-row-0`, `tree-row-N` (N ≥ 1) | Sidebar rows | `.side-body .folder`, `#file-list .file` (nth, offset 1) |
| `recent-N` | Sidebar recent | `.recent` (nth) |
| `meta`, `profile`, `export` | InfoBar | `.meta`, `.meta .right button:first-child`, `.meta .right button:last-child` |
| `pane-top-head`, `find-top`, `tool-wrap-top`, `log-top` | top pane | `.pane.top .pane-head`, `.pane.top .find`, `#wrap-top`, `#log-top` |
| `filter-band`, `filter-actions`, `edit-filters`, `flag-regex`, `flag-case`, `filter-bottom`, `history` | FilterBand | `.filter-band`, `.filter-actions`, `#edit-button`, `#regex`, `#case`, `.filter-bottom`, `.filter-bottom > button` |
| `token-N`, `token-state-N` | FilterChip | `.token` (nth), `.token .state` (nth) |
| `pane-bottom-head`, `find-bottom`, `tool-follow`, `tool-wrap-bottom`, `log-bottom` | bottom pane | `.pane.bottom .pane-head`, `.pane.bottom .find`, `#follow`, `#wrap-bottom`, `#log-bottom` |
| `row-top-N`, `row-bottom-N`, `gap-bottom-N` | `LogViewState::last_rows` | `#log-top .row`, `#log-bottom .row`, `#log-bottom .gap-row` (nth) |
| `statusbar` | StatusBar | `.footer` |
| `dialog` | DialogFrame | `dialog[open]` |

- [ ] **Step 1: Write the failing capture test**

`crates/rogg-gallery/tests/capture.rs`:
```rust
//! Runs the real binary, because headless Metal capture must own the main thread.
use std::process::Command;

fn png_size(path: &std::path::Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[1..4], b"PNG");
    let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    (w, h)
}

fn capture(args: &[&str]) {
    let status = Command::new(env!("CARGO_BIN_EXE_rogg-gallery")).args(args).status().expect("spawn gallery");
    assert!(status.success(), "capture failed: {args:?}");
}

#[test]
fn captures_controls_page_at_2x_with_geometry() {
    let dir = std::env::temp_dir().join(format!("rogg-capture-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("controls.png");
    let json = dir.join("controls.json");
    capture(&["--capture", "controls", "--size", "800x600", "--out", png.to_str().unwrap(), "--geometry", json.to_str().unwrap()]);
    assert_eq!(png_size(&png), (1600, 1200));
    let geometry: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(geometry["elements"]["flag-regex-on"]["h"], 27.0);
}

#[test]
fn captures_replica_with_rows_and_dialog() {
    let dir = std::env::temp_dir().join(format!("rogg-capture-replica-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("replica.png");
    let json = dir.join("replica.json");
    capture(&["--capture", "replica", "--theme", "light", "--density", "comfortable", "--dialog", "shortcuts", "--out", png.to_str().unwrap(), "--geometry", json.to_str().unwrap()]);
    assert_eq!(png_size(&png), (2880, 1800));
    let g: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    for key in ["titlebar", "tab-shell-0", "token-3", "row-top-0", "row-bottom-1", "gap-bottom-0", "statusbar", "dialog", "tree-row-10"] {
        assert!(g["elements"].get(key).is_some(), "missing geometry key {key}");
    }
    assert_eq!(g["elements"]["row-top-0"]["h"], 29.0, "comfortable row height");
}

#[test]
fn rejects_bad_arguments() {
    let out = Command::new(env!("CARGO_BIN_EXE_rogg-gallery")).args(["--capture", "nope", "--out", "/tmp/x.png"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown page"));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p rogg-gallery --test capture`
Expected: FAIL. The binary ignores `--capture` and opens a window, which a test runner cannot use, so the run hangs or times out. Stop it with Ctrl-C if needed.

- [ ] **Step 3: Implement `capture.rs`**

```rust
use crate::gallery::{GalleryView, Page};
use crate::replica::ReplicaDialog;
use gpui_kit::prelude::*;
use gpui_kit::{AppContext, Bounds, ElementId, Entity, HeadlessAppContext, Point, Window, WindowBounds, WindowOptions, px, size};
use rogg_ui::theme::{Appearance, Density, RoggTheme};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct CaptureOptions {
    pub page: Page,
    pub dialog: Option<ReplicaDialog>,
    pub appearance: Appearance,
    pub density: Density,
    pub width: f32,
    pub height: f32,
    pub out: PathBuf,
    pub geometry: Option<PathBuf>,
}

impl CaptureOptions {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
        let page_slug = value("--capture").ok_or("--capture <page> is required")?;
        let page = Page::from_slug(&page_slug).ok_or_else(|| format!("unknown page `{page_slug}`"))?;
        let dialog = match value("--dialog") {
            Some(d) => Some(ReplicaDialog::from_slug(&d).ok_or_else(|| format!("unknown dialog `{d}`"))?),
            None => None,
        };
        let appearance = match value("--theme").as_deref() {
            None | Some("dark") => Appearance::Dark,
            Some("light") => Appearance::Light,
            Some(other) => return Err(format!("unknown theme `{other}`")),
        };
        let density = match value("--density").as_deref() {
            None | Some("compact") => Density::Compact,
            Some("comfortable") => Density::Comfortable,
            Some(other) => return Err(format!("unknown density `{other}`")),
        };
        let (width, height) = match value("--size") {
            None => (1440., 900.),
            Some(s) => {
                let (w, h) = s.split_once('x').ok_or("--size must look like 1440x900")?;
                (w.parse().map_err(|_| "bad width")?, h.parse().map_err(|_| "bad height")?)
            }
        };
        let out = PathBuf::from(value("--out").ok_or("--out <png> is required")?);
        Ok(Self { page, dialog, appearance, density, width, height, out, geometry: value("--geometry").map(PathBuf::from) })
    }
}

#[derive(Serialize)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl From<Bounds<gpui_kit::Pixels>> for Rect {
    fn from(b: Bounds<gpui_kit::Pixels>) -> Self {
        Rect { x: f32::from(b.origin.x), y: f32::from(b.origin.y), w: f32::from(b.size.width), h: f32::from(b.size.height) }
    }
}

#[derive(Serialize)]
struct Geometry {
    scale: f32,
    window: Rect,
    elements: BTreeMap<String, Rect>,
}

fn collect_geometry(window: &Window, view: &Entity<GalleryView>, cx: &gpui_kit::App, opts: &CaptureOptions) -> Geometry {
    let mut elements = BTreeMap::new();
    for snap in gpui_kit::base::test_support::snapshots(window) {
        if let Some(ElementId::Name(name)) = snap.path().last() {
            elements.insert(name.to_string(), Rect::from(snap.bounds()));
        }
    }
    let replica = view.read(cx).replica.clone();
    for (prefix, state) in [("top", replica.read(cx).top.clone()), ("bottom", replica.read(cx).bottom.clone())] {
        let (mut line_ix, mut gap_ix) = (0, 0);
        for g in state.read(cx).last_rows() {
            let key = if g.is_gap {
                gap_ix += 1;
                format!("gap-{prefix}-{}", gap_ix - 1)
            } else {
                line_ix += 1;
                format!("row-{prefix}-{}", line_ix - 1)
            };
            elements.insert(key, Rect::from(g.bounds));
        }
    }
    Geometry { scale: 2., window: Rect { x: 0., y: 0., w: opts.width, h: opts.height }, elements }
}

/// Renders one page headlessly (Metal offscreen, scale 2) and writes a PNG plus optional geometry JSON.
pub fn run(opts: CaptureOptions) -> anyhow::Result<()> {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(rogg_ui::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    let (appearance, density) = (opts.appearance, opts.density);
    cx.update(|cx| {
        gpui_kit::init(cx);
        RoggTheme::new(appearance, density).install(cx);
    });
    let (page, dialog) = (opts.page, opts.dialog);
    let (handle, view) = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(opts.width), px(opts.height)) })),
            focus: false,
            show: false,
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, move |window, cx| cx.new(|cx| GalleryView::for_capture(page, dialog, window, cx)))
    })?;
    cx.run_until_parked();
    let geometry = cx.update_window(handle, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.render_frame(cx);
        collect_geometry(window, &view, cx, &opts)
    })?;
    let image = cx.capture_screenshot(handle)?;
    if let Some(parent) = opts.out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.save(&opts.out)?;
    if let Some(path) = &opts.geometry {
        std::fs::write(path, serde_json::to_string_pretty(&geometry)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn parses_defaults_and_flags() {
        let o = CaptureOptions::parse(&args("--capture replica --out a.png")).unwrap();
        assert_eq!((o.page, o.appearance, o.density, o.width, o.height), (Page::Replica, Appearance::Dark, Density::Compact, 1440., 900.));
        let o = CaptureOptions::parse(&args("--capture tabs --theme light --density comfortable --size 800x600 --dialog history --out b.png --geometry b.json")).unwrap();
        assert_eq!((o.appearance, o.density, o.width, o.dialog), (Appearance::Light, Density::Comfortable, 800., Some(ReplicaDialog::History)));
        assert!(CaptureOptions::parse(&args("--capture nope --out x.png")).unwrap_err().contains("unknown page"));
        assert!(CaptureOptions::parse(&args("--capture tabs")).unwrap_err().contains("--out"));
    }
}
```

In `gallery.rs`, add:
```rust
impl GalleryView {
    /// Page only, no gallery chrome; the replica opens `dialog` if given.
    pub fn for_capture(page: Page, dialog: Option<crate::replica::ReplicaDialog>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let this = Self::new(page, false, window, cx);
        if let Some(d) = dialog {
            this.replica.update(cx, |r, cx| r.open_dialog(d, cx));
        }
        this
    }
}
```

Add `pub mod capture;` to `lib.rs`. At the top of `main()` in `main.rs`, before the application starts, add:
```rust
    if args.iter().any(|a| a == "--capture") {
        match rogg_gallery::capture::CaptureOptions::parse(&args) {
            Ok(opts) => {
                if let Err(e) = rogg_gallery::capture::run(opts) {
                    eprintln!("capture failed: {e:#}");
                    std::process::exit(1);
                }
                return;
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        }
    }
```

Append to `justfile`:
```just
capture page="replica" theme="dark" density="compact":
    mkdir -p target/visual/manual
    cargo run -p rogg-gallery -- --capture {{page}} --theme {{theme}} --density {{density}} --out target/visual/manual/{{page}}-{{theme}}-{{density}}.png --geometry target/visual/manual/{{page}}-{{theme}}-{{density}}.json
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-gallery`
Expected: all tests pass, including 3 capture tests and the parse test.

Then run `just capture` and open `target/visual/manual/replica-dark-compact.png`. It must show the full replica at 2880×1800 with visible text, icons and highlights. If text or icons are missing, the text system or assets are not reaching the headless context. Stop and investigate with superpowers:systematic-debugging; do not continue to Task 16.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-gallery justfile
git commit -m "feat(gallery): headless --capture mode writing PNG and element geometry

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 16: Visual diff tool

**Files:**
- Create: `tools/visual-diff/{package.json,config.mjs,reference.mjs,gallery.mjs,compare.mjs,report.mjs}`
- Modify: `justfile`

**Interfaces:**
- Consumes: the reference HTML; `rogg-gallery --capture` (Task 15).
- Produces:
  - `npm run reference` writes `target/visual/reference/*.png|json`
  - `npm run gallery` writes `target/visual/gallery/*.png|json`
  - `npm run compare` writes `target/visual/diff/*.png`, `target/visual/results.json` and `target/visual/report.html`
  - `npm run all` runs all three
  - Capture names: `<theme>-<density>` and `<theme>-<density>--<dialog>`, for 4 variants × (1 window + 5 dialogs) = 24 pairs

- [ ] **Step 1: Install the tool's dependencies**

```bash
mkdir -p tools/visual-diff && cd tools/visual-diff
cat > package.json <<'EOF'
{
  "name": "rogg-visual-diff",
  "private": true,
  "type": "module",
  "scripts": {
    "reference": "node reference.mjs",
    "gallery": "node gallery.mjs",
    "compare": "node compare.mjs",
    "all": "node reference.mjs && node gallery.mjs && node compare.mjs",
    "approve": "node approve.mjs",
    "check": "node check.mjs"
  }
}
EOF
npm install --save-exact playwright pixelmatch pngjs
npx playwright install chromium
cd ../..
```
Expected: `package-lock.json` is created and Chromium is downloaded.

- [ ] **Step 2: Write `config.mjs`**

```js
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
export const REFERENCE_HTML = path.join(ROOT, 'docs/superpowers/specs/assets/rogg-main-window.html');
export const OUT = path.join(ROOT, 'target/visual');
export const GALLERY_BIN = path.join(ROOT, 'target/debug/rogg-gallery');
export const GOLDEN = path.join(ROOT, 'tools/visual-diff/golden');

export const SIZE = { width: 1440, height: 900 };
export const SCALE = 2;

export const VARIANTS = [
  { theme: 'dark', density: 'compact' },
  { theme: 'dark', density: 'comfortable' },
  { theme: 'light', density: 'compact' },
  { theme: 'light', density: 'comfortable' },
];

// Reference <dialog> id → gallery --dialog slug.
export const DIALOGS = [
  { html: 'tabs-dialog', gallery: 'all-tabs' },
  { html: 'profile-dialog', gallery: 'profile' },
  { html: 'settings-dialog', gallery: 'appearance' },
  { html: 'shortcuts-dialog', gallery: 'shortcuts' },
  { html: 'history-dialog', gallery: 'history' },
];

export const variantName = (v) => `${v.theme}-${v.density}`;
export const captureNames = () => VARIANTS.flatMap((v) => [variantName(v), ...DIALOGS.map((d) => `${variantName(v)}--${d.gallery}`)]);

// Geometry key → reference selector. `many` expands to key-0, key-1, …; `offset` shifts the index.
export const SELECTORS = [
  { key: 'titlebar', sel: '.titlebar' },
  { key: 'titlebar-leading', sel: '.titlebar-leading' },
  { key: 'tab-shell', sel: '.tab-shell', many: true },
  { key: 'title-actions', sel: '.title-actions' },
  { key: 'sidebar', sel: '.sidebar' },
  { key: 'side-top', sel: '.side-top' },
  { key: 'tree-row-0', sel: '.side-body .folder' },
  { key: 'tree-row', sel: '#file-list .file', many: true, offset: 1 },
  { key: 'recent', sel: '.recent', many: true },
  { key: 'side-bottom', sel: '.side-bottom' },
  { key: 'meta', sel: '.meta' },
  { key: 'profile', sel: '.meta .right button:first-child' },
  { key: 'export', sel: '.meta .right button:last-child' },
  { key: 'pane-top-head', sel: '.pane.top .pane-head' },
  { key: 'find-top', sel: '.pane.top .find' },
  { key: 'tool-wrap-top', sel: '#wrap-top' },
  { key: 'log-top', sel: '#log-top' },
  { key: 'filter-band', sel: '.filter-band' },
  { key: 'token', sel: '.token', many: true },
  { key: 'token-state', sel: '.token .state', many: true },
  { key: 'filter-actions', sel: '.filter-actions' },
  { key: 'edit-filters', sel: '#edit-button' },
  { key: 'flag-regex', sel: '#regex' },
  { key: 'flag-case', sel: '#case' },
  { key: 'filter-bottom', sel: '.filter-bottom' },
  { key: 'history', sel: '.filter-bottom > button' },
  { key: 'pane-bottom-head', sel: '.pane.bottom .pane-head' },
  { key: 'find-bottom', sel: '.pane.bottom .find' },
  { key: 'tool-follow', sel: '#follow' },
  { key: 'tool-wrap-bottom', sel: '#wrap-bottom' },
  { key: 'log-bottom', sel: '#log-bottom' },
  { key: 'row-top', sel: '#log-top .row', many: true, limit: 20 },
  { key: 'row-bottom', sel: '#log-bottom .row', many: true },
  { key: 'gap-bottom', sel: '#log-bottom .gap-row', many: true },
  { key: 'statusbar', sel: '.footer' },
  { key: 'dialog', sel: 'dialog[open]' },
];

// Color probes: a point at fraction (fx, fy) of a reference element's rect, compared in both images.
export const PROBES = [
  { name: 'title bar bg', key: 'titlebar', fx: 0.62, fy: 0.12 },
  { name: 'active tab bg', key: 'tab-shell-0', fx: 0.97, fy: 0.15 },
  { name: 'sidebar bg', key: 'sidebar', fx: 0.04, fy: 0.75 },
  { name: 'active file row', key: 'tree-row-1', fx: 0.6, fy: 0.15 },
  { name: 'info bar bg', key: 'meta', fx: 0.55, fy: 0.2 },
  { name: 'pane header bg', key: 'pane-top-head', fx: 0.4, fy: 0.2 },
  { name: 'find field bg', key: 'find-top', fx: 0.9, fy: 0.25 },
  { name: 'filter band bg', key: 'filter-band', fx: 0.3, fy: 0.2 },
  { name: 'filter band top border', key: 'filter-band', fx: 0.3, fy: 0.0 },
  { name: 'chip 0 fill', key: 'token-0', fx: 0.92, fy: 0.12 },
  { name: 'chip 3 (off) fill', key: 'token-3', fx: 0.92, fy: 0.12 },
  { name: 'regex flag border', key: 'flag-regex', fx: 0.5, fy: 0.0 },
  { name: 'follow (pressed) fill', key: 'tool-follow', fx: 0.05, fy: 0.5 },
  { name: 'selected row (top)', key: 'row-top-12', fx: 0.995, fy: 0.5 },
  { name: 'error row tint (top)', key: 'row-top-6', fx: 0.995, fy: 0.5 },
  { name: 'gap row bg', key: 'gap-bottom-0', fx: 0.9, fy: 0.5 },
  { name: 'status bar bg', key: 'statusbar', fx: 0.5, fy: 0.5 },
  { name: 'dialog bg', key: 'dialog', fx: 0.5, fy: 0.03 },
  { name: 'backdrop', key: 'statusbar', fx: 0.02, fy: 0.5 },
];
```

- [ ] **Step 3: Write `reference.mjs`**

```js
import { chromium } from 'playwright';
import fs from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { DIALOGS, OUT, REFERENCE_HTML, SCALE, SELECTORS, SIZE, VARIANTS, variantName } from './config.mjs';

// Strip the "study" frame so .app fills the viewport like a native window.
const CSS = `.study{max-width:none!important;padding:0!important;margin:0!important}
.study-head,.study-foot{display:none!important}
.app{height:${SIZE.height}px!important;min-height:0!important;border:0!important;border-radius:0!important;box-shadow:none!important}`;

async function geometry(page) {
  return page.evaluate((selectors) => {
    const app = document.querySelector('.app').getBoundingClientRect();
    const rect = (n) => {
      const r = n.getBoundingClientRect();
      return { x: r.x - app.x, y: r.y - app.y, w: r.width, h: r.height };
    };
    const out = {};
    for (const s of selectors) {
      const nodes = [...document.querySelectorAll(s.sel)].filter((n) => n.getClientRects().length).slice(0, s.limit ?? 1000);
      if (s.many) nodes.forEach((n, i) => (out[`${s.key}-${i + (s.offset ?? 0)}`] = rect(n)));
      else if (nodes[0]) out[s.key] = rect(nodes[0]);
    }
    return out;
  }, SELECTORS);
}

const dir = path.join(OUT, 'reference');
await fs.mkdir(dir, { recursive: true });
const browser = await chromium.launch();
const context = await browser.newContext({ viewport: SIZE, deviceScaleFactor: SCALE });
const page = await context.newPage();
for (const v of VARIANTS) {
  await page.goto(pathToFileURL(REFERENCE_HTML).href);
  await page.addStyleTag({ content: CSS });
  await page.evaluate(({ theme, density }) => {
    document.body.classList.toggle('light', theme === 'light');
    setDensity(density === 'comfortable');
  }, v);
  await page.evaluate(() => document.fonts.ready);
  await page.mouse.move(SIZE.width - 2, SIZE.height - 2);
  const name = variantName(v);
  await page.locator('.app').screenshot({ path: path.join(dir, `${name}.png`) });
  await fs.writeFile(path.join(dir, `${name}.json`), JSON.stringify({ elements: await geometry(page) }, null, 2));
  for (const d of DIALOGS) {
    await page.evaluate((id) => document.getElementById(id).showModal(), d.html);
    const file = `${name}--${d.gallery}`;
    await page.screenshot({ path: path.join(dir, `${file}.png`), clip: { x: 0, y: 0, ...SIZE } });
    await fs.writeFile(path.join(dir, `${file}.json`), JSON.stringify({ elements: await geometry(page) }, null, 2));
    await page.evaluate((id) => document.getElementById(id).close(), d.html);
  }
  console.log(`reference ${name} done`);
}
await browser.close();
```

- [ ] **Step 4: Write `gallery.mjs`**

```js
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { DIALOGS, GALLERY_BIN, OUT, ROOT, SIZE, VARIANTS, variantName } from './config.mjs';

execFileSync('cargo', ['build', '-p', 'rogg-gallery'], { cwd: ROOT, stdio: 'inherit' });
const dir = path.join(OUT, 'gallery');
fs.mkdirSync(dir, { recursive: true });
const capture = (file, v, extra = []) =>
  execFileSync(
    GALLERY_BIN,
    ['--capture', 'replica', '--theme', v.theme, '--density', v.density, '--size', `${SIZE.width}x${SIZE.height}`,
      '--out', path.join(dir, `${file}.png`), '--geometry', path.join(dir, `${file}.json`), ...extra],
    { stdio: 'inherit' },
  );
for (const v of VARIANTS) {
  const name = variantName(v);
  capture(name, v);
  for (const d of DIALOGS) capture(`${name}--${d.gallery}`, v, ['--dialog', d.gallery]);
  console.log(`gallery ${name} done`);
}
```

- [ ] **Step 5: Write `compare.mjs` and `report.mjs`**

`compare.mjs`:
```js
import fs from 'node:fs';
import path from 'node:path';
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';
import { OUT, PROBES, SCALE, captureNames } from './config.mjs';
import { writeReport } from './report.mjs';

const readPng = (f) => PNG.sync.read(fs.readFileSync(f));
const readGeometry = (f) => JSON.parse(fs.readFileSync(f, 'utf8')).elements;
const pixel = (png, x, y) => {
  const xi = Math.min(png.width - 1, Math.max(0, Math.round(x * SCALE)));
  const yi = Math.min(png.height - 1, Math.max(0, Math.round(y * SCALE)));
  const i = (yi * png.width + xi) * 4;
  return [png.data[i], png.data[i + 1], png.data[i + 2]];
};
const hex = (c) => '#' + c.map((v) => v.toString(16).padStart(2, '0')).join('');
const distance = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

fs.mkdirSync(path.join(OUT, 'diff'), { recursive: true });
const results = [];
for (const name of captureNames()) {
  const ref = readPng(path.join(OUT, 'reference', `${name}.png`));
  const gal = readPng(path.join(OUT, 'gallery', `${name}.png`));
  const result = { name, sizeMismatch: ref.width !== gal.width || ref.height !== gal.height };
  if (!result.sizeMismatch) {
    const diff = new PNG({ width: ref.width, height: ref.height });
    const count = pixelmatch(ref.data, gal.data, diff.data, ref.width, ref.height, { threshold: 0.1, includeAA: false, alpha: 0.25 });
    fs.writeFileSync(path.join(OUT, 'diff', `${name}.png`), PNG.sync.write(diff));
    result.diffPercent = (100 * count) / (ref.width * ref.height);
  }
  const rg = readGeometry(path.join(OUT, 'reference', `${name}.json`));
  const gg = readGeometry(path.join(OUT, 'gallery', `${name}.json`));
  result.geometry = Object.keys(rg)
    .filter((k) => k in gg)
    .map((k) => {
      const a = rg[k];
      const b = gg[k];
      const d = { dx: b.x - a.x, dy: b.y - a.y, dw: b.w - a.w, dh: b.h - a.h };
      return { key: k, ref: a, gallery: b, ...d, max: Math.max(...Object.values(d).map(Math.abs)) };
    })
    .sort((a, b) => b.max - a.max);
  result.missing = Object.keys(rg).filter((k) => !(k in gg));
  result.probes = PROBES.filter((p) => rg[p.key])
    .map((p) => {
      const r = rg[p.key];
      const x = r.x + r.w * p.fx;
      const y = r.y + r.h * p.fy;
      const a = pixel(ref, x, y);
      const b = pixel(gal, x, y);
      return { name: p.name, x: +x.toFixed(1), y: +y.toFixed(1), ref: hex(a), gallery: hex(b), distance: +distance(a, b).toFixed(1) };
    })
    .sort((a, b) => b.distance - a.distance);
  results.push(result);
}
fs.writeFileSync(path.join(OUT, 'results.json'), JSON.stringify(results, null, 2));
writeReport(results);
console.table(
  results.map((r) => ({
    capture: r.name,
    'diff %': r.sizeMismatch ? 'SIZE MISMATCH' : r.diffPercent.toFixed(2),
    'worst geometry Δ px': r.geometry[0] ? `${r.geometry[0].key} ${r.geometry[0].max.toFixed(1)}` : '-',
    'worst probe Δ': r.probes[0] ? `${r.probes[0].name} ${r.probes[0].distance}` : '-',
    'missing keys': r.missing.length,
  })),
);
console.log(`report: ${path.join(OUT, 'report.html')}`);
```

`report.mjs`:
```js
import fs from 'node:fs';
import path from 'node:path';
import { OUT } from './config.mjs';

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
const rect = (r) => `${r.x.toFixed(1)}, ${r.y.toFixed(1)} · ${r.w.toFixed(1)}×${r.h.toFixed(1)}`;

export function writeReport(results) {
  const sections = results
    .map((r) => {
      const geometry = r.geometry
        .slice(0, 40)
        .map((g) => `<tr class="${g.max > 2 ? 'bad' : g.max > 0.5 ? 'warn' : ''}"><td>${esc(g.key)}</td><td>${rect(g.ref)}</td><td>${rect(g.gallery)}</td><td>${g.dx.toFixed(1)}</td><td>${g.dy.toFixed(1)}</td><td>${g.dw.toFixed(1)}</td><td>${g.dh.toFixed(1)}</td></tr>`)
        .join('');
      const probes = r.probes
        .map((p) => `<tr class="${p.distance > 12 ? 'bad' : p.distance > 4 ? 'warn' : ''}"><td>${esc(p.name)}</td><td>${p.x}, ${p.y}</td><td><span class="sw" style="background:${p.ref}"></span>${p.ref}</td><td><span class="sw" style="background:${p.gallery}"></span>${p.gallery}</td><td>${p.distance}</td></tr>`)
        .join('');
      return `<section id="${esc(r.name)}"><h2>${esc(r.name)} <small>${r.sizeMismatch ? 'SIZE MISMATCH' : r.diffPercent.toFixed(2) + '% pixels differ'} · ${r.missing.length} geometry keys missing</small></h2>
<div class="overlay"><img src="reference/${r.name}.png"><img class="top" src="gallery/${r.name}.png" style="opacity:.5"></div>
<label>gallery opacity <input type="range" min="0" max="100" value="50" oninput="this.parentElement.previousElementSibling.querySelector('.top').style.opacity=this.value/100"></label>
<div class="grid"><figure><img src="reference/${r.name}.png"><figcaption>reference</figcaption></figure><figure><img src="gallery/${r.name}.png"><figcaption>gallery</figcaption></figure><figure><img src="diff/${r.name}.png"><figcaption>diff</figcaption></figure></div>
<h3>Geometry (largest deviation first, logical px)</h3><table><tr><th>key</th><th>reference</th><th>gallery</th><th>Δx</th><th>Δy</th><th>Δw</th><th>Δh</th></tr>${geometry}</table>
${r.missing.length ? `<p class="missing">Missing in gallery: ${r.missing.map(esc).join(', ')}</p>` : ''}
<h3>Color probes</h3><table><tr><th>probe</th><th>at</th><th>reference</th><th>gallery</th><th>Δ (RGB distance)</th></tr>${probes}</table></section>`;
    })
    .join('\n');
  const toc = results.map((r) => `<a href="#${esc(r.name)}">${esc(r.name)}</a>`).join(' · ');
  const html = `<!doctype html><meta charset="utf-8"><title>rogg visual diff</title>
<style>body{font:13px -apple-system,sans-serif;margin:24px;background:#111315;color:#e2e6e9}a{color:#a8c5b7}section{margin:40px 0;border-top:1px solid #34393f;padding-top:16px}
small{color:#929da7;font-weight:400}.overlay{position:relative;width:1440px;max-width:100%}.overlay img{width:100%;display:block}.overlay .top{position:absolute;inset:0}
.grid{display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-top:12px}.grid img{width:100%}figcaption{color:#929da7;font-size:11px}
table{border-collapse:collapse;margin:8px 0;font:11px ui-monospace,monospace}td,th{border:1px solid #34393f;padding:3px 6px;text-align:left}
tr.warn td{background:#443b28}tr.bad td{background:#49302f}.sw{display:inline-block;width:12px;height:12px;border:1px solid #fff3;margin-right:6px;vertical-align:-2px}.missing{color:#e0c283}</style>
<h1>rogg ⇄ reference</h1><p>${toc}</p>${sections}`;
  fs.writeFileSync(path.join(OUT, 'report.html'), html);
}
```

- [ ] **Step 6: Add the `just` recipe and run the whole thing**

Append to `justfile`:
```just
visual:
    cd tools/visual-diff && npm run all
    open target/visual/report.html
```

Run: `just visual`
Expected:
- 24 reference PNGs and 24 gallery PNGs, each 2880×1800;
- a console table with one row per capture;
- `target/visual/report.html` opens;
- no `SIZE MISMATCH` rows.

Missing keys are acceptable only for reference rows that are scrolled out of view in the gallery (`row-top-N` beyond the visible rows).

- [ ] **Step 7: Commit**

```bash
git add tools/visual-diff/package.json tools/visual-diff/package-lock.json tools/visual-diff/*.mjs justfile
git commit -m "feat(tools): visual diff of gallery replica against the reference HTML

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 17: Fix pass, joint review and golden images

**Files:**
- Modify: `crates/rogg-ui/src/**` (fixes found by the report)
- Create: `docs/superpowers/reviews/2026-10-06-m0-visual-review.md`
- Create: `tools/visual-diff/{approve.mjs,check.mjs}`, `tools/visual-diff/golden/*`
- Modify: `justfile`

**Interfaces:**
- Produces:
  - `npm run approve` copies the approved gallery captures into `golden/` and writes `golden/tolerance.json`
  - `npm run check` (and `just visual-check`) re-captures the gallery and fails if any golden capture differs by more than its tolerance

- [ ] **Step 1: Fix the obvious mismatches**

Work through `target/visual/report.html` from the top of each geometry and probe table. For each mismatch:
- Find the CSS rule in the reference HTML that defines the value.
- Change only the matching rogg-ui component or theme constant. If a theme constant changes, `theme_tokens.rs` must still pass; a failure there means the fix is wrong.
- Re-run `just visual`.
- Commit each logical fix separately, with a message like `fix(ui): <component> matches reference <property>`.

Stop iterating when no element is off by more than about 2 px except where the cause is font glyph metrics, and no probe differs by more than about 12 RGB distance. These numbers are guidance, not a gate; the user decides in Step 3.

Expected sources of residual difference, to be recorded rather than chased:
- text anti-aliasing and glyph rasterization (Chromium vs CoreText through gpui);
- CSS `font-weight: 550/650`, which gpui snaps to the nearest installed face;
- the native title bar has no fake traffic lights (the gallery draws them only for capture);
- elements added deliberately by the spec: the "Open as tab" tool button.

- [ ] **Step 2: Write the review log**

`docs/superpowers/reviews/2026-10-06-m0-visual-review.md`:
```markdown
# M0 visual review — gallery replica vs reference

Report: `target/visual/report.html` (regenerate with `just visual`)
Live: `cargo run -p rogg-gallery -- --page replica`, next to `docs/superpowers/specs/assets/rogg-main-window.html`

## Summary per capture
| Capture | Pixel diff % | Worst geometry Δ | Worst probe Δ | Notes |
|---|---|---|---|---|
<!-- copy the console table from `npm run compare` -->

## Fixed during the review
<!-- one line per fix commit: component — property — before → after -->

## Accepted differences
<!-- one line each: what differs, why it is accepted, who accepted it (user / pending) -->
- Text rasterization: Chromium vs CoreText anti-aliasing (expected, not fixable).
- Font weights 550/650 render as the nearest installed SF Pro face.
- "Open as tab" tool button in the filtered pane header (spec §5.3 addition).

## Open questions for the user
<!-- anything that needs a decision -->
```
Fill in every section with real data from the latest run before Step 3.

- [ ] **Step 3: CHECKPOINT. Joint review with the user (stop here)**

Do not continue until the user approves. Send the user:
1. the path to `target/visual/report.html` and the command `cargo run -p rogg-gallery -- --page replica`;
2. the summary table and the "Accepted differences" and "Open questions" sections of the review log;
3. the question: "Please review the report and the live gallery. Which differences should be fixed, and is the look approved?"

Apply each requested change as a separate commit, re-run `just visual`, update the review log, and ask again. Repeat until the user explicitly approves. Record the approval (date and wording) in the review log.

- [ ] **Step 4: Write `approve.mjs` and `check.mjs`**

`tools/visual-diff/approve.mjs`:
```js
import fs from 'node:fs';
import path from 'node:path';
import { GOLDEN, OUT, captureNames } from './config.mjs';

// Golden set: all four window variants plus every dialog in dark-compact (keeps the repo small).
const names = captureNames().filter((n) => !n.includes('--') || n.startsWith('dark-compact--'));
fs.mkdirSync(GOLDEN, { recursive: true });
for (const n of names) fs.copyFileSync(path.join(OUT, 'gallery', `${n}.png`), path.join(GOLDEN, `${n}.png`));
const tolerance = Object.fromEntries(names.map((n) => [n, 0.5]));
fs.writeFileSync(path.join(GOLDEN, 'tolerance.json'), JSON.stringify(tolerance, null, 2));
console.log(`approved ${names.length} golden captures into ${GOLDEN}`);
```

`tools/visual-diff/check.mjs`:
```js
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';
import { GOLDEN, OUT, ROOT } from './config.mjs';

// Re-capture the gallery, then compare against the approved golden images.
execFileSync('node', [path.join(ROOT, 'tools/visual-diff/gallery.mjs')], { stdio: 'inherit' });
const tolerance = JSON.parse(fs.readFileSync(path.join(GOLDEN, 'tolerance.json'), 'utf8'));
let failed = 0;
for (const [name, maxPercent] of Object.entries(tolerance)) {
  const golden = PNG.sync.read(fs.readFileSync(path.join(GOLDEN, `${name}.png`)));
  const fresh = PNG.sync.read(fs.readFileSync(path.join(OUT, 'gallery', `${name}.png`)));
  if (golden.width !== fresh.width || golden.height !== fresh.height) {
    console.error(`✗ ${name}: size ${fresh.width}×${fresh.height}, golden ${golden.width}×${golden.height}`);
    failed++;
    continue;
  }
  const diff = new PNG({ width: golden.width, height: golden.height });
  const count = pixelmatch(golden.data, fresh.data, diff.data, golden.width, golden.height, { threshold: 0.1, includeAA: false });
  const percent = (100 * count) / (golden.width * golden.height);
  if (percent > maxPercent) {
    fs.mkdirSync(path.join(OUT, 'check'), { recursive: true });
    fs.writeFileSync(path.join(OUT, 'check', `${name}.png`), PNG.sync.write(diff));
    console.error(`✗ ${name}: ${percent.toFixed(2)}% differs (max ${maxPercent}%), diff in target/visual/check/${name}.png`);
    failed++;
  } else {
    console.log(`✓ ${name}: ${percent.toFixed(3)}%`);
  }
}
process.exit(failed ? 1 : 0);
```

Append to `justfile`:
```just
visual-check:
    cd tools/visual-diff && npm run check
```

- [ ] **Step 5: Approve, verify and commit**

Only after the user's explicit approval in Step 3:

```bash
cd tools/visual-diff && npm run approve && cd ../..
just visual-check
just test
```
Expected: `visual-check` prints ✓ for all 9 golden captures and `just test` is green.

```bash
git add crates docs/superpowers/reviews tools/visual-diff/approve.mjs tools/visual-diff/check.mjs tools/visual-diff/golden justfile
git commit -m "test(visual): approve M0 gallery replica and add golden-image check

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

M0 is complete when the user has approved, the golden images are committed, and `just test` plus `just visual-check` pass. The next step is writing the M1 (engine) plan.
