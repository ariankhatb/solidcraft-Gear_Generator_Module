// More Edit entry points: the folder (Edit Folder), the folder's right-click Edit Gears, a feature
// the set made (BodyTransfer row), a component occurrence selected + Gears, the timeline feature's
// right-click menu.
import { connect, sleep } from './sc.mjs';
const sc = await connect(7980);
const prefix = process.argv[2] ?? 'after-entry';
const state = async () => { const d = await sc.dialog(); const c = d?.commands?.[0]; return d ? `${d.title}: ${c?.command} group=${c?.params?.group ?? '-'} items=${c?.params?.items?.length ?? '-'}` + (d.error ? ` error=${d.error}` : '') : 'no dialog'; };
const close = async () => { for (let i = 0; i < 2; i++) { if (await sc.dialog()) { await sc.call('ui.key', { key: 'Escape' }); await sleep(600); } } };
const out = {};
const doc = await sc.call('document.inspect', { timeline: 'summary', bodies: 'names' });
const feat = (doc.timeline ?? []).find((f) => f.type === 'GearsFeature');
const groups = doc.timeline_groups ?? doc.groups ?? [];
console.log('groups', JSON.stringify(groups).slice(0, 300));
await close();
// Folder by id/name (ui.editFolder = the folder's Edit).
for (const g of groups) { try { await sc.call('ui.editFolder', { folder: g.id ?? g.name }); await sleep(2000); out[`editFolder ${g.name}`] = await state(); await close(); } catch (e) { out[`editFolder ${g.name}`] = e.message; } }
// A feature the set made (BodyTransfer).
let h = await sc.handles('timeline:'); h = Array.isArray(h) ? h : h.handles ?? [];
const bt = h.find((x) => x.name === 'timeline:BodyTransfer1');
if (bt) { await sc.click(bt.x, bt.y, { double: true }); await sleep(2500); out.body_transfer_row = await state(); await sc.shot(`${prefix}-bodytransfer`); await close(); }
// Right-click on the Gears feature row, Edit.
const row = h.find((x) => x.name === `timeline:${feat.name}`);
if (row) {
  await sc.call('ui.menu', { x: row.x, y: row.y, target: { type: 'timeline', feature: feat.id } });
  await sleep(700);
  const m = await sc.call('ui.menu', {});
  const labels = (m?.items ?? []).map((i) => i.label);
  console.log('menu', JSON.stringify(labels).slice(0, 400));
  const pick = labels.find((l) => /Edit Gears|Edit Feature|^Edit$/.test(l));
  if (pick) { await sc.call('ui.menuPick', { item: pick }); await sleep(2500); out[`menu ${pick}`] = await state(); await sc.shot(`${prefix}-menu-edit`); await close(); }
  else await sc.call('ui.menu', { close: true });
}
// An occurrence selected + Gears.
const occ = await sc.exec('document.inspect', {}).catch(() => null);
const comps = (await sc.call('document.inspect', { timeline: false, bodies: false }))?.occurrences ?? [];
console.log('occ', JSON.stringify(comps).slice(0, 200));
if (comps[0]) { try { await sc.exec('select.set', { items: [{ type: 'occurrence', path: [comps[0].id ?? comps[0]] }] }); await sleep(400); await sc.call('ui.start', { command: 'gear.create' }); await sleep(2500); out.occurrence_and_gears = await state(); await close(); } catch (e) { out.occurrence_and_gears = e.message; } }
// A gear component activated (as a double-click did before), then Gears with nothing selected.
console.log(JSON.stringify(out, null, 1));
sc.close();
