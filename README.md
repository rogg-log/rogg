# rogg

rogg is a native log explorer in the spirit of glogg/klogg, written in Rust with gpui. It shows the full log on top, a filter line in the middle and the filtered log at the bottom. It opens single logs up to about 1 GB and zip/tar.gz support bundles, indexes and filters in the background, and follows live files.

Design: `docs/superpowers/specs/2026-10-06-rogg-design.md` (the app) and `docs/superpowers/specs/2026-10-09-rogg-cross-platform-design.md` (Linux and Windows).

`./dev` is the single entry point for development. `./dev help` lists every command.

## macOS

The primary platform, fully verified.

- **Build and run:** `./dev setup` once, then `./dev build`, `./dev run [files...]` (`-` reads stdin), `./dev demo`, `./dev gallery`.
- **Check:** `./dev check` builds, runs clippy, all tests and the golden visual check (it must report 0.000%).
- **Data:** settings, profiles, history, recent files and the session live in `~/Library/Application Support/rogg`; the cache is `~/Library/Caches/rogg`.
- **Keys:** the shortcuts use ⌘ (for example ⌘O, ⌘F, ⌘L). The native menu bar is used.

## Linux

Verified in a container, not yet on a real desktop. Wayland and X11 are both supported by gpui. The window draws its own title bar and ☰ menu; under a window manager that insists on server-side decorations, the system title bar is shown and rogg hides its own window controls.

- **Build and run (on a Linux machine):** `cargo build --release`, then `target/release/rogg [files...]`. The gpui build dependencies are listed in `tools/linux/Containerfile`.
- **From macOS:** `./dev linux-image` builds the `debian:trixie` image (needs Apple `container`). `./dev linux-build` and `./dev linux-test` build and test inside it. `./dev linux-shot [demo|<page>]` screenshots the running app under Xvfb with xfwm4 and lavapipe into `target/linux/shots/`. `./dev linux-shell` opens a shell.
  - `ROGG_LINUX_CPUS` (default 6) and `ROGG_LINUX_MEMORY` (default 20G) set the container limits.
- **Data:** settings and profiles in `$XDG_CONFIG_HOME/rogg` (else `~/.config/rogg`); history, recent files and the session in `$XDG_DATA_HOME/rogg` (else `~/.local/share/rogg`); the cache in `$XDG_CACHE_HOME/rogg` (else `~/.cache/rogg`).
- **Keys:** Ctrl replaces ⌘ (Ctrl+O, Ctrl+F, Ctrl+L), and Ctrl+Q quits. The ⌘H (Hide), ⌥⌘H (Hide Others) and ⌘M (Minimise) shortcuts are macOS-only; Minimise itself stays in the ☰ menu and the window controls.
- **Verified:** the workspace builds and its tests pass in the container, and `linux-shot` checks that the window renders. Hand checks on a real desktop are listed in `docs/superpowers/notes/2026-10-09-cross-platform-manual-checks.md`.

## Windows

Cross-built from macOS only; it has not been run.

- **Build:** `./dev windows-build` runs `cargo xwin build --workspace --target x86_64-pc-windows-msvc`. It needs the rustup target `x86_64-pc-windows-msvc`, `cargo-xwin` and Homebrew `llvm`; `./dev setup` offers all three after asking.
- **Run:** copy `target/x86_64-pc-windows-msvc/debug/rogg.exe` to a Windows machine. `rogg.exe -` reads stdin. That debug build keeps a console window for its output; a release build opens none, but it has to be made on Windows (`cargo build --release`), because gpui compiles its release shaders with the Windows SDK's `fxc.exe`.
- **Data:** settings, profiles, history, recent files and the session in `%APPDATA%\rogg`; the cache in `%LOCALAPPDATA%\rogg\cache`.
- **Keys:** Ctrl replaces ⌘. Alt+F4 closes the window, and the ☰ menu has Quit. The ⌘H, ⌥⌘H and ⌘M shortcuts are macOS-only; Minimise stays in the ☰ menu and the window controls.
- **Verified:** it compiles and links, and the Windows code paths are compile-checked. The tests are compile-checked only. The manual checks note covers the first run and the known Windows-only risks around memory-mapped logs.

## Other notes

- `ROGG_CHROME=mac|linux|windows` overrides the window chrome style at startup, and the gallery takes `--chrome`. On macOS, `./dev capture <page> <appearance> <density> [chrome]` captures the other styles.
- `./dev check-all` runs `check`, `linux-test` and `windows-build`.

## Builds

Every push to `main` runs [`.github/workflows/build.yml`](.github/workflows/build.yml). It runs clippy and the
tests on Linux, then builds:

| File | Platform |
|---|---|
| `rogg-macos-universal.tar.gz` | macOS 11+, Apple silicon and Intel: the bare `rogg` binary (ad-hoc signed, not notarised) |
| `rogg-macos-universal-app.tar.gz` | the same binary as `rogg.app`, to open from Finder (ad-hoc signed, not notarised) |
| `rogg-linux-x86_64.tar.gz` | Linux x86_64, glibc 2.35+ |
| `rogg-linux-arm64.tar.gz` | Linux arm64, glibc 2.35+ |
| `rogg-windows-x86_64.zip` | Windows x86_64 |

Each build becomes its own release, `v<version>-build.<n>` (titled e.g. `rogg 0.1.0 · build 5 · 2026-10-10 ·
5497615`), with `SHA256SUMS` and `manifest.json` (version, build number, commit, and each file's checksum and `kind`,
`binary` or `app`). The release is published only once every file is attached, and is marked latest; the newest 30
builds are kept. Latest files: `https://github.com/rogg-log/rogg/releases/latest/download/<file>`; a given build:
`…/releases/download/v0.1.0-build.5/<file>`. Check every download against `SHA256SUMS`. While the repository is
private, downloads need a GitHub token (`gh release download`).

`./dev ci-test` tests the packaging scripts locally; `./dev ci-lint` lints the workflow; `./dev mac-app` builds
`rogg.app` for this Mac into `target/mac-app/`; `./dev icon` regenerates the app icon from `crates/rogg-app/assets/icon/rogg.svg`.
