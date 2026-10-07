import fs from 'node:fs';
import path from 'node:path';
import { GOLDEN, OUT, PAGE, captureNames } from './config.mjs';

// The goldens are replica-page captures: never approve or check against another page.
if (PAGE !== 'replica') {
  console.error(`approve/check only run on the replica page (ROGG_VISUAL_PAGE=${PAGE})`);
  process.exit(1);
}

// Golden set: all four window variants plus every dialog in dark-compact (keeps the repo small).
const names = captureNames().filter((n) => !n.includes('--') || n.startsWith('dark-compact--'));
fs.mkdirSync(GOLDEN, { recursive: true });
for (const n of names) fs.copyFileSync(path.join(OUT, 'gallery', `${n}.png`), path.join(GOLDEN, `${n}.png`));
const tolerance = Object.fromEntries(names.map((n) => [n, 0.05]));
fs.writeFileSync(path.join(GOLDEN, 'tolerance.json'), JSON.stringify(tolerance, null, 2));
console.log(`approved ${names.length} golden captures into ${GOLDEN}`);
