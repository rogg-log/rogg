# M0 → M2 carry-over

Items found in M0 reviews that belong to M2 (main window wiring). Each was deliberately left out of M0. Fold them into the M2 plan.

## Log view interaction (rogg-ui `logview`)
- Draggable vertical scrollbar thumb. Today a click where the thumb is drawn selects a row. Also add a horizontal scrollbar when wrap is off.
- `LogViewState` API additions:
  - ensure-visible / scroll-to-row (spec §5.6, PgUp/PgDn, ⌘↑/⌘↓);
  - get/set of the scroll anchor, for per-tab state and session restore;
  - keep the anchor on the same line number across `set_rows`, so filter changes don't jump;
  - stick-to-bottom for live follow;
  - bound the wrapped-height cache, which grows with every visited row.
- Selection currently happens on mouse-down. Drag and shift-click range selection need separate down/up handling (spec §5.5). Also needs copy and the context menu.
- Wrap mode: `center_on_row` uses the estimated height and is not re-centred after measurement.
- Hover changes call `window.refresh()`; `cx.notify()` on the state would be narrower.
- `LogViewElement` relies on the `LogView` wrapper's overflow mask for hitbox clipping. Keep using the wrapper, or make the element `pub(crate)`.
- `LogViewState` is not a view. Don't wrap panes in cached views without notifying a view.
- Span producers (M1 engine) may emit byte offsets that aren't on char boundaries. `clip_spans` now snaps them, so this is safe but widens the highlight to the whole character.

## Layout and chrome
- Filter-band handle drag resizes the panes (spec §5.3). `MainWindowParts::split` already exists.
- Sidebar resize handle (155–360 px).
- Title bar:
  - set `app_owns_titlebar_drag: true` and handle double-click zoom (`titlebar_double_click`);
  - make the leading block's empty area draggable;
  - set the native traffic-light position.
- Tabs:
  - show `TabItem.tooltip` on hover;
  - reveal the close button on keyboard focus;
  - scroll the active tab into view.
- Any new window root (app root, welcome view) must set `theme::LINE_HEIGHT`, or text gets gpui's 1.618 default.

## Keyboard and focus
- Focus rings (2 px accent, 2 px offset).
- Dialogs close on Esc. Enter applies the filter expression, Escape cancels.
- The full keymap from spec §5.9.

## Dialogs and controls
- `ShortcutRow` / `PickerItem` constructors for use outside the crate.
- Indeterminate progress currently shows a static 30 % bar.

## Tooling
- The real-text timing guard (`crates/rogg-ui/tests/logview_real_text.rs`, 100 ms) has under 2× headroom in debug wrap mode. Relax it to about 300 ms if it flakes in CI.
- `just visual` doesn't bootstrap a fresh clone (`npm ci`, `npx playwright install chromium`). Golden coverage is the replica only: wrap/edit/search/empty states aren't guarded yet.
- Build the app with `-p rogg-app`, because the gallery enables gpui-kit `test-support` workspace-wide.
