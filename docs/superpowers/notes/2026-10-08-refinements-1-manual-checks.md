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
- [ ] **Live log:** run `./dev live` in a terminal and open its file. In the filtered pane, character-select part of a line while new lines arrive: the selection stays put, and ⌘C copies just those characters. Pinning a line or typing in Find keeps it too. Applying a different filter clears it.
- [ ] **Click with a wiggle:** click a line with a small trackpad or mouse wiggle: the row is selected, not a single character.
- [ ] **Huge full-line dialog:** open `Show full line…` on a line over 1 MB: the dialog opens and scrolls without a noticeable stall.

## Iteration C — colours and recent filters

- [ ] **Custom colour picker:** open Highlighting (the info bar's Profile button), duplicate Default, and select a rule. The Colour row shows the preset swatches, then the custom-colour swatch (a square in the rule's current colour with a small pen, no label), then the hex field. It opens a popover in rogg's colours on the HSLA tab: hue, saturation, lightness and alpha sliders, plus hex. The Palette tab next to it shows rogg's preset colours on top, then gpui-kit's own swatch grid; every pen swatch (rule, light theme, palette slots) has presets there.
- [ ] **Picking a colour:** dragging a slider or typing a hex in the popover recolours the matching lines at once. The hex field shows `#rrggbb`, or `#rrggbbaa` once alpha is below 100. Reopening Highlighting keeps the colour (it is saved).
- [ ] **Light theme:** the Light theme row has its own pen swatch. Leave it empty, and switching to the light theme shows the derived light variant. After a pick, the light theme shows exactly that colour.
- [ ] **Palette slots:** in "Filter-match palette", select a slot. Text, background, and both light-theme fields each have their own pen swatch, and a pick recolours the filter hits.
- [ ] **Saving a drag:** dragging a slider recolours the log live; the profile file (`~/Library/Application Support/rogg/profiles/…toml`) changes once, on release, not on every move.
- [ ] **Short window:** at the minimum window height (or with the split dragged low), the recent-filters list stays inside the filtered pane and never covers the pane header or the filter band.
- [ ] **Editor open:** with the expression editor open (⌘L) on a filter that matches nothing, clicking a recent filter closes the editor and applies the pick.
- [ ] **Staying in sync:** typing a hex into the field or clicking a preset swatch moves the pen swatch and the popover's sliders to the new colour.
- [ ] **Recent filters:** apply three or four filters, then one that matches nothing. Under `No matching lines`, `Recent filters` lists up to three earlier filters, newest first, each with its expression (mono) and summary. The current filter is not listed. Clicking one applies it, restoring terms, on/off states, `.*`, `Aa` and context, and the matches appear.
- [ ] **No history:** with a fresh history, a filter that matches nothing shows the plain `No matching lines`.
