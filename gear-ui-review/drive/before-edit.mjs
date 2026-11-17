// The old dialog editing a set made with gear items (examples-defined/03-compound-train.sc).
import path from 'node:path';
import { connect, sleep } from './sc.mjs';
const sc = await connect(7980);
await sc.call('ui.key', { key: 'Escape' });
const file = path.resolve('..', 'examples', process.argv[2] ?? '03-compound-train.sc');
console.log(JSON.stringify(await sc.call('ui.file', { path: file })).slice(0, 200));
for (let i = 0; i < 60; i++) { await sleep(1000); const s = await sc.call('ui.inspect', {}); if (!s.file_busy) break; }
await sleep(2000);
const doc = await sc.call('document.inspect', { timeline: 'summary', bodies: 'names' });
const gears = (doc.timeline ?? []).filter((f) => /gear/i.test(f.type ?? '') || /gear/i.test(f.name ?? ''));
console.log(JSON.stringify(gears).slice(0, 400));
await sc.call('ui.view', { view: 'iso' }); await sc.call('ui.camera', { fit: true });
const f = gears[gears.length - 1];
console.log(JSON.stringify(await sc.call('ui.editFeature', { feature: f.id })).slice(0, 300));
await sleep(2500);
await sc.shot(process.argv[3] ?? 'before-05-editing');
console.log(JSON.stringify(await sc.dialog()).slice(0, 600));
sc.close();
