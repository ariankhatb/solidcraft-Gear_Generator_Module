// Final round screenshots (sg-final, port 7980): a spur gear with bore and hub taken from the
// drawing's suggestions, its folded card, three gears on one axis with two suggested shaft
// links, a bevel side view picked apex first, and 09 opened and edited.
import path from 'node:path';
import * as w from './wt.mjs';
const which = process.argv[2] ?? 'all';

if (which === 'all' || which === 'spur') {
  // QA 3: picking A, then B: each takes the bore and hub circles drawn on its centre by itself;
  // A folds to "z20 m2 · bore 8 · hub 16". QA 2: clicking a card brings its settings into view.
  const sc = await w.begin('01-spur-pair.json', { zoom: 1.6 });
  await w.gears(sc);
  await w.pick(sc, [0, 20, 0]);              // A's pitch circle: its bore and hub come with it
  await w.sleep(2500);
  await sc.shot('final-01-spur-bore-hub-by-themselves');
  await w.pick(sc, [50, 30, 0]);             // B: A folds
  await w.sleep(2500);
  await sc.shot('final-02-folded-card-bore-hub');
  console.log(JSON.stringify((await w.request(sc))?.params?.items?.map((i) => i.components.map((c) => c.slot))));
  await sc.clickHandle('gear:card:A');
  await w.sleep(1500);
  await sc.shot('final-qa2-card-click-shows-settings');
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
  await w.sleep(2500);
  let h = [];
  for (let i = 0; i < 8 && !h.length; i++) { await sc.call('ui.scroll', { x: 1300, y: 600, delta: -300 }); await w.sleep(300); h = await sc.handles('dialog:section:Drivetrain'); h = Array.isArray(h) ? h : h.handles ?? []; }
  if (h[0]) { await sc.call('ui.scroll', { x: 1300, y: 600, delta: -(h[0].y - 330) }); await w.sleep(700); }
  await sc.shot('final-03-three-gears-one-axis-two-links');
  console.log(JSON.stringify(((await sc.dialog())?.commands?.[0]?.params?.links) ?? []));
  sc.close();
}

if (which === 'all' || which === 'bevel') {
  // QA 9: a side view with no contact line, clicked at the corner, then the other line.
  const sc = await w.begin(null);
  await sc.exec('sketch.create', { plane: { origin: [0, 0, 0], x_dir: [1, 0, 0], y_dir: [0, 0, 1] }, name: 'Side view' });
  await sc.exec('sketch.point', { point: [0, 0], id: 'apex' });
  await sc.exec('sketch.line', { points: [[0, 0], [0, 60]], ids: ['s1'] });
  await sc.exec('sketch.line', { points: [[0, 0], [60, 0]], ids: ['s2'] });
  await sc.exec('sketch.finish', {});
  await w.frame(sc, 'front', 1.6);
  await w.gears(sc);
  await w.chooseType(sc, 'Bevel Pair');
  await w.newGear(sc);
  await w.pick(sc, [0, 0, 0]);               // the corner
  await w.sleep(800);
  await sc.shot('final-04a-bevel-corner-first');
  // The other line: whichever the corner did not take.
  const picks = async () => ((await w.request(sc))?.params?.items?.[0]?.components ?? []).map((c) => c.curve);
  const first = await picks();
  console.log('corner took', JSON.stringify(first));
  await w.pick(sc, first.includes('s2') ? [0, 0, 40] : [40, 0, 0]);
  await w.sleep(1500);
  console.log('then', JSON.stringify(await picks()));
  await sc.shot('final-04b-bevel-side-view-from-two-lines');
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

if (which === 'all' || which === 'qa') {
  // QA 1: Edit opens at the top. QA 4: two tangent gears not linked yet: the suggestion, no collision.
  const sc = await w.begin(null);
  await sc.call('ui.file', { path: path.resolve('..', '..', 'examples', 'gears', '09-everything-drivetrain.sc') });
  for (let i = 0; i < 90; i++) { await w.sleep(1000); if (!(await sc.call('ui.inspect', {})).file_busy) break; }
  await w.sleep(3000);
  await w.frame(sc, 'iso', 1.1);
  const f = await w.gearsFeature(sc);
  await sc.call('ui.editFeature', { feature: f.id });
  await w.sleep(3000);
  await sc.shot('final-qa1-edit-opens-at-top');
  await sc.call('ui.key', { key: 'Escape' });
  sc.close();
  const s2 = await w.begin(null);
  await s2.exec('sketch.create', { plane: 'XY', name: 'Pair' });
  await s2.exec('sketch.circle.center', { center: [0, 0], radius: 20, id: 'a' });
  await s2.exec('sketch.circle.center', { center: [50, 0], radius: 30, id: 'b' });
  await s2.exec('sketch.finish', {});
  await w.frame(s2, 'top', 1.6);
  await w.gears(s2);
  await w.pick(s2, [0, 20, 0]);
  await w.pick(s2, [50, 30, 0]);
  await w.sleep(2000);
  await s2.call('ui.scroll', { x: 1300, y: 600, delta: -300 });
  await w.sleep(800);
  await s2.shot('final-qa4-suggestion-not-collision');
  s2.close();
}

if (which === 'all' || which === 'chamfer') {
  // The tooth chamfer: one setting under All gears; 01 edited to a 0.6 mm chamfer on both faces.
  const sc = await w.begin(null);
  await sc.call('ui.file', { path: path.resolve('..', '..', 'examples', 'gears', '01-spur-pair.sc') });
  for (let i = 0; i < 60; i++) { await w.sleep(1000); if (!(await sc.call('ui.inspect', {})).file_busy) break; }
  await w.sleep(2500);
  const f = await w.gearsFeature(sc);
  await sc.call('ui.editFeature', { feature: f.id });
  await w.sleep(3000);
  const row = async () => { let a = await sc.handles('dialog:row:Tooth chamfer'); a = Array.isArray(a) ? a : a.handles ?? []; return a[0]; };
  for (let k = 0; k < 12; k++) {
    const h = await row();
    if (h && h.y > 300 && h.y < 700) break;
    await sc.call('ui.scroll', { x: 1300, y: 700, delta: h ? -(h.y - 500) / 2 : -250 });
    await w.sleep(400);
  }
  await w.typeInto(sc, 'Tooth chamfer', '0.6 mm');
  await w.sleep(2500);
  await sc.shot('final-chamfer-setting');
  await w.ok(sc);
  await w.sleep(2000);
  for (const folder of ['sketches', 'joints']) await sc.exec('browser.visibility', { visible: false, folder });
  await sc.exec('browser.visibility', { visible: false, items: ['Housing cutout'] });
  await sc.call('ui.view', { view: 'iso' });
  const cam = await sc.call('ui.camera', { fit: true });
  if (cam?.distance) await sc.call('ui.camera', { distance: cam.distance * 0.55 });
  await w.sleep(1500);
  await sc.shot('final-chamfer-pair');
  // Only the chamfer changed: the gears are made again (no body kept from before).
  const vol = async () => (await sc.exec('inspect.center_of_mass', { bodies: ['Gear A·1', 'Gear B·1'] })).bodies.map((b) => b.volume);
  const v06 = await vol();
  const gs = await sc.exec('gear.inspect', {}).catch((e) => e.message);
  console.log('set request chamfer', JSON.stringify(JSON.stringify(gs).match(/tooth_chamfer[^}]*}/)?.[0] ?? null));
  for (const size of ['0.3 mm', '0.6 mm', '']) {
    await sc.call('ui.editFeature', { feature: f.id });
    await w.sleep(3000);
    for (let k = 0; k < 12; k++) {
      const h = await row();
      if (h && h.y > 300 && h.y < 700) break;
      await sc.call('ui.scroll', { x: 1300, y: 700, delta: h ? -(h.y - 500) / 2 : -250 });
      await w.sleep(400);
    }
    await w.typeInto(sc, 'Tooth chamfer', size || ' ');
    await w.sleep(1500);
    console.log('asked', JSON.stringify((await w.request(sc))?.params?.tooth_chamfer ?? null));
    await w.ok(sc);
    await w.sleep(1500);
    console.log('chamfer', size || 'off', JSON.stringify(await vol()), '(0.6 mm first:', JSON.stringify(v06), ')');
  }
  const doc = await sc.call('document.inspect', { timeline: 'summary', bodies: 'names' });
  console.log('errors', (doc.timeline ?? []).filter((x) => x.error).map((x) => x.name + ': ' + x.error));
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
  const sec = async () => { let a = await sc.handles('dialog:section:Housing'); a = Array.isArray(a) ? a : a.handles ?? []; return a[0]; };
  for (let k = 0; k < 10; k++) {
    const h = await sec();
    console.log('section y', h?.y);
    if (!h || (h.y > 380 && h.y < 520)) break;
    await sc.call('ui.scroll', { x: 1300, y: 700, delta: -(h.y - 450) / 2 });
    await w.sleep(500);
  }
  { let all = await sc.handles(''); all = Array.isArray(all) ? all : all.handles ?? []; const open = all.some((x) => /Make a housing|gear_cutout_bearing|Bearing/.test(x.name)); console.log('open', open); if (!open) { await sc.clickHandle('dialog:section:Housing cutout'); await w.sleep(1000); } }
  await sc.shot('final-07-housing-cutout-section');
  console.log(JSON.stringify((await w.request(sc))?.params?.clearance_body));
  sc.close();
}
