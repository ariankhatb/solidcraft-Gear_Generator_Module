// Walkthrough 6: several gears in one go. A sketch of four circles in a row (each touching the
// next): one box selection makes four Spur gears, the touching pairs are suggested as meshes,
// Accept, one OK builds one Gears feature.
import * as w from './wt.mjs';
const sc = await w.begin(null);
await sc.exec('sketch.create', { plane: 'XY', name: 'Row of gears' });
const circles = [[0, 20], [50, 30], [100, 20], [140, 20]];
circles.forEach(([x, r], i) => {});
for (const [i, [x, r]] of circles.entries()) await sc.exec('sketch.circle.center', { center: [x, 0], radius: r, id: `c${i + 1}` });
await sc.exec('sketch.finish', {});
await w.frame(sc, 'top', 1.4);
await w.gears(sc);
await sc.shot('w6-01-empty');
const a = await sc.toScreen([-35, 45, 0]); const b = await sc.toScreen([175, -45, 0]);
const xy = (p) => (Array.isArray(p) ? p : [p.x, p.y]);
const [x0, y0] = xy(a), [x1, y1] = xy(b);
await sc.call('ui.drag', { x0, y0, x1, y1, steps: 8 });
await w.sleep(1500);
await sc.shot('w6-02-box-selected-four-gears');
await sc.clickHandle('gear:accept');
await w.sleep(1000);
await sc.shot('w6-03-suggested-accepted');
const r = await w.request(sc);
console.log('items', r?.params?.items?.length, 'links', JSON.stringify(r?.params?.links));
await w.ok(sc);
await w.frame(sc, 'iso', 1.2);
await sc.shot('w6-04-built-one-feature');
const doc = await sc.call('document.inspect', { timeline: 'summary', bodies: 'names' });
console.log('gears features', (doc.timeline ?? []).filter((f) => f.type === 'GearsFeature').length, 'bodies', JSON.stringify(doc.bodies).slice(0, 300));
sc.close();
