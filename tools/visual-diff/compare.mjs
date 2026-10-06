import fs from 'node:fs';
import path from 'node:path';
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';
import { OUT, PROBES, SCALE, captureNames } from './config.mjs';
import { writeReport } from './report.mjs';

const readPng = (f) => PNG.sync.read(fs.readFileSync(f));
const readGeometry = (f) => JSON.parse(fs.readFileSync(f, 'utf8')).elements;
const pixel = (png, x, y) => {
  const xi = Math.min(png.width - 1, Math.max(0, Math.round(x * SCALE)));
  const yi = Math.min(png.height - 1, Math.max(0, Math.round(y * SCALE)));
  const i = (yi * png.width + xi) * 4;
  return [png.data[i], png.data[i + 1], png.data[i + 2]];
};
const BORDER_NUDGE = 0.25; // logical px, keeps 1 px border probes inside the border after device-pixel snapping
const hex = (c) => '#' + c.map((v) => v.toString(16).padStart(2, '0')).join('');
const distance = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

fs.rmSync(path.join(OUT, 'diff'), { recursive: true, force: true });
fs.mkdirSync(path.join(OUT, 'diff'), { recursive: true });
const results = [];
for (const name of captureNames()) {
  const ref = readPng(path.join(OUT, 'reference', `${name}.png`));
  const gal = readPng(path.join(OUT, 'gallery', `${name}.png`));
  const result = { name, sizeMismatch: ref.width !== gal.width || ref.height !== gal.height };
  if (!result.sizeMismatch) {
    const diff = new PNG({ width: ref.width, height: ref.height });
    const count = pixelmatch(ref.data, gal.data, diff.data, ref.width, ref.height, { threshold: 0.1, includeAA: false, alpha: 0.25 });
    fs.writeFileSync(path.join(OUT, 'diff', `${name}.png`), PNG.sync.write(diff));
    result.diffPercent = (100 * count) / (ref.width * ref.height);
  }
  const rg = readGeometry(path.join(OUT, 'reference', `${name}.json`));
  const gg = readGeometry(path.join(OUT, 'gallery', `${name}.json`));
  result.geometry = Object.keys(rg)
    .filter((k) => k in gg)
    .map((k) => {
      const a = rg[k];
      const b = gg[k];
      const d = { dx: b.x - a.x, dy: b.y - a.y, dw: b.w - a.w, dh: b.h - a.h };
      return { key: k, ref: a, gallery: b, ...d, max: Math.max(...Object.values(d).map(Math.abs)) };
    })
    .sort((a, b) => b.max - a.max);
  result.missing = Object.keys(rg).filter((k) => !(k in gg));
  // A probe runs only when its element exists in BOTH geometries, and each image is sampled at that image's own
  // element rect, so geometry offsets are not conflated with color errors. 1 px borders (fy = 0) are nudged inward.
  result.skipped = PROBES.filter((p) => !rg[p.key] || !gg[p.key]).map((p) => ({
    name: p.name,
    key: p.key,
    reason: !rg[p.key] && !gg[p.key] ? 'missing in both' : !rg[p.key] ? 'missing in reference' : 'missing in gallery',
  }));
  const at = (r, p) => [r.x + r.w * p.fx, r.y + r.h * p.fy + (p.fy === 0 ? BORDER_NUDGE : 0)];
  result.probes = PROBES.filter((p) => rg[p.key] && gg[p.key])
    .map((p) => {
      const [rx, ry] = at(rg[p.key], p);
      const [gx, gy] = at(gg[p.key], p);
      const a = pixel(ref, rx, ry);
      const b = pixel(gal, gx, gy);
      return {
        name: p.name,
        refAt: [+rx.toFixed(2), +ry.toFixed(2)],
        galleryAt: [+gx.toFixed(2), +gy.toFixed(2)],
        ref: hex(a),
        gallery: hex(b),
        distance: +distance(a, b).toFixed(1),
      };
    })
    .sort((a, b) => b.distance - a.distance);
  results.push(result);
}
fs.writeFileSync(path.join(OUT, 'results.json'), JSON.stringify(results, null, 2));
writeReport(results);
console.table(
  results.map((r) => ({
    capture: r.name,
    'diff %': r.sizeMismatch ? 'SIZE MISMATCH' : r.diffPercent.toFixed(2),
    'worst geometry Δ px': r.geometry[0] ? `${r.geometry[0].key} ${r.geometry[0].max.toFixed(1)}` : '-',
    'worst probe Δ': r.probes[0] ? `${r.probes[0].name} ${r.probes[0].distance}` : '-',
    'missing keys': r.missing.length,
    'skipped probes': r.skipped.length,
  })),
);
console.log(`report: ${path.join(OUT, 'report.html')}`);
