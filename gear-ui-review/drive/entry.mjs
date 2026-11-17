// Tries every Edit entry point on the open design's gear set and reports what each opens.
// node entry.mjs <shot-prefix>
import { connect, sleep } from './sc.mjs';
const sc = await connect(7980);
const prefix = process.argv[2] ?? 'entry';
const state = async () => { const d = await sc.dialog(); const c = d?.commands?.[0]; return d ? `${d.title}: ${c?.command} group=${c?.params?.group ?? '-'} items=${c?.params?.items?.length ?? '-'}` + (d.error ? ` error=${d.error}` : '') : 'no dialog'; };
const close = async () => { await sc.call('ui.key', { key: 'Escape' }); await sleep(600); if (await sc.dialog()) { await sc.call('ui.key', { key: 'Escape' }); await sleep(600); } };
const doc = await sc.call('document.inspect', { timeline: 'summary', bodies: 'names' });
const feat = (doc.timeline ?? []).find((f) => f.type === 'GearsFeature');
const body = (doc.bodies ?? []).map((b) => b.name ?? b).find((n) => /Gear/.test(n));
console.log('feature', JSON.stringify(feat), 'body', body);
const out = {};
await close();
// 1. Timeline row double-click.
let h = (await sc.handles('timeline:')); h = Array.isArray(h) ? h : h.handles ?? [];
const row = h.find((x) => x.name === `timeline:${feat.name}`) ?? h.find((x) => x.name.startsWith('timeline:') && x.name.includes(feat.name));
console.log('timeline handles', JSON.stringify(h.map((x) => x.name)).slice(0, 600));
if (row) { await sc.click(row.x, row.y, { double: true }); await sleep(2500); out.timeline_row = await state(); await sc.shot(`${prefix}-timeline-row`); await close(); }
const folder = h.find((x) => x.name.startsWith('timeline-folder:'));
if (folder) { await sc.click(folder.x, folder.y, { double: true }); await sleep(2500); out.folder = await state(); await sc.shot(`${prefix}-folder`); await close(); }
// 2. Body selected + Gears.
if (body) { await sc.exec('select.set', { items: [{ type: 'body', name: body }] }); await sleep(500); await sc.call('ui.start', { command: 'gear.create' }); await sleep(2500); out.body_and_gears = await state(); await sc.shot(`${prefix}-body-gears`); await close(); }
// 3. Viewport double-click on a gear face.
await sc.exec('select.set', { items: [] });
const pix = await sc.call('ui.inspect', {});
const vr = pix.viewport ?? pix.viewport_rect;
console.log('viewport', JSON.stringify(vr).slice(0, 200));
// Find a gear face: scan a grid.
let hit = null;
const cx = vr ? (vr.min?.[0] ?? vr.x ?? 400) : 400, cy = vr ? (vr.min?.[1] ?? vr.y ?? 250) : 250;
for (let y = 300; y < 900 && !hit; y += 60) for (let x = 600; x < 1300 && !hit; x += 60) {
  await sc.move(x, y); const ins = await sc.call('ui.inspect', {}); if (ins.hover?.type === 'face') hit = [x, y, ins.hover.body];
}
console.log('face at', JSON.stringify(hit));
if (hit) { await sc.click(hit[0], hit[1], { double: true }); await sleep(2500); out.viewport_double_click = await state(); await sc.shot(`${prefix}-viewport-dbl`); await close(); }
// 4. Edit Gears on the folder's right-click menu.
if (folder) { await sc.click(folder.x, folder.y, { button: 'right' }); await sleep(800); const m = await sc.call('ui.menu', {}); console.log('menu', JSON.stringify(m?.items?.map((i) => i.label)).slice(0, 300)); try { await sc.call('ui.menuPick', { item: 'Edit Gears' }); await sleep(2500); out.edit_gears_menu = await state(); } catch (e) { out.edit_gears_menu = 'menu: ' + e.message; } await close(); }
console.log(JSON.stringify(out, null, 1));
sc.close();
