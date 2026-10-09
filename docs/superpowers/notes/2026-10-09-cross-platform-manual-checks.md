# Cross-platform — manual checks

Hand checks for `docs/superpowers/specs/2026-10-09-rogg-cross-platform-design.md`.
Linux was verified in the container (`./dev linux-test`, `./dev linux-shot`) and Windows was only cross-built, so nothing here has been run on a real desktop yet.
Use a release build with sample data (`rogg --demo`) unless a check says otherwise.

## Linux (run on Wayland and on X11)

- [ ] **Window controls:** minimise, maximise/restore and close each act once per click.
- [ ] **Title bar drag:** dragging the title bar moves the window.
- [ ] **Double-click the title bar** to maximise, and again to restore. Known risk: the first press starts a compositor move, which may swallow the second click on some compositors.
- [ ] **Resize:** from every edge and corner, including when the window is tiled.
- [ ] **☰ menu:** it opens and closes, and the shortcut labels read `Ctrl+O` style.
- [ ] **☰ submenu items:** Edit ▸ Copy copies the selected log line; View ▸ Wrap toggles wrap; Theme and Density ▸ switch.
- [ ] **Ctrl shortcuts:** Ctrl+O, Ctrl+F, Ctrl+L, Ctrl+G, Ctrl+1…9, Ctrl+Q (quits). Nothing needs ⌘.
- [ ] **Data folders:** settings, history and the session appear under `~/.config/rogg` and `~/.local/share/rogg`, and the cache under `~/.cache/rogg` (or the `XDG_*` overrides). About shows the same paths.
- [ ] **Fonts:** Inter Medium and SemiBold render as distinct weights in headings.
- [ ] **Server-side decorations:** under a non-compositing X11 WM, or a Wayland compositor that forces server-side decorations, rogg shows the system title bar and hides its own window controls. The ☰ menu stays.
- [ ] **Stdin:** `cat some.log | rogg -` opens the piped log.

## Windows (first run)

- [ ] Everything in the Linux list that applies: controls, drag, double-click, resize, the ☰ menu and its submenus, and the Ctrl shortcuts. Quit through Alt+F4 or the ☰ menu's Quit.
- [ ] **Snap layouts** appear when hovering the maximise button.
- [ ] **Close button:** it turns red (#c42b1c) with a white ✕ on hover.
- [ ] **Controls** act once per click.
- [ ] **Data folders:** `%APPDATA%\rogg` and `%LOCALAPPDATA%\rogg\cache` are used.
- [ ] **Piping:** `type some.log | rogg.exe -` (and the PowerShell equivalent) opens the log.
- [ ] **Open file and open folder** dialogs work, and Show in Explorer opens the right folder.

### Known Windows-only risks

- **Truncation while open.** While rogg has a log open (memory-mapped), other programs cannot truncate it, so `copytruncate`-style rotation and truncate-on-open writers fail. Check this with a real logger.
- **SMB shares.** A followed file on a network share that another machine truncates may crash rogg: `EXCEPTION_IN_PAGE_ERROR` is not handled.
- **Rotation detection.** A writer that opens the log without `FILE_SHARE_READ` can make rotation detection miss a replaced file.
- **Tests.** Windows tests are compile-checked only. Several existing tests truncate mapped files and would likely fail in a real Windows test run.
