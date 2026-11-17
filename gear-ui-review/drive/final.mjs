// Final round screenshots (sg-final, port 7980): a spur gear with bore and hub taken from the
// drawing's suggestions, its folded card, three gears on one axis with two suggested shaft
// links, a bevel side view picked apex first, and 09 opened and edited.
import path from 'node:path';
import * as w from './wt.mjs';
const which = process.argv[2] ?? 'all';

if (which === 'all' || which === 'spur') {
  const sc = await w.begin('01-spur-pair.json', { zoom: 1.6 });
  await w.gears(sc);
  await w.pick(sc, [0, 20, 0]);              // A's pitch circle
  await w.sleep(1500);
  let h = await sc.handles('gear:use:'); h = Array.isArray(h) ? h : h.handles ?? [];
  console.log('use buttons', h.map((x) => x.name));
  await sc.shot('final-01a-spur-bore-hub-suggested');
  for (const name of ['gear:use:A:0:bore', 'gear:use:A:0:hub']) { try { await sc.clickHandle(name); await w.sleep(1200); } catch (e) { console.log(name, e.message); } }
  await sc.shot('final-01b-spur-bore-hub-from-suggestion');
  await w.pick(sc, [50, 30, 0]);             // B: A folds
  await w.sleep(2000);
  await sc.shot('final-02-folded-card');
  console.log(JSON.stringify((await w.request(sc))?.params?.items?.map((i) => i.components.map((c) => c.slot))));
  sc.close();
}

if (which === 'all' || which === 'shaft') {
  const sc = await w.begin(null);
  for (const [z, r, id] of [[0, 10, 'a'], [15, 20, 'b'], [30, 12, 'c']]) {
    await sc.exec('sketch.create', { plane: { origin: [0, 0, z], x_dir: [1, 0, 0], y_dir: [0, 1, 0] }, name: `Level ${z}` });
    await sc.exec('sketch.circle.center', { center: [0, 0], radius: r, id });
    await sc.exec('sketch.finish', {});
  }
  await w.frame(sc, 'iso', 1.8);
  await w.gears(sc);
  for (const [z, r] of [[0, 10], [15, 20], [30, 12]]) await w.pick(sc, [0, -r, z]);
  await w.sleep(1500);
  await sc.call('ui.scroll', { x: 1300, y: 600, delta: -400 });
  await w.sleep(600);
  await sc.shot('final-03-three-gears-one-axis-two-links');
  sc.close();
}

if (which === 'all' || which === 'bevel') {
  const sc = await w.begin('04-bevel-side-view.json', { view: 'front', zoom: 1.6 });
  await w.gears(sc);
  await w.chooseType(sc, 'Bevel Pair');
  await w.newGear(sc);
  await w.pick(sc, [0, 0, 0]);               // the apex first
  await w.sleep(800);
  await sc.shot('final-04a-bevel-apex-first');
  await w.pick(sc, [0, 0, -40]);
  await w.pick(sc, [40, 0, 0]);
  await w.sleep(1500);
  await sc.shot('final-04b-bevel-side-view-complete');
  sc.close();
}

if (which === 'all' || which === 'edit09') {
  const sc = await w.begin(null);
  await sc.call('ui.file', { path: path.resolve('..', '..', 'examples', 'gears', '09-everything-drivetrain.sc') });
  for (let i = 0; i < 90; i++) { await w.sleep(1000); if (!(await sc.call('ui.inspect', {})).file_busy) break; }
  await w.sleep(3000);
  await w.frame(sc, 'iso', 1.1);
  await sc.shot('final-05a-09-opened');
  const f = await w.gearsFeature(sc);
  await sc.call('ui.editFeature', { feature: f.id });
  await w.sleep(3000);
  await sc.shot('final-05b-09-edit-opens-the-set');
  await sc.clickHandle('gear:card:C');
  await w.sleep(800);
  await w.typeInto(sc, 'Helix angle', '30 deg');
  await w.sleep(1500);
  await sc.shot('final-05c-09-gear-c-helix-changed');
  await w.ok(sc);
  await w.frame(sc, 'iso', 1.1);
  await sc.shot('final-05d-09-updated-in-place');
  const doc = await sc.call('document.inspect', { timeline: 'summary', bodies: 'names' });
  console.log('gears features', (doc.timeline ?? []).filter((x) => x.type === 'GearsFeature').map((x) => x.id), 'errors', (doc.timeline ?? []).filter((x) => x.error).map((x) => x.name));
  sc.close();
}

if (which === 'all' || which === 'cutout') {
  // 01 as built (bores, hub, shaft lines, bearing points and the housing cutout), then Edit: the
  // Housing cutout section and the seats in the preview.
  const sc = await w.begin(null);
  await sc.call('ui.file', { path: path.resolve('..', '..', 'examples', 'gears', '01-spur-pair.sc') });
  for (let i = 0; i < 60; i++) { await w.sleep(1000); if (!(await sc.call('ui.inspect', {})).file_busy) break; }
  await w.sleep(2500);
  await w.frame(sc, 'iso', 1.3);
  await sc.shot('final-06a-01-with-housing-cutout');
  const f = await w.gearsFeature(sc);
  await sc.call('ui.editFeature', { feature: f.id });
  await w.sleep(3000);
  await sc.clickHandle('gear:card:A');
  await w.sleep(1000);
  await sc.shot('final-06b-01-edit-card-with-bearing-point');
  await w.sleep(800);
  let h = await sc.handles('dialog:section:Housing'); h = Array.isArray(h) ? h : h.handles ?? [];
  if (h[0]) { await sc.call('ui.scroll', { x: 1300, y: 600, delta: -(h[0].y - 300) }); await w.sleep(600); }
  await sc.shot('final-07-housing-cutout-section');
  console.log(JSON.stringify((await w.request(sc))?.params?.clearance_body));
  sc.close();
}
