// Renders crates/rogg-app/assets/icon/rogg.svg (and rogg-small.svg for 16 and 32 px) at every size macOS wants, then
// packs them into rogg.icns with iconutil, writes rogg.png (256 px) for the Linux desktop entry and rogg.ico
// (16–256 px) for rogg.exe. Uses the Playwright Chromium from tools/visual-diff. Run via ./dev icon.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const { chromium } = await import(join(root, "tools/visual-diff/node_modules/playwright/index.mjs"));
const icons = join(root, "crates/rogg-app/assets/icon");
const full = readFileSync(join(icons, "rogg.svg"), "utf8");
const small = readFileSync(join(icons, "rogg-small.svg"), "utf8");

// iconutil's iconset: each point size at 1x and 2x. The two smallest renders use the simplified artwork.
const sizes = [16, 32, 128, 256, 512];
const set = mkdtempSync(join(tmpdir(), "rogg-")) + "/rogg.iconset";
execFileSync("mkdir", ["-p", set]);

const browser = await chromium.launch();
const page = await browser.newPage();
async function render(svg, px, file) {
  await page.setViewportSize({ width: px, height: px });
  await page.setContent(`<html><body style="margin:0;background:transparent">${svg.replace("<svg ", `<svg width="${px}" height="${px}" `)}</body></html>`);
  await page.screenshot({ path: join(set, file), omitBackground: true, clip: { x: 0, y: 0, width: px, height: px } });
}
for (const size of sizes) {
  await render(size <= 32 ? small : full, size, `icon_${size}x${size}.png`);
  await render(size < 32 ? small : full, size * 2, `icon_${size}x${size}@2x.png`);
}
// The Linux desktop entry's icon (install.sh), shipped in the Linux archives.
await page.setViewportSize({ width: 256, height: 256 });
await page.setContent(`<html><body style="margin:0;background:transparent">${full.replace("<svg ", `<svg width="256" height="256" `)}</body></html>`);
await page.screenshot({ path: join(icons, "rogg.png"), omitBackground: true, clip: { x: 0, y: 0, width: 256, height: 256 } });
// rogg.ico for Windows (rogg.exe's icon): PNG images in one ICO container; 16 and 24 px use the simplified artwork.
const icoSizes = [16, 24, 32, 48, 64, 256];
const pngs = [];
for (const size of icoSizes) {
  await page.setViewportSize({ width: size, height: size });
  await page.setContent(`<html><body style="margin:0;background:transparent">${(size <= 24 ? small : full).replace("<svg ", `<svg width="${size}" height="${size}" `)}</body></html>`);
  pngs.push(await page.screenshot({ omitBackground: true, clip: { x: 0, y: 0, width: size, height: size } }));
}
const ico = Buffer.alloc(6 + 16 * pngs.length);
ico.writeUInt16LE(0, 0);
ico.writeUInt16LE(1, 2);
ico.writeUInt16LE(pngs.length, 4);
let offset = ico.length;
pngs.forEach((png, i) => {
  const at = 6 + 16 * i;
  const side = icoSizes[i] >= 256 ? 0 : icoSizes[i];
  ico.writeUInt8(side, at);
  ico.writeUInt8(side, at + 1);
  ico.writeUInt16LE(1, at + 4);
  ico.writeUInt16LE(32, at + 6);
  ico.writeUInt32LE(png.length, at + 8);
  ico.writeUInt32LE(offset, at + 12);
  offset += png.length;
});
writeFileSync(join(icons, "rogg.ico"), Buffer.concat([ico, ...pngs]));
await browser.close();

const out = join(icons, "rogg.icns");
execFileSync("iconutil", ["-c", "icns", set, "-o", out]);
rmSync(dirname(set), { recursive: true, force: true });
console.log(out);
