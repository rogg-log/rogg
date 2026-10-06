import { chromium } from 'playwright';
import fs from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { DIALOGS, OUT, REFERENCE_HTML, SCALE, SELECTORS, SIZE, VARIANTS, variantName } from './config.mjs';

// Strip the "study" frame so .app fills the viewport like a native window.
const CSS = `.study{max-width:none!important;padding:0!important;margin:0!important}
.study-head,.study-foot{display:none!important}
.app{height:${SIZE.height}px!important;min-height:0!important;border:0!important;border-radius:0!important;box-shadow:none!important}
*:focus-visible{outline:none!important}`;

async function geometry(page) {
  return page.evaluate((selectors) => {
    const app = document.querySelector('.app').getBoundingClientRect();
    const rect = (n) => {
      const r = n.getBoundingClientRect();
      return { x: r.x - app.x, y: r.y - app.y, w: r.width, h: r.height };
    };
    const out = {};
    for (const s of selectors) {
      const nodes = [...document.querySelectorAll(s.sel)].filter((n) => n.getClientRects().length).slice(0, s.limit ?? 1000);
      if (s.many) nodes.forEach((n, i) => (out[`${s.key}-${i + (s.offset ?? 0)}`] = rect(n)));
      else if (nodes[0]) out[s.key] = rect(nodes[0]);
    }
    return out;
  }, SELECTORS);
}

const dir = path.join(OUT, 'reference');
await fs.rm(dir, { recursive: true, force: true });
await fs.mkdir(dir, { recursive: true });
const browser = await chromium.launch();
const context = await browser.newContext({ viewport: SIZE, deviceScaleFactor: SCALE });
const page = await context.newPage();
for (const v of VARIANTS) {
  await page.goto(pathToFileURL(REFERENCE_HTML).href);
  await page.addStyleTag({ content: CSS });
  await page.evaluate(({ theme, density }) => {
    document.body.classList.toggle('light', theme === 'light');
    setDensity(density === 'comfortable');
  }, v);
  await page.evaluate(() => document.fonts.ready);
  await page.mouse.move(SIZE.width - 2, SIZE.height - 2);
  const name = variantName(v);
  await page.locator('.app').screenshot({ path: path.join(dir, `${name}.png`) });
  await fs.writeFile(path.join(dir, `${name}.json`), JSON.stringify({ elements: await geometry(page) }, null, 2));
  for (const d of DIALOGS) {
    await page.evaluate((id) => {
      // The tab picker list is filled by showTabPicker(); a bare showModal() would capture it empty.
      if (id === 'tabs-dialog') showTabPicker();
      else document.getElementById(id).showModal();
      document.activeElement?.blur(); // showModal() autofocuses the first control
    }, d.html);
    const file = `${name}--${d.gallery}`;
    await page.screenshot({ path: path.join(dir, `${file}.png`), clip: { x: 0, y: 0, ...SIZE } });
    await fs.writeFile(path.join(dir, `${file}.json`), JSON.stringify({ elements: await geometry(page) }, null, 2));
    await page.evaluate((id) => document.getElementById(id).close(), d.html);
  }
  console.log(`reference ${name} done`);
}
await browser.close();
