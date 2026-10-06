import fs from 'node:fs';
import path from 'node:path';
import { OUT } from './config.mjs';

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
const rect = (r) => `${r.x.toFixed(1)}, ${r.y.toFixed(1)} · ${r.w.toFixed(1)}×${r.h.toFixed(1)}`;

export function writeReport(results) {
  const sections = results
    .map((r) => {
      const geometry = r.geometry
        .slice(0, 40)
        .map((g) => `<tr class="${g.max > 2 ? 'bad' : g.max > 0.5 ? 'warn' : ''}"><td>${esc(g.key)}</td><td>${rect(g.ref)}</td><td>${rect(g.gallery)}</td><td>${g.dx.toFixed(1)}</td><td>${g.dy.toFixed(1)}</td><td>${g.dw.toFixed(1)}</td><td>${g.dh.toFixed(1)}</td></tr>`)
        .join('');
      const probes = r.probes
        .map((p) => `<tr class="${p.distance > 12 ? 'bad' : p.distance > 4 ? 'warn' : ''}"><td>${esc(p.name)}</td><td>${p.x}, ${p.y}</td><td><span class="sw" style="background:${p.ref}"></span>${p.ref}</td><td><span class="sw" style="background:${p.gallery}"></span>${p.gallery}</td><td>${p.distance}</td></tr>`)
        .join('');
      return `<section id="${esc(r.name)}"><h2>${esc(r.name)} <small>${r.sizeMismatch ? 'SIZE MISMATCH' : r.diffPercent.toFixed(2) + '% pixels differ'} · ${r.missing.length} geometry keys missing</small></h2>
<div class="overlay"><img src="reference/${r.name}.png"><img class="top" src="gallery/${r.name}.png" style="opacity:.5"></div>
<label>gallery opacity <input type="range" min="0" max="100" value="50" oninput="this.parentElement.previousElementSibling.querySelector('.top').style.opacity=this.value/100"></label>
<div class="grid"><figure><img src="reference/${r.name}.png"><figcaption>reference</figcaption></figure><figure><img src="gallery/${r.name}.png"><figcaption>gallery</figcaption></figure><figure><img src="diff/${r.name}.png"><figcaption>diff</figcaption></figure></div>
<h3>Geometry (largest deviation first, logical px)</h3><table><tr><th>key</th><th>reference</th><th>gallery</th><th>Δx</th><th>Δy</th><th>Δw</th><th>Δh</th></tr>${geometry}</table>
${r.missing.length ? `<p class="missing">Missing in gallery: ${r.missing.map(esc).join(', ')}</p>` : ''}
<h3>Color probes</h3><table><tr><th>probe</th><th>at</th><th>reference</th><th>gallery</th><th>Δ (RGB distance)</th></tr>${probes}</table></section>`;
    })
    .join('\n');
  const toc = results.map((r) => `<a href="#${esc(r.name)}">${esc(r.name)}</a>`).join(' · ');
  const html = `<!doctype html><meta charset="utf-8"><title>rogg visual diff</title>
<style>body{font:13px -apple-system,sans-serif;margin:24px;background:#111315;color:#e2e6e9}a{color:#a8c5b7}section{margin:40px 0;border-top:1px solid #34393f;padding-top:16px}
small{color:#929da7;font-weight:400}.overlay{position:relative;width:1440px;max-width:100%}.overlay img{width:100%;display:block}.overlay .top{position:absolute;inset:0}
.grid{display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-top:12px}.grid img{width:100%}figcaption{color:#929da7;font-size:11px}
table{border-collapse:collapse;margin:8px 0;font:11px ui-monospace,monospace}td,th{border:1px solid #34393f;padding:3px 6px;text-align:left}
tr.warn td{background:#443b28}tr.bad td{background:#49302f}.sw{display:inline-block;width:12px;height:12px;border:1px solid #fff3;margin-right:6px;vertical-align:-2px}.missing{color:#e0c283}</style>
<h1>rogg ⇄ reference</h1><p>${toc}</p>${sections}`;
  fs.writeFileSync(path.join(OUT, 'report.html'), html);
}
