# Refinements 1 — manual checks

Hand checks for each iteration of `docs/superpowers/specs/2026-10-08-rogg-refinements-1-design.md`.
Run them in `./dev demo` (release build with the sample data) unless a check says otherwise.

## Iteration A — UI polish and About

- [ ] **Folder chevron:** in the sidebar, an open folder's chevron points down and a closed one points right. Clicking the folder row turns it, and the chevron sits where the old glyph was, with no jump in the label.
- [ ] **All-tabs button:** the title bar's "open files" button (left of ⌘) shows the same chevron, pointing down. Clicking it still opens the Open files picker.
- [ ] **Labels:** the info bar reads `Profile: Services` with its dot, and the filter band reads `History` with its icon. Neither has a trailing `⌄`.
- [ ] **Breadcrumbs:** the `›` between path segments in the info bar is unchanged.
- [ ] **Band height:** with the filter band in view, enter edit mode three ways: the ✎ button, ⌘L, and Enter on the filter line. The band must not grow or shrink, and the panes above and below must not move.
- [ ] **Editor text:** in edit mode the expression text is vertically centred in its box and not clipped; the same goes for `Apply ↵`.
- [ ] **About:** rogg ▸ About rogg (the first item, followed by a separator) opens `About rogg`. It should show:
  - `rogg 0.1.0` and the tagline;
  - under Build, the short commit, today's build date, and the `rustc` version;
  - under Credits, the "Built with" line and 14 crates with their licenses.
- [ ] **Show in Finder:** the first data row opens Finder at `~/Library/Application Support/rogg`; the second opens `~/Library/Caches/rogg`, which is created first if it is missing.
- [ ] **Dialog dismissal:** Escape, the ✕ button, and a click outside each close About.
