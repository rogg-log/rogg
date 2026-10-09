# Cross-platform — manual checks

Hand checks for `docs/superpowers/specs/2026-10-09-rogg-cross-platform-design.md`.
Linux was verified in the container (`./dev linux-test`, `./dev linux-shot`) and Windows was only cross-built, so nothing here has been run on a real desktop yet.
Use a release build with sample data (`rogg --demo`) unless a check says otherwise.

## Linux (run on Wayland and on X11)

- [ ] **Window controls:** minimise, maximise/restore and close each act once per click.
- [ ] **Title bar drag:** dragging the title bar moves the window.
- [ ] **Double-click the title bar** (the empty drag area and the `rogg` label) to maximise, and again to restore. The move starts only once the pointer moves with the button held, so the second press should reach rogg; check that a slow drag still moves the window.
- [ ] **Resize:** from every edge and corner, including when the window is tiled. The resize zones come from gpui-kit's window border (4 px either side of the frame line); the log's right-edge scrollbar should still be draggable.
- [ ] **☰ menu:** it opens and closes, and the shortcut labels read `Ctrl+O` style.
- [ ] **☰ submenu items:** Edit ▸ Copy copies the selected log line; View ▸ Wrap toggles wrap; Theme and Density ▸ switch.
- [ ] **Ctrl shortcuts:** Ctrl+O, Ctrl+F, Ctrl+L, Ctrl+G, Ctrl+1…9, Ctrl+Q (quits). Nothing needs ⌘.
- [ ] **Data folders:** settings, history and the session appear under `~/.config/rogg` and `~/.local/share/rogg`, and the cache under `~/.cache/rogg` (or the `XDG_*` overrides). About shows the same paths: both the config and the data folders on Linux (two rows, "Settings and profiles" and "Session, history and recent files"), each with a working Show in Files button, then the cache.
- [ ] **Welcome screen:** the buttons read `Open file…  Ctrl+O` and `Open folder…  Ctrl+Shift+O`.
- [ ] **Platform wording:** About's tagline says "for Linux", Settings › Theme says "System follows the desktop appearance", and the Other… font picker says "installed on this computer".
- [ ] **Window grouping:** the dock or task switcher groups rogg's windows under the app id `rogg` (Wayland app_id, X11 WM_CLASS).
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
- [ ] **No console (release build):** double-clicking `rogg.exe` opens no console window, and `type file.log | rogg.exe -` still opens the piped log.
- [ ] **Open file and open folder** dialogs work, and Show in Explorer opens the right folder.
- [ ] **Welcome and wording:** the welcome buttons read `Ctrl+O` / `Ctrl+Shift+O`, and About's tagline says "for Windows".

### Known Windows-only risks

- **Truncation while open.** While rogg has a log open (memory-mapped), other programs cannot truncate it, so `copytruncate`-style rotation and truncate-on-open writers fail. Check this with a real logger.
- **SMB shares.** A followed file on a network share that another machine truncates may crash rogg: `EXCEPTION_IN_PAGE_ERROR` is not handled.
- **Rotation detection.** A writer that opens the log without `FILE_SHARE_READ` can make rotation detection miss a replaced file.
- **Tests.** Windows tests are compile-checked only. Several existing tests truncate mapped files and would likely fail in a real Windows test run.
