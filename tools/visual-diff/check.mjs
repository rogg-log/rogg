import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';
import { GOLDEN, OUT, ROOT } from './config.mjs';

// Re-capture the gallery, then compare against the approved golden images.
execFileSync('node', [path.join(ROOT, 'tools/visual-diff/gallery.mjs')], { stdio: 'inherit' });
const tolerance = JSON.parse(fs.readFileSync(path.join(GOLDEN, 'tolerance.json'), 'utf8'));
let failed = 0;
for (const [name, maxPercent] of Object.entries(tolerance)) {
  const golden = PNG.sync.read(fs.readFileSync(path.join(GOLDEN, `${name}.png`)));
  const fresh = PNG.sync.read(fs.readFileSync(path.join(OUT, 'gallery', `${name}.png`)));
  if (golden.width !== fresh.width || golden.height !== fresh.height) {
    console.error(`✗ ${name}: size ${fresh.width}×${fresh.height}, golden ${golden.width}×${golden.height}`);
    failed++;
    continue;
  }
  const diff = new PNG({ width: golden.width, height: golden.height });
  const count = pixelmatch(golden.data, fresh.data, diff.data, golden.width, golden.height, { threshold: 0.1, includeAA: false });
  const percent = (100 * count) / (golden.width * golden.height);
  if (percent > maxPercent) {
    fs.mkdirSync(path.join(OUT, 'check'), { recursive: true });
    fs.writeFileSync(path.join(OUT, 'check', `${name}.png`), PNG.sync.write(diff));
    console.error(`✗ ${name}: ${percent.toFixed(2)}% differs (max ${maxPercent}%), diff in target/visual/check/${name}.png`);
    failed++;
  } else {
    console.log(`✓ ${name}: ${percent.toFixed(3)}%`);
  }
}
process.exit(failed ? 1 : 0);
