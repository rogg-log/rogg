import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { DIALOGS, GALLERY_BIN, OUT, ROOT, SIZE, VARIANTS, variantName } from './config.mjs';

execFileSync('cargo', ['build', '-p', 'rogg-gallery'], { cwd: ROOT, stdio: 'inherit' });
const dir = path.join(OUT, 'gallery');
fs.mkdirSync(dir, { recursive: true });
const capture = (file, v, extra = []) =>
  execFileSync(
    GALLERY_BIN,
    ['--capture', 'replica', '--theme', v.theme, '--density', v.density, '--size', `${SIZE.width}x${SIZE.height}`,
      '--out', path.join(dir, `${file}.png`), '--geometry', path.join(dir, `${file}.json`), ...extra],
    { stdio: 'inherit' },
  );
for (const v of VARIANTS) {
  const name = variantName(v);
  capture(name, v);
  for (const d of DIALOGS) capture(`${name}--${d.gallery}`, v, ['--dialog', d.gallery]);
  console.log(`gallery ${name} done`);
}
