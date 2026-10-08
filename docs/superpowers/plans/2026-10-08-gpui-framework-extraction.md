# gpui-framework Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Copy rogg's GUI into a new, local-only repository at `/Users/vadim/dev/projects/gpui-framework`. Generalise the log-specific parts into neutral widgets. Add the theme builder, gallery, template, tooling and docs, so a new desktop app can depend on it and look like rogg.

**Architecture:**
- **Workspace:** one library crate (`gpui-framework`, imported as `gf`) plus two apps (`gpui-framework-gallery`, `gpui-framework-template`) in a new workspace.
- **Source:** the code comes from rogg at `main` f899486. It is copied with a one-off rename tool, then hand-edited where the spec generalises an API.
- **rogg:** not touched. The only rogg commits are the spec (already committed, a86596b) and this plan.

**Tech Stack:**
- Rust 1.99.0 (edition 2024).
- gpui-pre 0.3.8 through gpui-kit `=0.7.1`, re-exported as `gf::kit`.
- `rust-embed` for the icons.
- Node + Playwright + pixelmatch for golden images.
- zsh for `./dev`.

**Spec:** `docs/superpowers/specs/2026-10-08-gpui-framework-extraction-design.md` (in the rogg repo). Task 1 copies it into the new repo as `docs/history/2026-10-08-gpui-framework-extraction-design.md`. That is a deviation from spec §3/§10, which named `docs/spec/`: the record necessarily names the source app, and `docs/history/` is the one place that may. `docs/spec/README.md` points to it.

**Source tree (read-only):** `/Users/vadim/dev/projects/rogg2` at f899486. Paths below written as `rogg:crates/…` are in that tree. Paths without a prefix are in the new repo.

## Global Constraints

- **Location:** the new repository is `/Users/vadim/dev/projects/gpui-framework`.
  - `git init` it, and use a local branch `main`.
  - **No remote. Nothing is pushed or published.**
- **rogg is read-only:** never modify, build in, or commit to `/Users/vadim/dev/projects/rogg2` while executing this plan. Copy from it only.
- **No "rogg" anywhere:**
  - not in crate names, types, functions, element ids, asset paths, env vars, script names or docs;
  - the only exceptions are `docs/history/` and, until Task 13 deletes it, the one-off `tools/port/`;
  - `crates/gpui-framework/tests/no_rogg.rs` enforces this from Task 1 on.
- **Toolchain:**
  - `rust-toolchain.toml` channel `1.99.0` with rustfmt and clippy;
  - edition 2024;
  - `gpui-kit = "=0.7.1"`, `rust-embed = { version = "8.7.2", features = ["include-exclude"] }`;
  - the library re-exports gpui-kit as `pub use gpui_kit as kit;`;
  - code imports gpui as `gpui_kit`, never `gpui::`, except for the `extern crate gpui_kit as gpui;` alias that the Action derive needs.
- **Crate names:** `gpui-framework`, `gpui-framework-gallery`, `gpui-framework-template`. Docs and examples write `use gpui_framework as gf;`.
- **Theme accessor:** the trait is `ActiveTheme` and the method is `gf_theme()`. gpui-kit already owns `cx.theme()`.
- **Default look:** `Theme::new(appearance, density)`, `Theme::default_dark()` and `Theme::default_light()` must reproduce rogg's palette, metrics and fonts exactly.
- **Component contract:** plain data plus callbacks, `RenderOnce` builders, element ids marked `.test_support()`, no app state.
- **`./dev` is the entry point** in the new repo, and it sets the PATH for the keg-only rustup itself. Every step runs from `/Users/vadim/dev/projects/gpui-framework`.
- **Commits:**
  - conventional style: `feat: …`, `test: …`, `docs: …`, `build: …`, `chore: …`;
  - every commit message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Goldens:** generated fresh in Task 12 and approved by the user. Never approved silently.

## Review Focus

These are failure modes the spec implies but would otherwise go untested. Each has a test in its owning task.

1. **Theme not installed.** An app that renders a component before installing the theme should get a clear panic that says what to call, not an opaque `global not found` (Task 2: `gf_theme_without_install_panics_with_a_hint`).
2. **An app with its own assets.** The app's files must be found first, then the framework's icons, then gpui-kit's (Task 2: `app_assets_are_tried_before_framework_icons`).
3. **Theme overrides survive a light/dark switch.** Rebuilding a `ThemeBuilder` for the other appearance keeps the accent, the palette edits and the fonts (Task 2: `overrides_survive_an_appearance_switch`).
4. **Depending on the framework from outside its workspace.** A copied template with a `path` dependency must compile on its own (Task 11, Step 6: an out-of-workspace build).
5. **Empty input to builders.** `picker` with no items, `key_value_list` with no rows, and an empty recent list must render without a panic (Task 5: `generic_builders_accept_empty_input`).

---

## File map (rogg → framework)

| rogg file | framework file |
|---|---|
| `rogg:crates/rogg-ui/src/theme/{mod,color,metrics,palette,kit}.rs` | `crates/gpui-framework/src/theme/{mod,color,metrics,palette,kit}.rs`, plus a new `theme/builder.rs` |
| `rogg:crates/rogg-ui/src/{assets,icons}.rs` | `crates/gpui-framework/src/{assets,icons}.rs` |
| `rogg:crates/rogg-ui/assets/icons/rogg/*.svg` | `crates/gpui-framework/assets/icons/gf/*.svg` |
| `rogg:crates/rogg-ui/src/controls/*.rs` | `crates/gpui-framework/src/controls/*.rs` |
| `rogg:crates/rogg-ui/src/{form,toast,empty,dialogs,layout}.rs` | `crates/gpui-framework/src/{form,toast,empty,dialogs,layout}.rs` |
| `rogg:crates/rogg-ui/src/titlebar.rs` | `crates/gpui-framework/src/chrome/title_bar.rs` |
| `rogg:crates/rogg-ui/src/sidebar.rs` | `crates/gpui-framework/src/chrome/sidebar.rs` |
| `rogg:crates/rogg-ui/src/chrome.rs` (InfoBar, PaneHeader, StatusBar, bar_fraction) | `crates/gpui-framework/src/chrome/{info_bar,pane_header,status_bar}.rs` + `chrome/mod.rs` |
| `rogg:crates/rogg-ui/src/logview/*.rs` | `crates/gpui-framework/src/widgets/text_view/*.rs` |
| `rogg:crates/rogg-ui/src/filter.rs` | `crates/gpui-framework/src/widgets/chip_band.rs` |
| `rogg:crates/rogg-ui/tests/*.rs` (+ `common/`) | `crates/gpui-framework/tests/*.rs` (renamed per task) |
| `rogg:crates/rogg-gallery/src/{main,lib,gallery,pages,fixtures,capture}.rs` | `crates/gpui-framework-gallery/src/…` |
| `rogg:crates/rogg-gallery/src/replica/*` | `crates/gpui-framework-gallery/src/showcase/*` |
| `rogg:crates/rogg-gallery/tests/{capture,pages}.rs` | `crates/gpui-framework-gallery/tests/{capture,pages}.rs` |
| `rogg:tools/visual-diff/{config,gallery,check,approve,compare}.mjs`, `package.json`, `package-lock.json` | `tools/visual-diff/…`; `reference.mjs`, `report.mjs` and the HTML reference are not copied |
| `rogg:dev`, `rogg:justfile`, `rogg:rust-toolchain.toml` | `dev`, `justfile`, `rust-toolchain.toml` (adapted) |
| `rogg:docs/superpowers/specs/2026-10-06-rogg-design.md`, `…/2026-10-08-rogg-refinements-1-design.md` | `docs/history/rogg-design.md`, `docs/history/rogg-refinements-1-design.md` |
| `rogg:docs/superpowers/research/2026-10-06-gpui-api-notes.md` | source material for `docs/guides/gpui-gotchas.md` |

**Not copied:**
- the domain-specific form parts `EditableRuleRow`, `PaletteSlot` and `ProfileRuleView`;
- the rogg-only dialog builders `history`, `history_picker`, `export`, `profile` and `all_tabs`;
- the gallery's `Page::App` page and its `rogg-app`/`rogg-core` dependencies.

## Rename map (applied by `tools/port/port.py`, Task 1)

Applied in this order, as whole-word regexes:

| from | to |
|---|---|
| `ActiveRoggTheme` | `ActiveTheme` |
| `RoggTheme` | `Theme` |
| `.rogg()` | `.gf_theme()` |
| `icons/rogg/` | `icons/gf/` |
| `LogViewElement` / `LogViewState` / `LogViewEvent` / `LogView` | `TextViewElement` / `TextViewState` / `TextViewEvent` / `TextView` |
| `LogRowSource` / `LogLine` / `LogRow` | `RowSource` / `Line` / `Row` |
| `FilterBand` / `FilterChip` | `ChipBand` / `Chip` |
| `SpanStyle::Amber` / `SpanStyle::Hit` / `SpanStyle::SearchHit` | `SpanStyle::Warning` / `SpanStyle::Highlight` / `SpanStyle::Outline` |
| `PROFILE_DIALOG_W` / `FULL_LINE_DIALOG_W` / `FULL_LINE_TEXT_H` / `FULL_LINE_SHOWN_BYTES` / `full_line_shown` | `WIDE_DIALOG_W` / `TEXT_VIEWER_DIALOG_W` / `TEXT_VIEWER_TEXT_H` / `TEXT_VIEWER_SHOWN_BYTES` / `text_viewer_shown` |
| `crate::logview` / `rogg_ui::logview` | `crate::widgets::text_view` / `…::widgets::text_view` |
| `crate::filter` / `rogg_ui::filter` | `crate::widgets::chip_band` / `…::widgets::chip_band` |
| `crate::titlebar` / `rogg_ui::titlebar` | `crate::chrome::title_bar` / `…::chrome::title_bar` |
| `crate::sidebar` / `rogg_ui::sidebar` | `crate::chrome::sidebar` / `…::chrome::sidebar` |
| `"LogView"` | `"TextView"` |
| `rogg_ui::` | `crate::` (library sources, `--lib`) or `gpui_framework::` (tests, gallery, template) |
| `rogg_gallery::` | `gpui_framework_gallery::` |
| `ROGG_GALLERY_SMOKE` | `GF_GALLERY_SMOKE` |

Anything the map does not cover, such as field renames and generalised APIs, is a hand edit listed in the owning task. After every port the `no_rogg` test must pass.

---

# Stage 1 — Scaffold

### Task 1: Repository, workspace, `./dev`, port tool, and the no-rogg guard

**Files:**
- Create:
  - `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`;
  - `crates/gpui-framework/Cargo.toml`, `crates/gpui-framework/src/lib.rs`, `crates/gpui-framework/tests/no_rogg.rs`;
  - `dev`, `justfile`, `README.md`;
  - `tools/port/port.py`;
  - `docs/history/2026-10-08-gpui-framework-extraction-design.md` (copied from rogg) and `docs/spec/README.md`.
- Test: `crates/gpui-framework/tests/no_rogg.rs`, plus a unit test in `lib.rs`.

**Interfaces:**
- Produces:
  - the workspace with member `crates/gpui-framework` (Tasks 10 and 11 add the gallery and template);
  - `gpui_framework::{version() -> &'static str, Callback, IndexCallback, kit}`;
  - `tools/port/port.py SRC DST [--lib]`;
  - `./dev` with `help setup build test lint fmt check clean`. Later tasks add `gallery smoke capture template visual-check visual-approve`.

- [ ] **Step 1: Create the repository**

```bash
mkdir -p /Users/vadim/dev/projects/gpui-framework && cd /Users/vadim/dev/projects/gpui-framework
git init -b main
mkdir -p crates/gpui-framework/src crates/gpui-framework/tests tools/port docs/spec docs/history
cp /Users/vadim/dev/projects/rogg2/docs/superpowers/specs/2026-10-08-gpui-framework-extraction-design.md docs/history/
```

`.gitignore`:

```
/target
tools/visual-diff/node_modules
.DS_Store
```

- [ ] **Step 2: Workspace and toolchain**

`Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["crates/gpui-framework"]

[workspace.package]
version = "0.1.0"
edition = "2024"
publish = false
license = "Apache-2.0"

[workspace.dependencies]
gpui-kit = "=0.7.1"
rust-embed = { version = "8.7.2", features = ["include-exclude"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tempfile = "=3.27.0"
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.99.0"
components = ["rustfmt", "clippy"]
```

`crates/gpui-framework/Cargo.toml`:

```toml
[package]
name = "gpui-framework"
description = "A desktop GUI framework on gpui-kit: theme, components, widgets"
version.workspace = true
edition.workspace = true
publish.workspace = true
license.workspace = true

[dependencies]
gpui-kit.workspace = true
rust-embed.workspace = true

[dev-dependencies]
gpui-kit = { workspace = true, features = ["test-support"] }
```

- [ ] **Step 3: Write the failing tests**

`crates/gpui-framework/tests/no_rogg.rs`:

```rust
//! The framework carries no trace of the app it came from: "rogg" may only appear under docs/history (and, until it is
//! removed at the end of the extraction, the one-off tools/port).
use std::path::Path;

fn scan(dir: &Path, hits: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if matches!(name.as_str(), "target" | "node_modules" | ".git" | "history" | "port" | "golden") {
            continue;
        }
        if path.is_dir() {
            scan(&path, hits);
        } else if let Ok(text) = std::fs::read_to_string(&path) {
            if name.to_lowercase().contains("rogg") {
                hits.push(format!("file name: {}", path.display()));
            }
            for (n, line) in text.lines().enumerate() {
                // The guard's own pattern is spelled in pieces so this file does not match itself.
                if line.to_lowercase().contains(concat!("ro", "gg")) {
                    hits.push(format!("{}:{}", path.display(), n + 1));
                }
            }
        }
    }
}

#[test]
fn no_trace_of_the_source_app() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut hits = Vec::new();
    for dir in ["crates", "tools", "docs", "dev", "justfile", "README.md", "Cargo.toml"] {
        let path = root.join(dir);
        if path.is_file() {
            let text = std::fs::read_to_string(&path).unwrap();
            if text.to_lowercase().contains(concat!("ro", "gg")) {
                hits.push(path.display().to_string());
            }
        } else {
            scan(&path, &mut hits);
        }
    }
    assert!(hits.is_empty(), "traces of the source app: {hits:#?}");
}
```

`docs/spec/README.md`:

```markdown
# Specs

- `design-system.md` — the framework's binding design spec (written in Task 13).
- The extraction record (how this framework was cut out of its source app) is in `../history/2026-10-08-gpui-framework-extraction-design.md`.
```

The library unit test, at the end of `src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn version_is_the_package_version() {
        assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
    }
}
```

- [ ] **Step 4: The library root**

`crates/gpui-framework/src/lib.rs`:

```rust
//! gpui-framework: a desktop GUI framework on gpui-kit. Theme and design tokens, components, chrome and widgets.
//! Import it as `use gpui_framework as gf;`.

// The Action derive emits `gpui::` paths; gpui-kit documents aliasing itself as `gpui` for that.
extern crate gpui_kit as gpui;

/// The gpui-kit version the framework is built on. Apps use this instead of their own `gpui-kit` dependency, so the
/// versions can never drift.
pub use gpui_kit as kit;

use gpui_kit::{App, Window};
use std::rc::Rc;

/// Callback with no payload, used by every component.
pub type Callback = Rc<dyn Fn(&mut Window, &mut App)>;
/// Callback carrying an index (tab, chip, menu item...).
pub type IndexCallback = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Crate version (shown in the gallery title).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
```

- [ ] **Step 5: The port tool**

`tools/port/port.py`:

```python
#!/usr/bin/env python3
"""One-off helper for the extraction: copy a file from the source app into gpui-framework, applying the rename map.

Usage: tools/port/port.py SRC DST [--lib]
  --lib   DST is inside crates/gpui-framework/src (crate paths become `crate::`); otherwise `gpui_framework::`.
Removed at the end of the extraction (Task 13).
"""
import pathlib
import re
import sys

SRC_CRATE = "ro" + "gg_ui"
SRC_GALLERY = "ro" + "gg_gallery"
SRC_NAME = "Ro" + "gg"

COMMON = [
    (rf"\bActive{SRC_NAME}Theme\b", "ActiveTheme"),
    (rf"\b{SRC_NAME}Theme\b", "Theme"),
    (r"\.ro" + r"gg\(\)", ".gf_theme()"),
    (r"icons/ro" + r"gg/", "icons/gf/"),
    (r"\bLogViewElement\b", "TextViewElement"),
    (r"\bLogViewState\b", "TextViewState"),
    (r"\bLogViewEvent\b", "TextViewEvent"),
    (r"\bLogView\b", "TextView"),
    (r"\bLogRowSource\b", "RowSource"),
    (r"\bLogLine\b", "Line"),
    (r"\bLogRow\b", "Row"),
    (r"\bFilterBand\b", "ChipBand"),
    (r"\bFilterChip\b", "Chip"),
    (r"SpanStyle::Amber\b", "SpanStyle::Warning"),
    (r"SpanStyle::Hit\b", "SpanStyle::Highlight"),
    (r"SpanStyle::SearchHit\b", "SpanStyle::Outline"),
    (r"\bPROFILE_DIALOG_W\b", "WIDE_DIALOG_W"),
    (r"\bFULL_LINE_DIALOG_W\b", "TEXT_VIEWER_DIALOG_W"),
    (r"\bFULL_LINE_TEXT_H\b", "TEXT_VIEWER_TEXT_H"),
    (r"\bFULL_LINE_SHOWN_BYTES\b", "TEXT_VIEWER_SHOWN_BYTES"),
    (r"\bfull_line_shown\b", "text_viewer_shown"),
    (rf"\b(crate|{SRC_CRATE})::logview\b", r"\1::widgets::text_view"),
    (rf"\b(crate|{SRC_CRATE})::filter\b", r"\1::widgets::chip_band"),
    (rf"\b(crate|{SRC_CRATE})::titlebar\b", r"\1::chrome::title_bar"),
    (rf"\b(crate|{SRC_CRATE})::sidebar\b", r"\1::chrome::sidebar"),
    (r'"LogView"', '"TextView"'),
    (rf"\b{SRC_GALLERY}::", "gpui_framework_gallery::"),
    (r"\bRO" + r"GG_GALLERY_SMOKE\b", "GF_GALLERY_SMOKE"),
]


def port(text: str, lib: bool) -> str:
    for pattern, repl in COMMON:
        text = re.sub(pattern, repl, text)
    text = re.sub(rf"\b{SRC_CRATE}::", "crate::" if lib else "gpui_framework::", text)
    text = re.sub(rf"\b{SRC_CRATE}\b", "gpui_framework", text)
    return text


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if len(args) != 2:
        sys.exit(__doc__)
    src, dst = map(pathlib.Path, args)
    dst.parent.mkdir(parents=True, exist_ok=True)
    dst.write_text(port(src.read_text(), "--lib" in sys.argv))
    left = [n + 1 for n, line in enumerate(dst.read_text().splitlines()) if ("ro" + "gg") in line.lower()]
    if left:
        print(f"{dst}: lines still mentioning the source app (hand-edit them): {left}")


if __name__ == "__main__":
    main()
```

Make it executable with `chmod +x tools/port/port.py`.

- [ ] **Step 6: `./dev` and `justfile`**

Copy `rogg:dev` to `dev` and adapt it. Keep `step/ok/die/run/run_visual/confirm/open_report/toolchain/need_rust/need_node` and `cmd_check` (the loop over `build lint test` plus `visual-check`) verbatim, then:
- Set `CACHE_DIR` to nothing and remove `cmd_clean`'s `--cache` branch. `clean` just runs `cargo clean`.
- Remove these commands and their help lines: `run`, `demo`, `bench`, `visual`, `visual-app`, `genlog`, `bundle`, `live`, `pipe`.
- Keep `help`, `setup`, `build`, `test`, `lint`, `fmt`, `check`, `clean` in this task.
- Keep `gallery`, `smoke`, `capture`, `template`, `visual-check` and `visual-approve` in the help text, marked `(Task 10/11/12)`. Their functions print `die "not available yet"` until the owning task wires them.
- `cmd_test` crate shortcuts: `framework|gallery|template` map to `-p gpui-framework`, `-p gpui-framework-gallery` and `-p gpui-framework-template`.
- The help title is `gpui-framework developer commands`.
- Until Task 12, `cmd_check` must run with `--no-visual` by default: `local visual=0` unless `tools/visual-diff/golden/tolerance.json` exists.

`justfile`: copy `rogg:justfile` and drop the recipes for removed commands. Each remaining recipe is `@./dev <cmd> "$@"`.

- [ ] **Step 7: Run the tests to verify they fail, then pass**

Run `./dev setup` first. Its npm steps may fail because `tools/visual-diff` doesn't exist yet. Make `cmd_setup` skip the visual-diff part when `tools/visual-diff/package.json` is missing, printing `(visual-diff not set up yet)`.

Then run `./dev test framework`. The expected first failure is `no_trace_of_the_source_app`, if any copied text still has traces: the `dev` script copied from rogg has comments mentioning it. Fix the `dev`/`justfile` texts until it passes.

Then `./dev check` (no visual yet). Expected: build ✓ lint ✓ test ✓.

- [ ] **Step 8: README skeleton**

`README.md`:

````markdown
# gpui-framework

A desktop GUI framework for macOS apps on [gpui](https://www.gpui.rs) via gpui-kit: a design system (theme, tokens, icons),
components (controls, forms, dialogs, toasts, empty states, window chrome and layout) and two widgets (a virtualised text
view and a chip band), with a component gallery, a starter template and golden-image visual regression.

```rust
use gpui_framework as gf;
```

Status: extraction in progress — see `docs/history/2026-10-08-gpui-framework-extraction-design.md`.

## Developer commands

`./dev help` lists everything; `./dev check` is the gate (build, clippy, tests, visual check).
````

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "build: scaffold gpui-framework workspace, ./dev, port tool and the no-trace guard

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

# Stage 2 — Theme, icons, controls

### Task 2: Theme, `ThemeBuilder`, icons and assets

**Files:**
- Create via the port tool, with `--lib`, from `rogg:crates/rogg-ui/src/`:
  - `crates/gpui-framework/src/theme/{mod,color,metrics,palette,kit}.rs`;
  - `crates/gpui-framework/src/{icons,assets}.rs`;
  - `assets/icons/gf/*.svg`, copied as is.
- Create: `crates/gpui-framework/src/theme/builder.rs`.
- Tests:
  - port `rogg:crates/rogg-ui/tests/{theme_tokens,theme_install,assets}.rs` and `common/mod.rs` to `crates/gpui-framework/tests/`;
  - port `glyphs.rs`, changing its scan roots to `src` and `../gpui-framework-gallery/src`;
  - add the new `crates/gpui-framework/tests/theme_builder.rs`.
- Modify: `crates/gpui-framework/src/lib.rs`, adding the modules and re-exports.

**Interfaces:**
- Produces:
  - `gf::theme::{Theme, ThemeBuilder, ActiveTheme, Palette, ChipColor, HitColor, Color, Metrics, Density, size, Fonts, TextScale, Appearance, UI_FONT, MONO_FONT, BASE_UI_FONT_SIZE}`;
  - `Theme::{new(Appearance, Density), default_dark(), default_light(), with_log_font, with_ui_font_size, text_scale, install}`;
  - `Color::mix(self, Color, f32) -> Color`;
  - the `ThemeBuilder` API below;
  - `gf::icons::{IconName, icon, chevron}`;
  - `gf::assets::{Assets, WithApp}`, with `Assets::with(app)`;
  - the re-exports `gf::{Theme, ActiveTheme}`.

- [ ] **Step 1: Port the files**

```bash
R=/Users/vadim/dev/projects/rogg2/crates/rogg-ui
for f in mod color metrics palette kit; do tools/port/port.py $R/src/theme/$f.rs crates/gpui-framework/src/theme/$f.rs --lib; done
for f in icons assets; do tools/port/port.py $R/src/$f.rs crates/gpui-framework/src/$f.rs --lib; done
mkdir -p crates/gpui-framework/assets/icons/gf && cp $R/assets/icons/*/*.svg crates/gpui-framework/assets/icons/gf/
mkdir -p crates/gpui-framework/tests/common
tools/port/port.py $R/tests/common/mod.rs crates/gpui-framework/tests/common/mod.rs
for f in theme_tokens theme_install assets glyphs; do tools/port/port.py $R/tests/$f.rs crates/gpui-framework/tests/$f.rs; done
```

Hand edits:
- `assets.rs`:
  - the doc comment becomes "The framework's icons, then gpui-kit's default icons";
  - `if !path.starts_with("icons/gf")`;
  - the asset folder path stays `assets`.
- `theme/mod.rs`:
  - add `mod builder; pub use builder::ThemeBuilder;`;
  - rename the remaining doc mentions of the source app ("reference" stays);
  - add `default_dark` and `default_light` (Step 3);
  - replace `ActiveTheme::gf_theme`'s body (Step 3).
- `lib.rs`: add `pub mod theme; pub mod assets; pub mod icons; pub use theme::{ActiveTheme, Theme};`.
- `tests/glyphs.rs`: scan `src` and `../gpui-framework-gallery/src` (missing directories are skipped).
- Everywhere: replace any remaining rogg wording in comments; the port tool lists the lines.

- [ ] **Step 2: Write the failing tests**

`crates/gpui-framework/tests/theme_builder.rs`:

```rust
mod common;
use gpui_framework::kit::{AssetSource, SharedString, TestAppContext};
use gpui_framework::theme::{ActiveTheme, Appearance, Color, Density, Palette, Theme, ThemeBuilder};
use std::borrow::Cow;

#[test]
fn defaults_are_the_built_in_look() {
    assert_eq!(Theme::default_dark(), Theme::new(Appearance::Dark, Density::Compact));
    assert_eq!(Theme::default_light(), Theme::new(Appearance::Light, Density::Compact));
    assert_eq!(ThemeBuilder::new().build(), Theme::default_dark(), "a builder with no overrides changes nothing");
}

#[test]
fn accent_override_sets_the_accent_and_derives_its_roles() {
    let accent = Color::rgb(0x4f7cff);
    let t = ThemeBuilder::new().accent(accent).build();
    let bg = Theme::default_dark().palette.bg;
    assert_eq!(t.palette.accent, accent);
    assert_eq!(t.palette.accent_bg, bg.mix(accent, gpui_framework::theme::ACCENT_BG_MIX));
    assert_eq!(t.palette.accent_border, bg.mix(accent, gpui_framework::theme::ACCENT_BORDER_MIX));
    assert_eq!(t.palette.text, Theme::default_dark().palette.text, "other roles are untouched");
}

#[test]
fn palette_fonts_and_sizes_override() {
    let t = ThemeBuilder::new()
        .palette(|p: &mut Palette| p.error = Color::rgb(0xff0000))
        .ui_font("Inter")
        .mono_font("JetBrains Mono")
        .log_font("Menlo", 13.)
        .ui_font_size(15.)
        .density(Density::Comfortable)
        .build();
    assert_eq!(t.palette.error, Color::rgb(0xff0000));
    assert_eq!((t.fonts.ui.as_ref(), t.fonts.mono.as_ref(), t.fonts.log.as_ref()), ("Inter", "JetBrains Mono", "Menlo"));
    assert_eq!(t.metrics, Density::Comfortable.metrics_for(13.));
    assert_eq!(f32::from(t.fonts.ui_size), 15.);
}

#[test]
fn overrides_survive_an_appearance_switch() {
    let builder = ThemeBuilder::new().accent(Color::rgb(0x4f7cff)).palette(|p: &mut Palette| p.error = Color::rgb(0xff0000)).ui_font("Inter");
    let light = builder.clone().appearance(Appearance::Light).build();
    assert_eq!(light.appearance, Appearance::Light);
    assert_eq!(light.palette.accent, Color::rgb(0x4f7cff));
    assert_eq!(light.palette.error, Color::rgb(0xff0000));
    assert_eq!(light.fonts.ui.as_ref(), "Inter");
    assert_eq!(light.palette.bg, Theme::default_light().palette.bg, "the base palette follows the appearance");
}

#[test]
fn mix_blends_each_channel() {
    assert_eq!(Color::rgb(0x000000).mix(Color::rgb(0xffffff), 0.5), Color::rgb(0x808080));
    assert_eq!(Color::rgb(0x102030).mix(Color::rgb(0x405060), 0.), Color::rgb(0x102030));
    assert_eq!(Color::rgb(0x102030).mix(Color::rgb(0x405060), 1.), Color::rgb(0x405060));
    assert_eq!(Color::rgba(0x00000000).mix(Color::rgba(0x000000ff), 0.5), Color::rgba(0x00000080));
}

#[gpui_framework::kit::test]
fn builder_install_makes_the_theme_active(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_framework::kit::init(cx);
        ThemeBuilder::new().accent(Color::rgb(0x4f7cff)).install(cx);
        assert_eq!(cx.gf_theme().palette.accent, Color::rgb(0x4f7cff));
    });
}

// If the gpui test macro rejects `#[should_panic]`, write this as a plain `#[test]` that runs the same body inside
// `std::panic::catch_unwind` (with a `TestAppContext::single()`), and assert the panic message contains "Theme::install".
#[gpui_framework::kit::test]
#[should_panic(expected = "Theme::install")]
fn gf_theme_without_install_panics_with_a_hint(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let _ = cx.gf_theme();
    });
}

struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui_framework::kit::Result<Option<Cow<'static, [u8]>>> {
        Ok((path == "app/logo.svg").then(|| Cow::Borrowed(&b"<svg/>"[..])))
    }
    fn list(&self, _: &str) -> gpui_framework::kit::Result<Vec<SharedString>> {
        Ok(vec!["app/logo.svg".into()])
    }
}

#[test]
fn app_assets_are_tried_before_framework_icons() {
    let assets = gpui_framework::assets::Assets::with(AppAssets);
    assert_eq!(assets.load("app/logo.svg").unwrap().as_deref(), Some(&b"<svg/>"[..]));
    assert!(assets.load("icons/gf/file.svg").unwrap().is_some(), "framework icons still load");
    let listed = assets.list("").unwrap();
    assert!(listed.iter().any(|p| p.as_ref() == "app/logo.svg") && listed.iter().any(|p| p.as_ref() == "icons/gf/file.svg"));
}
```

- [ ] **Step 3: Run them to verify they fail**

Run: `./dev test framework theme_builder`
Expected: compile errors (no `ThemeBuilder`, `Color::mix`, `default_dark`, `ACCENT_BG_MIX` or `Assets::with`).

- [ ] **Step 4: Implement**

`theme/color.rs`: add to `impl Color`:

```rust
    /// Linear blend per channel (alpha included): `t = 0` is `self`, `t = 1` is `other`.
    pub fn mix(self, other: Color, t: f32) -> Color {
        let t = t.clamp(0., 1.);
        let ch = |shift: u32| {
            let a = ((self.0 >> shift) & 0xff) as f32;
            let b = ((other.0 >> shift) & 0xff) as f32;
            ((a + (b - a) * t).round() as u32) << shift
        };
        Color(ch(24) | ch(16) | ch(8) | ch(0))
    }
```

`theme/mod.rs`: add to `impl Theme`:

```rust
    /// The built-in dark theme, compact density.
    pub fn default_dark() -> Self {
        Self::new(Appearance::Dark, Density::Compact)
    }

    /// The built-in light theme, compact density.
    pub fn default_light() -> Self {
        Self::new(Appearance::Light, Density::Compact)
    }
```

and replace the accessor:

```rust
pub trait ActiveTheme {
    /// The active framework theme. Panics with a hint when no theme was installed.
    fn gf_theme(&self) -> &Theme;
}

impl ActiveTheme for App {
    fn gf_theme(&self) -> &Theme {
        self.try_global::<Theme>().expect("gpui-framework theme not installed: call Theme::install(cx) or ThemeBuilder::install(cx) after gpui_kit::init(cx)")
    }
}
```

`theme/builder.rs`:

```rust
//! App overrides on top of the built-in theme.

use super::{Appearance, Color, Density, Palette, Theme};
use gpui_kit::{App, SharedString};
use std::rc::Rc;

/// How far an overridden accent's background role sits from the page background toward the accent.
pub const ACCENT_BG_MIX: f32 = 0.2;
/// How far an overridden accent's border role sits from the page background toward the accent.
pub const ACCENT_BORDER_MIX: f32 = 0.45;

/// Builds a [`Theme`] from the built-in look plus app overrides. Overrides survive a change of appearance or density:
/// keep the builder and call `builder.clone().appearance(Appearance::Light).build()`.
#[derive(Clone)]
pub struct ThemeBuilder {
    appearance: Appearance,
    density: Density,
    accent: Option<Color>,
    palette_edits: Vec<Rc<dyn Fn(&mut Palette)>>,
    ui_font: Option<SharedString>,
    mono_font: Option<SharedString>,
    log_font: Option<(SharedString, f32)>,
    ui_font_size: Option<f32>,
}

impl Default for ThemeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeBuilder {
    /// Dark, compact, no overrides.
    pub fn new() -> Self {
        Self {
            appearance: Appearance::Dark,
            density: Density::Compact,
            accent: None,
            palette_edits: Vec::new(),
            ui_font: None,
            mono_font: None,
            log_font: None,
            ui_font_size: None,
        }
    }

    pub fn appearance(mut self, appearance: Appearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    /// The accent colour. `accent_bg` and `accent_border` are derived from it and the page background.
    pub fn accent(mut self, accent: Color) -> Self {
        self.accent = Some(accent);
        self
    }

    /// Edits any palette role. Edits run after the accent derivation, in the order given.
    pub fn palette(mut self, edit: impl Fn(&mut Palette) + 'static) -> Self {
        self.palette_edits.push(Rc::new(edit));
        self
    }

    pub fn ui_font(mut self, family: impl Into<SharedString>) -> Self {
        self.ui_font = Some(family.into());
        self
    }

    /// Chips, dialogs and the text view's number gutter.
    pub fn mono_font(mut self, family: impl Into<SharedString>) -> Self {
        self.mono_font = Some(family.into());
        self
    }

    /// The text view's line font and its compact size (comfortable adds 1 px, row heights follow).
    pub fn log_font(mut self, family: impl Into<SharedString>, compact_size: f32) -> Self {
        self.log_font = Some((family.into(), compact_size));
        self
    }

    /// The base UI font size (13 px by default); UI text scales with it, bar and row heights do not.
    pub fn ui_font_size(mut self, size: f32) -> Self {
        self.ui_font_size = Some(size);
        self
    }

    pub fn build(&self) -> Theme {
        let mut t = Theme::new(self.appearance, self.density);
        if let Some(accent) = self.accent {
            let bg = t.palette.bg;
            t.palette.accent = accent;
            t.palette.accent_bg = bg.mix(accent, ACCENT_BG_MIX);
            t.palette.accent_border = bg.mix(accent, ACCENT_BORDER_MIX);
        }
        for edit in &self.palette_edits {
            edit(&mut t.palette);
        }
        if let Some(family) = &self.ui_font {
            t.fonts.ui = family.clone();
        }
        if let Some(family) = &self.mono_font {
            t.fonts.mono = family.clone();
        }
        if let Some((family, size)) = &self.log_font {
            t = t.with_log_font(family.clone(), *size);
        }
        if let Some(size) = self.ui_font_size {
            t = t.with_ui_font_size(size);
        }
        t
    }

    /// Builds and installs the theme (mirrored into gpui-kit). Call after `gpui_kit::init(cx)`.
    pub fn install(&self, cx: &mut App) {
        self.build().install(cx);
    }
}
```

`theme/mod.rs` also re-exports the constants: `pub use builder::{ACCENT_BG_MIX, ACCENT_BORDER_MIX, ThemeBuilder};`.

`assets.rs`: add after `Assets`:

```rust
impl Assets {
    /// The app's assets first, then the framework's icons, then gpui-kit's: `application().with_assets(Assets::with(MyAssets))`.
    pub fn with<A: AssetSource>(app: A) -> WithApp<A> {
        WithApp(app)
    }
}

/// An app asset source chained in front of [`Assets`].
pub struct WithApp<A>(pub A);

impl<A: AssetSource> AssetSource for WithApp<A> {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Ok(Some(data)) = self.0.load(path) {
            return Ok(Some(data));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out = self.0.list(path).unwrap_or_default();
        out.extend(Assets.list(path)?);
        out.sort();
        out.dedup();
        Ok(out)
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `./dev test framework`
Expected: PASS for `theme_builder`, `theme_tokens`, `theme_install`, `assets`, `glyphs`, `no_rogg`, and the unit tests in `theme/` (ported from rogg).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: theme with ThemeBuilder, icons and chained assets

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 3: Controls

**Files:**
- Create via the port tool (`--lib`): `crates/gpui-framework/src/controls/{mod,buttons,find_field,segmented,select}.rs`, from `rogg:crates/rogg-ui/src/controls/`.
- Test: port `rogg:crates/rogg-ui/tests/controls.rs` to `crates/gpui-framework/tests/controls.rs`.
- Modify: `lib.rs` (`pub mod controls;`).

**Interfaces:**
- Produces: `gf::controls::{IconButton (icon, glyph, chevron_down, compact, force_hover, on_click), FlagButton, ToolButton, TextButton (meta, small, icon, dot), ActionButton (new, primary, on_click), Segmented, CompactSelect, FindField}`, with signatures unchanged from rogg.

- [ ] **Step 1: Port and fix imports**

```bash
R=/Users/vadim/dev/projects/rogg2/crates/rogg-ui
for f in mod buttons find_field segmented select; do tools/port/port.py $R/src/controls/$f.rs crates/gpui-framework/src/controls/$f.rs --lib; done
tools/port/port.py $R/tests/controls.rs crates/gpui-framework/tests/controls.rs
```

Add `pub mod controls;` to `lib.rs`. Hand edits: none beyond comment wording.

- [ ] **Step 2: Run the tests**

Run: `./dev test framework controls`
Expected: PASS. This includes `icon_buttons_use_reference_box_sizes` with its `chevron-open` assertion.

- [ ] **Step 3: Gate and commit**

Run `./dev check`. Expected: all green.

```bash
git add -A
git commit -m "feat: controls (buttons, segmented, select, find field)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

# Stage 3 — Forms, toasts, empty states, dialogs

### Task 4: Form controls, toasts and generic empty states

**Files:**
- Create via the port tool (`--lib`): `crates/gpui-framework/src/{form,toast,empty}.rs`.
- Test:
  - port `rogg:crates/rogg-ui/tests/form.rs` to `crates/gpui-framework/tests/form.rs`;
  - add the new `crates/gpui-framework/tests/empty.rs`.
- Modify: `lib.rs`.

**Interfaces:**
- Consumes: `gf::controls::ActionButton`, `gf::icons`, `gf::theme`.
- Produces:
  - `gf::form::{FormRow, SectionLabel, Toggle, Stepper, Swatch, TextField, CustomColor, pen_ink, PEN_DARK, PEN_LIGHT}`;
  - `gf::toast::ToastStack`;
  - `gf::empty::{EmptyState, loading, error, welcome, WelcomeAction, RecentItem, with_recent_list, RecentEntry}`.

- [ ] **Step 1: Port**

```bash
R=/Users/vadim/dev/projects/rogg2/crates/rogg-ui
for f in form toast empty; do tools/port/port.py $R/src/$f.rs crates/gpui-framework/src/$f.rs --lib; done
tools/port/port.py $R/tests/form.rs crates/gpui-framework/tests/form.rs
```

`RecentItem` moves into `empty.rs`, because `chrome::sidebar` will import it from there. Copy the struct from `rogg:crates/rogg-ui/src/sidebar.rs`:

```rust
/// A recent file or folder (sidebar "Recent", welcome list); `missing` greys it out.
#[derive(Clone, Debug, PartialEq)]
pub struct RecentItem {
    pub label: SharedString,
    pub is_folder: bool,
    pub missing: bool,
}
```

Hand edits in `form.rs`:
- Delete `EditableRuleRow`, `PaletteSlot` and their imports.
- In `tests/form.rs`, delete the tests that use those two types (look for `EditableRuleRow` / `PaletteSlot` / `profile-rule` / `palette-slot`).

Hand edits in `empty.rs`, replacing the domain builders:
- Delete `no_matches`, `RecentFilter` and `no_matches_with_recent`.
- Replace `welcome` with the code below.
- Keep `EmptyState`, `loading`, `error` and `LoadingBar`.

```rust
/// A button on the welcome screen.
pub struct WelcomeAction {
    pub id: SharedString,
    pub label: SharedString,
    pub primary: bool,
    pub on_click: Callback,
}

/// A welcome screen: title, body, action buttons and an optional recent list.
pub fn welcome(title: impl Into<SharedString>, body: impl Into<SharedString>, actions: Vec<WelcomeAction>, recent: Vec<RecentItem>, on_recent: IndexCallback) -> EmptyState {
    let buttons = div().flex().gap(px(7.)).children(actions.into_iter().map(|a| {
        let button = ActionButton::new(a.id, a.label);
        let button = if a.primary { button.primary() } else { button };
        button.on_click(call(a.on_click))
    }));
    let mut state = EmptyState::new("welcome", title).body(body).child(buttons);
    if !recent.is_empty() {
        state = state.child(WelcomeRecent { items: recent, on_recent });
    }
    state
}

/// An entry of a recent list under an empty state: a mono first line and a muted summary.
#[derive(Clone, Debug, PartialEq)]
pub struct RecentEntry {
    pub primary: SharedString,
    pub secondary: SharedString,
}

/// `state` followed by a titled list of entries (e.g. recent searches under "No matches"); a click calls `on_pick`.
/// Without entries, `state` is returned unchanged.
pub fn with_recent_list(state: EmptyState, title: impl Into<SharedString>, entries: Vec<RecentEntry>, on_pick: IndexCallback) -> EmptyState {
    if entries.is_empty() {
        return state;
    }
    state.child(RecentList { title: title.into(), items: entries, on_pick })
}
```

`RecentList` is rogg's `RecentFilters` element, renamed:
- `items: Vec<RecentEntry>`, `primary`/`secondary` instead of `expression`/`summary`;
- the heading text is `self.title`, not "Recent filters";
- element ids `recent-list` and `recent-entry-{i}`.

Keep its height handling (scroll when it doesn't fit) as rogg has it.

- [ ] **Step 2: Write the failing tests**

`crates/gpui-framework/tests/empty.rs`:

```rust
mod common;
use gpui_framework::empty::{EmptyState, RecentEntry, RecentItem, WelcomeAction, error, loading, welcome, with_recent_list};
use gpui_framework::kit::prelude::*;
use gpui_framework::kit::TestAppContext;
use std::cell::Cell;
use std::rc::Rc;

#[gpui_framework::kit::test]
fn welcome_renders_the_apps_actions_and_recent_items(cx: &mut TestAppContext) {
    common::setup(cx);
    let clicked = Rc::new(Cell::new(false));
    let c = clicked.clone();
    let w = common::open(cx, 800., 600., move |_, _| {
        let c = c.clone();
        let actions = vec![
            WelcomeAction { id: "new-doc".into(), label: "New document".into(), primary: true, on_click: Rc::new(move |_, _| c.set(true)) },
            WelcomeAction { id: "open-doc".into(), label: "Open…".into(), primary: false, on_click: Rc::new(|_, _| {}) },
        ];
        let recent = vec![RecentItem { label: "notes.md".into(), is_folder: false, missing: false }];
        welcome("Start", "Pick something", actions, recent, Rc::new(|_, _, _| {})).into_any_element()
    });
    assert!(common::exists(cx, w, "new-doc") && common::exists(cx, w, "open-doc") && common::exists(cx, w, "welcome-recent-0"));
    common::click(cx, w, "new-doc");
    assert!(clicked.get());
}

#[gpui_framework::kit::test]
fn recent_list_shows_entries_and_reports_a_pick(cx: &mut TestAppContext) {
    common::setup(cx);
    let picked = Rc::new(Cell::new(None));
    let p = picked.clone();
    let w = common::open(cx, 600., 400., move |_, _| {
        let p = p.clone();
        let entries = vec![RecentEntry { primary: "a || b".into(), secondary: "2 include".into() }, RecentEntry { primary: "c".into(), secondary: "1 include".into() }];
        with_recent_list(EmptyState::new("empty", "No matches"), "Recent searches", entries, Rc::new(move |i, _, _| p.set(Some(i)))).into_any_element()
    });
    assert!(common::exists(cx, w, "recent-list") && common::exists(cx, w, "recent-entry-1"));
    common::click(cx, w, "recent-entry-1");
    assert_eq!(picked.get(), Some(1));
}

#[gpui_framework::kit::test]
fn an_empty_recent_list_leaves_the_state_unchanged(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 600., 400., |_, _| with_recent_list(EmptyState::new("empty", "No matches"), "Recent", vec![], Rc::new(|_, _, _| {})).into_any_element());
    assert!(common::exists(cx, w, "empty") && !common::exists(cx, w, "recent-list"));
}

#[gpui_framework::kit::test]
fn loading_and_error_states_render(cx: &mut TestAppContext) {
    common::setup(cx);
    let a = common::open(cx, 600., 300., |_, _| loading("Loading 40%", Some(0.4), Rc::new(|_, _| {})).into_any_element());
    assert!(common::exists(cx, a, "loading-fill") && common::exists(cx, a, "cancel"));
    let b = common::open(cx, 600., 300., |_, _| error("Could not open", "/tmp/x", Rc::new(|_, _| {})).into_any_element());
    assert!(common::exists(cx, b, "retry"));
}
```

- [ ] **Step 3: Run them to verify they fail, then implement, then pass**

Run: `./dev test framework empty`. Expected first: compile errors (`welcome` signature, `with_recent_list`). After Step 1's hand edits, run `./dev test framework` and expect PASS.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: form controls, toasts and generic empty states

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 5: Dialogs: frame, rows and generic builders

**Files:**
- Create via the port tool (`--lib`): `crates/gpui-framework/src/dialogs.rs`.
- Test: port `rogg:crates/rogg-ui/tests/dialogs.rs` to `crates/gpui-framework/tests/dialogs.rs`, adapted.
- Modify: `lib.rs`; `theme/metrics.rs`, whose renamed size constants come from Task 2's port.

**Interfaces:**
- Produces:
  - `gf::dialogs::{DialogFrame (new, description, child, note, width, on_close), ModalLayer, RuleRow, ShortcutRow, PickerItem}`;
  - `picker(title, &[(SharedString, bool)], IndexCallback) -> DialogFrame`;
  - `key_value_list(title, &[(SharedString, SharedString)]) -> DialogFrame`;
  - `shortcuts(title, &[(SharedString, SharedString)]) -> DialogFrame`;
  - `about(&AboutView, IndexCallback) -> DialogFrame`;
  - `text_viewer(title, Entity<TextareaState>, Option<SharedString>, Callback) -> DialogFrame`;
  - `text_viewer_shown(&str) -> (&str, bool)`;
  - `input_prompt(title, Entity<InputState>) -> DialogFrame`;
  - `AboutView`, `TEXT_VIEWER_SHOWN_BYTES`.

- [ ] **Step 1: Port**

```bash
R=/Users/vadim/dev/projects/rogg2/crates/rogg-ui
tools/port/port.py $R/src/dialogs.rs crates/gpui-framework/src/dialogs.rs --lib
tools/port/port.py $R/tests/dialogs.rs crates/gpui-framework/tests/dialogs.rs
```

- [ ] **Step 2: Replace the domain builders**

In `dialogs.rs`:
- Delete `ProfileRuleView`, `profile`, `appearance`, `history`, `history_picker`, `statistics`, `export`, `all_tabs`, `go_to_line` and `full_line`, and add the generic builders below.
- Keep `DialogFrame`, `ModalLayer`, `RuleRow`, `ShortcutRow`, `PickerItem`, `picker`, `AboutView`, `about`, `AboutHeader`, `AboutNote`, `DataRow`, `FullLineText` (renamed `ViewerText`) and `GoToLineField` (renamed `PromptField`).
- The constant `TEXT_VIEWER_SHOWN_BYTES` and `text_viewer_shown` come from the port's rename.

```rust
/// A titled list of label/value rows (statistics, settings summaries, …).
pub fn key_value_list(title: impl Into<SharedString>, rows: &[(SharedString, SharedString)]) -> DialogFrame {
    rows.iter().enumerate().fold(DialogFrame::new(title), |d, (i, (k, v))| d.child(RuleRow::text(format!("kv-{i}"), k.clone()).trailing(v.clone())))
}

/// A titled keyboard-shortcut list: label left, monospace keys right.
pub fn shortcuts(title: impl Into<SharedString>, rows: &[(SharedString, SharedString)]) -> DialogFrame {
    rows.iter().enumerate().fold(DialogFrame::new(title), |d, (i, (label, keys))| {
        d.child(ShortcutRow { id: format!("shortcut-row-{i}").into(), label: label.clone(), keys: keys.clone() })
    })
}

/// A read-only, selectable, wrapped text in the log font with Copy (e.g. a whole long line). `cut_note` is shown when
/// only part of the text is displayed (see [`text_viewer_shown`]).
pub fn text_viewer(title: impl Into<SharedString>, view: Entity<TextareaState>, cut_note: Option<SharedString>, on_copy: Callback) -> DialogFrame {
    let mut frame = DialogFrame::new(title).width(size::TEXT_VIEWER_DIALOG_W).child(ViewerText { view });
    if let Some(note) = cut_note {
        frame = frame.child(AboutNote(note));
    }
    frame.child(div().flex().justify_end().pt(px(14.)).child(ActionButton::new("viewer-copy", "Copy").primary().on_click(move |w, cx| on_copy(w, cx))))
}

/// A titled single-line input (e.g. "Go to line").
pub fn input_prompt(title: impl Into<SharedString>, input: Entity<InputState>) -> DialogFrame {
    DialogFrame::new(title).child(PromptField { input })
}
```

Element id renames: `full-line-text` becomes `viewer-text`, and `full-line-copy` becomes `viewer-copy`.

- [ ] **Step 3: Adapt the tests, then write the failing ones**

In `tests/dialogs.rs`, delete the tests for the removed builders and keep the frame, rows, picker and about tests. Then append:

```rust
#[gpui_framework::kit::test]
fn generic_builders_accept_empty_input(cx: &mut TestAppContext) {
    use gpui_framework::dialogs::{key_value_list, picker, shortcuts};
    common::setup(cx);
    let a = common::open(cx, 900., 700., |_, _| picker("Choose", &[], Rc::new(|_, _, _| {})).into_any_element());
    assert!(common::exists(cx, a, "dialog"));
    let b = common::open(cx, 900., 700., |_, _| key_value_list("Stats", &[]).into_any_element());
    assert!(common::exists(cx, b, "dialog"));
    let c = common::open(cx, 900., 700., |_, _| shortcuts("Keys", &[]).into_any_element());
    assert!(common::exists(cx, c, "dialog"));
}

#[gpui_framework::kit::test]
fn key_value_list_and_shortcuts_render_their_rows(cx: &mut TestAppContext) {
    use gpui_framework::dialogs::{key_value_list, shortcuts};
    common::setup(cx);
    let rows: Vec<(SharedString, SharedString)> = vec![("Matched".into(), "6 of 40".into()), ("Scan".into(), "4 ms".into())];
    let r = rows.clone();
    let a = common::open(cx, 900., 700., move |_, _| key_value_list("Stats", &r).into_any_element());
    assert!(common::exists(cx, a, "kv-0") && common::exists(cx, a, "kv-1"));
    let b = common::open(cx, 900., 700., move |_, _| shortcuts("Keys", &rows).into_any_element());
    assert!(common::exists(cx, b, "shortcut-row-1"));
}

#[gpui_framework::kit::test]
fn text_viewer_is_wide_and_copies(cx: &mut TestAppContext) {
    use gpui_framework::dialogs::text_viewer;
    use gpui_framework::kit::component::input::TextareaState;
    use gpui_framework::kit::{AppContext, Entity};
    use gpui_framework::theme::size;
    use std::cell::RefCell;
    common::setup(cx);
    let copied = Rc::new(Cell::new(false));
    let c = copied.clone();
    let slot: Rc<RefCell<Option<Entity<TextareaState>>>> = Rc::default();
    let w = common::open(cx, 1200., 900., move |window, cx| {
        let view = slot.borrow_mut().get_or_insert_with(|| cx.new(|cx| TextareaState::new(window, cx).default_value("hello"))).clone();
        let c = c.clone();
        text_viewer("Line 2", view, Some("cut".into()), Rc::new(move |_, _| c.set(true))).into_any_element()
    });
    assert_eq!(common::bounds(cx, w, "dialog").size.width, px(size::TEXT_VIEWER_DIALOG_W));
    assert!(common::exists(cx, w, "viewer-text"));
    common::click(cx, w, "viewer-copy");
    assert!(copied.get());
}

#[test]
fn the_text_viewer_shows_at_most_1_mb_on_a_char_boundary() {
    use gpui_framework::dialogs::{TEXT_VIEWER_SHOWN_BYTES, text_viewer_shown};
    assert_eq!(text_viewer_shown("short"), ("short", false));
    let long = "é".repeat(TEXT_VIEWER_SHOWN_BYTES);
    let (shown, cut) = text_viewer_shown(&long);
    assert!(cut && shown.len() <= TEXT_VIEWER_SHOWN_BYTES && long.is_char_boundary(shown.len()));
}
```

(Add `use gpui_framework::kit::{SharedString, px};` and `use std::cell::Cell;` if the ported file lacks them.)

- [ ] **Step 4: Run them, implement until they pass**

Run: `./dev test framework dialogs`. Expected first: failures for the new builders. Then PASS.

- [ ] **Step 5: Gate and commit**

Run `./dev check`.

```bash
git add -A
git commit -m "feat: dialogs with generic builders (picker, key-value list, shortcuts, about, text viewer, input prompt)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

# Stage 4 — Layout and chrome

### Task 6: Layout and window chrome

**Files:**
- Create via the port tool (`--lib`):
  - `crates/gpui-framework/src/layout.rs`;
  - `chrome/title_bar.rs` (from `titlebar.rs`);
  - `chrome/sidebar.rs` (from `sidebar.rs`);
  - `chrome/info_bar.rs`, `chrome/pane_header.rs` and `chrome/status_bar.rs`, split from `chrome.rs`.
- Create: `chrome/mod.rs`.
- Test: port `rogg:crates/rogg-ui/tests/{chrome,titlebar,sidebar,layout}.rs` (`titlebar.rs` becomes `title_bar.rs`).
- Modify: `lib.rs`.

**Interfaces:**
- Consumes: controls, `empty::RecentItem` (Task 4).
- Produces:
  - `gf::chrome::{TitleBar, TabItem, Sidebar, TreeRow, SidebarFooter, InfoBar, BarAction, PaneHeader, StatusBar, StatPart, Progress, bar_fraction}`, with modules `chrome::{title_bar, sidebar, info_bar, pane_header, status_bar}`;
  - `gf::layout::{MainWindowLayout, MainWindowParts, SingleParts, LayoutMeasure, BandDrag, SidebarDrag}`.

- [ ] **Step 1: Port and split**

```bash
R=/Users/vadim/dev/projects/rogg2/crates/rogg-ui
tools/port/port.py $R/src/layout.rs crates/gpui-framework/src/layout.rs --lib
tools/port/port.py $R/src/titlebar.rs crates/gpui-framework/src/chrome/title_bar.rs --lib
tools/port/port.py $R/src/sidebar.rs crates/gpui-framework/src/chrome/sidebar.rs --lib
tools/port/port.py $R/src/chrome.rs crates/gpui-framework/src/chrome/info_bar.rs --lib
for t in chrome sidebar layout; do tools/port/port.py $R/tests/$t.rs crates/gpui-framework/tests/$t.rs; done
tools/port/port.py $R/tests/titlebar.rs crates/gpui-framework/tests/title_bar.rs
```

Split `chrome/info_bar.rs` into three files:
- `info_bar.rs` keeps `InfoBar`;
- `pane_header.rs` gets `PaneHeader`;
- `status_bar.rs` gets `StatPart`, `Progress`, `StatusBar` and `bar_fraction`.

Each file keeps only the imports it needs.

`chrome/mod.rs`:

```rust
//! Window chrome: title bar with tabs, sidebar, info bar, pane header and status bar.
pub mod info_bar;
pub mod pane_header;
pub mod sidebar;
pub mod status_bar;
pub mod title_bar;

pub use info_bar::{BarAction, InfoBar};
pub use pane_header::PaneHeader;
pub use sidebar::{Sidebar, SidebarFooter, TreeRow};
pub use status_bar::{Progress, StatPart, StatusBar, bar_fraction};
pub use title_bar::{TabItem, TitleBar};
```

`chrome/sidebar.rs`: remove the `RecentItem` definition and use `crate::empty::RecentItem`. Re-export it as `pub use crate::empty::RecentItem;` so `chrome::sidebar::RecentItem` still resolves for tests.

The empty-state call `crate::chrome::bar_fraction` in `empty.rs` resolves through the re-export.

- [ ] **Step 2: Generalise the info bar**

Replace `InfoBar`'s `profile`, `on_profile` and `on_export` with a list of actions:

```rust
/// A right-side info bar action: a meta text button with an optional dot or icon.
#[derive(Clone)]
pub struct BarAction {
    pub id: SharedString,
    pub label: SharedString,
    pub icon: Option<IconName>,
    /// A small dot before the label (e.g. a state indicator).
    pub dot: bool,
    pub on_click: Option<Callback>,
}

/// Breadcrumbs on the left (clickable), an optional note, and actions on the right separated by `|`.
#[derive(IntoElement)]
pub struct InfoBar {
    crumbs: Vec<SharedString>,
    actions: Vec<BarAction>,
    snapshot: Option<SharedString>,
    on_crumb: Option<IndexCallback>,
}

impl InfoBar {
    pub fn new(crumbs: Vec<SharedString>) -> Self {
        Self { crumbs, actions: vec![], snapshot: None, on_crumb: None }
    }

    pub fn action(mut self, action: BarAction) -> Self {
        self.actions.push(action);
        self
    }

    /// A muted note after the breadcrumbs (e.g. `Snapshot · 14:32:08`).
    pub fn snapshot(mut self, note: Option<SharedString>) -> Self {
        self.snapshot = note;
        self
    }

    pub fn on_crumb(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_crumb = Some(Rc::new(f));
        self
    }
}
```

In `render`:
- The right side becomes one `TextButton::meta(a.id, a.label)` per action, with `.dot()` when `a.dot` and `.icon(i)` when `a.icon` is set, and the click wired when `on_click` is set.
- Consecutive actions are separated by the existing `sep("|")` element.
- The root element id `meta` becomes `info-bar`.

In `tests/chrome.rs`:
- `InfoBar::new(crumbs, "Services".into())` becomes `InfoBar::new(crumbs).action(BarAction { id: "profile".into(), label: "Profile: Services".into(), icon: None, dot: true, on_click: … }).action(BarAction { id: "export".into(), label: "Export".into(), icon: Some(IconName::Export), dot: false, on_click: … })`;
- the existing callback assertions keep working through these ids;
- `"meta"` becomes `"info-bar"`.

- [ ] **Step 3: Run the tests**

Run: `./dev test framework`
Expected: PASS: `chrome`, `title_bar`, `sidebar` (including the chevron test), `layout` (including `a_short_filtered_pane_keeps_recent_filters_inside_it`).

That last test used rogg's recent-filters list; rename it `a_short_pane_keeps_a_recent_list_inside_it`, use `with_recent_list` and the `recent-list`/`recent-entry-{i}` ids, and keep its assertions.

- [ ] **Step 4: Gate and commit**

Run `./dev check`.

```bash
git add -A
git commit -m "feat: window layout and chrome (title bar, sidebar, info bar with actions, pane header, status bar)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

# Stage 5 — The text view

### Task 7: `widgets::text_view`: port with a neutral data model

**Files:**
- Create via the port tool (`--lib`): `crates/gpui-framework/src/widgets/text_view/{mod,actions,element,model,scroll,state,text}.rs`, from `rogg:crates/rogg-ui/src/logview/`.
- Create: `crates/gpui-framework/src/widgets/mod.rs`.
- Test:
  - port `rogg:crates/rogg-ui/tests/logview.rs` to `tests/text_view.rs`;
  - port `logview_real_text.rs` to `tests/text_view_real_text.rs`, registered in `Cargo.toml` as `[[test]] name = "text_view_real_text" harness = false`.

**Interfaces:**
- Produces:
  - `gf::widgets::text_view::{TextView, TextViewState, TextViewElement, TextViewEvent, Line, Row, RowSource, VecRows, Span, SpanStyle, RowTint, CharSelection, ContextTarget, RowGeometry, ScrollTo, MAX_WRAPPED_LINES, MAX_COPY_ROWS, HBAR_HOVER_ZONE, HBAR_LINGER}`;
  - modules `actions` (`KEY_CONTEXT = "TextView"`), `scroll` and `text`.
- `Line { number: u64, text: SharedString, spans: Vec<Span>, marked: bool, dimmed: bool, tint: Option<RowTint> }`.
- `Row::{Line(Line), Gap { label: SharedString }}`.
- `TextViewEvent::{Selected { row, number }, ToggleMark { number }, TailLeft}`. Task 8 adds `ContextAction`.
- `SpanStyle::{Faint, Muted, Error, Warning, Fg(Color), Highlight(usize), Mark { fg, bg }, Bold, Outline}`.

- [ ] **Step 1: Port**

```bash
R=/Users/vadim/dev/projects/rogg2/crates/rogg-ui
mkdir -p crates/gpui-framework/src/widgets/text_view
for f in mod actions element model scroll state text; do tools/port/port.py $R/src/logview/$f.rs crates/gpui-framework/src/widgets/text_view/$f.rs --lib; done
tools/port/port.py $R/tests/logview.rs crates/gpui-framework/tests/text_view.rs
tools/port/port.py $R/tests/logview_real_text.rs crates/gpui-framework/tests/text_view_real_text.rs
```

`widgets/mod.rs`: `pub mod text_view;` (Task 9 adds `pub mod chip_band;`). In `lib.rs`: `pub mod widgets;`. Add the `[[test]]` entry to `crates/gpui-framework/Cargo.toml`:

```toml
[[test]]
name = "text_view_real_text"
harness = false
```

- [ ] **Step 2: Hand edits for the neutral model**

| where | change |
|---|---|
| `model.rs` `LogLine` → `Line` | fields `pinned` → `marked`, `context` → `dimmed`; doc comments say "marker" and "dimmed (e.g. context lines)" |
| `model.rs` `Row::Gap { omitted: u64 }` | becomes `Row::Gap { label: SharedString }` |
| `element.rs` `shape_row` | the gap label is `label.clone()`, not `format!("··· {omitted} lines omitted")` |
| `element.rs` paint | `pinned` → `marked`; the marker uses `IconName::PinFilled` / `Pin` as before (the icons stay; only the concept is renamed) |
| `model.rs` `SpanStyle` | `Amber` → `Warning`, `Hit` → `Highlight`, `SearchHit` → `Outline`; the docs describe them generically |
| `text.rs` `segments` | follows the renamed variants |
| `state.rs` events | `TogglePin` → `ToggleMark`; delete `AddTerm` and `ShowFullLine`; `TailLeft` and `Selected` stay |
| `mod.rs` `context_menu` | temporarily: built-in `Copy` and `Copy with line numbers`, plus `Mark` / `Unmark` (emits `ToggleMark`); the word items go (Task 8 adds app actions) |
| `actions.rs` | the namespace `log_view` → `text_view`; `KEY_CONTEXT = "TextView"` |
| all files | comments drop the domain wording ("log", "filter", "pin" → "line", "marker"); behaviour is unchanged |

Tests:
- In `tests/text_view.rs` and `tests/text_view_real_text.rs`, update the constructors (`pinned` → `marked`, `context` → `dimmed`, `Gap { omitted: 4 }` → `Gap { label: "··· 4 lines omitted".into() }`), the event names, and the id `log-test` → `text-test` (also `log-real` → `text-real`).
- Delete the test assertions that covered `AddTerm`/`ShowFullLine` and keep everything else.
- Keep the real-text cases list in `main` as is, renamed.

- [ ] **Step 3: Run the tests**

Run: `./dev test framework text_view` and `./dev test framework --test text_view_real_text`.
Expected: PASS: every ported case, including the wrap cap, char drags, the click threshold, survival across rebuilds, and the display-cut tests.

- [ ] **Step 4: Gate and commit**

Run `./dev check`.

```bash
git add -A
git commit -m "feat: text_view widget (virtualised numbered text lines) with a neutral data model

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 8: `text_view` context-menu actions defined by the app

**Files:**
- Modify:
  - `crates/gpui-framework/src/widgets/text_view/{mod,state}.rs`;
  - `crates/gpui-framework/src/widgets/text_view/menu.rs` (new: `MenuAction`, `MenuEntry`, `menu_entries`).
- Test: `crates/gpui-framework/tests/text_view_menu.rs` (new).

**Interfaces:**
- Consumes: `TextViewState::context_target() -> Option<&ContextTarget>` (`number`, `marked`, `token`) from Task 7. Rename `ContextTarget.pinned` → `marked` if the port left it.
- Produces:
  - `MenuAction::{new(id, label), for_word(id, f, fallback)}`;
  - `TextViewState::set_menu_actions(Vec<MenuAction>, cx)`;
  - `menu_entries(&ContextTarget, &[MenuAction]) -> Vec<MenuEntry>`;
  - `MenuEntry { label: SharedString, enabled: bool, kind: EntryKind }` with `EntryKind::{Mark, Copy, CopyWithNumbers, App(SharedString), Separator}`;
  - `TextViewEvent::ContextAction { id: SharedString, number: u64, word: Option<String> }`.

- [ ] **Step 1: Write the failing tests**

`crates/gpui-framework/tests/text_view_menu.rs`:

```rust
use gpui_framework::kit::{AppContext, TestAppContext};
use gpui_framework::widgets::text_view::{ContextTarget, EntryKind, MenuAction, TextViewEvent, TextViewState, VecRows, menu_entries};
use std::cell::RefCell;
use std::rc::Rc;

fn target(word: Option<&str>) -> ContextTarget {
    ContextTarget { number: 7, marked: false, token: word.map(str::to_string) }
}

fn actions() -> Vec<MenuAction> {
    vec![
        MenuAction::for_word("filter", |w| format!("Filter on “{w}”").into(), "Filter on selection"),
        MenuAction::new("show-line", "Show full line…"),
    ]
}

#[test]
fn entries_are_mark_copy_then_app_actions() {
    let entries = menu_entries(&target(Some("timeout")), &actions());
    let kinds: Vec<&EntryKind> = entries.iter().map(|e| &e.kind).collect();
    assert_eq!(
        kinds,
        [&EntryKind::Mark, &EntryKind::Separator, &EntryKind::Copy, &EntryKind::CopyWithNumbers, &EntryKind::Separator, &EntryKind::App("filter".into()), &EntryKind::App("show-line".into())]
    );
    assert_eq!(entries[0].label.as_ref(), "Mark");
    assert_eq!(entries[5].label.as_ref(), "Filter on “timeout”");
    assert!(entries.iter().all(|e| e.enabled));
}

#[test]
fn word_actions_are_disabled_without_a_word() {
    let entries = menu_entries(&target(None), &actions());
    let filter = entries.iter().find(|e| e.kind == EntryKind::App("filter".into())).unwrap();
    assert_eq!((filter.label.as_ref(), filter.enabled), ("Filter on selection", false));
    let show = entries.iter().find(|e| e.kind == EntryKind::App("show-line".into())).unwrap();
    assert!(show.enabled, "actions that don't need a word stay enabled");
}

#[test]
fn a_marked_line_offers_unmark_and_no_app_actions_means_no_trailing_separator() {
    let entries = menu_entries(&ContextTarget { number: 1, marked: true, token: None }, &[]);
    assert_eq!(entries[0].label.as_ref(), "Unmark");
    assert_ne!(entries.last().unwrap().kind, EntryKind::Separator);
}

#[gpui_framework::kit::test]
fn an_app_action_emits_its_id_line_and_word(cx: &mut TestAppContext) {
    let state = cx.new(|_| TextViewState::new(Rc::new(VecRows::new(vec![]))));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    cx.update(|cx| {
        cx.subscribe(&state, move |_, ev: &TextViewEvent, _| {
            if let TextViewEvent::ContextAction { id, number, word } = ev {
                s.borrow_mut().push((id.to_string(), *number, word.clone()));
            }
        })
        .detach();
    });
    state.update(cx, |s, cx| {
        s.set_menu_actions(actions(), cx);
        s.run_menu_action("filter", &target(Some("timeout")), cx);
    });
    assert_eq!(*seen.borrow(), vec![("filter".to_string(), 7, Some("timeout".to_string()))]);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev test framework text_view_menu`
Expected: compile errors.

- [ ] **Step 3: Implement**

`widgets/text_view/menu.rs`:

```rust
//! The text view's context menu: built-in entries plus actions the app defines.

use super::state::ContextTarget;
use gpui_kit::SharedString;
use std::rc::Rc;

/// An app-defined context-menu item. Its label may depend on the word under the pointer; an item that needs a word is
/// disabled (showing its fallback label) when there is none.
#[derive(Clone)]
pub struct MenuAction {
    pub id: SharedString,
    label: Rc<dyn Fn(Option<&str>) -> SharedString>,
    needs_word: bool,
}

impl MenuAction {
    /// A fixed label, always enabled.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        let label: SharedString = label.into();
        Self { id: id.into(), label: Rc::new(move |_| label.clone()), needs_word: false }
    }

    /// A label built from the word under the pointer; disabled with `fallback` when there is no word.
    pub fn for_word(id: impl Into<SharedString>, f: impl Fn(&str) -> SharedString + 'static, fallback: impl Into<SharedString>) -> Self {
        let fallback: SharedString = fallback.into();
        Self { id: id.into(), label: Rc::new(move |w| w.map_or_else(|| fallback.clone(), &f)), needs_word: true }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Mark,
    Copy,
    CopyWithNumbers,
    App(SharedString),
    Separator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuEntry {
    pub label: SharedString,
    pub enabled: bool,
    pub kind: EntryKind,
}

/// Longest word shown in a label, in chars; longer words are cut and end with `…`.
pub const MAX_LABEL_CHARS: usize = 40;

/// `word` cut on a char boundary to [`MAX_LABEL_CHARS`] for labels.
pub fn label_word(word: &str) -> String {
    match word.char_indices().nth(MAX_LABEL_CHARS) {
        Some((cut, _)) => format!("{}…", &word[..cut]),
        None => word.to_string(),
    }
}

/// The entries for `target`: Mark/Unmark, Copy, Copy with line numbers, then the app actions (with separators).
pub fn menu_entries(target: &ContextTarget, actions: &[MenuAction]) -> Vec<MenuEntry> {
    let entry = |label: &str, kind| MenuEntry { label: SharedString::from(label.to_string()), enabled: true, kind };
    let mut out = vec![
        entry(if target.marked { "Unmark" } else { "Mark" }, EntryKind::Mark),
        entry("", EntryKind::Separator),
        entry("Copy", EntryKind::Copy),
        entry("Copy with line numbers", EntryKind::CopyWithNumbers),
    ];
    if !actions.is_empty() {
        out.push(entry("", EntryKind::Separator));
        let word = target.token.as_deref().map(label_word);
        for a in actions {
            out.push(MenuEntry { label: (a.label)(word.as_deref()), enabled: !a.needs_word || word.is_some(), kind: EntryKind::App(a.id.clone()) });
        }
    }
    out
}
```

`state.rs`:
- add `menu_actions: Vec<MenuAction>` to `TextViewState` (empty in `new`);
- add `TextViewEvent::ContextAction { id: SharedString, number: u64, word: Option<String> }`;
- add these methods:

```rust
    /// App-defined context-menu items (after the built-in Mark / Copy entries).
    pub fn set_menu_actions(&mut self, actions: Vec<MenuAction>, cx: &mut Context<Self>) {
        self.menu_actions = actions;
        cx.notify();
    }

    pub fn menu_actions(&self) -> &[MenuAction] {
        &self.menu_actions
    }

    /// Emits the app action `id` for `target` (the line and the full word under the pointer).
    pub fn run_menu_action(&mut self, id: &str, target: &ContextTarget, cx: &mut Context<Self>) {
        cx.emit(TextViewEvent::ContextAction { id: SharedString::from(id.to_string()), number: target.number, word: target.token.clone() });
    }
```

`mod.rs`:
- add `mod menu; pub use menu::{EntryKind, MAX_LABEL_CHARS, MenuAction, MenuEntry, label_word, menu_entries};`;
- delete the old `label_token`/`MAX_LABEL_CHARS` (now in `menu.rs`), and move its unit test there, renamed to `label_word`;
- rebuild `context_menu` from `menu_entries`:

```rust
fn context_menu(menu: PopupMenu, state: &Entity<TextViewState>, cx: &App) -> PopupMenu {
    let Some(target) = state.read(cx).context_target().cloned() else { return menu };
    let entries = menu_entries(&target, state.read(cx).menu_actions());
    entries.into_iter().fold(menu, |menu, e| {
        let (s, t) = (state.clone(), target.clone());
        match e.kind {
            EntryKind::Separator => menu.separator(),
            EntryKind::Mark => menu.item(PopupMenuItem::new(e.label).on_click(move |_, _, cx| emit(&s, TextViewEvent::ToggleMark { number: t.number }, cx))),
            EntryKind::Copy => menu.item(PopupMenuItem::new(e.label).on_click(move |_, _, cx| copy(&s, false, cx))),
            EntryKind::CopyWithNumbers => menu.item(PopupMenuItem::new(e.label).on_click(move |_, _, cx| copy(&s, true, cx))),
            EntryKind::App(id) => {
                menu.item(PopupMenuItem::new(e.label).disabled(!e.enabled).on_click(move |_, _, cx| s.update(cx, |s, cx| s.run_menu_action(&id, &t, cx))))
            }
        }
    })
}
```

- [ ] **Step 4: Run the tests**

Run: `./dev test framework text_view`
Expected: PASS (`text_view`, `text_view_menu`, the unit tests).

- [ ] **Step 5: Gate and commit**

Run `./dev check`.

```bash
git add -A
git commit -m "feat: text_view context menu with app-defined actions

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

# Stage 6 — The chip band

### Task 9: `widgets::chip_band`

**Files:**
- Create via the port tool (`--lib`): `crates/gpui-framework/src/widgets/chip_band.rs`, from `rogg:crates/rogg-ui/src/filter.rs`.
- Test: port `rogg:crates/rogg-ui/tests/filter.rs` to `crates/gpui-framework/tests/chip_band.rs`, plus new tests.
- Modify: `widgets/mod.rs` (`pub mod chip_band;`).

**Interfaces:**
- Consumes: `layout::BandDrag`, controls.
- Produces:
  - `gf::widgets::chip_band::{ChipBand, Chip, ChipState, ChipColor, chip_color, BandFlag}`;
  - `ChipBand::new(Vec<(SharedString, ChipState)>)` with the builders `flags(Vec<BandFlag>)`, `on_flag(IndexCallback)`, `left_action(label, IconName, Callback)`, `select(label, Vec<SharedString>, usize, IndexCallback)`, `summary(SharedString)`, `hint(SharedString)`, `apply_label(SharedString)`, `editing(Option<Entity<InputState>>)`, `error(Option<SharedString>)`, `chip_colors(Option<Vec<ChipColor>>)`, `focused(bool)`, `on_cycle(IndexCallback-like)`, `on_edit(Callback)`, `on_apply(Callback)`;
  - `BandFlag { id: SharedString, label: SharedString, pressed: bool }`.

- [ ] **Step 1: Port**

```bash
R=/Users/vadim/dev/projects/rogg2/crates/rogg-ui
tools/port/port.py $R/src/filter.rs crates/gpui-framework/src/widgets/chip_band.rs --lib
tools/port/port.py $R/tests/filter.rs crates/gpui-framework/tests/chip_band.rs
```

- [ ] **Step 2: Generalise the parameters**

| rogg (`FilterBand`) | framework (`ChipBand`) |
|---|---|
| `regex(bool)`, `case_sensitive(bool)`, `on_toggle_regex`, `on_toggle_case` | `flags(Vec<BandFlag>)` + `on_flag(f(index))`; each flag renders `FlagButton::new(format!("flag-{id}"), label).pressed(pressed)` |
| `on_history` + hard-coded `TextButton::small("history", "History").icon(History)` | `left_action(label, icon, cb)`, rendered as `TextButton::small("band-action", label).icon(icon)`, only when set |
| `context(items, selected)` + `on_context` + the "Context" label | `select(label, options, selected, on_select)`: a label and `CompactSelect::new("band-select", …)`, only when set |
| hard-coded hint text | `hint(text)`, only when set |
| `summary(text)` | `summary(text)` (unchanged) |
| `"Apply ↵"` | `apply_label(text)`, default `"Apply ↵"` |

```rust
/// A toggle flag next to the chips (e.g. `.*`, `Aa`).
#[derive(Clone, Debug, PartialEq)]
pub struct BandFlag {
    pub id: SharedString,
    pub label: SharedString,
    pub pressed: bool,
}
```

Element id renames:
- `filter-band` → `chip-band`, `filters` → `chips`, `filter-actions` → `band-actions`, `edit-filters` → `band-edit`, `filter-bottom` → `band-bottom`, `filter-error` → `band-error`;
- `token-{i}` → `chip-{i}`, `token-state-{i}` → `chip-state-{i}`;
- `history` → `band-action`, `context-select` → `band-select`;
- `expression`, `apply`, `band-handle` and `band-focus` stay.

Comments drop the filter wording; `ChipState` keeps IN / OFF / NOT.

In `tests/chip_band.rs`:
- update the ids;
- `.regex(true)` becomes `.flags(vec![BandFlag { id: "regex".into(), label: ".*".into(), pressed: true }, …])`;
- keep the geometry tests (`chips_and_flags_use_reference_heights`, `the_band_keeps_its_height_in_edit_mode`, …), using `flag-regex` through the flag id.

- [ ] **Step 3: Write the new failing tests**

Append to `tests/chip_band.rs`:

```rust
#[gpui_framework::kit::test]
fn optional_parts_render_only_when_given(cx: &mut TestAppContext) {
    use gpui_framework::widgets::chip_band::ChipBand;
    common::setup(cx);
    let bare = common::open(cx, 1200., 200., |_, _| ChipBand::new(terms(2)).into_any_element());
    for id in ["band-action", "band-select", "flag-regex"] {
        assert!(!common::exists(cx, bare, id), "{id} without being configured");
    }
    let full = common::open(cx, 1200., 200., |_, _| {
        ChipBand::new(terms(2))
            .flags(vec![BandFlag { id: "regex".into(), label: ".*".into(), pressed: false }])
            .left_action("Recent".into(), gpui_framework::icons::IconName::History, Rc::new(|_, _| {}))
            .select("Scope".into(), vec!["All".into(), "Mine".into()], 0, Rc::new(|_, _, _| {}))
            .summary("2 include".into())
            .hint("Click a chip to cycle".into())
            .into_any_element()
    });
    for id in ["band-action", "band-select", "flag-regex"] {
        assert!(common::exists(cx, full, id), "{id} missing");
    }
}

#[gpui_framework::kit::test]
fn a_flag_click_reports_its_index(cx: &mut TestAppContext) {
    use gpui_framework::widgets::chip_band::ChipBand;
    common::setup(cx);
    let clicked = Rc::new(Cell::new(None));
    let c = clicked.clone();
    let w = common::open(cx, 1200., 200., move |_, _| {
        let c = c.clone();
        ChipBand::new(terms(1))
            .flags(vec![BandFlag { id: "regex".into(), label: ".*".into(), pressed: false }, BandFlag { id: "case".into(), label: "Aa".into(), pressed: true }])
            .on_flag(move |i, _, _| c.set(Some(i)))
            .into_any_element()
    });
    common::click(cx, w, "flag-case");
    assert_eq!(clicked.get(), Some(1));
}
```

(Import `gpui_framework::widgets::chip_band::BandFlag` at the top of the file.)

- [ ] **Step 4: Run, implement until they pass**

Run: `./dev test framework chip_band`. Expected first: compile errors. Then PASS.

- [ ] **Step 5: Gate and commit**

Run `./dev check`.

```bash
git add -A
git commit -m "feat: chip_band widget with configurable flags, action, select, summary and hint

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

# Stage 7 — Gallery and showcase

### Task 10: Gallery app, showcase page, capture

**Files:**
- Create: `crates/gpui-framework-gallery/Cargo.toml`.
- Create via the port tool, without `--lib`: `src/{main,lib,gallery,pages,fixtures,capture}.rs`, and `src/showcase/{mod,data,state,view}.rs` from `replica/`.
- Test: port `rogg:crates/rogg-gallery/tests/{capture,pages}.rs`.
- Modify:
  - the root `Cargo.toml` (add the member);
  - `dev` (wire `gallery`, `smoke`, `capture`).

**Interfaces:**
- Consumes: the whole library.
- Produces:
  - the binary `gpui-framework-gallery` with `--page <slug>`;
  - `--capture <page> [--dialog <slug>] [--theme dark|light] [--density compact|comfortable] [--size WxH] --out <png> [--geometry <json>]`;
  - page slugs `tokens controls tabs sidebar chips textview chrome empty dialogs showcase`;
  - dialog slugs `picker key-values shortcuts about`;
  - `GF_GALLERY_SMOKE=1` quits after 1.5 s, printing `smoke ok`.

- [ ] **Step 1: Manifest and port**

`crates/gpui-framework-gallery/Cargo.toml`:

```toml
[package]
name = "gpui-framework-gallery"
description = "Every gpui-framework component and state, with headless capture"
version.workspace = true
edition.workspace = true
publish.workspace = true
license.workspace = true

[dependencies]
gpui-framework = { path = "../gpui-framework" }
gpui-kit = { workspace = true, features = ["test-support"] }
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true

[dev-dependencies]
gpui-kit = { workspace = true, features = ["test-support"] }
serde_json.workspace = true
```

```bash
G=/Users/vadim/dev/projects/rogg2/crates/rogg-gallery
for f in main lib gallery pages fixtures capture; do tools/port/port.py $G/src/$f.rs crates/gpui-framework-gallery/src/$f.rs; done
for f in mod data state view; do tools/port/port.py $G/src/replica/$f.rs crates/gpui-framework-gallery/src/showcase/$f.rs; done
mkdir -p crates/gpui-framework-gallery/tests
for f in capture pages; do tools/port/port.py $G/tests/$f.rs crates/gpui-framework-gallery/tests/$f.rs; done
```

Add `"crates/gpui-framework-gallery"` to the workspace members.

- [ ] **Step 2: Hand edits**

- **Pages:**
  - delete `Page::App` and its slug, title and render;
  - `Page::Replica` becomes `Page::Showcase` (`"showcase"`, "Showcase");
  - `Page::Filter` becomes `Page::Chips` (`"chips"`, "Chip band");
  - `Page::LogView` becomes `Page::TextView` (`"textview"`, "Text view").
- **`main.rs` and `capture.rs`:**
  - remove `install_globals`, `AppPaths` and the `seed_app_state` history seeding (no `rogg_app`/`rogg_core`);
  - the theme install is `Theme::default_dark().install(cx)`, with the capture options' appearance and density;
  - the smoke env var is `GF_GALLERY_SMOKE`;
  - assets come from `gpui_framework::assets::Assets`.
- **`showcase/`** (from `replica/`):
  - `ReplicaState`/`ReplicaDialog` become `ShowcaseState`/`ShowcaseDialog`;
  - the dialogs become `ShowcaseDialog::{Picker, KeyValues, Shortcuts, About}`, slugs `picker`, `key-values`, `shortcuts`, `about`, built with `dialogs::picker` (the open tabs), `key_value_list` (sample statistics), `shortcuts` (sample keys) and `about` (a sample `AboutView` for "Showcase 0.1.0");
  - the view uses `ChipBand` (flags `.*`/`Aa`, left action "History", select "Context", a summary and a hint), `TextView` panes with app menu actions (`MenuAction::for_word("filter", …, "Filter on selection")`, `for_word("exclude", …)`, `new("show-line", "Show full line…")`), `InfoBar::new(crumbs).action(…)` with "Profile: Services" (dot) and "Export" (icon), the title bar, the sidebar and the status bar;
  - the data (`data.rs`) keeps the sample service log, with `pinned` → `marked`, `context` → `dimmed` and gap rows labelled `"··· N lines omitted"`.
- **`pages.rs`:**
  - drop the `EditableRuleRow`/`PaletteSlot` specimens;
  - the empty-states page shows `welcome(...)` with two actions, `loading`, `error`, and `with_recent_list(EmptyState::new(…, "No matches"), "Recent searches", …)`;
  - the dialogs page shows every generic builder.
- **Fixtures:** strings mentioning the source app become neutral ("Showcase", "sample.log", "service.log", …). The port tool reports any lines left.
- **`tests/capture.rs`:** `captures_replica_with_rows_and_dialog` becomes `captures_showcase_with_rows_and_dialog` (`--capture showcase --dialog about`); `CARGO_BIN_EXE_rogg-gallery` becomes `CARGO_BIN_EXE_gpui-framework-gallery`; the temp dir prefix is `gf-capture-`.
- **`tests/pages.rs`:** update the slug list.

- [ ] **Step 3: Wire `./dev`**

```zsh
cmd_gallery() { need_rust; run cargo run -p gpui-framework-gallery -- "$@"; }
cmd_smoke()   { need_rust; GF_GALLERY_SMOKE=1 run cargo run -p gpui-framework-gallery; }
cmd_capture() {
  need_rust
  local page=${1:-showcase} theme=${2:-dark} density=${3:-compact}
  local out=target/visual/manual/$page-$theme-$density
  run mkdir -p target/visual/manual
  run cargo run -p gpui-framework-gallery -- --capture "$page" --theme "$theme" --density "$density" --out "$out.png" --geometry "$out.json"
  ok "$out.png"
}
```

Update the help lines; drop the `(Task 10)` markers.

- [ ] **Step 4: Run the tests and look**

Run: `./dev test gallery`, `./dev smoke`, `./dev capture showcase dark compact`, `./dev capture controls light compact`.
Expected: tests PASS; smoke prints `smoke ok`; the captures exist.

Open `target/visual/manual/showcase-dark-compact.png` and confirm the main window is complete: title bar with tabs, sidebar, two text panes, the chip band, the status bar.

- [ ] **Step 5: Gate and commit**

Run `./dev check`.

```bash
git add -A
git commit -m "feat: component gallery with the showcase window and headless capture

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

# Stage 8 — Template

### Task 11: Starter template app

**Files:**
- Create:
  - `crates/gpui-framework-template/Cargo.toml`;
  - `crates/gpui-framework-template/src/main.rs`;
  - `crates/gpui-framework-template/tests/smoke.rs`.
- Modify:
  - the root `Cargo.toml` (add the member);
  - `dev` (wire `template`).

**Interfaces:**
- Consumes: `gf::{Theme, ThemeBuilder, assets::Assets, chrome::*, layout::*, widgets::text_view::*, dialogs::about, toast::ToastStack, kit}`.
- Produces: the binary `gpui-framework-template`. `GF_TEMPLATE_SMOKE=1` quits after 1.5 s, printing `smoke ok`.

- [ ] **Step 1: Write the failing smoke test**

`crates/gpui-framework-template/tests/smoke.rs`:

```rust
//! The starter app opens a window and quits by itself (the real binary: gpui must own the main thread).
use std::process::Command;

#[test]
fn the_template_opens_and_quits() {
    let out = Command::new(env!("CARGO_BIN_EXE_gpui-framework-template")).env("GF_TEMPLATE_SMOKE", "1").output().expect("spawn template");
    assert!(out.status.success(), "template exited with {:?}: {}", out.status, String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("smoke ok"));
}
```

`crates/gpui-framework-template/Cargo.toml`:

```toml
[package]
name = "gpui-framework-template"
description = "A minimal gpui-framework app to copy as a starting point"
version.workspace = true
edition.workspace = true
publish.workspace = true
license.workspace = true

[dependencies]
gpui-framework = { path = "../gpui-framework" }
```

Add `"crates/gpui-framework-template"` to the workspace members.

- [ ] **Step 2: Run it to verify it fails**

Run: `./dev test template`
Expected: a compile error (no `main.rs`).

- [ ] **Step 3: Write the template**

`crates/gpui-framework-template/src/main.rs`:

```rust
//! A minimal gpui-framework app: title bar, sidebar, one text view, status bar, an About dialog, a toast, menus and a
//! light/dark switch. Copy this crate to start a new app (see docs/guides/new-app.md).

use gpui_framework as gf;
use gf::chrome::{StatPart, StatusBar, TabItem, TitleBar, TreeRow, Sidebar};
use gf::dialogs::{AboutView, ModalLayer, about};
use gf::kit::prelude::*;
use gf::kit::{App, Bounds, Context, Entity, Menu, MenuItem, QuitMode, SharedString, Window, WindowBounds, WindowOptions, div, px, size};
use gf::layout::{MainWindowLayout, SingleParts};
use gf::theme::{Appearance, ThemeBuilder};
use gf::toast::ToastStack;
use gf::widgets::text_view::{Line, Row, TextView, TextViewState, VecRows};
use std::rc::Rc;
use std::time::Duration;

gf::kit::actions!(template, [ShowAbout, ToggleAppearance, Quit]);

struct AppView {
    theme: ThemeBuilder,
    text: Entity<TextViewState>,
    about_open: bool,
    toasts: Vec<(u64, SharedString)>,
}

impl AppView {
    fn new(theme: ThemeBuilder, cx: &mut Context<Self>) -> Self {
        let rows = (1..=200u64)
            .map(|n| Row::Line(Line { number: n, text: format!("line {n}: hello from gpui-framework").into(), spans: vec![], marked: false, dimmed: false, tint: None }))
            .collect();
        let text = cx.new(|_| TextViewState::new(Rc::new(VecRows::new(rows))));
        Self { theme, text, about_open: false, toasts: vec![(1, "Welcome — this is a toast".into())] }
    }

    fn toggle_appearance(&mut self, cx: &mut Context<Self>) {
        let next = match self.theme.build().appearance {
            Appearance::Dark => Appearance::Light,
            Appearance::Light => Appearance::Dark,
        };
        self.theme = self.theme.clone().appearance(next);
        self.theme.install(cx);
    }
}

impl Render for AppView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let title_bar = TitleBar::new(vec![TabItem { label: "notes.txt".into(), tooltip: "~/notes.txt".into(), active: true }]).traffic_lights(true);
        let sidebar = Sidebar::new().count("1 file".into()).rows(vec![
            TreeRow::Folder { label: "Documents".into(), expanded: true, depth: 0, note: None, failed: false },
            TreeRow::File { label: "notes.txt".into(), size: "4 KB".into(), active: true, depth: 1, failed: false },
        ]);
        let status = StatusBar::new("UTF-8 · LF".into()).stats(vec![StatPart::Strong("200".into()), StatPart::Text(" lines".into())]);
        let body = MainWindowLayout::single(SingleParts {
            titlebar: title_bar.into_any_element(),
            sidebar: Some(sidebar.into_any_element()),
            body: TextView::new("text", self.text.clone()).into_any_element(),
            status: status.into_any_element(),
        });
        let close = {
            let this = this.clone();
            move |_: &mut Window, cx: &mut App| this.update(cx, |v, cx| {
                v.about_open = false;
                cx.notify();
            })
        };
        let dismiss = {
            let this = this.clone();
            move |id: u64, _: &mut Window, cx: &mut App| this.update(cx, |v, cx| {
                v.toasts.retain(|(t, _)| *t != id);
                cx.notify();
            })
        };
        div()
            .size_full()
            .relative()
            .on_action(cx.listener(|v, _: &ShowAbout, _, cx| {
                v.about_open = true;
                cx.notify();
            }))
            .on_action(cx.listener(|v, _: &ToggleAppearance, _, cx| v.toggle_appearance(cx)))
            .child(body)
            .when(self.about_open, |d| {
                let view = AboutView {
                    version: "Template 0.1.0".into(),
                    tagline: "A gpui-framework starter app".into(),
                    build: vec![],
                    data: vec![],
                    built_with: "Built with Rust, gpui and gpui-framework".into(),
                    credits: vec![],
                };
                d.child(ModalLayer::new(about(&view, Rc::new(|_, _, _| {})).on_close(close.clone())).on_dismiss(close))
            })
            .when(!self.toasts.is_empty(), |d| d.child(ToastStack::new(self.toasts.clone()).on_dismiss(dismiss)))
    }
}

fn main() {
    gf::kit::application().with_assets(gf::assets::Assets).with_quit_mode(QuitMode::LastWindowClosed).run(|cx: &mut App| {
        gf::kit::init(cx);
        // Your app's look: start from the built-in theme and override what you need, e.g.
        // let theme = ThemeBuilder::new().accent(gf::theme::Color::rgb(0x4f7cff));
        let theme = ThemeBuilder::new();
        theme.install(cx);
        cx.bind_keys([gf::kit::KeyBinding::new("cmd-q", Quit, None), gf::kit::KeyBinding::new("cmd-shift-l", ToggleAppearance, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_menus(vec![Menu::new("Template").items([
            MenuItem::action("About Template", ShowAbout),
            MenuItem::action("Toggle Light/Dark", ToggleAppearance),
            MenuItem::separator(),
            MenuItem::action("Quit", Quit),
        ])]);
        let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
        gf::kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, move |_, cx| {
            cx.new(|cx| AppView::new(theme.clone(), cx))
        })
        .expect("open window");
        cx.activate(true);
        if std::env::var_os("GF_TEMPLATE_SMOKE").is_some() {
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

If `SingleParts`' field names differ from those above, check them in `crates/gpui-framework/src/layout.rs` (ported from rogg) and use the real ones. The same goes for the exact `ModalLayer`/`about` call shapes from Task 5. Keep the template's behaviour as described.

`dev`:

```zsh
cmd_template() { need_rust; run cargo run -p gpui-framework-template; }
```

- [ ] **Step 4: Run the tests**

Run: `./dev test template` and `./dev template` (close the window by hand).
Expected: the smoke test PASSes, and the window shows the chrome, the text lines and the toast. ⌘⇧L switches light/dark, and the menu opens About.

- [ ] **Step 5: Gate and commit**

Run `./dev check`.

```bash
git add -A
git commit -m "feat: starter template app

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 6: Out-of-workspace build (Review Focus 4)**

```bash
T=$(mktemp -d)/myapp && mkdir -p $T/src
cp crates/gpui-framework-template/src/main.rs $T/src/
cp rust-toolchain.toml $T/
cat > $T/Cargo.toml <<'EOF'
[package]
name = "myapp"
version = "0.1.0"
edition = "2024"

[dependencies]
gpui-framework = { path = "/Users/vadim/dev/projects/gpui-framework/crates/gpui-framework" }
EOF
(cd $T && PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH" cargo build --offline 2>&1 | tail -3)
```

Expected: `Finished`. If it fails because the library inherits workspace keys (`version.workspace = true`, …), cargo resolves them from the framework's own workspace, which is correct; a real failure means a missing re-export, so fix it in the library. Record the result in the commit message of Task 13's `new-app.md` guide.

---

# Stage 9 — Visual tooling and goldens

### Task 12: Visual regression for the gallery

**Files:**
- Create: `tools/visual-diff/{package.json,package-lock.json,config.mjs,gallery.mjs,check.mjs,approve.mjs,compare.mjs}`, from rogg, adapted.
- Modify: `dev` (wire `visual-check` and `visual-approve`; `setup` installs the npm packages and Playwright).
- Create in Step 5, after the user's approval only: `tools/visual-diff/golden/*.png` and `tolerance.json`.

**Interfaces:**
- Produces:
  - `./dev visual-check`: re-captures and compares against the goldens (fails over 0.05 %);
  - `./dev visual-approve`: asks first, then overwrites the goldens.
- The golden set is `showcase` in `dark-compact`, `dark-comfortable`, `light-compact` and `light-comfortable`, plus every dialog (`picker`, `key-values`, `shortcuts`, `about`) in `dark-compact`.

- [ ] **Step 1: Copy and adapt**

```bash
V=/Users/vadim/dev/projects/rogg2/tools/visual-diff
mkdir -p tools/visual-diff
cp $V/package.json $V/package-lock.json $V/check.mjs $V/approve.mjs $V/gallery.mjs $V/compare.mjs $V/config.mjs tools/visual-diff/
```

- **`config.mjs`:**
  - delete `REFERENCE_HTML`, `PAGE`'s env override, `SELECTORS` and `PROBES`;
  - `PAGE = 'showcase'`;
  - `OUT = path.join(ROOT, 'target/visual')`;
  - `GALLERY_BIN = path.join(ROOT, 'target/debug/gpui-framework-gallery')`;
  - `DIALOGS = ['picker', 'key-values', 'shortcuts', 'about'].map((gallery) => ({ gallery }))`;
  - keep `VARIANTS`, `SIZE`, `SCALE`, `variantName` and `captureNames`.
- **`gallery.mjs`:** run `cargo build -p gpui-framework-gallery`.
- **`check.mjs` / `approve.mjs`:** drop the `PAGE !== 'replica'` guards.
- **`compare.mjs`:** keep only what `check.mjs` imports. If `check.mjs` doesn't import it, delete it.
- **`package.json`:** `"name": "gpui-framework-visual-diff"`; scripts `"gallery"`, `"check"`, `"approve"` and `"all": "node gallery.mjs"`.

`dev`:

```zsh
cmd_visual_check()   { need_rust; need_node; step "Visual check against the goldens"; run_visual npm run check; }
cmd_visual_approve() {
  need_rust; need_node
  confirm "Overwrite the approved goldens in $VISUAL_DIR/golden with fresh gallery captures?" || die "cancelled"
  step "Capture and approve"
  run_visual npm run all
  run_visual npm run approve
  ok "review and commit the changes under $VISUAL_DIR/golden"
}
```

`cmd_check` turns the visual step on once `tools/visual-diff/golden/tolerance.json` exists (Task 1's rule).

- [ ] **Step 2: Capture for review**

Run: `./dev setup`, then `npm --prefix tools/visual-diff run all`. With cargo on PATH, as `./dev` sets it, the captures land in `target/visual/gallery/`.

Also capture rogg's replica for the comparison, read-only. Use the rogg binary that is already built if present; never build in rogg:

```bash
ls /Users/vadim/dev/projects/rogg2/target/debug/rogg-gallery && /Users/vadim/dev/projects/rogg2/target/debug/rogg-gallery --capture replica --out /tmp/source-replica-dark-compact.png
```

If the binary is missing, use rogg's committed golden `rogg:tools/visual-diff/golden/dark-compact.png` instead.

- [ ] **Step 3: Hand-off for the visual review (stop here)**

Show the user `target/visual/gallery/dark-compact.png` (the showcase) next to the source replica capture, plus the light variant and one dialog. List the deliberate differences: neutral sample data, generic labels, the dialog set. Ask for approval. Do not approve on your own.

- [ ] **Step 4: After approval, approve and gate**

Run: `yes | ./dev visual-approve` (the user approved in Step 3), then `./dev check`.
Expected: all green, and visual-check at 0.000 %.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "test(visual): golden-image regression for the gallery showcase and dialogs (approved)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

# Stage 10 — Documentation

### Task 13: Design-system spec, component reference, guides, history, README

**Files:**
- Create:
  - `docs/spec/design-system.md`;
  - `docs/components/*.md`;
  - `docs/guides/{getting-started,theming,testing,visual-regression,gpui-gotchas,new-app}.md`;
  - `docs/history/{rogg-design.md,rogg-refinements-1-design.md}`;
  - `docs/assets/*.png` (captures).
- Modify:
  - `README.md`, `docs/spec/README.md`;
  - `crates/gpui-framework/tests/no_rogg.rs` (drop `port` from the skip list);
  - `dev` (drop the `(Task N)` markers).
- Delete: `tools/port/`.

**Interfaces:**
- Consumes: every public API above.
- The docs must use the real names.

- [ ] **Step 1: History**

```bash
mkdir -p docs/history
cp /Users/vadim/dev/projects/rogg2/docs/superpowers/specs/2026-10-06-rogg-design.md docs/history/rogg-design.md
cp /Users/vadim/dev/projects/rogg2/docs/superpowers/specs/2026-10-08-rogg-refinements-1-design.md docs/history/rogg-refinements-1-design.md
```

Prepend to each:

```markdown
> **Archive.** This is the design spec of the app gpui-framework was extracted from (2026-10-08). It is kept for
> reference and is not binding; the framework's spec is `docs/spec/design-system.md`.
```

- [ ] **Step 2: `docs/spec/design-system.md`**

Write the binding spec from `docs/history/rogg-design.md` §5 and §7 and `rogg-refinements-1-design.md` §A–C. Rewrite them in framework terms, with no app or domain wording. Sections:
1. Principles: data plus callbacks, density, text scale, test ids.
2. Theme: palette roles (table from `Palette`), metrics and `size` constants, densities, fonts (SF Pro / SF Mono via `.AppleSystemUIFontMonospaced`), the text scale, `ThemeBuilder`, the gpui-kit mirror.
3. Icons: the set, `chevron(open)`.
4. Window layout: the title bar, sidebar, info bar, panes and band, status bar, with sizes.
5. Controls and forms: each control's sizes and states, including the custom-colour swatch with a pen and the fixed-width text field.
6. Chip band: chip states, colours, flags, edit mode at 28 px.
7. Text view element: rows, gutters, gap rows, spans, the wrap cap, horizontal scroll, tail mode, selection (row and characters, the 3 px threshold, lifetime), copy, the 64 KB cut, the context menu with app actions, keyboard actions.
8. Empty and transient states: toasts, loading, error, welcome, recent lists.
9. Dialogs and popovers: frame sizes, rows, the generic builders, the text viewer's 1 MB cap.
10. Visual decisions: the accepted list (see the Design memory in the history docs).
11. Verification: the gallery pages, capture, golden check and approve.

Every number must match the code. Read it from `theme/metrics.rs`, `palette.rs` and the components; do not trust the history doc where the code changed since.

- [ ] **Step 3: Component reference**

Write one file per public component in `docs/components/`, using this template:

```markdown
# <Component>

<one-paragraph purpose>

![<component>](../assets/<page>-dark-compact.png)

## Use

```rust
use gpui_framework as gf;
// a minimal real example, compiled in spirit from the gallery page
```

## API

| builder / field | meaning |
|---|---|

## Events and callbacks

## Test ids

## States and density
```

The files:
- `theme.md`, `icons.md`, `buttons.md`, `segmented.md`, `compact-select.md`, `find-field.md`, `form.md`;
- `dialogs.md`, `toasts.md`, `empty-states.md`;
- `title-bar.md`, `sidebar.md`, `info-bar.md`, `pane-header.md`, `status-bar.md`, `layout.md`;
- `text-view.md`, `chip-band.md`.

Captures: run `./dev capture <page> dark compact` for each gallery page and copy the PNGs into `docs/assets/`.

- [ ] **Step 4: Guides**

- `getting-started.md`: add the path dependency; `use gpui_framework as gf;`; `gf::kit::application().with_assets(gf::assets::Assets)`; `gf::kit::init(cx)`; `Theme::default_dark().install(cx)`; open a window and render a `TitleBar`.
- `theming.md`: `ThemeBuilder` (accent, palette edits, fonts, sizes, density), the light/dark switch keeping overrides, the palette roles table, and the mix rule for accent roles.
- `testing.md`:
  - `TestAppContext` has a no-op text system, so assert on sizes fixed in px only;
  - the real-text harness (`harness = false`, `HeadlessAppContext` with the macOS text system) is for glyph-dependent behaviour;
  - `.test_support()` ids and `common::{open, bounds, exists, click}`;
  - capture tests run the real binary;
  - patterns: RED by temporarily breaking the code.
- `visual-regression.md`: what the goldens cover, `./dev visual-check`, reading `target/visual/check/*.png`, `./dev visual-approve` only after review.
- `gpui-gotchas.md`: from `rogg:docs/superpowers/research/2026-10-06-gpui-api-notes.md`, rewritten neutrally, plus:
  - `App::reveal_path` panics on the test platform, so gate it;
  - gpui-kit `Input`/`Textarea` set their own text size, so style the input itself (it refines its style last);
  - the family name "SF Mono" falls back to Helvetica, so use `.AppleSystemUIFontMonospaced`;
  - `Theme::change` reloads kit defaults, so mirror after it;
  - `InputState::set_value` and `ColorPickerState::set_value` don't emit;
  - the kit `ColorPicker` trigger can't draw a swatch plus an icon (the overlay technique);
  - missing `rerun-if-changed` paths make cargo rerun every build.
- `new-app.md`: copy `crates/gpui-framework-template` into a new repo, set `gpui-framework = { path = "…/gpui-framework/crates/gpui-framework" }`, copy `rust-toolchain.toml`, build. Note the out-of-workspace build verified in Task 11 Step 6.

- [ ] **Step 5: README and cleanup**

`README.md`:
- the showcase screenshot `docs/assets/showcase-dark-compact.png`;
- a quick start (5 lines);
- the crate layout;
- the `./dev` command table;
- links to the spec, the components, the guides and the history;
- the license line `Apache-2.0`.

Cleanup:
- delete `tools/port/`;
- in `no_rogg.rs`, remove `"port"` from the skip list;
- update `docs/spec/README.md` (the design-system spec now exists).

- [ ] **Step 6: Gate and commit**

Run `./dev check`. Expected: all green, `no_trace_of_the_source_app` included, now without the port skip.

```bash
git add -A
git commit -m "docs: design-system spec, component reference, guides, history; remove the port tool

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 14: Final gate and hand-off

- [ ] **Step 1:** Run `./dev check` and `./dev template` (the window opens), then `git log --oneline` (the history matches Tasks 1–13) and `git status` (clean).
- [ ] **Step 2:** Confirm rogg is untouched: in `/Users/vadim/dev/projects/rogg2`, `git status --short` shows no tracked changes, and `git log -1` is this plan's commit.
- [ ] **Step 3: Hand-off (stop here).** Summarise for the user: what the framework contains, how to start an app (`docs/guides/new-app.md`), the `./dev` commands, any deviations ruled during execution, and the location `/Users/vadim/dev/projects/gpui-framework` (local git, no remote).

---

## Spec coverage

| Spec section | Task |
|---|---|
| §2 decisions: one-time copy, scope, structure, naming, theming, extras, docs, git, toolchain | 1 (repo, toolchain, naming guard), 2 (theming), 10–12 (extras), 13 (docs) |
| §3 repository layout | 1, 2–9 (library), 10, 11, 12, 13 |
| §4.1 theme (`ThemeBuilder`, `gf_theme`, defaults, kit mirror, size renames) | 2 (renames applied by the port), 5 (dialog sizes) |
| §4.2 controls, forms, empty states, layout; dropped domain parts | 3, 4, 6 |
| §4.3 chrome, including generic info bar actions | 6 |
| §4.4 icons and asset chaining | 2 |
| §5.1 text view: model, events, context actions, unchanged behaviour | 7, 8 |
| §5.2 chip band | 9 |
| §5.3 generic dialog builders | 5 |
| §6 gallery, showcase, template | 10, 11 |
| §7 `./dev`, visual-diff | 1, 10, 11, 12 |
| §8 testing (moved tests + new tests, gate) | 2–12, 14 |
| §9 documentation | 13 |
| §10 stages, the spec copied in | 1–14 |
| §11 out of scope | not implemented (no publish, no CI, rogg untouched: Task 14 Step 2) |
