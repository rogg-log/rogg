# M4 follow-ups (post-v1)

These are the Minor findings that the M4 task reviews and the final review deferred. None of them blocks v1. The final review triaged the whole list and found none that must be fixed before merge. The plan's "Deferred from M4" list keeps the larger post-v1 items.

## Behaviour
- **Session autosave cost:** autosave serialises (pretty JSON) and writes on the UI thread every 30 s. A derived tab of a live parent stores its explicit line list, so a very large one causes a short hitch. Fix: compact JSON or run-length lines, or write in the background (keep the quit save synchronous).
- **Startup parsing:** `session.json` is parsed twice at start (window frame, then tabs).
- **`.rogg` files:** they always open new tabs, even for files that are already open.
- **Fullscreen:** a fullscreen window restores as maximised.
- **Restored frames:** a frame that passes the 100×100 visibility check can still have its title bar above the screen; AppKit usually moves it down. A window on an external display that is still connected reopens centred on the primary display, because gpui reports all displays at (0, 0).
- **"Other…" log font picker:** it measures every installed family on the UI thread the first time it opens (plan-mandated).
- **Exports:**
  - A second export on the same tab silently cancels the first.
  - Core export checks for cancellation every 65,536 lines, so a short export can still finish after its tab closed.
  - The footer hides filtering progress while an export runs.
- **Live tabs:** a job started while an idle live tab sleeps is first polled up to 100 ms late.
- **Profile files:**
  - `ProfileStore::put` ignores a failed delete of the old file after a rename, so both files can remain.
  - `profile::delete` assumes the file name follows the profile name.
  - `load_dir` drops later duplicate names (and case-variant files) without reporting them, overwrites an existing `.bak`, and treats read errors as unparseable.
- **Settings:**
  - Negative or larger-than-u32 numbers in `settings.toml` make the whole file unparseable (reset to defaults plus `.bak`) instead of clamping.
  - `SettingsStore::update` rewrites the file even when nothing changed.
  - `cache_limit_bytes` does not saturate.
- **Session values:**
  - `Session::version` is never checked on load.
  - `DEFAULT_SPLIT` is duplicated from rogg-ui without a parity test.
- **Derived tabs:**
  - A derived tab built from an open tab keeps the parent's mapping alive after the parent closes, and the parent's cache lease is released with the parent.
  - A nested tab of a snapshot shows no Snapshot note.
- **Persistence and clock edge cases:**
  - `persist::write_atomic` leaves its `.partial` behind when `fs::write` fails mid-write (it is overwritten next time).
  - `clock::local_hms` maps a pre-epoch time to the epoch's local time.
- **Gallery:** `set_theme`/`set_density` to the already-stored value clears a gallery pin but leaves the pinned theme installed (gallery only).

## UI polish
- **Profile editor:**
  - The note ("Profiles are saved in …") wraps in the middle of the path.
  - The light-colour hint shows the derived value even when an override is set.
  - Unreadable light or slot colours are silently not applied, with no error state.
- **Settings and font picker:**
  - The font picker hides dot-named families, so SF Mono is never shown checked there.
  - Settings and the font picker share the dialog body's scroll id.
  - `open_dialog` keeps the Go-to-line subscription until close.
- **Form controls and rows:**
  - A disabled step button inside a rule row shows the row's pointer cursor.
  - `index_at` re-shapes rows without their bold runs. Hit-testing drifts only for fonts whose bold advances differ.
- **Theme and statistics:**
  - Kit `mono_font_size` is unscaled (unused).
  - Gallery literal text sizes are unscaled.
  - The statistics popover shows the Context and Pinned rows even at 0.
- **Code cleanup:**
  - `chip_color`'s `None` arm is unused and duplicates the slot lookup in `FilterChip::render`.
  - The step-callback wrapper closure is duplicated in `Stepper` and `EditableRuleRow`.
  - The `mono` variable in `logview/element.rs` `index_at` now holds the log font.
  - The gallery uses the full path `rogg_ui::dialogs::DialogFrame`.

## Tests
- **No test exists for:**
  - invalid UTF-8 in `load_toml` / `read_toml_strict`;
  - partial or missing `settings.toml`;
  - profile palette truncation and import name taken from the file stem;
  - duplicates, `Panes::includes`, `derive_light` on greys;
  - empty display, `^`-style line rules, bold union, `Both` panes and bold-only rules in `style`;
  - the nested-derived clamp, the `lines`/`snapshot` round trip and the version stamp in sessions;
  - mid-run cancellation and empty-line materialise;
  - the `InflatePoll::Running` wait branch;
  - the no-handler case of `on_stats`;
  - a render at a UI font size other than 13 px;
  - the enabled stepper-up path, `Swatch::on_click`, `TextField::error`;
  - thin Settings coverage: density, toggles, UI size, profile select, log-font select, the "Other…" path, ⌘, replacing a dialog;
  - swatch clicks clearing the colour error, and `slot-add` hiding at `MAX_PALETTE`;
  - a broken import leaving the profile list unchanged;
  - the derived-tab refusal toasts and demo crumb alignment;
  - `window_options`, an unknown profile on restore, and the quit path;
  - the empty-tab Statistics arm.
- **Weak assertions:**
  - The unreadable-profile and corrupt-session tests match a fragment of the toast, not the full copy.
  - The snapshot test checks only that the note is present.
  - The rules test's tint check could pass vacuously.
  - The idle-poll test asserts only `poll_interval`.
  - The mid-scan truncation test does not guarantee the scan is still running, and its 400k-line fixture is heavy.
  - The export-failure test assumes it does not run as root.
- **Process:** several tasks have no captured RED run (Tasks 8, 10, parts of 7 and 15).
- **Visual:** the app captures for the history and all-tabs dialogs moved slightly. The plain-app rise is attributed to the Services profile without being isolated.
