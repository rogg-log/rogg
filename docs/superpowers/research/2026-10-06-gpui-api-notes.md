# rogg API notes: gpui-pre 0.3.8 + gpui-kit 0.7.1 (macOS)

Research date 2026-10-06. Sources extracted under `/tmp/rogg-research/`:
`gpui-pre-0.3.8`, `gpui-pre-macros-0.3.8`, `gpui-pre-platform-0.3.8`, `gpui-pre-macos-0.3.8`, `gpui-pre-apple-0.3.8`,
`gpui-base-0.7.1`, `gpui-component-0.7.1`, `gpui-kit-0.7.1`, `gpui-kit-assets-0.7.1`.
Citations are `file:line` relative to the extracted crate sources (re-extract with `curl -sL -A rogg https://crates.io/api/v1/crates/<name>/<ver>/download | tar xz`) (abbreviations are defined at the top of each section: `G/`/`gp/` = `gpui-pre-0.3.8/src/`, `GC/` = `gpui-component-0.7.1/src/`, `GB/` = `gpui-base-0.7.1/src/`, `GKA/` = `gpui-kit-assets-0.7.1/`, `GKT/` = `gpui-kit-0.7.1/tests/`, `macos/` = `gpui-pre-macos-0.3.8/src/`, `apple/` = `gpui-pre-apple-0.3.8/src/`).

## TL;DR (things verified by actually compiling/running, not just reading)

Scratch crates (isolated `CARGO_HOME=/tmp/rogg-research/cargo-home`, nothing installed system-wide):
`/tmp/rogg-research/scratch` (plain `gpui-kit = "=0.7.1"`) and `/tmp/rogg-research/scratch2` (`features = ["test-support"]`).

- `Cargo.toml`: just `gpui-kit = "=0.7.1"`; `use gpui_kit::*;` is gpui. Clean build in ~1 min with rustc 1.99; **no Xcode `metal` needed** (gpui-kit forces `runtime_shaders`).
- `gpui_kit::application().with_assets(gpui_kit::assets::Assets).with_quit_mode(QuitMode::LastWindowClosed).run(|cx| { gpui_kit::init(cx); gpui_kit::open_window(opts, cx, |window, cx| cx.new(|cx| View::new(window, cx))) })` - compiles & runs.
- `/tmp/rogg-research/scratch/examples/apicheck.rs` (full source in the Appendix) **compiles and runs without panic**, exercising: custom `Element` impl (request_layout/prepaint/paint, `insert_hitbox`, `is_hovered`, `paint_quad(fill(..))`, `shape_line(..).paint(..)`, `on_mouse_event`), `uniform_list` + `cx.processor` + `UniformListScrollHandle::scroll_to_item_strict(ix, ScrollStrategy::Center)`, `actions!`, `KeyBinding::new("cmd-f", Find, Some("LogView"))`, `cx.bind_keys`, `cx.on_action`, `cx.set_menus(vec![Menu::new(..).items([MenuItem::action(..), MenuItem::separator()])])`, `key_context`/`track_focus`/`on_action(cx.listener(..))`, `cx.spawn(async move |this, cx| ..)` + `background_executor().timer(..)` + `background_spawn`, `InputState`/`Input` (`.xsmall().appearance(false).cleanable(true).prefix(..).text_size(px(10.))`, `Styled::h(input, px(24.))`), `subscribe_in` + `InputEvent`, `Button` `.ghost().compact()`, `ButtonCustomVariant`, `Switch`, `Checkbox`, `TitleBar` + `TabBar`/`Tab`, `Tooltip`, `context_menu` (PopupMenu), `window.push_notification`, `window.open_dialog`, `Theme::change(ThemeMode::Dark, None, cx)` + `Theme::update`, `TitleBar::window_options()`, `svg().path(..).size(..).text_color(..)`, `.hover`, `.font_weight(FontWeight(550.))`, `BoxShadow::new(..).blur_radius(..)`, `rgba(0xRRGGBBAA)`, `hsla(..)`.
- **Native window number** works via `raw-window-handle` 0.6 + `objc2` 0.6 (`HasWindowHandle::window_handle(window)` -> NSView -> `window` -> `windowNumber`), see Q5. But `screencapture -l<n>` failed from this shell ("could not create image from window" - Screen Recording permission), so prefer:
- **In-process pixel capture works** (with `gpui-kit/test-support`, debug build): headless `HeadlessAppContext` + `capture_screenshot` (no window, no permission) and live-window `window.render_to_image()`. Both produced correct PNGs (`/tmp/rogg-research/scratch2/headless.png`, `live.png`), 2x scale.
- **Element geometry dump works** (with `test-support`): mark elements `.id("x").test_support()`; `gpui_kit::test::TestWindowExt::find(window, "x").bounds()` and `gpui_kit::base::test_support::snapshots(window)` return window-local bounds - in headless AND in a live app. `debug_selector`/`debug_bounds` is only readable via `VisualTestContext` (Q15).
- Window size: `WindowBounds::Windowed(Bounds::new(origin, size(640,400)))` gives **content** size 640x400; `window.bounds()` = 640x432 with the default titlebar, 640x400 with `appears_transparent: true`. `Bounds::centered` clamps to the display.
- Fonts on this Mac: `"Menlo"` resolves; **`"SF Mono"` silently resolves to Helvetica**; `".AppleSystemUIFontMonospaced"` resolves (system SF Mono). System UI font = `".SystemUIFont"`.
- Gotchas: `use gpui_kit::*; use gpui_kit::component::*;` -> `Size` ambiguous (E0659, verified); `Input::h()` is multi-line-only and shadows `Styled::h`; `.context_menu(..)` returns a wrapper - put `.debug_selector`/`.id`-dependent calls before it; `QuitMode::Default` = Explicit on macOS; gpui-component `theme::init` forces Light (sync with system yourself); `Theme::global_mut` doesn't propagate - use `Theme::update`.

## A. Dependencies & bootstrapping

Crates involved (all extracted under `/tmp/rogg-research/`):

| crate (package) | lib name | role |
|---|---|---|
| `gpui-kit 0.7.1` | `gpui_kit` | facade: re-exports gpui, platform, base, component, assets |
| `gpui-pre 0.3.8` | `gpui` | Zed gpui snapshot of `zed@279fe07` (`gpui-pre-0.3.8/Cargo.toml` description) |
| `gpui-pre-macros 0.3.8` | `gpui_macros` | derive/proc macros (with gpui-kit facade path rewriting) |
| `gpui-pre-platform 0.3.8` | `gpui_platform` | `application()`, `headless()`, `current_platform()` |
| `gpui-pre-macos 0.3.8` | `gpui_macos` | `MacPlatform`, `MacWindow`, CoreText text system |
| `gpui-pre-apple 0.3.8` | `gpui_apple` | Metal renderer + `shaders.metal` + build.rs (shader compile) |
| `gpui-base 0.7.1` | `gpui_base` | unstyled behaviour, `Root`, theme tokens, test_support |
| `gpui-component 0.7.1` | `gpui_component` | styled components |
| `gpui-kit-assets 0.7.1` | `gpui_kit_assets` | Lucide icons via rust-embed |

Dependency chain for macOS: `gpui-kit` -> `gpui_platform` (`gpui-pre-platform`) -> `gpui_macos` (`gpui-pre-macos`, `cfg(target_os="macos")`) -> `gpui_apple` (`gpui-pre-apple`).
- `gpui-kit-0.7.1/Cargo.toml:401-409`:
  ```toml
  [target.'cfg(not(any(target_os = "ios", target_os = "android")))'.dependencies.gpui_platform]
  version = "=0.3.8"
  features = ["font-kit", "x11", "wayland", "runtime_shaders"]
  package = "gpui-pre-platform"
  ```
- `gpui-pre-platform-0.3.8/Cargo.toml` (normalized): `[target.'cfg(target_os = "macos")'.dependencies.gpui_macos] version="=0.3.8", default-features=false, package="gpui-pre-macos"`; feature `runtime_shaders = ["gpui_macos/runtime_shaders"]`.
- `gpui-pre-macos-0.3.8/Cargo.toml`: `runtime_shaders = ["gpui_apple/runtime_shaders"]`.

### 1. Cargo lines, facade, platform selection, minimal main

**An app depends on `gpui-kit` alone.** No direct `gpui-pre` dependency is needed or recommended.

```toml
[dependencies]
gpui-kit = "=0.7.1"          # default features = ["component", "assets"]

[dev-dependencies]
gpui-kit = { version = "=0.7.1", features = ["test-support"] }
```

Evidence:
- `gpui-kit-0.7.1/README.md:3-12`: "One dependency ... `gpui-kit` depends on the matching set of GPUI crates, so an application never lists GPUI itself. `use gpui_kit::*;` is GPUI".
- Default features: `gpui-kit-0.7.1/Cargo.toml.orig` `[features] default = ["component", "assets"]`; `component = ["dep:gpui-component"]`, `assets = ["dep:gpui-kit-assets"]`, `test-support = ["gpui/test-support", "gpui_platform/test-support", "gpui-base/test-support", "gpui-component?/test-support"]`, plus `inspector`, `profiler`, `decimal`, `speech`, `tree-sitter*`.
- Re-exports, `gpui-kit-0.7.1/src/lib.rs`:
  - `:96` `pub use ::gpui::*;` (everything in gpui at the root of `gpui_kit`)
  - `:98-99` `#[doc(hidden)] pub use crate as gpui;` -> `gpui_kit::gpui::Window` works, but it is the *kit* crate, not the gpui crate (comment `:80-94`).
  - `:106` `pub use ::gpui_base as base;`
  - `:108` `pub use ::gpui_platform as platform;` (non-iOS/Android)
  - `:139` `pub use ::gpui_component as component;` (feature `component`)
  - `:142` `pub use ::gpui_kit_assets as assets;` (feature `assets`)
  - `:167` `pub use ::gpui_platform::application;`
  - `:46-77` own `actions!` macro (gpui's `actions!` expands `$crate::Action` which also works, but kit's is the documented one).
- Proc-macro path rewriting: `gpui-pre-macros-0.3.8/src/gpui_pre_facade_paths.rs:11-18` uses `proc_macro_crate::crate_name("gpui-kit")`; if the *consuming crate* depends on `gpui-kit`, every `gpui::` / `gpui_platform::` path emitted by `#[derive(IntoElement)]`, `#[derive(Render)]`, `#[derive(Action)]`, `#[gpui::test]`, style macros etc. is rewritten to `gpui_kit::` / `gpui_kit::platform::` (`:30-35`, `:50-67`). So derives work with only `gpui-kit` in Cargo.toml. (If you ALSO add `gpui = { package = "gpui-pre" }` it still compiles since types are identical, but it is pointless.)
- Kit's own compile-time check that the glob is enough: `gpui-kit-0.7.1/tests/exports.rs:10-43` (`use gpui_kit::*; actions!(exports,[Quit]); #[derive(Action, Clone, PartialEq)] ... #[derive(Render)] ... #[derive(IntoElement)] ...`).

**Platform selection.** There is NO `Application::new()` in this snapshot. `gpui-pre-0.3.8/src/app.rs:178`:
```rust
pub fn with_platform(platform: Rc<dyn Platform>) -> Self
```
is the only constructor (plus `new_inaccessible(platform)` `:195`). The macOS platform is chosen by `gpui_platform`:
- `gpui-pre-platform-0.3.8/src/gpui_platform.rs:13-21`
  ```rust
  pub fn application() -> gpui::Application {
      #[cfg(not(target_family = "wasm"))]
      gpui::Application::with_platform(current_platform(false))
  }
  ```
- `:23-25` `pub fn headless() -> gpui::Application` (= `current_platform(true)`)
- `:67-71` `pub fn current_platform(headless: bool) -> Rc<dyn Platform>` -> `Rc::new(gpui_macos::MacPlatform::new(headless))` on macOS.
- Re-exported as `gpui_kit::application` (`gpui-kit-0.7.1/src/lib.rs:167`) and `gpui_kit::platform::{application, headless, current_platform}`.

Other `Application` builder methods (`gpui-pre-0.3.8/src/app.rs`):
```rust
pub fn with_assets(self, asset_source: impl AssetSource) -> Self            // :202
pub fn with_http_client(self, http_client: Arc<dyn HttpClient>) -> Self     // :218
pub fn with_quit_mode(self, mode: QuitMode) -> Self                         // :227
pub fn with_windowing(self, request: WindowingRequest) -> Self              // :240
pub fn run<F>(self, on_finish_launching: F) where F: 'static + FnOnce(&mut App) // :248
```
`QuitMode` (`app.rs:400-408`): `Default` (= `Explicit` on macOS: app does NOT quit when last window closes), `LastWindowClosed`, `Explicit`. For a single-window log viewer use `.with_quit_mode(QuitMode::LastWindowClosed)` or handle `cx.on_window_closed`/`Quit` action yourself.

`App::open_window` (`app.rs:1431-1434`):
```rust
pub fn open_window<V: 'static + Render>(
    &mut self,
    options: crate::WindowOptions,
    build_root_view: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> anyhow::Result<WindowHandle<V>>
```
`App::activate(&self, ignoring_other_apps: bool)` (`app.rs:1467`) - gpui examples call `cx.activate(true)` after opening the window (`gpui-pre-0.3.8/examples/hello_world.rs:114`) so it comes to front when launched from a terminal.

**Kit window helper** (`gpui-kit-0.7.1/src/lib.rs:148-164`):
```rust
pub fn open_window<V: Render>(
    options: WindowOptions,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> Result<(AnyWindowHandle, Entity<V>)>
// body: cx.open_window(options, |window, cx| { let view = build(window, cx); ...; cx.new(|cx| base::Root::new(view, window, cx)) })
```
It wraps your view in `gpui_base::Root` (same type re-exported as `gpui_component::Root`, `gpui-component-0.7.1/src/lib.rs:97`). Returns the `AnyWindowHandle` (not `WindowHandle<Root>`).

**Minimal working main** (from `gpui-kit-0.7.1/README.md:25-43`, same in `src/lib.rs:118-136`, and `gpui-component-0.7.1/README.md:133-166`):
```rust
use gpui_kit::component::button::*;
use gpui_kit::*;

struct Hello;

impl Render for Hello {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(Button::new("ok").primary().label("Let's Go!"))
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Hello))
            .expect("failed to open window");
    });
}
```
With icons (component README `gpui-component-0.7.1/README.md:170-175`): `gpui_kit::application().with_assets(gpui_kit::assets::Assets)`.

Equivalent manual form (if you want your own root wrapping):
```rust
cx.open_window(options, |window, cx| {
    let view = cx.new(|cx| MyView::new(window, cx));
    cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
})?;
```

SCRATCH VERIFICATION: see "Build verification" at the end of this file.

### 2. Metal shaders / Xcode

- Shader build lives in `gpui-pre-apple-0.3.8/build.rs` (NOT in gpui-pre / gpui-pre-macos). `:18-25`:
  ```rust
  pub fn run() {
      let header_path = generate_shader_bindings();   // cbindgen -> scene.h
      #[cfg(feature = "runtime_shaders")]
      emit_stitched_shaders(&header_path);
      #[cfg(not(feature = "runtime_shaders"))]
      compile_metal_shaders(&header_path);
  }
  ```
- Without the feature it shells out to `xcrun -sdk macosx metal ...` and `xcrun -sdk macosx metallib` (`:162-200`) -> needs full Xcode (Command Line Tools alone have no `metal`).
- With `runtime_shaders` it only writes `OUT_DIR/stitched_shaders.metal` (`:109-124`) and compiles at startup via `device.new_library_with_source(&SHADERS_SOURCE_FILE, ...)` (`gpui-pre-apple-0.3.8/src/metal_renderer.rs:30-33, 250-256`).
- Feature name: **`runtime_shaders`**, defined in `gpui-pre-apple` (`runtime_shaders = []`), forwarded by `gpui-pre-macos` and `gpui-pre-platform` (both `runtime_shaders = [...]`).
- **gpui-kit already enables it unconditionally** (`gpui-kit-0.7.1/Cargo.toml:401-409`, features list contains `"runtime_shaders"`). So an app depending on gpui-kit does not need Xcode for shader compilation. (Only if you depend on `gpui-pre-platform` directly without gpui-kit do you need to add `features = ["runtime_shaders"]`.) Cost: shader compile at each app launch (small one-time startup cost).
- cbindgen still runs at build time (build-dependency), no Xcode needed for that.
- (FYI this machine does have `xcrun -f metal` -> `/Applications/Xcode-beta.app/.../metal`.)

### 3. Required init calls, assets, Root

- `gpui_kit::init(cx: &mut App)` (`gpui-kit-0.7.1/src/lib.rs:174-180`) = `gpui_component::init(cx)` when `component` feature is on (else `gpui_base::init`). Doc: "Call it once, before using anything else."
- `gpui_component::init` (`gpui-component-0.7.1/src/lib.rs:128-150`) calls `theme::init`, `global_state::init`, `inspector::init` (only `#[cfg(any(feature = "inspector", debug_assertions))]`), `root::init`, `gpui_base::init`, `input::init` (registers Input key bindings), `dock`, `sheet`, `list`, `command`, `notification`, `popover`, `menu`, `table`, `tooltip`, ... Must be called before creating any component, otherwise key bindings/themes are missing.
- `gpui_base::init` (`gpui-base-0.7.1/src/lib.rs:228-245`): `Theme::global_mut(cx)`, dialog/focus_trap/popover/sheet/select/input/... init.
- Assets: `Application::with_assets(self, asset_source: impl AssetSource) -> Self` (`gpui-pre-0.3.8/src/app.rs:202`); kit default icon set `gpui_kit::assets::Assets` (see Q18 for combining with own SVGs).
- `Root::new` (`gpui-base-0.7.1/src/root.rs:147`):
  ```rust
  pub fn new(view: impl Into<AnyView>, window: &mut Window, cx: &mut Context<Self>) -> Self
  ```
  On macOS (non-test) it also installs an accessibility hit-test forwarder on the NSWindow class (`root.rs:148-149`, `gpui-base-0.7.1/src/macos_accessibility.rs:16-34`).
  Accessors: `Root::read(window, cx) -> &Root` (`:176`), `Root::update(window, cx, |root, window, cx| ..)` (`:183`), `root.view() -> &AnyView`, `root.plugin::<V>()`. The component dialog/notification layer is a `RootPlugin` (`gpui-component-0.7.1/src/root.rs:434` `impl gpui_base::RootPlugin for WindowState`), registered by `gpui_component::init` -> `root::init`. So `gpui_kit::open_window` + `gpui_kit::init` is sufficient for dialogs/notifications (details in Q16).

### 4. Edition / MSRV

- Every crate: `edition = "2024"` (`*/Cargo.toml:13`), none declares `rust-version` (grep for `rust-version` in all `Cargo.toml`/`Cargo.toml.orig` returns nothing).
- Edition 2024 => rustc >= 1.85. The code uses let-chains (`if !focus_assigned && let Some(focus_id) = ...`, `gpui-pre-0.3.8/src/element.rs:621`; `text_system.rs:198,846`), stabilized in Rust 1.88 => **effective MSRV >= 1.88**; exact minimum cannot be determined from source. Use current stable (local toolchain: rustc 1.99.0 at `~/.rustup/toolchains/stable-aarch64-apple-darwin`).
- Pinning: kit pins `gpui-pre = "=0.3.8"` exactly (`gpui-kit-0.7.1/Cargo.toml` `[dependencies.gpui] version = "=0.3.8"`), so pin `gpui-kit = "=0.7.1"` too.

### Build verification (scratch project)

A scratch crate `/tmp/rogg-research/scratch` (`Cargo.toml`: only `gpui-kit = "=0.7.1"`, edition 2024) with the main above plus `.with_assets(gpui_kit::assets::Assets)`, `Bounds::centered(None, size(px(800.), px(600.)), cx)`, `WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }`, `actions!(scratch, [Quit])`, `rgb(..)` styling and a `Button` **compiled cleanly with rustc 1.99.0 in 58 s** (isolated `CARGO_HOME=/tmp/rogg-research/cargo-home`, no Xcode `metal` invoked thanks to `runtime_shaders`), and the binary ran for 5 s without panicking (no stderr output). Only warning: future-incompat in transitive `block v0.1.6`.

## B. Windowing (Q5–Q7) and Fonts (Q14)

Paths are relative to `/tmp/rogg-research/`. `gpui-pre` = `gpui-pre-0.3.8/src`, `macos` = `gpui-pre-macos-0.3.8/src`, `apple` = `gpui-pre-apple-0.3.8/src`, `platform` = `gpui-pre-platform-0.3.8/src`.

### 5. WindowOptions, TitlebarOptions, WindowBounds, opening a window, native window id, capture

**`WindowOptions`**: `gpui-pre-0.3.8/src/platform.rs:2436-2507` (`#[derive(Debug)]`, no `Clone`):

```rust
pub struct WindowOptions {
    pub window_bounds: Option<WindowBounds>,        // None = inherit/cascade
    pub titlebar: Option<TitlebarOptions>,
    pub focus: bool,
    pub show: bool,
    pub kind: WindowKind,
    pub is_movable: bool,
    pub app_owns_titlebar_drag: bool,               // NEW vs upstream (see Q6)
    pub inactive_frame_interval: Option<Duration>,  // NEW vs upstream
    pub is_resizable: bool,
    pub is_minimizable: bool,
    pub display_id: Option<DisplayId>,
    pub window_background: WindowBackgroundAppearance,
    pub app_id: Option<String>,
    pub window_min_size: Option<Size<Pixels>>,
    pub window_decorations: Option<WindowDecorations>,
    pub icon: Option<Arc<image::RgbaImage>>,        // X11 only
    pub tabbing_identifier: Option<String>,
}
```

Defaults (`platform.rs:2609-2635`): `window_bounds: None`, `titlebar: Some(TitlebarOptions { title: None, appears_transparent: false, traffic_light_position: None })`, `focus: true`, `show: true`, `kind: WindowKind::Normal`, `is_movable: true`, `app_owns_titlebar_drag: false`, `inactive_frame_interval: Some(Duration::from_micros(33_333))`, `is_resizable: true`, `is_minimizable: true`, the rest `None`/default.

**`TitlebarOptions`**: `platform.rs:2637-2649`:

```rust
#[derive(Debug, Default)]
pub struct TitlebarOptions {
    pub title: Option<SharedString>,
    pub appears_transparent: bool,               // hide system titlebar (macOS/Windows)
    pub traffic_light_position: Option<Point<Pixels>>,
}
```

You can also change the traffic-light position at runtime with `pub fn set_traffic_light_position(&self, position: Point<Pixels>)` (`window.rs:2931`).

**`WindowBounds`**: `platform.rs:2574-2607`:

```rust
pub enum WindowBounds { Windowed(Bounds<Pixels>), Maximized(Bounds<Pixels>), Fullscreen(Bounds<Pixels>) }
impl WindowBounds {
    pub fn get_bounds(&self) -> Bounds<Pixels>;
    pub fn centered(size: Size<Pixels>, cx: &App) -> Self;   // = Windowed(Bounds::centered(None, size, cx))
}
```

`Bounds::centered(display_id: Option<DisplayId>, size: Size<Pixels>, cx: &App) -> Self` (`geometry.rs:740`) **clamps the size to the display's visible bounds** (`size.min(&visible_bounds.size)`, `geometry.rs:748`). For an exact size in screenshots, use `Bounds::new(point(..), size(..))` (`geometry.rs:785`) or make sure the display is big enough. Helper constructors: `pub const fn point` (`geometry.rs:111`), `pub const fn size` (`geometry.rs:425`), `pub fn bounds` (`geometry.rs:731`).

`WindowKind` (`platform.rs:2651-2680`): `Normal`, `PopUp`, `AnchoredPopup(popup::PopupOptions)` (new), `Floating`, `LayerShell(..)` (Linux/wayland only), and a modal variant. `WindowBackgroundAppearance` (`platform.rs:2713-2733`): `Opaque` (default), `Transparent`, `Blurred`, `MicaBackdrop`, `MicaAltBackdrop`.

**Opening a window**: `App::open_window` (`gpui-pre-0.3.8/src/app.rs:1431-1435`):

```rust
pub fn open_window<V: 'static + Render>(
    &mut self,
    options: crate::WindowOptions,
    build_root_view: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> anyhow::Result<WindowHandle<V>>
```

The gpui-kit wrapper `gpui_kit::open_window(options, cx, |window, cx| -> Entity<V>)` wraps the view in `gpui_base::Root` and returns `(AnyWindowHandle, Entity<V>)` (`gpui-kit-0.7.1/src/lib.rs:148-164`).

Fixed-size window with a custom titlebar (based on `examples/hello_world.rs:100-112` and `gpui-component title_bar.rs:59-92`):

```rust
let opts = WindowOptions {
    window_bounds: Some(WindowBounds::Windowed(Bounds::new(point(px(100.), px(100.)), size(px(1200.), px(760.))))),
    titlebar: Some(TitlebarOptions { title: None, appears_transparent: true,
                                     traffic_light_position: Some(point(px(9.), px(9.))) }),
    app_owns_titlebar_drag: true,   // we draw the titlebar and call start_window_move
    is_resizable: false,            // optional: lock size (examples/window.rs:225 uses it)
    window_min_size: Some(size(px(800.), px(500.))),
    ..Default::default()
};
// or: WindowOptions { window_bounds: ..., ..gpui_kit::component::TitleBar::window_options() }
```

Other `Window` getters: `pub fn bounds(&self) -> Bounds<Pixels>` (screen coordinates, `window.rs:2745`), `pub fn window_bounds(&self) -> WindowBounds` (`window.rs:2464`), `pub fn viewport_size(&self) -> Size<Pixels>` (`window.rs:2810`), `pub fn scale_factor(&self) -> f32` (`window.rs:2973`), `pub fn resize(&mut self, size: Size<Pixels>)` (`window.rs:2774`), `pub fn set_window_title(&mut self, title: &str)` (`window.rs:2924`).

**Native window number / CGWindowID**: gpui does **not** expose it. `windowNumber` does not appear in gpui-pre or gpui-pre-macos (grep for `windowNumber|window_number|CGWindowID` returns nothing). What gpui does expose is `raw-window-handle` 0.6 (`gpui-pre-0.3.8/Cargo.toml:418-419`):

```rust
impl HasWindowHandle for Window { fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> }   // window.rs:7434-7437
impl HasDisplayHandle for Window { ... }                                                                                    // window.rs:7440-7445
```

On macOS it returns `RawWindowHandle::AppKit(AppKitWindowHandle::new(native_view))`, which is an **NSView** pointer, not an NSWindow (`gpui-pre-macos-0.3.8/src/window.rs:2410-2418`). To get the CGWindowID, the app needs its own `raw-window-handle = "0.6"` dependency, because gpui does not re-export the crate. It also needs objc (`objc2`/`cocoa`) to call `[[view window] windowNumber]`:

```rust
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
// NOTE: `window.window_handle()` resolves to the INHERENT method
// `pub fn window_handle(&self) -> AnyWindowHandle` (window.rs:2311), so call the trait explicitly:
let h = HasWindowHandle::window_handle(window).unwrap();
if let RawWindowHandle::AppKit(a) = h.as_raw() {
    let ns_view = a.ns_view.as_ptr() as *mut objc2::runtime::AnyObject;
    let ns_window: *mut AnyObject = unsafe { objc2::msg_send![ns_view, window] };
    let num: isize = unsafe { objc2::msg_send![ns_window, windowNumber] };  // == CGWindowID for `screencapture -l<num>`
}
```

The source confirms that the handle points at an NSView. **This snippet was compiled and run - see "Runtime verification" at the end of this section (printed `windowNumber = Some(3876)`).**

**Offscreen and headless capture.** This snapshot has more of it than upstream. Everything below is behind `feature = "test-support"`:

1. `Window::render_to_image` (`window.rs:2749-2756`):
   ```rust
   #[cfg(any(test, feature = "test-support"))]
   pub fn render_to_image(&self) -> anyhow::Result<image::RgbaImage>
   ```
   It renders `rendered_frame.scene` again into the window's next CAMetalLayer drawable without presenting it, then reads the pixels back (`apple/metal_renderer.rs:565-592`, via `macos/window.rs:2350-2354`). The readback converts BGRA to RGBA (`metal_renderer.rs:1287-1313`). It only works if the layer has `framebuffer_only(false)`, and that is set **only when `test-support` and `debug_assertions` are both on** (`metal_renderer.rs:183-187`). So in a real running app this works in a **debug build with the gpui `test-support` feature** (for example `gpui-kit/test-support`). In release builds, reading a framebuffer-only texture is not valid. Callers in the source: `HeadlessAppContext::capture_screenshot` (`app/headless_app_context.rs:168-171`) and `VisualTestAppContext::capture_screenshot` (`app/visual_test_context.rs:384-386`).
2. `Window::painted_quads(&self) -> Vec<Quad>` and `painted_underlines()` (`window.rs:2757-2770`, test-support) return the scene primitives of the last frame, so you can assert on geometry without rasterizing.
3. **`HeadlessAppContext`** (`app/headless_app_context.rs`, exported at `app.rs:32-33` under `any(test, feature="test-support")`) is a cross-platform `TestPlatform` with a real text system and an optional real GPU renderer:
   ```rust
   pub fn with_platform(platform_text_system: Arc<dyn PlatformTextSystem>, asset_source: Arc<dyn AssetSource>,
       renderer_factory: impl Fn() -> Result<Option<Box<dyn PlatformHeadlessRenderer>>> + 'static) -> Self   // :65-69
   pub fn open_window<V: Render + 'static>(&mut self, size: Size<Pixels>, build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>) -> Result<WindowHandle<V>>  // :102-106 (show:false, focus:false)
   pub fn update<R>(&mut self, f: impl FnOnce(&mut App) -> R) -> R                                              // :149
   pub fn update_window<R>(&mut self, window: AnyWindowHandle, f: impl FnOnce(AnyView, &mut Window, &mut App) -> R) -> Result<R>  // :155-159
   pub fn capture_screenshot(&mut self, window: AnyWindowHandle) -> Result<RgbaImage>                          // :168
   pub fn run_until_parked(&self); pub fn advance_clock(&self, d: Duration);
   ```
   The headless `TestWindow::render_to_image` calls `renderer.render_scene_to_image(scene, bounds.size.to_device_pixels(scale_factor))` (`platform/test/window.rs:543-553`). The test window `scale_factor` defaults to **2.0** (`platform/test/window.rs:129`), so images come out at 2x. The Metal headless renderer is `gpui_platform::current_headless_renderer()` (`platform/gpui_platform.rs:109-118`, `cfg(any(feature="bench-support", feature="test-support"))`). It returns `MetalHeadlessRenderer::new()`, which renders into an offscreen `MTLTexture` (`apple/metal_renderer.rs:599-620, 1641-1672`). No window and no display are needed.
   **This is exactly what `gpui-kit-0.7.1/tests/rendering.rs` does** (`:51-58`, `:73-101`):
   ```rust
   let mut cx = HeadlessAppContext::with_platform(
       gpui_kit::platform::current_platform(true).text_system(),   // real CoreText shaping
       assets,                                                     // Arc<dyn AssetSource>
       gpui_kit::platform::current_headless_renderer,              // Metal offscreen
   );
   cx.update(gpui_kit::init);
   let (handle, _) = cx.update(|cx| gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds{origin: Default::default(), size: size(px(80.), px(60.))})), focus: false, show: false, ..Default::default() }, cx, |_, cx| cx.new(|_| Checked))).unwrap();
   cx.update_window(handle, |_, window, cx| { window.render_frame(cx); /* TestWindowExt */ }).unwrap();
   let rgba: image::RgbaImage = cx.capture_screenshot(handle).unwrap();
   ```
   The harness is a plain `fn main()` with `harness = false` and `test = false` (`gpui-kit-0.7.1/Cargo.toml.orig`, `[[test]] name="rendering"`), which keeps it on the main thread. It needs `gpui-kit` features `test-support` + `assets`. **This is usable outside `cargo test`**: any binary or example that enables `gpui-kit/test-support` can call these APIs, because they are ordinary `pub` items behind a Cargo feature. It is the most robust route for deterministic, display-independent PNG snapshots of rogg views: save with `image::RgbaImage::save`, which needs your own `image` dependency.
4. **`VisualTestAppContext`** (`app/visual_test_context.rs`, macOS + test-support only, `app.rs:40-41`) runs a real `MacPlatform` with a deterministic dispatcher. `pub fn new(platform: Rc<dyn Platform>) -> Self` / `with_asset_source(platform, asset_source)` (`:46-62`). `open_offscreen_window(size, build_root)` places a *shown* window at (-10000, -10000) (`:93-118`). `capture_screenshot(window)` (`:384`).
5. `gpui_platform::headless() -> gpui::Application` (`platform/gpui_platform.rs:23-25`) creates `MacPlatform::new(true)`, a headless app with no capture.
6. The `screen-capture` feature (`gpui-pre-platform` Cargo.toml features) enables ScreenCaptureKit *screen sources* for streaming (`platform.rs:22-34, 359-370`). It is not a window-screenshot API.
7. There is no `render_to_image` or screenshot API outside `test-support`, and no `toggle_headless`-based capture. `examples/toggle_headless.rs` only switches windowing modes (`WindowingRequest`).

### 6. Window drag in a custom titlebar

- `pub fn start_window_move(&self)` (`window.rs:2884-2890`). On macOS it calls `[window performWindowDragWithEvent: [NSApp currentEvent]]` (`macos/window.rs:2209-2220`), so call it **from inside a mouse event handler** (mouse-down or mouse-move).
- `WindowControlArea { Drag, Close, Max, Min }` (`window.rs:747-758`), applied with `InteractiveElement::window_control_area(self, area: WindowControlArea) -> Self` (`elements/div.rs:1255-1260`). **On macOS this does nothing**: `fn on_hit_test_window_control(&self, _callback: ...) {}` is empty (`macos/window.rs:2050-2051`). It only matters on Windows. On macOS you must call `start_window_move` yourself.
- `WindowOptions::app_owns_titlebar_drag: bool` (`platform.rs:2461-2472`) is new. Set it to `true` when drawing your own titlebar: the whole content view is then reported as app-owned titlebar content (`macos/window.rs:3528-3545`), so AppKit neither drags the window natively nor delays clicks to detect double-clicks ("a delay first observed on macOS 27").
- `pub fn titlebar_double_click(&self)` (`window.rs:6792-6796`) honors the user's `AppleActionOnDoubleClick` setting (Minimize/Maximize/Fill) (`macos/window.rs:2142-2207`). Also available: `zoom_window()` (`:2875`), `minimize_window()` (`:6497`), `toggle_fullscreen()` (`:6502`), `is_fullscreen()` (`:2779`), `is_maximized()` (`:2418`).
- Reference implementation: `gpui-component-0.7.1/src/title_bar.rs:320-386`. It sets a `should_move` flag in `on_mouse_down(Left)`, clears it on mouse-up and `on_mouse_down_out`, and calls `start_window_move()` in `on_mouse_move` when the flag is set. That way a click without movement still reaches the children. On macOS it also binds `on_double_click(|_, window, _| window.titlebar_double_click())`. `TitleBar::window_options()` (`title_bar.rs:81-92`) returns `appears_transparent: true`, `traffic_light_position: Some(point(px(9.), px(9.)))`, `app_owns_titlebar_drag: true`. `TITLE_BAR_HEIGHT = px(34.)` (`:15`), and the left padding on macOS is `px(80.)` (`:17`) to clear the traffic lights. Simpler pattern: `.on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())` (`examples/window_shadow.rs:171-176`). Interactive children such as tabs should `cx.stop_propagation()` on mouse-down, so a tab click does not start a move. This is inferred from the flag design, not stated in the source.

### 7. Window appearance (dark/light)

- `pub enum WindowAppearance { Light /*default*/, VibrantLight, Dark, VibrantDark }` (`platform.rs:2685-2708`). gpui-component maps it with `impl From<WindowAppearance> for ThemeMode` (`gpui-component theme/mod.rs:829`).
- `Window::appearance(&self) -> WindowAppearance` (`window.rs:2805`) reads `NSWindow.effectiveAppearance` (`macos/window.rs:1633-1638`).
- `App::window_appearance(&self) -> WindowAppearance` (`app.rs:1578`) reads `NSApp.effectiveAppearance` (`macos/platform.rs:797-803`).
- **New:** `App::set_window_appearance(&self, appearance: Option<WindowAppearance>)` (`app.rs:1592`) forces `NSApplication.appearance`, which affects window chrome. `None` follows the system again (`macos/platform.rs:805-822`).
- Observing changes:
  - `Window::observe_window_appearance(&self, callback: impl FnMut(&mut Window, &mut App) + 'static) -> Subscription` (`window.rs:2253-2268`)
  - `Context<T>::observe_window_appearance(&self, window: &mut Window, callback: impl FnMut(&mut T, &mut Window, &mut Context<T>) + 'static) -> Subscription` (`app/context.rs:456-472`)
- gpui-component: `Theme::sync_system_appearance(window: Option<&mut Window>, cx: &mut App)` (`theme/mod.rs:353-363`) and `Theme::change(mode: impl Into<ThemeMode>, _window: Option<&mut Window>, cx: &mut App)` (`:389`). **`theme::init` forces `ThemeMode::Light`** (`theme/mod.rs:36-41`), and nothing in gpui-component or gpui-base subscribes to appearance changes: grep finds no `observe_window_appearance` there. The app must call `Theme::sync_system_appearance(Some(window), cx)` after opening the window and re-sync from an `observe_window_appearance` callback.

### 14. Fonts

- **System UI font name**: `".SystemUIFont"` is the virtual family, and `Font::default()` is `font(".SystemUIFont")` (`text_system.rs:1292-1314`). `pub fn font(family: impl Into<SharedString>) -> Font` (`text_system.rs:1317-1325`). On macOS `MacTextSystem::load_family` maps it with `gpui::font_name_with_fallbacks(name, ".AppleSystemUIFont")` (`macos/text_system.rs:282`). Other virtual names: `".ZedSans"` maps to `"IBM Plex Sans"` and `".ZedMono"` to `"Lilex"` (`text_system.rs:1420-1431`). **Neither font is bundled or installed on macOS**: `gpui-pre-0.3.8/resources` contains only `windows`.
- **Missing family**: `TextSystem::resolve_font(&self, font: &Font) -> FontId` (`text_system.rs:364-389`) falls back through `fallback_font_stack` = `.ZedMono, .ZedSans, Helvetica, Segoe UI, Ubuntu, Adwaita Sans, Cantarell, Noto Sans, DejaVu Sans, Arial` (`text_system.rs:255-267`). It panics only if all of them fail. On macOS, a missing monospace family **silently falls back to Helvetica (proportional)**. The private `fn font_id` (`text_system.rs:328`) caches the error.
- **"SF Mono"**: not determinable from source. On this machine (macOS 27), `NSFontManager.availableFontFamilies` does not list "SF Mono", and `NSFont(name: "SF Mono" / "SFMono-Regular")` returns nil, although `/System/Library/Fonts/SFNSMono.ttf` exists. `.AppleSystemUIFontMonospaced` (the `monospacedSystemFont`) does resolve through NSFont. **Recommendation: default to `"Menlo"`**, or embed a font via `add_fonts`. **Runtime check (below): `resolve_font(&font(".AppleSystemUIFontMonospaced"))` resolves to that family (not a fallback), so it is usable as an SF-Mono-equivalent family name; `"SF Mono"` falls back to Helvetica.**
- **Listing fonts**: `TextSystem::all_font_names(&self) -> Vec<String>` (sorted and deduplicated, `text_system.rs:281-289`), reached through `cx.text_system()` (`app.rs:321`: `pub fn text_system(&self) -> Arc<TextSystem>`) or `window.text_system()` (`window.rs:2402`: `-> &Arc<WindowTextSystem>`, which derefs to `TextSystem`). The macOS implementation enumerates `CTFontCollection` (`macos/text_system.rs:102-125`). gpui-component caches the list once, noting it costs about 100 ms (`theme/mono_font.rs:64-72`).
- **Embedding fonts**: `TextSystem::add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()>` (`text_system.rs:295-302`).
- **Font features**: `pub struct FontFeatures(pub Arc<Vec<(String, u32)>>)` with `FontFeatures::disable_ligatures()` (`calt=0`), `tag_value_list()`, and `is_calt_enabled()` (`text_system/font_features.rs:6-31`). Styled methods: `fn font_family(self, impl Into<SharedString>)` (`styled.rs:708`), `fn font_features(self, FontFeatures)` (`styled.rs:714`), `fn font(self, Font)` (`styled.rs:720`), `fn font_weight(self, FontWeight)` (`styled.rs:522`).
- **Monospace detection**: there is **no API** (no `is_monospace` in gpui-pre or gpui-pre-macos). A workaround is to compare advances: `TextSystem::advance(&self, font_id: FontId, font_size: Pixels, ch: char) -> Result<Size<Pixels>>` (`text_system.rs:433`) for `'i'` vs `'M'`, after `let id = ts.resolve_font(&font("Menlo"))`. Remember that `resolve_font` silently substitutes a fallback, so compare `ts.get_font_for_id(id).map(|f| f.family)` (`text_system.rs:354`) with the requested name. That is what gpui-component does in `theme/system_font.rs:40-44`. Other metrics: `em_width`, `em_advance`, `ch_width`, `ch_advance`, `ascent`, `descent`, `line_wrapper` (`text_system.rs:464-545`).
- **gpui-component theme defaults** (`theme/mod.rs:770-773`): `font_family: ".SystemUIFont"`, `font_size: px(16.)`, `mono_font_family: mono_font::default_mono_font_family()` = `TypographyTokens::default().mono` = **`"Menlo"` on macOS** (`gpui-base theme_tokens.rs:215-217`), `mono_font_size: px(13.)`. The fields are `pub font_family: SharedString`, `pub font_size: Pixels`, `pub mono_font_family: SharedString`, `pub mono_font_size: Pixels` (`theme/mod.rs:166-183`). At init, `.SystemUIFont` is swapped for the concrete installed family it resolves to (`theme/system_font.rs:32-49`). If Menlo is missing, the mono default falls back to `Monaco`, then `Courier New`, then `.SystemUIFont` (`theme/mono_font.rs:20-26, 39-45`).
- **The `font-kit` feature is mandatory for text on macOS.** Without it, `MacPlatform::new` uses `NoopTextSystem` and logs "no text will be rendered" (`macos/platform.rs:235-245`). gpui-kit enables `gpui_platform` features `font-kit`, `x11`, `wayland`, and `runtime_shaders` (`gpui-kit-0.7.1/Cargo.toml:401-409`).

### Surprises (vs upstream Zed gpui)

- `WindowOptions` has new fields: `app_owns_titlebar_drag`, `inactive_frame_interval`, `tabbing_identifier`. `WindowKind` has new `AnchoredPopup`. Struct literals copied from upstream that list every field without `..Default::default()` will not compile.
- `WindowControlArea::Drag` does nothing on macOS (`macos/window.rs:2050`). Dragging needs `start_window_move()`, and with a custom titlebar you should set `app_owns_titlebar_drag: true`.
- The platform lives in separate crates: `gpui-pre-platform` (`gpui_platform::application()` / `headless()` / `current_platform(bool)`) → `gpui-pre-macos` (`MacPlatform`) → `gpui-pre-apple` (Metal renderer). Text needs the `font-kit` feature. gpui-kit turns that on, along with `runtime_shaders`.
- `window.window_handle()` is the inherent `AnyWindowHandle` getter, not raw-window-handle. Use `HasWindowHandle::window_handle(window)`. The AppKit handle is an NSView. There is no windowNumber API.
- There is real pixel capture through `HeadlessAppContext` + `current_headless_renderer` (Metal offscreen, 2x scale) and `Window::render_to_image()` / `painted_quads()`, all behind `test-support`. For a live window, readback additionally needs `debug_assertions`.
- `App::set_window_appearance(Option<WindowAppearance>)` is new. gpui-component's `theme::init` hard-codes Light, so syncing with the system is the app's job.
- Bundled `.ZedMono`/`.ZedSans` map to Lilex/IBM Plex Sans, which are not installed. A missing family falls back silently to Helvetica on macOS. "SF Mono" is not resolvable by family name on this machine, so use "Menlo" (the gpui-component default).

### Runtime verification (scratch probe, macOS 27 / this machine)

Compiled and ran `/tmp/rogg-research/scratch/examples/winnum.rs` (deps: `gpui-kit = "=0.7.1"`, `raw-window-handle = "0.6"`, `objc2 = "0.6"`). Results:

- The windowNumber snippet above **works** (after fixing it to compile as below); it printed `windowNumber = Some(3876)` - usable as `screencapture -l3876 -o out.png`.
  ```rust
  use raw_window_handle::{HasWindowHandle, RawWindowHandle};
  fn window_number(window: &Window) -> Option<isize> {
      let h = HasWindowHandle::window_handle(window).ok()?;
      let RawWindowHandle::AppKit(a) = h.as_raw() else { return None };
      let ns_view = a.ns_view.as_ptr() as *mut objc2::runtime::AnyObject;
      unsafe {
          let ns_window: *mut objc2::runtime::AnyObject = objc2::msg_send![ns_view, window];
          if ns_window.is_null() { return None; }
          Some(objc2::msg_send![ns_window, windowNumber])
      }
  }
  // call inside handle.update(cx, |_, window, _| window_number(window))
  ```
- Size semantics (`WindowBounds::Windowed(Bounds::new(point(px(100.), px(100.)), size(px(640.), px(400.))))`, scale 2):
  - default titlebar: `window.viewport_size() = 640x400` (content), `window.bounds() = 640x432` (frame incl. 32px system titlebar). So the size in `WindowBounds` is the **content** size.
  - `appears_transparent: true`: `viewport = 640x400`, `bounds = 640x400` (content fills frame; draw your own titlebar).
- Fonts: `all_font_names()` contains `"Menlo"` but not `"SF Mono"` nor `".SystemUIFont"`. `resolve_font(&font("SF Mono"))` -> family `"Helvetica"` (silent fallback, confirmed). `resolve_font(&font(".AppleSystemUIFontMonospaced"))` -> `".AppleSystemUIFontMonospaced"` (**resolves** - this is the SF Mono system monospaced font; usable as a mono family name). `"Menlo"` -> `"Menlo"`.

## C. Elements & styling (gpui-pre 0.3.8) — questions 8, 9, 10

Paths are relative to `/tmp/rogg-research/`. `G=gpui-pre-0.3.8/src`, `M=gpui-pre-macros-0.3.8/src`.
Everything below is re-exported from `gpui_kit::*` (gpui-kit `src/lib.rs` does `pub use ::gpui::*;`).
Traits to import: `G/prelude.rs:5-9` exports `AppContext as _, BorrowAppContext, Context, Element,
InteractiveElement, IntoElement, ParentElement, Refineable, Render, RenderOnce, StatefulInteractiveElement,
Styled, StyledImage, TaskExt as _, VisualContext, util::FluentBuilder`.

### 8. Styling methods on `div()`

`Styled` trait: `G/styled.rs:22` `pub trait Styled: Sized { fn style(&mut self) -> &mut StyleRefinement; ... }`.
It pulls in macro-generated methods at `G/styled.rs:26-34`:
```rust
gpui_macros::style_helpers!();            // w/h/size/min_*/max_*/gap*, rounded*
gpui_macros::visibility_style_methods!();
gpui_macros::margin_style_methods!();
gpui_macros::padding_style_methods!();   // p, px, py, pt, pb, pl, pr
gpui_macros::position_style_methods!();  // relative, absolute, inset, top, bottom, left, right
gpui_macros::overflow_style_methods!();  // overflow_hidden, overflow_x_hidden, overflow_y_hidden
gpui_macros::cursor_style_methods!();    // cursor(..), cursor_pointer, cursor_text, ...
gpui_macros::border_style_methods!();    // border_color, border, border_1, border_t_1, ...
gpui_macros::box_shadow_style_methods!();// shadow(Vec<BoxShadow>), shadow_sm, ...
```
Macro generators live in `M/styles.rs`. Custom-value setters are generated by
`generate_custom_value_setter` (`M/styles.rs:665`), whose emitted signature is
(`M/styles.rs:689`):
```rust
fn #method_name(mut self, length: impl std::clone::Clone + Into<gpui::#length_type>) -> Self
```
where `length_type` is `Length` if `auto_allowed` else `DefiniteLength` (`M/styles.rs:540-546`),
and `AbsoluteLength` for `rounded*` (`M/styles.rs:598`) and `border*` (`M/styles.rs:345`).
Predefined setters (`border_1`, `rounded_md`, `gap_2`, `px_2`, `w_full`, …) come from
`generate_predefined_setter` (`M/styles.rs:623`), name = `{prefix}_{suffix}`.

| Method | Exists? | Where / signature |
|---|---|---|
| `.h(px(..))`, `.w(..)`, `.size(..)` | yes | prefixes `"w"` `M/styles.rs:846`, `"h"` `:852`, `"size"` `:858` (auto_allowed → `impl Clone + Into<Length>`); also `min_w/min_h/max_w/max_h/min_size/max_size` (`:861-895`). `w_full`, `h_full`, `size_full` from suffix `"full"` = `relative(1.)` |
| `.bg(rgb(0x..))` | yes | `G/styled.rs:490` `fn bg<F>(mut self, fill: F) -> Self where F: Into<Fill>`; `Fill: From<Hsla>` `G/style.rs:875`, `From<Rgba>` `:881`, `From<Background>` `:887` |
| `.border_1()` | yes | prefix `"border"` `M/styles.rs:1281` × suffix `"1"` = `px(1.)` `M/styles.rs:1337-1339`; also `.border(len)`, `border_t_1`, `border_b_1`, `border_l_1`, `border_r_1`, `border_x_*`, `border_y_*` (`:1290-1325`) |
| `.border_color(..)` | yes | `M/styles.rs:369`: `fn border_color<C>(mut self, border_color: C) -> Self where C: Into<gpui::Hsla>, Self: Sized` |
| `.border_dashed()` | yes | `G/styled.rs:500` |
| `.rounded(px(..))` | yes | prefix `"rounded"` `M/styles.rs:1164` (`impl Clone + Into<AbsoluteLength>`), plus `rounded_t/_b/_l/_r/_tl/_tr/_bl/_br`; suffixes `none/xs(2px)/sm(4px)/md(6px)/lg(8px)/xl(12px)/2xl/3xl/full` `M/styles.rs:1228-1276` |
| `.px(..)` / `.py` / `.p` / `.pl` … | yes | `"p"` `M/styles.rs:754`, `"px"` `:777`, `"py"` `:783` (padding is NOT auto-allowed → `Into<DefiniteLength>`) |
| `.gap(..)` / `.gap_x` / `.gap_y` | yes | `"gap"` `M/styles.rs:906` (+`gap_x`, `gap_y` `:912-923`) |
| `.text_size(..)` | yes | `G/styled.rs:538` `fn text_size(mut self, size: impl Into<AbsoluteLength>) -> Self`; presets `text_xs` (0.75rem) `:545`, `text_sm`, `text_base`, … |
| `.font_family(..)` | yes | `G/styled.rs:708` `fn font_family(mut self, family_name: impl Into<SharedString>) -> Self` |
| `.font_features(..)` | yes | `G/styled.rs:714` `fn font_features(mut self, features: FontFeatures) -> Self` |
| `.font(Font)` | yes | `G/styled.rs:720` |
| `.font_weight(FontWeight(550.))` | yes | `G/styled.rs:522` `fn font_weight(mut self, weight: FontWeight) -> Self`; `G/text_system.rs:1128` `pub struct FontWeight(pub f32);` so `FontWeight(550.)` is valid. Consts `THIN..BLACK` `G/text_system.rs:1159-1175` (`NORMAL`=400, `MEDIUM`=500, `SEMIBOLD`=600). |
| `.line_height(..)` | yes | `G/styled.rs:740` `fn line_height(mut self, line_height: impl Into<DefiniteLength>) -> Self` (px(..) or `relative(1.4)`) |
| `.text_color(..)` | yes | `G/styled.rs:514` `fn text_color(mut self, color: impl Into<Hsla>) -> Self` (cascades) |
| `.shadow(..)` | yes | `M/styles.rs:390` `fn shadow(mut self, shadows: std::vec::Vec<gpui::BoxShadow>) -> Self`; presets `shadow_none/2xs/xs/sm/md/lg/xl/2xl`. `BoxShadow::new(offset_x: Pixels, offset_y: Pixels, color: Hsla)` `G/style.rs:366`, builders `.blur_radius(Pixels)`, `.spread_radius(Pixels)`, `.inset()` `G/style.rs:376-392`; struct fields `color, offset, blur_radius, spread_radius, inset` `G/style.rs:349-360` |
| `.overflow_hidden()` | yes | `M/styles.rs:135` |
| `.flex_1()` | yes | `G/styled.rs:181` (grow 1, shrink 1, basis 0%) |
| `.flex_shrink_0()` | yes | `G/styled.rs:252` |
| `.absolute()` / `.relative()` | yes | `M/styles.rs:118` / `:111` |
| `.top(..)`, `.left(..)`, `.right`, `.bottom`, `.inset` | yes | prefixes `M/styles.rs:817` (top), `:829` (left); auto_allowed → `Into<Length>` |
| `.opacity(f32)` | yes | `G/styled.rs:746` `fn opacity(mut self, opacity: f32) -> Self` |
| `.hover(\|s\| s.bg(..))` | yes | `InteractiveElement::hover` `G/elements/div.rs:842` `fn hover(mut self, f: impl FnOnce(StyleRefinement) -> StyleRefinement) -> Self` (debug_assert if set twice). Also `group(name)` `:773` + `group_hover(name, f)` `:852`. `StatefulInteractiveElement::active(f)` `:1561` (needs `.id`). |
| `.cursor_pointer()` | yes | `M/styles.rs:178` (`CursorStyle::PointingHand`); generic `.cursor(CursorStyle)` `M/styles.rs:164`; `cursor_text`, `cursor_move`, `cursor_not_allowed`, … |
| `.id(..)` → stateful | yes | `G/elements/div.rs:779` `fn id(mut self, id: impl Into<ElementId>) -> Stateful<Self>` |
| `.on_click(..)` | yes (stateful only) | `StatefulInteractiveElement::on_click` `G/elements/div.rs:1590`: `fn on_click(mut self, listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self`. Non-primary clicks: `on_aux_click` `:1602`. `ClickEvent::is_right_click()` / `is_middle_click()` `G/interactive.rs:353,367`. |
| `.on_mouse_down(MouseButton::Middle, ..)` | yes (non-stateful OK) | `G/elements/div.rs:869`: `fn on_mouse_down(mut self, button: MouseButton, listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static) -> Self` (fires in bubble phase only if `hitbox.is_hovered`, `:126-140`). `MouseButton { Left, Right, Middle, Navigate(NavigationDirection) }` `G/interactive.rs:453-466`. Also `on_mouse_up` `:930`, `on_mouse_move` `:1006`, `on_scroll_wheel` `:1060`, `on_mouse_down_out` `:980`, `on_any_mouse_down` `:918`. |
| `.tooltip(..)` | yes (stateful only) | `G/elements/div.rs:1682`: `fn tooltip(mut self, build_tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self`; `hoverable_tooltip` `:1693`. gpui-component helper: `gpui_component::tooltip::Tooltip::new(text)` (`gpui-component-0.7.1/src/tooltip.rs:43`) `.build(window, cx) -> AnyView` (`:81`), e.g. `.tooltip(\|w, cx\| Tooltip::new("Find").build(w, cx))`. |
| `.track_focus(&h)`, `.key_context(..)`, `.on_action(..)` | yes | `G/elements/div.rs:788`, `:830`, `:1106` |
| `.occlude()` / `.block_mouse_except_scroll()` / `.window_control_area(..)` | yes | `G/elements/div.rs:1250`, `:1266`, `:1257` |
| `.debug_selector(\|\| ..)` | yes but feature-gated | `G/elements/div.rs:883-901`: real only under `#[cfg(any(test, all(debug_assertions, feature = "test-support")))]`; otherwise a no-op. (Relevant for Q15.) |

Colors (`G/color.rs`):
```rust
pub fn rgb(hex: u32) -> Rgba            // :14  (0xRRGGBB, a = 1.0)
pub fn rgba(hex: u32) -> Rgba           // :20  (0xRRGGBBAA)
pub const fn hsla(h: f32, s: f32, l: f32, a: f32) -> Hsla  // :424 (all 0..1, clamped)
pub struct Hsla { pub h: f32, pub s: f32, pub l: f32, pub a: f32 }  // :334
pub struct Rgba { r, g, b, a: f32 }    // :39
impl From<Rgba> for Hsla               // :677   → `let c: Hsla = rgb(0x1e1e1e).into();`
impl From<Hsla> for Rgba               // :194
impl TryFrom<&'_ str> for Rgba         // :224   ("#rrggbb[aa]")
Hsla::to_rgb(self) -> Rgba             // :525
Hsla::opacity(&self, factor: f32) -> Self   // :637  (a *= factor)
Hsla::alpha(&self, a: f32) -> Self          // :667  (a = a)
Hsla::blend(self, other: Hsla) -> Hsla      // :580
Rgba::opacity / Rgba::alpha                  // :126 / :95
black(), white(), transparent_black(), transparent_white(), opaque_grey(l, a)  // :434-474
```
Anything taking `impl Into<Hsla>` accepts `rgb(..)`/`rgba(..)` directly.

### 9. `svg()` and assets

Element API (`G/elements/svg.rs`):
```rust
pub fn svg() -> Svg                                        // :27
impl Svg {
    pub fn path(mut self, path: impl Into<SharedString>) -> Self           // :40  (loaded via AssetSource)
    pub fn external_path(mut self, path: impl Into<SharedString>) -> Self  // :46  (filesystem, via use_asset)
    pub fn data(mut self, data: &[u8]) -> Self                              // :53  (raw bytes, cached by hash)
    pub fn with_transformation(mut self, transformation: Transformation) -> Self // :66 (render only, not layout/hitbox)
}
impl Styled for Svg            // :200  → .size(px(15.)), .text_color(..), .flex_shrink_0() ...
impl InteractiveElement for Svg // :206
```
So `svg().path("icons/x.svg").size(px(15.)).text_color(c)` is valid.

**Color comes only from `text_color`**: in `Svg::paint` every branch is `self.path.as_ref().zip(style.text.color)` (`G/elements/svg.rs:150-185`), i.e. if the svg element has no `text_color` in its own style **nothing is painted**. Then it calls
```rust
// G/window.rs:4849
pub fn paint_svg(&mut self, bounds: Bounds<Pixels>, path: SharedString, mut data: Option<&[u8]>,
                 transformation: TransformationMatrix, color: Hsla, cx: &App) -> Result<()>
```
which rasterizes via `cx.svg_renderer.render_alpha_mask(..)` and inserts a **`MonochromeSprite`** with `color: color.opacity(element_opacity)` (`G/window.rs:4870-4906`).
`render_alpha_mask` (`G/svg_renderer.rs:231-262`) renders the SVG with **resvg/usvg** into a tiny_skia `Pixmap` and keeps **only `p.alpha()`** of each pixel (`:244-250`). Consequences:
- SVG is a monochrome alpha mask tinted by `text_color`. Original fill/stroke colors are ignored (multicolor icons become single-color silhouettes). Use `img()` for color images.
- Stroke-based icons work: resvg rasterizes strokes normally (`stroke="currentColor"` resolves to some opaque color; only alpha matters). Proof: gpui-kit-assets ships Lucide icons that are pure stroke icons, e.g. `gpui-kit-assets-0.7.1/assets/icons/hand-helping.svg` (`fill="none" stroke="currentColor" stroke-width="2"`).
- Stroke width scales with the viewBox: `rasterize_tree` uses `Transform::from_scale(width / svg_size.width(), ...)` (`G/svg_renderer.rs:299-302`), so a 24-unit viewBox with `stroke-width="2"` drawn at 15px gives ~1.25px strokes. No runtime stroke-width override exists; to change it, ship a different SVG (or pass modified bytes via `.data(..)`).
- Raster size: `paint_svg` snaps bounds to device pixels then rasterizes at `SMOOTH_SVG_SCALE_FACTOR = 2.` × device size (`G/window.rs:4861-4867`, `G/svg_renderer.rs:81`) and draws it downscaled into the element bounds, centered (`G/window.rs:4883-4896`). Aspect ratio follows the SVG width (`SvgSize::Size` branch, `G/svg_renderer.rs:277-280`). Cached in the sprite atlas keyed by `(path, size)`.
- Text in SVGs is resolved against system fonts + bundled `fonts/ibm-plex-sans/...` and `fonts/lilex/...` loaded from the AssetSource (`G/svg_renderer.rs:309-322`), with a warning log if absent (harmless).

AssetSource (`G/assets.rs:13-19`):
```rust
pub trait AssetSource: 'static + Send + Sync {
    /// Load the given asset from the source path.
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>>;
    /// List the assets at the given path.
    fn list(&self, path: &str) -> Result<Vec<SharedString>>;
}
impl AssetSource for ()   // :21
```
Registration (`G/app.rs:202`):
```rust
pub fn with_assets(self, asset_source: impl AssetSource) -> Self
```
(on `Application`; it also rebuilds `SvgRenderer::new(asset_source)` `:206`). Usage: `gpui_kit::application().with_assets(MyAssets).run(|cx| ...)`. Only one AssetSource per app → to combine our SVGs with `gpui_kit::assets::Assets`, write a wrapper whose `load` tries our files first and falls back to the kit's (Q18 covers the kit side).

### 10. Custom `Element`, text shaping, painting, hit testing

#### Element trait (verbatim, `G/element.rs:53-144`)
```rust
pub trait Element: 'static + IntoElement {
    type RequestLayoutState: 'static;
    type PrepaintState: 'static;

    fn id(&self) -> Option<ElementId>;

    fn source_location(&self) -> Option<&'static panic::Location<'static>>;

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState);

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState;

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    );

    // provided (accessibility, new vs. older upstream):
    fn a11y_role(&self) -> Option<accesskit::Role> { None }                       // :114
    fn write_a11y_info(&self, _node: &mut accesskit::Node) {}                       // :122
    fn a11y_synthetic_children(&mut self, _prepaint: &mut Self::PrepaintState,
                               _builder: &mut A11ySubtreeBuilder) {}                // :133
    fn into_any(self) -> AnyElement { AnyElement::new(self) }                      // :141
}
```
IntoElement (`G/element.rs:147-159`):
```rust
pub trait IntoElement: Sized {
    type Element: Element;
    fn into_element(self) -> Self::Element;
    fn into_any_element(self) -> AnyElement { self.into_element().into_any() }
}
```
Pattern for a custom element (copied from `gpui-pre-0.3.8/examples/input.rs:414-420`):
```rust
impl IntoElement for TextElement {
    type Element = Self;
    fn into_element(self) -> Self::Element { self }
}
```
`Render` `G/element.rs:165-168`: `fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;`
`RenderOnce` `G/element.rs:181-186`: `fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement;` (+ `#[derive(IntoElement)]`, `M/gpui_macros.rs:44`).

Layout helpers (`G/window.rs`):
```rust
pub fn request_layout(&mut self, style: Style, children: impl IntoIterator<Item = LayoutId>, cx: &mut App) -> LayoutId  // :5071 (#[must_use])
pub fn request_measured_layout<F>(&mut self, style: Style, measure: F) -> LayoutId
    where F: Fn(Size<Option<Pixels>>, Size<AvailableSpace>, &mut Window, &mut App) -> Size<Pixels> + 'static   // :5100
pub fn text_style(&self) -> TextStyle      // :2407  (TextStyle::font() G/style.rs:543, line_height_in_pixels(rem) :554, to_run(len) :559)
pub fn line_height(&self) -> Pixels        // :3049
pub fn rem_size(&self) -> Pixels           // :2986
pub fn scale_factor(&self) -> f32          // :2973
pub fn mouse_position(&self) -> Point<Pixels>  // :3244
pub fn text_system(&self) -> &Arc<WindowTextSystem>  // :2402
```
Low-ceremony alternative: `canvas(prepaint, paint)` (`G/elements/canvas.rs:10-13`):
```rust
pub fn canvas<T>(
    prepaint: impl 'static + FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> T,
    paint: impl 'static + FnOnce(Bounds<Pixels>, T, &mut Window, &mut App),
) -> Canvas<T>
```
(`Canvas` is `Styled`, so `.size_full()` etc.)

#### Text shaping (`G/text_system.rs`)
`WindowTextSystem` derefs to `TextSystem` (`:600-606`).
```rust
// :638
pub fn shape_line(&self, text: SharedString, font_size: Pixels, runs: &[TextRun],
                  force_width: Option<Pixels>) -> ShapedLine          // panics (debug_assert) on '\n'
// :689  cache by caller hash, avoids building a SharedString on hit
pub fn shape_line_by_hash(&self, text_hash: u64, text_len: usize, font_size: Pixels, runs: &[TextRun],
                          force_width: Option<Pixels>, materialize_text: impl FnOnce() -> SharedString) -> ShapedLine
// :750
pub fn shape_text(&self, text: SharedString, font_size: Pixels, runs: &[TextRun],
                  wrap_width: Option<Pixels>, line_clamp: Option<usize>) -> Result<SmallVec<[WrappedLine; 1]>>
// :886
pub fn layout_line(&self, text: &str, font_size: Pixels, runs: &[TextRun], force_width: Option<Pixels>) -> Arc<LineLayout>
// TextSystem:
pub fn all_font_names(&self) -> Vec<String>                         // :284
pub fn resolve_font(&self, font: &Font) -> FontId                   // :370
pub fn advance(&self, font_id: FontId, font_size: Pixels, ch: char) -> Result<Size<Pixels>>  // :433
pub fn em_width(&self, font_id: FontId, font_size: Pixels) -> Result<Pixels>   // :464
pub fn ch_advance(&self, font_id: FontId, font_size: Pixels) -> Result<Pixels> // :485
pub fn line_wrapper(self: &Arc<Self>, font: Font, font_size: Pixels) -> LineWrapperHandle // :545
```
`TextRun` (`G/text_system.rs:1228-1241`):
```rust
pub struct TextRun {
    pub len: usize,                    // utf8 bytes
    pub font: Font,
    pub color: Hsla,
    pub background_color: Option<Hsla>,
    pub underline: Option<UnderlineStyle>,
    pub strikethrough: Option<StrikethroughStyle>,
}
```
`Font` (`G/text_system.rs:1292-1309`): `{ family: SharedString, features: FontFeatures, fallbacks: Option<FontFallbacks>, weight: FontWeight, style: FontStyle }`; doc at `:1295`: *"The special name ".SystemUIFont" is used to identify the system UI font"*. Helper `pub fn font(family: impl Into<SharedString>) -> Font` `:1318`.
macOS font matching: `font_kit::matching::find_best_match` on the family's faces with `weight: FontkitWeight(value.0)` (`gpui-pre-macos-0.3.8/src/text_system.rs:161-168, 714-716`) → `FontWeight(550.)` selects the nearest existing face by CSS matching rules; it does not synthesize intermediate weights.

`ShapedLine` (`G/text_system/line.rs:43-50`, derefs to `Arc<LineLayout>`, pub field `text: SharedString`):
```rust
pub fn paint(&self, origin: Point<Pixels>, line_height: Pixels, align: TextAlign,
             align_width: Option<Pixels>, window: &mut Window, cx: &mut App) -> Result<()>      // line.rs:108
pub fn paint_background(&self, origin: Point<Pixels>, line_height: Pixels, align: TextAlign,
             align_width: Option<Pixels>, window: &mut Window, cx: &mut App) -> Result<()>      // line.rs:160
pub fn width(&self) -> Pixels   // :88
pub fn len(&self) -> usize      // :80
pub fn split_at(&self, byte_index: usize) -> (ShapedLine, ShapedLine)  // :193
```
`LineLayout` (`G/text_system/line_layout.rs:19-32`) pub fields `font_size, width, ascent, descent, runs: Vec<ShapedRun>, len`; methods `index_for_x(x) -> Option<usize>` `:61`, `closest_index_for_x(x) -> usize` `:78`, `x_for_index(index) -> Pixels` `:108`.
`WrappedLine` (`line.rs:443-450`):
```rust
pub fn paint(&self, origin: Point<Pixels>, line_height: Pixels, align: TextAlign,
             bounds: Option<Bounds<Pixels>>, window: &mut Window, cx: &mut App) -> Result<()>  // line.rs:460
```
plus `WrappedLineLayout` helpers `size(line_height)`, `index_for_position`, `position_for_index` (`line_layout.rs:313-428`).
`LineWrapper` (`G/text_system/line_wrapper.rs:29`):
```rust
pub fn wrap_line<'a>(&'a mut self, fragments: &'a [LineFragment], wrap_width: Pixels,
                     indent_adjustment: IndentAdjustment) -> impl Iterator<Item = Boundary> + 'a   // :52
pub fn truncate_line<'a>(&mut self, line: SharedString, truncate_width: Pixels, truncation_affix: &str,
                         runs: &'a [TextRun], truncate_from: TruncateFrom) -> (SharedString, Cow<'a, [TextRun]>) // :299
pub enum IndentAdjustment { NoIndent, #[default] SameIndent, ExtraColumns(u32) }   // :18-25
pub enum TruncateFrom { Start, End, Middle }                                       // :7-14
LineFragment::text(&str) / LineFragment::element(width, len_utf8)                  // :682 / :687
```

#### Painting (`G/window.rs`)
```rust
pub fn paint_quad(&mut self, quad: PaintQuad)                                           // :4550
pub fn paint_path(&mut self, mut path: Path<Pixels>, color: impl Into<Background>)       // :4621
pub fn paint_svg(&mut self, bounds, path: SharedString, data: Option<&[u8]>, transformation, color: Hsla, cx: &App) -> Result<()> // :4849
pub fn paint_image(..)                                                                  // :4919
pub fn paint_layer<R>(&mut self, bounds: Bounds<Pixels>, f: impl FnOnce(&mut Self) -> R) -> R   // :4399
pub fn with_content_mask<R>(&mut self, mask: Option<ContentMask<Pixels>>, f: impl FnOnce(&mut Self) -> R) -> R // :4029
pub struct ContentMask<P> { pub bounds: Bounds<P> }                                     // :2171
pub fn with_element_offset<R>(..) / with_absolute_element_offset<R>(..)                 // :4048 / :4066
pub fn with_text_style<F, R>(&mut self, style: Option<TextStyleRefinement>, f: F) -> R  // :3976
pub fn request_animation_frame(&self)                                                   // :2670
pub fn refresh(&mut self)                                                               // :2316
```
`PaintQuad` (`G/window.rs:7625-7638`): pub fields `bounds, corner_radii: Corners<Pixels>, background: Background, border_widths: Edges<Pixels>, border_color: Hsla, border_style: BorderStyle`; builders `.corner_radii(..)` `:7642`, `.border_widths(..)` `:7650`, `.border_color(..)` `:7658`, `.background(..)` `:7666`.
```rust
pub fn quad(bounds: Bounds<Pixels>, corner_radii: impl Into<Corners<Pixels>>, background: impl Into<Background>,
            border_widths: impl Into<Edges<Pixels>>, border_color: impl Into<Hsla>, border_style: BorderStyle) -> PaintQuad  // :7675
pub fn fill(bounds: impl Into<Bounds<Pixels>>, background: impl Into<Background>) -> PaintQuad                            // :7694
pub fn outline(bounds: impl Into<Bounds<Pixels>>, border_color: impl Into<Hsla>, border_style: BorderStyle) -> PaintQuad  // :7706 (1px border, transparent bg)
```
e.g. `window.paint_quad(fill(row_bounds, rgb(0x2a2d2e)))`; border: `window.paint_quad(outline(b, c, BorderStyle::Solid))` or `fill(b, bg).border_widths(px(1.)).border_color(c)`.

#### Hit testing & events (`G/window.rs`)
```rust
pub fn insert_hitbox(&mut self, bounds: Bounds<Pixels>, behavior: HitboxBehavior) -> Hitbox   // :5157 (prepaint only)
pub struct Hitbox { pub id: HitboxId, #[deref] pub bounds: Bounds<Pixels>, pub content_mask: ContentMask<Pixels>, pub behavior: HitboxBehavior }  // :831-841
impl Hitbox {
    pub fn is_hovered(&self, window: &Window) -> bool                                  // :860 (false during keyboard modality!)
    pub fn is_hovered_at(&self, position: Point<Pixels>, window: &Window) -> bool      // :866
    pub fn should_handle_scroll(&self, window: &Window) -> bool                        // :881 (use for ScrollWheelEvent)
}
pub enum HitboxBehavior { #[default] Normal, BlockMouse, BlockMouseExceptScroll }     // :888-938
pub fn on_mouse_event<Event: MouseEvent>(&mut self,
    mut listener: impl FnMut(&Event, DispatchPhase, &mut Window, &mut App) + 'static)  // :5294 (paint only; cleared each frame)
pub fn on_key_event<Event: KeyEvent>(&mut self,
    listener: impl Fn(&Event, DispatchPhase, &mut Window, &mut App) + 'static)         // :5317
pub fn set_cursor_style(&mut self, style: CursorStyle, hitbox: &Hitbox)               // :3993 (paint only)
pub fn set_window_cursor_style(&mut self, style: CursorStyle)                         // :4005
```
Phase rules (enforced by `debug_assert_*`): `request_layout`/`insert_hitbox` in prepaint; `paint_*`, `on_mouse_event`, `set_cursor_style` in paint.

#### Examples to copy from
- `gpui-pre-0.3.8/examples/input.rs:405-582` — full custom `Element` (`TextElement`): `request_layout` with `Style` + `window.line_height()`, `prepaint` builds `TextRun`s from `window.text_style()`, calls `window.text_system().shape_line(display_text, font_size, &runs, None)` (`:504-506`), computes cursor/selection via `line.x_for_index(..)` and `fill(..)`, `paint` calls `window.paint_quad(..)` and `line.paint(bounds.origin, window.line_height(), gpui::TextAlign::Left, None, window, cx)` (`:565-572`).
- `gpui-pre-0.3.8/examples/window_shadow.rs:38-60` — `canvas` + `window.insert_hitbox(.., HitboxBehavior::Normal)` + `window.set_cursor_style(.., &hitbox)`.
- `gpui-pre-0.3.8/examples/data_table.rs:329-352` — `window.on_mouse_event(..)` inside a canvas (drag scrollbar).
- `gpui-pre-0.3.8/examples/painting.rs:364` — `window.paint_quad(quad(..))`.
- `gpui-base-0.7.1/src/input/base/element.rs` (many `shape_line` calls, e.g. `:1241`, `:1265`) — production text editor element; `gpui-base-0.7.1/src/plot/label.rs:39` — simple shaped label; `gpui-base-0.7.1/src/virtual_list.rs` — custom `impl Element` virtual list.

### Surprises vs. upstream Zed gpui (Q8–Q10 scope)
1. **No text at all without `font-kit`** on macOS: `gpui-pre-macos-0.3.8/src/gpui_macos.rs:29-30` gates `mod text_system` on `feature = "font-kit"`, and `platform.rs:238-242` logs "compiled without the `font-kit` feature, so no text will be rendered". gpui-kit turns it on for you (gpui-kit Cargo.toml `gpui_platform` features `font-kit`, `runtime_shaders`); if you ever depend on `gpui-pre-platform` directly, add `features = ["font-kit"]`.
2. `Element` has extra provided accessibility methods (`a11y_role`, `write_a11y_info`, `a11y_synthetic_children`, `G/element.rs:114-138`) and `StatefulInteractiveElement::role(..)` (`G/elements/div.rs:1310`); required methods are the same as recent upstream (`id`, `source_location`, `request_layout`/`prepaint`/`paint` with `inspector_id`).
3. `LineWrapper::wrap_line` takes a third arg `indent_adjustment: IndentAdjustment` (`line_wrapper.rs:52-57`); `truncate_line` takes `truncate_from: TruncateFrom` (incl. `Middle`) and returns `(SharedString, Cow<[TextRun]>)` (`:299-306`).
4. New `shape_line_by_hash` (`text_system.rs:689`) for allocation-free cache hits — useful for log rows.
5. `BoxShadow` has an `inset: bool` field and builder API `BoxShadow::new(x, y, color).blur_radius(..).spread_radius(..).inset()` (`style.rs:349-392`) — struct-literal code from older gpui needs `inset: false` / `..`.
6. `Hitbox::is_hovered` returns `false` while the last input was keyboard (`window.rs:845-860`); use `is_hovered_at` / `should_handle_scroll` when that matters.
7. `debug_selector` is a no-op unless `cfg(test)` or (`debug_assertions` AND `test-support`) (`div.rs:883-901`) — release/dev builds without `test-support` record nothing.
8. Svg only paints when `text_color` is set on the svg element itself (`svg.rs:150-185`); output is alpha-only (monochrome).

## C (part 2): Lists, focus/actions/menus, entities/async, tests

All paths are relative to `/tmp/rogg-research/`. `gp/` = `gpui-pre-0.3.8/src/`.
In app code, import everything through `gpui_kit::*` (gpui-kit does `pub use ::gpui::*;`, gpui-kit-0.7.1/src/lib.rs:95).

### 11. `uniform_list`, `list`/`ListState`, `ScrollHandle`, VirtualList

**`uniform_list`**, gp/elements/uniform_list.rs:20-29
```rust
#[track_caller]
pub fn uniform_list<R>(
    id: impl Into<ElementId>,
    item_count: usize,
    f: impl 'static + Fn(Range<usize>, &mut Window, &mut App) -> Vec<R>,
) -> UniformList
where
    R: IntoElement,
```
- The closure takes `(&mut Window, &mut App)`, not the view. To get view state, wrap it in `cx.processor(...)` (gp/app/context.rs:260-269):
  ```rust
  pub fn processor<E, R>(
      &self,
      f: impl Fn(&mut T, E, &mut Window, &mut Context<T>) -> R + 'static,
  ) -> impl Fn(E, &mut Window, &mut App) -> R + 'static
  ```
  Usage example: gpui-pre-0.3.8/examples/uniform_list.rs:17-39 (`uniform_list("entries", 50, cx.processor(|_this, range, _window, _cx| { ... }))` then `.h_full()`).
- It sets `overflow.y = Scroll` by default (uniform_list.rs:31-32). Rows are measured from item 0, or from the item set by `.with_width_from_item(Some(ix))` (uniform_list.rs:622).
- Builder methods on `UniformList`: `with_width_from_item(Option<usize>)` :622, `with_sizing_behavior(ListSizingBehavior)` :628, `with_horizontal_sizing_behavior(ListHorizontalSizingBehavior)` :636 (`Unconstrained` turns on horizontal scrolling, :646-648), `with_decoration(impl UniformListDecoration)` :653, `track_scroll(&UniformListScrollHandle)` :683, `y_flipped(bool)` :690. It also implements `Styled` (:256).

**`UniformListScrollHandle`**: `#[derive(Clone, Debug, Default)] pub struct UniformListScrollHandle(pub Rc<RefCell<UniformListScrollState>>);` (uniform_list.rs:79-80). `UniformListScrollState.base_handle: ScrollHandle` is public (:112).
- `pub fn new() -> Self` :136
- `pub fn scroll_to_item(&self, ix: usize, strategy: ScrollStrategy)` :150. This is non-strict: it does nothing if the item is already fully visible (doc :146-149).
- `pub fn scroll_to_item_strict(&self, ix: usize, strategy: ScrollStrategy)` :163. Always repositions. **For "center the current match" use `scroll_to_item_strict(ix, ScrollStrategy::Center)`.**
- `scroll_to_item_with_offset(&self, ix, strategy, offset: usize)` :182 and `scroll_to_item_strict_with_offset(...)` :201
- `y_flipped()` :216, `is_scrollable()` :231, `is_scrolled_to_end() -> Option<bool>` :241, `scroll_to_bottom()` :252
- `logical_scroll_top_index()` :222 exists only with `#[cfg(any(test, feature = "test-support"))]` (:221). For the top row in production builds, use `handle.0.borrow().base_handle.logical_scroll_top().0` (`ScrollHandle::logical_scroll_top`, div.rs:4406).
- All scroll calls are **deferred**: they store `deferred_scroll_to_item` and take effect at the next layout (:151-156). The view must still `cx.notify()`.

**`ScrollStrategy`** (uniform_list.rs:83-99): `Top`, `Center`, `Bottom`, `Nearest`.

**`list` / `ListState`** (variable row height), gp/elements/list.rs
```rust
pub fn list(
    state: ListState,
    render_item: impl FnMut(usize, &mut Window, &mut App) -> AnyElement + 'static,
) -> List                                                     // list.rs:23-26
#[derive(Clone)] pub struct ListState(Rc<RefCell<StateInner>>); // :53-54
pub fn new(item_count: usize, alignment: ListAlignment, overdraw: Pixels) -> Self // :314
pub enum ListAlignment { Top, Bottom }                        // :164-169
```
- `List::with_sizing_behavior(ListSizingBehavior)` :46. `ListSizingBehavior { Infer, #[default] Auto }` :188-194.
- `ListState` methods (line numbers in list.rs): `measure_all(self)` 336, `with_uniform_item_height(self, Pixels)` 347, `reset(&self, element_count)` 355, `reset_with_uniform_height` 372, `remeasure()` 400, `remeasure_items(Range)` 412, `item_count()` 478, `is_scrolled_to_end()` 484, `splice(&self, old_range: Range<usize>, count: usize)` 503, `splice_focusable` 511, `set_scroll_handler` 552, `logical_scroll_top() -> ListOffset` 560, `scroll_by(Pixels)` 565, `scroll_to_end()` 603, `set_follow_mode(FollowMode)` 617, `scroll_to(ListOffset)` 660, `scroll_to_reveal_item(&self, ix: usize)` 677, `bounds_for_item(ix) -> Option<Bounds<Pixels>>` 711, `viewport_bounds()` 799.
- `list` has no "center item" strategy. That makes `uniform_list` the better fit for a log viewer with fixed-height rows.

**`ScrollHandle`** (for `div().id(..).overflow_y_scroll().track_scroll(&h)`), gp/elements/div.rs:4248-4249 `#[derive(Clone, Debug)] pub struct ScrollHandle(Rc<RefCell<ScrollHandleState>>);` (has `Default`, :4251)
- Methods: `new()` 4259, `offset() -> Point<Pixels>` 4264, `max_offset()` 4269, `top_item()` 4274, `bottom_item()` 4293, `bounds()` 4312, `bounds_for_item(ix)` 4317, `scroll_to_item(ix)` 4322, `scroll_to_top_of_item(ix)` 4332, `scroll_to_bottom()` 4392, `set_offset(Point<Pixels>)` 4400, `logical_scroll_top() -> (usize, Pixels)` 4406, `logical_scroll_bottom()` 4421, `children_count()` 4436.
- `overflow_scroll` / `overflow_y_scroll` / `track_scroll(&ScrollHandle)` / `on_click` belong to `StatefulInteractiveElement` (trait starts at div.rs:1306; methods at :1522, :1535, :1549, :1590). They require `.id(...)` first.
- A fully custom `Element` gets no scroll handling for free. It must register `window.on_mouse_event::<ScrollWheelEvent>` itself (covered in Q10) and keep its own offset. Alternatively, put it inside a `uniform_list` or a scrolling div that has a `ScrollHandle`.

**gpui-component `VirtualList`**: gpui-component-0.7.1/src/virtual_list.rs:2 re-exports `gpui_base::{VirtualList, VirtualListScrollHandle, h_virtual_list, v_virtual_list}`. It is for rows of different but **known** sizes (gpui-base-0.7.1/src/virtual_list.rs:1-12):
```rust
pub fn v_virtual_list<R, V>(
    view: Entity<V>,
    id: impl Into<ElementId>,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    f: impl 'static + Fn(&mut V, Range<usize>, &mut Window, &mut Context<V>) -> Vec<R>,
) -> VirtualList where R: IntoElement, V: Render,              // gpui-base/src/virtual_list.rs:139-147
```
`VirtualListScrollHandle::new()` :89, `scroll_to_item(ix, ScrollStrategy)` :107, `scroll_to_bottom()` :123. `VirtualList::track_scroll(&VirtualListScrollHandle)` :236. Unlike `uniform_list`, its closure receives `&mut V` directly.

### 12. Focus, actions, key bindings, menus

**`actions!`**: the gpui version is at gp/action.rs:25-41, and gpui-kit ships an identical copy (gpui-kit-0.7.1/src/lib.rs:47-76) that expands `$crate::Action` against `gpui_kit`:
```rust
actions!(rogg, [OpenFile, Find, FindNext, FindPrev, CloseTab]);   // namespaced
actions!([Foo, Bar]);                                              // no namespace
```
Each name becomes `#[derive(Clone, PartialEq, Default, Debug, Action)] #[action(namespace = $ns)] pub struct $name;`. Registering two actions with the same name **panics at App creation** (action.rs:54). For data-carrying actions use `#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)] #[action(namespace = x)]` (action.rs:58-66; the derive is re-exported at action.rs:4). gpui-pre's macros rewrite `gpui::` paths to `<gpui-kit crate name>::` when the calling crate depends on `gpui-kit` (gpui-pre-macros-0.3.8/src/gpui_pre_facade_paths.rs:11-18, 30-35). An app that depends only on `gpui-kit` can therefore use `#[derive(Action)]`, `#[derive(IntoElement)]` and `#[gpui_kit::test]` without a `gpui` dependency.

**`KeyBinding::new`**, gp/keymap/binding.rs:32-33 (panics on a parse error):
```rust
pub fn new<A: Action>(keystrokes: &str, action: A, context: Option<&str>) -> Self
```
**`cx.bind_keys`**, gp/app.rs:2486: `pub fn bind_keys(&mut self, bindings: impl IntoIterator<Item = KeyBinding>)`. `clear_key_bindings()` :2492.

**Global (app-level) action handler**, gp/app.rs:2505-2508. It runs at the end of the bubble phase, so it fires only if no element handled the action (doc :2502-2504):
```rust
pub fn on_action<A: Action>(&mut self, listener: impl Fn(&A, &mut Self) + 'static) -> &mut Self
```
**Quit**: `pub fn quit(&self)` gp/app.rs:1160. `pub fn on_window_closed(&self, on_closed: impl FnMut(&mut App, WindowId) + 'static) -> Subscription` gp/app.rs:2630-2633. examples/on_window_close_quit.rs:51-53 calls `cx.quit()` from it.

**Element-side** (`InteractiveElement`, gp/elements/div.rs):
- `fn key_context<C, E>(self, key_context: C) -> Self where C: TryInto<KeyContext, Error = E>, E: Display` :830. Use a `&str` such as `"LogView"`.
- `fn track_focus(self, focus_handle: &FocusHandle) -> Self` :788 (also marks the element focusable). `fn focusable(self)` :1516. `tab_index(isize)` :809, `tab_stop(bool)` :800.
- `fn on_action<A: Action>(self, listener: impl Fn(&A, &mut Window, &mut App) + 'static) -> Self` :1106. `capture_action` :1092. `on_key_down(impl Fn(&KeyDownEvent, &mut Window, &mut App))` :1133.
- `fn focus(self, f: impl FnOnce(StyleRefinement) -> StyleRefinement)` :1273 sets focused styles. `in_focus(...)` :1283.
- `on_mouse_down(button: MouseButton, listener: impl Fn(&MouseDownEvent, &mut Window, &mut App))` :869. `on_scroll_wheel` :1060.

**`cx.listener`**, gp/app/context.rs:246-249 (holds a weak view):
```rust
pub fn listener<E: ?Sized>(
    &self,
    listener: impl Fn(&mut T, &E, &mut Window, &mut Context<T>) + 'static,
) -> impl Fn(&E, &mut Window, &mut App) + 'static
```
Typical pattern: `.on_action(cx.listener(|this, _: &FindNext, window, cx| { ... }))`.

**`FocusHandle`** (gp/window.rs)
- Create it with `cx.focus_handle()`. `pub fn focus_handle(&self) -> FocusHandle` gp/app.rs:2927 (available on `Context<T>` via Deref to App).
- `pub fn focus(&self, window: &mut Window, cx: &mut App)` window.rs:610. **It takes `cx`**, so it is `handle.focus(window, cx)`, not upstream's older `focus(window)`. `Window::focus(&mut self, handle: &FocusHandle, cx: &mut App)` :2346, `Window::focused(&self, cx: &App) -> Option<FocusHandle>` :2329, `Window::blur(&mut self, cx)` :2359.
- `is_focused(&self, window: &Window) -> bool` :615, `contains_focused(&self, window, cx: &App)` :621, `within_focused(&self, window, cx: &mut App)` :627, `contains(&self, other, window)` :632, `tab_index(self, isize)` :582, `tab_stop(self, bool)` :593, `downgrade()` :602.
- `pub trait Focusable: 'static { fn focus_handle(&self, cx: &App) -> FocusHandle; }` window.rs:710-713. Helpers: `Context::focus_view(&mut self, view: &Entity<W>, window)` context.rs:283, `Context::focus_self(&mut self, window)` context.rs:747 (deferred).
- Focus events: `Context::on_focus` :540, `on_focus_in` :566, `on_blur` :589, `on_focus_out` :635 (gp/app/context.rs).

**Menus**, gp/platform/app_menu.rs
```rust
pub struct Menu { pub name: SharedString, pub items: Vec<MenuItem>, pub disabled: bool }      // :4-13
impl Menu { pub fn new(name: impl Into<SharedString>) -> Self; pub fn items(self, impl IntoIterator<Item = MenuItem>) -> Self; pub fn disabled(self, bool) -> Self } // :17,26,32
pub enum MenuItem {                                                                         // :76-104
    Separator, Submenu(Menu), SystemMenu(OsMenu),
    Action { name: SharedString, action: Box<dyn Action>, os_action: Option<OsAction>, checked: bool, disabled: bool },
}
MenuItem::separator() :108, ::submenu(Menu) :113, ::os_submenu(name, SystemMenuType::Services) :118,
MenuItem::action(name: impl Into<SharedString>, action: impl Action) :126,
MenuItem::os_action(name, action, os_action: OsAction) :137, .checked(bool) :176, .disabled(bool) :198
pub enum OsAction { Cut, Copy, Paste, SelectAll, Undo, Redo }                               // :311-329
```
- **Surprise:** `Menu` gained a third field, `disabled`. A struct literal `Menu { name, items }` will not compile. Use `Menu::new("File").items([...])`.
- `pub fn set_menus(&self, menus: impl IntoIterator<Item = Menu>)` gp/app.rs:2668. `get_menus()` :2674, `set_dock_menu(Vec<MenuItem>)` :2679.
- Working example: gpui-pre-0.3.8/examples/set_menus.rs:98-117 (`cx.set_menus([Menu::new("set_menus").items([...])])`). It also registers handlers with `cx.on_action(quit)` (:39) and calls `cx.activate(true)` (:36). Menu key equivalents come from the keymap: `set_menus` passes `&self.keymap.borrow()` (app.rs:2670).

### 13. Entities, views, async

- **`AppContext` trait** (gp/gpui.rs:221-297). `use gpui_kit::prelude::*` imports it as `AppContext as _` (gp/prelude.rs:5-9):
  ```rust
  fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T>;   // :228
  fn update_entity<T, R>(&mut self, handle: &Entity<T>, update: impl FnOnce(&mut T, &mut Context<T>) -> R) -> R; // :243-249
  fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R> where R: Send + 'static; // :287-289
  ```
  **Surprise:** `AppContext` has no associated `type Result<T>`. `update_entity` / `Entity::update` return `R` directly, even on `AsyncApp`, which panics with "app was released before async operation completed" if the App is gone (gp/app/async_context.rs:28-33, 58-66). Only `WeakEntity::update` returns `Result<R>`.
- **`Entity<T>`** (gp/app/entity_map.rs): `read(&self, cx: &'a App) -> &'a T` :485, `read_with<R, C: AppContext>(&self, cx: &C, f: impl FnOnce(&T, &App) -> R) -> R` :491, `update<R, C: AppContext>(&self, cx: &mut C, update: impl FnOnce(&mut T, &mut Context<T>) -> R) -> R` :497-501, `update_in<R, C: VisualContext>(&self, cx: &mut C, update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R) -> C::Result<R>` :523-527, `downgrade() -> WeakEntity<T>` :470, `entity_id()` :464.
- **`WeakEntity<T>`**: `upgrade() -> Option<Entity<T>>` :792, `update<C: AppContext, R>(&self, cx: &mut C, update: impl FnOnce(&mut T, &mut Context<T>) -> R) -> Result<R>` :803-809, `update_in(&self, cx, impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R) -> Result<R>` :819-825 (looks up the entity's current window), `read_with(&self, cx: &C, ...) -> Result<R>` :839.
- **`Render`** (gp/element.rs:165-168):
  ```rust
  pub trait Render: 'static + Sized {
      fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
  }
  ```
- **`RenderOnce`** (element.rs:181-186): `fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement;`. Pair it with `#[derive(IntoElement)]`.
- **Surprise:** `#[derive(IntoElement)]` expands to `type Element = gpui::ViewElement<Self>` (gpui-pre-macros-0.3.8/src/derive_into_element.rs:10-16). Upstream used `Component<Self>`; `struct Component` does not exist here. A blanket `impl<T: RenderOnce> View for T` exists (gp/view.rs:208). `Entity<V: Render>` implements `IntoElement` (view.rs:100). `AnyView::cached(style)` / `Entity::cached(style)` return `ViewElement` (view.rs:44, 242).
- **`EventEmitter`**: `pub trait EventEmitter<E: Any>: 'static {}` gp/gpui.rs:346. Emit with `pub fn emit<Evt>(&mut self, event: Evt) where T: EventEmitter<Evt>, Evt: 'static` (gp/app/context.rs:760-763).
- **Notify / observe / subscribe** (gp/app/context.rs). All of these return a `Subscription` that must be stored or `.detach()`ed.
  ```rust
  pub fn notify(&mut self)                                                               // :221
  pub fn observe<W>(&mut self, entity: &Entity<W>, on_notify: impl FnMut(&mut T, Entity<W>, &mut Context<T>) + 'static) -> Subscription // :64-68
  pub fn subscribe<T2, Evt>(&mut self, entity: &Entity<T2>, on_event: impl FnMut(&mut T, Entity<T2>, &Evt, &mut Context<T>) + 'static) -> Subscription // :94-98
  pub fn observe_in<V2>(&mut self, observed: &Entity<V2>, window: &mut Window, on_notify: impl FnMut(&mut T, Entity<V2>, &mut Window, &mut Context<T>) + 'static) -> Subscription // :317-322
  pub fn subscribe_in<Emitter, Evt>(&mut self, emitter: &Entity<Emitter>, window: &Window, on_event: impl FnMut(&mut T, &Entity<Emitter>, &Evt, &mut Window, &mut Context<T>) + 'static) -> Subscription // :341-346
  ```
  Use `subscribe_in` with gpui-component `InputState` events. Note that `subscribe` passes `Entity<T2>` by value while `subscribe_in` passes `&Entity<Emitter>`.
- **Async (AsyncFn closures; requires Rust ≥ 1.85 async closures)**
  ```rust
  // Context<T>, gp/app/context.rs:230-233
  pub fn spawn<AsyncFn, R>(&self, f: AsyncFn) -> Task<R>
  where T: 'static, AsyncFn: AsyncFnOnce(WeakEntity<T>, &mut AsyncApp) -> R + 'static, R: 'static
  // Context<T>, gp/app/context.rs:670-673
  pub fn spawn_in<AsyncFn, R>(&self, window: &Window, f: AsyncFn) -> Task<R>
  where R: 'static, AsyncFn: AsyncFnOnce(WeakEntity<T>, &mut AsyncWindowContext) -> R + 'static
  // App, gp/app.rs:2177-2180
  pub fn spawn<AsyncFn, R>(&self, f: AsyncFn) -> Task<R>
  where AsyncFn: AsyncFnOnce(&mut AsyncApp) -> R + 'static, R: 'static
  // Window, gp/window.rs:2693-2696
  pub fn spawn<AsyncFn, R>(&self, cx: &App, f: AsyncFn) -> Task<R>
  where R: 'static, AsyncFn: AsyncFnOnce(&mut AsyncWindowContext) -> R + 'static
  // BackgroundExecutor, gp/executor.rs:103
  pub fn spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R> where R: Send + 'static
  // BackgroundExecutor, gp/executor.rs:183
  pub fn timer(&self, duration: Duration) -> Task<()>
  ```
  `cx.background_executor()` returns `BackgroundExecutor` by value on `App` (app.rs:311) and `&BackgroundExecutor` on `AsyncApp` (async_context.rs:159). `cx.background_spawn(fut)` is available on every context through `AppContext` (gpui.rs:287). There is **no `Timer` type**: use `cx.background_executor().timer(dur).await`.

  Canonical pattern:
  ```rust
  self.task = Some(cx.spawn(async move |this, cx| {
      let lines = cx.background_spawn(async move { index_file(path) }).await;
      this.update(cx, |this, cx| { this.lines = lines; cx.notify(); }).ok();
  }));
  ```
- **`Task`** is `scheduler::Task` re-exported from gpui-pre-scheduler (gp/executor.rs:12-14; struct at gpui-pre-scheduler-0.3.8/src/executor.rs:402). Methods: `Task::ready(val)` :539, `is_ready()` :562, `detach(self)` :572. Dropping a `Task` cancels it, so keep it in a field or `.detach()` it. `TaskExt::detach_and_log_err(self, cx: &App)` applies to `Task<Result<T, E>>` (gp/executor.rs:38-40; also in prelude as `TaskExt as _`).

### 15. Testing, element geometry, capture

**Test macro.** `#[gpui::test]` (gpui-pre-macros-0.3.8/src/gpui_macros.rs:250-253) is re-exported as `gpui::test` (gp/gpui.rs:116-118), so through gpui-kit it is `#[gpui_kit::test]` (gpui-kit-0.7.1/tests/test_macro.rs:12-24). Function parameters can be `&mut TestAppContext` / `TestAppContext`, `StdRng` or `BackgroundExecutor`, and the function may be sync or `async` (gpui-pre-macros-0.3.8/src/test.rs:143-164, 252-255). Supported arguments: `seed`, `seeds(..)`, `iterations`, `retries`, `on_failure` (gpui_macros.rs:231-240).
- Required setup: dev-dependency `gpui-kit = { version = "=0.7.1", features = ["test-support"] }`. `TestAppContext` and `VisualTestContext` exist only with `cfg(any(test, feature = "test-support"))` (gp/app.rs:36-41, 69-74).
- Gotcha: in test modules **don't `use gpui_kit::*`**. With test-support enabled the glob brings in the `test` attribute and shadows the built-in `#[test]` (gpui-kit-0.7.1/tests/test_macro.rs:4-6).

**`TestAppContext`** (gp/app/test_context.rs):
- `add_window<F, V>(&mut self, build_window: F) -> WindowHandle<V> where F: FnOnce(&mut Window, &mut Context<V>) -> V` :279. Note the closure returns `V`, not an Entity.
- `open_window(&mut self, window_size: Size<Pixels>, build_window: F) -> WindowHandle<V>` :302 opens a window of a fixed size.
- `add_window_view<F, V>(&mut self, build_root_view: F) -> (Entity<V>, &mut VisualTestContext)` :347
- `add_empty_window() -> &mut VisualTestContext` :326
- `update(&self, f: impl FnOnce(&mut App) -> R)` :266, `run_until_parked()` :565, `dispatch_action(window, action)` :570, `simulate_keystrokes(&mut self, window: AnyWindowHandle, keystrokes: &str)` :587 (space-separated, e.g. `"cmd-f a b enter"`), `simulate_input(window, &str)` :603, `simulate_window_resize(window, Size<Pixels>)` :461.
- Test windows use the mocked `TestPlatform`. `add_window` / `add_window_view` use `Bounds::maximized` of the test display (:285-287).

**`VisualTestContext`** (struct at test_context.rs:827; derefs to `TestAppContext`):
- `update(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R` :837, `from_window(window: AnyWindowHandle, cx: &TestAppContext)` :846
- `simulate_keystrokes(&str)` :883, `simulate_input(&str)` :889, `simulate_mouse_move(position, button: impl Into<Option<MouseButton>>, modifiers)` :894, `simulate_mouse_down(position, MouseButton, Modifiers)` :908, `simulate_mouse_up` :924, `simulate_click(&mut self, position: Point<Pixels>, modifiers: Modifiers)` :939 (left button only; for a middle click use `simulate_mouse_down/up(.., MouseButton::Middle, ..)`), `simulate_event<E: InputEvent>` :1027, `simulate_resize(Size<Pixels>)` :972
- `debug_bounds(&mut self, selector: &'static str) -> Option<Bounds<Pixels>>` :990

**`debug_selector` gating (important).** Selectors are recorded only under `#[cfg(any(test, all(debug_assertions, feature = "test-support")))]`. Otherwise the method is a no-op (gp/elements/div.rs:884-900; recording at :2526-2531). The only public reader is `VisualTestContext::debug_bounds`, because the map is `pub(crate) debug_bounds` on the frame (gp/window.rs:1002). Consequences:
- Calling `.debug_selector(|| "x".into())` always compiles, so it is safe to leave in production code.
- Reading the bounds requires a debug build with test-support **and** a `VisualTestContext`, i.e. a TestPlatform window. **A running app cannot read `debug_bounds`.**

**gpui-kit test helpers** (gpui-kit-0.7.1/src/test.rs, needs feature `test-support`; TESTING.md):
- `TestWindowExt` is implemented on `Window` (test.rs:25-51, 205-272): `find(id) -> ElementSnapshot`, `try_find(id) -> Option<ElementSnapshot>`, `within(id)`, `render_frame(cx)`, `click(id, cx)`, `click_at(id, offset, cx)`, `right_click`, `double_click`, `hover`, `scroll(id, ScrollDelta, cx)`, `drag(from, to, cx)`, `drag_to(from_id, to_id, cx)`, `press("cmd-f", cx)`, `input("text", cx)`.
- `ElementSnapshot` getters (gpui-base-0.7.1/src/test_support.rs:17-115): `bounds() -> Bounds<Pixels>` :88, `visible()` :91, `path() -> &[ElementId]` :68, `role()`, `label()`, `value()`, `focused()`, `selected()`, `checked()`, `expanded()`, `disabled()`.
- Elements are registered with `.id("x").test_support()`. Call `.test_support()` **before** `.track_focus` (TESTING.md:50-54, 69-73). Example usage: gpui-kit-0.7.1/tests/window.rs:8-55 (`window.find("trigger").bounds().size == size(px(120.), px(32.))`). The fixture for opening a window of a specific size is at gpui-kit-0.7.1/tests/common/mod.rs:8-31; it uses `gpui_kit::open_window` with `WindowBounds::Windowed(Bounds{origin: Point::default(), size})`.
- **Without the feature**: `gpui_kit::TestSupportExt` is still exported (gpui-kit-0.7.1/src/lib.rs:182). `.test_support()` then returns the element unchanged as `ObservedElement<E> = E` (gpui-base-0.7.1/src/observe.rs:5-9, 27-35), but `gpui_base::test_support` (snapshots and `find`) does not exist (gpui-base-0.7.1/src/lib.rs:247-248). Bounds can therefore be read only when `test-support` is enabled.
- **Usable in a running (non-test) app if built with `gpui-kit/test-support`:** the registry is a process-wide thread-local keyed per window (test_support.rs:121-123, 147-150) and holds no TestPlatform state. `#[doc(hidden)] pub fn snapshots(window: &Window) -> Vec<ElementSnapshot>` (test_support.rs:172-188) and `find(window, scope, id)` (:147) can be called from app code, e.g. behind a `--dump-geometry` debug action. They return `.bounds()` and `.path()` for every `.test_support()`-registered element. This relies on a doc-hidden API. **VERIFIED at runtime** (see "Runtime verification" in Q15 below): works both in a `HeadlessAppContext` and in a live `gpui_kit::application()` window, debug build with `gpui-kit/test-support`. gpui's own doc says test-support "can stay enabled in release builds" (div.rs:881-883).

**Other ways to get geometry or pixels from a live window:**
- `Window::render_to_image(&self) -> anyhow::Result<image::RgbaImage>` (gp/window.rs:2749-2756, fn at :2753) under `cfg(any(test, feature = "test-support"))`. It renders the current scene to a texture without presenting it, and it is a plain `Window` method, so a test-support build of the real app can call it (Metal path; exact macOS behaviour not verified). Also `painted_quads() -> Vec<Quad>` :2763 and `painted_underlines()` :2769.
- `VisualTestAppContext` (gp/app/visual_test_context.rs:12-31; macOS + test-support only, gp/app.rs:40-41,73-74) runs the real `MacPlatform`/Metal with deterministic scheduling:
  - `VisualTestAppContext::new(platform: Rc<dyn Platform>)` :45 and `with_asset_source(platform, Arc<dyn AssetSource>)` :53. The default source is a no-op, so SVG icons won't render (:42-43).
  - `open_offscreen_window(size, build_root) -> Result<WindowHandle<V>>` :97 places the window at (-10000,-10000) (:104-107).
  - `capture_screenshot(&mut self, window: AnyWindowHandle) -> Result<RgbaImage>` :384. The platform can come from `gpui_platform::current_platform(false)` (gpui-pre-platform-0.3.8/src/gpui_platform.rs:67) or `current_headless_renderer()` (:110-117, test-support).
  - gpui-kit's opt-in `rendering` test target uses real Metal images (gpui-kit-0.7.1/TESTING.md:107-113).
- Accessibility tree, usable in release builds with no feature flags: during prepaint, elements that have an id **and** a non-`GenericContainer` role get AccessKit nodes with bounds in device pixels (gp/element.rs:366-381). This happens only while an AT client is active (`window.a11y.is_active()`). Add `.id(..).role(Role::Group).accessibility_id("rogg-search")` to make an element visible to external macOS AX tooling as `AXIdentifier` (div.rs:1310, 1326). A plain div reports no role (div.rs:1905-1911). Not verified at runtime.
- Inspector: `Window::toggle_inspector(&mut self, cx)` under `cfg(any(feature = "inspector", debug_assertions))` (gp/window.rs:6959-6960) is an interactive picker, not a dump API.

### Surprises vs upstream Zed gpui (for Q11–15)
1. `FocusHandle::focus(&self, window: &mut Window, cx: &mut App)` takes `cx` (gp/window.rs:610), and so does `Window::focus(handle, cx)` (:2346).
2. `AppContext` has no `Result` associated type. `Entity::update` returns `R` even in async contexts and panics if the app was released (gp/app/async_context.rs:28-33). `WeakEntity::update` returns `anyhow::Result<R>`.
3. Spawn closures are Rust async closures: `cx.spawn(async move |this, cx| ...)` with `cx: &mut AsyncApp` (context.rs:230-233). The older `|this, mut cx| async move {}` form does not compile.
4. `#[derive(IntoElement)]` targets `ViewElement<Self>` (no `Component<T>`), and `View` is a trait with blanket impls (gp/view.rs:192-224).
5. `Menu` has a `disabled` field (app_menu.rs:4-13). Use `Menu::new(..).items(..)`.
6. `uniform_list`'s closure is `Fn(Range<usize>, &mut Window, &mut App)`, so you need `cx.processor(...)` (context.rs:260). The `uniform_list(view, id, n, |this, range, window, cx| ..)` form from older docs is gone. `scroll_to_item` is non-strict; use `scroll_to_item_strict` for forced centering.
7. `debug_selector` records only in `debug_assertions` builds with `test-support`, and is a silent no-op elsewhere (div.rs:884-900).
8. Accessibility is built in: `StatefulInteractiveElement` has `role`, `aria_label`, `accessibility_id`, etc. (div.rs:1306-1412), and the `Element` trait has `a11y_role` / `write_a11y_info` hooks (used at gp/element.rs:370-380).
9. gpui-pre's proc-macros rewrite `gpui::` to `gpui_kit::` when the consuming crate depends on `gpui-kit` (gpui_pre_facade_paths.rs:11-35). Apps should depend on `gpui-kit` alone. `#[gpui_kit::test]` is GPUI's test macro.
10. There is no `Timer` type. Use `background_executor().timer(Duration)` (gp/executor.rs:183). `Task` comes from the separate `gpui-pre-scheduler` crate.

#### Runtime verification of geometry dump & pixel capture (scratch probes)

All built with `gpui-kit = { version = "=0.7.1", features = ["test-support"] }`, debug profile, rustc 1.99, on this Mac.

1. **Headless (no window on screen, no Screen Recording permission needed)** - `/tmp/rogg-research/scratch2/src/main.rs`:
   ```rust
   let mut cx = HeadlessAppContext::with_platform(
       gpui_kit::platform::current_platform(true).text_system(),
       Arc::new(gpui_kit::assets::Assets),
       gpui_kit::platform::current_headless_renderer,
   );
   cx.update(gpui_kit::init);
   let (handle, _) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
       window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Default::default(), size: size(px(300.), px(120.)) })),
       focus: false, show: false, ..Default::default() }, cx, |_, cx| cx.new(|_| V))).unwrap();
   cx.update_window(handle, |_, window, cx| {
       use gpui_kit::test::TestWindowExt;
       window.render_frame(cx);
       let snap = window.find("hello");                       // element marked .id("hello").test_support()
       println!("{:?}", snap.bounds());                       // Bounds { origin: (8px, 8px), size: 284px x 26px }
       for s in gpui_kit::base::test_support::snapshots(window) { println!("{:?} {:?}", s.path(), s.bounds()); }
   }).unwrap();
   let img = cx.capture_screenshot(handle).unwrap();         // image::RgbaImage, 600x240 (2x)
   img.save("headless.png").unwrap();                         // PNG saving works (gpui's image dep has png)
   ```
   Output: correct bounds for `hello` and for gpui-component `Button` "ok" (components self-register; path e.g. `[View(..), Name("gpui_component::window_border::WindowBorder"), Name("root"), View(..), Name("gpui_component::button::button::Button"), Name("gpui_base::button::Button"), Name("ok")]`); PNG rendered correctly (text + primary button, light theme).
2. **Live app window** - `/tmp/rogg-research/scratch2/examples/live.rs`: inside `gpui_kit::application().run(..)`, after 500 ms (`cx.spawn(async move |cx| { cx.background_executor().timer(..).await; cx.update(|cx| h.update(cx, |_, window, _| ...)) })`), `gpui_kit::base::test_support::snapshots(window)` returned `Name("hello")` bounds `(8,8) 284x26`, and `window.render_to_image()` returned a correct 600x400 RgbaImage (saved `live.png`, shows the text).
3. `screencapture -x -o -l<windowNumber>` from this shell failed with "could not create image from window" (Screen Recording permission for the terminal is presumably not granted) - so prefer the in-process `render_to_image` / headless `capture_screenshot` paths for automated screenshots.

## D. gpui-kit / gpui-component components (0.7.1)

Abbreviations for paths (all relative to `/tmp/rogg-research/`):
- `GC` = `gpui-component-0.7.1/src`
- `GB` = `gpui-base-0.7.1/src`
- `GKA` = `gpui-kit-assets-0.7.1`
- `GKT` = `gpui-kit-0.7.1/tests`

Import paths: with the app depending only on `gpui-kit`, these are `gpui_kit::component::...` (re-export of `gpui_component`, `gpui-kit-0.7.1/src/lib.rs` `pub use ::gpui_component as component;`). Example test imports: `GKT/overlays.rs:2-8`:
```rust
use gpui_kit::component::{
    Root, WindowExt,
    button::Button,
    dialog::{DialogAction, DialogClose, DialogFooter},
    input::{Input, InputState},
    notification::Notification,
};
```
Crate-root re-exports in `GC/lib.rs`: modules `GC/lib.rs:26-92` (`accordion, alert, …, button, checkbox, color_picker, dialog, dock, input, list, menu, notification, popover, resizable, scroll, select, sidebar, switch, tab, table, theme, tooltip, tree, …`); `pub use gpui_base::Root;` (`GC/lib.rs:97`); `pub use styled::*;` (gives `Sizable`, `Size`, `StyleSized`, `h_flex`, `v_flex`, `StyledExt`; `GC/styled.rs:2`); `pub use theme::*;` (`GC/lib.rs:116`, gives `ActiveTheme`, `Theme`, `ThemeMode`, `ThemeColor`); `pub use title_bar::*;` (`GC/lib.rs:118`); `pub use window_ext::WindowExt;` (`GC/lib.rs:121`); `pub use virtual_list::{VirtualList, VirtualListScrollHandle, h_virtual_list, v_virtual_list};` (`GC/lib.rs:119`); resizable re-exports `GC/lib.rs:111-114`.

`gpui_component::init` (`GC/lib.rs:128-149`) — must be called once (gpui_kit::init calls it):
```rust
pub fn init(cx: &mut App) {
    theme::init(cx);
    global_state::init(cx);
    #[cfg(any(feature = "inspector", debug_assertions))]
    inspector::init(cx);
    root::init(cx);
    gpui_base::init(cx);
    input::init(cx);
    ...
    tooltip::init(cx);
}
```

### 16. Components

#### Root (component vs base) — window root for dialogs/notifications
- There is only ONE Root type: `gpui_component::Root` is a re-export of `gpui_base::Root` (`GC/lib.rs:97`).
- `gpui_base::Root::new` (`GB/root.rs:147`):
  ```rust
  pub fn new(view: impl Into<AnyView>, window: &mut Window, cx: &mut Context<Self>) -> Self
  ```
  Also `Root::read(window, cx)` (`GB/root.rs:176`), `Root::update(window, cx, f)` (`GB/root.rs:183`), `Root::view()`, `Root::plugin::<V>()`, `Root::register_plugin::<V: RootPlugin>(cx, build)` (`GB/root.rs:100-103`).
- Component functionality (dialogs, sheets, notifications, tooltips, native-menu fallback) lives in a **Root plugin** `WindowState` registered by `gpui_component::init` → `root::init` (`GC/root.rs:19-21`):
  ```rust
  pub(crate) fn init(cx: &mut App) {
      gpui_base::Root::register_plugin::<WindowState>(cx, WindowState::new);
  }
  ```
  Plugin factories are captured when a Root is created, so `init` must run BEFORE opening windows (`GB/root.rs:43-44`, and the panic message `GC/root.rs:474-475`: `"component window state is missing; call gpui_component::init before gpui_kit::open_window"`).
- The plugin's `Render` output is mounted automatically as an overlay above app content (`GB/root.rs:36-38`); it renders sheet, dialog and notification layers itself (`GC/root.rs:461-471`, `GC/root.rs:483-495`, `WindowStateLayers` with `.debug_selector(|| "root-layers".to_string())` and `.children(WindowState::dialog_layer(..)) .children(WindowState::notification_layer(..))`). There is **no** `Root::render_dialog_layer` / `render_notification_layer` to call manually (grep finds none in GC).
- The plugin also: sets `window.set_rem_size(cx.theme().font_size)` every render (`GC/root.rs:436`), sets root surface `font_family(theme.font_family)`, `bg(theme.tokens.background)`, `text_color(theme.foreground)` (`GC/root.rs:440-448`), and wraps the surface in `window_border()` (`GC/root.rs:450-458`; window_border only draws on client-decorated Linux, `GC/window_border.rs:29`, `window_paddings` returns 0 for `Decorations::Server` `GC/window_border.rs:88-92`).
- Conclusion: `gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| MyView::new(window, cx)))` is sufficient: it wraps the view in `base::Root` (`gpui-kit-0.7.1/src/lib.rs` `cx.new(|cx| base::Root::new(view, window, cx))`). Do NOT wrap your view in another Root. If calling `cx.open_window` directly, return `cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))`.

#### Button (`GC/button/button.rs`)
- Constructor: `pub fn new(id: impl Into<ElementId>) -> Self` (`:241`). `#[derive(IntoElement)] pub struct Button` (`:189-190`).
- Variant trait `pub trait ButtonVariants: Sized { fn with_variant(self, variant: ButtonVariant) -> Self; fn primary/secondary/danger/warning/success/info/ghost/link/text(self) -> Self; fn custom(self, style: ButtonCustomVariant) -> Self }` (`:48-100`). Import `button::{Button, ButtonVariants}`.
- `pub enum ButtonVariant { Default, Primary, Secondary, Danger, Info, Success, Warning, Ghost, Link, Text, Custom(ButtonCustomVariant) }` (`:146-158`).
- Custom colors: `ButtonCustomVariant::new(cx: &App) -> Self` (`:103`), then `.color(Hsla)` (bg, default transparent) `:114`, `.foreground(Hsla)` `:120`, `.hover(Hsla)` `:126`, `.active(Hsla)` `:132`, `.shadow(bool)` `:138`.
  ```rust
  Button::new("x").custom(ButtonCustomVariant::new(cx).color(bg).foreground(fg).hover(hov).active(act))
  ```
- Other builders: `.label(impl Into<SharedString>)` `:371`, `.icon(impl Into<ButtonIcon>)` `:397` (Icon/IconName convert), `.tooltip(impl Into<SharedString>)` `:403`, `.tooltip_with_action(..)` `:418`, `.loading(bool)` `:435`, `.compact()` `:441` ("padding will be reduced"), `.on_click(impl Fn(&ClickEvent, &mut Window, &mut App) + 'static)` `:447-450`, `.on_hover(impl Fn(&bool, &mut Window, &mut App))` `:456`, `.outline()` `:332`, `.rounded(impl Into<ButtonRounded>)` `:338` (`ButtonRounded::{None, Small, Medium, Large, Size(Pixels)}` `:24-31`, `From<Pixels>` `:33`), `.tab_stop(bool)` `:480`, `.dropdown_caret(bool)` `:486`, `.toggled(bool)` `:497`. Implements `Sizable` (`:562`), `Disableable` (`:524`), `Selectable` (`:542`), `Styled` (`:580`, forwards to base style), `ParentElement` (`:586`), `InteractiveElement` (`:592`).
- Size → geometry (`:658-683`): icon-only button: `XSmall => size_5()` (20px), `Small => size_6()` (24px), `Medium|Large => size_8()`; labelled: `XSmall => h_5().px_1()`, `Small => h_6().px_2()` (compact: `min_w_6().px_1p5()`), `Medium => h_8().px_2p5()`, `Size::Size(px) => px(size*0.2)`. Text size: `XSmall => text_xs`, `Small|Medium => text_sm`, else `text_base` (`GC/sizing.rs` `button_text_size`).
- Restyling: the user's `Styled` refinements are applied LAST: `.refine_style(&instance_style)` (`:733`), so `.bg(..)`, `.h(px(..))`, `.px(..)`, `.text_color(..)`, `.rounded(..)` on a Button override the variant's *normal* style. But hover/active colors come from the variant (`:711-724` calls `this.hover(...)`/`.active(...)`), so use `ButtonCustomVariant` (or `.ghost()`) for hover colors. Calling `.hover(..)` yourself on an interactive Button will hit gpui's `debug_assert!(self.interactivity().hover_style.is_none(), "hover style already set")` (`gpui-pre-0.3.8/src/elements/div.rs:842-847`) in debug builds.
- `Sizable for Button` also has `prepare_for_toolbar(self) -> Self { self.ghost().compact() }` (`:569-571`).

#### Input (`GC/input/input.rs`, state in `GB/input/base/state.rs`)
- `pub type InputState = InputBaseState<InputMode>;` (`GB/input/input/mod.rs:10`), re-exported as `gpui_component::input::InputState` (`GC/input/mod.rs:24-36`).
- Create: `impl InputBaseState<InputMode> { pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self }` (`GB/input/base/state.rs:10501-10505`). Builder methods: `pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self` (`:814`), `pub fn default_value(mut self, value: impl Into<SharedString>) -> Self` (`:1241`), `pub fn masked(mut self, masked: bool) -> Self` (`:10527`).
  ```rust
  let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
  ```
  (Same pattern used in `GC/list/list.rs:95-96` and `GKT/input.rs:55-58`.)
- Read/write: `pub fn value(&self) -> SharedString` (`:1257`), `pub fn text(&self) -> &Rope` (`:1287`), `pub fn set_value(&mut self, value: impl Into<super::InputContent>, window: &mut Window, cx: &mut Context<Self>)` (`:948-953`; does NOT emit `InputEvent::Change`, `:944-946`), `pub fn set_placeholder(&mut self, placeholder: impl Into<SharedString>, _: &mut Window, cx: &mut Context<Self>)` (`:899-904`), `pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>)` (`:1315`), `pub fn clean(&mut self, window, cx)` (`:2073`), `pub fn select_all(&mut self, _: &mut Window, cx: &mut Context<Self>)` (`:3025`). Note `focus`/`set_value` take `&mut Context<InputState>` → call via `state.update(cx, |s, cx| s.focus(window, cx))`.
- Events (`GB/input/base/state.rs:125-131`), `impl<M: InputModeKind> EventEmitter<InputEvent> for InputBaseState<M> {}` (`:562`):
  ```rust
  #[derive(Clone)]
  pub enum InputEvent {
      Change,
      PressEnter { secondary: bool, shift: bool },
      Focus,
      Blur,
  }
  ```
  Subscribe pattern (`GC/list/list.rs:98-99`, `:270-286`):
  ```rust
  let _sub = cx.subscribe_in(&query_input, window, Self::on_query_input_event);
  fn on_query_input_event(&mut self, state: &Entity<InputState>, event: &InputEvent,
                          window: &mut Window, cx: &mut Context<Self>) {
      match event { InputEvent::Change => { let text = state.read(cx).value(); ... } _ => {} }
  }
  ```
- Render element: `pub fn new(state: &Entity<InputState>) -> Self` (`GC/input/input.rs:213`). Builders: `.id(impl Into<ElementId>)` `:207`, `.prefix(impl IntoElement)` `:276`, `.suffix(impl IntoElement)` `:281`, `.h_full()` `:287` / `.h(impl Into<DefiniteLength>)` `:293` (both documented "Multi-line only"), `.appearance(bool)` `:299` ("if false the input field will no border, background"), `.bordered(bool)` `:305` (default true), `.focus_bordered(bool)` `:311`, `.cleanable(bool)` `:317` (clear button when non-empty), `.mask_toggle()` `:323`, `.disabled(bool)` `:346`, `.readonly(bool)` `:356`, `.tab_index(isize)` `:362`, `.context_menu(f: impl Fn(NativeMenu, &mut Window, &mut App) -> NativeMenu + 'static)` `:371-374`. Implements `Sizable` (`:146`), `Styled` (`:529`), `Selectable`, `FocusableExt::focus_ring(bool)` (`:164-167`, sets focus_bordered).
- Font: the text element uses the inherited `window.text_style()` (`GB/input/base/element.rs:1219-1220`, `:1636-1640`), so `.text_size(px(..))`, `.font_family(..)`, `.text_color(..)` set on `Input` (Styled → `refine_style(&self.style)` at `GC/input/input.rs:793`, after the size defaults at `:775-778`) apply to the edited text.
- Example in-source of a borderless search input with icon prefix (`GC/list/list.rs:684-692`):
  ```rust
  Input::new(&input)
      .with_size(self.options.size)
      .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground))
      .cleanable(true)
      .p_0()
      .appearance(false),
  ```
- Built-in key context is `"Input"` (`GB/input/base/state.rs:133`), bindings registered in `input::init`.

#### Popover (`GC/popover.rs`)
- `pub fn new(id: impl Into<ElementId>) -> Self` (`:136`). Builders: `.anchor(impl Into<Anchor>)` `:162` (default `Anchor::TopLeft` = opens below trigger, left-aligned), `.offset(impl Into<Pixels>)` `:169`, `.arrow(bool)` `:177`, `.mouse_button(MouseButton)` `:183`, `.trigger<T: Selectable + IntoElement + 'static>(T)` `:189-191` (e.g. a `Button`), `.default_open(bool)` `:205`, `.open(bool)` `:215` (controlled, pair with `on_open_change`), `.on_open_change(F: Fn(&bool, &mut Window, &mut App))` `:225`, `.trigger_style(StyleRefinement)` `:238`, `.overlay_closable(bool)` `:244`, `.content<F, E>(F) where F: Fn(&mut PopoverState, &mut Window, &mut Context<PopoverState>) -> E` `:253-257`, `.appearance(bool)` `:270` (false = no bg/border/shadow/padding), `.track_focus(&FocusHandle)` `:279`. `ParentElement` + `Styled` (`:285`, `:291`). Close from content: `PopoverState::dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>)` (`GB/popover.rs:61`).

#### PopupMenu / ContextMenu / DropdownMenu (`GC/menu/`)
- Exports (`GC/menu/mod.rs:9-12`): `AppMenuBar`, `ContextMenu, ContextMenuExt, ContextMenuState`, `DropdownMenu`, `PopupMenu, PopupMenuItem`.
- Right-click menu on any element (`GC/menu/context_menu.rs:17-27`):
  ```rust
  pub trait ContextMenuExt: InteractiveElement + ParentElement + Styled {
      #[track_caller]
      fn context_menu(
          mut self,
          f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
      ) -> ContextMenu<Self>
  ```
  Blanket impl for all such elements (`:43`). It makes the element relative and adds an absolute child (`:19-21`). Id is derived from the element id (or caller location) (`:30-37`), so give the element a stable `.id(..)`.
- Dropdown from a Button: `pub trait DropdownMenu` with `fn dropdown_menu(self, f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static) -> DropdownMenuPopover<Self>` and `dropdown_menu_with_anchor(..)` (`GC/menu/dropdown_menu.rs:12-31`); `impl DropdownMenu for Button {}` (`:33`).
- `PopupMenu` builder (`GC/menu/popup_menu.rs`): `.menu(label, action: Box<dyn Action>)` `:465`, `.menu_with_disabled(..)` `:481`, `.menu_with_check(..)` `:565`, `.menu_with_icon(..)` `:543`, `.menu_element(..)` `:587`, `.link(..)` `:498`, `.label(..)` `:492`, `.separator()` `:680`, `.submenu(..)` `:694`, `.item(impl Into<PopupMenuItem>)` `:728`, `.action_context(FocusHandle)` `:371` (actions dispatched to that handle), `.min_w/.max_w/.max_h/.scrollable` `:429-447`. Items with closures: `PopupMenuItem::new(label)` `:71` + `.on_click<F: Fn(&ClickEvent, &mut Window, &mut App)>` `:196-198`, `.icon(..)` `:126`, `.checked(bool)` `:180`, `.disabled(bool)` `:161`; `PopupMenuItem::separator()` `:113`.
- NOTE: `Input::context_menu` uses a different type, `NativeMenu` (`GC/input/input.rs:371-374`, builder `.menu_with_disabled(label, disabled, Box<dyn Action>)`, `.separator()`, `.menu(..)`, `.show(position, window, cx)` at `:614-660`).

#### Dialog (`GC/dialog/`, opened via `WindowExt`)
- `pub trait WindowExt: Sized` (`GC/window_ext.rs:13`), `impl WindowExt for Window` (`:92`):
  ```rust
  fn open_dialog<F>(&mut self, cx: &mut App, build: F)
  where
      F: Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static;              // :31-33
  fn open_alert_dialog<F>(&mut self, cx: &mut App, build: F)
  where F: Fn(AlertDialog, &mut Window, &mut App) -> AlertDialog + 'static;   // :51-53
  fn has_active_dialog(&mut self, cx: &mut App) -> bool;                      // :56
  fn close_dialog(&mut self, cx: &mut App);                                    // :59
  fn close_all_dialogs(&mut self, cx: &mut App);                               // :62
  fn open_sheet<F>(&mut self, cx: &mut App, build: F)
  where F: Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static;               // :15-17
  ```
  The builder closure is `Fn` (re-invoked every render) — clone captured entities inside it (as in `GKT/overlays.rs:36-40`).
- `Dialog` builders (`GC/dialog/dialog.rs`): `new(cx: &mut App)` `:329`, `.title(impl IntoElement)` `:365`, `.content(F: Fn(DialogContent, &mut Window, &mut App) -> DialogContent)` `:356`, `.footer(impl IntoElement)` `:381`, `.button_props(DialogButtonProps)` `:391`, `.on_close(impl Fn(&ClickEvent, &mut Window, &mut App))` `:405`, `.on_ok(impl Fn(&ClickEvent, &mut Window, &mut App) -> bool)` `:416` (return true to close), `.on_cancel(.. -> bool)` `:427`, `.close_button(bool)` `:436`, `.margin_top(..)` `:442`, `.w(..)`/`.width(..)` `:452/:460`, `.max_w(..)` `:466`, `.overlay(bool)` `:472`, `.overlay_closable(bool)` `:480`, `.keyboard(bool)` `:486`; `ParentElement` + `Styled`. Footer helpers `DialogFooter`, `DialogAction`, `DialogClose` (`GC/dialog/mod.rs`; usage `GKT/overlays.rs:42-44`, `:261-266`). `DialogButtonProps` builders `.ok_text`, `.ok_variant`, `.cancel_text`, `.cancel_variant`, `.show_cancel` (`GC/dialog/dialog.rs:59-83`).
- Full example (`GKT/overlays.rs:36-60`):
  ```rust
  window.open_dialog(cx, move |dialog, _, _| {
      dialog.title("Edit profile")
          .child(Input::new(&draft).id("name"))
          .footer(DialogFooter::new().child(DialogAction::new().child(Button::new("ok").label("Save"))))
          .on_ok(move |_, window, cx| { /* ... */ true })
  });
  ```

#### Notification
- `fn push_notification(&mut self, note: impl Into<Notification>, cx: &mut App);` (`GC/window_ext.rs:66`), plus `remove_notification::<T>`, `clear_notifications`, `notifications` (`:70-81`).
- `Notification::new()` (`GC/notification.rs:174`), `.message(..)` `:197`, constructors `Notification::info/success/warning/error(message)` `:203-224`, `.title(..)` `:250`, `.icon(..)` `:258`, `.with_type(NotificationType)` `:264`, `.placement(Anchor)` `:273`, `.autohide(bool)` `:321`, `.on_click(..)` `:327`, `.on_close(impl Fn(&mut Window, &mut App))` `:339`, `.action(..)` `:347`, `.id::<T>()` `:236`, `.id1::<T>(key)` `:242`, `.delivery(..)`/`.system()` (OS notification) `:297-316`. Example: `window.push_notification(Notification::new().message("Updated").autohide(true), cx);` (`GKT/overlays.rs:76-79`).

#### Tooltip (`GC/tooltip.rs`)
- `Tooltip::new(text: impl Into<Text>) -> Self` `:43`, `Tooltip::element<E, F: Fn(&mut Window, &mut App) -> E>(builder)` `:53`, `.action(&dyn Action, Option<&str>)` `:69` (shows key binding), `.key_binding(Option<Kbd>)` `:75`, `.build(self, _: &mut Window, cx: &mut App) -> AnyView` `:81`.
- Attach with gpui's stateful `.tooltip` (signature `fn tooltip(mut self, build_tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self`, `gpui-pre-0.3.8/src/elements/div.rs:1682`):
  ```rust
  div().id("x").tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))   // GC/attachment.rs:490
  ```
  Components (Button, Checkbox, Switch) have their own `.tooltip(impl Into<SharedString>)`.

#### Select (`GC/select.rs`)
- State: `SelectState<D: SearchableListDelegate>`; `pub fn new(delegate: D, selected_index: Option<IndexPath>, window: &mut Window, cx: &mut Context<Self>) -> Self` (`:152-157`). `Vec<T: SearchableListItem>` is a delegate (`GC/searchable_list/vec.rs:47`), items impl for `String`, `SharedString`, `&'static str` (`:9-33`). Tests use `SearchableVec::new(vec!["Rust","Go"])` (`GKT/controls.rs:79-84`).
- State methods: `.searchable(bool)` `:303`, `set_selected_index` `:309`, `set_selected_value` `:337`, `set_items` `:357`, `selected_index(&self, cx) -> Option<IndexPath>` `:367`, `selected_value(&self)` `:372`, `focus` `:377`.
- Event: `pub enum SelectEvent<D> { Confirm(Option<<D::Item as SearchableListItem>::Value>) }` (`:71-76`), `EventEmitter<SelectEvent<D>>` + `EventEmitter<DismissEvent>` (`:767`, `:774`).
- Element: `Select::new(&Entity<SelectState<D>>)` `:649`, `.placeholder`, `.menu_width`, `.menu_max_h`, `.icon`, `.cleanable`, `.search_placeholder`, `.disabled`, `.empty(..)`, `.appearance(bool)` (`:659-735`).

#### Switch / Checkbox
- `Switch::new(id: impl Into<ElementId>)` (`GC/switch.rs:35`), `.checked(bool)` `:56`, `.label(..)` `:62`, `.on_change<F: Fn(&bool, &mut Window, &mut App) + 'static>` `:92-95`, `.color(impl Into<Hsla>)` `:102`, `.tooltip(..)` `:108`; `Sizable`, `Disableable`, `Styled`.
- `Checkbox::new(id)` (`GC/checkbox.rs:39`), `.label(impl Into<Text>)` `:72`, `.checked(bool)` `:86`, `.on_change(impl Fn(&bool, &mut Window, &mut App) + 'static)` `:102`, `.on_click` `:92`, `.tooltip` `:66`; `Sizable`, `Disableable`, `Styled`, `ParentElement`.
- Controlled usage (`GKT/controls.rs:30-44`): `Checkbox::new("agree").label("Agree").checked(self.agreed).on_change(cx.listener(|this, checked, _, cx| { this.agreed = *checked; cx.notify(); }))`.

#### Scrollbar (`GC/scroll/`)
- `Scrollbar` is gpui-base's type (`GC/scroll/mod.rs:5-8`). Constructors `Scrollbar::new(&H)`, `::horizontal(&H)`, `::vertical(&H)` where `H: ScrollbarHandle + Clone` (`GB/scrollbar.rs:836`, `:853`, `:859`).
- `pub trait ScrollbarHandle: 'static { fn viewport_bounds(&self) -> Bounds<Pixels>; fn offset(&self) -> Point<Pixels>; fn set_offset(&self, offset: Point<Pixels>); fn content_size(&self) -> Size<Pixels>; fn start_drag(&self) {} fn end_drag(&self) {} }` (`GB/scrollbar.rs:69-82`). Implemented for `gpui::ScrollHandle` `:84`, `UniformListScrollHandle` `:102`, `ListState` `:121` — and you can implement it for a custom element's scroll model (useful for a custom-painted log view).
- Element extension `pub trait ScrollableElement` (`GC/scroll/scrollable.rs:16`), impl for `Div` and `Stateful<E>` (`:189-190`): `.scrollbar(&handle, axis)` `:19-23`, `.vertical_scrollbar(&handle)` `:32`, `.horizontal_scrollbar(&handle)` `:38`, `.overflow_scrollbar()` / `.overflow_x_scrollbar()` / `.overflow_y_scrollbar()` → `Scrollable<Self>` (`:45-60`; wrapper id defaults to caller location, `.id(..)` to override `:90`).
- Scrollbar visibility mode is theme-wide: `Theme::set_scrollbar_mode(ScrollbarMode, cx)` (`GC/theme/mod.rs:377`), `ScrollbarMode::{Scrolling, Hover, Always}`.

#### Tabs (`GC/tab/`)
- `TabBar::new(id)` (`GC/tab/tab_bar.rs:59`), `.with_variant(TabVariant)`/`.pill()/.outline()/.segmented()/.underline()` `:80-104`, `.menu(bool)` `:110`, `.max_width(..)` `:118`, `.track_scroll(&ScrollHandle)` `:127`, `.prefix(..)` `:133`, `.suffix(..)` `:139`, `.child(impl Into<Tab>)` `:151`, `.children(..)` `:145`, `.selected_index(usize)` `:157`, `.last_empty_space(..)` `:163`, `.on_click<F: Fn(&usize, &mut Window, &mut App)>` `:171-174`; `Sizable`, `Styled`.
- `Tab::new()` (`GC/tab/tab.rs:478`), `.label(..)` `:483`, `.icon(..)` `:499`, `.prefix(..)` `:535`, `.suffix(..)` `:541` (no built-in close button; put a close Button in `suffix`, see comment `:855`), `.disabled(bool)` `:547`, `.on_click(impl Fn(&ClickEvent, &mut Window, &mut App))` `:553`; `From<&'static str|String|SharedString|Icon|IconName>` `:421-445`; `ParentElement`, `Selectable`, `InteractiveElement`, `StatefulInteractiveElement`, `Styled`, `Sizable`.
- `pub enum TabVariant { Tab (default), Outline, Pill, Segmented, Underline }` (`GC/tab/tab.rs:14-21`).

#### TitleBar (`GC/title_bar.rs`) — relevant for a custom titlebar
- `pub const TITLE_BAR_HEIGHT: Pixels = px(34.);` `:15`; macOS left padding 80px for traffic lights `:16-17`.
- `TitleBar::new()` `:50`; `TitleBar::title_bar_options() -> TitlebarOptions { title: None, appears_transparent: true, traffic_light_position: Some(point(px(9.0), px(9.0))) }` `:59-65`; `TitleBar::window_options() -> WindowOptions { titlebar: Some(..), app_owns_titlebar_drag: true, ..Default::default() }` `:81-90`.
- Render: mouse-down + mouse-move calls `window.start_window_move()`, macOS double-click → `window.titlebar_double_click()`, inner bar uses `.window_control_area(WindowControlArea::Drag)` (render `:320`, double-click `:348`, `start_window_move` `:368`, drag area `:379`). `ParentElement` + `Styled`.

#### Sidebar (`GC/sidebar/`)
- `Sidebar<E: SidebarItem>::new(id)` (`GC/sidebar/mod.rs:238`), `.side(Side)` `:254`, `.collapsible(..)` `:264`, `.collapsed(bool)` `:270`, `.header(..)` `:276`, `.footer(..)` `:282`, `.child(E)` `:288`, `.children(..)` `:294`; `SidebarToggleButton::new()` `:311`. Items: `SidebarMenu`, `SidebarMenuItem::new(label)` (`GC/sidebar/menu.rs:443`), `SidebarGroup` (`GC/sidebar/group.rs`).

#### ColorPicker
- `pub use gpui_base::{ColorPickerEvent, ColorPickerState};` (`GC/color_picker.rs:10`). `ColorPickerState::new(window: &mut Window, cx: &mut Context<Self>) -> Self` (`GB/color_picker.rs:194`), `.default_value(impl Into<Hsla>)` `:253`, `value(&self) -> Option<Hsla>` `:262`, `set_value(&mut self, value: impl Into<Hsla>, window, cx)` `:287`. `pub enum ColorPickerEvent { Change(Option<Hsla>) }` (`GB/color_picker.rs:92-95`).
- Elements: `ColorPicker::new(&Entity<ColorPickerState>)` (`GC/color_picker.rs:73`) with `.featured_colors(Vec<Hsla>)` `:93`, `.icon` `:102`, `.label` `:110`, `.anchor(Anchor)` `:129`; `ColorSelect::new(&state)` `:546` (`.featured_colors`, `.placeholder`).

#### Resizable panels
- `h_resizable(id: impl Into<ElementId>) -> ResizablePanelGroup` / `v_resizable(id)` (`GC/resizable.rs:28-35`, styled wrappers around `gpui_base::h_resizable`), `resizable_panel() -> ResizablePanel` (`GB/resizable/mod.rs:27`, re-exported `GC/lib.rs:111-114`).
- `ResizablePanelGroup`: `.with_state(&Entity<ResizableState>)` (`GB/resizable/panel.rs:67`), `.axis(Axis)` `:73`, `.child(impl Into<ResizablePanel>)` `:83`, `.children(..)` `:89`, `.size(Pixels)` `:101`, `.on_resize(..)` `:111`; `EventEmitter<ResizablePanelEvent>` `:135`.
- `ResizablePanel`: `.visible(bool)` `:269`, `.size(impl Into<Pixels>)` `:275`, `.size_range(impl Into<Range<Pixels>>)` `:283`, `Styled` + `ParentElement` (`:289`, `:295`). `ResizableState` methods `sizes()`, `resize_panel`, `insert_panel`, `remove_panel`, `reset_panel` (`GB/resizable/mod.rs:56-233`).

#### Theme (`GC/theme/mod.rs`)
- `pub trait ActiveTheme { fn theme(&self) -> &Theme; }` implemented for `App` (`:44-53`) → `cx.theme().border` etc. (works from `Context<T>` via deref to App).
- `pub struct Theme` (`:147-212`): `pub colors: ThemeColor`, `pub tokens: ThemeTokens`, `pub highlight_theme`, `pub light_theme: Rc<ThemeConfig>`, `pub dark_theme: Rc<ThemeConfig>`, `pub mode: ThemeMode`, `pub font_family: SharedString` (default `.SystemUIFont`, `:160-166`), `pub font_size: Pixels` (default 16px, `:167-168`), `pub mono_font_family: SharedString` (macOS default `Menlo`, `:169-181`), `pub mono_font_size: Pixels` (default 13px), `pub radius: Pixels`, `pub radius_lg: Pixels`, `pub shadow: bool`, `pub focus_ring: bool`, `pub transparent: Hsla`, `pub scrollbar_mode`, `notification`, `list`, `sheet`, `motion`. Defaults `From<&ThemeColor> for Theme` (`:765-787`): font `.SystemUIFont`, 16px, radius 6px, radius_lg 8px, shadow true. `Theme: Deref<Target = ThemeColor>` (`:220-226`), so `cx.theme().background` reads `colors.background`.
- `ThemeColor` (`GC/theme/theme_color.rs:59`), all `Hsla`: e.g. `accent` `:61`, `background` `:67`, `border` `:69`, `foreground` `:165`, `input` `:175`, `list*` `:183-193`, `muted`/`muted_foreground` `:195-197`, `popover` `:199`, `primary*` `:203-209`, `ring` `:213`, `scrollbar*` `:215-219`, `selection` `:229`, `tab*` `:263-273`, `table*` `:275-293`, `title_bar`/`title_bar_border` `:295-297`, `status_bar` `:299`, `overlay` `:311`, `window_border` `:317`, named colors `red..cyan_light` `:320-342`.
- Accessors / mutation:
  ```rust
  pub fn global(cx: &App) -> &Theme                                    // :239
  pub fn global_mut(cx: &mut App) -> &mut Theme                        // :251  (raw; does NOT sync tokens/base or refresh)
  pub fn update<R>(cx: &mut App, edit: impl FnOnce(&mut Theme) -> R) -> R   // :288 (preferred: reconciles tokens, sync_base, refresh_windows)
  pub fn change(mode: impl Into<ThemeMode>, _window: Option<&mut Window>, cx: &mut App)  // :389
  pub fn sync_system_appearance(window: Option<&mut Window>, cx: &mut App)               // :354
  pub fn sync_base(cx: &mut App)                                                          // :473
  pub fn set_scrollbar_mode(mode: ScrollbarMode, cx: &mut App)                            // :377
  ```
  `Theme::change` reloads the mode's registered `ThemeConfig` (`:398` → `edit(.., reload_mode = true)` → `theme.apply_config(&config)` `:325`), which re-applies config font_size/font_family/mono/radius/shadow if the config sets them (`GC/theme/schema.rs:1085-1105`) — so apply app overrides with `Theme::update` AFTER `change`/mode switch. The doc on `update` (`:282-285`) says: edit colors in a second `update` after switching mode. The root surface paints `theme.tokens.background` (`GC/root.rs:445`) — editing only `colors` via `global_mut` will not change it; `update` reconciles.
  ```rust
  Theme::change(ThemeMode::Dark, None, cx);
  Theme::update(cx, |t| { t.background = rgb(0x1e1e1e).into(); t.border = ..; t.radius = px(4.); t.font_size = px(13.); t.mono_font_family = "Menlo".into(); });   // NOT "SF Mono": it silently resolves to Helvetica (see Q14 runtime check); ".AppleSystemUIFontMonospaced" also resolves
  ```
- `pub enum ThemeMode { #[default] Light, Dark }` (`:807-811`), `is_dark()`, `name()`, `From<WindowAppearance>` (`:828-835`). `theme::init` starts in Light (`:36-41`).

### 17. Compact 24px search field, 10px font, transparent bg, no border

Size system (`GC/sizing.rs`):
- `pub enum Size { Size(Pixels), XSmall, Small, #[default] Medium, Large }` (`:6-13`); `impl From<Pixels> for Size` (`:181-185`).
- `pub trait Sizable: Sized { fn with_size(mut self, size: impl Into<Size>) -> Self; fn xsmall(self) -> Self; fn small(self) -> Self; fn large(self) -> Self; }` (`:190-221`). (No `.medium()`; use `with_size(Size::Medium)`.)
- Input sizing used by `Input::render` (`GC/input/input.rs:771-778`): `.line_height(Rems(1.25))` (`:537`), single-line: `.input_px(size).input_py(size)`, then `.input_h(size)` and `.input_text_size(size)`.
  - `input_px`: Large 12, Medium 10, Small 8, XSmall 4, `Size(_)` 8 px (`GC/sizing.rs:147-155`).
  - `input_py`: Large 10, Medium 8, Small 2, XSmall 0, `Size(_)` 2 px (`:158-166`).
  - `input_h`: Large `h_11`, Medium `h_8`, Small `h_6`, XSmall `h_5`, `Size(_)` `h_6` (`:280-288`). With rem = 16px: h_5 = 20px, h_6 = 24px, h_8 = 32px (`gpui-pre-macros-0.3.8/src/styles.rs:973-981`: suffix "5" = `rems(1.25)`, "6" = `rems(1.5)`).
  - `input_text_size`: XSmall `text_xs` (0.75rem = 12px, `gpui-pre-0.3.8/src/styled.rs:545-547`), Small/Medium `text_sm`, Large `text_base`, `Size(s)` → `s * 0.875` (`GC/sizing.rs:245-252`).
  - Gap between prefix/input/suffix: Small 4px, Large 8px, else 6px (`GC/input/input.rs:700-704`).
  - IMPORTANT: rem is set by the component Root plugin to `theme.font_size` every frame (`GC/root.rs:436`), so all `h_5`, `text_xs`, etc. scale with `Theme::font_size` (default 16px).
- `.appearance(false)`: skips `bg`, `rounded(theme.radius)`, `border_1().border_color(theme.input)` (`GC/input/input.rs:784-791`) and the focus border/ring (`:747-753`, `:794-797` both require `self.appearance`). `.bordered(false)` keeps bg but drops the border (`:787-789`).
- Because user `Styled` refinements are applied after the size defaults (`.refine_style(&self.style)` at `:793`), you can override height/padding/text/line-height. Caveat: `Input` has an INHERENT `h(..)` (`:293`) that only affects multi-line inputs (applied at `:780-783` only `when(presentation.is_multi_line())`), so `Input::new(&s).h(px(24.))` does nothing for a single-line input. Use the Styled helpers that are not shadowed (`.h_6()`, which is 1.5rem) or call the trait method explicitly: `Styled::h(input, px(24.))`. When a suffix/clear button is shown, `.pr(size.input_px())` is applied after your style (`:809-810`).
- Recommended recipe (all methods exist as cited):
  ```rust
  use gpui_kit::component::{input::{Input, InputState}, Sizable, Icon, IconName, ActiveTheme};
  let input = Input::new(&self.search)              // GC/input/input.rs:213
      .xsmall()                                     // h_5/px 4/py 0/text_xs defaults
      .appearance(false)                            // no bg, border, radius, focus ring
      .cleanable(true)                              // :317
      .prefix(Icon::new(IconName::Search).size(px(12.)).text_color(cx.theme().muted_foreground))
      .text_size(px(10.))                           // Styled, overrides input_text_size
      .line_height(px(14.))                         // overrides Rems(1.25)
      .px(px(6.));
  let input = Styled::h(input, px(24.));            // NOT the inherent Input::h
  // wrap in a div if you want your own bg/border: div().h(px(24.)).bg(..).border_1().border_color(..).rounded(px(4.)).child(input)
  ```
  The edited text and placeholder use the inherited text style (`GB/input/base/element.rs:1219-1220`), so `.text_size`/`.font_family` reach the glyphs.

### 18. gpui-kit-assets

- Crate: `gpui-kit-assets 0.7.1`, lib `gpui_kit_assets` (`GKA/Cargo.toml`), re-exported as `gpui_kit::assets` when the `assets` feature (default) is on. Deps: `gpui` (= gpui-pre 0.3.8), and on native `rust-embed = "8.7.2"` with features `interpolate-folder-path`, `include-exclude` (`GKA/Cargo.toml` `[target.'cfg(not(target_family = "wasm"))'.dependencies.rust-embed]`). The crate is `links = "gpui-kit-default-icons"` and has a `build.rs`.
- `pub use native_assets::{AllAssets, Assets};` (`GKA/src/lib.rs:35-36`).
  - `Assets` = default component bundle; generated by build.rs into `OUT_DIR/default_assets.rs` as `#[derive(rust_embed::RustEmbed)] #[folder = "assets"] #[include = "icons/xxx.svg"]... pub struct Assets;` (`GKA/build.rs:110-120`), included at `GKA/src/native_assets.rs:5`. The default list is `GKA/default-icons.txt` (106 non-empty lines; the README says "101 component icons", a doc/data mismatch).
  - `AllAssets`: `#[derive(rust_embed::RustEmbed)] #[folder = "assets"] #[include = "icons/**/*.svg"] pub struct AllAssets;` (`GKA/src/native_assets.rs:7-11`): all 1,830 SVGs (1,818 Lucide 1.43.0 + 12 GPUI Kit, README), files in `GKA/assets/icons/` (1830 `.svg`, counted with `find`).
  - `AssetSource` impl for both (`GKA/src/native_assets.rs:13-42`):
    ```rust
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() { return Ok(None); }
        Self::get(path).map(|file| Some(file.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{}\"", path))
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> { ... starts_with(path) ... }
    ```
    NOTE: a missing path returns `Err`, not `Ok(None)` — so in a combined source, try your own source first and fall back to `Assets` last.
- gpui's trait (`gpui-pre-0.3.8/src/assets.rs:13-19`):
  ```rust
  pub trait AssetSource: 'static + Send + Sync {
      fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>>;
      fn list(&self, path: &str) -> Result<Vec<SharedString>>;
  }
  ```
  Registered with `pub fn with_assets(self, asset_source: impl AssetSource) -> Self` on `Application` (`gpui-pre-0.3.8/src/app.rs:202`); README usage: `gpui_kit::application().with_assets(gpui_kit::assets::Assets)`.
- Path convention: `"icons/<kebab-name>.svg"`; the enum variant is the PascalCase of the file stem (`GKA/build.rs:25-48`, `icons.insert(variant, format!("icons/{stem}.svg"))`). `IconName::path(self) -> SharedString` and `IconName::ALL` generated (`GKA/build.rs:53-67`), included at `GKA/src/icon.rs:3`. Trait `pub trait IconNamed { fn path(self) -> SharedString; }` (`GKA/src/icon.rs:7-9`) — implement it for your own enum to use with `Icon::new(..)`.
- Two `IconName` enums: `gpui_kit::assets::IconName` (full catalog) and `gpui_component::IconName` (legacy subset, generated from `default-icons.txt` via `__component_icon_names!`, `GC/icon.rs:13-38`); `From<component::IconName> for assets::IconName` (`GC/icon.rs:22-27`). `Icon::new` accepts either (`impl<T: IconNamed> From<T> for Icon`, `GC/icon.rs:59-63`). Test: `assert_eq!(LegacyIconName::Search.path(), "icons/search.svg")` (`GKT/assets.rs:23`).
- Using custom icons (`GC/icon.rs`): `Icon::new(icon: impl Into<Icon>)` `:105`, `Icon::empty()` `:163`, `.path(impl Into<SharedString>)` `:117` ("For example: `icons/foo.svg`"), `.data(&[u8])` `:136` (raw SVG bytes, no asset registration), `.rotate(..)` `:170`, `.transform(..)` `:158`; `Styled` with overridden `text_color` `:204-213`; `Sizable` → XSmall `size_3`, Small `size_3p5`, Medium `size_4`, Large `size_6`, `Size(px)` (`:187-193`). Default size = current text size (`:186`, `:224`). Icons render through `svg()` tinted with `text_color` (fallback = inherited text color, `:176`, `:225`). Lucide SVGs are stroke-based with `stroke="currentColor" stroke-width="2"` (`GKA/assets/icons/search.svg`).
  ```rust
  Icon::empty().path("icons/rogg/filter.svg").size(px(15.)).text_color(fg)
  ```
- Selected bundled extras without copying files: `icon_assets!(pub ExtraIcons, [Search, Check]);` (`GKA/src/lib.rs:65-86`); unlisted paths return `Ok(None)`.
- Combined source — the crate's own example (`GKA/examples/extra_assets.rs:20-37`):
  ```rust
  struct AppAssets;
  impl AssetSource for AppAssets {
      fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
          if let Some(bytes) = ExtraIcons.load(path)? { return Ok(Some(bytes)); }
          gpui_kit_assets::Assets.load(path)
      }
      fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
          let mut paths = gpui_kit_assets::Assets.list(path)?;
          paths.extend(ExtraIcons.list(path)?);
          paths.sort(); paths.dedup(); Ok(paths)
      }
  }
  ```
  Adapted for our own SVG directory (uses `rust-embed` like the crate does; the app would add `rust-embed` as its own dependency — `gpui_kit` does not re-export it):
  ```rust
  use gpui_kit::{AssetSource, Result, SharedString};
  use std::borrow::Cow;

  #[derive(rust_embed::RustEmbed)]
  #[folder = "assets"]              // contains assets/icons/rogg/*.svg
  #[include = "icons/**/*.svg"]
  struct RoggAssets;

  pub struct AppAssets;
  impl AssetSource for AppAssets {
      fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
          if let Some(f) = RoggAssets::get(path) { return Ok(Some(f.data)); }
          gpui_kit::assets::Assets.load(path)   // Err if missing -> keep last
      }
      fn list(&self, path: &str) -> Result<Vec<SharedString>> {
          let mut v = gpui_kit::assets::Assets.list(path)?;
          v.extend(RoggAssets::iter().filter(|n| n.starts_with(path)).map(|n| SharedString::from(n.to_string())));
          v.sort(); v.dedup(); Ok(v)
      }
  }
  // gpui_kit::application().with_assets(AppAssets).run(|cx| { gpui_kit::init(cx); ... });
  ```
  (`gpui_kit::AssetSource`, `Result`, `SharedString` come from `pub use ::gpui::*;` in gpui-kit; `GKT/assets.rs:1` imports `gpui_kit::AssetSource`.) Alternatively, without rust-embed: `include_bytes!` constants matched by path, as the `icon_assets!` macro does (`GKA/src/lib.rs:71-78`).

### Surprises (Q16-18)
1. There is no separate component `Root`: `gpui_component::Root` IS `gpui_base::Root` (`GC/lib.rs:97`). Dialog/notification/sheet layers come from a Root *plugin* registered in `gpui_component::init`; windows opened before `init` panic on `open_dialog`/`push_notification` (`GC/root.rs:474-475`). No `Root::render_dialog_layer` calls needed (unlike older gpui-component versions).
2. The Root plugin calls `window.set_rem_size(cx.theme().font_size)` every frame (`GC/root.rs:436`): every `rems`-based helper (`h_5`, `px_2`, `text_xs`, ...) inside the window scales with `Theme::font_size`. Changing the theme font size changes all component metrics.
3. `Input::h(..)` / `Input::h_full()` are inherent and multi-line only (`GC/input/input.rs:287-297`, `:780-783`); they shadow `Styled::h`. Use `.h_6()` or `Styled::h(input, px(24.))`.
4. Calling `.hover(..)` on a gpui-component `Button` trips gpui's `debug_assert!(... "hover style already set")` (`gpui-pre-0.3.8/src/elements/div.rs:843-846`) because Button sets its own hover (`GC/button/button.rs:713`). Use `ButtonCustomVariant`.
5. `Theme::global_mut` edits do not propagate to `theme.tokens`, the Base scrollbar/resizer projection, or trigger a redraw; use `Theme::update` (`GC/theme/mod.rs:241-288`). `Theme::change` reloads the registered config and can overwrite radius/fonts (`GC/theme/schema.rs:1085-1105`).
6. `gpui_kit_assets::Assets::load` returns `Err` for unknown paths (`GKA/src/native_assets.rs:27-29`), while `icon_assets!` sources return `Ok(None)`. Put `Assets` last in a combined source.
7. Default `Assets` only embeds the ~100-icon component subset; `IconName` (from `gpui_kit::assets`) enumerates all 1,830 icons, so `IconName::Foo` can compile but fail to load at runtime unless `AllAssets` or `icon_assets!` provides it.
8. `Input::context_menu` uses `NativeMenu` (`GC/input/input.rs:371-374`), not `PopupMenu`; the generic `.context_menu(..)` on divs (ContextMenuExt) uses `PopupMenu`.
9. `TitleBar::window_options()` sets `app_owns_titlebar_drag: true` (`GC/title_bar.rs:81-90`), a gpui-pre `WindowOptions` field that upstream Zed's gpui doesn't have.
10. `Dialog`/`Sheet` builder closures are `Fn` and are re-run every render (`GC/window_ext.rs:31-33`, stored as `Rc<dyn Fn>` `GC/root.rs:47-52`). Capture clones and avoid creating entities inside them.

## E. Sanity

### 19. Examples to crib from

Note: the published `gpui-component` crate has no `examples/` (the gallery `story` crate is not on crates.io); `gpui-pre-0.3.8/examples/*.rs` are present but use `gpui::`/`gpui_platform::` paths (`use gpui_platform::application;`) - with gpui-kit replace by `use gpui_kit::*;` and `gpui_kit::application()`.

**Custom titlebar (+ tabs)**
- `gpui-component-0.7.1/src/title_bar.rs` - `TitleBar` (RenderOnce): `TITLE_BAR_HEIGHT = px(34.)` (`:15`), macOS left padding `px(80.)` for traffic lights (`:17`), `TitleBar::title_bar_options()` -> `TitlebarOptions { title: None, appears_transparent: true, traffic_light_position: Some(point(px(9.0), px(9.0))) }` (`:59-65`), `TitleBar::window_options()` sets `app_owns_titlebar_drag: true` (`:81-91`), render (`:319-403`) shows drag via `on_mouse_down` flag + `on_mouse_move -> window.start_window_move()` (`:352-371`), `.window_control_area(WindowControlArea::Drag)` (`:379`), double-click -> `window.titlebar_double_click()` on macOS (`:347-349`). Children are arbitrary (`.children(self.children)` `:398`), so a `TabBar` can be placed inside.
- Tabs: `gpui-component-0.7.1/src/tab/tab_bar.rs` (TabBar; in-file tests/usages `:609`, `:681`, `:740` "scrolling-tabs"), `gpui-component-0.7.1/src/tab/tab.rs`; real usage in Dock: `gpui-component-0.7.1/src/dock/tab_panel.rs:469` (`TabBar::new("tab-bar")`); kit test `gpui-kit-0.7.1/tests/controls.rs:53` (`TabBar::new("settings-tabs")`).
- Plain gpui: `gpui-pre-0.3.8/examples/window_movable.rs` (TitlebarOptions + dragging), `gpui-pre-0.3.8/examples/window_shadow.rs` (client-side decorations, resize edges), `gpui-pre-0.3.8/examples/image_gallery.rs:260-266` (`titlebar: Some(TitlebarOptions { appears_transparent: false, .. })`).

**Virtual list with custom row painting**
- `gpui-pre-0.3.8/examples/uniform_list.rs:14-41` - minimal `uniform_list("entries", 50, cx.processor(|_this, range, _window, _cx| ...))`.
- `gpui-pre-0.3.8/examples/data_table.rs` - `uniform_list(...)` (`:432`) + `.track_scroll(&self.scroll_handle)` (`:447`) with `UniformListScrollHandle` (`:262,272`), plus a custom scrollbar painted with `canvas(...)` (`:326-370`) that reads `scroll_handle.0.borrow().base_handle.bounds()/offset()` (`:282-290`) and calls `set_offset`.
- `gpui-pre-0.3.8/examples/list_example.rs` - variable-height `list()` + `ListState`/`ListAlignment`.
- Custom Element painting shaped text: `gpui-pre-0.3.8/examples/input.rs:422` (`impl Element for TextElement`), `:506` `window.text_system().shape_line(display_text, font_size, &runs, None)`, `:562,578` `window.paint_quad(..)` for selection/cursor.
- `gpui-pre-0.3.8/examples/painting.rs:359-364` - `canvas(...)` + `window.paint_quad(quad(...))`; `gpui-pre-0.3.8/examples/text_layout.rs`.
- Component level: `gpui-component-0.7.1/src/table/state.rs:2495` (Table uses `uniform_list`), `gpui-base-0.7.1/src/virtual_list.rs` (`impl Element` for variable-size virtual list), `gpui-base-0.7.1/src/input/base/element.rs:2470` (`impl<M: InputModeKind> Element for TextElement<M>` - production-grade text painting incl. line shaping, selection, scrolling).

**Window bounds**
- `gpui-pre-0.3.8/examples/hello_world.rs:100-114` (`Bounds::centered(None, size(px(500.), px(500.0)), cx)`, `WindowBounds::Windowed(bounds)`, then `cx.activate(true)`).
- `gpui-pre-0.3.8/examples/window.rs:319-330` (`cx.observe_window_bounds(window, |_, window, _| window.bounds())`), `:34-35`, many `WindowKind`/`WindowBounds` variants `:98-242`.
- `gpui-pre-0.3.8/examples/window_positioning.rs` (displays, explicit origins).
- Kit headless test windows: `gpui-kit-0.7.1/tests/window.rs` (+ `tests/common/mod.rs`), geometry assertions via `window.find("id").bounds()`.

### 20. Surprises vs upstream Zed gpui (cross-cutting; per-area lists are at the end of each section)

1. **No `Application::new()`** - only `Application::with_platform(Rc<dyn Platform>)` (`gpui-pre-0.3.8/src/app.rs:178`). Use `gpui_kit::application()` (= `gpui_platform::application()`, `gpui-pre-platform-0.3.8/src/gpui_platform.rs:13`). Platform code is split into `gpui-pre-platform` / `gpui-pre-macos` / `gpui-pre-apple` crates.
2. **Proc-macros rewrite `gpui::` -> `gpui_kit::`** when the consuming crate depends on `gpui-kit` (`gpui-pre-macros-0.3.8/src/gpui_pre_facade_paths.rs:11-67`). Do not add a separate `gpui` dependency; write `use gpui_kit::*;`. `gpui_kit::gpui` is the kit itself (`gpui-kit-0.7.1/src/lib.rs:98-99`).
3. **Glob collision `Size`**: `use gpui_kit::*; use gpui_kit::component::*;` makes `Size` ambiguous (gpui `Size<T>` struct vs component `Size` enum) - verified by compiling `/tmp/rogg-research/scratch/examples/globs.rs`: `error[E0659]: 'Size' is ambiguous`. Import component items explicitly or refer to `gpui_kit::Size` / `gpui_kit::component::Size`.
4. **Quit behaviour**: `QuitMode::Default` = `Explicit` on macOS (`app.rs:400-408`), so closing the last window does not quit. Use `.with_quit_mode(QuitMode::LastWindowClosed)`.
5. **`runtime_shaders` is forced on by gpui-kit** (`gpui-kit-0.7.1/Cargo.toml:401-409`) - no Xcode `metal` needed, shaders compiled at runtime each launch.
6. **AccessKit built in**: gpui re-exports `accesskit`, `Role`, `Toggled`, `AccessibleAction` (`gpui-pre-0.3.8/src/gpui.rs:97-99`); `Application::new_inaccessible` exists (`app.rs:195`); base `Root::new` patches NSWindow `accessibilityHitTest:` on macOS (`gpui-base-0.7.1/src/root.rs:148-149`). gpui-kit's test helpers read element identity/bounds via accessibility properties.
7. **`gpui_kit::open_window` returns `(AnyWindowHandle, Entity<V>)`** and always wraps in `gpui_base::Root` (`gpui-kit-0.7.1/src/lib.rs:148-164`), unlike upstream gpui-component where you wrapped `Root::new(view.into(), window, cx)` yourself.
8. `WindowOptions` has a fork-specific `app_owns_titlebar_drag` field (used at `gpui-component-0.7.1/src/title_bar.rs:88`).
9. Inspector is auto-initialized in debug builds by `gpui_component::init` (`#[cfg(any(feature = "inspector", debug_assertions))] inspector::init(cx);`, `gpui-component-0.7.1/src/lib.rs:131-132`).
10. Other per-area traps (details/citations in the section Surprises lists): `FocusHandle::focus(&self, window, cx)` takes `cx` (Q12); `Menu` has a `disabled` field -> use `Menu::new(..).items(..)` (Q12); spawn closures are `async move |this, cx|` AsyncFn closures (Q13); `BoxShadow` has `inset` (Q8); `LineWrapper::wrap_line` has an extra `IndentAdjustment` arg (Q10); `Hitbox::is_hovered` is false after keyboard input (Q10); `debug_selector` is a no-op unless `cfg(test)` or `debug_assertions && test-support` (Q15); `WindowControlArea::Drag` is a no-op on macOS (Q6); `Input::h()` shadows `Styled::h` (Q17); `Assets::load` returns `Err` on missing paths -> put it last in a combined AssetSource (Q18); `IconName` (full catalog) can name icons not embedded in the default `Assets` bundle (Q18); calling `.hover()` on a component `Button` hits a debug_assert (Q16); `.context_menu(..)` wraps the element in `ContextMenu<E>` which has no `debug_selector` - call such methods before it (verified by compile error).

## Appendix: compiled API-check program (`/tmp/rogg-research/scratch/examples/apicheck.rs`)

Compiles with `gpui-kit = "=0.7.1"`, `raw-window-handle = "0.6"`, `objc2 = "0.6"`; runs (window opens, no panic). Not every code path is exercised at runtime (e.g. `on_find` is only bound), but every signature type-checks.

```rust
use gpui_kit::*;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable, Theme, ThemeMode, WindowExt, TitleBar,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    input::{Input, InputEvent, InputState},
    notification::Notification,
    tab::{Tab, TabBar},
    tooltip::Tooltip,
    menu::ContextMenuExt,
    switch::Switch,
    checkbox::Checkbox,
};
use std::time::Duration;

actions!(rogg, [Quit, Find, NextMatch]);

struct Rows { search: Entity<InputState>, scroll: UniformListScrollHandle, focus: FocusHandle, _subs: Vec<Subscription>, count: usize, task: Option<Task<()>> }

impl Rows {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let sub = cx.subscribe_in(&search, window, |this: &mut Self, state: &Entity<InputState>, ev: &InputEvent, _w, cx| {
            match ev { InputEvent::Change => { let _v: SharedString = state.read(cx).value(); this.count += 1; cx.notify(); }
                       InputEvent::PressEnter { .. } => {}, _ => {} }
        });
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(10)).await;
            let n = cx.background_spawn(async move { 1000usize }).await;
            this.update(cx, |this, cx| { this.count = n; cx.notify(); }).ok();
        });
        Self { search, scroll: UniformListScrollHandle::new(), focus: cx.focus_handle(), _subs: vec![sub], count: 100, task: Some(task) }
    }
    fn on_find(&mut self, _: &Find, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |s, cx| s.focus(window, cx));
        self.scroll.scroll_to_item_strict(50, ScrollStrategy::Center);
        window.push_notification(Notification::new().message("hi").autohide(true), cx);
        window.open_dialog(cx, |dialog, _w, _cx| dialog.title("T").child("body"));
    }
}

struct RowEl { text: SharedString }
impl IntoElement for RowEl { type Element = Self; fn into_element(self) -> Self { self } }
impl Element for RowEl {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;
    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }
    fn request_layout(&mut self, _id: Option<&GlobalElementId>, _iid: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = px(16.).into();
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(&mut self, _id: Option<&GlobalElementId>, _iid: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, _cx: &mut App) -> Option<Hitbox> {
        Some(window.insert_hitbox(bounds, HitboxBehavior::Normal))
    }
    fn paint(&mut self, _id: Option<&GlobalElementId>, _iid: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), hitbox: &mut Option<Hitbox>, window: &mut Window, cx: &mut App) {
        let hovered = hitbox.as_ref().map(|h| h.is_hovered(window)).unwrap_or(false);
        window.paint_quad(fill(bounds, if hovered { rgba(0xffffff22) } else { rgba(0x00000000) }));
        let run = TextRun { len: self.text.len(), font: font("Menlo"), color: rgb(0xdddddd).into(), background_color: None, underline: None, strikethrough: None };
        let line = window.text_system().shape_line(self.text.clone(), px(12.), &[run], None);
        line.paint(bounds.origin, px(16.), TextAlign::Left, None, window, cx).ok();
        window.on_mouse_event(|_e: &MouseDownEvent, _phase, _w, _cx| {});
    }
}

impl Render for Rows {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = window.appearance();
        div().size_full().flex().flex_col().track_focus(&self.focus).key_context("LogView")
            .on_action(cx.listener(Self::on_find))
            .font_family(cx.theme().mono_font_family.clone())
            .child(TitleBar::new().child(TabBar::new("tabs").selected_index(0).child(Tab::new().label("a.log")).on_click(cx.listener(|_, _ix: &usize, _, _| {}))))
            .child(div().h(px(24.)).child(Styled::h(Input::new(&self.search).xsmall().appearance(false).cleanable(true).prefix(Icon::new(IconName::Search).size(px(12.))).text_size(px(10.)), px(24.))))
            .child(Button::new("b").ghost().compact().label("x").tooltip("tip"))
            .child(Button::new("c").custom(ButtonCustomVariant::new(cx).color(rgb(0x333333).into()).hover(rgb(0x444444).into())).label("y"))
            .child(Switch::new("s").checked(true)).child(Checkbox::new("k").checked(false))
            .child(svg().path("icons/search.svg").size(px(15.)).text_color(rgb(0xffffff)))
            .child(div().id("hov").debug_selector(|| "hov".into()).hover(|s| s.bg(rgba(0xffffff10))).cursor_pointer().on_click(|_, _, _| {})
                .on_mouse_down(MouseButton::Middle, |_, _, _| {})
                .tooltip(|w, cx| Tooltip::new("t").build(w, cx))
                .context_menu(|menu, _, _| menu.menu("Find", Box::new(Find)).separator())
                .font_weight(FontWeight(550.)).line_height(px(14.)).shadow(vec![BoxShadow::new(px(0.), px(1.), hsla(0., 0., 0., 0.3)).blur_radius(px(4.))]).opacity(0.9))
            .child(uniform_list("rows", self.count, cx.processor(|_this, range: std::ops::Range<usize>, _w, _cx| {
                range.map(|ix| RowEl { text: format!("line {ix}").into() }).collect::<Vec<_>>()
            })).track_scroll(&self.scroll).flex_1())
    }
}

fn main() {
    gpui_kit::application().with_assets(gpui_kit::assets::Assets).with_quit_mode(QuitMode::LastWindowClosed).run(|cx| {
        gpui_kit::init(cx);
        cx.bind_keys([KeyBinding::new("cmd-f", Find, Some("LogView")), KeyBinding::new("cmd-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_menus(vec![Menu::new("rogg").items([MenuItem::action("Find", Find), MenuItem::separator(), MenuItem::action("Quit", Quit)])]);
        Theme::change(ThemeMode::Dark, None, cx);
        Theme::update(cx, |t| { t.radius = px(4.); t.font_size = px(13.); t.mono_font_family = "Menlo".into(); t.colors.background = rgb(0x1e1e1e).into(); });
        let opts = WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds::new(point(px(100.), px(100.)), size(px(900.), px(600.))))), window_min_size: Some(size(px(400.), px(300.))), ..TitleBar::window_options() };
        let (_h, view) = gpui_kit::open_window(opts, cx, |window, cx| cx.new(|cx| Rows::new(window, cx))).unwrap();
        let _ = view;
        _h.update(cx, |_, window, _| { use raw_window_handle::{HasWindowHandle, RawWindowHandle}; if let Ok(h) = HasWindowHandle::window_handle(window) { if let RawWindowHandle::AppKit(a) = h.as_raw() { unsafe { let v = a.ns_view.as_ptr() as *mut objc2::runtime::AnyObject; let w: *mut objc2::runtime::AnyObject = objc2::msg_send![v, window]; let n: isize = objc2::msg_send![w, windowNumber]; println!("WINDOW {n}"); } } } }).unwrap();
        cx.activate(true);
    });
}
```
