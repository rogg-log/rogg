# rogg — Design Spec

Date: 2026-10-06
Status: Draft for review
Visual reference: [`assets/rogg-main-window.html`](assets/rogg-main-window.html)

## 1. Intent

rogg is a native macOS log explorer in the spirit of glogg/klogg: a full log
on top, a filter line in the middle, and a filtered log at the bottom. It is
written in Rust with gpui and gpui-kit.

**Primary use case:** investigating saved logs and support bundles. A typical
input is a single log up to ~1 GB, or a zip/tar.gz bundle of ~10 such logs.

**Success criteria**

1. The UI matches the HTML reference closely: layout, sizes, colors,
   typography, states, in dark and light themes and both densities. The user
   and I review this together, with the help of an automated visual
   comparison report, and the user approves it *before* the main app is
   assembled (milestone M0) and again on the real app (milestone M2).
2. A 1 GB file is scrollable within moments of opening; indexing and filtering
   run in the background with progress, never freeze the UI, and are
   cancellable.
3. Every feature listed in this spec is covered by automated tests; engine
   performance has benchmarks with explicit targets.

**Lesson from the previous attempt:** the earlier build did not look like the
reference. This design therefore makes visual parity a gated, measured step
with a dedicated component gallery, rather than an afterthought.

## 2. Decisions

| Topic | Decision |
|---|---|
| Platform | macOS only (Apple Silicon). No cross-platform abstractions. |
| Language / GUI | Rust stable; `gpui-pre =0.3.8` + `gpui-kit =0.7.1` (crates.io, exact pins). |
| Engine approach | Memory-mapped files + compact line index + parallel chunked scan. |
| UI approach | Own design system on top of gpui-kit primitives; custom log view element. |
| Encodings | UTF-8 (lossy, invalid bytes → U+FFFD); UTF-16 LE/BE auto-detected by BOM and transcoded. |
| Structure | One spec, five milestones (M0–M4), each with its own plan and review. |
| Visual gate | `rogg-gallery` binary + screenshot/geometry diff against the HTML reference. |
| v1 extras | Compressed single files (`.gz`), `.tgz`, `.tar`, nested archives. |
| Deferred | Bundle-wide search, timestamp navigation, merged timeline, other platforms, extra encodings. |

## 3. Architecture

### 3.1 Workspace

```
rogg/
├─ Cargo.toml               workspace, exact dependency pins
├─ rust-toolchain.toml      pinned stable toolchain
├─ justfile                 test / bench / visual / run tasks
├─ crates/
│  ├─ rogg-core/            UI-agnostic engine and models
│  ├─ rogg-ui/              theme, icons, components, log view element
│  ├─ rogg-app/             `rogg` binary: windows, tabs, actions, wiring
│  └─ rogg-gallery/         `rogg-gallery` binary: component gallery + mockup replica
├─ tools/
│  ├─ visual-diff/          Node + Playwright + pixelmatch comparison harness
│  └─ genlog/               Rust bin: synthetic log and bundle generator
└─ docs/superpowers/specs/  this spec and the reference
```

### 3.2 Boundaries

- **`rogg-core`** has no gpui dependency. It owns sources, cache, encoding,
  line index, filter parsing and engine, search, statistics, and the data
  models for profiles, settings, history, recent files and sessions.
- **`rogg-ui`** renders view-models and reports intents. Components take plain
  data (for example `FilterChipProps { text, state, color_slot, focused }`) and
  emit events through callbacks. It never touches files or the engine.
- **`rogg-gallery`** feeds canned view-models into the same `rogg-ui`
  components that the app uses. Nothing is duplicated between the gallery and
  the app.
- **`rogg-app`** is the only crate that knows both sides. It maps engine
  state to view-models, routes actions, and persists state.

### 3.3 Concurrency

- Long jobs (indexing, filtering, searching, decompression, export) run on
  background threads. They use rayon for data parallelism and a dedicated
  thread per job for orchestration.
- Jobs publish progress and partial results to the UI over a channel consumed
  by a gpui background task. The UI redraws at most once per frame.
- Every job holds a cancellation token (`Arc<AtomicBool>` plus a generation
  number). A new filter, a closed tab or a superseded request cancels within
  milliseconds; results carrying a stale generation are dropped.

### 3.4 Dependencies

- **GUI:** `gpui-pre`, `gpui-kit`.
- **Engine:** `memmap2`, `memchr`, `regex` (byte-level), `aho-corasick`,
  `rayon`.
- **Archives and compression:** `flate2`, `zip`, `tar`.
- **Encoding:** `encoding_rs`.
- **File watching:** `notify` (FSEvents).
- **Persistence:** `serde`, `toml` (settings, profiles), `serde_json`
  (sessions, history, recent files).
- **Paths and errors:** `directories`, `thiserror`.
- **Tests and benchmarks:** `proptest`, `criterion`, `tempfile`.

Exact versions are pinned in the workspace `Cargo.toml` during M0.

### 3.5 Toolchain

- **Rust:** `brew install rustup`, then `rustup-init` installs the stable
  toolchain pinned in `rust-toolchain.toml`.
- **Xcode:** Xcode is installed (`xcode-select -p` →
  `/Applications/Xcode-beta.app/...`). M0 verifies that `xcrun metal` works.
  If it does not, gpui's `runtime_shaders` feature is enabled instead.
- **Node:** already installed, used only by `tools/visual-diff`.
- **just:** `brew install just`.

## 4. Core engine (`rogg-core`)

### 4.1 Sources

Every input resolves to a `LogSource` backed by a UTF-8 byte file that can be
memory-mapped, or by a derived view (§4.9).

| Input | Handling |
|---|---|
| Plain file | Memory-mapped in place, no copy. |
| `.gz` (including `name.log.N.gz`) | Stream-decompressed into the cache. The file is viewable while decompression proceeds. |
| `.zip`, `.tar`, `.tar.gz`, `.tgz` | The archive is listed cheaply into a tree. A member is extracted into the cache only when opened. Nested archives appear as expandable nodes: the inner archive is extracted, then listed. |
| Folder | Recursive scan up to a configurable depth (default 4). Lists regular files that are text or known archives; binaries are skipped by sniffing the first 8 KB for NUL bytes. |
| stdin | When stdin is not a TTY, `rogg` copies it into a cache file as it arrives and opens a tab in live-follow mode. |

The sidebar tree model (`SourceTree`) is part of `rogg-core`. Its nodes are
folder, archive, member and file, each with a size (and the expanded size,
where known).

### 4.2 Cache

- **Location:** `~/Library/Caches/rogg/`.
- **Key:** a hash of (canonical path, member path, size, mtime). Reopening
  reuses existing entries.
- **Atomic writes:** each entry is written to `*.partial`, then renamed. A
  crash or cancellation never leaves a corrupt entry.
- **Eviction:** least recently used, once total size exceeds the limit
  (default 20 GB, configurable). Entries currently open are never evicted.

### 4.3 Encoding

- If the first bytes are a UTF-16 LE or BE BOM, the file is transcoded to
  UTF-8 into the cache (streaming, with progress).
- Otherwise the file is treated as UTF-8. Invalid sequences are rendered as
  U+FFFD; the bytes on disk are untouched.
- Line endings are detected from the first 64 KB: LF, CRLF or mixed. A
  trailing CR is excluded from the displayed and matched line content.
- The status bar shows the encoding and line endings, for example
  `UTF-8 · LF` or `UTF-16LE → UTF-8 · CRLF`.

### 4.4 Line index

- Built in parallel. The memory map is split into ~16 MB chunks; each chunk
  scans for `\n` with `memchr`, and the chunk results are stitched in order.
- **Storage:** blocks of 4096 lines, each with a `u64` base offset plus `u32`
  relative offsets. That is about 4 bytes per line, ~40 MB for 10 M lines.
- Tracks the longest line in bytes, used for horizontal scroll extent.
- Lines become available progressively: the UI can scroll the indexed prefix
  while indexing continues. Progress is reported in bytes.
- Lines over 64 KB are truncated for display with a `…` marker, but are
  matched in full.

### 4.5 Growth and live follow

- `notify` watches the backing file of plain-file and stdin sources.
- **Append:** only the new tail is indexed incrementally. The active filter
  then scans just the new lines and appends to its results.
- **Shrink, replace or rotate** (size decreases, or the inode changes): the
  file is re-indexed from scratch and a toast says "File was truncated or
  replaced — reloaded."
- **Follow:** the full log auto-scrolls to the end. **Pause** stops
  auto-scrolling while the index keeps updating; the control shows the
  number of new lines since the pause, for example `+1,204`.

### 4.6 Filter expression

- The expression text is split on `||`; each term is trimmed and empty terms
  are dropped. `\|\|` is a literal `||`. In regex mode the unescaped text is
  passed to the regex engine, so alternation is written as `(a|b)`.
- Each term has a state: **Include**, **Off** or **Exclude**. Clicking a
  chip's state segment cycles Include → Off → Exclude → Include.
- When the expression is edited, a term whose text is unchanged keeps its
  state; new terms start as Include.
- **Global flags:** regex (`.*`) and case sensitive (`Aa`); case-insensitive
  is the default.
- **Invalid regex:** the new expression is rejected, the error appears as
  `Previous results retained · <message>`, and the last valid filter keeps
  running.

### 4.7 Filter semantics

- Let *I* be the Include terms and *E* the Exclude terms.
  - If *I* is non-empty, a line **matches** when it matches any term in *I*
    and no term in *E*.
  - If *I* is empty and *E* is non-empty, a line matches when it matches no
    term in *E*.
  - If there are no active terms, nothing matches.
- **Pins:** pinned lines always appear in the filtered view, whether or not
  they match.
- **Context ±N:** each match and pin also brings in N surrounding lines (UI
  presets None/±1/±2/±3; up to 10 via settings). A context line that matches
  a term in *E* is not shown, unless it is itself pinned. Context lines
  render dimmed.
- **Gap rows:** a gap between two shown lines renders as a
  `··· N lines omitted` row.
- **Composition:** the filtered view is computed from three independent sets
  (matches, pins, context). Changing pins or the context amount recomputes
  the view without rescanning the file.

### 4.8 Filter engine

- Compiles the active terms into a matcher:
  - All plain Include terms go into one Aho-Corasick automaton (with ASCII
    case folding when case-insensitive).
  - Regex Include terms go into a `regex::bytes::RegexSet`.
  - Exclude terms are compiled the same way, but are tested only on lines
    that pass the Include stage (or on all lines when *I* is empty).
- The line range is split into chunks scanned on the rayon pool. Chunk
  results are emitted in order as sorted `Vec<u64>` line numbers. The UI
  shows partial results and progress as chunks complete.
- **Per-term hit counts** are gathered in the same scan, for statistics.
- **Rescans:** toggling a single chip's state triggers a full rescan
  (target: ~1 s per GB).
- **Match spans for highlighting** are computed only for rows being painted:
  each Include term is run over the visible line to produce `(range,
  color_slot)` spans. When spans overlap, the term listed earlier wins. The
  color slot is the term index modulo the palette length.

### 4.9 Derived sources ("Open filtered as tab")

- A derived source is the parent's filtered rows, frozen at creation and copied
  into a cache file (`streams/derived-<pid>-<n>.log`). The tab maps that file
  like any log.
- It behaves like an ordinary file: its lines are numbered from 1. They do
  **not** keep their original numbers (decided at the M4 plan review,
  2026-10-07).
- Derived sources can nest (a derived source of a derived source). The copy is
  deleted when its tab closes and swept after a crash.

### 4.10 Search (per panel)

- A plain or regex query with its own case flag, independent of the filters.
- **Scope:** the whole source for the full log; the shown rows for the
  filtered log.
- The hit count is computed in the background (`1,204 hits`).
  Enter / Shift-Enter jumps to the next / previous hit relative to the
  selection. Hits are outlined with the accent color.

### 4.11 Statistics

Computed for each filter run:

- matched lines / total lines
- number of pinned lines, and how many of them do not match
- number of context lines
- per-term hit counts
- scan duration

### 4.12 History

- Every applied filter is recorded as {terms with states, regex flag, case
  flag, context}.
- Duplicates are removed (an existing entry moves to the top); the list keeps
  the 100 most recent entries.
- History is global across tabs and persisted to
  `~/Library/Application Support/rogg/history.json`. Choosing an entry
  restores all of these fields.

### 4.13 Errors

- The core returns typed `thiserror` errors and never panics on input data.
- A corrupt archive member marks only its own tree node as failed.
- I/O errors during indexing surface as the tab error state, with Retry.

## 5. UI (`rogg-ui`, `rogg-app`)

### 5.1 Theme

All values are transcribed from the reference CSS into
`rogg_ui::theme::Theme`, with `dark()` and `light()` constructors. No
component uses gpui-kit's default colors.

| Token | Dark | Light |
|---|---|---|
| page | `#111315` | `#e9ecea` |
| bg / panel | `#191c1f` | `#fcfcfa` |
| side | `#1e2125` | `#f2f4f1` |
| raised | `#25292e` | `#e7ebe7` |
| hover | `#2c3137` | `#e7ece7` |
| border | `#34393f` | `#d1d8d1` |
| line | `#292e33` | `#e5e9e3` |
| text | `#e2e6e9` | `#29372d` |
| log text | `#bbc5ce` | `#435047` |
| muted | `#929da7` | `#69766c` |
| faint | `#67737e` | `#849086` |
| accent | `#a8c5b7` | `#386a4f` |
| accent-bg | `#263e34` | `#e0ece3` |
| accent-border (pressed) | `#50665c` | `#50665c` |
| error | `#efaaa2` | `#a44437` |
| amber | `#e0c283` | `#9a6c20` |
| selection | `#263844` | `#e1ece6` |
| error row tint | `#8f352e` at 6% alpha (`#8f352e10`) | same |
| filter band border | `#49534f` | `#49534f` |
| filter band handle | `#65776e` | `#65776e` |

**Hit palette** (dark text / background; light text / background):

| Slot | Dark | Light |
|---|---|---|
| 0 | `#ffc4bb` / `#744037` | `#763227` / `#f1c7bc` |
| 1 | `#f2d693` / `#64502b` | `#714c16` / `#ead49b` |
| 2 | `#acd7ea` / `#315064` | `#325e70` / `#bfdae6` |
| 3 | `#cbb5eb` / `#514169` | `#624580` / `#d9c9eb` |

**Filter chip colors** (border / background / term text):

| Slot | Dark | Light |
|---|---|---|
| 0 | `#67514b` / `#362b29` / token `error` | `#dcc4bc` / `#f8eae6` / token `error` |
| 1 | `#64583d` / `#342f24` / token `amber` | `#d8cba6` / `#f7f0dd` / token `amber` |
| 2 | `#415762` / `#263037` / `#a2c9d8` | `#b9d0da` / `#e7f0f4` / `#426b7b` |
| 3 | `#5c4c6c` / `#302a37` / `#c2acd9` | `#cdbdda` / `#ece5f5` / `#776083` |

**Fonts:** UI 13 px system font (SF Pro). Log text defaults to SF Mono at
12 px; the family and size are user-selectable (§6.2). Layout metrics (row
height, gutters) do not depend on the chosen family; only glyph widths
change.

**Density:**
- **Compact:** row 23 px, log font 12 px, pane header 35 px.
- **Comfortable:** row 29 px, log font 13 px, pane header 39 px.

**Shape:**
- Buttons: 5 px radius, 1 px transparent border.
- Pressed toggle (`aria-pressed=true` in the reference): accent text,
  accent-bg fill, `#50665c` border.
- Focus ring: 2 px accent, 2 px offset.

Values the light theme leaves unspecified (for example `accent-border`) reuse
the dark value. M0 confirms each one against the light-theme screenshot.

### 5.2 Icons

The 16 SVG symbols in the reference (`file`, `folder`, `search`, `wrap`,
`pin`, `filter`, `follow`, `check`, `off`, `minus`, `history`, `edit`,
`export`, `sun`, `settings`, `side`) are copied verbatim as SVG assets.

- **Rendering:** 15 px by default, 1.6 stroke, round caps and joins, tinted
  with the current text color, matching the CSS.
- **New icons** (open-as-tab `↗`, pause, close) are drawn in the same style.

### 5.3 Window layout

All sizes are in logical points and taken from the reference.

- **Title bar, 42 px.** Transparent macOS title bar with custom content.
  - **Leading block** (width = sidebar width, 13 px padding): traffic lights
    (system), sidebar toggle icon button (25×25), `rogg` label (14 px, 600
    weight, muted).
  - **Tabs:** height 29 px, max width 152 px, 3 px gap. The active tab has
    the raised background, a border, and a 1 px accent underline (70%
    opacity, inset 9 px). Tab icon 12 px. Label 11 px, ellipsized. The close
    button (18×20) is visible on hover or focus, and always on the active
    tab. Middle-click closes.
  - **Window drag area:** the remaining space.
  - **Right side:** `⌄` (all tabs), `⌘` (shortcuts), settings icon.
- **Sidebar.**
  - Default width 211 px, resizable 155–360 px, toggleable.
  - Header (43 px): `FILES` (10 px, uppercase, 1 px letter spacing, 600
    weight, muted) and a file count.
  - Tree: folder rows 30 px at 11 px / 500 weight; file rows 30 px at 12 px,
    indented 27 px, with a size label (10 px, faint). The active file has the
    selection background and an accent icon.
  - `RECENT` section, then a footer (10 px, 1.8 line height) summarizing the
    bundle.
- **Info bar, 33 px.** Breadcrumb (11 px, muted) on the left. On the right:
  `● Profile: <name>⌄` | `Export`.
- **Full log pane** (flex 1.1).
  - Header (35 px): `Full log` (11 px, 550 weight), count (10 px, faint),
    find field (24 px tall, 112 px input), and the `Wrap` tool button.
  - Log view below.
- **Filter band.**
  - Background: side color, 1 px `#49534f` top and bottom borders.
  - Handle: 30×3 px at top center. Dragging it resizes the two panes.
  - **Filter line** (min height 30 px): filter icon (accent), chips separated
    by `||` (11 px mono, faint), then the actions: edit icon button, `.*`
    and `Aa` flag buttons (27 px tall, min width 28 px, mono).
  - **Bottom row** (10 px): `History⌄` button, Context select, summary
    (`2 include · 1 exclude · 1 off`), and the hint (9 px, faint, right
    aligned) `Click a state to cycle Include → Off → Exclude`.
- **Filtered log pane** (flex 1). Header as in the full log pane, plus
  `Follow selection`, `Open as tab` and `Wrap` tool buttons.
- **Footer, 29 px.** Statistics (bold numbers), selected line, then on the
  right: progress (thin bar + percent while a job runs), encoding and line
  endings.

### 5.4 Filter chip

- Height 28 px, 5 px radius, 1 px border; colors by slot (§5.1).
- **State segment** (left): icon (11 px: `check` / `off` / `minus`) and label
  (`IN` / `OFF` / `NOT`, 9 px system font), with a right border at
  `#ffffff15`. Clicking it cycles the state.
- **Term:** 12 px mono, 6 px left and 8 px right padding.
- **Off:** transparent background, the standard border color, faint term
  text.
- **Exclude:** term text struck through.
- **Edit mode** (edit button or ⌘L): the chips are replaced by a single-line
  expression input (accent border) plus an `Apply ↵` button. Enter applies,
  Escape cancels. Errors appear under the line in the error color.

### 5.5 Log view element

A custom gpui `Element`, shared by both panes.

- **Row layout:** 24 px pin gutter, 65 px right-aligned line number (10 px,
  faint, 12 px right padding), then the text column (20 px right padding).
- **Pin glyph:** 11 px. Hidden by default, 70% opacity on row hover, and
  solid accent fill when pinned.
- **Row states:**
  - hover background
  - selected: selection background plus a 2 px accent inset bar on the left
  - profile row tint
  - context row: muted text
  - gap row: 17 px, 9 px system font, faint, 89 px left inset
- **Text layers**, painted in order: base log text color → profile rule
  colors → filter-match spans (filtered pane only; hit palette, 2 px radius)
  → search-hit outline (1 px accent).
- **Rendering** is virtualized: only visible rows are shaped and painted.
  Shaped lines are cached per (line number, width, wrap mode, theme
  generation).
- **Wrap off:** horizontal scroll, with extent from the longest line.
- **Wrap on:** visible rows are measured; a row-height cache provides stable
  scroll positions, and unmeasured rows use an estimate.
- **Selection and copy:** click selects a line; shift-click and drag select a
  range. ⌘C copies raw lines; the context-menu entry copies with line
  numbers.
- **Context menu:** Pin/Unpin, Copy, Copy with line numbers, Filter on
  selection (adds the selected text as an Include term), Exclude selection
  (adds it as an Exclude term).
- **Scrollbars:** thin, `#4b545d` thumb on a transparent track, as in the
  reference.

### 5.6 Interactions

- **Follow selection** (toggle, default on): selecting a filtered row scrolls
  the full log so the original line is centered and selected.
- **Selecting a full-log row** also selects the matching filtered row when it
  is shown, without scrolling the filtered pane unless the row is off-screen.
- **Pin** via the gutter, ⌘B or the context menu. Pins update both panes.
- **Per-tab state:** expression and states, flags, context, pins, selection,
  scroll positions, split ratio, wrap per pane, follow, active profile.
- **Open as tab** (§4.9): opens a new tab labeled `<name> ⟩ filtered`. Its
  tooltip shows the source expression; its breadcrumb is the full chain, and
  clicking a breadcrumb segment focuses that tab if it is open. A snapshot of
  a live source shows `Snapshot · HH:MM:SS` in the info bar.

### 5.7 Empty and transient states

1. **No tabs:** welcome view with `Open file` / `Open folder` buttons, a
   drop-target hint and the recent list.
2. **No matches:** `No matching lines` / `Enable an include filter or pin a
   line in the full log.` (as in the reference).
3. **Loading, extracting, indexing:** progress shown in place, with a cancel
   button.
4. **Error:** message, path and `Retry`.
5. **Toasts:** reloads, recovered settings, export finished.

### 5.8 Dialogs and popovers

The common style comes from the reference `dialog`: side background, 1 px
border, 10 px radius, 22 px padding, 500 px wide, a dark backdrop, a
`✕` icon button, and 17 px / 550 weight titles.

Dialogs and popovers:

- **All tabs** (`⌄`): a list with the current tab checked.
- **Highlighting profile editor** (§6.1).
- **Appearance and Settings** (§6.2).
- **Keyboard shortcuts** (`⌘`).
- **Filter history:** each entry shows its expression and states; click
  restores it.
- **Statistics:** opened from the footer (§4.11).
- **Export** (§6.5).
- **Go to line.**

### 5.9 Keyboard

| Shortcut | Action |
|---|---|
| ⌘O / ⇧⌘O | Open file / open folder |
| ⌘W | Close tab |
| ⌘1…⌘9, ⌃Tab / ⌃⇧Tab | Switch tab |
| ⌘F | Find in focused pane |
| Enter / ⇧Enter (in find) | Next / previous hit |
| ⌘L | Edit filters |
| ⌥⌘R / ⌥⌘C | Toggle regex / case |
| ⌘B | Pin or unpin selected line |
| ⌘G | Go to line |
| ↑ ↓, PgUp/PgDn, ⌘↑ / ⌘↓ | Move selection, page, top/bottom |
| Return | Reveal selected line in full log |
| Tab / ⇧Tab | Cycle focus: full log → filter line → filtered log |
| ⇧⌘F | Toggle follow selection |
| ⇧⌘T | Open filtered as tab |
| ⌘E | Export |
| ⌘\\ | Toggle sidebar |
| ⌘, | Settings |
| Space (full log, live source) | Pause / resume follow |

The native menu bar has File (Open, Open Folder, Open Recent ▸, Export,
Close Tab), Edit (Copy, Find, Go to Line), View (Sidebar, Wrap, Theme,
Density), and Window. All bindings are defined in one keymap table so they
can be remapped later.

## 6. Profiles, settings, persistence

### 6.1 Highlighting profiles

- **Contents:** a profile is `{ name, rules: Vec<LineRule>, hit_palette:
  Vec<HitColor> }`.
- **LineRule fields:**
  - `pattern`, `regex: bool`, `case_sensitive: bool`
  - `effect`: any combination of match text color, whole-line text color,
    row tint, bold
  - `panes`: full, filtered, or both
  - `enabled`
  - a color for each theme
- **Order:** rules are evaluated top to bottom; the first matching rule wins
  for each effect.
- **Editor:**
  - Rows show a swatch, the pattern (mono), an effect summary and the panes
    (styled like `.profile-rule` in the reference).
  - Rules can be reordered, enabled and disabled, edited and deleted.
  - Colors use a swatch picker plus a hex field. Light-theme colors are
    derived automatically and can be overridden.
  - Palette slots can be added, edited and removed.
- **Built-in profiles:**
  - **Default:** ERROR (error-colored text plus the subtle row tint), WARN
    (amber text), DEBUG (muted text).
  - **Services:** the reference example (ERROR, WARN, `request_id=\S+`).
- **Assignment:** each tab has one active profile; settings define the
  default.
- **Storage:** `~/Library/Application Support/rogg/profiles/*.toml`.
  Profiles can be imported and exported as `.roggprofile` (TOML).

### 6.2 Settings

Stored in `~/Library/Application Support/rogg/settings.toml`. Changes apply
immediately.

- **Appearance:**
  - Theme: System / Light / Dark, following macOS appearance changes live
  - UI font size
  - Log font family: a short curated list of monospace fonts that ship with
    macOS (SF Mono default, Menlo, Monaco, Courier New), plus "Other…" to
    pick any installed monospace font. Fonts that are not available are
    hidden.
  - Log font size (9–20 px). The row height follows the density and scales
    with the font size.
  - Density: Compact / Comfortable
- **Behavior:**
  - Default wrap
  - Default follow selection
  - Default context (0–10)
  - Default regex and case flags
  - Restore session on launch (default on)
  - Cache size limit
  - Folder scan depth
  - Default profile

### 6.3 Recent files

- Up to 20 entries: files, folders and archives (with the last opened
  member), stored in `recent.json`.
- Shown in the sidebar's Recent section, the welcome view and File → Open
  Recent.
- Missing entries render faint and can be removed.

### 6.4 Session restore

- **When:** saved on quit and every 30 s to `session.json`.
- **Window state:** window frame, sidebar visibility and width, active tab.
- **Per tab:** source (path plus member, or derived-source definition),
  profile, filter state (expression, term states, flags, context), pins,
  selection, scroll positions, split ratio, wrap, follow.
- **Derived tabs:** stored as parent reference plus filter definition, and
  rebuilt deterministically on restore. If the parent was a live or stdin
  source, the explicit line list is stored instead.
- **stdin tabs** are not restored.
- **Missing sources:** the tab opens in the error state.

### 6.5 Export

All exports run in the background with progress.

1. **Filtered lines → `.log`.** Options: include context lines, include gap
   markers, prefix original line numbers.
2. **Session → `.rogg`** (JSON), for the current tab or all tabs. Same schema
   as session restore. Opening a `.rogg` file restores it.
3. **Copy filter expression** to the clipboard.

### 6.6 Corruption handling

Unparseable settings, profile, history, recent-file or session files are
renamed to `*.bak`, replaced with defaults, and reported with a toast.

## 7. Visual verification

### 7.1 `rogg-gallery`

- **Purpose:** a separate binary that shows every `rogg-ui` component in every
  state, in dark and light, compact and comfortable.
- **Pages:**
  - **Tokens:** color swatches with hex values, type scale, spacing.
  - **Controls:** icon buttons (normal, hover, pressed, focus), flag
    buttons, tool buttons, segmented control, find field (empty, with
    query, with hit count), select.
  - **Tabs:** inactive, hover, active, long label, overflow, all-tabs
    popover.
  - **Sidebar:** folder and file rows, active row, sizes, recent entries,
    footer.
  - **Filter band:** chips in each state × each slot, the `||` separator,
    edit mode, error line, bottom row.
  - **Log view:** normal, hover, selected, pinned, error tint, context, gap
    row, hit spans, search hit; wrap off and on; both densities.
  - **Pane headers, info bar, footer** (with progress).
  - **Empty states:** welcome, no matches, loading, error.
  - **Dialogs:** each dialog from §5.8.
  - **Mockup replica:** the full main window rebuilt from the reference's
    `samples` array, with the same line numbers (120480+), timestamp
    formula, filters (`error` IN, `timeout` IN, `healthcheck` NOT, `retry`
    OFF), pin on index 7 and selection on index 12.
- **Capture mode:** `rogg-gallery --capture <page> --theme <dark|light>
  --density <compact|comfortable> --size <W>x<H> --out <png>
  [--geometry <json>]`.
  - Opens the page at an exact window size and waits for a stable frame.
  - Captures the window with `screencapture -l <windowid>`. The terminal
    needs Screen Recording permission once; `just visual` prints setup
    instructions if it is missing.
  - Writes the geometry of key components as JSON (bounds recorded during
    prepaint).

### 7.2 `tools/visual-diff`

1. **Fonts:** an exact font match is not required. Comparison runs use the
   app's default faces (system UI font, SF Mono). Differences that come only
   from glyph shapes are expected and are not treated as defects.
2. **Render the reference** in headless Chromium (Playwright) at device scale
   2. Hide the study header and footer, size the `.app` element to the
   target window size, and screenshot it for each theme × density. Each
   dialog is captured opened.
3. **Record reference geometry** (a guide, not a hard gate): `getBoundingClientRect` for a fixed list of
   selectors (titlebar, tab-shell, sidebar rows, meta, pane-head, find,
   tool, filter-band, token, flag, row, gap-row, footer, dialog), relative
   to `.app`.
4. **Capture the gallery** replica and dialogs with identical parameters.
5. **Compare:**
   - pixelmatch with anti-aliasing detection, reporting the percentage of
     differing pixels
   - per-selector geometry deltas
   - sampled color checks at fixed points (backgrounds, borders, chip
     fills)
6. **Report:** `target/visual/report.html` with side-by-side, overlay and
   diff images, plus geometry and color tables. The tables sort elements by
   largest deviation so the most visible differences come first.

**Approval process:** there are no fixed numeric pass thresholds. The report
is a tool for a joint review:

1. I iterate on the components until there are no obvious mismatches in
   layout, proportions, colors or states.
2. The user and I go through the report and the live gallery together.
3. Each mismatch found is either fixed or explicitly accepted.

The gate is passed when the user approves. The report's numbers are
guidance, not criteria.

After approval, the captures are committed as golden images. `just visual`
then flags regressions with loose tolerances agreed during the review, so
font rendering noise does not fail the check.

## 8. Testing

- **`rogg-core`:**
  - Unit tests: expression parsing and escaping, state retention on edit,
    filter semantics (every rule in §4.7), context and gap composition,
    encoding detection, CRLF handling, cache keys and eviction, archive
    listing and extraction (fixtures: zip, tar, tar.gz, tgz, gz, nested,
    corrupt), derived sources, history deduplication, settings and session
    round-trips and corruption recovery.
  - Property tests (`proptest`): parallel index == naive index; parallel
    filter == naive single-threaded filter on random content, chunk sizes
    and expressions; incremental append == full reindex.
  - Benchmarks (`criterion`, on `genlog` output): index 1 GB < 2 s; filter
    1 GB with 4 plain terms < 1.5 s; filter with 2 regex terms < 3 s; memory
    for a 1 GB / 10 M-line file under 100 MB beyond the memory map (targets
    on an M-series Mac).
- **`rogg-ui`:** gpui `TestAppContext` tests for component behavior: chip
  state cycling and events, tab close and middle-click, find field events,
  focus traversal, and log view selection, hit-testing and virtualization
  bounds.
- **`rogg-app`:** gpui integration tests driving actions on fixture files:
  open → filter → pin → context → follow → open as tab → export →
  session save and restore; live append and truncate; stdin pipe.
- **Visual:** golden-image regression via `just visual` (§7).
- **Entry points:** `just test` (all cargo tests), `just bench`,
  `just visual`, `just gallery`, `just run`.

## 9. Milestones

Each milestone gets its own implementation plan, review and sign-off.

**M0 — Foundation and visual approval**
- Toolchain install, workspace, pinned dependencies.
- Theme tokens, icons, all `rogg-ui` components (log view on canned data),
  `rogg-gallery` with every page and the mockup replica,
  `tools/visual-diff`.
- **Gate:** joint review of the visual report and live gallery (§7.2); the
  user approves.

**M1 — Engine**
- Plain-file source, encoding, line index, filter parsing, semantics and
  engine, pins, context and gaps, search, statistics, history model.
- `genlog`, property tests, benchmarks meeting targets.

**M2 — Main window**
- App shell, title bar tabs, sidebar (plain files), both panes wired to the
  engine, filter band, follow selection, wrap, find, keyboard, menu bar,
  empty states, progress, recent files.
- `rogg --demo` loads the reference sample data.
- **Gate:** joint review of the real app against the reference, using the
  same report; the user approves.

**M3 — Sources**
- Folders, `.gz`, zip, tar, tar.gz/tgz, nested archives, cache with eviction.
- stdin, live follow with pause, truncate and rotate handling.

**M4 — Advanced**
- Profile editor and profile application.
- Settings dialog.
- Sessions and `.rogg` files.
- Export.
- Open filtered as tab.
- History and statistics popovers.

**Deferred (post-v1):**
- Bundle-wide search with per-file match counts.
- Timestamp detection, jump-to-time, time range limits.
- Merged timeline across files.
- Linux and Windows.
- Additional encodings and the reopen-with-encoding picker.
- User-remappable keybindings UI.
