# rogg Refinements 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The first round of post-v1 refinements, in three iterations that are each merged on their own:
- **A:** UI polish (no `⌄` glyphs, one rotated chevron, a stable filter-band height) and an About dialog.
- **B:** long wrapped rows capped at 3 lines, "Show full line…", and partial text selection within a row.
- **C:** any colour in profiles (colour picker), and recent filters in the empty filtered view.

**Architecture:**
- **`rogg-ui`** gains:
  - the `ChevronRight` icon and a `chevron(open)` helper;
  - the About and Full-line dialog builders, and a `DialogFrame::width`;
  - the `CustomColor` form control (gpui-kit's `ColorPicker`);
  - the recent-filters list for the empty view;
  - in the log view: the 3-line cap, a `ShowFullLine` event and character selection (state, mouse and paint).
- **`rogg-core`** gains one gpui-free helper, `history::recent_except`.
- **`rogg-app`** wires it all together:
  - `build.rs` for the build details, `about.rs`, and the `ShowAbout` action and menu item;
  - `Dialog::About` and `Dialog::FullLine`;
  - colour pickers in the profile editor;
  - recent filters in `tab_view`.

**Tech Stack:**
- Rust 1.99.0 (edition 2024).
- gpui-pre 0.3.8 through gpui-kit =0.7.1, which provides the `ColorPicker` and `Textarea` components.
- No new dependencies.
- `./dev` is the entry point for everything (build, tests, lint, visual check).

**Spec:** `docs/superpowers/specs/2026-10-08-rogg-refinements-1-design.md` (approved 2026-10-08).

**Decisions this plan makes (flagged for the plan review):**

1. **Character selection look.** While a row has a character selection, its background is not painted in the selection colour. The 2 px accent bar stays, and the selected characters are painted in `p.selection`. Otherwise they would be invisible on an already-selected row.
2. **Drag mode.** Once a drag leaves its start row it stays a whole-row drag until release. Coming back to the start row does not return to characters.
3. **`Custom…` swatch.** This is gpui-kit's `ColorPicker` trigger: a small square in the field's current colour, labelled `Custom…`. Its popover lists rogg's preset colours as "featured" colours, above the kit's palette, the HSL and alpha sliders, and the hex field.
4. **Build date.** `build.rs` reruns when git `HEAD` (or the branch it points to) moves. The date is the date of that build script run, not of every incremental build.
5. **Full-line dialog.** It is 760 px wide, and the text box is 360 px tall.
6. **Cache folder in About.** `Show in Finder` creates the folder first if it does not exist, so Finder always has something to show.

## Global Constraints

- **Visual baseline:**
  - From now on the app is not a pixel replica of `assets/rogg-main-window.html`.
  - The replica goldens (`./dev visual-check`) are a regression baseline.
  - Any intended change to them is shown to the user, then re-approved with `./dev visual-approve`, which asks first. The goldens are never re-approved silently.
- **Build hygiene:**
  - `./dev check` must be green at the end of every iteration: build, clippy `-D warnings`, all tests, visual-check.
  - Prefer `./dev …` over raw cargo/npm in steps and instructions.
- **Code layering:**
  - `rogg-core` never depends on gpui.
  - `rogg-ui` stays plain data plus callbacks: no app state.
  - The app crate imports gpui as `gpui_kit`. Never write `gpui::`.
- **Engine contracts from v1 still hold:**
  - never drop a `FilterJob` on a rayon thread;
  - slice lines to 64 KB before display (`MAX_DISPLAY_TEXT_BYTES`).
- **Exact strings (from the spec):**
  - `Profile: <name>`, `History`;
  - `About rogg` (menu item and dialog title), `A fast log explorer for macOS`, `Show in Finder`;
  - `Built with Rust, gpui and gpui-kit`;
  - `Show full line…`, `Line <number>`, `Copy`, `Custom…`, `Recent filters`, `No matching lines`.
- **Fixed values:**
  - The wrapped-row cap is **3** visual lines.
  - The full-line display cap is **1 MB** (1,048,576 bytes).
  - At most **3** recent filters are listed.
  - The filter chip height is **28 px** (`size::CHIP_H`).
- **Branches and merges:**
  - One branch per iteration: `refinements-1a`, `refinements-1b`, `refinements-1c`, each in a worktree.
  - Each iteration ends with a hand-off: a summary, `./dev demo` for the user to try, and the merge choice. The user decides every merge.
- **Commits:** follow the existing style, e.g. `feat(ui): …`, `fix(app): …`, `test: …`, `docs: …`.

## Review Focus

These are the inputs a person will meet that no task's tests would otherwise exercise. Each has a test in the owning task.

1. **A non-ASCII character selection.** Dragging across `é`, `日` or `🙂` snaps to char boundaries: no panic, and copy gives whole chars (Task 7: `char_selection_snaps_to_char_boundaries`).
2. **A line longer than the 64 KB display cut.** `Show full line…` and Copy carry the *whole* line, not the cut (Task 6: a 70 KB line). A character selection never reaches the `…` marker bytes (Task 7: `a_char_selection_stops_at_the_display_cut`).
3. **A wrapped row of 3 or fewer lines next to a capped row.** The short row keeps its natural height, the capped row is exactly 3 rows tall, and both are hit-testable (Task 5).
4. **A colour with alpha.** The picker writes `#rrggbbaa` when alpha < 1, and the profile parses it back (Task 10: the light override with alpha 0x80).
5. **Recent filters when the current filter is the only history entry.** No empty "Recent filters" heading is shown (Task 11).

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `crates/rogg-ui/assets/icons/rogg/chevron-right.svg` | create | The disclosure chevron |
| `crates/rogg-ui/src/icons.rs` | modify | `IconName::ChevronRight`, `chevron(open)` |
| `crates/rogg-ui/src/controls/buttons.rs` | modify | `IconButton::chevron_down` |
| `crates/rogg-ui/src/sidebar.rs` | modify | Folder rows use `chevron(expanded)` |
| `crates/rogg-ui/src/titlebar.rs` | modify | The all-tabs button uses the chevron |
| `crates/rogg-ui/src/chrome.rs` | modify | `Profile: <name>` (no `⌄`) |
| `crates/rogg-ui/src/filter.rs` | modify | `History` label; 28 px editor and Apply |
| `crates/rogg-gallery/src/pages.rs` | modify | Specimens follow A1/A2 |
| `crates/rogg-ui/tests/glyphs.rs` | create | Guard: no `⌄` in UI sources |
| `crates/rogg-ui/src/dialogs.rs` | modify | `DialogFrame::width`; `about()`, `full_line()` |
| `crates/rogg-ui/src/theme/metrics.rs` | modify | `FULL_LINE_DIALOG_W`, `FULL_LINE_TEXT_H` |
| `crates/rogg-app/build.rs` | create | Commit, date and rustc as compile-time env |
| `crates/rogg-app/src/about.rs` | create | `BuildInfo`, `CREDITS`, `data_folders`, `view` |
| `crates/rogg-app/src/actions.rs`, `menus.rs` | modify | `ShowAbout`; "About rogg" first in the rogg menu |
| `crates/rogg-app/src/workspace/mod.rs` | modify | `Dialog::About`, `Dialog::FullLine`, reveal, accessors |
| `crates/rogg-ui/src/logview/{element,state,mod}.rs` | modify | 3-line cap, `ShowFullLine`, character selection |
| `crates/rogg-app/src/tab.rs` | modify | Route `ShowFullLine` to `TabEvent::ShowFullLine` |
| `crates/rogg-ui/src/theme/color.rs` | modify | `Color::from_hsla` |
| `crates/rogg-ui/src/form.rs` | modify | `CustomColor` |
| `crates/rogg-app/src/dialogs/profiles.rs` | modify | Colour pickers per colour field |
| `crates/rogg-core/src/history.rs` | modify | `recent_except` |
| `crates/rogg-ui/src/empty.rs` | modify | `RecentFilter`, `no_matches_with_recent` |
| `crates/rogg-app/src/history.rs`, `tab_view.rs` | modify | Recent filters in the empty filtered view |
| `docs/superpowers/notes/2026-10-08-refinements-1-manual-checks.md` | create | Hand checks per iteration |

---

# Iteration A — UI polish and About (branch `refinements-1a`)

Setup (controller):
1. Enter a worktree named `refinements-1a` from `main`.
2. Run `./dev setup`, since a fresh worktree lacks `tools/visual-diff/node_modules`.
3. Run `./dev check` for a green baseline.

### Task 1: One chevron, no `⌄` glyphs (spec A1, A2)

**Files:**
- Create: `crates/rogg-ui/assets/icons/rogg/chevron-right.svg`, `crates/rogg-ui/tests/glyphs.rs`
- Modify:
  - `crates/rogg-ui/src/icons.rs`;
  - `crates/rogg-ui/src/controls/buttons.rs`;
  - `crates/rogg-ui/src/sidebar.rs:132`;
  - `crates/rogg-ui/src/titlebar.rs:190`;
  - `crates/rogg-ui/src/chrome.rs:96`;
  - `crates/rogg-ui/src/filter.rs:367`;
  - `crates/rogg-gallery/src/pages.rs:121,137,139`;
  - `crates/rogg-ui/tests/controls.rs:17`.
- Test: `crates/rogg-ui/tests/sidebar.rs`, `crates/rogg-ui/tests/glyphs.rs`, `crates/rogg-ui/tests/assets.rs` (existing; it checks every `IconName::ALL` path loads)

**Interfaces:**
- Produces:
  - `IconName::ChevronRight`;
  - `pub fn chevron(open: bool) -> Svg` in `rogg_ui::icons`;
  - `IconButton::chevron_down(id)`.
- Produces element ids: `tree-chevron-{i}` (collapsed) and `tree-chevron-open-{i}` (expanded).

- [ ] **Step 1: Write the failing tests**

Append to `crates/rogg-ui/tests/sidebar.rs`:

```rust
#[gpui_kit::test]
fn folder_rows_show_a_chevron_that_turns_when_open(cx: &mut TestAppContext) {
    common::setup(cx);
    let rows = vec![
        TreeRow::Folder { label: "open".into(), expanded: true, depth: 0, note: None, failed: false },
        TreeRow::Folder { label: "closed".into(), expanded: false, depth: 0, note: None, failed: false },
    ];
    let w = common::open(cx, 211., 400., move |_, _| Sidebar::new().rows(rows.clone()).into_any_element());
    assert!(common::exists(cx, w, "tree-chevron-open-0") && !common::exists(cx, w, "tree-chevron-0"));
    assert!(common::exists(cx, w, "tree-chevron-1") && !common::exists(cx, w, "tree-chevron-open-1"));
}
```

Create `crates/rogg-ui/tests/glyphs.rs`:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `./dev test ui folder_rows_show_a_chevron` and `./dev test ui no_down_chevron`
Expected:
- FAIL: no element `tree-chevron-open-0`.
- FAIL: hits list `sidebar.rs`, `titlebar.rs`, `chrome.rs`, `filter.rs`, `buttons.rs`, `pages.rs`.

- [ ] **Step 3: Add the icon**

Create `crates/rogg-ui/assets/icons/rogg/chevron-right.svg`. It uses the same style as `chevron-down.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="m9 6 6 6-6 6"/></svg>
```

In `crates/rogg-ui/src/icons.rs`:
- add the `ChevronRight` variant after `ChevronDown`;
- add it to `ALL`, which becomes `[IconName; 23]`;
- map it to `"icons/rogg/chevron-right.svg"`;
- add the helper:

```rust
use gpui_kit::{Svg, Transformation, radians, svg};

/// The disclosure chevron: points right when closed and is rotated 90° clockwise (pointing down) when open.
pub fn chevron(open: bool) -> Svg {
    let s = icon(IconName::ChevronRight);
    if open { s.with_transformation(Transformation::rotate(radians(std::f32::consts::FRAC_PI_2))) } else { s }
}
```

(If `Transformation`/`radians` are not re-exported at the `gpui_kit` root, import them from `gpui_kit::` where `Svg::with_transformation` lives — check `~/.cargo/registry/src/*/gpui-pre-0.3.8/src/elements/svg.rs`.)

- [ ] **Step 4: Use the chevron everywhere**

`crates/rogg-ui/src/controls/buttons.rs`:
- add the `ButtonContent::ChevronDown` variant;
- add the constructor below;
- render it as `chevron(true).size(px(13.)).text_color(p.muted)`;
- change the doc comment on `IconButton` to `(⌘, ✕)`.

```rust
    /// The "open files" button: the chevron, pointing down.
    pub fn chevron_down(id: impl Into<SharedString>) -> Self {
        Self { id: id.into(), content: ButtonContent::ChevronDown, compact: false, force_hover: false, on_click: None }
    }
```

`crates/rogg-ui/src/sidebar.rs`. Replace the `⌄`/`›` text child of the folder row with:

```rust
            .child(
                div()
                    .id(SharedString::from(if expanded { format!("tree-chevron-open-{i}") } else { format!("tree-chevron-{i}") }))
                    .test_support()
                    .flex_none()
                    .child(chevron(expanded).size(px(11.)).text_color(p.muted)),
            )
```

Other changes:
- `titlebar.rs:190`: `button(IconButton::chevron_down("all-tabs").compact(), self.on_all_tabs)`.
- `chrome.rs:96`: `format!("Profile: {}", self.profile)`.
- `filter.rs:367`: `TextButton::small("history", "History")`.
- `pages.rs`:
  - `IconButton::chevron_down("ib-chevron").compact()`;
  - `"Profile: Services"`;
  - `"History"`.
- `tests/controls.rs:17`: `IconButton::chevron_down("all-tabs").compact()`.

Breadcrumb `›` separators stay as they are.

- [ ] **Step 5: Run the tests**

Run: `./dev test ui`
Expected: PASS, including `assets` (the new icon loads), `sidebar` and `glyphs`.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-ui crates/rogg-gallery
git commit -m "feat(ui): one rotated chevron for disclosure; no trailing ⌄ on text buttons"
```

### Task 2: The filter band keeps its height in edit mode (spec A3)

**Files:**
- Modify: `crates/rogg-ui/src/filter.rs:283-326` (the editor branch of `FilterBand::render`)
- Test: `crates/rogg-ui/tests/filter.rs`

**Interfaces:**
- Consumes: `FilterBand::editing(Option<Entity<InputState>>)`, which exists today.

- [ ] **Step 1: Write the failing test**

Append to `crates/rogg-ui/tests/filter.rs` (add `use std::cell::RefCell;` and `use gpui_kit::{AppContext, Entity};`):

```rust
#[gpui_kit::test]
fn the_band_keeps_its_height_in_edit_mode(cx: &mut TestAppContext) {
    common::setup(cx);
    let chips = common::open(cx, 1200., 200., |_, _| FilterBand::new(terms(3)).into_any_element());
    let slot: Rc<RefCell<Option<Entity<InputState>>>> = Rc::default();
    let editing = common::open(cx, 1200., 200., move |window, cx| {
        let input = slot.borrow_mut().get_or_insert_with(|| cx.new(|cx| InputState::new(window, cx).default_value("error || timeout"))).clone();
        FilterBand::new(terms(3)).editing(Some(input)).into_any_element()
    });
    let a = common::bounds(cx, chips, "filter-band");
    let b = common::bounds(cx, editing, "filter-band");
    assert_eq!(a.size.height, b.size.height, "chips {a:?} vs editor {b:?}");
    assert_eq!(common::bounds(cx, editing, "expression").size.height, px(28.));
    assert_eq!(common::bounds(cx, editing, "apply").size.height, px(28.));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `./dev test ui the_band_keeps_its_height`
Expected: FAIL. The editor's line is taller: the 6 px padding plus the kit input's own height.

- [ ] **Step 3: Implement**

In the `Some(input)` branch:
- **`expression` div:** replace `.py(px(6.))` with `.h(px(size::CHIP_H)).flex().items_center()`, and give the `Input` `.h(px(size::CHIP_H - 2.))`. The 2 px are the div's 1 px border, top and bottom.
- **`apply` div:** replace `.py(px(5.))` with `.h(px(size::CHIP_H)).flex().items_center()`.

Keep the horizontal paddings, borders and colours.

- [ ] **Step 4: Run the tests**

Run: `./dev test ui filter`
Expected: PASS. The existing `chips_and_flags_use_reference_heights` is unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui/src/filter.rs crates/rogg-ui/tests/filter.rs
git commit -m "fix(ui): the expression editor and Apply take the chip height, so the band keeps its height"
```

### Task 3: About dialog (spec A4)

**Files:**
- Create: `crates/rogg-app/build.rs`, `crates/rogg-app/src/about.rs`, `crates/rogg-app/tests/about.rs`
- Modify:
  - `crates/rogg-app/src/lib.rs` (`pub mod about;`);
  - `crates/rogg-app/src/actions.rs` (`ShowAbout`);
  - `crates/rogg-app/src/menus.rs`;
  - `crates/rogg-app/src/workspace/mod.rs`;
  - `crates/rogg-ui/src/dialogs.rs`.
- Test:
  - `crates/rogg-app/src/about.rs` (unit);
  - `crates/rogg-app/src/menus.rs` (unit);
  - `crates/rogg-ui/tests/dialogs.rs`;
  - `crates/rogg-app/tests/about.rs`.

**Interfaces:**
- Produces:
  - `rogg_app::about::{BuildInfo, build_info(), describe(..), CREDITS, data_folders(cx), view(cx)}`;
  - `rogg_ui::dialogs::{AboutView, about(view, on_reveal)}`;
  - `Dialog::About`;
  - `Workspace::{reveal(path, cx), reveal_data_folder(i, cx), last_revealed()}`;
  - the action `rogg_app::actions::ShowAbout`.
- Produces element ids:
  - `about-header`;
  - `about-build-{0..2}` (commit, build date, Rust);
  - `about-data-{0,1}` and `about-reveal-{0,1}`;
  - `about-credit-{0..13}`.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-app/src/about.rs` (test module at the end of the new file; write the file with just the tests and `todo!()` bodies first, or write the tests after Step 3 and watch them pass. Either way, run them):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_build_details_read_unknown() {
        let b = describe(None, None, None);
        assert_eq!((b.commit, b.date, b.rustc), ("unknown", "unknown", "unknown"));
        assert_eq!(b.version, env!("CARGO_PKG_VERSION"));
        let b = describe(Some("abc1234"), Some("2026-10-08"), Some("rustc 1.99.0"));
        assert_eq!((b.commit, b.date, b.rustc), ("abc1234", "2026-10-08", "rustc 1.99.0"));
    }

    #[test]
    fn credits_list_the_spec_crates_with_licenses() {
        let names: Vec<&str> = CREDITS.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            ["gpui-kit", "regex", "aho-corasick", "memchr", "memmap2", "rayon", "notify", "flate2", "zip", "tar", "encoding_rs", "serde", "serde_json", "toml"]
        );
        assert!(CREDITS.iter().all(|(_, license)| !license.is_empty()));
    }
}
```

`crates/rogg-app/src/menus.rs`: update the existing test:

```rust
        let app = &menus[0];
        assert_eq!(app.items.len(), 9, "About rogg, separator, Settings…, separator, Hide rogg, Hide Others, Show All, separator, Quit rogg");
        assert!(matches!(&app.items[0], MenuItem::Action { name, .. } if name.as_ref() == "About rogg"));
```

(If `MenuItem::Action` has a different shape in gpui-pre 0.3.8, match its name field the way the file already matches `MenuItem::Submenu`.)

`crates/rogg-app/tests/about.rs`:

```rust
mod common;
use gpui_kit::TestAppContext;
use rogg_app::actions::ShowAbout;
use rogg_app::args::Args;

#[gpui_kit::test]
fn about_opens_from_its_action_and_reveals_the_data_folders(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![] });
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(ShowAbout), cx)).unwrap();
    cx.run_until_parked();
    assert!(common::exists(cx, w, "about-header"));
    assert!(common::exists(cx, w, "about-build-0") && common::exists(cx, w, "about-build-2"));
    assert!(common::exists(cx, w, "about-credit-13"));
    common::click(cx, w, "about-reveal-0");
    assert_eq!(ws.read_with(cx, |ws, _| ws.last_revealed().map(|p| p.to_path_buf())), Some(dir.path().to_path_buf()), "Application Support (the settings folder)");
    common::click(cx, w, "about-reveal-1");
    assert_eq!(ws.read_with(cx, |ws, _| ws.last_revealed().map(|p| p.to_path_buf())), Some(dir.path().join("cache")));
    assert!(dir.path().join("cache").is_dir(), "a missing cache folder is created so Finder can show it");
}
```

`crates/rogg-ui/tests/dialogs.rs` (append; the test checks the builder renders every row):

```rust
#[gpui_kit::test]
fn about_lists_build_data_and_credit_rows(cx: &mut TestAppContext) {
    use rogg_ui::dialogs::{AboutView, about};
    common::setup(cx);
    let view = AboutView {
        version: "rogg 0.1.0".into(),
        tagline: "A fast log explorer for macOS".into(),
        build: vec![("Commit".into(), "abc1234".into()), ("Built".into(), "2026-10-08".into()), ("Rust".into(), "rustc 1.99.0".into())],
        data: vec![("Settings".into(), "/tmp/a".into()), ("Cache".into(), "/tmp/b".into())],
        built_with: "Built with Rust, gpui and gpui-kit".into(),
        credits: vec![("regex".into(), "MIT OR Apache-2.0".into())],
    };
    let revealed = Rc::new(Cell::new(None));
    let r = revealed.clone();
    let w = common::open(cx, 900., 900., move |_, _| about(&view, Rc::new({ let r = r.clone(); move |i, _, _| r.set(Some(i)) })).into_any_element());
    assert!(common::exists(cx, w, "about-header") && common::exists(cx, w, "about-build-2") && common::exists(cx, w, "about-credit-0"));
    common::click(cx, w, "about-reveal-1");
    assert_eq!(revealed.get(), Some(1));
}
```

(Add `use std::cell::Cell; use std::rc::Rc;` if the file lacks them.)

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev test app about` and `./dev test ui about_lists`
Expected: compile errors (no `about` module, no `ShowAbout`, no `dialogs::about`).

- [ ] **Step 3: The build script and `about.rs`**

`crates/rogg-app/build.rs`:

```rust
//! Build details for the About dialog: the short git commit, the build date and the compiler version. Anything
//! that cannot be found (no git checkout, no `date`) is left unset, and the app shows `unknown`.
use std::process::Command;

fn output(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let values = [
        ("ROGG_GIT_COMMIT", output("git", &["rev-parse", "--short", "HEAD"])),
        ("ROGG_BUILD_DATE", output("date", &["+%Y-%m-%d"])),
        ("ROGG_RUSTC_VERSION", output(&rustc, &["--version"])),
    ];
    for (key, value) in values {
        if let Some(v) = value {
            println!("cargo:rustc-env={key}={v}");
        }
    }
    // Rerun when HEAD moves (a checkout, or a commit on the current branch), so the commit stays current.
    println!("cargo:rerun-if-changed=build.rs");
    if let Some(dir) = output("git", &["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
    }
    if let (Some(common), Some(branch)) =
        (output("git", &["rev-parse", "--path-format=absolute", "--git-common-dir"]), output("git", &["symbolic-ref", "-q", "HEAD"]))
    {
        println!("cargo:rerun-if-changed={common}/{branch}");
    }
}
```

`crates/rogg-app/src/about.rs`:

```rust
//! The About dialog's content (Refinements 1, A4): version, build details, data folders and credits.

use crate::paths::AppPaths;
use gpui_kit::{App, SharedString};
use rogg_ui::dialogs::AboutView;
use std::path::{Path, PathBuf};

pub const UNKNOWN: &str = "unknown";
pub const TAGLINE: &str = "A fast log explorer for macOS";
pub const BUILT_WITH: &str = "Built with Rust, gpui and gpui-kit";

pub struct BuildInfo {
    pub version: &'static str,
    pub commit: &'static str,
    pub date: &'static str,
    pub rustc: &'static str,
}

/// Build details captured by `build.rs`.
pub fn build_info() -> BuildInfo {
    describe(option_env!("ROGG_GIT_COMMIT"), option_env!("ROGG_BUILD_DATE"), option_env!("ROGG_RUSTC_VERSION"))
}

/// `unknown` for each detail the build could not find.
pub fn describe(commit: Option<&'static str>, date: Option<&'static str>, rustc: Option<&'static str>) -> BuildInfo {
    BuildInfo { version: env!("CARGO_PKG_VERSION"), commit: commit.unwrap_or(UNKNOWN), date: date.unwrap_or(UNKNOWN), rustc: rustc.unwrap_or(UNKNOWN) }
}

/// The main open-source crates and their licenses, copied from each crate's manifest on 2026-10-08.
pub const CREDITS: [(&str, &str); 14] = [
    ("gpui-kit", "Apache-2.0"),
    ("regex", "MIT OR Apache-2.0"),
    ("aho-corasick", "Unlicense OR MIT"),
    ("memchr", "Unlicense OR MIT"),
    ("memmap2", "MIT OR Apache-2.0"),
    ("rayon", "MIT OR Apache-2.0"),
    ("notify", "CC0-1.0"),
    ("flate2", "MIT OR Apache-2.0"),
    ("zip", "MIT"),
    ("tar", "MIT OR Apache-2.0"),
    ("encoding_rs", "(Apache-2.0 OR MIT) AND BSD-3-Clause"),
    ("serde", "MIT OR Apache-2.0"),
    ("serde_json", "MIT OR Apache-2.0"),
    ("toml", "MIT OR Apache-2.0"),
];

/// The folders the About dialog offers to show in Finder: the one holding settings, profiles, session, history and
/// recent files, then the cache. From `AppPaths`, so tests see their temp directories.
pub fn data_folders(cx: &App) -> Vec<(SharedString, PathBuf)> {
    let Some(paths) = cx.try_global::<AppPaths>() else { return vec![] };
    let support = paths.settings.parent().map(Path::to_path_buf).unwrap_or_default();
    vec![("Settings, profiles, session, history and recent files".into(), support), ("Cache".into(), paths.cache.clone())]
}

pub fn view(cx: &App) -> AboutView {
    let b = build_info();
    AboutView {
        version: format!("rogg {}", b.version).into(),
        tagline: TAGLINE.into(),
        build: vec![("Commit".into(), b.commit.into()), ("Built".into(), b.date.into()), ("Rust".into(), b.rustc.into())],
        data: data_folders(cx).into_iter().map(|(label, path)| (label, path.display().to_string().into())).collect(),
        built_with: BUILT_WITH.into(),
        credits: CREDITS.iter().map(|(n, l)| (SharedString::from(*n), SharedString::from(*l))).collect(),
    }
}
```

- [ ] **Step 4: The dialog builder in `rogg-ui`**

Append to `crates/rogg-ui/src/dialogs.rs` (import `crate::form::SectionLabel`):

```rust
/// The About dialog's content, prepared by the app.
#[derive(Clone, Debug, PartialEq)]
pub struct AboutView {
    /// `rogg 0.1.0`.
    pub version: SharedString,
    pub tagline: SharedString,
    /// (label, value): commit, build date, Rust.
    pub build: Vec<(SharedString, SharedString)>,
    /// (label, folder path); each row has a Show in Finder button.
    pub data: Vec<(SharedString, SharedString)>,
    pub built_with: SharedString,
    /// (crate, license).
    pub credits: Vec<(SharedString, SharedString)>,
}

/// About rogg (spec A4): header, Build, Data (with Show in Finder) and Credits sections.
pub fn about(view: &AboutView, on_reveal: IndexCallback) -> DialogFrame {
    let mut frame = DialogFrame::new("About rogg").child(AboutHeader { version: view.version.clone(), tagline: view.tagline.clone() });
    frame = frame.child(SectionLabel::new("Build"));
    for (i, (k, v)) in view.build.iter().enumerate() {
        frame = frame.child(RuleRow::text(format!("about-build-{i}"), k.clone()).trailing(v.clone()));
    }
    frame = frame.child(SectionLabel::new("Data"));
    for (i, (label, path)) in view.data.iter().enumerate() {
        frame = frame.child(DataRow { index: i, label: label.clone(), path: path.clone(), on_reveal: on_reveal.clone() });
    }
    frame = frame.child(SectionLabel::new("Credits")).child(AboutNote(view.built_with.clone()));
    for (i, (name, license)) in view.credits.iter().enumerate() {
        frame = frame.child(RuleRow::code(format!("about-credit-{i}"), name.clone()).trailing(license.clone()));
    }
    frame
}

#[derive(IntoElement)]
struct AboutHeader {
    version: SharedString,
    tagline: SharedString,
}

impl RenderOnce for AboutHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = cx.rogg().text_scale();
        let p = cx.rogg().palette;
        div()
            .id("about-header")
            .test_support()
            .flex()
            .flex_col()
            .gap(px(4.))
            .py(px(12.))
            .child(div().text_size(s.px(15.)).font_weight(FontWeight::MEDIUM).child(self.version))
            .child(div().text_size(s.px(12.)).text_color(p.muted).child(self.tagline))
    }
}

#[derive(IntoElement)]
struct AboutNote(SharedString);

impl RenderOnce for AboutNote {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div().pb(px(6.)).text_size(cx.rogg().text_scale().px(12.)).text_color(cx.rogg().palette.muted).child(self.0)
    }
}

/// A data folder: label, path (mono, truncated) and Show in Finder.
#[derive(IntoElement)]
struct DataRow {
    index: usize,
    label: SharedString,
    path: SharedString,
    on_reveal: IndexCallback,
}

impl RenderOnce for DataRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let s = t.text_scale();
        let p = t.palette;
        let (i, on_reveal) = (self.index, self.on_reveal);
        div()
            .id(SharedString::from(format!("about-data-{i}")))
            .test_support()
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(10.))
            .border_t_1()
            .border_color(p.border)
            .text_size(s.px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(3.))
                    .child(self.label)
                    .child(div().truncate().font_family(t.fonts.mono.clone()).text_size(s.px(11.)).text_color(p.muted).child(self.path)),
            )
            .child(ActionButton::new(SharedString::from(format!("about-reveal-{i}")), "Show in Finder").on_click(move |w, cx| on_reveal(i, w, cx)))
    }
}
```

(`ActionButton::new` takes the id the way `empty.rs` calls it. `SectionLabel` is in `crate::form`.)

- [ ] **Step 5: Wire it into the app**

1. `actions.rs`: add `ShowAbout` to the `actions!` list. `lib.rs`: `pub mod about;`.
2. `menus.rs`: the rogg menu starts `MenuItem::action("About rogg", ShowAbout), MenuItem::separator(), MenuItem::action("Settings…", OpenSettings), …`.
3. `workspace/mod.rs`:
   - Add `About` to `enum Dialog`, and the field `last_revealed: Option<PathBuf>` to `Workspace`, initialised to `None` in `Workspace::new`.
   - In `dialog_element`:

```rust
            Dialog::About => dialogs::about(&crate::about::view(cx), Rc::new(act_ix(this, |ws, i, _, cx| ws.reveal_data_folder(i, cx))))
                .on_close(close)
                .into_any_element(),
```

   - In `root()`: `.on_action(cx.listener(|ws, _: &ShowAbout, _, cx| ws.open_dialog(Dialog::About, cx)))`.
   - Methods:

```rust
    /// About › Show in Finder for data folder `index`. A missing folder is created first, so Finder has one to show.
    pub fn reveal_data_folder(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some((_, path)) = crate::about::data_folders(cx).into_iter().nth(index) {
            let _ = std::fs::create_dir_all(&path);
            self.reveal(path, cx);
        }
    }

    /// Shows `path` in Finder. Remembered for tests: the test platform's reveal does nothing.
    pub fn reveal(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        cx.reveal_path(&path);
        self.last_revealed = Some(path);
    }

    pub fn last_revealed(&self) -> Option<&Path> {
        self.last_revealed.as_deref()
    }
```

- [ ] **Step 6: Run the tests**

Run: `./dev test app about`, `./dev test app menus`, `./dev test ui dialogs`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-app crates/rogg-ui
git commit -m "feat(app): About dialog with build details, data folders and credits"
```

### Task 4: Iteration A gate and hand-off

**Files:**
- Create: `docs/superpowers/notes/2026-10-08-refinements-1-manual-checks.md`
- Modify (only after the user approves): `tools/visual-diff/golden/*`

- [ ] **Step 1: Run the gate without the visual check**

Run: `./dev check --no-visual`
Expected: build, lint and test all ✓.

- [ ] **Step 2: Run the visual check and expect the intended differences**

Run: `./dev visual-check`
Expected: FAIL only on pages showing the changed glyphs and labels: the title bar, the sidebar folder rows, the info bar's Profile button, the filter band's History button, and the controls page. Then run `./dev visual` and open `target/visual/report.html` to see the diffs.

Any difference *outside* those places is a regression: fix it before continuing.

- [ ] **Step 3: Write the manual checks**

Create `docs/superpowers/notes/2026-10-08-refinements-1-manual-checks.md` with an "Iteration A" section. Each item is something the user does in `./dev demo`:
- The folder chevron turns when a folder is opened and closed.
- The all-tabs chevron points down.
- `Profile: Services` and `History` have no `⌄`.
- Edit (✎), ⌘L and Enter leave the band's height unchanged.
- rogg ▸ About rogg shows the version, the commit and the date. Both Show in Finder buttons open Finder at the right folder.

- [ ] **Step 4: Commit the notes**

```bash
git add docs/superpowers/notes/2026-10-08-refinements-1-manual-checks.md
git commit -m "docs: refinements 1 manual checks (iteration A)"
```

- [ ] **Step 5: Hand off to the user (stop here)**

Tell the user:
- what changed;
- the visual report path and which pages changed, and why;
- that `./dev demo` lets them try it.

Ask the user:
- (1) to approve the new captures. On approval, run `./dev visual-approve` (it asks for confirmation) and commit `tools/visual-diff/golden` with `test(visual): re-approve goldens for refinements 1 iteration A`;
- (2) how to integrate: merge locally / keep the branch.

Merge only on their choice: `git merge --ff-only refinements-1a` from `main`.

---

# Iteration B — Log view: long rows, full line, partial selection (branch `refinements-1b`)

Setup (controller): enter a worktree `refinements-1b` from the updated `main`, run `./dev setup`, then run `./dev check` for the baseline.

### Task 5: Wrapped rows show at most 3 lines (spec B1)

**Files:**
- Modify:
  - `crates/rogg-ui/src/logview/element.rs` (`Shaped`, `RowBody::Line`, `shape_row`, `body_height`, `prepaint`, `paint`, `LogViewPrepaint`);
  - `crates/rogg-ui/src/logview/state.rs` (`RowGeometry`).
- Test:
  - `crates/rogg-ui/tests/logview_real_text.rs`, which uses CoreText and so measures real wraps;
  - the existing `crates/rogg-ui/tests/logview.rs` must still pass.

**Interfaces:**
- Produces:
  - `pub const MAX_WRAPPED_LINES: usize = 3` in `logview::element`, re-exported from `rogg_ui::logview`;
  - `RowGeometry { row, bounds, is_gap, clipped }`, where `clipped` is true when a wrapped row hides lines.

- [ ] **Step 1: Write the failing test**

In `crates/rogg-ui/tests/logview_real_text.rs`, add the case below and register it in `main`'s `cases` (the array grows to 4):

```rust
/// Spec B1: a long wrapped row is cut to 3 visual lines and marked clipped; a short wrapped row keeps its height.
fn long_wrapped_rows_show_three_lines(cx: &mut HeadlessAppContext) {
    use rogg_ui::theme::ActiveRoggTheme;
    let (w, state) = open_state(cx, vec![line(1, "word ".repeat(400), vec![]), line(2, "two words", vec![])], true);
    render(cx, w);
    render(cx, w);
    let row_h = cx.update(|cx| f32::from(cx.rogg().metrics.row_height));
    let rows = cx.update(|cx| state.read(cx).last_rows().to_vec());
    assert_eq!(f32::from(rows[0].bounds.size.height), row_h * 3., "capped at 3 lines");
    assert!(rows[0].clipped);
    assert_eq!(f32::from(rows[1].bounds.size.height), row_h, "a short row keeps one line");
    assert!(!rows[1].clipped);
    assert_eq!(rows[1].bounds.origin.y, rows[0].bounds.bottom(), "the next row starts right below the capped one");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `./dev test ui --test logview_real_text long_wrapped`
Expected: compile error, because `clipped` does not exist yet. Once it compiles, it fails on the height: about 10 lines.

- [ ] **Step 3: Implement the cap**

In `element.rs`:

```rust
/// Wrapped rows show at most this many visual lines; the rest is clipped and the last one is marked with `…`.
pub const MAX_WRAPPED_LINES: usize = 3;

impl Shaped {
    /// Visual lines after wrapping (1 for an unwrapped line).
    fn visual_lines(&self) -> usize {
        match self {
            Shaped::Line(_) => 1,
            Shaped::Wrapped(lines) => lines.iter().map(|l| l.wrap_boundaries().len() + 1).sum::<usize>().max(1),
        }
    }
}
```

The other changes in `element.rs`:
- **`RowBody::Line`:** gains `clipped: bool`. Set it at the end of `shape_row`: `let clipped = text.visual_lines() > MAX_WRAPPED_LINES;`.
- **`body_height`:** for `Shaped::Wrapped`, use `h.clamp(f32::from(row_h), f32::from(row_h) * MAX_WRAPPED_LINES as f32)`.
- **`RowGeometry`** (in `state.rs`): gains `pub clipped: bool`, documented as "a wrapped row cut to `MAX_WRAPPED_LINES`".
- **`prepaint`:** fills `clipped` from the body. Line rows use their flag, and gap rows use `false`.
- **`prepaint`:** shapes the marker once and stores it in `LogViewPrepaint` as `ellipsis: ShapedLine`:

```rust
        let ellipsis = window.text_system().shape_line("…".into(), theme.metrics.log_font_size, &[run('…'.len_utf8(), &fonts.log, theme.palette.faint, None)], None);
```

- **`paint`:** inside the row's text clip, after `text.paint(..)`. The marker sits in the 20 px right padding (`LINE_PAD_R`) beside the last shown line:

```rust
                            if *clipped && let Some(w) = pre.wrap_width {
                                let at = point(pre.text_left + w + px(4.), row.bounds.top() + row_h * (MAX_WRAPPED_LINES - 1) as f32);
                                pre.ellipsis.paint(at, row_h, TextAlign::Left, None, window, cx).ok();
                            }
```

The row's clip mask already ends at `row.bounds.bottom()`, so lines past the third are not painted. Heights recorded through `record_heights` are the capped ones, so scrolling and hit-testing follow.

Re-export `MAX_WRAPPED_LINES` from `logview/mod.rs`.

- [ ] **Step 4: Run the tests**

Run: `./dev test ui`
Expected: PASS. That includes `logview_real_text` (all 4 cases) and the existing `wrapped_line_with_hits_across_wrap_boundaries_paints` (its height is still a multiple of 23).

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): wrapped rows show at most 3 lines, the last ending with a faint …"
```

### Task 6: "Show full line…" (spec B2)

**Files:**
- Modify:
  - `crates/rogg-ui/src/logview/state.rs` (`LogViewEvent::ShowFullLine`);
  - `crates/rogg-ui/src/logview/mod.rs` (the context menu);
  - `crates/rogg-ui/src/dialogs.rs` (`DialogFrame::width`, `full_line`, `full_line_shown`);
  - `crates/rogg-ui/src/theme/metrics.rs`;
  - `crates/rogg-app/src/tab.rs` (`on_log_event`, `TabEvent::ShowFullLine`);
  - `crates/rogg-app/src/workspace/mod.rs` (`Dialog::FullLine`, `open_full_line`, `full_line_dialog`).
- Test:
  - `crates/rogg-ui/src/dialogs.rs` (unit);
  - `crates/rogg-ui/tests/logview.rs` (exhaustive match update);
  - `crates/rogg-app/tests/full_line.rs` (new).

**Interfaces:**
- Produces:
  - `LogViewEvent::ShowFullLine { number: u64 }`;
  - `TabEvent::ShowFullLine { number: u64, text: SharedString }`;
  - `rogg_ui::dialogs::{full_line(number, view, cut, on_copy), full_line_shown(text) -> (&str, bool), FULL_LINE_SHOWN_BYTES}`;
  - `DialogFrame::width(f32)`;
  - `Workspace::{open_full_line(number, text, window, cx), full_line_dialog() -> Option<(u64, SharedString)>}`;
  - the element ids `full-line-text` and `full-line-copy`.

- [ ] **Step 1: Write the failing tests**

Unit tests at the end of `crates/rogg-ui/src/dialogs.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{FULL_LINE_SHOWN_BYTES, full_line_shown};

    #[test]
    fn the_full_line_dialog_shows_at_most_1_mb_on_a_char_boundary() {
        assert_eq!(full_line_shown("short"), ("short", false));
        let long = "é".repeat(FULL_LINE_SHOWN_BYTES); // 2 MB of 2-byte chars
        let (shown, cut) = full_line_shown(&long);
        assert!(cut && shown.len() <= FULL_LINE_SHOWN_BYTES && shown.len() > FULL_LINE_SHOWN_BYTES - 2);
        assert!(long.is_char_boundary(shown.len()));
    }
}
```

`crates/rogg-app/tests/full_line.rs`:

```rust
mod common;
use gpui_kit::{AppContext, TestAppContext};
use rogg_app::args::Args;
use rogg_ui::logview::LogViewEvent;

#[gpui_kit::test]
fn show_full_line_opens_the_whole_line_and_copy_copies_it(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let long = format!("start {} end", "x".repeat(70 * 1024));
    let log = common::write_log(dir.path(), "a.log", &["short", &long]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    let full = tab.read_with(cx, |t, _| t.full.clone());
    full.update(cx, |_, cx| cx.emit(LogViewEvent::ShowFullLine { number: 2 }));
    cx.run_until_parked();
    assert!(common::exists(cx, w, "full-line-text"));
    let (number, text) = ws.read_with(cx, |ws, _| ws.full_line_dialog()).expect("the dialog is open");
    assert_eq!((number, text.len()), (2, long.len()), "the whole line, past the 64 KB display cut");
    common::click(cx, w, "full-line-copy");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).map(|t| t.len()), Some(long.len()));
}
```

(If line numbers in this workspace start at 0 rather than 1, use the number of the second line. Check `number_base()` for a plain file. The test must address the 70 KB line.)

In `crates/rogg-ui/tests/logview.rs`, the exhaustive `match` in `click_selects_and_gutter_click_toggles_pin` gains `LogViewEvent::ShowFullLine { number } => format!("full {number}"),`.

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev test ui dialogs` and `./dev test app full_line`
Expected: compile errors (no `ShowFullLine`, no `full_line_shown`).

- [ ] **Step 3: Implement the log view side**

- **`state.rs`:** add a variant to `LogViewEvent`:

```rust
    /// Context menu "Show full line…": open the whole line in a dialog.
    ShowFullLine { number: u64 },
```

- **`mod.rs` `context_menu`:** add an item after "Copy with line numbers", before the separator. It is always present on a line row:

```rust
        .item(PopupMenuItem::new("Show full line…").on_click(move |_, _, cx| emit(&s6, LogViewEvent::ShowFullLine { number }, cx)))
```

  Add `s6` to the clones: `let (s1, s2, s3, s6) = (state.clone(), state.clone(), state.clone(), state.clone());`.

- [ ] **Step 4: Implement the dialog builder**

In `metrics.rs` (`size`):

```rust
    pub const FULL_LINE_DIALOG_W: f32 = 760.;
    pub const FULL_LINE_TEXT_H: f32 = 360.;
```

In `dialogs.rs`:
- **`DialogFrame`:** gains `width: f32` (default `size::DIALOG_W`) and the builder `pub fn width(mut self, w: f32) -> Self`. `render` uses `.w(px(self.width))`.
- Then add:

```rust
/// Bytes of a line the full-line dialog shows; Copy still copies the whole line.
pub const FULL_LINE_SHOWN_BYTES: usize = 1024 * 1024;

/// The part of `text` the full-line dialog shows (at most 1 MB, cut on a char boundary), and whether it was cut.
pub fn full_line_shown(text: &str) -> (&str, bool) {
    let end = text.floor_char_boundary(FULL_LINE_SHOWN_BYTES.min(text.len()));
    (&text[..end], end < text.len())
}

/// "Show full line…" (spec B2): the whole line, read-only, selectable and wrapped, in the log font, with Copy.
pub fn full_line(number: u64, view: Entity<TextareaState>, cut: bool, on_copy: Callback) -> DialogFrame {
    let mut frame = DialogFrame::new(format!("Line {number}")).width(size::FULL_LINE_DIALOG_W).child(FullLineText { view });
    if cut {
        frame = frame.child(AboutNote("The line is longer than 1 MB: the first 1 MB is shown. Copy copies the whole line.".into()));
    }
    frame.child(div().flex().justify_end().pt(px(14.)).child(ActionButton::new("full-line-copy", "Copy").primary().on_click(move |w, cx| on_copy(w, cx))))
}

#[derive(IntoElement)]
struct FullLineText {
    view: Entity<TextareaState>,
}

impl RenderOnce for FullLineText {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        div()
            .id("full-line-text")
            .test_support()
            .mt(px(12.))
            .h(px(size::FULL_LINE_TEXT_H))
            .p(px(8.))
            .bg(t.palette.bg)
            .border_1()
            .border_color(t.palette.border)
            .rounded(px(5.))
            .font_family(t.fonts.log.clone())
            .text_size(t.metrics.log_font_size)
            .text_color(t.palette.log_text)
            .child(Textarea::new(&self.view).readonly(true).appearance(false).h(relative(1.)))
    }
}
```

Import `gpui_kit::component::input::{Textarea, TextareaState}` and `relative`. `Textarea` wraps by default. If it does not pick up the parent's font, set the font on the `TextareaState`'s style the way `Input` does in `filter.rs`.

- [ ] **Step 5: Route it through the app**

- **`tab.rs`:** add `ShowFullLine { number: u64, text: SharedString }` to `TabEvent`.
- **`tab.rs` `on_log_event`:** add an arm. It uses the pane's own row source, whose `copy_text` is the whole line:

```rust
            LogViewEvent::ShowFullLine { number } => {
                let state = if pane == Pane::Full { &self.full } else { &self.filtered };
                let rows = state.read(cx).rows().clone();
                if let Some(text) = rows.row_for_number(*number).and_then(|row| rows.copy_text(row)) {
                    cx.emit(TabEvent::ShowFullLine { number: *number, text: text.into() });
                }
            }
```

- **`workspace/mod.rs`:**
  - Add `FullLine { number: u64, text: SharedString, view: Entity<TextareaState>, cut: bool }` to `Dialog`.
  - In the tab-event handler, add `TabEvent::ShowFullLine { number, text } => self.open_full_line(*number, text.clone(), window, cx),`.
  - Add these methods:

```rust
    /// "Show full line…": opens the line in a read-only text box (at most 1 MB shown) with Copy.
    pub fn open_full_line(&mut self, number: u64, text: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let (shown, cut) = rogg_ui::dialogs::full_line_shown(&text);
        let shown = SharedString::from(shown.to_string());
        let view = cx.new(|cx| TextareaState::new(window, cx).default_value(shown));
        self.open_dialog(Dialog::FullLine { number, text, view, cut }, cx);
    }

    /// The open full-line dialog's line number and whole text (tests).
    pub fn full_line_dialog(&self) -> Option<(u64, SharedString)> {
        match &self.dialog {
            Some(Dialog::FullLine { number, text, .. }) => Some((*number, text.clone())),
            _ => None,
        }
    }
```

  - In `dialog_element`:

```rust
            Dialog::FullLine { number, text, view, cut } => {
                let whole = text.clone();
                dialogs::full_line(*number, view.clone(), *cut, Rc::new(move |_, cx: &mut App| cx.write_to_clipboard(ClipboardItem::new_string(whole.to_string()))))
                    .on_close(close)
                    .into_any_element()
            }
```

- [ ] **Step 6: Run the tests**

Run: `./dev test ui` and `./dev test app`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-ui crates/rogg-app
git commit -m "feat: Show full line… opens the whole line in a dialog with Copy"
```

### Task 7: Character selection: state, copy and lifetime (spec B3, part 1)

**Files:**
- Modify: `crates/rogg-ui/src/logview/state.rs`, `crates/rogg-ui/src/logview/mod.rs` (re-export)
- Test: `crates/rogg-ui/tests/logview.rs`

**Interfaces:**
- Produces:
  - `pub struct CharSelection { pub number: u64, pub anchor: usize, pub head: usize }` with `fn range(&self) -> Range<usize>`, re-exported from `rogg_ui::logview`;
  - `LogViewState::{char_selection() -> Option<CharSelection>, set_char_selection(number, anchor, head, cx), clear_char_selection(cx)}`.
- Offsets are byte offsets into the row's display text. The setter snaps them to char boundaries and stops them at the display cut.

- [ ] **Step 1: Write the failing tests**

Append to `crates/rogg-ui/tests/logview.rs` (import `CharSelection` if needed):

```rust
#[gpui_kit::test]
fn a_char_selection_copies_its_characters_and_clears_on_other_selections(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(20, None))); // row i shows "line {i}", numbered 1000 + i
    state.update(cx, |s, cx| {
        s.set_selected(Some(1003), cx);
        s.set_char_selection(1003, 5, 2, cx);
    });
    state.read_with(cx, |s, _| {
        assert_eq!(s.char_selection().map(|c| c.range()), Some(2..5));
        assert_eq!(s.selected_text(false).as_deref(), Some("ne "), "Copy takes just the characters");
        assert_eq!(s.selected_text(true).as_deref(), Some("1003\tline 3\n"), "with line numbers copies whole rows");
    });
    state.update(cx, |s, cx| s.move_selection(1, false, cx));
    assert_eq!(state.read_with(cx, |s, _| s.char_selection()), None, "a keyboard move clears it");
    state.update(cx, |s, cx| {
        s.set_char_selection(1004, 0, 4, cx);
        s.set_rows(rows(20, None), cx);
    });
    assert_eq!(state.read_with(cx, |s, _| s.char_selection()), None, "new rows (a filter change) clear it");
    state.update(cx, |s, cx| {
        s.set_char_selection(1004, 0, 4, cx);
        s.set_selected(Some(1004), cx);
    });
    assert_eq!(state.read_with(cx, |s, _| s.char_selection()), None, "a click (plain selection) clears it");
    state.update(cx, |s, cx| s.set_char_selection(1004, 3, 3, cx));
    assert_eq!(state.read_with(cx, |s, _| s.char_selection()), None, "an empty range is no selection");
}

#[gpui_kit::test]
fn char_selection_snaps_to_char_boundaries(cx: &mut TestAppContext) {
    common::setup(cx);
    // "a" 0..1, "é" 1..3, "日" 3..6, "b" 6..7
    let line = LogRow::Line(LogLine { number: 1, text: "aé日b".into(), spans: vec![], pinned: false, context: false, tint: None });
    let state = cx.new(|_| LogViewState::new(Rc::new(VecRows::new(vec![line]))));
    state.update(cx, |s, cx| {
        s.set_selected(Some(1), cx);
        s.set_char_selection(1, 2, 5, cx);
    });
    state.read_with(cx, |s, _| {
        assert_eq!(s.char_selection().map(|c| c.range()), Some(1..3));
        assert_eq!(s.selected_text(false).as_deref(), Some("é"));
    });
}

#[gpui_kit::test]
fn a_char_selection_stops_at_the_display_cut(cx: &mut TestAppContext) {
    common::setup(cx);
    let text = "x".repeat(300 * 1024); // longer than the display cut, which ends in `…`
    let line = LogRow::Line(LogLine { number: 1, text: text.into(), spans: vec![], pinned: false, context: false, tint: None });
    let state = cx.new(|_| LogViewState::new(Rc::new(VecRows::new(vec![line]))));
    state.update(cx, |s, cx| {
        s.set_selected(Some(1), cx);
        s.set_char_selection(1, 0, usize::MAX, cx);
    });
    let copied = state.read_with(cx, |s, _| s.selected_text(false)).unwrap();
    assert!(copied.chars().all(|c| c == 'x'), "never the … marker");
    assert_eq!(copied.len(), rogg_ui::logview::text::MAX_DISPLAY_TEXT_BYTES, "everything up to the cut");
}
```

(If `MAX_DISPLAY_TEXT_BYTES` is not on a char boundary for this text it still is here, since the text is all ASCII. If `text` is not a public module path, use the re-export the crate offers.)

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev test ui char_selection` and `./dev test ui a_char_selection`
Expected: compile errors (no `set_char_selection`).

- [ ] **Step 3: Implement**

In `state.rs`:

```rust
/// Characters selected within one row by a drag: byte offsets into the row's display text, on char boundaries and
/// within the display cut. `anchor` is where the drag started, `head` where the pointer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharSelection {
    pub number: u64,
    pub anchor: usize,
    pub head: usize,
}

impl CharSelection {
    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}
```

The other changes in `state.rs`:
- **The field:** `pub(crate) char_selection: Option<CharSelection>` (initialised to `None`).
- **Clearing:** `set_selected`, `extend_selection` and `set_rows` each set it to `None`. `extend_rows` keeps it, because the row is still there.
- **New methods:**

```rust
    pub fn char_selection(&self) -> Option<CharSelection> {
        self.char_selection
    }

    /// Selects characters `anchor..head` of line `number` (a drag within one row). Offsets snap back to char boundaries
    /// and stop at the display cut (never inside the `…` marker); an empty range clears the selection.
    pub fn set_char_selection(&mut self, number: u64, anchor: usize, head: usize, cx: &mut Context<Self>) {
        let Some(LogRow::Line(line)) = self.rows.row_for_number(number).map(|row| self.rows.row(row)) else { return };
        let (display, truncated) = display_text(&line.text, MAX_DISPLAY_TEXT_BYTES);
        let cut = if truncated { display.len() - '…'.len_utf8() } else { display.len() };
        let snap = |i: usize| display.floor_char_boundary(i.min(cut));
        let (anchor, head) = (snap(anchor), snap(head));
        self.char_selection = (anchor != head).then_some(CharSelection { number, anchor, head });
        cx.notify();
    }

    pub fn clear_char_selection(&mut self, cx: &mut Context<Self>) {
        if self.char_selection.take().is_some() {
            cx.notify();
        }
    }
```

- **`selected_text`:** a new first branch:

```rust
        if !with_numbers
            && let Some(c) = self.char_selection
            && self.selection_range() == Some((c.number, c.number))
        {
            // The display text's kept prefix has the same bytes as the line (control chars become spaces one for one),
            // so the offsets index the full line too.
            let text = self.rows.copy_text(self.rows.row_for_number(c.number)?)?;
            let r = c.range();
            let end = text.floor_char_boundary(r.end.min(text.len()));
            return Some(text[text.floor_char_boundary(r.start.min(end))..end].to_string());
        }
```

Imports: `super::model::LogRow`, `super::text::{MAX_DISPLAY_TEXT_BYTES, display_text}` and `std::ops::Range`. Re-export `CharSelection` from `logview/mod.rs`.

- [ ] **Step 4: Run the tests**

Run: `./dev test ui logview`
Expected: PASS (new and existing).

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): character selection state; Copy takes the selected characters"
```

### Task 8: Character selection: mouse and paint (spec B3, part 2)

**Files:**
- Modify: `crates/rogg-ui/src/logview/element.rs`, `crates/rogg-ui/src/logview/state.rs` (`Drag::Select`)
- Test: `crates/rogg-ui/tests/logview_real_text.rs`

**Interfaces:**
- Consumes: `CharSelection`, `set_char_selection` and `clear_char_selection` from Task 7.
- Produces: `Drag::Select { chars: Option<(u64, usize)> }`, which is crate-private.
  - It holds the start row's number and the byte index where the press landed.
  - It is `None` once the drag has left that row, and for shift-clicks and presses in the gutter.

- [ ] **Step 1: Write the failing tests**

In `crates/rogg-ui/tests/logview_real_text.rs`, add these cases and register them in `main`:

```rust
fn press_move_release(cx: &mut HeadlessAppContext, w: AnyWindowHandle, from: Point<gpui_kit::Pixels>, to: Point<gpui_kit::Pixels>) {
    use gpui_kit::{InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.dispatch_event(MouseMoveEvent { position: from, pressed_button: None, modifiers: Default::default() }.to_platform_input(), cx);
        window.render_frame(cx);
        window.dispatch_event(MouseDownEvent { button: MouseButton::Left, position: from, modifiers: Default::default(), click_count: 1, first_mouse: false }.to_platform_input(), cx);
        window.render_frame(cx);
        if to != from {
            window.dispatch_event(MouseMoveEvent { position: to, pressed_button: Some(MouseButton::Left), modifiers: Default::default() }.to_platform_input(), cx);
            window.render_frame(cx);
        }
        window.dispatch_event(MouseUpEvent { button: MouseButton::Left, position: to, modifiers: Default::default(), click_count: 1 }.to_platform_input(), cx);
        window.render_frame(cx);
    })
    .unwrap();
}

/// Spec B3: a drag inside one row selects characters (wrapped or not); crossing rows selects whole rows; a click clears.
fn dragging_within_a_row_selects_characters(cx: &mut HeadlessAppContext) {
    use gpui_kit::point;
    use rogg_ui::theme::size::{GUTTER_NUMBER_W, GUTTER_PIN_W};
    let left = GUTTER_PIN_W + GUTTER_NUMBER_W;
    let text = "alpha beta gamma";
    for wrap in [false, true] {
        let (w, state) = open_state(cx, vec![line(1, text, vec![]), line(2, "second row", vec![])], wrap);
        render(cx, w);
        let y1 = 4. + 11.;
        press_move_release(cx, w, point(px(left + 1.), px(y1)), point(px(left + 40.), px(y1)));
        let (sel, copied) = cx.update(|cx| (state.read(cx).char_selection(), state.read(cx).selected_text(false)));
        let range = sel.expect("characters selected").range();
        assert!(range.start == 0 && range.end > 0 && range.end < text.len(), "wrap={wrap}: {range:?}");
        assert_eq!(copied.as_deref(), Some(&text[range.clone()]), "wrap={wrap}");
        // Past the end of the text selects to the end.
        press_move_release(cx, w, point(px(left + 1.), px(y1)), point(px(850.), px(y1)));
        assert_eq!(cx.update(|cx| state.read(cx).char_selection().map(|c| c.range())), Some(0..text.len()), "wrap={wrap}");
        // Into the next row: whole rows.
        let row2_top = cx.update(|cx| state.read(cx).last_rows()[1].bounds.origin.y);
        press_move_release(cx, w, point(px(left + 1.), px(y1)), point(px(left + 20.), row2_top + px(5.)));
        let (sel, range) = cx.update(|cx| (state.read(cx).char_selection(), state.read(cx).selection_range()));
        assert_eq!((sel, range), (None, Some((1, 2))), "wrap={wrap}");
        // A click clears any character selection.
        press_move_release(cx, w, point(px(left + 1.), px(y1)), point(px(left + 40.), px(y1)));
        press_move_release(cx, w, point(px(left + 20.), px(y1)), point(px(left + 20.), px(y1)));
        assert_eq!(cx.update(|cx| state.read(cx).char_selection()), None, "wrap={wrap}");
    }
}
```

Add `Point` to the file's imports.

- [ ] **Step 2: Run it to verify it fails**

Run: `./dev test ui --test logview_real_text dragging_within`
Expected: FAIL, with "characters selected" panicking on `None`.

- [ ] **Step 3: Implement the mouse handling**

In `element.rs`:

1. **`TextTarget`.** Rename `MenuTarget` to `TextTarget` and build the list once, before the mouse handlers. Share it as `Rc<Vec<TextTarget>>` between the left-press, move and right-click handlers. It holds `hitbox`, `row`, `number`, `pinned`, `display` and `origin`, as today.
2. **`closest_index`.** Add it next to `index_at`:

```rust
/// Byte index in `display` nearest to `at` (relative to the text origin): past the end of a line gives its end.
#[allow(clippy::too_many_arguments)]
fn closest_index(display: &SharedString, at: Point<Pixels>, font: &Font, font_size: Pixels, wrap: Option<Pixels>, row_h: Pixels, color: Color, window: &mut Window) -> usize {
    if display.is_empty() {
        return 0;
    }
    let runs = [run(display.len(), font, color, None)];
    let index = match wrap {
        None => window.text_system().shape_line(display.clone(), font_size, &runs, None).closest_index_for_x(at.x),
        Some(w) => window
            .text_system()
            .shape_text(display.clone(), font_size, &runs, Some(w), None)
            .ok()
            .and_then(|lines| lines.first().map(|l| l.closest_index_for_position(at, row_h).unwrap_or_else(|i| i)))
            .unwrap_or(0),
    };
    display.floor_char_boundary(index.min(display.len()))
}
```

(Check the exact names on gpui-pre 0.3.8's `LineLayout` and `WrappedLineLayout`. `closest_index_for_x` and `closest_index_for_position` are the closest-hit variants of the `index_for_*` calls that `index_at` uses today.)

3. **Left press on a row's text** (not the gutter):
   - **Without shift:** after `set_selected` (which clears any character selection), compute `anchor = closest_index(&t.display, e.position - t.origin, …)` and set `s.drag = Some(Drag::Select { chars: Some((number, anchor)) })`.
   - **With shift:** `Drag::Select { chars: None }`.
   - **Press left of `text_left`:** that is the number gutter, so `chars: None`.
4. **Move with `Drag::Select { chars }`.** Find the hovered target, then:
   - **Same row as the start** (`chars == Some((n, anchor))` and the target's number is `n`): `head = closest_index(..)`, then `s.set_char_selection(n, anchor, head, cx)`. Do not emit `Selected`.
   - **Another row:**
     - set `s.drag = Some(Drag::Select { chars: None })`;
     - call `s.clear_char_selection(cx)`;
     - then do today's `extend_selection` / emit `Selected`, as before.
   - **Above or below the panel:** as today. `autoscroll_select` extends rows, so set `chars: None` first.
5. **`Drag::Select` patterns** elsewhere in the file become `Drag::Select { .. }`.

Never set a cursor style: the pointer stays the arrow.

- [ ] **Step 4: Implement the paint**

1. **`LogViewPrepaint`** gains `char_selection: Option<CharSelection>`, read from the state in `prepaint` together with `selection`.
2. **Row background** in `paint`:

```rust
                        let chars = pre.char_selection.filter(|c| c.number == *number).map(|c| c.range());
                        // A row with selected characters is not painted as selected: the characters carry the selection colour.
                        let bg = if selected && chars.is_none() { Some(p.selection) } else if let Some(t) = tint { … } else if hovered { … } else { None };
```

   (The accent bar for `selected` stays.)
3. **Text clip.** Inside the row's text clip, compute `geometry` also when `chars` is set, i.e. `(!marks.is_empty() || !outlines.is_empty() || chars.is_some())`. Paint the character boxes *before* the hit marks, using the full visual-line height:

```rust
                            if let (Some(range), Some((geometry, _))) = (&chars, &geometry) {
                                geometry.boxes(range, |(visual, x0, x1)| {
                                    let b = Bounds::new(point(origin.x + x0, origin.y + row_h * visual as f32), gsize(x1 - x0, row_h));
                                    if b.intersects(&visible) {
                                        window.paint_quad(fill(b, p.selection));
                                    }
                                });
                            }
```

- [ ] **Step 5: Run the tests**

Run: `./dev test ui`
Expected: PASS: all `logview` tests (drag over rows still extends, shift-click still extends) and every `logview_real_text` case.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): drag within a row selects characters; across rows selects whole rows"
```

### Task 9: Iteration B gate and hand-off

- [ ] **Step 1:** Run `./dev check`.
  - Expected: green, including visual-check. The replica has no row longer than 3 wrapped lines and no character selection.
  - If the visual check differs: run `./dev visual`. A difference caused by B1 (a replica row that now caps) is intended and goes to the user with the report. Anything else is a regression to fix.
- [ ] **Step 2:** Add an "Iteration B" section to `docs/superpowers/notes/2026-10-08-refinements-1-manual-checks.md`. Check in `./dev demo` with Wrap on, plus a real log with very long lines:
  - a long line shows 3 lines and `…`;
  - right-click shows `Show full line…`, and the dialog shows and copies the line;
  - a drag inside a line selects characters, and ⌘C pastes them;
  - a drag across lines selects rows;
  - a click, an arrow key or a new filter clears the character selection;
  - the pointer stays the arrow throughout.
  
  Commit as `docs: refinements 1 manual checks (iteration B)`.
- [ ] **Step 3: Hand off (stop here).** Summarise the changes, invite the user to `./dev demo`, and ask: merge locally / keep the branch. Merge only on their choice: `git merge --ff-only refinements-1b` from `main`. If goldens changed intentionally, run `./dev visual-approve` only after the user's OK.

---

# Iteration C — Colours and recent filters (branch `refinements-1c`)

Setup (controller): enter a worktree `refinements-1c` from the updated `main`, run `./dev setup`, then run `./dev check` for the baseline.

### Task 10: Any colour in profiles (spec C1)

**Files:**
- Modify:
  - `crates/rogg-ui/src/theme/color.rs` (`Color::from_hsla`);
  - `crates/rogg-ui/src/form.rs` (`CustomColor`);
  - `crates/rogg-app/src/dialogs/profiles.rs`.
- Test:
  - `crates/rogg-ui/src/theme/color.rs` (unit);
  - `crates/rogg-ui/tests/form.rs`;
  - `crates/rogg-app/tests/profile_editor.rs`.

**Interfaces:**
- Produces:
  - `Color::from_hsla(Hsla) -> Color`;
  - `rogg_ui::form::CustomColor::new(id, &Entity<ColorPickerState>).featured(Vec<Color>)`;
  - `ProfileEditor::{rule_pickers() -> Option<[Entity<ColorPickerState>; 2]>, slot_pickers() -> Option<[Entity<ColorPickerState>; 4]>}`.
  - The rule pickers are colour and light override. The slot pickers are fg, bg, light fg and light bg.
- Produces element ids: `rule-custom`, `rule-light-custom`, `slot-fg-custom`, `slot-bg-custom`, `slot-light-fg-custom`, `slot-light-bg-custom`.

- [ ] **Step 1: Write the failing tests**

`color.rs` unit test:

```rust
    #[test]
    fn from_hsla_round_trips_8_bit_colours() {
        for c in [0xff0000ff, 0xffffffff, 0x000000ff, 0x336699ff, 0x3fa7ff80, 0x00000000] {
            assert_eq!(Color::from_hsla(Color(c).hsla()), Color(c), "{c:#010x}");
        }
    }
```

`crates/rogg-ui/tests/form.rs`:

```rust
#[gpui_kit::test]
fn custom_colour_renders_a_picker_trigger(cx: &mut TestAppContext) {
    use gpui_kit::component::color_picker::ColorPickerState;
    use rogg_ui::form::CustomColor;
    use rogg_ui::theme::Color;
    common::setup(cx);
    let slot: Rc<RefCell<Option<gpui_kit::Entity<ColorPickerState>>>> = Rc::default();
    let w = common::open(cx, 400., 200., move |window, cx| {
        let state = slot.borrow_mut().get_or_insert_with(|| cx.new(|cx| ColorPickerState::new(window, cx))).clone();
        CustomColor::new("rule-custom", &state).featured(vec![Color::rgb(0xff0000)]).into_any_element()
    });
    assert!(common::exists(cx, w, "rule-custom"));
}
```

(Adjust the `ColorPickerState` path to where gpui-kit re-exports it. `gpui_kit::component::color_picker` re-exports `gpui_base::ColorPickerState`.)

`crates/rogg-app/tests/profile_editor.rs`:

```rust
#[gpui_kit::test]
fn custom_colours_are_written_as_hex_and_applied(cx: &mut TestAppContext) {
    use gpui_kit::rgba;
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-duplicate"); // "Default copy": an editable profile with rules
    let name = f.editor.read_with(cx, |e, _| e.profile_name().to_string());
    cx.update_window(f.w, |_, window, cx| f.editor.update(cx, |e, cx| e.select_rule(Some(0), window, cx))).unwrap();
    let [color, light] = f.editor.read_with(cx, |e, _| e.rule_pickers()).expect("the rule form has pickers");
    cx.update_window(f.w, |_, window, cx| color.update(cx, |p, cx| p.select_color(rgba(0x336699ff).into(), window, cx))).unwrap();
    cx.update_window(f.w, |_, window, cx| light.update(cx, |p, cx| p.select_color(rgba(0x33669980).into(), window, cx))).unwrap();
    cx.run_until_parked();
    let rule = cx.update(|cx| cx.global::<ProfileStore>().get(&name).rules[0].clone());
    assert_eq!(rule.color.0, "#336699");
    assert_eq!(rule.light.map(|l| l.0).as_deref(), Some("#33669980"), "alpha below 1 is kept as #rrggbbaa");
    let (_, color_input, _) = f.editor.read_with(cx, |e, _| e.rule_inputs()).unwrap();
    assert_eq!(color_input.read_with(cx, |i, _| i.value().to_string()), "#336699", "the hex field shows the picked colour");

    cx.update_window(f.w, |_, window, cx| f.editor.update(cx, |e, cx| e.select_slot(Some(0), window, cx))).unwrap();
    let [fg, bg, _, light_bg] = f.editor.read_with(cx, |e, _| e.slot_pickers()).expect("the slot form has pickers");
    for (picker, value) in [(&fg, 0x112233ff_u32), (&bg, 0x445566ff), (&light_bg, 0x778899ff)] {
        cx.update_window(f.w, |_, window, cx| picker.update(cx, |p, cx| p.select_color(rgba(value).into(), window, cx))).unwrap();
    }
    cx.run_until_parked();
    let slot = cx.update(|cx| cx.global::<ProfileStore>().get(&name).hit_palette[0].clone());
    assert_eq!((slot.fg.0.as_str(), slot.bg.0.as_str()), ("#112233", "#445566"));
    assert_eq!(slot.light_bg.map(|c| c.0).as_deref(), Some("#778899"));
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev test ui from_hsla`, `./dev test ui custom_colour`, `./dev test app custom_colours`
Expected: compile errors.

- [ ] **Step 3: Implement `Color::from_hsla` and `CustomColor`**

`color.rs`:

```rust
    /// The colour as `0xRRGGBBAA`, rounding each channel to 8 bits.
    pub fn from_hsla(h: Hsla) -> Color {
        let c = gpui_kit::Rgba::from(h);
        let ch = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
        Color((ch(c.r) << 24) | (ch(c.g) << 16) | (ch(c.b) << 8) | ch(c.a))
    }
```

`form.rs`:

```rust
/// The `Custom…` swatch (spec C1): gpui-kit's colour picker in a popover — the preset colours as featured colours,
/// the kit palette, hue, saturation, lightness and alpha sliders, and a hex field. Styled by the kit theme mirror.
#[derive(IntoElement)]
pub struct CustomColor {
    id: SharedString,
    state: Entity<ColorPickerState>,
    featured: Vec<Color>,
}

impl CustomColor {
    pub fn new(id: impl Into<SharedString>, state: &Entity<ColorPickerState>) -> Self {
        Self { id: id.into(), state: state.clone(), featured: vec![] }
    }

    pub fn featured(mut self, colors: Vec<Color>) -> Self {
        self.featured = colors;
        self
    }
}

impl RenderOnce for CustomColor {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().id(self.id).test_support().flex_none().child(
            ColorPicker::new(&self.state)
                .small()
                .label("Custom…")
                .accessibility_label("Custom colour")
                .featured_colors(self.featured.iter().map(|c| c.hsla()).collect()),
        )
    }
}
```

Imports: `gpui_kit::component::color_picker::{ColorPicker, ColorPickerState}`, `gpui_kit::component::Sizable` (for `.small()`) and `gpui_kit::Entity`.

- [ ] **Step 4: Wire the pickers into the profile editor**

In `crates/rogg-app/src/dialogs/profiles.rs`:

1. **Extract the change handlers into methods,** so the inputs and the pickers share them. The bodies move out of the existing subscriptions unchanged:
   - `fn apply_rule_color(&mut self, text: String, cx: &mut Context<Self>)`: the parse check, `color_error`, and the edit of `r.color`;
   - `fn apply_rule_light(&mut self, text: String, cx: &mut Context<Self>)`;
   - `fn apply_slot_field(&mut self, field: usize, text: String, cx: &mut Context<Self>)`.

   The input subscriptions now just call them.
2. **New fields:**
   - `rule_pickers: Option<([Entity<ColorPickerState>; 2], Vec<Subscription>)>`;
   - `slot_pickers: Option<([Entity<ColorPickerState>; 4], Vec<Subscription>)>`.

   Both are `None` in `new`. Add the accessors `rule_pickers()` and `slot_pickers()`, which return clones of the arrays.
3. **In `select_rule`.** Next to the inputs, create one picker per colour field:
   - its default value is the field's current resolved colour (`crate::profiles::resolve_color(..)`, as the swatches do), as `hsla`;
   - subscribe to it.

   On `ColorPickerEvent::Change(Some(h))`:

```rust
                cx.subscribe_in(&picker, window, move |e, _, ev: &ColorPickerEvent, window, cx| {
                    let ColorPickerEvent::Change(Some(h)) = ev else { return };
                    let hex = ColorSpec::hex(Color::from_hsla(*h).0).0;
                    if let Some(([_, color, light], _)) = &e.rule_inputs {
                        let input = if field == 0 { color } else { light };
                        input.update(cx, |i, cx| i.set_value(hex.clone(), window, cx));
                    }
                    if field == 0 { e.apply_rule_color(hex, cx) } else { e.apply_rule_light(hex, cx) }
                })
```

   (`field` is 0 for the colour and 1 for the light override. If `set_value` also emits `InputEvent::Change`, the apply simply runs twice with the same value.)
4. **In `select_slot`.** Do the same with four pickers (fg, bg, light fg, light bg) and `apply_slot_field(field, hex, cx)`.
5. **Clearing the pickers.** `select_rule(None, ..)` and `select_slot(None, ..)` drop their pickers, as they drop their inputs.
6. **Render.**
   - **Rule form:**
     - Colour row: swatches, then `CustomColor::new("rule-custom", &pickers[0]).featured(<the swatch colours>)`, then the hex field.
     - Light theme row: the light field, then `CustomColor::new("rule-light-custom", &pickers[1])`.
   - **Slot form:** after each `TextField`, add its `CustomColor`, with ids `slot-fg-custom`, `slot-bg-custom`, `slot-light-fg-custom` and `slot-light-bg-custom`.

`ColorSpec::hex` already writes `#rrggbb` for opaque colours and `#rrggbbaa` otherwise. Tokens stay available through the swatches. A picked hex still derives its light variant until it is overridden, since that logic is untouched.

- [ ] **Step 5: Run the tests**

Run: `./dev test ui` and `./dev test app profile_editor`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-ui crates/rogg-app
git commit -m "feat: Custom… colour picker for rule colours, light overrides and palette slots"
```

### Task 11: Recent filters in the empty filtered view (spec C2)

**Files:**
- Modify:
  - `crates/rogg-core/src/history.rs` (`recent_except`);
  - `crates/rogg-ui/src/empty.rs`;
  - `crates/rogg-app/src/tab_view.rs:162-164`.
- Test:
  - `crates/rogg-core/src/history.rs` (unit);
  - `crates/rogg-ui/tests/dialogs.rs`;
  - `crates/rogg-app/tests/filter.rs`.

**Interfaces:**
- Produces:
  - `rogg_core::history::{recent_except(entries, current, n) -> Vec<HistoryEntry>, RECENT_IN_EMPTY_VIEW}` (the latter is 3);
  - `rogg_ui::empty::{RecentFilter { expression, summary }, no_matches_with_recent(id, recent, on_pick)}`;
  - the element ids `recent-filters` and `recent-filter-{i}`.

- [ ] **Step 1: Write the failing tests**

`history.rs` unit test (the module already has an `entry(expr)` helper):

```rust
    #[test]
    fn recent_except_skips_the_current_filter_and_keeps_the_newest() {
        let mut h = History::default();
        for e in ["a", "b", "c", "d", "e"] {
            h.record(entry(e));
        }
        let names = |v: Vec<HistoryEntry>| v.iter().map(HistoryEntry::expression).collect::<Vec<_>>();
        assert_eq!(names(recent_except(h.entries(), &entry("e").to_spec(), 3)), ["d", "c", "b"], "newest first, current skipped");
        assert_eq!(names(recent_except(h.entries(), &entry("zzz").to_spec(), 3)), ["e", "d", "c"]);
        assert!(recent_except(&[entry("only")], &entry("only").to_spec(), 3).is_empty());
    }
```

`crates/rogg-ui/tests/dialogs.rs`:

```rust
#[gpui_kit::test]
fn no_matches_lists_recent_filters_and_reports_a_pick(cx: &mut TestAppContext) {
    use rogg_ui::empty::{RecentFilter, no_matches_with_recent};
    common::setup(cx);
    let picked = Rc::new(Cell::new(None));
    let p = picked.clone();
    let recent = vec![
        RecentFilter { expression: "error || timeout".into(), summary: "2 include".into() },
        RecentFilter { expression: "warn".into(), summary: "1 include · regex".into() },
    ];
    let w = common::open(cx, 600., 400., move |_, _| {
        let p = p.clone();
        no_matches_with_recent("empty-results", recent.clone(), Rc::new(move |i, _, _| p.set(Some(i)))).into_any_element()
    });
    assert!(common::exists(cx, w, "recent-filters") && common::exists(cx, w, "recent-filter-1"));
    common::click(cx, w, "recent-filter-1");
    assert_eq!(picked.get(), Some(1));
    let empty = common::open(cx, 600., 400., |_, _| no_matches_with_recent("empty-results", vec![], Rc::new(|_, _, _| {})).into_any_element());
    assert!(common::exists(cx, empty, "empty-results") && !common::exists(cx, empty, "recent-filters"), "no list without history");
}
```

`crates/rogg-app/tests/filter.rs` (add the `Args` and `common::open_workspace` imports as the other app tests do):

```rust
#[gpui_kit::test]
fn the_empty_filtered_view_offers_recent_filters(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", &["alpha one", "beta two"]);
    let (w, ws) = common::open_workspace(cx, rogg_app::args::Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    let apply = |cx: &mut TestAppContext, expr: &str| {
        doc.update(cx, |d, cx| d.set_expression(expr, cx)).unwrap();
        common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none()));
    };
    apply(cx, "nothing-here");
    assert!(common::exists(cx, w, "empty-results"));
    assert!(!common::exists(cx, w, "recent-filters"), "the only history entry is the current filter");
    for expr in ["alpha", "beta", "gamma", "zzz"] {
        apply(cx, expr);
    }
    // History, newest first: zzz (current, skipped), gamma, beta, alpha, nothing-here.
    assert!(common::exists(cx, w, "recent-filter-2") && !common::exists(cx, w, "recent-filter-3"), "at most three");
    common::click(cx, w, "recent-filter-1");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none()));
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.expression()), "beta");
    assert!(!common::exists(cx, w, "empty-results"), "beta matches a line");
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev test core recent_except`, `./dev test ui no_matches_lists`, `./dev test app the_empty_filtered_view`
Expected: compile errors.

- [ ] **Step 3: Implement**

`rogg-core/src/history.rs`:

```rust
/// Recent filters offered in the empty filtered view.
pub const RECENT_IN_EMPTY_VIEW: usize = 3;

/// Up to `n` entries, newest first, without the one equal to `current` (the filter already applied).
pub fn recent_except(entries: &[HistoryEntry], current: &FilterSpec, n: usize) -> Vec<HistoryEntry> {
    let current = HistoryEntry::from_spec(current);
    entries.iter().filter(|e| **e != current).take(n).cloned().collect()
}
```

`rogg-ui/src/empty.rs`:

```rust
/// A recent filter in the empty filtered view: its expression and its summary (`2 include · 1 exclude · regex · ±2`).
#[derive(Clone, Debug, PartialEq)]
pub struct RecentFilter {
    pub expression: SharedString,
    pub summary: SharedString,
}

/// `No matching lines`, then up to three recent filters (spec C2); a click applies one. Without any, the view is
/// `no_matches` unchanged.
pub fn no_matches_with_recent(id: impl Into<SharedString>, recent: Vec<RecentFilter>, on_pick: IndexCallback) -> EmptyState {
    let state = no_matches(id);
    if recent.is_empty() {
        return state;
    }
    state.child(RecentFilters { items: recent, on_pick })
}

#[derive(IntoElement)]
struct RecentFilters {
    items: Vec<RecentFilter>,
    on_pick: IndexCallback,
}

impl RenderOnce for RecentFilters {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let s = t.text_scale();
        let p = t.palette;
        let on_pick = self.on_pick;
        div()
            .id("recent-filters")
            .test_support()
            .mt(px(14.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .w(px(360.))
            .child(div().px(px(10.)).pb(px(4.)).text_size(s.px(10.)).text_color(p.faint).child("Recent filters"))
            .children(self.items.into_iter().enumerate().map(move |(i, f)| {
                let on_pick = on_pick.clone();
                div()
                    .id(SharedString::from(format!("recent-filter-{i}")))
                    .test_support()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(div().truncate().font_family(t.fonts.mono.clone()).text_size(s.px(12.)).text_color(p.text).child(f.expression))
                    .child(div().text_size(s.px(10.)).text_color(p.muted).child(f.summary))
                    .on_click(move |_, w, cx| on_pick(i, w, cx))
            }))
    }
}
```

`rogg-app/src/tab_view.rs`. Replace the `no_matches` branch:

```rust
            let bottom_body = if shown == 0 && d.filter.progress.is_none() {
                let entries = cx.try_global::<HistoryStore>().map(|h| h.entries().to_vec()).unwrap_or_default();
                let recent = recent_except(&entries, &d.filter.spec, RECENT_IN_EMPTY_VIEW);
                let items = recent.iter().map(|e| RecentFilter { expression: e.expression().into(), summary: e.summary().into() }).collect();
                let specs: Rc<Vec<FilterSpec>> = Rc::new(recent.iter().map(HistoryEntry::to_spec).collect());
                let pick = tab_act_ix(tab, move |t, i, _, cx| {
                    if let Some(spec) = specs.get(i).cloned() {
                        let _ = t.doc.update(cx, |d, cx| d.apply_spec(spec, cx));
                    }
                });
                empty::no_matches_with_recent("empty-results", items, Rc::new(pick)).into_any_element()
            } else {
```

(Cloning the whole history to slice it is wasteful. If borrowing the global in place fits the surrounding borrow of `cx`, do that instead. Otherwise, take only the first `RECENT_IN_EMPTY_VIEW + 1` entries.)

- [ ] **Step 4: Run the tests**

Run: `./dev test core history`, `./dev test ui dialogs`, `./dev test app filter`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-core crates/rogg-ui crates/rogg-app
git commit -m "feat: recent filters in the empty filtered view"
```

### Task 12: Iteration C gate, notes and hand-off

- [ ] **Step 1:** Run `./dev check`.
  - Expected: green.
  - The replica's filtered pane is not empty, and the profile editor is not on a replica page, so the goldens are unchanged.
  - Any difference is a regression to explain, or an intended change for the user to review.
- [ ] **Step 2:** Add an "Iteration C" section to the manual-checks note. Check in `./dev demo`:
  - In Highlighting:
    - `Custom…` opens the picker in rogg's colours, with the presets on top;
    - a picked colour fills the hex field and recolours the log at once;
    - alpha below 100 % writes `#rrggbbaa`;
    - the light override and all four palette-slot fields have their own picker.
  - In the filtered pane:
    - a filter that matches nothing lists up to three recent filters;
    - a click applies one.

  Commit as `docs: refinements 1 manual checks (iteration C)`.
- [ ] **Step 3:** Update the memory `rogg-milestone-status` (controller): Refinements 1 is done, with the merge commits.
- [ ] **Step 4: Hand off (stop here).** Summarise, invite `./dev demo`, and ask: merge locally / keep the branch. Merge only on the user's choice.

---

## Spec coverage

| Spec item | Task |
|---|---|
| A1 Profile / History labels without `⌄` | 1 |
| A2 ChevronRight, rotated on folders and the all-tabs button; breadcrumbs untouched; gallery specimens | 1 |
| A3 Band height in edit mode (with test) | 2 |
| A4 About: menu item first + separator, title, header, build (commit, date, rustc; `unknown` fallback), data rows with Show in Finder (AppPaths), credits | 3 |
| B1 3-line cap, faint `…`, capped height for scroll, hit-testing and selection; wrap off unchanged | 5 |
| B2 Show full line… always on line rows; `Line <n>`; read-only, selectable, wrapped, log font; Copy copies the whole line; 1 MB display cap with a note | 6 |
| B3 Character drag within a row; snapping; selection colour; crossing rows gives whole rows; click clears; ⌘C and Copy use the characters; "with line numbers" uses whole rows; cleared by keyboard, filter change or row gone; arrow pointer; works wrapped | 7, 8 |
| C1 `Custom…` swatch with gpui-kit ColorPicker; `#rrggbb`/`#rrggbbaa`; rule, light override, slot fg/bg and their overrides; tokens and derived light kept | 10 |
| C2 Up to 3 recent filters, newest first, current skipped; expression (mono) and summary; click applies; nothing without history | 11 |
| Testing and `./dev check` per iteration; user review and `visual-approve` | 4, 9, 12 |
