// Walkthrough 7: edit a built set (03-compound-train built through the dialog in w2, or the
// examples file): Edit opens it with its gears; change gear A's module; OK updates in place.
// Then a circle is deleted from its sketch: Edit shows the gear's pick as missing (named), the
// status line asks for the replacement; a new circle is drawn and picked.
import path from 'node:path';
import * as w from './wt.mjs';
const sc = await w.begin(null);
await sc.call('ui.file', { path: path.resolve('..', 'examples', '01-spur-pair.sc') });
for (let i = 0; i < 60; i++) { await w.sleep(1000); if (!(await sc.call('ui.inspect', {})).file_busy) break; }
await w.sleep(2000);
await w.frame(sc, 'iso', 1.3);
const f = await w.gearsFeature(sc);
console.log('feature', JSON.stringify(f));
await sc.call('ui.editFeature', { feature: f.id });
await w.sleep(2500);
await sc.shot('w7-01-edit-opens-the-set');
await sc.clickHandle('gear:card:A');
await w.sleep(600);
await w.typeInto(sc, 'Module', '2.5 mm');
await w.sleep(1500);
await sc.shot('w7-02-module-changed');
console.log('cmd', JSON.stringify((await w.request(sc))?.command), JSON.stringify((await w.request(sc))?.params?.items?.[0]?.gears));
await w.ok(sc);
const doc = await sc.call('document.inspect', { timeline: 'summary', bodies: 'names' });
console.log('gears features after OK', (doc.timeline ?? []).filter((x) => x.type === 'GearsFeature').map((x) => x.id));
await sc.shot('w7-03-updated-in-place');
// Delete circle b from its sketch, then Edit again.
const sk = (doc.timeline ?? []).find((x) => x.type === 'Sketch' || /Pitch/.test(x.name));
console.log('sketch', JSON.stringify(sk));
try {
  await sc.exec('sketch.edit', { sketch: sk.id });
  await sc.exec('sketch.delete', { entities: ['b'] });
  await sc.exec('sketch.circle.center', { center: [55, 0], radius: 35, id: 'b2' });
  await sc.exec('sketch.finish', {});
} catch (e) { console.log('sketch edit', e.message); }
await w.sleep(2000);
await sc.call('ui.editFeature', { feature: f.id });
await w.sleep(2500);
await sc.shot('w7-04-missing-pick-named');
await sc.clickHandle('gear:card:B');
await w.sleep(500);
await w.frame(sc, 'top', 1.4);
await w.pick(sc, [55, 35, 0]);
await w.sleep(1500);
await sc.shot('w7-05-re-picked');
console.log('err', JSON.stringify((await sc.dialog())?.error));
await w.ok(sc);
await w.frame(sc, 'iso', 1.2);
await sc.shot('w7-06-updated');
sc.close();
