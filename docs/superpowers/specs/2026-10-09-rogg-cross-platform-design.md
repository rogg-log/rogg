# rogg — Linux and Windows support — Design

Date: 2026-10-09
Status: Approved in conversation; written for review
Builds on: `2026-10-06-rogg-design.md` (v1) and `2026-10-08-rogg-refinements-1-design.md`, at `main` 8d68000

## 1. Intent

rogg is macOS-only today. This work makes it build and run on Linux and Windows too.
- **Look:** rogg keeps its own look everywhere, but follows each platform's conventions: keys, window controls, menu, fonts and data folders.
- **Linux** is verified here in a Linux VM (Apple `container`).
- **Windows** is implemented and cross-compiled from macOS, but not run.

**Success means all of the following:**
1. On macOS, behaviour and look are unchanged, `./dev check` is green, and the replica goldens are unchanged.
2. In a Linux container, the workspace builds, `cargo test --workspace` passes, and the real app runs under a virtual X11 display. Screenshots of it, including the ☰ menu open, are produced for the user's review.
3. The workspace builds for `x86_64-pc-windows-msvc` from macOS (`cargo xwin build`). The Windows-specific code paths have unit tests where they can run.
4. Gallery captures of the Linux and Windows chrome, rendered on macOS, are produced for review.

## 2. Decisions (agreed in conversation)

| Topic | Decision |
|---|---|
| Linux verification | Build, the full test suite, and screenshots of the running app under Xvfb with software Vulkan. No Linux goldens. |
| Look and feel | rogg's design with platform conventions: Ctrl instead of ⌘ (labels too), window controls drawn by rogg on the right, an in-window ☰ menu, a bundled font pair, XDG / `%APPDATA%` folders |
| Windows verification | Cross-compiles from macOS with cargo-xwin, plus unit tests for the Windows code paths. Not run. |
| Packaging | Not now. Binaries only. |
| Architecture | A per-crate platform layer. OS calls sit behind `cfg(target_os)`. The UI chrome style is a runtime value, defaulting to the build OS and overridable, so Linux and Windows chrome can be tested and captured on macOS. |

## 3. OS layer — `rogg-core::platform` (no gpui)

All OS-specific code lives in `rogg-core::platform`, with one implementation per OS behind `cfg(target_os)`.

| Concern | macOS (unchanged) | Linux | Windows |
|---|---|---|---|
| Config: settings, profiles | `~/Library/Application Support/rogg` | `$XDG_CONFIG_HOME/rogg`, else `~/.config/rogg` | `%APPDATA%\rogg` |
| Data: history, recent, session | `~/Library/Application Support/rogg` | `$XDG_DATA_HOME/rogg`, else `~/.local/share/rogg` | `%APPDATA%\rogg` |
| Cache | `~/Library/Caches/rogg` | `$XDG_CACHE_HOME/rogg`, else `~/.cache/rogg` | `%LOCALAPPDATA%\rogg\cache` |
| Home folder | `$HOME` | `$HOME` | `%USERPROFILE%` |
| Is a process alive (cache cleanup of `stream-<pid>`/`derived-<pid>` files) | `libc::kill(pid, 0)` (EPERM counts as alive) | same | `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` + `GetExitCodeProcess` = `STILL_ACTIVE` |
| Is stdin a pipe, socket or file (`rogg -`) | `fstat` | same | `GetFileType(GetStdHandle(STD_INPUT_HANDLE))` is `FILE_TYPE_PIPE` or `FILE_TYPE_DISK` |
| Reveal in file manager | gpui `reveal_path` (Finder) | gpui `reveal_path`; falls back to opening the parent folder | gpui `reveal_path` (Explorer) |

- **The public path API stays the same:** `history_path()`, `recent_path()`, `settings_path()`, `profiles_dir()`, `session_path()`, `cache_dir()`, `home_dir()`. Only the base folders differ. `AppPaths` is unchanged.
- **The base-folder rules are pure functions** that take the environment as input, for example `fn config_base(os, env) -> PathBuf`. All three platforms' rules are therefore unit-tested on every OS. Unset or empty variables fall back as the table says, and never become cwd-relative.
- **UI text names the real folders.** The profile editor note ("Profiles are saved in …") and the About dialog's data rows show the actual platform path, never a hard-coded `~/Library`.
- **Unix-only code** (FIFO detection, the non-UTF-8 path argument test) stays behind `cfg(unix)`. Windows gets its own branch where the feature applies.
- **New dependency:** `windows-sys` (Windows target only, pinned, with only the features needed: Threading, System Console, Storage FileSystem, Foundation).

## 4. UI chrome — rogg-ui and rogg-app

### 4.1 Chrome style
- **The type:** `rogg_ui::theme::ChromeStyle { Mac, Linux, Windows }` is a field of the theme.
- **Choosing it:** it defaults to the build OS. The `ROGG_CHROME=mac|linux|windows` environment variable overrides it at startup, and the gallery takes `--chrome mac|linux|windows`.
- **What it decides:** the title bar layout, the window controls, the menu entry point, the shortcut label format and the font pair.

### 4.2 Title bar and window controls
- **Mac:** traffic-light space on the left, as today, with a transparent native title bar.
- **Linux and Windows:** the window opens with client-side decorations (`WindowDecorations::Client`). There is no traffic-light space. rogg draws minimise, maximise/restore and close on the right of the title bar.
  - **Windows style:** 46×42 px rectangular buttons with no gap. Close turns red (`#c42b1c`) with white glyphs on hover. The others take the hover background.
  - **Linux style:** round 24 px buttons with 8 px gaps (GNOME-like), muted glyphs and a hover fill.
- **Behaviour:** double-clicking the title bar's drag area maximises or restores the window. Dragging moves the window. The window edges resize it, using gpui's client-decoration resize areas. The maximise glyph changes when the window is maximised.
- **Ids:** `window-minimize`, `window-maximize`, `window-close`.

### 4.3 Menu
- **Mac:** the native menu bar, unchanged.
- **Linux and Windows:** a ☰ button (`app-menu`) at the left of the title bar opens a dropdown menu.
  - It has the same items, in the same order: About rogg, Settings…, then File (Open…, Open Folder…, Open Recent ▸, Export…, Close Tab), Edit (Copy, Find, Go to Line…), View (Sidebar, Wrap, Open Filtered as Tab, Theme ▸, Density ▸), Window (Minimize, Zoom), and Quit.
  - Each item shows its shortcut.
  - Both menus are built from one definition (`menus.rs`) so they cannot drift apart.

### 4.4 Keys and labels
- **Bindings:** they use `secondary-` (⌘ on macOS, Ctrl elsewhere), so every current macOS binding behaves as before.
- **macOS-only bindings:** ⌘H Hide, ⌥⌘H Hide Others and ⌘M Minimise are bound on macOS only.
- **Quit:** Linux gets `ctrl-q`. On Windows, Alt+F4 closes the window through the OS, and the menu has Quit.
- **Label formatter:** one formatter renders shortcut labels per chrome style. Mac renders `⌘ O`, `⌘ ↑ / ⌘ ↓`, `⌥ ⌘ C`. Linux and Windows render `Ctrl+O`, `Ctrl+↑ / Ctrl+↓`, `Ctrl+Alt+C`. The shortcuts dialog and the ☰ menu both use it.

### 4.5 Fonts
- **macOS:** SF Pro (`.SystemUIFont`) and SF Mono (`.AppleSystemUIFontMonospaced`), unchanged.
- **Linux and Windows:**
  - **The pair:** rogg bundles Inter (Regular, Medium, SemiBold) for the UI and JetBrains Mono (Regular, Bold) for the log and mono text. Both are under the SIL Open Font License 1.1.
  - **Loading:** they are embedded with the other assets and registered with gpui's text system at startup.
  - **Credits:** their licences are listed in About's credits.
  - **The log font picker** still lists the system fonts, and the setting format is unchanged.

### 4.6 Visual baseline
- The replica goldens are Mac-style and must not change.
- Linux and Windows chrome captures (gallery `--chrome`) and the Linux screenshots are for review only. There are no goldens for them.

## 5. Tooling and verification

### 5.1 Linux in Apple `container`
- **Image:** `tools/linux/Containerfile`, based on `debian:bookworm` (arm64, like the host). It contains:
  - rustup with the pinned toolchain (from `rust-toolchain.toml`) and `pkg-config`;
  - gpui's Linux build dependencies: xkbcommon (incl. x11), wayland, X11/xcb, fontconfig, freetype, the Vulkan loader;
  - Mesa's lavapipe (software Vulkan), Xvfb and ImageMagick.
- **Mounts:** the repo is mounted into the container. Builds use `CARGO_TARGET_DIR=target/linux`, so they never clash with macOS builds, and the cargo registry is cached in a named volume.
- **New `./dev` commands** (macOS host):
  - `linux-image`: build or refresh the image;
  - `linux-build`: `cargo build --workspace --all-targets` inside the container;
  - `linux-test`: `cargo test --workspace` inside the container;
  - `linux-shot [demo|<gallery page>]`:
    1. start Xvfb (1440×900×24);
    2. run `rogg --demo` (or the gallery page) with `WGPU_BACKEND=vulkan` on lavapipe;
    3. wait for the first frame;
    4. capture the screen (`import -window root`) into `target/linux/shots/<name>.png`;
    5. for the demo, also send the shortcut that opens the ☰ menu (`xdotool`), or click it, and capture `demo-menu.png`;
    6. quit the app;
  - `linux-shell`: an interactive shell in the container.

### 5.2 Windows from macOS
- `./dev windows-build` runs `cargo xwin build --workspace --target x86_64-pc-windows-msvc`.
- `./dev setup` offers to install `cargo-xwin` and the `x86_64-pc-windows-msvc` rustup target, asking first. cargo-xwin downloads the MSVC CRT and Windows SDK into its own cache on first use.

### 5.3 Portability of the build and tests
- `rogg-app/build.rs` computes the build date in Rust rather than by running `date`. It keeps the `unknown` fallbacks, and git stays optional.
- macOS-only tests are gated with `cfg(target_os = "macos")`:
  - the CoreText real-text harness (`logview_real_text`);
  - the gallery `--capture` tests, which need headless Metal;
  - the release memory test that reads macOS RSS.

  Platform-neutral tests cover their logic.
- `./dev check` on macOS is unchanged, including visual-check and the goldens. `./dev check-all` runs `check`, then `linux-test`, then `windows-build`.

## 6. Testing
- **Path rules:** pure unit tests for all three platforms, run on every OS.
- **Process and stdin checks:** per-OS tests. The Unix ones run on macOS and in the Linux container. The Windows ones are compiled by the cross-build and are not run.
- **ChromeStyle:**
  - title bar layout tests for each style (ids, sizes, presence of the window controls and the ☰ button);
  - label formatter tests for both formats;
  - a menu-definition test checking that the ☰ items match the native menu.
- **Keymap:** a test that every binding resolves for both `secondary-` meanings, and that the macOS-only bindings are absent on other OSes.
- **Fonts:** a test that the bundled fonts load (the files are present and gpui registers the families).
- **Acceptance:**
  - macOS `./dev check` green with the goldens unchanged;
  - `./dev linux-test` green;
  - `./dev linux-shot` screenshots reviewed by the user;
  - `./dev windows-build` succeeds.

## 7. Order
1. The OS layer (§3) and the `build.rs` fix.
2. `ChromeStyle`, the keymap with the label formatter, and the bundled fonts.
3. The Linux and Windows title bar, window controls and ☰ menu, with gallery captures in all three styles.
4. The Linux container tooling, with `linux-test` green.
5. Linux screenshots, handed to the user for review.
6. The Windows cross-build.
7. Docs: a README section per platform, and the design spec updated (the platforms section, and §5.9 keyboard).

## 8. Out of scope
- Installers and packaging (AppImage, .deb, .msi, zip).
- Running or testing on real Windows. CI.
- Linux goldens.
- Wayland-specific testing: the backend is compiled in, but the container tests use X11 (Xvfb).
- System tray, single-instance behaviour, file associations, "Open with".
- HiDPI or fractional scaling tuning beyond gpui's defaults.
- Platform-specific drag-and-drop details beyond what gpui provides.
