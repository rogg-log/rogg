# gpui-framework — extraction of rogg's GUI into a reusable project — Design

Date: 2026-10-08
Status: Approved in conversation; written for review
Source: rogg at `main` f899486 (v1 M0–M4 + Refinements 1)
Target: `/Users/vadim/dev/projects/gpui-framework` (a new, local-only git repository)

## 1. Intent

rogg's GUI came out well, and the user wants to reuse it in future desktop GUI projects. This project extracts the GUI into a dedicated framework that a new app can depend on and look right with no styling of its own. The framework covers:
- the components;
- the design system: the theme, the tokens and the visual decisions;
- the specs, the documentation, the gallery and the visual-regression tooling.

**Success means all of the following:**
- A new app adds one dependency (`gpui-framework`, used as `gf`), installs the default theme, opens a window, and looks like rogg.
- The starter template builds and runs.
- The framework's own `./dev check` is green: build, clippy, tests, and the visual check against its own goldens.
- The showcase page reproduces rogg's main window from framework parts. The user reviews it against a rogg capture before the goldens are approved.

## 2. Decisions (agreed in conversation)

| Topic | Decision |
|---|---|
| Relationship to rogg | **One-time copy.** rogg stays untouched and keeps its own `rogg-ui`; the two copies may diverge |
| Scope | All generic components, **plus** the log view and the filter band, generalised into domain-neutral widgets (`text_view`, `chip_band`) |
| Structure | **One component crate plus two apps**: `gpui-framework` (the library), `gpui-framework-gallery`, `gpui-framework-template` |
| Naming | Workspace and crate `gpui-framework`, imported as `gf` in examples and docs (`use gpui_framework as gf;`). No "rogg" anywhere in names, types, ids, assets or docs, except `docs/history/` |
| Theming | rogg's look is the **built-in default** (dark/light × compact/comfortable). Apps may override the accent, any palette role, the fonts and the base sizes through a `ThemeBuilder` |
| Extras | Gallery app with `--capture`; golden-image visual regression; a starter app template; a `./dev` entry point (plus a `justfile` wrapper) |
| Docs | A design-system spec, a component reference, guides, and rogg's specs archived in `docs/history/` |
| Git | `git init` in the target folder, local only: no remote, nothing pushed |
| Toolchain | Rust 1.99.0 (`rust-toolchain.toml`), edition 2024, gpui-pre 0.3.8 through gpui-kit **=0.7.1**, re-exported as `gf::kit` so apps cannot drift. macOS only, like rogg |

## 3. Repository layout

```
gpui-framework/
  Cargo.toml                   workspace (members below), shared package metadata, pinned deps
  rust-toolchain.toml          1.99.0
  dev                          the single entry point (zsh), adapted from rogg's
  justfile                     thin wrapper around ./dev
  README.md                    what it is, showcase screenshot, quick start, ./dev commands
  crates/
    gpui-framework/            the library
      assets/icons/gf/         the SVG icons (rogg's 23, renamed directory)
      src/
        lib.rs                 module tree; Callback / IndexCallback; re-export `kit` (= gpui_kit)
        theme/                 Theme, ThemeBuilder, Palette, Metrics + size, Density, Fonts, TextScale,
                               Color, Appearance, ActiveTheme, kit mirror
        assets.rs icons.rs
        controls/              IconButton, FlagButton, ToolButton, TextButton, ActionButton,
                               Segmented, CompactSelect, FindField
        form.rs                FormRow, SectionLabel, Toggle, Stepper, Swatch, TextField, CustomColor, pen_ink
        dialogs.rs             DialogFrame (+ width), ModalLayer, RuleRow, ShortcutRow, PickerItem,
                               generic builders (§5.3)
        toast.rs empty.rs layout.rs
        chrome/                title_bar, sidebar, pane_header, info_bar, status_bar
        widgets/
          text_view/           from logview (§5.1)
          chip_band/           from filter (§5.2)
      tests/                   headless component tests + the real-text harness (harness = false)
    gpui-framework-gallery/    pages + showcase + --capture
    gpui-framework-template/   a minimal starter app
  tools/visual-diff/           goldens check/approve (Node + Playwright pixel diff)
  docs/
    spec/design-system.md      the framework's binding spec
    spec/2026-10-08-gpui-framework-extraction-design.md   this document (founding record)
    components/*.md            one page per component
    guides/*.md                getting-started, theming, testing, visual-regression, gpui-gotchas, new-app
    history/                   rogg-design.md, rogg-refinements-1-design.md (verbatim, archive header)
```

## 4. Library: what moves and how it is renamed

Every component keeps today's contract: plain data plus callbacks, no app state, `RenderOnce` builders, element ids marked with `.test_support()`.

### 4.1 Theme

| rogg | framework |
|---|---|
| `RoggTheme`, `ActiveRoggTheme`, `cx.rogg()` | `Theme`, `ActiveTheme`, `cx.gf_theme()`. gpui-kit already defines `cx.theme()`, so the new accessor must not clash with it |
| `RoggTheme::new(appearance, density)` / `install` | `Theme::new(appearance, density)` / `install`, giving exactly rogg's look; `Theme::default_dark()` / `default_light()` |
| none | **`ThemeBuilder`**: start from a default theme, then override any of: the accent (and the roles derived from it), any `Palette` role, the UI/mono/log font families, the log font size, the UI text scale, and the density. `build()` gives a `Theme`, and `install` also re-mirrors gpui-kit |
| `theme/kit.rs` mirror | unchanged, including the slider and tab tokens added in Refinements 1 |
| `size::*` constants (`CHIP_H`, `DIALOG_W`, `PROFILE_DIALOG_W`, `FULL_LINE_DIALOG_W`, …) | kept. Domain names become generic (`PROFILE_DIALOG_W` → `WIDE_DIALOG_W`, `FULL_LINE_*` → `TEXT_VIEWER_*`) |

### 4.2 Controls, forms, toasts, empty states, layout

- **Moved as is:**
  - controls: `IconButton` (including `chevron_down`), `FlagButton`, `ToolButton`, `TextButton`, `ActionButton`, `Segmented`, `CompactSelect`, `FindField`;
  - form controls: `FormRow`, `SectionLabel`, `Toggle`, `Stepper`, `Swatch`, `TextField` (with the fixed width that never shrinks), `CustomColor` (the swatch with a pen), `pen_ink`;
  - `ToastStack`, `ModalLayer`, `DialogFrame`.
- **Dropped,** because they belong to rogg's highlighting editor: `EditableRuleRow`, `PaletteSlot`, `ProfileRuleView`.
- **Empty states:**
  - `EmptyState` with its builder, `loading`, `error`;
  - `with_recent_list(state, items, on_pick)`, generalised from `no_matches_with_recent`: a titled list of entries, each a mono line plus a summary, under any empty state;
  - `welcome(title, body, actions, recent)`, generic: the app passes its own action buttons.
- **Layout:** `MainWindowLayout` and its parts (sidebar, two panes with a draggable band between them, or a single body), `LayoutMeasure`, `BandDrag`, `SidebarDrag`.

### 4.3 Chrome

- **`title_bar`:** traffic-light space, a sidebar toggle, scrollable tabs (`TabItem`), and right-side actions. The actions are the all-tabs chevron, shortcuts and settings, each optional through its callback.
- **`sidebar`:** a tree (`TreeRow::{Folder, File}` with chevrons, notes and a failed state), a recent list and a footer.
- **`pane_header`:** a title, a count, a find field and tool buttons.
- **`info_bar`:** breadcrumbs plus a generic list of right-side actions (`TextButton`s). rogg's "Profile: X" and "Export" become one example.
- **`status_bar`:** the parts list (`StatPart`) and an optional `Progress`.

### 4.4 Icons and assets

- The icon set moves with its names: `IconName`, `icon()` and `chevron(open)`. The asset directory becomes `icons/gf/`.
- `Assets` embeds them, and apps chain it with their own assets.

## 5. The widgets

### 5.1 `widgets::text_view` (from `logview`)

A virtualised view of numbered text lines, built for millions of rows. Everything rogg has today carries over unchanged:
- **Layout:** the number gutter and the marker gutter; wrap, with the 3-line cap and the `…` marker; horizontal scroll, with the hover-only scrollbar.
- **Scrolling:** tail mode; scroll anchoring across row replacement.
- **Selection:**
  - row selection, by click, shift-click, drag across rows and the keyboard;
  - character selection: a drag within a row, a 3 px threshold, kept across rebuilds while the row exists, cleared by a click or a key.
- **Copy:** of the selected characters, or of whole rows with or without numbers.
- **Text handling:** the 64 KB display cut; `is_display_cut`; the right-click word lookup; real-text hit testing.

| rogg | framework |
|---|---|
| `LogView`, `LogViewState`, `LogViewElement` | `TextView`, `TextViewState`, `TextViewElement` |
| `LogLine { number, text, spans, pinned, context, tint }` | `Line { number, text, spans, marked, dimmed, tint }` |
| `LogRow::{Line, Gap { omitted }}` | `Row::{Line, Gap { label }}`. The app supplies the label; the gallery uses "··· N lines omitted" |
| `LogRowSource`, `VecRows` | `RowSource`, `VecRows` (same methods, `is_display_cut` included) |
| `SpanStyle::{Faint, Muted, Error, Amber, Fg, Hit(n), Mark{fg,bg}, Bold, SearchHit}` | `SpanStyle::{Faint, Muted, Error, Warning, Fg, Highlight(n), Mark{fg,bg}, Bold, Outline}` |
| `RowTint::{Error, Color}` | `RowTint::{Error, Color}` |
| events `Selected`, `TogglePin`, `AddTerm`, `TailLeft`, `ShowFullLine` | `Selected`, `ToggleMark`, `TailLeft`, `ContextAction { id, number, word }` |
| a hard-coded context menu | built-in `Copy` and `Copy with line numbers`, plus an app-supplied list of `MenuAction { id, label(word) }` items. A `label` may depend on the word under the pointer, and an item may be disabled when there is no word |
| `log_view` actions, the `"LogView"` key context | `text_view` actions, the `"TextView"` key context |
| `CharSelection`, `MAX_WRAPPED_LINES`, `MAX_COPY_ROWS`, `display_text`, `token_at`, `segments` | unchanged |

The gallery and the template show rogg's "Filter on …", "Exclude …" and "Show full line…" as examples of app actions. The text viewer dialog (§5.3) is the generic "Show full line".

### 5.2 `widgets::chip_band` (from `filter`)

| rogg | framework |
|---|---|
| `FilterBand`, `FilterChip`, `ChipState`, `ChipColor`, `chip_color` | `ChipBand`, `Chip`, `ChipState` (IN / OFF / NOT, `next`), `ChipColor`, `chip_color` |
| the `.*` and `Aa` flags | a generic list of `(id, label, pressed)` toggle flags |
| History button, Context select, summary, hint | all optional parameters: a left action button (label and icon), a labelled select, a summary text and a hint text |
| expression editor and Apply | kept, with the 28 px height from Refinements 1. The Apply label is a parameter |
| the band handle and `BandDrag` | kept |

### 5.3 Dialog builders

- **Generic builders:**
  - `picker(title, items, on_pick)`;
  - `key_value_list(title, rows)`, which replaces the statistics and appearance builders;
  - `shortcuts(title, rows)`;
  - `about(AboutView, on_reveal)`;
  - `text_viewer(title, textarea, cut_note, on_copy)`, from "Show full line", with `TEXT_VIEWER_SHOWN_BYTES` = 1 MB;
  - `input_prompt(title, input)`, from "Go to line".
- **Dropped,** being rogg-only: `history`, `history_picker`, `export`, `profile`, `all_tabs`. An app builds these from `picker` and `RuleRow`.

## 6. Gallery, showcase, template

- **The gallery** keeps rogg's pages, retitled and with neutral data:
  - foundations: colours, type scale, metrics, icons;
  - controls and form controls, including the open custom-colour picker;
  - dialogs, toasts, empty states, chrome, and the widgets.

  `--capture <page> --theme dark|light --density compact|comfortable --out <png> --geometry <json>` works as it does now.
- **The showcase page** replaces rogg's replica. It is a full main window built only from framework parts:
  - the title bar with tabs;
  - the sidebar tree;
  - two text-view panes with the chip band between them;
  - the info and status bars.

  Its fixed sample data is a fake service log with highlighted spans, a marked line and gap rows. It is the main golden page.
- **The template** is about 200 lines, compiled in the workspace:
  - a window with the title bar, a sidebar, one text view (sample rows) and a status bar;
  - an About dialog and a toast;
  - app menus and keybindings, with a light/dark switch;
  - theme install, with a `ThemeBuilder` accent override shown as a commented example.

  `docs/guides/new-app.md` explains how to copy it into a new repository and depend on the framework by path.

## 7. Tooling

- **`./dev` commands:**
  - `help`, `setup`, `build [--release]`, `test [crate] [filter]`, `lint`, `fmt`;
  - `gallery`, `smoke`, `template`, `capture`;
  - `visual-check`, `visual-approve` (asks first);
  - `check [--no-visual]`, which is the gate: build, lint, test, visual-check;
  - `clean`.

  `DRY=1` prints the commands without running them. The rogg-only commands are not carried over: `demo`, `run`, `genlog`, `bundle`, `live`, `pipe`, `bench`, `visual`, `visual-app`.
- **`tools/visual-diff`** carries over `gallery.mjs`, `check.mjs`, `approve.mjs`, `config.mjs` and `compare.mjs`, as needed for golden checks. It drops `reference.mjs`, the HTML reference and the replica-vs-reference report.
  - It captures the showcase and the dialog overlays, in dark/light and compact/comfortable, with a 0.05 % threshold.
  - The goldens are generated fresh in the new repo and approved once by the user.

## 8. Testing

- **What moves:** every `rogg-ui` test, with renamed types and ids:
  - the headless `TestAppContext` component and geometry tests;
  - the CoreText real-text harness (`harness = false`), which covers char boundaries, dense hits, word lookup, the wrap cap, and character drags;
  - the unit tests.
- **New tests:**
  - `ThemeBuilder` overrides: the accent, a palette role, fonts, and mirroring into gpui-kit;
  - the generic dialog and empty-state builders;
  - `text_view` context actions: an app item receives the line number and the word;
  - `chip_band` with custom flags and optional parts;
  - the gallery capture smoke tests;
  - the template compiles and opens headlessly (a smoke test).
- **What stays with rogg:** rogg-app's integration tests. Their component-level coverage already exists in the moved tests.
- **The gate:** `./dev check` green in the new repo, the template runs, and the user reviews the showcase against a rogg capture before `./dev visual-approve`.

## 9. Documentation

- **`docs/spec/design-system.md`** is the binding spec. It is rogg spec §5 (theme, icons, window layout, chip, text-view element, interactions, empty and transient states, dialogs and popovers, keyboard) and §7 (gallery, visual diff), rewritten in generic terms. It also covers:
  - the accepted visual decisions: SF Mono through `.AppleSystemUIFontMonospaced`; the vertical thumb always visible and the horizontal one shown on hover or scroll; the 1 px focus line on the perimeter; the gap label in the UI font; the letter-spacing approximation;
  - the Refinements 1 behaviour: chevrons, the band height, the 3-line cap, character selection, the custom-colour swatch with a pen, and the recent list in empty states.
- **`docs/components/<name>.md`,** one page per public component and widget: what it's for, its builder API, its callbacks and events, its test element ids, its states and density behaviour, and a gallery capture.
- **`docs/guides/`:**
  - `getting-started.md`: add the dependency, install the theme and assets, open a window;
  - `theming.md`: `ThemeBuilder`, the palette roles, densities, the text scale;
  - `testing.md`: headless versus the real-text harness and why; ids and `test_support`; patterns from rogg;
  - `visual-regression.md`: capture, check, approve, reviewing diffs;
  - `gpui-gotchas.md`: rogg's API research notes plus the lessons learned. For example:
    - the test platform's `reveal_path` panics;
    - the kit `Input`'s own text size overrides inherited styles, so style the input itself;
    - "SF Mono" falls back to Helvetica;
    - `TestAppContext`'s text system is a no-op;
    - kit theme tokens need mirroring after `Theme::change`;
  - `new-app.md`: start from the template.
- **`docs/history/`** holds `rogg-design.md` (2026-10-06) and `rogg-refinements-1-design.md` (2026-10-08) verbatim, each with a header saying it is the archive this framework came from and is not binding.
- **`README.md`.**

## 10. Process and stages

This spec lives in rogg's `docs/superpowers/specs/`, because the new repo does not exist until the plan runs. The plan's first task creates the repo (`git init`, local, no remote) and copies this spec into `docs/spec/`.

Stages, each ending with `./dev check` green in the new repo (the visual check joins at stage 9):
1. Scaffold: the workspace, toolchain, `./dev`, `justfile`, README skeleton, and the copied spec.
2. Theme, `ThemeBuilder`, icons, assets, controls.
3. Forms, dialogs (generic builders), toasts, empty states.
4. Layout and chrome.
5. `widgets::text_view`.
6. `widgets::chip_band`.
7. Gallery and showcase.
8. Template.
9. Visual tooling and goldens. The user reviews the showcase against a rogg capture, then approves.
10. Docs: the design-system spec, component pages, guides, history.

rogg is not modified. The only rogg commits are this spec and its plan.

## 11. Out of scope

- Publishing to crates.io; CI; Windows and Linux.
- Migrating rogg onto the framework (this is a one-time copy).
- New components beyond rogg's, except `ThemeBuilder` and the generic dialog and empty-state builders.
- rogg's engine and domain: `rogg-core`, sources, the filter-expression engine, profiles, settings, sessions, export.
