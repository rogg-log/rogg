# M4 manual checks

Release build: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo build --release -p rogg-app`, then run
`target/release/rogg`. These need a real machine (native panels, quit, the window manager, the system appearance).

## Settings
1. ⌘, opens Settings; the title bar's settings button too.
2. Theme System, then switch macOS between light and dark (System Settings → Appearance): rogg follows at once.
3. Log font: SF Mono, Menlo, Monaco, Courier New (only the installed ones are listed); "Other…" lists installed
   monospace fonts only (no proportional fonts); picking one applies it and returns to Settings.
4. Log font size 9 and 20 in both densities: rows stay readable, pins stay centred, selection and search outlines fit.
5. UI font size 11 and 16: bars keep their heights; no text is clipped in the title bar, info bar, filter band, footer.
6. Cache size limit 1 GB with a large bundle open: closing tabs frees the cache (Finder: ~/Library/Caches/rogg).
7. Folder scan depth 1, then open a folder with nested logs: only top-level files are listed.
8. Quit and relaunch: every setting is kept.

## Profiles
9. Profile: … in the info bar opens the editor for that tab's profile; choosing another profile restyles the tab.
10. New, Duplicate, Rename (Enter applies), Delete (not on Default); tabs using a deleted profile switch to Default.
11. Edit a rule's pattern, flags, effects, panels and colours; rows change while typing; an invalid regex shows its error.
12. Light theme: a hex colour's derived light variant is readable; an override replaces it.
13. Palette: add and edit a slot; filter chips and filtered-log marks use the new colours.
14. Import… / Export… through the native panels; a damaged `.roggprofile` is reported and left untouched.

## Export and derived tabs
15. ⌘E: export filtered lines with each option; the footer shows `Exporting N%` on a 1 GB file; the UI stays responsive;
    closing the tab mid-export leaves no file.
16. Save session… for this tab and for all tabs; open the `.rogg` with ⌘O: the tabs come back with filters and pins.
17. Copy: the clipboard holds the expression.
18. ⇧⌘T on a 1 GB file's filtered view: `Building tab N%`, then the tab, numbered from 1 like an ordinary file; a nested
    tab works the same; the breadcrumb jumps back to the parents.
19. A live file: ⇧⌘T shows `Snapshot · HH:MM:SS`.

## Session
20. Open files, an archive member, a derived tab; set filters, pins, selection, wrap, split, sidebar width; quit with ⌘Q
    and relaunch: everything is back, including the window frame (also when it was maximised).
21. Move a restored file away and relaunch: its tab shows the error state with Retry; the others are fine.
22. Settings → Restore session off: relaunch opens empty.

## Statistics, history, defaults and carry-over fixes
23. Click the footer statistics: `Filtered log statistics` opens; on a 1 GB file, its rows update while the filter runs.
24. The filter history popover: each entry shows its term states plus `regex` / `case` / `±N` when set; clicking one
    restores the expression, states, flags and context.
25. Settings behaviour defaults (wrap, follow selection, context lines, regex and case for new tabs, line spacing,
    default highlighting profile): open a new file and it starts with exactly those.
26. An idle live tab still shows appended lines within about 0.1 s (`just live`); a `.gz` with trailing junk after the
    stream opens fully.
