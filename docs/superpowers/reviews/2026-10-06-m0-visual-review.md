# M0 visual review — gallery replica vs reference

Report: `target/visual/report.html` (regenerate with `just visual`)
Live: `cargo run -p rogg-gallery -- --page replica`, next to `docs/superpowers/specs/assets/rogg-main-window.html`

## Summary per capture

Copied from the `npm run compare` console table of the final run (after all fixes below). "Worst geometry" is the
largest |Δx|, |Δy|, |Δw| or |Δh| of any compared element; "worst probe" is the largest RGB distance of any color probe.

| Capture | Pixel diff % | Worst geometry Δ | Worst probe Δ | Notes |
|---|---|---|---|---|
| dark-compact | 3.95 | find-bottom 100.1 | error row tint (top) 1 | find-bottom: "Open as tab" (accepted). Next worst: token-3 11.7 (monospace advance, see below). Missing keys = top rows 15–19 (virtualized, off-screen). |
| dark-compact--all-tabs | 3.12 | find-bottom 100.1 | chip 0 fill 1.7 | Dialog Δy −0.3, Δh +0.1. |
| dark-compact--profile | 2.91 | find-bottom 100.1 | chip 0 fill 1.7 | Dialog Δy −0.1, Δh +0.2. |
| dark-compact--appearance | 3.19 | find-bottom 100.1 | filter band bg 41.7 | Probe lands on a dialog rule divider that sits ≤1 px apart (rows 43.5 vs 44 px). Dialog Δh −1.9. |
| dark-compact--shortcuts | 3.30 | find-bottom 100.1 | filter band bg 41.7 | Same divider effect (rows 33.5 vs 34 px). Dialog Δh −2.9. |
| dark-compact--history | 3.25 | find-bottom 100.1 | filter band bg 41.7 | Same divider effect. Dialog Δh +1.1. |
| dark-comfortable | 3.86 | find-bottom 100.1 | error row tint (top) 1 | As dark-compact; "selected row (top)" probe skipped (row clipped at this density). |
| dark-comfortable--all-tabs | 3.14 | find-bottom 100.1 | chip 0 fill 1.7 | |
| dark-comfortable--profile | 3.02 | find-bottom 100.1 | chip 0 fill 1.7 | |
| dark-comfortable--appearance | 3.29 | find-bottom 100.1 | filter band bg 41.7 | Divider effect. |
| dark-comfortable--shortcuts | 3.40 | find-bottom 100.1 | filter band bg 41.7 | Divider effect. |
| dark-comfortable--history | 3.30 | find-bottom 100.1 | filter band bg 41.7 | Divider effect. |
| light-compact | 3.86 | find-bottom 100.1 | backdrop 1.7 | |
| light-compact--all-tabs | 3.15 | find-bottom 100.1 | chip 0 fill 5.2 | |
| light-compact--profile | 2.96 | find-bottom 100.1 | chip 0 fill 5.8 | |
| light-compact--appearance | 3.25 | find-bottom 100.1 | filter band bg 53.8 | Divider effect. |
| light-compact--shortcuts | 3.39 | find-bottom 100.1 | filter band bg 53.8 | Divider effect. |
| light-compact--history | 3.27 | find-bottom 100.1 | filter band bg 53.8 | Divider effect. |
| light-comfortable | 3.75 | find-bottom 100.1 | backdrop 1.7 | |
| light-comfortable--all-tabs | 3.11 | find-bottom 100.1 | chip 0 fill 5.2 | |
| light-comfortable--profile | 3.00 | find-bottom 100.1 | chip 0 fill 5.8 | |
| light-comfortable--appearance | 3.27 | find-bottom 100.1 | filter band bg 53.8 | Divider effect. |
| light-comfortable--shortcuts | 3.40 | find-bottom 100.1 | filter band bg 53.8 | Divider effect. |
| light-comfortable--history | 3.26 | find-bottom 100.1 | filter band bg 53.8 | Divider effect. |

Before this pass the pixel diff was 3.79–7.98 % (window captures 4.13–4.55 %), the all-tabs dialog was 119 px too
tall (a reference capture bug, see below) and probes reached 207.9.

Excluding find-bottom, every remaining geometry delta above 2 px is a chip or `.*`/`Aa` flag position or width, all
caused by the monospace advance difference described under "Accepted differences". Every probe in the window captures
is within 5.8 RGB distance.

## Fixed during the review
- Title bar tab — active accent underline — invisible (gpui paints a border over its children) → drawn on the bottom border as in the reference (98d4747)
- Window root — line height — gpui default 1.618 → CSS `normal` ≈ 1.2; recent rows 34 → 31 px, Profile/Export 28 → 23/25 px, History 22 → 18 px, filter band 80 → 78 px (36d8b14)
- Title bar sidebar toggle — size — 25 px title-actions variant → 27 px `.icon-btn` (647835e)
- Sidebar folder row — label overflow / hover — overflowing, no hover → truncates, hover background (32cb986)
- Context select — text color / size / fill / width / caret — 13 px bright text, 80 % alpha fill, 56 px → 10 px muted text, opaque `var(--bg)`, widest-option width (≈66 px), heavy caret (bfb4f5c)
- Filter chip state segment — width — 1 px narrow → includes the transparent button border (eed26e7)
- Filter Apply button — hover — none → `var(--hover)` background (f3f1ec5)
- Action button — non-primary text color — text → muted (40cf1de)
- Info bar / pane header / status bar — text wrapping — could wrap → nowrap, crumbs and pane title ellipsize, right groups and progress track keep size (d8bcce8)
- Filter band — resize handle — top border cut through the handle → handle drawn over the border (4da1a53)
- Reference capture (tools) — all-tabs dialog — captured with an empty list → filled via `showTabPicker()` (3be146b)
- Log view — text clip — unwrapped text cut 20 px before the edge → runs to the row edge like `.line{overflow:hidden}` (f87e7ac)
- Find field — width without a query — 143 → 148 px (the reference keeps the empty `.found` gap) (3c55ad0)
- Sidebar FILES / RECENT labels — letter spacing — none (−4.3 px) → 1 px tracking emulated per glyph (+1.5 px) (32db4fa)
- Tab picker rows — height / icon color — 2 px short each (dialog −5.9 px), muted icon → transparent button border, icon in text color (dialog +0.1 px) (1aa4c96)

## Accepted differences
- Text rasterization: Chromium vs CoreText anti-aliasing (expected, not fixable). — accepted (user decisions 2026-10-07)
- Font weights 550/650 render as the nearest installed SF Pro face. — accepted (user decisions 2026-10-07)
- "Open as tab" tool button in the filtered pane header (spec §5.3 addition); it shifts the bottom find field 100 px left (find-bottom Δx −100.1). — accepted (user decisions 2026-10-07)
- Monospace font: headless Chromium resolves the reference's `"SFMono-Regular", Consolas, monospace` to a fallback (CDP reports Courier: unslashed zero, 7.2 px advance at 12 px), while rogg uses the system monospace (SF Mono, spec Appearance "System monospace · 12 px"). Log columns sit 7–14 px further right, chips are 1–4 px wider (token-3 Δx 11.7 accumulated), `.*`/`Aa` flags 1.4 px wider, shortcut keys differ in shape. — accepted by user 2026-10-07 (keep SF Mono)
- Gap rows ("··· 4 lines omitted"): Chromium renders `9px -apple-system` there as Helvetica; rogg uses the system UI font, so the label is about 10 % wider. — accepted by user 2026-10-07 (keep system UI font)
- Brand "rogg" `letter-spacing:-.7px` is not applied: gpui has no letter spacing and per-glyph emulation does not narrow it (glyph boxes round up), so the brand is about 2 px wider. FILES / RECENT use per-glyph tracking and end up about 1.5 px wider than the reference. — accepted by user 2026-10-07
- Scrollbar thumb: the gallery draws the thin `#4b545d` thumb whenever a pane overflows (spec §5.5); headless Chromium uses hidden macOS overlay scrollbars, so the reference shows none (visible in the top pane and in the comfortable bottom pane). — accepted by user 2026-10-07 (keep always-visible thumb)
- Line-box rounding: Chromium rounds `line-height: normal` per font (ascent and descent separately), gpui uses 1.2 em: the Profile button is 23 vs 24 px, dialog rule rows 43.5 vs 44 px and shortcut rows 33.5 vs 34 px (dialogs end up −2.9 to +1.1 px). The dialog-capture "filter band bg" probes (41.7 / 53.8) sample a dialog divider at these sub-pixel offsets, not the band. — accepted (user decisions 2026-10-07)
- Traffic lights are drawn only for the capture; the native window shows the real ones. — accepted (user decisions 2026-10-07)
- Missing keys: the reference renders all 40 top-pane rows, the virtualized gallery only the visible ones (rows 15–19 compact, 12–19 comfortable are off-screen). Not a visual difference.
- Not a difference (checked): the dialog backdrop matches the reference pixel for pixel in both themes (`#0007`; dark backdrop #0d0f11 vs #0d0f11, light #868685 vs #868685). The weak dark / strong light contrast is how the reference itself looks. The "⌄" after "Profile: Services" and "History" is equally small in the reference (same glyph and size). Timestamps are already faint (`.timestamp{color:var(--faint)}`).

## Accepted additions
- Sidebar folder rows have a hover background and 4 px rounding; the reference has none (`.folder` is a plain div). Kept by user decision 2026-10-07.

## User decisions (2026-10-07)
1. Log monospace: keep SF Mono. 2. Scrollbars: keep the always-visible thin thumb. 3. Gap-row label: keep the system UI font. 4. Brand / label letter-spacing approximation: acceptable. 5. Folder-row hover: keep.

## Open questions for the user (answered above)
1. Log monospace font: keep SF Mono (the system monospace named in the spec, slashed zero, slightly wider), or match the Courier-like fallback the reference shows in Chromium? Recommendation: keep SF Mono.
2. Scrollbars: keep the always-visible thin thumb (spec §5.5), or behave like macOS overlay scrollbars (thumb only while scrolling), which is what the browser shows?
3. Gap-row label font: keep the system UI font, or switch to Helvetica to match the Chromium rendering of the reference?
4. Brand letter spacing (about 2 px) and per-glyph tracking on FILES / RECENT: is the current approximation acceptable?

## Approval

Approved by the user on 2026-10-07 ("Approved"), after answering the five open questions as recorded above. Golden set: 9 captures in tools/visual-diff/golden/ (tolerance 0.05 % each, tightened from 0.5 % at the final branch review because captures are deterministic at 0.000 %; `approve.mjs` writes 0.05 too); check with `just visual-check`.

`just visual-check` on a fresh re-capture of HEAD: all 9 captures at 0.000 % (dark-compact, dark-compact--all-tabs, --profile, --appearance, --shortcuts, --history, dark-comfortable, light-compact, light-comfortable), all ✓. `just test` green.
