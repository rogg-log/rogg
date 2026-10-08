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

## Iteration B — long rows, full line, character selection

Use `./dev demo`, then a real log with very long lines (for example `./dev genlog`, or any file with lines over a few thousand characters).

- [ ] **3-line cap:** turn Wrap on. A line that wraps past three lines shows exactly three, with a faint `…` just right of the third. The next line starts right below it, and scrolling past it is smooth with no jumps. Lines of one to three wrapped lines look as before, and Wrap off is unchanged.
- [ ] **Show full line…:** right-click any line (Wrap on or off, either pane): the menu has `Show full line…` after `Copy with line numbers`. It opens `Line <n>` with the whole line in the log font and size, wrapped, scrollable and selectable (⌘A and ⌘C work inside). `Copy` copies the whole line, including the part past the 64 KB display cut. Escape and ✕ close it.
- [ ] **Very long line:** a line over 1 MB shows the first 1 MB and the note. Copy still pastes the whole line.
- [ ] **Character drag:** press on a line's text and drag within that line: the characters are highlighted in the selection colour, and that row's selection background is gone (the accent bar stays). ⌘C pastes just those characters, and so does the right-click menu's `Copy`. `Copy with line numbers` copies the whole row.
- [ ] **Non-ASCII:** the same drag across `é`, `日` or emoji never splits a character.
- [ ] **Into another row:** drag from one line into the next: whole rows are selected, as before. Coming back to the first line keeps whole rows until you release.
- [ ] **Clearing:** a plain click, an arrow key, or a new filter clears the character selection.
- [ ] **Wrapped rows:** in a wrapped row, drag from its first visual line onto its second: the highlight follows across both visual lines.
- [ ] **Cut line:** on a line cut at 64 KB (it ends in `…`), dragging to the end stops before the `…`.
- [ ] **Pointer:** the pointer stays the arrow throughout and never turns into an I-beam.
