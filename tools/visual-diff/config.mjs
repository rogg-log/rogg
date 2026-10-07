import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
export const REFERENCE_HTML = path.join(ROOT, 'docs/superpowers/specs/assets/rogg-main-window.html');
// `ROGG_VISUAL_PAGE=app` compares the reference with the real app (rogg --demo) instead of the gallery replica.
export const PAGE = process.env.ROGG_VISUAL_PAGE ?? 'replica';
export const OUT = path.join(ROOT, PAGE === 'replica' ? 'target/visual' : `target/visual-${PAGE}`);
export const GALLERY_BIN = path.join(ROOT, 'target/debug/rogg-gallery');
export const GOLDEN = path.join(ROOT, 'tools/visual-diff/golden');

export const SIZE = { width: 1440, height: 900 };
export const SCALE = 2;

export const VARIANTS = [
  { theme: 'dark', density: 'compact' },
  { theme: 'dark', density: 'comfortable' },
  { theme: 'light', density: 'compact' },
  { theme: 'light', density: 'comfortable' },
];

// Reference <dialog> id → gallery --dialog slug, and the pages that have it (profile and settings are M4 in the app).
export const DIALOGS = [
  { html: 'tabs-dialog', gallery: 'all-tabs', pages: ['replica', 'app'] },
  { html: 'profile-dialog', gallery: 'profile', pages: ['replica'] },
  { html: 'settings-dialog', gallery: 'appearance', pages: ['replica'] },
  { html: 'shortcuts-dialog', gallery: 'shortcuts', pages: ['replica', 'app'] },
  { html: 'history-dialog', gallery: 'history', pages: ['replica', 'app'] },
].filter((d) => d.pages.includes(PAGE));

export const variantName = (v) => `${v.theme}-${v.density}`;
export const captureNames = () => VARIANTS.flatMap((v) => [variantName(v), ...DIALOGS.map((d) => `${variantName(v)}--${d.gallery}`)]);

// Geometry key → reference selector. `many` expands to key-0, key-1, …; `offset` shifts the index.
export const SELECTORS = [
  { key: 'titlebar', sel: '.titlebar' },
  { key: 'titlebar-leading', sel: '.titlebar-leading' },
  { key: 'tab-shell', sel: '.tab-shell', many: true },
  { key: 'title-actions', sel: '.title-actions' },
  { key: 'sidebar', sel: '.sidebar' },
  { key: 'side-top', sel: '.side-top' },
  { key: 'tree-row-0', sel: '.side-body .folder' },
  { key: 'tree-row', sel: '#file-list .file', many: true, offset: 1 },
  { key: 'recent', sel: '.recent', many: true },
  { key: 'side-bottom', sel: '.side-bottom' },
  { key: 'meta', sel: '.meta' },
  { key: 'profile', sel: '.meta .right button:first-child' },
  { key: 'export', sel: '.meta .right button:last-child' },
  { key: 'pane-top-head', sel: '.pane.top .pane-head' },
  { key: 'find-top', sel: '.pane.top .find' },
  { key: 'tool-wrap-top', sel: '#wrap-top' },
  { key: 'log-top', sel: '#log-top' },
  { key: 'filter-band', sel: '.filter-band' },
  { key: 'token', sel: '.token', many: true },
  { key: 'token-state', sel: '.token .state', many: true },
  { key: 'filter-actions', sel: '.filter-actions' },
  { key: 'edit-filters', sel: '#edit-button' },
  { key: 'flag-regex', sel: '#regex' },
  { key: 'flag-case', sel: '#case' },
  { key: 'filter-bottom', sel: '.filter-bottom' },
  { key: 'history', sel: '.filter-bottom > button' },
  { key: 'pane-bottom-head', sel: '.pane.bottom .pane-head' },
  { key: 'find-bottom', sel: '.pane.bottom .find' },
  { key: 'tool-follow', sel: '#follow' },
  { key: 'tool-wrap-bottom', sel: '#wrap-bottom' },
  { key: 'log-bottom', sel: '#log-bottom' },
  { key: 'row-top', sel: '#log-top .row', many: true, limit: 20 },
  { key: 'row-bottom', sel: '#log-bottom .row', many: true },
  { key: 'gap-bottom', sel: '#log-bottom .gap-row', many: true },
  { key: 'statusbar', sel: '.footer' },
  { key: 'dialog', sel: 'dialog[open]' },
];

// Color probes: a point at fraction (fx, fy) of a reference element's rect, compared in both images.
export const PROBES = [
  { name: 'title bar bg', key: 'titlebar', fx: 0.62, fy: 0.12 },
  { name: 'active tab bg', key: 'tab-shell-0', fx: 0.97, fy: 0.15 },
  { name: 'sidebar bg', key: 'sidebar', fx: 0.04, fy: 0.75 },
  { name: 'active file row', key: 'tree-row-1', fx: 0.6, fy: 0.15 },
  { name: 'info bar bg', key: 'meta', fx: 0.55, fy: 0.2 },
  { name: 'pane header bg', key: 'pane-top-head', fx: 0.4, fy: 0.2 },
  { name: 'find field bg', key: 'find-top', fx: 0.9, fy: 0.25 },
  { name: 'filter band bg', key: 'filter-band', fx: 0.3, fy: 0.2 },
  { name: 'filter band top border', key: 'filter-band', fx: 0.3, fy: 0.0 },
  { name: 'chip 0 fill', key: 'token-0', fx: 0.92, fy: 0.12 },
  { name: 'chip 3 (off) fill', key: 'token-3', fx: 0.92, fy: 0.12 },
  { name: 'regex flag border', key: 'flag-regex', fx: 0.5, fy: 0.0 },
  { name: 'follow (pressed) fill', key: 'tool-follow', fx: 0.05, fy: 0.5 },
  // Row probes sample the left gutter (pin column, never text), because the gallery's vertical scrollbar thumb and
  // overflowing text both occupy the right edge.
  // Top-pane rows 0-11 are visible at both densities. The selected row (top, 12) is clipped when comfortable, so it is
  // skipped there; the selected row is also probed in the bottom pane (row-bottom-1 is line 120492).
  { name: 'selected row (top)', key: 'row-top-12', fx: 0.01, fy: 0.5 },
  { name: 'selected row (bottom)', key: 'row-bottom-1', fx: 0.01, fy: 0.5 },
  { name: 'error row tint (top)', key: 'row-top-6', fx: 0.01, fy: 0.5 },
  { name: 'gap row bg', key: 'gap-bottom-0', fx: 0.9, fy: 0.5 },
  { name: 'status bar bg', key: 'statusbar', fx: 0.5, fy: 0.5 },
  { name: 'dialog bg', key: 'dialog', fx: 0.5, fy: 0.03 },
  { name: 'backdrop', key: 'statusbar', fx: 0.02, fy: 0.5 },
];
