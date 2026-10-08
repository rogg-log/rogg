# rogg — Refinements 1 (post-v1 iteration) — Design

Date: 2026-10-08
Status: Approved in conversation; written for review
Builds on: `2026-10-06-rogg-design.md` (v1, M0–M4 merged)

## Intent

This is the first round of refinements after v1. It covers eight user-requested improvements to existing flows: UI polish, log-view text handling, colour choice and quick reuse of filters.

**The visual baseline changes.** From now on the app is no longer a pixel replica of the HTML draft (`assets/rogg-main-window.html`); the draft was not perfect. The replica goldens (`./dev visual-check`) stay as a regression baseline. When a change alters them on purpose, the user reviews the new captures, and they are re-approved with `./dev visual-approve`.

## Iterations

Each iteration is implemented, reviewed and merged on its own. `./dev check` must be green, and the user gets a build to try.

| Iteration | Items | Theme |
|---|---|---|
| A | 1, 2, 3, 8 | Quick UI polish and an About dialog |
| B | 5, 6 | Log view: long wrapped rows and partial text selection |
| C | 4, 7 | Any colour in profiles; recent filters in the empty filtered view |

## A. UI polish

### A1. No trailing `⌄` on text buttons (items 1, 2)
- The info bar's profile button reads `Profile: Services` (the dot stays). The trailing `⌄` is removed.
- The filter band's history button reads `History` (the icon stays). The trailing `⌄` is removed.

### A2. One chevron, rotated (item 2)
- A new icon, `ChevronRight`, is drawn in the same style as the existing icons.
- Sidebar folder rows show `ChevronRight` when collapsed. Expanded rows show the same icon rotated 90° clockwise, so it points down. This replaces the `›` and `⌄` text glyphs.
- The title bar's "open files" (all tabs) button shows `ChevronRight` rotated 90°. This replaces the `⌄` glyph.
- Breadcrumb separators (`›`) are separators, not toggles, and do not change.
- Gallery specimens that show these glyphs are updated to match.

### A3. The filter band keeps its height in edit mode (item 3)
Entering edit mode (the edit button, ⌘L, Enter) must not change the band's height. In edit mode the expression field and the Apply button take the chip height (28 px), so the filter line is as tall as it is with chips. A test compares the band's bounds in both modes.

### A4. About dialog (item 8)
- **Opening it:** the rogg menu has an "About rogg" item first, followed by a separator. It opens a modal dialog in the standard dialog style (spec §5.8) with the title `About rogg`.
- **Header:** `rogg <version>` and the tagline `A fast log explorer for macOS`.
- **Build:**
  - the short git commit and the build date, captured at compile time by `rogg-app/build.rs`, with `unknown` outside a git checkout;
  - the Rust toolchain version.
- **Data:** two rows, each with a `Show in Finder` button (`reveal_path`):
  - settings, profiles, session, history and recent files: `~/Library/Application Support/rogg`;
  - the cache: `~/Library/Caches/rogg`.

  The paths come from `AppPaths`, so tests see their temp directories.
- **Credits:**
  - "Built with Rust, gpui and gpui-kit";
  - a list of the main open-source crates with their licenses. The license strings are copied from each crate's manifest when the list is written: gpui-kit, regex, aho-corasick, memchr, memmap2, rayon, notify, flate2, zip, tar, encoding_rs, serde, serde_json, toml.

## B. Log view

### B1. Long wrapped rows show at most 3 lines (item 5)
- With wrap on, a row whose text wraps to more than three visual lines shows only the first three. The third line ends with a faint `…`, and the rest is clipped.
- The row's height, scrolling, hit-testing and selection use the capped height.
- Rows of three lines or fewer are unchanged. With wrap off nothing changes, since long lines already scroll horizontally.

### B2. "Show full line…" (item 5)
- **The menu item:** the row context menu gets a `Show full line…` item. It is always available on line rows, whether wrap is on or off, and also for lines cut at 64 KB.
- **The dialog:** it opens with the title `Line <number>` (the display number) and shows the whole line.
  - The text is read-only, selectable and scrollable, in the log font, and wraps.
  - A `Copy` button copies the whole line.
- **Very long lines:** above 1 MB the dialog shows the first 1 MB and a note saying the line was cut. Copy still copies the full line.

### B3. Partial text selection (item 6)
- **Selecting characters:** pressing on a row's text and dragging while the pointer stays in that row selects characters in the row. The selection snaps to character boundaries and is painted in the selection colour.
- **Selecting rows:** once the drag crosses into another row, it becomes the whole-row range selection that exists today.
- **A click:** a click without a drag selects the row, as today, and clears any character selection.
- **Copying:** ⌘C and the context menu's `Copy` copy just the selected characters when a character selection exists, and whole rows otherwise. `Copy with line numbers` always copies whole rows.
- **Lifetime:** keyboard selection moves, a new filter and the row disappearing all clear the character selection.
- **Pointer:** it stays the arrow and never changes to an I-beam.
- **Wrapped rows:** the character selection works on wrapped rows within their visible lines.

## C. Colours and filters

### C1. Any colour in profiles (item 4)
- **Where:** the rule form's colour row keeps its preset swatches and hex field, and gains a custom-colour swatch after them: a square the size of the presets, filled with the field's current colour, with a small pen icon inside and no text label (user change at the iteration C review, 2026-10-08).
  - It opens gpui-kit's `ColorPicker` popover: hue, saturation and lightness sliders, alpha, and hex.
  - The picker is styled by the existing kit theme mirror, so no gpui-kit default colours appear.
  - Choosing a colour writes `#rrggbb`, or `#rrggbbaa` when alpha is not 1, into the rule colour. The colour applies and saves at once, like the other edits.
- **The same picker** is offered for:
  - the rule's light-theme override;
  - the foreground and background of a filter-match palette slot;
  - the slot's light-theme overrides.
- **Existing behaviour is kept:** colour tokens (`error`, `hit-2`…) stay available as presets, and a hex colour still derives its light variant unless overridden.

### C2. Recent filters in the empty filtered view (item 7)
- **The list:** the filtered pane's empty view (`No matching lines`) shows up to three recent filters from the history, newest first. The entry equal to the tab's current filter is skipped.
- **Each entry** shows its expression in the mono font and its summary (`2 include · 1 exclude · regex · ±2`).
- **Clicking an entry** applies it to the tab. This restores the terms, states, flags and context, as the history popover does.
- **No history:** with no history the view is as it is today.

## Testing
- **Iteration A:**
  - the button labels;
  - chevron rotation state on folder rows and the all-tabs button;
  - band height equal in both modes;
  - the About dialog's rows, `Show in Finder` wiring (a `reveal` hook observable in tests) and the build-info fallback.
- **Iteration B:**
  - capped row height for long wrapped rows, and `…` presence;
  - `Show full line…` opening the dialog with the full text (including a line over 64 KB);
  - character selection by drag within a row;
  - the switch to row selection across rows;
  - copy of the partial text, and the selection cleared by click, keyboard and a new filter.
- **Iteration C:**
  - a custom colour written as hex and applied (rule, override, palette fg/bg);
  - recent filters listed (at most 3, current skipped), and a click applying one;
  - no list without history.
- **Every iteration:**
  - `./dev check` is green;
  - iterations that change the replica captures end with a user review and `./dev visual-approve`.

## Out of scope
- Keyboard-driven character selection (shift+arrows within a row), double-click to select a word, and selection across rows at character level.
- A system (NSColorPanel) colour picker.
- Changing the reference HTML draft.
